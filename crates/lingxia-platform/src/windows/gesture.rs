//! Gestures on GPU pages: touch goes to the page's window through the message its touch
//! path reads, a trackpad through the precise scroll message it reads, mouse through posted
//! button messages. A WebView page takes no touch here; a trackpad reaches it as the wheel.
use super::Platform;
use super::mouse::deepest_child_at;
use super::screenshot::resolve_screenshot_window;
use crate::error::PlatformError;
use crate::traits::gesture::{
    AppGesture, AppGestureRequest, AppGestureResult, ChromeElement, GestureDelivery, GestureEnd,
    GestureInput, GesturePoint, GestureTarget, HostCapability, PullRefreshIndicator,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    ScreenToClient,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, GetClassNameW, GetClientRect, GetWindowRect, GetWindowTextW, IsWindowVisible,
    PostMessageW, RegisterWindowMessageW, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
};
use windows::core::BOOL;
use windows::core::w;

const MK_LBUTTON: usize = 0x0001;

#[derive(Clone, Copy)]
struct Held {
    input: GestureInput,
    /// The window the gesture went down in, and how its client maps from target points.
    window: isize,
    origin: (f64, f64),
    scale: f64,
    touch: bool,
    last: (f64, f64),
    /// A trackpad's first point, which the whole scroll reaches, and the window pixels of
    /// travel already posted: scroll messages carry whole pixels.
    anchor: (f64, f64),
    posted: (f64, f64),
    scrolled: bool,
    began: Instant,
}

static NEXT: AtomicU64 = AtomicU64::new(1);

fn held() -> std::sync::MutexGuard<'static, HashMap<u64, Held>> {
    static HELD: OnceLock<Mutex<HashMap<u64, Held>>> = OnceLock::new();
    HELD.get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

/// The message the GPU page window's touch path reads (registered there under this name).
fn touch_message() -> u32 {
    // SAFETY: a constant name.
    unsafe { RegisterWindowMessageW(w!("LingXiaViewTouch")) }
}
/// The GPU page window's precise scroll: `wparam` packs the phase over the fingers' travel as
/// two `i16` window pixels (registered there under this name).
fn scroll_message() -> u32 {
    // SAFETY: a constant name.
    unsafe { RegisterWindowMessageW(w!("LingXiaViewScroll")) }
}

/// The shell's own chrome of a host window, in its client pixels.
///
/// With `reveal_tabs` a collapsed sidebar is asked to show its tabs; the flag returned is
/// true while that reveal is under way, and the caller asks again once it has drawn.
pub type WindowsChromeProvider =
    Arc<dyn Fn(isize, bool) -> (Vec<ChromeElement>, bool) + Send + Sync>;
static CHROME_PROVIDER: Mutex<Option<WindowsChromeProvider>> = Mutex::new(None);

pub fn set_windows_chrome_provider(provider: WindowsChromeProvider) {
    *CHROME_PROVIDER.lock().unwrap_or_else(|p| p.into_inner()) = Some(provider);
}

/// Visible push buttons under `window`, such as a failed page's recovery button.
fn child_buttons(window: isize) -> Vec<ChromeElement> {
    unsafe extern "system" fn visit(child: HWND, found: LPARAM) -> BOOL {
        // SAFETY: `found` is the pair below, alive for the enumeration.
        let (window, found) = unsafe { &mut *(found.0 as *mut (HWND, Vec<ChromeElement>)) };
        let mut class = [0u16; 32];
        let mut text = [0u16; 128];
        // SAFETY: buffers of their stated lengths; reads of a live child.
        unsafe {
            let len = GetClassNameW(child, &mut class) as usize;
            if !IsWindowVisible(child).as_bool()
                || !String::from_utf16_lossy(&class[..len]).eq_ignore_ascii_case("button")
            {
                return BOOL(1);
            }
            let len = GetWindowTextW(child, &mut text) as usize;
            let mut rect = RECT::default();
            let _ = GetWindowRect(child, &mut rect);
            let mut origin = POINT {
                x: rect.left,
                y: rect.top,
            };
            let _ = ScreenToClient(*window, &mut origin);
            found.push(ChromeElement {
                label: String::from_utf16_lossy(&text[..len]),
                role: "button".into(),
                scope: "window".into(),
                selected: false,
                bounds: [
                    f64::from(origin.x),
                    f64::from(origin.y),
                    f64::from(rect.right - rect.left),
                    f64::from(rect.bottom - rect.top),
                ],
            });
        }
        BOOL(1)
    }
    let mut found = (HWND(window as *mut _), Vec::new());
    // SAFETY: the callback only touches `found`, which outlives the call.
    unsafe {
        let _ = EnumChildWindows(
            Some(found.0),
            Some(visit),
            LPARAM(&mut found as *mut _ as isize),
        );
    }
    found.1
}

#[async_trait]
impl AppGesture for Platform {
    // A desktop, as on macOS: a finger is there for a spec that asks for one.
    fn gesture_inputs(&self) -> Vec<GestureInput> {
        vec![GestureInput::Mouse, GestureInput::Trackpad]
    }

    fn host_capabilities(&self) -> Vec<HostCapability> {
        let mut capabilities = if CHROME_PROVIDER
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_some()
        {
            vec![HostCapability::Chrome, HostCapability::HostInput]
        } else {
            Vec::new()
        };
        if super::pull_to_refresh::has_indicator_reader() {
            capabilities.push(HostCapability::RefreshIndicator);
        }
        capabilities
    }

    async fn pull_refresh_indicator(
        &self,
        _app_id: &str,
        webtag: &str,
    ) -> Result<Option<PullRefreshIndicator>, PlatformError> {
        super::pull_to_refresh::windows_pull_refresh_indicator(webtag)
    }

    // Bounds are host client pixels: the unit a window gesture takes here.
    async fn chrome_elements(
        &self,
        scope: Option<String>,
    ) -> Result<Vec<ChromeElement>, PlatformError> {
        let window = resolve_screenshot_window(None)?.0 as isize;
        let provider = CHROME_PROVIDER
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let reveal = matches!(scope.as_deref(), Some("sidebar" | "tabBar"));
        let mut elements = match provider {
            Some(provider) => {
                let (elements, opening) = provider(window, reveal);
                if opening {
                    // The peek slides open before its rows can be pressed.
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    provider(window, reveal).0
                } else {
                    elements
                }
            }
            None => Vec::new(),
        };
        elements.extend(child_buttons(window));
        if let Some(scope) = scope {
            elements.retain(|element| element.scope == scope);
        }
        Ok(elements)
    }

    fn webview_page_gestures(&self) -> bool {
        false
    }

    async fn perform_app_gesture(
        &self,
        request: AppGestureRequest,
    ) -> Result<AppGestureResult, PlatformError> {
        perform(request).await
    }

    async fn orient_window(&self, landscape: bool) -> Result<[f64; 2], PlatformError> {
        let window = resolve_screenshot_window(None)?.0 as isize;
        // The page decides the orientation, not the window: the sidebar and title bar make a
        // squarish window's page portrait. Resizing can also change that chrome (a narrow
        // window folds its sidebar), so the page is measured again after each turn.
        for _ in 0..3 {
            let (width, height) = client_size(window);
            let (pw, ph) = largest_page(window).unwrap_or((width, height));
            if (pw > ph) == landscape {
                break;
            }
            {
                let mut frame = RECT::default();
                let mut monitor = MONITORINFO {
                    cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                // SAFETY: reads and resizes a live top-level window; the resize is sent to its thread.
                unsafe {
                    let hwnd = HWND(window as *mut _);
                    GetWindowRect(hwnd, &mut frame)
                        .map_err(|e| PlatformError::Platform(format!("window rect: {e}")))?;
                    let _ = GetMonitorInfoW(
                        MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
                        &mut monitor,
                    );
                    let work = monitor.rcWork;
                    let chrome = (
                        f64::from(frame.right - frame.left) - pw,
                        f64::from(frame.bottom - frame.top) - ph,
                    );
                    // Windows caps a window at its work area: a page too tall to turn there
                    // shrinks until it fits.
                    let fit = ((f64::from(work.right - work.left) - chrome.0) / ph)
                        .min((f64::from(work.bottom - work.top) - chrome.1) / pw)
                        .min(1.0);
                    let width = (chrome.0 + ph * fit).round() as i32;
                    let height = (chrome.1 + pw * fit).round() as i32;
                    let left = frame.left.min(work.right - width).max(work.left);
                    let top = frame.top.min(work.bottom - height).max(work.top);
                    SetWindowPos(
                        hwnd,
                        None,
                        left,
                        top,
                        width,
                        height,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                    .map_err(|e| PlatformError::Platform(format!("resize window: {e}")))?;
                }
            }
            // The shell lays out the resized window on its own thread.
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        let (width, height) = client_size(window);
        // SAFETY: reads a window's DPI.
        let dpi = unsafe { GetDpiForWindow(HWND(window as *mut _)) };
        let scale = if dpi == 0 { 1.0 } else { f64::from(dpi) / 96.0 };
        Ok([width / scale, height / scale])
    }

    async fn gesture_viewport(&self, target: GestureTarget) -> Result<[f64; 4], PlatformError> {
        if let GestureTarget::Page { webtag, .. } = &target
            && let Some([width, height]) = lingxia_webview::platform::windows::find_webview_handler(
                &lingxia_webview::WebTag::from(webtag.as_str()),
            )
            .and_then(|handler| handler.content_viewport())
        {
            return Ok([0.0, 0.0, width, height]);
        }
        let (window, scale) = resolve(&target)?;
        let (width, height) = client_size(window);
        Ok([0.0, 0.0, width / scale, height / scale])
    }
}

/// The window a target's points are in, and its device pixels per point.
fn resolve(target: &GestureTarget) -> Result<(isize, f64), PlatformError> {
    match target {
        GestureTarget::Window { window_id } => {
            let window = resolve_screenshot_window(window_id.as_deref())?;
            Ok((window.0 as isize, 1.0))
        }
        GestureTarget::Page { webtag, .. } => {
            let tag = lingxia_webview::WebTag::from(webtag.as_str());
            let handler = lingxia_webview::platform::windows::find_webview_handler(&tag)
                .ok_or_else(|| PlatformError::Platform(format!("no page window for {webtag}")))?;
            // A GPU page's window moves between host parents; its points are its own.
            let window = handler
                .content_window()
                .unwrap_or_else(|| handler.native_view().window);
            // A device frame shrinks the page to fit the screen, below the window's DPI.
            let scale = handler.content_scale().unwrap_or_else(|| {
                // SAFETY: reads a window's DPI.
                let dpi = unsafe { GetDpiForWindow(HWND(window as *mut _)) };
                if dpi == 0 { 1.0 } else { f64::from(dpi) / 96.0 }
            });
            Ok((window, scale))
        }
    }
}

fn client_size(window: isize) -> (f64, f64) {
    let mut rect = RECT::default();
    // SAFETY: an out-parameter on the stack.
    let _ = unsafe { GetClientRect(HWND(window as *mut _), &mut rect) };
    (
        f64::from(rect.right - rect.left),
        f64::from(rect.bottom - rect.top),
    )
}

/// The largest visible page window (GPU page or WebView) under `window`, client pixels.
fn largest_page(window: isize) -> Option<(f64, f64)> {
    unsafe extern "system" fn visit(child: HWND, found: LPARAM) -> BOOL {
        // SAFETY: `found` is the `Option` below, alive for the enumeration.
        let found = unsafe { &mut *(found.0 as *mut Option<(f64, f64)>) };
        let mut name = [0u16; 64];
        // SAFETY: a buffer of its stated length; reads of a live child.
        let (len, visible) = unsafe {
            (
                GetClassNameW(child, &mut name),
                IsWindowVisible(child).as_bool(),
            )
        };
        let class = String::from_utf16_lossy(&name[..len.max(0) as usize]);
        if visible && (class == "LingXiaGpuPage" || class.starts_with("Chrome_WidgetWin")) {
            let (w, h) = client_size(child.0 as isize);
            if found.is_none_or(|(fw, fh)| w * h > fw * fh) {
                *found = Some((w, h));
            }
        }
        true.into()
    }
    let mut found: Option<(f64, f64)> = None;
    // SAFETY: enumerates a live window's children with a callback that only reads them.
    unsafe {
        let _ = EnumChildWindows(
            Some(HWND(window as *mut _)),
            Some(visit),
            LPARAM(&mut found as *mut _ as isize),
        );
    }
    found
}

fn is_gpu_page(window: HWND) -> bool {
    let mut name = [0u16; 32];
    // SAFETY: a buffer of its stated length.
    let len = unsafe { GetClassNameW(window, &mut name) };
    String::from_utf16_lossy(&name[..len.max(0) as usize]) == "LingXiaGpuPage"
}

fn lparam(x: f64, y: f64) -> LPARAM {
    let (x, y) = (x.round() as i32, y.round() as i32);
    LPARAM((((y as u16) as isize) << 16) | ((x as u16) as isize))
}

fn post(window: isize, message: u32, wparam: usize, at: (f64, f64)) -> Result<(), PlatformError> {
    // SAFETY: posts to a window handle; a stale one fails.
    unsafe {
        PostMessageW(
            Some(HWND(window as *mut _)),
            message,
            WPARAM(wparam),
            lparam(at.0, at.1),
        )
    }
    .map_err(|err| PlatformError::Platform(format!("PostMessageW failed: {err}")))
}

/// Scroll `held`'s window by fingers travelling `delta` window pixels in `phase` (0 began,
/// 1 changed, 2 ended, 3 momentum): its content follows them. A window that is not a GPU page
/// reads it as the wheel.
fn scroll(held: &Held, phase: usize, delta: (f64, f64)) -> Result<(), PlatformError> {
    let at = client_point(held, held.anchor);
    if is_gpu_page(HWND(held.window as *mut _)) {
        let pack = |v: f64| usize::from(v.clamp(-32768.0, 32767.0) as i16 as u16);
        let wparam = pack(delta.0) | (pack(delta.1) << 16) | (phase << 32);
        return post(held.window, scroll_message(), wparam, at);
    }
    use windows::Win32::UI::WindowsAndMessaging::{WM_MOUSEHWHEEL, WM_MOUSEWHEEL};
    let mut screen = POINT {
        x: at.0.round() as i32,
        y: at.1.round() as i32,
    };
    // SAFETY: maps a point of a live window.
    let _ = unsafe { ClientToScreen(HWND(held.window as *mut _), &mut screen) };
    let screen = (f64::from(screen.x), f64::from(screen.y));
    // A wheel notch (120) scrolls about 100 pixels; a notch away from the user scrolls up,
    // as fingers moving down do.
    let units = |pixels: f64| ((pixels * 1.2).round() as i32 as i16 as u16 as usize) << 16;
    if delta.1 != 0.0 {
        post(held.window, WM_MOUSEWHEEL, units(delta.1), screen)?;
    }
    if delta.0 != 0.0 {
        post(held.window, WM_MOUSEHWHEEL, units(-delta.0), screen)?;
    }
    Ok(())
}

fn fling_velocity(shape: &crate::traits::gesture::GestureShape) -> (f64, f64) {
    let Ok(points) = shape.points() else {
        return (0.0, 0.0);
    };
    let [.., a, b] = points.as_slice() else {
        return (0.0, 0.0);
    };
    let dt = (b.at_ms - a.at_ms).max(1.0) / 1000.0;
    ((b.x - a.x) / dt, (b.y - a.y) / dt)
}

/// The momentum a precision touchpad's driver posts after a flick: per-frame deltas decaying
/// from the release velocity (target points per second), one per display refresh. A timer
/// sleep is no pacer here: Windows wakes it on a 15.6 ms tick, every other refresh.
async fn momentum(held: &Held, velocity: (f64, f64)) -> Result<u32, PlatformError> {
    let held = *held;
    tokio::task::spawn_blocking(move || {
        use windows::Win32::Graphics::Dwm::DwmFlush;
        const FRAME: f64 = 1.0 / 60.0;
        let decay = 0.998f64.powf(FRAME * 1000.0);
        let (mut vx, mut vy) = (velocity.0 * held.scale, velocity.1 * held.scale);
        let (mut rx, mut ry) = (0.0, 0.0);
        let mut events = 0;
        while (vx * FRAME).hypot(vy * FRAME) >= 0.5 && events < 240 {
            rx += vx * FRAME;
            ry += vy * FRAME;
            let (dx, dy) = (rx.trunc(), ry.trunc());
            (rx, ry) = (rx - dx, ry - dy);
            scroll(&held, 3, (dx, dy))?;
            events += 1;
            vx *= decay;
            vy *= decay;
            // SAFETY: no arguments; waits for the compositor's next frame.
            if unsafe { DwmFlush() }.is_err() {
                std::thread::sleep(Duration::from_millis(16));
            }
        }
        Ok(events)
    })
    .await
    .map_err(|err| PlatformError::Platform(format!("momentum thread failed: {err}")))?
}

/// Point `at` (target points) in `held`'s window client pixels.
fn client_point(held: &Held, at: (f64, f64)) -> (f64, f64) {
    (
        held.origin.0 + at.0 * held.scale,
        held.origin.1 + at.1 * held.scale,
    )
}

/// The window under the gesture's first point, which takes the whole gesture.
fn begin(
    target: &GestureTarget,
    input: GestureInput,
    at: (f64, f64),
) -> Result<Held, PlatformError> {
    let (window, scale) = resolve(target)?;
    let (base, at_px) = popup_at(window, target, (at.0 * scale, at.1 * scale));
    let (child, _) = deepest_child_at(base, at_px);
    // The target's origin in the child's client space.
    let mut origin = POINT { x: 0, y: 0 };
    // SAFETY: maps between two live windows.
    unsafe {
        let _ = ClientToScreen(HWND(window as *mut _), &mut origin);
        let _ = ScreenToClient(child, &mut origin);
    }
    Ok(Held {
        input,
        window: child.0 as isize,
        origin: (f64::from(origin.x), f64::from(origin.y)),
        scale,
        // Touch reaches a non-GPU window as the mouse messages Windows promotes it to.
        touch: input == GestureInput::Touch && is_gpu_page(child),
        last: at,
        anchor: at,
        posted: (0.0, 0.0),
        scrolled: false,
        began: Instant::now(),
    })
}

/// A window point lands on what a press there would reach: a visible popup the window owns
/// (a sidebar peek, say) when one covers it, else the window itself. Other apps' windows do
/// not count: posted input reaches the app whatever lies above it. The point is returned in
/// the chosen window's client pixels.
fn popup_at(window: isize, target: &GestureTarget, at: (f64, f64)) -> (isize, (f64, f64)) {
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GW_OWNER, GWL_EXSTYLE, GetWindow, GetWindowLongPtrW, WS_EX_TRANSPARENT,
    };
    unsafe extern "system" fn visit(candidate: HWND, found: LPARAM) -> BOOL {
        // SAFETY: `found` is the tuple below, alive for the enumeration.
        let (host, screen, found) = unsafe { &mut *(found.0 as *mut (HWND, POINT, Option<HWND>)) };
        let mut rect = RECT::default();
        // SAFETY: reads of a live top-level window.
        let covers = unsafe {
            IsWindowVisible(candidate).as_bool()
                && GetWindowLongPtrW(candidate, GWL_EXSTYLE) as u32 & WS_EX_TRANSPARENT.0 == 0
                && GetWindow(candidate, GW_OWNER).is_ok_and(|owner| owner == *host)
                && GetWindowRect(candidate, &mut rect).is_ok()
        } && screen.x >= rect.left
            && screen.x < rect.right
            && screen.y >= rect.top
            && screen.y < rect.bottom;
        if covers {
            *found = Some(candidate);
            return BOOL(0);
        }
        BOOL(1)
    }
    if !matches!(target, GestureTarget::Window { .. }) {
        return (window, at);
    }
    let host = HWND(window as *mut _);
    let mut screen = POINT {
        x: at.0.round() as i32,
        y: at.1.round() as i32,
    };
    // SAFETY: maps a point of a live window.
    let _ = unsafe { ClientToScreen(host, &mut screen) };
    let mut found = (host, screen, None);
    // SAFETY: the callback only touches `found`, which outlives the call; top-level windows
    // enumerate top first.
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&mut found as *mut _ as isize));
    }
    let Some(popup) = found.2 else {
        return (window, at);
    };
    let mut client = screen;
    // SAFETY: maps a point into a live window.
    let _ = unsafe { ScreenToClient(popup, &mut client) };
    (popup.0 as isize, (f64::from(client.x), f64::from(client.y)))
}

fn send(held: &Held, phase: usize, at: (f64, f64)) -> Result<(), PlatformError> {
    let client = client_point(held, at);
    if held.touch {
        return post(held.window, touch_message(), phase, client);
    }
    let marker = lingxia_webview::platform::windows::SYNTHETIC_MOUSE_WPARAM_MARKER;
    match phase {
        0 => post(held.window, WM_LBUTTONDOWN, MK_LBUTTON | marker, client),
        1 => post(held.window, WM_MOUSEMOVE, MK_LBUTTON | marker, client),
        _ => post(held.window, WM_LBUTTONUP, marker, client),
    }
}

async fn perform(request: AppGestureRequest) -> Result<AppGestureResult, PlatformError> {
    let points = request
        .shape
        .points()
        .map_err(PlatformError::InvalidParameter)?;
    let held_input =
        match request.finger {
            Some(id) => Some(held().get(&id).map(|held| held.input).ok_or_else(|| {
                PlatformError::InvalidParameter(format!("finger {id} is not held"))
            })?),
            None => None,
        };
    let input = held_input
        .or(request.input)
        .unwrap_or(GestureInput::Trackpad);
    if input == GestureInput::Mouse && request.end == GestureEnd::Cancel {
        return Err(PlatformError::InvalidParameter(
            "a mouse gesture cannot be cancelled; end it with up".into(),
        ));
    }
    // A rejected continuation keeps its finger held.
    let continued =
        match request.finger {
            Some(id) => Some(held().remove(&id).ok_or_else(|| {
                PlatformError::InvalidParameter(format!("finger {id} is not held"))
            })?),
            None => None,
        };
    let mut state = match continued {
        Some(held) => held,
        None => {
            let first = points
                .first()
                .map(|p| (p.x, p.y))
                .ok_or_else(|| PlatformError::InvalidParameter("empty gesture".into()))?;
            let state = begin(&request.target, input, first)?;
            if input != GestureInput::Trackpad {
                send(&state, 0, first)?;
            }
            state
        }
    };
    let started = Instant::now();
    let mut events = u32::from(continued.is_none());
    for GesturePoint { x, y, at_ms } in points.into_iter().skip(usize::from(continued.is_none())) {
        let due = started + Duration::from_secs_f64(at_ms / 1000.0);
        tokio::time::sleep_until(due.into()).await;
        state.last = (x, y);
        if input == GestureInput::Trackpad {
            // Fingers on a trackpad scroll without moving the cursor: the content follows them.
            let travel = (
                ((x - state.anchor.0) * state.scale).round(),
                ((y - state.anchor.1) * state.scale).round(),
            );
            let delta = (travel.0 - state.posted.0, travel.1 - state.posted.1);
            if delta != (0.0, 0.0) {
                let phase = if state.scrolled { 1 } else { 0 };
                scroll(&state, phase, delta)?;
                state.posted = travel;
                state.scrolled = true;
                events += 1;
            }
            continue;
        }
        send(&state, 1, (x, y))?;
        events += 1;
    }
    let finger = match request.end {
        GestureEnd::Hold => {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            held().insert(id, state);
            Some(id)
        }
        // Fingers that rest and lift without scrolling press the trackpad: a click, held for
        // as long as they rested.
        GestureEnd::Up if input == GestureInput::Trackpad && !state.scrolled => {
            let marker = lingxia_webview::platform::windows::SYNTHETIC_MOUSE_WPARAM_MARKER;
            let at = client_point(&state, state.last);
            post(state.window, WM_LBUTTONDOWN, MK_LBUTTON | marker, at)?;
            tokio::time::sleep(state.began.elapsed()).await;
            post(state.window, WM_LBUTTONUP, marker, at)?;
            events += 2;
            None
        }
        end if input == GestureInput::Trackpad => {
            scroll(&state, 2, (0.0, 0.0))?;
            events += 1;
            if end == GestureEnd::Up
                && matches!(
                    request.shape,
                    crate::traits::gesture::GestureShape::Fling { .. }
                )
            {
                events += momentum(&state, fling_velocity(&request.shape)).await?;
            }
            None
        }
        end => {
            send(
                &state,
                if end == GestureEnd::Cancel { 3 } else { 2 },
                state.last,
            )?;
            events += 1;
            None
        }
    };
    Ok(AppGestureResult {
        input,
        delivered: if state.touch {
            GestureDelivery::View
        } else {
            GestureDelivery::Window
        },
        events,
        duration_ms: started.elapsed().as_secs_f64() * 1000.0,
        finger,
    })
}
