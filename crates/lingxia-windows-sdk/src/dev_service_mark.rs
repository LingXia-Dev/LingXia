//! Prod build on the dev service. One owned, click-through chip per top-level
//! window, raised above that window's floats. It is not topmost, so it stays
//! with the app and does not cover other programs.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC,
    GetDeviceCaps, HGDIOBJ, LOGPIXELSX, ReleaseDC, SelectObject,
};
use windows::Win32::System::LibraryLoader;
use windows::Win32::UI::WindowsAndMessaging::{
    self, CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindow, GetWindowRect, HWND_TOP,
    IsIconic, IsWindowVisible, RegisterClassW, SWP_NOACTIVATE, SWP_NOSIZE, SWP_SHOWWINDOW,
    SetWindowPos, UpdateLayeredWindow, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

fn class_name() -> PCWSTR {
    w!("LingXiaDevServiceMark")
}

struct Mark {
    window: isize,
    width: i32,
    height: i32,
}

static MARKS: OnceLock<Mutex<HashMap<isize, Mark>>> = OnceLock::new();

fn marks() -> &'static Mutex<HashMap<isize, Mark>> {
    MARKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn handle(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

fn from_handle(value: isize) -> HWND {
    HWND(value as *mut c_void)
}

/// Keep the chip glued to a top-level window. Owned popups (floats, sheets)
/// are skipped; the owner's chip is raised above them.
pub(crate) fn sync(hwnd: HWND) {
    if !is_host(hwnd) {
        return;
    }
    if !lingxia_app_context::dev_service_banner()
        || !unsafe { IsWindowVisible(hwnd) }.as_bool()
        || unsafe { IsIconic(hwnd) }.as_bool()
    {
        destroy(hwnd);
        return;
    }
    let mut frame = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut frame) }.is_err() {
        return;
    }
    let window_width = frame.right - frame.left;
    let window_height = frame.bottom - frame.top;
    if window_width < 200 || window_height < 200 {
        destroy(hwnd);
        return;
    }
    let dpi = window_dpi(hwnd);
    let (width, height) = chip_size(dpi);
    let x = frame.right - width - px(dpi, 16);
    let y = frame.bottom - height - px(dpi, 16);
    let owner = handle(hwnd);
    let existing = marks().lock().ok().and_then(|map| {
        map.get(&owner)
            .map(|mark| (mark.window, mark.width, mark.height))
    });
    let window = if let Some((window, painted_width, painted_height)) = existing
        && painted_width == width
        && painted_height == height
        && is_alive(window)
    {
        window
    } else {
        destroy(hwnd);
        let Some(window) = create(hwnd, x, y, width, height) else {
            return;
        };
        if !paint(from_handle(window), width, height, dpi) {
            unsafe {
                let _ = DestroyWindow(from_handle(window));
            }
            return;
        }
        if let Ok(mut map) = marks().lock() {
            map.insert(
                owner,
                Mark {
                    window,
                    width,
                    height,
                },
            );
        }
        log::info!("dev service mark shown");
        window
    };
    unsafe {
        let _ = SetWindowPos(
            from_handle(window),
            Some(HWND_TOP),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

/// After a float is brought to the top, put the chip back above it.
pub(crate) fn raise(owner: HWND) {
    let Some(window) = marks()
        .lock()
        .ok()
        .and_then(|map| map.get(&handle(owner)).map(|mark| mark.window))
    else {
        return;
    };
    if !is_alive(window) {
        return;
    }
    unsafe {
        let _ = SetWindowPos(
            from_handle(window),
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | WindowsAndMessaging::SWP_NOMOVE,
        );
    }
}

pub(crate) fn destroy(owner: HWND) {
    let Some(mark) = marks()
        .lock()
        .ok()
        .and_then(|mut map| map.remove(&handle(owner)))
    else {
        return;
    };
    if is_alive(mark.window) {
        unsafe {
            let _ = DestroyWindow(from_handle(mark.window));
        }
    }
}

fn is_host(hwnd: HWND) -> bool {
    unsafe {
        let owner = GetWindow(hwnd, WindowsAndMessaging::GW_OWNER).unwrap_or_default();
        if !owner.0.is_null() {
            return false;
        }
        WindowsAndMessaging::GetParent(hwnd)
            .map(|parent| parent.0.is_null())
            .unwrap_or(true)
    }
}

fn is_alive(window: isize) -> bool {
    window != 0 && unsafe { WindowsAndMessaging::IsWindow(Some(from_handle(window))) }.as_bool()
}

fn window_dpi(hwnd: HWND) -> i32 {
    unsafe {
        let dc = GetDC(Some(hwnd));
        if dc.is_invalid() {
            return 96;
        }
        let dpi = GetDeviceCaps(Some(dc), LOGPIXELSX);
        let _ = ReleaseDC(Some(hwnd), dc);
        if dpi > 0 { dpi } else { 96 }
    }
}

fn px(dpi: i32, dp: i32) -> i32 {
    (dp * dpi + 48) / 96
}

fn chip_size(dpi: i32) -> (i32, i32) {
    (px(dpi, 58), px(dpi, 22))
}

fn create(owner: HWND, x: i32, y: i32, width: i32, height: i32) -> Option<isize> {
    register();
    let instance = unsafe { LibraryLoader::GetModuleHandleW(None) }
        .ok()
        .map(|module| windows::Win32::Foundation::HINSTANCE(module.0));
    let window = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            class_name(),
            PCWSTR::null(),
            WS_POPUP,
            x,
            y,
            width,
            height,
            Some(owner),
            None,
            instance,
            None,
        )
    }
    .ok()?;
    Some(handle(window))
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
                "dev service mark class registration failed: {}",
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

fn paint(hwnd: HWND, width: i32, height: i32, dpi: i32) -> bool {
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
        fill_chip(pixels, width, height, dpi);
        let text = RECT {
            left: px(dpi, 18),
            top: 0,
            right: width - px(dpi, 8),
            bottom: height,
        };
        crate::layered_text::blend_supersampled_text_mask(
            dc,
            pixels,
            width,
            height,
            "DEV",
            text,
            0x00ff_ffff,
            (11 * dpi + 36) / 72,
            500,
            false,
        );
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let origin = POINT::default();
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
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

fn fill_chip(pixels: &mut [u32], width: i32, height: i32, dpi: i32) {
    let radius = px(dpi, 10) as f32;
    let dot = px(dpi, 6) as f32;
    let cx = px(dpi, 8) as f32 + dot / 2.0;
    let cy = height as f32 / 2.0;
    for y in 0..height {
        for x in 0..width {
            let cover = round_rect_coverage(x, y, width, height, radius);
            if cover == 0 {
                pixels[(y * width + x) as usize] = 0;
                continue;
            }
            let mut pixel = premultiply(198, 40, 40, cover);
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let dist = (dx * dx + dy * dy).sqrt() - dot / 2.0;
            let dot_cover = if dist <= -0.5 {
                255
            } else if dist >= 0.5 {
                0
            } else {
                ((0.5 - dist) * 255.0).round() as u32
            };
            if dot_cover > 0 {
                pixel = src_over(pixel, premultiply(255, 255, 255, dot_cover));
            }
            pixels[(y * width + x) as usize] = pixel;
        }
    }
}

fn round_rect_coverage(x: i32, y: i32, width: i32, height: i32, radius: f32) -> u32 {
    let px = x as f32 + 0.5 - width as f32 / 2.0;
    let py = y as f32 + 0.5 - height as f32 / 2.0;
    let half_w = width as f32 / 2.0;
    let half_h = height as f32 / 2.0;
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
