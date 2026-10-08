//! Refresh feedback on Windows: a click-through layered child window drawn as
//! a full-width bar along the top of the page's content rect. A refresh fades
//! it in with a band sweeping over a translucent base (breathing instead with
//! client-area animations off); the end, no earlier than [`MIN_VISIBLE_MS`]
//! after the start, turns it solid and fades it out. A start while it
//! finishes takes it straight back to running. Start and end are announced
//! through UI Automation.
//!
//! Every function runs on the host window's thread. Win32 calls are made only
//! once the [`bars`] guard is dropped, since they may send messages that
//! re-enter this module.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ffi::c_void;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HDC, HGDIOBJ,
    SelectObject,
};
use windows::Win32::System::LibraryLoader;
use windows::Win32::UI::Accessibility::{
    NotificationKind_ActionCompleted, NotificationKind_Other, NotificationProcessing_MostRecent,
    UiaClientsAreListening, UiaHostProviderFromHwnd, UiaRaiseNotificationEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    self, CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, SET_WINDOW_POS_FLAGS,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetWindowPos, UpdateLayeredWindow,
    WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_NOPARENTNOTIFY,
    WS_EX_TRANSPARENT,
};
use windows::core::{BSTR, PCWSTR, w};

/// Host-window timer driving every bar on that window; killed while none
/// needs frames.
pub(crate) const TIMER_ID: usize = 0x5A17;
/// About one frame per display refresh while a band sweeps.
const SWEEP_TICK_MS: u32 = 16;
/// Enough for fades and breathing.
const CALM_TICK_MS: u32 = 33;
const FADE_IN_MS: f32 = 120.0;
/// One pass of the band across the bar.
const SWEEP_MS: f32 = 1200.0;
/// Band width, as a share of the bar.
const BAND_SHARE: f32 = 0.35;
/// Each soft edge, as a share of the band.
const BAND_EDGE: f32 = 0.3;
/// Opacity of the base under the band.
const BASE_ALPHA: f32 = 0.4;
/// With animations off: full opacity down to [`BREATH_LOW`] and back once
/// per period.
const BREATH_MS: f32 = 1600.0;
const BREATH_LOW: f32 = 0.5;
/// Turning solid when done, then fading out.
const SOLID_MS: f32 = 150.0;
const FADE_OUT_MS: f32 = 250.0;
/// Measured from the start, so a refresh that ends at once still shows.
const MIN_VISIBLE_MS: f32 = 600.0;
/// Bar height in CSS px; scaled to the page.
const BAR_CSS_HEIGHT: f64 = 3.0;
const DEFAULT_RGB: u32 = 0x667085;
const CLASS_NAME: PCWSTR = w!("LingXiaRefreshBar");

/// What the bar shows at one moment.
#[derive(Clone, Copy, Debug)]
struct Frame {
    /// Fade in and out.
    envelope: f32,
    /// The breathing with animations off; 1 otherwise.
    breath: f32,
    /// `0` is the translucent base, `1` the full colour.
    solid: f32,
    /// Left edge of the band as a share of the width; `None` without animations.
    band: Option<f32>,
}

impl Frame {
    fn opacity(&self) -> f32 {
        self.envelope * self.breath
    }
}

/// Where a run fades in from: nothing, or what was on screen at a restart.
#[derive(Clone, Copy, Debug)]
struct Origin {
    opacity: f32,
    solid: f32,
}

impl Origin {
    fn fresh(animate: bool) -> Self {
        let solid = if animate { 0.0 } else { 1.0 };
        Self {
            opacity: 0.0,
            solid,
        }
    }
}

struct Bar {
    host: isize,
    window: isize,
    /// The band's clock. Kept across a restart, so the band never jumps.
    epoch: Instant,
    started: Instant,
    stopped: Option<Instant>,
    origin: Origin,
    /// Sampled when the run began.
    animate: bool,
    rgb: u32,
    /// Host client coordinates; empty while the page has no on-screen rect.
    rect: RECT,
    /// Top-left and top-right radii of the page surface under the bar.
    radii: [i32; 2],
    shown: bool,
    /// Hidden with its page off screen: no frames until a layout pass.
    parked: bool,
    /// The window's pixels; only while shown.
    surface: Option<Surface>,
    /// The start was announced, so the end is too.
    announced: bool,
}

impl Bar {
    fn new(
        host: HWND,
        window: isize,
        now: Instant,
        animate: bool,
        rgb: u32,
        announced: bool,
    ) -> Self {
        Bar {
            host: handle(host),
            window,
            epoch: now,
            started: now,
            stopped: None,
            origin: Origin::fresh(animate),
            animate,
            rgb,
            rect: RECT::default(),
            radii: [0; 2],
            shown: false,
            parked: false,
            surface: None,
            announced,
        }
    }

    fn frame(&self, now: Instant) -> Option<Frame> {
        frame(
            now - self.started,
            self.stopped.map(|stopped| stopped - self.started),
            self.origin,
            self.started - self.epoch,
            self.animate,
        )
    }

    /// Back to running after a stop, from what is on screen if it already
    /// began to finish. `None` if it was still running; else whether to
    /// announce the start.
    fn restart(&mut self, now: Instant, animate: bool, rgb: u32, visible: bool) -> Option<bool> {
        let stopped = self.stopped?;
        if is_completing(now - self.started, Some(stopped - self.started)) {
            self.origin = match self.frame(now) {
                Some(frame) => Origin {
                    opacity: frame.opacity(),
                    solid: frame.solid,
                },
                None => {
                    self.epoch = now;
                    Origin::fresh(animate)
                }
            };
            self.started = now;
            self.animate = animate;
            self.rgb = rgb;
        }
        self.stopped = None;
        self.parked = false;
        let announce = !self.announced && visible;
        self.announced |= announce;
        Some(announce)
    }

    /// Ticks keep coming while on screen, or finishing so it can be taken down.
    fn wants_frames(&self) -> bool {
        !self.parked || self.stopped.is_some()
    }

    /// Marks it hidden; the op hides the window and frees its surface.
    fn hide(&mut self) -> Option<Op> {
        let surface = self.surface.take();
        std::mem::take(&mut self.shown).then_some(Op::Hide(self.window, surface))
    }
}

/// Win32 work on a bar window, decided under the registry lock and run once
/// the guard is dropped.
enum Op {
    /// Hide the window; the surface it no longer needs goes with the op.
    Hide(isize, Option<Surface>),
    /// Bring the window to the top of its siblings, moved to the rect if any.
    Raise(isize, Option<RECT>),
}

impl Op {
    fn run(self) {
        match self {
            Op::Hide(window, _surface) => unsafe {
                let _ = WindowsAndMessaging::ShowWindow(
                    from_handle(window),
                    WindowsAndMessaging::SW_HIDE,
                );
            },
            Op::Raise(window, rect) => set_pos(window, rect, SWP_NOACTIVATE),
        }
    }
}

static BARS: OnceLock<Mutex<HashMap<String, Bar>>> = OnceLock::new();
/// The tick interval armed on each host window.
static TIMERS: OnceLock<Mutex<HashMap<isize, u32>>> = OnceLock::new();

fn lock<T>(slot: &'static OnceLock<Mutex<T>>, init: fn() -> T) -> MutexGuard<'static, T> {
    let mutex = slot.get_or_init(|| Mutex::new(init()));
    mutex.lock().unwrap_or_else(|poisoned| {
        mutex.clear_poison();
        poisoned.into_inner()
    })
}

/// The registry. Never hold the guard across a Win32 call.
fn bars() -> MutexGuard<'static, HashMap<String, Bar>> {
    lock(&BARS, HashMap::new)
}

/// Runs `f` on the bar for `key` under the lock; the guard is gone when this
/// returns, so the Win32 work `f` decides on goes after it.
fn with_bar<R>(key: &str, f: impl FnOnce(&mut Bar) -> R) -> Option<R> {
    bars().get_mut(key).map(f)
}

fn handle(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

fn from_handle(value: isize) -> HWND {
    HWND(value as *mut c_void)
}

/// A refresh is running on the page: started and not yet stopped.
#[cfg(feature = "components")]
pub(crate) fn is_refreshing(webtag_key: &str) -> bool {
    with_bar(webtag_key, |bar| bar.stopped.is_none()).unwrap_or(false)
}

/// The page has a bar, running or completing.
pub(crate) fn has_bar(webtag_key: &str) -> bool {
    bars().contains_key(webtag_key)
}

/// Shows the bar for `webtag_key` on `host` in `rgb` (default grey when
/// `None`). Returns whether a new refresh began; a start while running is a no-op.
pub(crate) fn start(host: HWND, webtag_key: &str, page_visible: bool, rgb: Option<u32>) -> bool {
    let rgb = rgb.unwrap_or(DEFAULT_RGB);
    let animate = animations_enabled();
    let now = Instant::now();
    let restarted = with_bar(webtag_key, |bar| {
        bar.restart(now, animate, rgb, page_visible)
    });
    let announce_start = match restarted {
        Some(None) => return false,
        Some(Some(announce)) => announce,
        None => {
            let Some(window) = create_window(host) else {
                return false;
            };
            let inserted = match bars().entry(webtag_key.to_string()) {
                Entry::Occupied(_) => false,
                Entry::Vacant(slot) => {
                    slot.insert(Bar::new(host, window, now, animate, rgb, page_visible));
                    true
                }
            };
            if !inserted {
                destroy_window(window);
                return false;
            }
            page_visible
        }
    };
    schedule(host);
    if announce_start {
        announce(host, false);
    }
    true
}

/// The refresh finished; the bar completes once it has been up long enough.
pub(crate) fn stop(webtag_key: &str) {
    let now = Instant::now();
    let host = with_bar(webtag_key, |bar| {
        bar.stopped.is_none().then(|| {
            bar.stopped = Some(now);
            bar.host
        })
    });
    // A parked bar needs ticks again to be taken down.
    if let Some(Some(host)) = host {
        schedule(from_handle(host));
    }
}

/// The page went away: take the bar down at once and say nothing.
pub(crate) fn abort(webtag_key: &str) {
    let Some(bar) = bars().remove(webtag_key) else {
        return;
    };
    destroy_window(bar.window);
    schedule(from_handle(bar.host));
}

/// Places the bar along the top of the page rect `rect` (host client
/// coordinates, `scale` device px per CSS px); an empty rect hides it.
pub(crate) fn place(host: HWND, webtag_key: &str, rect: RECT, scale: f64, radii: [i32; 2]) {
    let Some(moved) = with_bar(webtag_key, |bar| bar.host != handle(host)) else {
        return;
    };
    if moved && !rehost(host, webtag_key) {
        return;
    }
    let height = ((BAR_CSS_HEIGHT * scale).round() as i32).max(1);
    let placed = if rect.right > rect.left && rect.bottom - rect.top > height {
        RECT {
            bottom: rect.top + height,
            ..rect
        }
    } else {
        RECT::default()
    };
    let Some(op) = with_bar(webtag_key, |bar| {
        bar.rect = placed;
        bar.radii = radii;
        bar.parked &= is_empty(placed);
        if is_empty(placed) {
            bar.hide()
        } else {
            bar.shown.then_some(Op::Raise(bar.window, Some(placed)))
        }
    }) else {
        return;
    };
    if let Some(op) = op {
        op.run();
    }
    schedule(host);
}

/// The page moved to another host window: the bar follows it there.
fn rehost(host: HWND, webtag_key: &str) -> bool {
    let Some(window) = create_window(host) else {
        return false;
    };
    let previous = with_bar(webtag_key, |bar| {
        bar.shown = false;
        let surface = bar.surface.take();
        let host = std::mem::replace(&mut bar.host, handle(host));
        (host, std::mem::replace(&mut bar.window, window), surface)
    });
    let Some((previous_host, previous_window, _surface)) = previous else {
        destroy_window(window);
        return false;
    };
    destroy_window(previous_window);
    schedule(from_handle(previous_host));
    true
}

/// After a layout pass on `host`: keeps its bars above the page surfaces it
/// restacked, and unparks them to see whether their page is back on screen.
pub(crate) fn raise(host: HWND) {
    let ops: Vec<Op> = bars()
        .values_mut()
        .filter(|bar| bar.host == handle(host))
        .filter_map(|bar| {
            bar.parked = false;
            bar.shown.then_some(Op::Raise(bar.window, None))
        })
        .collect();
    ops.into_iter().for_each(Op::run);
    schedule(host);
}

/// A shown bar's next picture, drawn and presented outside the lock.
struct Paint {
    key: String,
    window: isize,
    rect: RECT,
    /// First frame: show the window too.
    show: bool,
    surface: Option<Surface>,
    rgb: u32,
    frame: Frame,
    radii: [i32; 2],
}

impl Paint {
    /// Draws into the surface, made or remade to size, and presents it.
    fn present(&mut self) {
        let (width, height) = (
            self.rect.right - self.rect.left,
            self.rect.bottom - self.rect.top,
        );
        if self
            .surface
            .as_ref()
            .is_none_or(|surface| surface.size != (width, height))
        {
            self.surface = Surface::new(width, height);
        }
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        paint_bar(
            surface.pixels(),
            width as usize,
            self.rgb,
            self.frame,
            self.radii,
        );
        surface.present(from_handle(self.window));
        if self.show {
            set_pos(
                self.window,
                Some(self.rect),
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }
}

/// One frame for every bar on `host`; a page `page_visible` rejects keeps its
/// refresh but not its bar.
pub(crate) fn tick(host: HWND, page_visible: &dyn Fn(&str) -> bool) {
    let keys: Vec<String> = bars()
        .iter()
        .filter(|(_, bar)| bar.host == handle(host))
        .map(|(key, _)| key.clone())
        .collect();
    // Asked before locking: the callback is the host's.
    let visible: Vec<(bool, String)> = keys
        .into_iter()
        .map(|key| (page_visible(&key), key))
        .collect();
    let now = Instant::now();
    let (mut ops, mut paints, mut finished) = (Vec::new(), Vec::new(), Vec::new());
    {
        let mut bars = bars();
        for (on_screen, key) in visible {
            let Some(bar) = bars.get_mut(&key).filter(|bar| bar.host == handle(host)) else {
                continue;
            };
            let Some(frame) = bar.frame(now) else {
                finished.extend(bars.remove(&key).map(|bar| (bar, on_screen)));
                continue;
            };
            if is_empty(bar.rect) || !on_screen {
                bar.parked = true;
                ops.extend(bar.hide());
                continue;
            }
            paints.push(Paint {
                window: bar.window,
                rect: bar.rect,
                show: !std::mem::replace(&mut bar.shown, true),
                surface: bar.surface.take(),
                rgb: bar.rgb,
                frame,
                radii: bar.radii,
                key,
            });
        }
    }
    ops.into_iter().for_each(Op::run);
    paints.iter_mut().for_each(Paint::present);
    // Hand the surfaces back; one whose bar went meanwhile is freed after.
    let unclaimed: Vec<Option<Surface>> = {
        let mut bars = bars();
        paints
            .into_iter()
            .filter_map(|paint| match bars.get_mut(&paint.key) {
                Some(bar) if bar.window == paint.window && bar.shown => {
                    bar.surface = paint.surface;
                    None
                }
                _ => Some(paint.surface),
            })
            .collect()
    };
    drop(unclaimed);
    for (bar, on_screen) in finished {
        destroy_window(bar.window);
        if bar.announced && on_screen {
            announce(host, true);
        }
    }
    schedule(host);
}

fn is_empty(rect: RECT) -> bool {
    rect.right <= rect.left || rect.bottom <= rect.top
}

/// Brings `window` to the top of its siblings, moved to `rect` if given.
fn set_pos(window: isize, rect: Option<RECT>, flags: SET_WINDOW_POS_FLAGS) {
    let flags = if rect.is_some() {
        flags
    } else {
        flags | SWP_NOMOVE | SWP_NOSIZE
    };
    let r = rect.unwrap_or_default();
    unsafe {
        let _ = SetWindowPos(
            from_handle(window),
            Some(WindowsAndMessaging::HWND_TOP),
            r.left,
            r.top,
            r.right - r.left,
            r.bottom - r.top,
            flags,
        );
    }
}

/// Arms, re-paces or kills `host`'s timer: fast while a band sweeps, calm for
/// fades and breathing, none while every bar is parked.
fn schedule(host: HWND) {
    let wanted = bars()
        .values()
        .filter(|bar| bar.host == handle(host) && bar.wants_frames())
        .map(|bar| {
            if bar.animate && !bar.parked {
                SWEEP_TICK_MS
            } else {
                CALM_TICK_MS
            }
        })
        .min();
    {
        let mut timers = lock(&TIMERS, HashMap::new);
        if timers.get(&handle(host)).copied() == wanted {
            return;
        }
        match wanted {
            Some(interval) => timers.insert(handle(host), interval),
            None => timers.remove(&handle(host)),
        };
    }
    unsafe {
        match wanted {
            Some(interval) => {
                let _ = WindowsAndMessaging::SetTimer(Some(host), TIMER_ID, interval, None);
            }
            None => {
                let _ = WindowsAndMessaging::KillTimer(Some(host), TIMER_ID);
            }
        }
    }
}

fn millis(duration: Duration) -> f32 {
    duration.as_secs_f32() * 1000.0
}

/// When the bar starts finishing, in ms from the start.
fn completion_at(stopped_after: Duration) -> f32 {
    millis(stopped_after).max(MIN_VISIBLE_MS)
}

/// The bar has begun to finish (turning solid or fading).
fn is_completing(elapsed: Duration, stopped_after: Option<Duration>) -> bool {
    stopped_after.is_some_and(|stopped| millis(elapsed) >= completion_at(stopped))
}

fn lerp(a: f32, b: f32, x: f32) -> f32 {
    a + (b - a) * x
}

/// Left edge of the band `clock_ms` into the sweep; it wraps off screen.
fn band_left(clock_ms: f32) -> f32 {
    -BAND_SHARE + (1.0 + BAND_SHARE) * (clock_ms.max(0.0) / SWEEP_MS).rem_euclid(1.0)
}

/// Opacity of the breathing bar `t_ms` after the start.
fn breath(t_ms: f32) -> f32 {
    let cycle = (t_ms.max(0.0) / BREATH_MS).rem_euclid(1.0);
    1.0 - (1.0 - BREATH_LOW) * (1.0 - (std::f32::consts::TAU * cycle).cos()) / 2.0
}

/// The bar `elapsed` after a start from `origin`, or `None` once faded out.
/// `stopped_after` and `sweep_offset` (the band's clock) are from the start.
fn frame(
    elapsed: Duration,
    stopped_after: Option<Duration>,
    origin: Origin,
    sweep_offset: Duration,
    animate: bool,
) -> Option<Frame> {
    let t = millis(elapsed);
    let band = animate.then(|| band_left(t + millis(sweep_offset)));
    let running = |t: f32| {
        let x = (t / FADE_IN_MS).clamp(0.0, 1.0);
        Frame {
            envelope: lerp(origin.opacity, 1.0, x),
            breath: if animate { 1.0 } else { breath(t) },
            solid: lerp(origin.solid, if animate { 0.0 } else { 1.0 }, x),
            band,
        }
    };
    let Some(complete_at) = stopped_after.map(completion_at).filter(|at| t >= *at) else {
        return Some(running(t));
    };
    let from = running(complete_at);
    let since = t - complete_at;
    if since < SOLID_MS {
        let x = since / SOLID_MS;
        return Some(Frame {
            envelope: lerp(from.envelope, 1.0, x),
            breath: lerp(from.breath, 1.0, x),
            solid: lerp(from.solid, 1.0, x),
            band,
        });
    }
    let fade = 1.0 - (since - SOLID_MS) / FADE_OUT_MS;
    (fade > 0.0).then_some(Frame {
        envelope: fade,
        breath: 1.0,
        solid: 1.0,
        band,
    })
}

/// Band coverage of the column centred at `share` of the width: full in the
/// middle, ramping linearly over the soft edges.
fn band_coverage(share: f32, left: f32) -> f32 {
    let u = (share - left) / BAND_SHARE;
    if !(0.0..=1.0).contains(&u) {
        return 0.0;
    }
    (u.min(1.0 - u) / BAND_EDGE).min(1.0)
}

/// Paints premultiplied ARGB `pixels` (top-down rows of `width`): base, band
/// and solid fill at the frame's opacity, top corners clipped to `radii`.
fn paint_bar(pixels: &mut [u32], width: usize, rgb: u32, frame: Frame, radii: [i32; 2]) {
    if width == 0 {
        return;
    }
    let height = pixels.len() / width;
    let opacity = frame.opacity();
    let solid = frame.solid.clamp(0.0, 1.0);
    let alpha = |x: usize| {
        let band = frame.band.map_or(0.0, |left| {
            band_coverage((x as f32 + 0.5) / width as f32, left)
        });
        (1.0 - (1.0 - BASE_ALPHA) * (1.0 - band) * (1.0 - solid)) * opacity
    };
    let corner_rows = (radii[0].max(radii[1]).max(0) as usize).min(height);
    let (corners, plain) = pixels[..width * height].split_at_mut(corner_rows * width);
    for (y, row) in corners.chunks_exact_mut(width).enumerate() {
        for (x, pixel) in row.iter_mut().enumerate() {
            *pixel = premultiply(rgb, alpha(x) * corner_coverage(x, y, width, radii));
        }
    }
    // Rows below the corners are copies of one row.
    if plain.is_empty() {
        return;
    }
    let (first, rest) = plain.split_at_mut(width);
    for (x, pixel) in first.iter_mut().enumerate() {
        *pixel = premultiply(rgb, alpha(x));
    }
    for row in rest.chunks_exact_mut(width) {
        row.copy_from_slice(first);
    }
}

/// How much of pixel (`x`, `y`) lies inside the rounded top corners.
fn corner_coverage(x: usize, y: usize, width: usize, radii: [i32; 2]) -> f32 {
    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
    let inside = |cx: f32, radius: f32| {
        let distance = ((px - cx).powi(2) + (py - radius).powi(2)).sqrt();
        (radius - distance + 0.5).clamp(0.0, 1.0)
    };
    let left = radii[0].max(0) as f32;
    let right = radii[1].max(0) as f32;
    if left > 0.0 && px < left && py < left {
        return inside(left, left);
    }
    if right > 0.0 && px > width as f32 - right && py < right {
        return inside(width as f32 - right, right);
    }
    1.0
}

fn premultiply(rgb: u32, alpha: f32) -> u32 {
    let alpha = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    let channel = |shift: u32| (((rgb >> shift) & 0xFF) * alpha + 127) / 255;
    (alpha << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

/// Windows' "Show animations in Windows" setting; off means no motion.
fn animations_enabled() -> bool {
    let mut enabled = windows::core::BOOL(1);
    unsafe {
        let _ = WindowsAndMessaging::SystemParametersInfoW(
            WindowsAndMessaging::SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut enabled as *mut _ as *mut c_void),
            WindowsAndMessaging::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
    enabled.as_bool()
}

/// Announces the start, or the end when `finished`, to listening UIA clients.
fn announce(host: HWND, finished: bool) {
    if !unsafe { UiaClientsAreListening() }.as_bool() {
        return;
    }
    let (text, kind) = if finished {
        (
            lingxia_platform::i18n::text("pull_refresh.refreshed", "Refreshed"),
            NotificationKind_ActionCompleted,
        )
    } else {
        (
            lingxia_platform::i18n::text("pull_refresh.refreshing", "Refreshing"),
            NotificationKind_Other,
        )
    };
    let root = unsafe { WindowsAndMessaging::GetAncestor(host, WindowsAndMessaging::GA_ROOT) };
    let target = if root.0.is_null() { host } else { root };
    let Ok(provider) = (unsafe { UiaHostProviderFromHwnd(target) }) else {
        return;
    };
    let result = unsafe {
        UiaRaiseNotificationEvent(
            &provider,
            kind,
            NotificationProcessing_MostRecent,
            &BSTR::from(text),
            &BSTR::from("lingxia.pull-refresh"),
        )
    };
    if let Err(error) = result {
        log::debug!("refresh announcement failed: {error}");
    }
}

fn create_window(host: HWND) -> Option<isize> {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    let instance = unsafe { LibraryLoader::GetModuleHandleW(None) }
        .ok()
        .map(|module| HINSTANCE(module.0));
    REGISTERED.get_or_init(|| {
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.unwrap_or_default(),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        if unsafe { RegisterClassW(&class) } == 0 {
            log::error!(
                "refresh bar class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    // Hidden until the first frame; no WM_PARENTNOTIFY to the host.
    let window = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_NOPARENTNOTIFY,
            CLASS_NAME,
            PCWSTR::null(),
            WS_CHILD | WS_CLIPSIBLINGS,
            0,
            0,
            1,
            1,
            Some(host),
            None,
            instance,
            None,
        )
    };
    match window {
        Ok(window) => Some(handle(window)),
        Err(error) => {
            log::warn!("refresh bar window creation failed: {error}");
            None
        }
    }
}

fn destroy_window(window: isize) {
    unsafe {
        if WindowsAndMessaging::IsWindow(Some(from_handle(window))).as_bool() {
            let _ = DestroyWindow(from_handle(window));
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_NCHITTEST => LRESULT(WindowsAndMessaging::HTTRANSPARENT as isize),
        WindowsAndMessaging::WM_MOUSEACTIVATE => {
            LRESULT(WindowsAndMessaging::MA_NOACTIVATE as isize)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// A memory DC holding a top-down 32-bit DIB: the pixels
/// `UpdateLayeredWindow` presents, remade only when the size changes.
struct Surface {
    size: (i32, i32),
    dc: isize,
    bitmap: isize,
    previous: isize,
    bits: usize,
}

impl Surface {
    fn new(width: i32, height: i32) -> Option<Self> {
        if width <= 0 || height <= 0 {
            return None;
        }
        unsafe {
            let dc = CreateCompatibleDC(None);
            if dc.is_invalid() {
                return None;
            }
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut c_void = std::ptr::null_mut();
            let bitmap = match CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
            {
                Ok(bitmap) if !bits.is_null() => bitmap,
                result => {
                    if let Ok(bitmap) = result {
                        let _ = DeleteObject(HGDIOBJ(bitmap.0));
                    }
                    let _ = DeleteDC(dc);
                    return None;
                }
            };
            let previous = SelectObject(dc, HGDIOBJ(bitmap.0));
            Some(Self {
                size: (width, height),
                dc: dc.0 as isize,
                bitmap: bitmap.0 as isize,
                previous: previous.0 as isize,
                bits: bits as usize,
            })
        }
    }

    fn pixels(&mut self) -> &mut [u32] {
        let (width, height) = self.size;
        // SAFETY: `bits` is the DIB's `width * height` 32-bit pixels, alive
        // until `drop`, and only reachable through `&mut self`.
        unsafe { std::slice::from_raw_parts_mut(self.bits as *mut u32, (width * height) as usize) }
    }

    fn present(&self, window: HWND) {
        let size = SIZE {
            cx: self.size.0,
            cy: self.size.1,
        };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        unsafe {
            let _ = UpdateLayeredWindow(
                window,
                None,
                None,
                Some(&size),
                Some(HDC(self.dc as *mut c_void)),
                Some(&POINT::default()),
                COLORREF(0),
                Some(&blend),
                WindowsAndMessaging::ULW_ALPHA,
            );
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            let dc = HDC(self.dc as *mut c_void);
            let _ = SelectObject(dc, HGDIOBJ(self.previous as *mut c_void));
            let _ = DeleteObject(HGDIOBJ(self.bitmap as *mut c_void));
            let _ = DeleteDC(dc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BAND_SHARE, Frame, Origin, band_left, breath, frame, is_completing, paint_bar};
    use std::time::Duration;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[track_caller]
    fn assert_near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 1e-4,
            "{actual}, expected {expected}"
        );
    }

    /// A refresh started from nothing, its band clock at zero.
    fn fresh(t: u64, stopped: Option<u64>, animate: bool) -> Option<Frame> {
        let origin = Origin::fresh(animate);
        frame(ms(t), stopped.map(ms), origin, Duration::ZERO, animate)
    }

    #[test]
    fn flowing_bar_fades_in_then_sweeps_across_the_translucent_base() {
        let first = fresh(0, None, true).unwrap();
        assert_eq!((first.opacity(), first.solid), (0.0, 0.0));
        assert_near(first.band.unwrap(), -BAND_SHARE);
        assert_near(fresh(60, None, true).unwrap().opacity(), 0.5);
        assert_near(
            fresh(600, None, true).unwrap().band.unwrap(),
            (1.0 - BAND_SHARE) / 2.0,
        );
        assert!(fresh(1_190, None, true).unwrap().band.unwrap() > 0.97);
        assert!(
            fresh(1_210, None, true).unwrap().band.unwrap() < -0.3,
            "wrapped"
        );
        let late = fresh(60_300, None, true).unwrap();
        assert_eq!((late.opacity(), late.solid), (1.0, 0.0));
        assert!((late.band.unwrap() - band_left(300.0)).abs() < 1e-3);
    }

    #[test]
    fn done_waits_for_the_minimum_then_turns_solid_and_fades() {
        let held = fresh(599, Some(10), true).unwrap();
        assert_eq!((held.opacity(), held.solid), (1.0, 0.0));
        assert!(!is_completing(ms(599), Some(ms(10))));
        assert!(is_completing(ms(600), Some(ms(10))));
        assert_near(fresh(675, Some(10), true).unwrap().solid, 0.5);
        let solid = fresh(750, Some(10), true).unwrap();
        assert_eq!((solid.opacity(), solid.solid), (1.0, 1.0));
        assert_near(fresh(875, Some(10), true).unwrap().opacity(), 0.5);
        assert!(fresh(1_000, Some(10), true).is_none());
        // Stopped late: finishes from then.
        assert_eq!(fresh(3_000, Some(3_000), true).unwrap().solid, 0.0);
        assert_eq!(fresh(3_150, Some(3_000), true).unwrap().solid, 1.0);
        assert!(fresh(3_400, Some(3_000), true).is_none());
    }

    #[test]
    fn restart_during_the_finish_returns_to_flowing_without_a_flash() {
        let before = fresh(875, Some(10), true).unwrap();
        // What `start` keeps: what is on screen, and the band's clock.
        let origin = Origin {
            opacity: before.opacity(),
            solid: before.solid,
        };
        let restarted =
            |t, stopped: Option<u64>| frame(ms(t), stopped.map(ms), origin, ms(875), true);
        let after = restarted(0, None).unwrap();
        assert_near(after.opacity(), before.opacity());
        assert_eq!(after.solid, 1.0, "no jump in colour");
        assert_near(after.band.unwrap(), before.band.unwrap());
        let flowing = restarted(125, None).unwrap();
        assert_eq!((flowing.opacity(), flowing.solid), (1.0, 0.0));
        assert!(restarted(30_000, None).is_some(), "running again");
        assert_eq!(restarted(599, Some(10)).unwrap().solid, 0.0);
        assert!(restarted(1_000, Some(10)).is_none());
    }

    #[test]
    fn without_animations_the_solid_bar_breathes_and_never_sweeps() {
        for (t, opacity) in [(400, 0.75), (800, 0.5), (1_600, 1.0)] {
            assert_near(fresh(t, None, false).unwrap().opacity(), opacity);
        }
        for t in (120..=4_000).step_by(10) {
            let frame = fresh(t, None, false).unwrap();
            let in_range = (0.5 - 1e-5..=1.0 + 1e-5).contains(&frame.opacity());
            assert!(in_range, "{t} ms");
            assert_eq!((frame.solid, frame.band), (1.0, None));
        }
        assert_eq!(fresh(0, None, false).unwrap().band, None);
        assert_near(fresh(120, None, false).unwrap().opacity(), breath(120.0));
        // Done at the bottom of a breath: back to full, then the same fade.
        assert_near(fresh(950, Some(800), false).unwrap().opacity(), 1.0);
        assert_near(fresh(1_075, Some(800), false).unwrap().opacity(), 0.5);
        assert!(fresh(1_200, Some(800), false).is_none());
    }

    #[test]
    fn paints_premultiplied_base_band_and_rounded_corners() {
        const RGB: u32 = 0x2865FF;
        const FULL: u32 = 0xFF00_0000 | RGB;
        let flowing = Frame {
            envelope: 1.0,
            breath: 1.0,
            solid: 0.0,
            band: Some(0.3),
        };
        let still = Frame {
            band: None,
            ..flowing
        };
        let solid = Frame {
            solid: 1.0,
            ..still
        };
        let faded = Frame {
            envelope: 0.5,
            ..solid
        };
        // (frame, width, height, radii, pixel, expected). At 200 px the band
        // covers 60..130; rows below the corner radius copy the plain row.
        let cases: [(Frame, usize, usize, [i32; 2], usize, u32); 12] = [
            (flowing, 200, 3, [0, 0], 10, 0x6610_2866),
            (flowing, 200, 3, [0, 0], 199, 0x6610_2866),
            (flowing, 200, 3, [0, 0], 95, FULL),
            (flowing, 200, 3, [0, 0], 2 * 200 + 95, FULL),
            (still, 200, 3, [0, 0], 95, 0x6610_2866),
            (faded, 200, 3, [0, 0], 0, 0x8014_3380),
            (solid, 100, 3, [8, 8], 0, 0),
            (solid, 100, 3, [8, 8], 99, 0),
            (solid, 100, 3, [8, 8], 2 * 100 + 8, FULL),
            (solid, 100, 6, [4, 0], 0, 0),
            (solid, 100, 6, [4, 0], 5 * 100, FULL),
            (solid, 100, 6, [4, 0], 99, FULL),
        ];
        for (index, (frame, width, height, radii, at, expected)) in cases.into_iter().enumerate() {
            let mut pixels = vec![0; width * height];
            paint_bar(&mut pixels, width, RGB, frame, radii);
            assert_eq!(pixels[at], expected, "case {index}: {:08X}", pixels[at]);
        }
        let mut pixels = vec![0; 200];
        paint_bar(&mut pixels, 200, RGB, flowing, [0, 0]);
        let alpha = |x: usize| pixels[x] >> 24;
        assert!(
            0x66 < alpha(63) && alpha(63) < alpha(66) && alpha(66) < 0xFF,
            "soft edge"
        );
    }
}
