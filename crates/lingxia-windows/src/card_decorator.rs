//! Rounded-corner card decoration for attached webview surfaces.
//!
//! Attached cards/panels are `WS_CHILD` windows, so DWM corner rounding does
//! not apply and a GDI window region clips to an aliased staircase. Instead,
//! four tiny per-pixel-alpha "cap" child windows are layered over each card's
//! corners, above the card's WebView2 child: each cap paints the renderer's
//! card-corner color outside the rounded-corner arc, anti-aliased coverage
//! along the arc, and full transparency inside, visually rounding the card
//! without clipping it.
//!
//! This is shell chrome decoration, not webview mechanics, so it lives here in
//! the Windows host SDK and plugs into `lingxia-webview` through the
//! [`WindowsCardDecorator`] seam. `lingxia-webview` decides *when* to update
//! the caps and supplies the resolved color/radius (from the chrome renderer)
//! and the docked-bottom flag (from group state); this module owns the cap
//! windows and their GDI painting.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex, OnceLock};

use lingxia_webview::platform::windows::lingxia_host::{
    WindowsCardDecorator, set_windows_card_decorator,
};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HGDIOBJ,
    ReleaseDC, SelectObject,
};
use windows::Win32::System::LibraryLoader;
use windows::Win32::UI::WindowsAndMessaging;
use windows::core::{PCWSTR, w};

/// Registers the corner-cap decorator with `lingxia-webview`. Called once at
/// host startup.
pub(crate) fn install() {
    set_windows_card_decorator(Arc::new(WindowsCardCaps));
}

struct WindowsCardCaps;

impl WindowsCardDecorator for WindowsCardCaps {
    fn update(&self, parent: HWND, card: RECT, color: COLORREF, side: i32, square_bottom: bool) {
        if rect_width(&card) < side * 2 || rect_height(&card) < side * 2 {
            self.destroy(parent);
            return;
        }

        let sets = CORNER_CAPS.get_or_init(|| Mutex::new(HashMap::new()));
        let existing = sets
            .lock()
            .ok()
            .and_then(|sets| {
                sets.get(&hwnd_handle(parent))
                    .map(|set| (set.caps, set.side, set.color))
            })
            .filter(|(caps, cap_side, cap_color)| {
                *cap_side == side
                    && *cap_color == color.0
                    && caps.iter().all(|cap| is_window_handle_valid(*cap))
            });
        let caps = match existing {
            Some((caps, _, _)) => caps,
            None => {
                self.destroy(parent);
                let Some(caps) = create_corner_caps(parent, side, color) else {
                    return;
                };
                if let Ok(mut sets) = sets.lock() {
                    sets.insert(
                        hwnd_handle(parent),
                        CornerCapSet {
                            caps,
                            side,
                            color: color.0,
                        },
                    );
                }
                caps
            }
        };

        let positions = [
            (card.left, card.top),
            (card.right - side, card.top),
            (card.left, card.bottom - side),
            (card.right - side, card.bottom - side),
        ];
        for (index, (cap, (x, y))) in caps.iter().zip(positions).enumerate() {
            // A main card flush above a docked bottom panel keeps square bottom
            // corners: its bottom caps would notch the shared dock edge.
            let hide = square_bottom && index >= 2;
            unsafe {
                let _ = WindowsAndMessaging::SetWindowPos(
                    hwnd_from_handle(*cap),
                    Some(WindowsAndMessaging::HWND_TOP),
                    x,
                    y,
                    side,
                    side,
                    WindowsAndMessaging::SWP_NOACTIVATE
                        | WindowsAndMessaging::SWP_NOOWNERZORDER
                        | WindowsAndMessaging::SWP_NOCOPYBITS
                        | if hide {
                            WindowsAndMessaging::SWP_HIDEWINDOW
                        } else {
                            WindowsAndMessaging::SWP_SHOWWINDOW
                        },
                );
            }
        }
    }

    fn raise(&self, parent: HWND) {
        let caps = CORNER_CAPS
            .get()
            .and_then(|sets| sets.lock().ok())
            .and_then(|sets| sets.get(&hwnd_handle(parent)).map(|set| set.caps));
        let Some(caps) = caps else {
            return;
        };
        for cap in caps {
            unsafe {
                let _ = WindowsAndMessaging::SetWindowPos(
                    hwnd_from_handle(cap),
                    Some(WindowsAndMessaging::HWND_TOP),
                    0,
                    0,
                    0,
                    0,
                    WindowsAndMessaging::SWP_NOMOVE
                        | WindowsAndMessaging::SWP_NOSIZE
                        | WindowsAndMessaging::SWP_NOACTIVATE
                        | WindowsAndMessaging::SWP_NOOWNERZORDER,
                );
            }
        }
    }

    fn destroy(&self, parent: HWND) {
        let removed = CORNER_CAPS
            .get()
            .and_then(|sets| sets.lock().ok())
            .and_then(|mut sets| sets.remove(&hwnd_handle(parent)));
        let Some(set) = removed else {
            return;
        };
        for cap in set.caps {
            let cap = hwnd_from_handle(cap);
            // Group layout runs on whichever UI thread triggered it, so a cap
            // may belong to a different thread than the caller; `DestroyWindow`
            // fails cross-thread, so those caps are closed via `WM_CLOSE` on
            // their owning thread.
            unsafe {
                if WindowsAndMessaging::DestroyWindow(cap).is_err() {
                    let _ = WindowsAndMessaging::PostMessageW(
                        Some(cap),
                        WindowsAndMessaging::WM_CLOSE,
                        WPARAM::default(),
                        LPARAM::default(),
                    );
                }
            }
        }
    }
}

use windows::Win32::Foundation::RECT;

struct CornerCapSet {
    /// Cap handles ordered top-left, top-right, bottom-left, bottom-right.
    caps: [isize; 4],
    /// Cap side length (the corner radius) the bitmaps were rendered at.
    side: i32,
    /// `COLORREF` value the bitmaps were rendered with.
    color: u32,
}

/// Live corner-cap sets, keyed by the window the caps are children of.
static CORNER_CAPS: OnceLock<Mutex<HashMap<isize, CornerCapSet>>> = OnceLock::new();

/// Cap windows take no input: `WS_EX_TRANSPARENT` already excludes the layered
/// caps from hit testing, and `HTTRANSPARENT` covers any hit test that still
/// reaches the window.
unsafe extern "system" fn corner_cap_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WindowsAndMessaging::WM_NCHITTEST {
        return LRESULT(WindowsAndMessaging::HTTRANSPARENT as isize);
    }
    unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn corner_cap_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        // Register with the same module handle cap creation passes to
        // `CreateWindowExW`: classes are keyed by (name, module).
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let class = WindowsAndMessaging::WNDCLASSW {
            lpfnWndProc: Some(corner_cap_proc),
            hInstance: module,
            lpszClassName: w!("LingXiaCardCornerCap"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "corner cap class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaCardCornerCap")
}

/// Creates the four layered cap windows of one card and renders their
/// per-pixel-alpha bitmaps. Returns `None` (destroying any partial set) when a
/// window fails to create.
fn create_corner_caps(parent: HWND, side: i32, color: COLORREF) -> Option<[isize; 4]> {
    let class = corner_cap_class();
    let mut caps = [0isize; 4];
    for corner in 0..4 {
        let result = unsafe {
            WindowsAndMessaging::CreateWindowExW(
                WindowsAndMessaging::WS_EX_LAYERED
                    | WindowsAndMessaging::WS_EX_TRANSPARENT
                    | WindowsAndMessaging::WS_EX_NOACTIVATE,
                class,
                PCWSTR::null(),
                WindowsAndMessaging::WS_CHILD,
                0,
                0,
                side,
                side,
                Some(parent),
                None,
                LibraryLoader::GetModuleHandleW(None)
                    .ok()
                    .map(|module| HINSTANCE(module.0)),
                None,
            )
        };
        let cap = match result {
            Ok(cap) => cap,
            Err(err) => {
                log::warn!("corner cap creation failed for {parent:?}: {err}");
                for created in &caps[..corner] {
                    unsafe {
                        let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(*created));
                    }
                }
                return None;
            }
        };
        paint_corner_cap(cap, corner, side, color);
        caps[corner] = hwnd_handle(cap);
    }
    Some(caps)
}

/// Uploads one cap's premultiplied 32-bit ARGB bitmap via
/// `UpdateLayeredWindow` (`ULW_ALPHA`).
fn paint_corner_cap(cap: HWND, corner: usize, side: i32, color: COLORREF) {
    let pixels = corner_cap_pixels(corner, side, color);
    upload_layered_window_pixels(cap, side, side, &pixels);
}

/// Uploads a premultiplied 32-bit ARGB top-down pixel buffer to a layered
/// window via `UpdateLayeredWindow` (`ULW_ALPHA`).
fn upload_layered_window_pixels(window: HWND, width: i32, height: i32, pixels: &[u32]) {
    unsafe {
        let screen_dc = GetDC(None);
        if screen_dc.is_invalid() {
            return;
        }
        let memory_dc = CreateCompatibleDC(Some(screen_dc));
        if !memory_dc.is_invalid() {
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    // Negative height: top-down rows, matching `pixels`.
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut c_void = std::ptr::null_mut();
            if let Ok(bitmap) =
                CreateDIBSection(Some(screen_dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                && !bits.is_null()
            {
                std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u32>(), pixels.len());
                let old_bitmap = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
                let size = SIZE {
                    cx: width,
                    cy: height,
                };
                let origin = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                };
                let _ = WindowsAndMessaging::UpdateLayeredWindow(
                    window,
                    None,
                    None,
                    Some(&size),
                    Some(memory_dc),
                    Some(&origin),
                    COLORREF(0),
                    Some(&blend),
                    WindowsAndMessaging::ULW_ALPHA,
                );
                if !old_bitmap.is_invalid() {
                    let _ = SelectObject(memory_dc, old_bitmap);
                }
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
            }
            let _ = DeleteDC(memory_dc);
        }
        let _ = ReleaseDC(None, screen_dc);
    }
}

/// Premultiplied ARGB pixels of one corner cap, top-down row order.
/// `corner`: 0 top-left, 1 top-right, 2 bottom-left, 3 bottom-right.
fn corner_cap_pixels(corner: usize, side: i32, color: COLORREF) -> Vec<u32> {
    let radius = side as f32;
    // Arc center in cap-local coordinates: the cap corner pointing into the
    // card interior.
    let (center_x, center_y) = match corner {
        0 => (radius, radius),
        1 => (0.0, radius),
        2 => (radius, 0.0),
        _ => (0.0, 0.0),
    };
    let red = color.0 & 0xff;
    let green = (color.0 >> 8) & 0xff;
    let blue = (color.0 >> 16) & 0xff;
    let mut pixels = Vec::with_capacity((side * side) as usize);
    for y in 0..side {
        for x in 0..side {
            let mut outside = 0u32;
            for sub_y in 0..4 {
                for sub_x in 0..4 {
                    let sample_x = x as f32 + (sub_x as f32 + 0.5) / 4.0;
                    let sample_y = y as f32 + (sub_y as f32 + 0.5) / 4.0;
                    let dx = sample_x - center_x;
                    let dy = sample_y - center_y;
                    if dx * dx + dy * dy > radius * radius {
                        outside += 1;
                    }
                }
            }
            let alpha = outside * 255 / 16;
            let premultiply = |channel: u32| channel * alpha / 255;
            pixels.push(
                (alpha << 24)
                    | (premultiply(red) << 16)
                    | (premultiply(green) << 8)
                    | premultiply(blue),
            );
        }
    }
    pixels
}

fn hwnd_handle(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

fn hwnd_from_handle(handle: isize) -> HWND {
    HWND(handle as *mut c_void)
}

fn is_window_handle_valid(handle: isize) -> bool {
    if handle == 0 {
        return false;
    }
    unsafe { WindowsAndMessaging::IsWindow(Some(hwnd_from_handle(handle))).as_bool() }
}

fn rect_width(rect: &RECT) -> i32 {
    rect.right - rect.left
}

fn rect_height(rect: &RECT) -> i32 {
    rect.bottom - rect.top
}
