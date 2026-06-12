//! Native playback controls for the video component — the macOS player's
//! bottom bar (`MacLxMediaPlayer`): play/pause, a seekable progress
//! slider, elapsed/total time, mute and fullscreen toggles, auto-hiding
//! while playing.
//!
//! The bar is a per-pixel-alpha layered child window floating over the
//! video surface: an analytically rendered translucent capsule
//! (anti-aliased rounded rect + slider) with GDI-drawn glyphs (Segoe MDL2
//! Assets) and time text (Segoe UI), followed by an alpha fix-up because
//! GDI zeroes the alpha of the pixels it touches — the same approach as
//! the device-frame toolbar. All calls run on the UI thread that owns the
//! parent window.

use std::sync::Arc;

use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleDC, CreateDIBSection, CreateFontW,
    DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, DeleteDC, DeleteObject, FF_DONTCARE,
    FW_NORMAL, FW_SEMIBOLD, GetDC, GetTextExtentPoint32W, HGDIOBJ, OUT_DEFAULT_PRECIS, ReleaseDC,
    SelectObject, SetBkMode, SetTextColor, TRANSPARENT, TextOutW,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    self, ULW_ALPHA, UpdateLayeredWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW,
};
use windows::core::{PCWSTR, w};

/// Bar metrics (device pixels; the bar is small enough that DPI-scaling
/// can come later with the rest of the component chrome).
const BAR_HEIGHT: i32 = 36;
const BAR_RADIUS: f32 = 8.0;
const BAR_MARGIN: i32 = 12;
const BAR_MAX_WIDTH: i32 = 640;
const BAR_MIN_WIDTH: i32 = 220;
const BAR_COLOR: (u32, u32, u32) = (0x20, 0x20, 0x20);
const BAR_ALPHA: u32 = 230;
const BUTTON_WIDTH: i32 = 30;
const SIDE_PADDING: i32 = 10;
const TRACK_HEIGHT: f32 = 4.0;
const KNOB_RADIUS: f32 = 6.0;
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
}

/// User intents reported to the component host.
pub(crate) enum ControlsAction {
    TogglePlay,
    ToggleMute,
    ToggleFullscreen,
    Seek(f64),
}

pub(crate) type ControlsActionSink = Arc<dyn Fn(ControlsAction) + Send + Sync>;

/// Per-window state stashed in `GWLP_USERDATA`.
struct ControlsInner {
    sink: ControlsActionSink,
    state: ControlsState,
    width: i32,
    /// Seek drag in progress: the ratio under the cursor.
    drag_ratio: Option<f64>,
}

/// Element rects computed from the current width.
struct BarLayout {
    play: (i32, i32),
    time: (i32, i32),
    track: (i32, i32),
    mute: (i32, i32),
    fullscreen: (i32, i32),
}

fn bar_layout(width: i32, time_width: i32, show_progress: bool) -> BarLayout {
    let play = (SIDE_PADDING, SIDE_PADDING + BUTTON_WIDTH);
    let time = (play.1, play.1 + time_width);
    let fullscreen = (width - SIDE_PADDING - BUTTON_WIDTH, width - SIDE_PADDING);
    let mute = (fullscreen.0 - BUTTON_WIDTH, fullscreen.0);
    let track = if show_progress {
        (time.1 + 10, mute.0 - 10)
    } else {
        (0, 0)
    };
    BarLayout {
        play,
        time,
        track,
        mute,
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
            drag_ratio: None,
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

    /// Anchors the bar to the bottom of a `parent_width`x`parent_height`
    /// surface and repaints at the new width.
    pub(crate) fn layout(&self, parent_width: i32, parent_height: i32) {
        let width = (parent_width - 2 * BAR_MARGIN).clamp(BAR_MIN_WIDTH, BAR_MAX_WIDTH);
        let x = (parent_width - width) / 2;
        let y = parent_height - BAR_HEIGHT - BAR_MARGIN;
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

    pub(crate) fn hide(&self) {
        unsafe {
            let _ = WindowsAndMessaging::ShowWindow(self.window(), WindowsAndMessaging::SW_HIDE);
        }
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

/// Ratio shown by the slider: the drag preview wins over playback.
fn shown_ratio(inner: &ControlsInner) -> f64 {
    inner.drag_ratio.unwrap_or_else(|| {
        if inner.state.duration > 0.0 {
            (inner.state.position / inner.state.duration).clamp(0.0, 1.0)
        } else {
            0.0
        }
    })
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
    let time_text = time_label(&inner.state);

    // Slider drawn analytically (premultiplied) before the GDI text pass.
    let time_width = 86; // fixed slot, enough for "00:00 / 00:00"
    let layout = bar_layout(width, time_width, inner.state.show_progress);
    if inner.state.show_progress && layout.track.1 > layout.track.0 + 20 {
        draw_slider(
            &mut pixels,
            width,
            height,
            layout.track.0,
            layout.track.1,
            ratio,
        );
    }

    // GDI pass: glyphs + time text, then restore the alpha GDI zeroed.
    draw_texts(hwnd, &mut pixels, width, height, inner, &layout, &time_text);

    upload(hwnd, width, height, &pixels);
}

/// The translucent capsule background with 1px anti-aliased corners,
/// premultiplied BGRA.
fn capsule_pixels(width: i32, height: i32) -> Vec<u32> {
    let (red, green, blue) = BAR_COLOR;
    let half_w = width as f32 / 2.0 - BAR_RADIUS;
    let half_h = height as f32 / 2.0 - BAR_RADIUS;
    let center_x = width as f32 / 2.0;
    let center_y = height as f32 / 2.0;
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            let qx = (x as f32 + 0.5 - center_x).abs() - half_w;
            let qy = (y as f32 + 0.5 - center_y).abs() - half_h;
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
            let distance = outside + qx.max(qy).min(0.0) - BAR_RADIUS;
            let coverage = (0.5 - distance).clamp(0.0, 1.0);
            let alpha = (BAR_ALPHA as f32 * coverage) as u32;
            let premultiply = |channel: u32| (channel * alpha + 127) / 255;
            pixels.push(
                (alpha << 24) | (premultiply(red) << 16) | (premultiply(green) << 8) | premultiply(blue),
            );
        }
    }
    pixels
}

/// Blends an opaque-over-bar pixel (premultiplied against BAR_ALPHA).
fn blend_pixel(pixels: &mut [u32], width: i32, x: i32, y: i32, color: (u32, u32, u32), coverage: f32) {
    if coverage <= 0.0 {
        return;
    }
    let index = (y * width + x) as usize;
    let Some(slot) = pixels.get_mut(index) else {
        return;
    };
    let existing = *slot;
    let alpha = (existing >> 24) & 0xff;
    let mix = |old: u32, new: u32| -> u32 {
        let new_pre = (new * alpha + 127) / 255;
        (old as f32 + (new_pre as f32 - old as f32) * coverage) as u32
    };
    let red = mix((existing >> 16) & 0xff, color.0);
    let green = mix((existing >> 8) & 0xff, color.1);
    let blue = mix(existing & 0xff, color.2);
    *slot = (alpha << 24) | (red << 16) | (green << 8) | blue;
}

/// Track, fill and knob of the progress slider.
fn draw_slider(pixels: &mut [u32], width: i32, height: i32, left: i32, right: i32, ratio: f64) {
    let center_y = height as f32 / 2.0;
    let track_top = center_y - TRACK_HEIGHT / 2.0;
    let track_bottom = center_y + TRACK_HEIGHT / 2.0;
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
                    (0x6a, 0x6a, 0x6a)
                };
                blend_pixel(pixels, width, x, y, color, band);
            }
            // Knob.
            let dx = px - knob_x;
            let dy = py - center_y;
            let distance = (dx * dx + dy * dy).sqrt() - KNOB_RADIUS;
            let coverage = (0.5 - distance).clamp(0.0, 1.0);
            if coverage > 0.0 {
                blend_pixel(pixels, width, x, y, (0xff, 0xff, 0xff), coverage);
            }
        }
    }
}

/// Segoe MDL2 Assets glyphs.
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
            (DEFAULT_PITCH.0 as u32 | FF_DONTCARE.0 as u32).into(),
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
            (DEFAULT_PITCH.0 as u32 | FF_DONTCARE.0 as u32).into(),
            w!("Segoe UI"),
        );
        SetBkMode(dc, TRANSPARENT);

        let mut draw_centered = |text: &str, slot: (i32, i32), color: u32, font: HGDIOBJ| {
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
        draw_centered(
            if inner.state.playing { GLYPH_PAUSE } else { GLYPH_PLAY },
            layout.play,
            0x00f0f0f0,
            glyph_font_obj,
        );
        draw_centered(time_text, layout.time, 0x00d8d8d8, text_font_obj);
        draw_centered(
            if inner.state.muted { GLYPH_MUTE } else { GLYPH_VOLUME },
            layout.mute,
            0x00d0d0d0,
            glyph_font_obj,
        );
        draw_centered(
            if inner.state.fullscreen { GLYPH_RESTORE } else { GLYPH_FULLSCREEN },
            layout.fullscreen,
            0x00d0d0d0,
            glyph_font_obj,
        );

        // Copy back and fix the GDI-zeroed alpha (premultiplied to the
        // bar alpha, like the device-frame toolbar text).
        std::ptr::copy_nonoverlapping(bits as *const u32, pixels.as_mut_ptr(), pixels.len());
        for pixel in pixels.iter_mut() {
            if (*pixel >> 24) == 0 && (*pixel & 0x00ff_ffff) != 0 {
                let red = ((*pixel >> 16) & 0xff) * BAR_ALPHA / 255;
                let green = ((*pixel >> 8) & 0xff) * BAR_ALPHA / 255;
                let blue = (*pixel & 0xff) * BAR_ALPHA / 255;
                *pixel = (BAR_ALPHA << 24) | (red << 16) | (green << 8) | blue;
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

fn ratio_at(inner: &ControlsInner, x: i32) -> Option<f64> {
    let layout = bar_layout(inner.width, 86, inner.state.show_progress);
    if !inner.state.show_progress || layout.track.1 <= layout.track.0 {
        return None;
    }
    Some(((x - layout.track.0) as f64 / (layout.track.1 - layout.track.0) as f64).clamp(0.0, 1.0))
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
            let layout = bar_layout(inner.width, 86, inner.state.show_progress);
            let hit = |slot: (i32, i32)| x >= slot.0 && x < slot.1;
            if hit(layout.play) {
                (inner.sink)(ControlsAction::TogglePlay);
            } else if hit(layout.mute) {
                (inner.sink)(ControlsAction::ToggleMute);
            } else if hit(layout.fullscreen) {
                (inner.sink)(ControlsAction::ToggleFullscreen);
            } else if inner.state.show_progress && hit(layout.track) {
                unsafe {
                    SetCapture(hwnd);
                }
                inner.drag_ratio = ratio_at(inner, x);
                repaint(hwnd);
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xffff) as i16 as i32;
            if let Some(inner) = inner_mut(hwnd)
                && inner.drag_ratio.is_some()
            {
                inner.drag_ratio = ratio_at(inner, x);
                repaint(hwnd);
            }
            // Mouse over the bar keeps it shown.
            unsafe {
                let _ = WindowsAndMessaging::SetTimer(
                    Some(hwnd),
                    AUTO_HIDE_TIMER_ID,
                    AUTO_HIDE_MS,
                    None,
                );
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_LBUTTONUP => {
            if let Some(inner) = inner_mut(hwnd)
                && let Some(ratio) = inner.drag_ratio.take()
            {
                unsafe {
                    let _ = ReleaseCapture();
                }
                let target = ratio * inner.state.duration.max(0.0);
                (inner.sink)(ControlsAction::Seek(target));
                repaint(hwnd);
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_TIMER if wparam.0 == AUTO_HIDE_TIMER_ID => {
            unsafe {
                let _ = WindowsAndMessaging::KillTimer(Some(hwnd), AUTO_HIDE_TIMER_ID);
            }
            let hide = inner_mut(hwnd)
                .map(|inner| inner.state.playing && inner.drag_ratio.is_none())
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
