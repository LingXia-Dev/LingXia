//! Native playback controls for the video component, mirroring the macOS player's
//! bottom bar (`MacLxMediaPlayer`): play/pause, a seekable progress
//! slider, elapsed/total time, mute and fullscreen toggles, auto-hiding
//! while playing.
//!
//! The bar is a per-pixel-alpha layered child window floating over the
//! video surface: an analytically rendered translucent capsule
//! (anti-aliased rounded rect + slider) with shared design icons and time
//! text, followed by an alpha fix-up because GDI can zero the alpha of the
//! pixels it touches. All calls run on the UI thread that owns the parent
//! window.

use std::sync::Arc;

use crate::{WindowsDesignIcon, draw_windows_design_icon_with_color};
use windows::Win32::Foundation::{
    COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleDC, CreateDIBSection, CreateFontW,
    DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, DeleteDC, DeleteObject, FF_DONTCARE, FW_NORMAL,
    FW_SEMIBOLD, GetDC, GetTextExtentPoint32W, HGDIOBJ, OUT_DEFAULT_PRECIS, ReleaseDC,
    SelectObject, SetBkMode, SetTextColor, TRANSPARENT, TextOutW,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    self, ULW_ALPHA, UpdateLayeredWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW,
};
use windows::core::{PCWSTR, w};

/// Bar metrics (device pixels), matching the macOS player's bottom bar:
/// a full-width 48px strip over the video with a clear→black gradient
/// scrim, controls vertically centered with 8px gaps.
const BAR_HEIGHT: i32 = 48;
const BAR_MIN_WIDTH: i32 = 220;
/// Bottom-edge opacity of the gradient scrim (black at 0.6).
const BAR_ALPHA: u32 = 153;
const BUTTON_WIDTH: i32 = 28;
const SIDE_PADDING: i32 = 8;
const GAP: i32 = 8;
const TRACK_HEIGHT: f32 = 4.0;
const KNOB_RADIUS: f32 = 6.0;
const VOLUME_WIDTH: i32 = 60;
/// Hide delay while playing, mirroring the macOS bar.
const AUTO_HIDE_MS: u32 = 2500;
const AUTO_HIDE_TIMER_ID: usize = 0x4C58_4342; // "LXCB"

/// Playback state the bar renders.
#[derive(Clone, Default)]
pub(crate) struct ControlsState {
    pub playing: bool,
    pub muted: bool,
    pub fullscreen: bool,
    pub position: f64,
    pub duration: f64,
    pub show_progress: bool,
    /// Active quality label; `None` hides the quality slot.
    pub quality: Option<String>,
    /// Playback rate; `None` hides the rate slot.
    pub rate: Option<f64>,
    /// Volume in `0.0..=1.0` (the bar's volume slider).
    pub volume: f64,
}

/// User intents reported to the component host. Menu anchors are screen
/// coordinates of the originating slot's top-left corner.
pub(crate) enum ControlsAction {
    TogglePlay,
    ToggleMute,
    ToggleFullscreen,
    Seek(f64),
    SetVolume(f64),
    QualityMenu { anchor: (i32, i32) },
    RateMenu { anchor: (i32, i32) },
}

/// Active drag gesture on one of the sliders.
#[derive(Clone, Copy)]
enum Drag {
    Seek(f64),
    Volume(f64),
}

pub(crate) type ControlsActionSink = Arc<dyn Fn(ControlsAction) + Send + Sync>;

/// Per-window state stashed in `GWLP_USERDATA`.
struct ControlsInner {
    sink: ControlsActionSink,
    state: ControlsState,
    width: i32,
    /// Slider drag in progress with the ratio under the cursor.
    drag: Option<Drag>,
    /// Last live seek issued during the drag (throttling).
    last_live_seek: Option<std::time::Instant>,
    /// Last observed cursor position: every repaint under a resting
    /// cursor synthesizes a WM_MOUSEMOVE, which must not count as
    /// activity or the bar never auto-hides.
    last_mouse: Option<(i32, i32)>,
}

/// Element rects computed from the current width, in the macOS bar's
/// order: play | progress | time | quality | rate | mute | volume |
/// fullscreen. Empty slots collapse to zero width.
struct BarLayout {
    play: (i32, i32),
    track: (i32, i32),
    time: (i32, i32),
    quality: (i32, i32),
    rate: (i32, i32),
    mute: (i32, i32),
    volume: (i32, i32),
    fullscreen: (i32, i32),
}

const QUALITY_WIDTH: i32 = 48;
const RATE_WIDTH: i32 = 38;

fn bar_layout(width: i32, time_width: i32, state: &ControlsState) -> BarLayout {
    let play = (SIDE_PADDING, SIDE_PADDING + BUTTON_WIDTH);
    let fullscreen = (width - SIDE_PADDING - BUTTON_WIDTH, width - SIDE_PADDING);
    let volume = (fullscreen.0 - GAP - VOLUME_WIDTH, fullscreen.0 - GAP);
    let mute = (volume.0 - 4 - BUTTON_WIDTH, volume.0 - 4);
    let rate = if state.rate.is_some() {
        (mute.0 - GAP - RATE_WIDTH, mute.0 - GAP)
    } else {
        (mute.0, mute.0)
    };
    let quality = if state.quality.is_some() {
        (rate.0 - GAP - QUALITY_WIDTH, rate.0 - GAP)
    } else {
        (rate.0, rate.0)
    };
    let time = (quality.0 - GAP - time_width, quality.0 - GAP);
    let track = if state.show_progress {
        (play.1 + GAP, time.0 - GAP)
    } else {
        (0, 0)
    };
    BarLayout {
        play,
        track,
        time,
        quality,
        rate,
        mute,
        volume,
        fullscreen,
    }
}

pub(crate) struct VideoControls {
    pub(crate) hwnd: isize,
}

impl VideoControls {
    /// Creates the (hidden) bar as a child of `parent`; `layout` positions
    /// it and `poke` reveals it.
    pub(crate) fn create(parent: HWND, sink: ControlsActionSink) -> Option<Self> {
        let hwnd = unsafe {
            WindowsAndMessaging::CreateWindowExW(
                WINDOW_EX_STYLE(WindowsAndMessaging::WS_EX_LAYERED.0),
                controls_class(),
                PCWSTR::null(),
                WINDOW_STYLE(
                    WindowsAndMessaging::WS_CHILD.0 | WindowsAndMessaging::WS_CLIPSIBLINGS.0,
                ),
                0,
                0,
                BAR_MIN_WIDTH,
                BAR_HEIGHT,
                Some(parent),
                None,
                GetModuleHandleW(None)
                    .ok()
                    .map(|module| HINSTANCE(module.0)),
                None,
            )
        }
        .ok()?;
        let inner = Box::new(ControlsInner {
            sink,
            state: ControlsState::default(),
            width: BAR_MIN_WIDTH,
            drag: None,
            last_live_seek: None,
            last_mouse: None,
        });
        unsafe {
            WindowsAndMessaging::SetWindowLongPtrW(
                hwnd,
                WindowsAndMessaging::GWLP_USERDATA,
                Box::into_raw(inner) as isize,
            );
        }
        Some(Self {
            hwnd: hwnd.0 as isize,
        })
    }

    fn window(&self) -> HWND {
        HWND(self.hwnd as *mut _)
    }

    /// Anchors the bar to the bottom edge of a `parent_width` x
    /// `parent_height` surface (full width, like the macOS bar) and
    /// repaints at the new width.
    pub(crate) fn layout(&self, parent_width: i32, parent_height: i32) {
        let width = parent_width.max(BAR_MIN_WIDTH);
        let x = 0;
        let y = parent_height - BAR_HEIGHT;
        unsafe {
            let _ = WindowsAndMessaging::MoveWindow(self.window(), x, y, width, BAR_HEIGHT, false);
            let _ = WindowsAndMessaging::SetWindowPos(
                self.window(),
                Some(WindowsAndMessaging::HWND_TOP),
                0,
                0,
                0,
                0,
                WindowsAndMessaging::SWP_NOMOVE
                    | WindowsAndMessaging::SWP_NOSIZE
                    | WindowsAndMessaging::SWP_NOACTIVATE,
            );
        }
        if let Some(inner) = inner_mut(self.window()) {
            inner.width = width;
        }
        repaint(self.window());
    }

    /// Merges the new playback state and repaints when visible.
    pub(crate) fn update(&self, state: ControlsState) {
        let Some(inner) = inner_mut(self.window()) else {
            return;
        };
        inner.state = state;
        if unsafe { WindowsAndMessaging::IsWindowVisible(self.window()) }.as_bool() {
            repaint(self.window());
        }
    }

    /// Reveals the bar (mouse activity) and restarts the auto-hide timer.
    pub(crate) fn poke(&self) {
        unsafe {
            let _ = WindowsAndMessaging::ShowWindow(self.window(), WindowsAndMessaging::SW_SHOWNA);
            let _ = WindowsAndMessaging::SetTimer(
                Some(self.window()),
                AUTO_HIDE_TIMER_ID,
                AUTO_HIDE_MS,
                None,
            );
        }
        repaint(self.window());
    }
}

fn inner_mut<'a>(hwnd: HWND) -> Option<&'a mut ControlsInner> {
    let raw =
        unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA) };
    if raw == 0 {
        None
    } else {
        Some(unsafe { &mut *(raw as *mut ControlsInner) })
    }
}

fn controls_class() -> PCWSTR {
    use std::sync::OnceLock;
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let class = WNDCLASSW {
            lpfnWndProc: Some(controls_proc),
            hInstance: unsafe { GetModuleHandleW(None) }
                .map(|module| HINSTANCE(module.0))
                .unwrap_or_default(),
            lpszClassName: w!("LingXiaVideoControls"),
            ..Default::default()
        };
        unsafe {
            WindowsAndMessaging::RegisterClassW(&class);
        }
    });
    w!("LingXiaVideoControls")
}

fn format_time(seconds: f64) -> String {
    let total = if seconds.is_finite() && seconds > 0.0 {
        seconds as i64
    } else {
        0
    };
    format!("{}:{:02}", total / 60, total % 60)
}

fn time_label(state: &ControlsState) -> String {
    format!(
        "{} / {}",
        format_time(state.position),
        format_time(state.duration)
    )
}

/// Ratio shown by the progress slider: the drag preview wins.
fn shown_ratio(inner: &ControlsInner) -> f64 {
    if let Some(Drag::Seek(ratio)) = inner.drag {
        return ratio;
    }
    if inner.state.duration > 0.0 {
        (inner.state.position / inner.state.duration).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Ratio shown by the volume slider: the drag preview wins.
fn shown_volume(inner: &ControlsInner) -> f64 {
    if let Some(Drag::Volume(ratio)) = inner.drag {
        return ratio;
    }
    inner.state.volume.clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Painting
// ---------------------------------------------------------------------------

/// Renders the bar and uploads it through `UpdateLayeredWindow`.
fn repaint(hwnd: HWND) {
    let Some(inner) = inner_mut(hwnd) else {
        return;
    };
    let width = inner.width.max(BAR_MIN_WIDTH);
    let height = BAR_HEIGHT;
    let mut pixels = capsule_pixels(width, height);

    let ratio = shown_ratio(inner);
    let volume = shown_volume(inner);
    let time_text = time_label(&inner.state);

    // Sliders drawn analytically (premultiplied) before the GDI text pass.
    let time_width = 86; // fixed slot, enough for "00:00 / 00:00"
    let layout = bar_layout(width, time_width, &inner.state);
    if inner.state.show_progress && layout.track.1 > layout.track.0 + 20 {
        draw_slider(
            &mut pixels,
            width,
            height,
            layout.track.0,
            layout.track.1,
            ratio,
            TRACK_HEIGHT,
            KNOB_RADIUS,
        );
    }
    draw_slider(
        &mut pixels,
        width,
        height,
        layout.volume.0,
        layout.volume.1,
        volume,
        3.0,
        5.0,
    );

    // GDI pass: glyphs + time text, then restore the alpha GDI zeroed.
    draw_texts(hwnd, &mut pixels, width, height, inner, &layout, &time_text);

    upload(hwnd, width, height, &pixels);
}

/// The macOS bar's gradient scrim: transparent at the top edge fading to
/// black at 0.6 opacity at the bottom, premultiplied BGRA (black, so the
/// color channels stay zero).
fn capsule_pixels(width: i32, height: i32) -> Vec<u32> {
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        let alpha = BAR_ALPHA * (y as u32 + 1) / height.max(1) as u32;
        let row = alpha << 24;
        for _ in 0..width {
            pixels.push(row);
        }
    }
    pixels
}

/// Composites an opaque element pixel over the scrim (src-over on
/// premultiplied BGRA, src alpha = coverage).
fn blend_pixel(
    pixels: &mut [u32],
    width: i32,
    x: i32,
    y: i32,
    color: (u32, u32, u32),
    coverage: f32,
) {
    if coverage <= 0.0 {
        return;
    }
    let index = (y * width + x) as usize;
    let Some(slot) = pixels.get_mut(index) else {
        return;
    };
    let existing = *slot;
    let src_alpha = (coverage * 255.0) as u32;
    let inverse = 255 - src_alpha;
    let out = |channel: u32, old: u32| (channel * src_alpha + old * inverse + 127) / 255;
    let alpha = src_alpha + ((existing >> 24) & 0xff) * inverse / 255;
    let red = out(color.0, (existing >> 16) & 0xff);
    let green = out(color.1, (existing >> 8) & 0xff);
    let blue = out(color.2, existing & 0xff);
    *slot = (alpha << 24) | (red << 16) | (green << 8) | blue;
}

/// Track, fill and knob of a horizontal slider.
fn draw_slider(
    pixels: &mut [u32],
    width: i32,
    height: i32,
    left: i32,
    right: i32,
    ratio: f64,
    track_height: f32,
    knob_radius: f32,
) {
    let center_y = height as f32 / 2.0;
    let track_top = center_y - track_height / 2.0;
    let track_bottom = center_y + track_height / 2.0;
    let knob_x = left as f32 + (right - left) as f32 * ratio as f32;
    for y in 0..height {
        for x in left..right {
            let py = y as f32 + 0.5;
            let px = x as f32 + 0.5;
            // Track band with soft edges.
            let band = (track_bottom - py).clamp(0.0, 1.0) * (py - track_top).clamp(0.0, 1.0);
            if band > 0.0 {
                let color = if px <= knob_x {
                    (0xff, 0xff, 0xff)
                } else {
                    (0x8a, 0x8a, 0x8a)
                };
                blend_pixel(pixels, width, x, y, color, band * 0.9);
            }
            // Knob.
            let dx = px - knob_x;
            let dy = py - center_y;
            let distance = (dx * dx + dy * dy).sqrt() - knob_radius;
            let coverage = (0.5 - distance).clamp(0.0, 1.0);
            if coverage > 0.0 {
                blend_pixel(pixels, width, x, y, (0xff, 0xff, 0xff), coverage);
            }
        }
    }
}

/// Segoe MDL2 Assets glyph fallback used when generated design icons
/// are not available in the app assets.
const GLYPH_PLAY: &str = "\u{E768}";
const GLYPH_PAUSE: &str = "\u{E769}";
const GLYPH_VOLUME: &str = "\u{E767}";
const GLYPH_MUTE: &str = "\u{E74F}";
const GLYPH_FULLSCREEN: &str = "\u{E740}";
const GLYPH_RESTORE: &str = "\u{E73F}";

/// Draws glyphs/time with GDI on a DIB copy of `pixels`, then restores
/// the alpha bytes GDI zeroed (text pixels take the bar alpha).
fn draw_texts(
    hwnd: HWND,
    pixels: &mut [u32],
    width: i32,
    height: i32,
    inner: &ControlsInner,
    layout: &BarLayout,
    time_text: &str,
) {
    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
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
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(dc);
            ReleaseDC(None, screen);
            return;
        };
        let old_bitmap = SelectObject(dc, HGDIOBJ(bitmap.0));
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u32, pixels.len());

        let glyph_font = CreateFontW(
            -16,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32 | FF_DONTCARE.0 as u32,
            w!("Segoe MDL2 Assets"),
        );
        let text_font = CreateFontW(
            -12,
            0,
            0,
            0,
            FW_SEMIBOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32 | FF_DONTCARE.0 as u32,
            w!("Segoe UI"),
        );
        SetBkMode(dc, TRANSPARENT);

        let draw_centered = |text: &str, slot: (i32, i32), color: u32, font: HGDIOBJ| {
            let _ = SelectObject(dc, font);
            SetTextColor(dc, COLORREF(color));
            let wide: Vec<u16> = text.encode_utf16().collect();
            let mut extent = SIZE::default();
            let _ = GetTextExtentPoint32W(dc, &wide, &mut extent);
            let x = slot.0 + ((slot.1 - slot.0) - extent.cx) / 2;
            let y = (height - extent.cy) / 2;
            let _ = TextOutW(dc, x, y, &wide);
        };

        let glyph_font_obj = HGDIOBJ(glyph_font.0);
        let text_font_obj = HGDIOBJ(text_font.0);

        let icon_rect = |slot: (i32, i32)| {
            let size = 18;
            let left = slot.0 + ((slot.1 - slot.0) - size) / 2;
            let top = (height - size) / 2;
            RECT {
                left,
                top,
                right: left + size,
                bottom: top + size,
            }
        };
        let draw_design_icon =
            |icon: WindowsDesignIcon, fallback: &str, slot: (i32, i32), color: u32| {
                if !draw_windows_design_icon_with_color(dc, icon, icon_rect(slot), color) {
                    draw_centered(fallback, slot, color, glyph_font_obj);
                }
            };

        draw_design_icon(
            if inner.state.playing {
                WindowsDesignIcon::Pause
            } else {
                WindowsDesignIcon::Play
            },
            if inner.state.playing {
                GLYPH_PAUSE
            } else {
                GLYPH_PLAY
            },
            layout.play,
            0x00f0f0f0,
        );
        draw_centered(time_text, layout.time, 0x00d8d8d8, text_font_obj);
        if let Some(quality) = inner.state.quality.as_deref() {
            draw_centered(quality, layout.quality, 0x00d8d8d8, text_font_obj);
        }
        if let Some(rate) = inner.state.rate {
            let label = if (rate - rate.round()).abs() < 0.01 {
                format!("{:.0}x", rate)
            } else {
                format!("{:.2}x", rate)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string()
            };
            draw_centered(&label, layout.rate, 0x00d8d8d8, text_font_obj);
        }
        draw_design_icon(
            if inner.state.muted {
                WindowsDesignIcon::VolumeOff
            } else {
                WindowsDesignIcon::VolumeOn
            },
            if inner.state.muted {
                GLYPH_MUTE
            } else {
                GLYPH_VOLUME
            },
            layout.mute,
            0x00d0d0d0,
        );
        draw_design_icon(
            if inner.state.fullscreen {
                WindowsDesignIcon::FullscreenExit
            } else {
                WindowsDesignIcon::FullscreenEnter
            },
            if inner.state.fullscreen {
                GLYPH_RESTORE
            } else {
                GLYPH_FULLSCREEN
            },
            layout.fullscreen,
            0x00d0d0d0,
        );

        // Copy back and fix the GDI-zeroed alpha: text/glyph pixels are
        // opaque elements over the scrim (anti-aliasing blends toward the
        // black gradient underneath, which suits the dark bar).
        std::ptr::copy_nonoverlapping(bits as *const u32, pixels.as_mut_ptr(), pixels.len());
        for pixel in pixels.iter_mut() {
            if (*pixel >> 24) == 0 && (*pixel & 0x00ff_ffff) != 0 {
                *pixel |= 0xff00_0000;
            }
        }

        let _ = SelectObject(dc, old_bitmap);
        let _ = DeleteObject(HGDIOBJ(glyph_font.0));
        let _ = DeleteObject(HGDIOBJ(text_font.0));
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
        let _ = hwnd; // window itself untouched in this pass
    }
}

/// Uploads premultiplied BGRA pixels through `UpdateLayeredWindow`.
fn upload(hwnd: HWND, width: i32, height: i32, pixels: &[u32]) {
    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
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
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(dc);
            ReleaseDC(None, screen);
            return;
        };
        let old_bitmap = SelectObject(dc, HGDIOBJ(bitmap.0));
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u32, pixels.len());

        let size = SIZE {
            cx: width,
            cy: height,
        };
        let origin = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
            ..Default::default()
        };
        let _ = UpdateLayeredWindow(
            hwnd,
            Some(screen),
            None,
            Some(&size),
            Some(dc),
            Some(&origin),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );

        let _ = SelectObject(dc, old_bitmap);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
    }
}

// ---------------------------------------------------------------------------
// Interaction
// ---------------------------------------------------------------------------

fn commit_drag(inner: &ControlsInner, drag: Drag) {
    match drag {
        Drag::Seek(ratio) => {
            let target = ratio * inner.state.duration.max(0.0);
            (inner.sink)(ControlsAction::Seek(target));
        }
        Drag::Volume(ratio) => (inner.sink)(ControlsAction::SetVolume(ratio)),
    }
}

fn slot_ratio(slot: (i32, i32), x: i32) -> f64 {
    if slot.1 <= slot.0 {
        return 0.0;
    }
    ((x - slot.0) as f64 / (slot.1 - slot.0) as f64).clamp(0.0, 1.0)
}

unsafe extern "system" fn controls_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xffff) as i16 as i32;
            let Some(inner) = inner_mut(hwnd) else {
                return LRESULT(0);
            };
            let layout = bar_layout(inner.width, 86, &inner.state);
            let hit = |slot: (i32, i32)| x >= slot.0 && x < slot.1;
            // Menus pop above the bar, anchored at the slot's top edge.
            let anchor = |slot: (i32, i32)| {
                let mut point = POINT { x: slot.0, y: 0 };
                unsafe {
                    let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut point);
                }
                (point.x, point.y)
            };
            if hit(layout.play) {
                (inner.sink)(ControlsAction::TogglePlay);
            } else if hit(layout.mute) {
                (inner.sink)(ControlsAction::ToggleMute);
            } else if hit(layout.fullscreen) {
                (inner.sink)(ControlsAction::ToggleFullscreen);
            } else if inner.state.quality.is_some() && hit(layout.quality) {
                (inner.sink)(ControlsAction::QualityMenu {
                    anchor: anchor(layout.quality),
                });
            } else if inner.state.rate.is_some() && hit(layout.rate) {
                (inner.sink)(ControlsAction::RateMenu {
                    anchor: anchor(layout.rate),
                });
            } else if hit(layout.volume) {
                unsafe {
                    SetCapture(hwnd);
                }
                let ratio = slot_ratio(layout.volume, x);
                inner.drag = Some(Drag::Volume(ratio));
                (inner.sink)(ControlsAction::SetVolume(ratio));
                repaint(hwnd);
            } else if inner.state.show_progress && hit(layout.track) {
                unsafe {
                    SetCapture(hwnd);
                }
                inner.drag = Some(Drag::Seek(slot_ratio(layout.track, x)));
                repaint(hwnd);
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_MOUSEACTIVATE => {
            // Interacting with the bar must not shuffle activation/focus —
            // an activation pass can steal the mouse capture mid-drag.
            LRESULT(WindowsAndMessaging::MA_NOACTIVATE as isize)
        }
        WindowsAndMessaging::WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xffff) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
            let mut moved = false;
            if let Some(inner) = inner_mut(hwnd) {
                moved = inner.last_mouse != Some((x, y));
                inner.last_mouse = Some((x, y));
                let layout = bar_layout(inner.width, 86, &inner.state);
                match inner.drag {
                    Some(Drag::Seek(_)) => {
                        let ratio = slot_ratio(layout.track, x);
                        inner.drag = Some(Drag::Seek(ratio));
                        // Scrub live (throttled) so the drag takes effect
                        // even if the button-up never reaches us.
                        let due = inner
                            .last_live_seek
                            .is_none_or(|last| last.elapsed().as_millis() >= 150);
                        if due {
                            inner.last_live_seek = Some(std::time::Instant::now());
                            let target = ratio * inner.state.duration.max(0.0);
                            (inner.sink)(ControlsAction::Seek(target));
                        }
                        repaint(hwnd);
                    }
                    Some(Drag::Volume(_)) => {
                        let ratio = slot_ratio(layout.volume, x);
                        inner.drag = Some(Drag::Volume(ratio));
                        (inner.sink)(ControlsAction::SetVolume(ratio));
                        repaint(hwnd);
                    }
                    None => {}
                }
            }
            // Real movement over the bar keeps it shown (repaints under a
            // resting cursor synthesize this message).
            if moved {
                unsafe {
                    let _ = WindowsAndMessaging::SetTimer(
                        Some(hwnd),
                        AUTO_HIDE_TIMER_ID,
                        AUTO_HIDE_MS,
                        None,
                    );
                }
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_LBUTTONUP => {
            if let Some(inner) = inner_mut(hwnd)
                && let Some(drag) = inner.drag.take()
            {
                unsafe {
                    let _ = ReleaseCapture();
                }
                inner.last_live_seek = None;
                commit_drag(inner, drag);
                repaint(hwnd);
            }
            LRESULT(0)
        }
        // The system (or an activation pass) took the capture away while
        // dragging: commit the position under the cursor instead of
        // dropping the gesture.
        WindowsAndMessaging::WM_CAPTURECHANGED => {
            if let Some(inner) = inner_mut(hwnd)
                && let Some(drag) = inner.drag.take()
            {
                inner.last_live_seek = None;
                commit_drag(inner, drag);
                repaint(hwnd);
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_TIMER if wparam.0 == AUTO_HIDE_TIMER_ID => {
            unsafe {
                let _ = WindowsAndMessaging::KillTimer(Some(hwnd), AUTO_HIDE_TIMER_ID);
            }
            let hide = inner_mut(hwnd)
                .map(|inner| inner.state.playing && inner.drag.is_none())
                .unwrap_or(true);
            if hide {
                unsafe {
                    let _ = WindowsAndMessaging::ShowWindow(hwnd, WindowsAndMessaging::SW_HIDE);
                }
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_NCDESTROY => {
            let raw = unsafe {
                WindowsAndMessaging::SetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA, 0)
            };
            if raw != 0 {
                drop(unsafe { Box::from_raw(raw as *mut ControlsInner) });
            }
            unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
