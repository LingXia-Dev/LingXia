//! The chip that follows the pointer while a shell element is dragged (an
//! aside tab toward the sidebar, a sidebar group toward the main's edge).
//! Windows has no drag image for a drag the shell runs itself, so without it
//! the only feedback would be the cursor; this is the macOS drag snapshot's
//! counterpart: the element's name on a translucent chip, click-through,
//! owned by the host so it stays with the app.

use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HGDIOBJ,
    ReleaseDC, SelectObject,
};
use windows::Win32::System::LibraryLoader;
use windows::Win32::UI::WindowsAndMessaging::{
    self, CreateWindowExW, DefWindowProcW, DestroyWindow, HWND_TOP, RegisterClassW, SWP_NOACTIVATE,
    SWP_NOSIZE, SWP_SHOWWINDOW, SetWindowPos, UpdateLayeredWindow, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

/// The chip's colors (`0xRRGGBB`): fill, hairline border, text.
#[derive(Clone, Copy)]
pub(crate) struct GhostColors {
    pub fill: u32,
    pub border: u32,
    pub text: u32,
}

struct Ghost {
    window: isize,
    width: i32,
    height: i32,
}

static GHOST: Mutex<Option<Ghost>> = Mutex::new(None);

fn class_name() -> PCWSTR {
    w!("LingXiaDragGhost")
}

fn px(dp: i32) -> i32 {
    crate::dpi::px(dp)
}

/// Show the chip for `label` under the pointer at `screen`. Replaces any
/// chip already shown.
pub(crate) fn show(owner: HWND, label: &str, colors: GhostColors, screen: POINT) {
    hide();
    let label = label.trim();
    if label.is_empty() {
        return;
    }
    let width = (px(28) + px(7) * label.chars().count().min(24) as i32).clamp(px(72), px(220));
    let height = px(28);
    let Some(window) = create(owner, width, height) else {
        return;
    };
    if !paint(window, width, height, label, colors) {
        unsafe {
            let _ = DestroyWindow(window);
        }
        return;
    }
    if let Ok(mut ghost) = GHOST.lock() {
        *ghost = Some(Ghost {
            window: window.0 as isize,
            width,
            height,
        });
    }
    move_to(screen);
}

/// Keep the chip just below and right of the pointer, like a drag image.
pub(crate) fn move_to(screen: POINT) {
    let Some((window, _, _)) = current() else {
        return;
    };
    unsafe {
        let _ = SetWindowPos(
            window,
            Some(HWND_TOP),
            screen.x + px(12),
            screen.y + px(14),
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
    }
}

pub(crate) fn hide() {
    let ghost = GHOST.lock().ok().and_then(|mut ghost| ghost.take());
    if let Some(ghost) = ghost {
        unsafe {
            let _ = DestroyWindow(HWND(ghost.window as *mut c_void));
        }
    }
}

fn current() -> Option<(HWND, i32, i32)> {
    GHOST.lock().ok().and_then(|ghost| {
        ghost
            .as_ref()
            .map(|ghost| (HWND(ghost.window as *mut c_void), ghost.width, ghost.height))
    })
}

fn create(owner: HWND, width: i32, height: i32) -> Option<HWND> {
    register();
    let instance = unsafe { LibraryLoader::GetModuleHandleW(None) }
        .ok()
        .map(|module| windows::Win32::Foundation::HINSTANCE(module.0));
    unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            class_name(),
            w!("LingXia Drag"),
            WS_POPUP,
            0,
            0,
            width,
            height,
            Some(owner),
            None,
            instance,
            None,
        )
    }
    .ok()
}

fn register() {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .ok()
            .map(|module| windows::Win32::Foundation::HINSTANCE(module.0))
            .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: module,
            lpszClassName: class_name(),
            ..Default::default()
        };
        if unsafe { RegisterClassW(&class) } == 0 {
            log::error!(
                "drag ghost class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WindowsAndMessaging::WM_NCHITTEST {
        return LRESULT(WindowsAndMessaging::HTTRANSPARENT as isize);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn paint(hwnd: HWND, width: i32, height: i32, label: &str, colors: GhostColors) -> bool {
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return false;
        }
        let dc = CreateCompatibleDC(Some(screen));
        if dc.is_invalid() {
            let _ = ReleaseDC(None, screen);
            return false;
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
        let Ok(bitmap) = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(dc);
            let _ = ReleaseDC(None, screen);
            return false;
        };
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            let _ = ReleaseDC(None, screen);
            return false;
        }
        let old = SelectObject(dc, HGDIOBJ(bitmap.0));
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u32>(), (width * height) as usize);
        fill_chip(pixels, width, height, colors);
        let text = RECT {
            left: px(12),
            top: 0,
            right: width - px(12),
            bottom: height,
        };
        crate::layered_text::blend_supersampled_text_mask(
            dc,
            pixels,
            width,
            height,
            label,
            text,
            colors.text,
            px(12),
            500,
            false,
        );
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let origin = POINT::default();
        // The chip reads as a drag image: slightly see-through, like the
        // macOS drag snapshot.
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 230,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let ok = UpdateLayeredWindow(
            hwnd,
            Some(screen),
            None,
            Some(&size),
            Some(dc),
            Some(&origin),
            COLORREF(0),
            Some(&blend),
            WindowsAndMessaging::ULW_ALPHA,
        )
        .is_ok();
        if !old.is_invalid() {
            let _ = SelectObject(dc, old);
        }
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
        let _ = ReleaseDC(None, screen);
        ok
    }
}

fn fill_chip(pixels: &mut [u32], width: i32, height: i32, colors: GhostColors) {
    let radius = px(8) as f32;
    let border = px(1).max(1) as f32;
    let channels = |rgb: u32| ((rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff);
    let (fr, fg, fb) = channels(colors.fill);
    let (br, bg, bb) = channels(colors.border);
    for y in 0..height {
        for x in 0..width {
            let outer = coverage(x, y, width as f32, height as f32, radius, 0.0);
            if outer == 0 {
                pixels[(y * width + x) as usize] = 0;
                continue;
            }
            let inner = coverage(x, y, width as f32, height as f32, radius, border);
            // Border where the outer shape covers and the inner does not.
            let mut pixel = premultiply(br, bg, bb, outer);
            if inner > 0 {
                pixel = src_over(pixel, premultiply(fr, fg, fb, inner));
            }
            pixels[(y * width + x) as usize] = pixel;
        }
    }
}

/// Anti-aliased coverage of a rounded rect inset by `inset` on every side.
fn coverage(x: i32, y: i32, width: f32, height: f32, radius: f32, inset: f32) -> u32 {
    let px = x as f32 + 0.5 - width / 2.0;
    let py = y as f32 + 0.5 - height / 2.0;
    let half_w = width / 2.0 - inset;
    let half_h = height / 2.0 - inset;
    let radius = (radius - inset).max(0.0);
    let qx = px.abs() - half_w + radius;
    let qy = py.abs() - half_h + radius;
    let dist = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - radius;
    if dist <= -0.5 {
        255
    } else if dist >= 0.5 {
        0
    } else {
        ((0.5 - dist) * 255.0).round() as u32
    }
}

fn premultiply(red: u32, green: u32, blue: u32, alpha: u32) -> u32 {
    let scale = |channel: u32| (channel * alpha + 127) / 255;
    (alpha << 24) | (scale(red) << 16) | (scale(green) << 8) | scale(blue)
}

fn src_over(dst: u32, src: u32) -> u32 {
    let sa = (src >> 24) & 0xff;
    if sa == 0 {
        return dst;
    }
    if sa == 255 {
        return src;
    }
    let inv = 255 - sa;
    let channel = |src_c: u32, dst_c: u32| src_c + (dst_c * inv + 127) / 255;
    let a = sa + (((dst >> 24) & 0xff) * inv + 127) / 255;
    (a << 24)
        | (channel((src >> 16) & 0xff, (dst >> 16) & 0xff) << 16)
        | (channel((src >> 8) & 0xff, (dst >> 8) & 0xff) << 8)
        | channel(src & 0xff, dst & 0xff)
}
