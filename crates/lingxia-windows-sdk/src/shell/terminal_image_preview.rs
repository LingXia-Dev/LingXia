//! Native resizable preview for images drawn inside a terminal pane.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};

use image::ImageReader;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLACK_BRUSH, BeginPaint, DIB_RGB_COLORS, EndPaint,
    FillRect, GetStockObject, HALFTONE, HBRUSH, MONITOR_DEFAULTTONEAREST, MONITORINFO, PAINTSTRUCT,
    SRCCOPY, SetStretchBltMode, StretchDIBits,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows::Win32::UI::WindowsAndMessaging::{
    self, WINDOW_EX_STYLE, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};
use windows::core::{PCWSTR, w};

use super::terminal_grid::TerminalPreviewImage;

const DEFAULT_CLIENT_WIDTH: i32 = 760;
const DEFAULT_CLIENT_HEIGHT: i32 = 560;

struct PreviewState {
    owner: isize,
    width: u32,
    height: u32,
    /// Top-down BGRA pixels composited against the preview's black backdrop.
    pixels: Vec<u8>,
}

static PREVIEWS: OnceLock<Mutex<HashMap<isize, isize>>> = OnceLock::new();

fn previews() -> std::sync::MutexGuard<'static, HashMap<isize, isize>> {
    PREVIEWS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) fn show(owner: HWND, image: TerminalPreviewImage) {
    let decoded = match ImageReader::new(std::io::Cursor::new(&image.png))
        .with_guessed_format()
        .and_then(|reader| reader.decode().map_err(std::io::Error::other))
    {
        Ok(decoded) => decoded.to_rgba8(),
        Err(error) => {
            log::error!("terminal image preview decode failed: {error}");
            return;
        }
    };
    let (width, height) = decoded.dimensions();
    let mut pixels = decoded.into_raw();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
        }
        pixel[3] = 255;
    }

    let owner_key = owner.0 as isize;
    let previous = { previews().remove(&owner_key) };
    if let Some(previous) = previous {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(HWND(previous as *mut c_void));
        }
    }

    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(owner) }.max(96);
    let scale = |logical: i32| logical.saturating_mul(dpi as i32) / 96;
    let style = WS_OVERLAPPEDWINDOW;
    let ex_style = WINDOW_EX_STYLE::default();
    let mut bounds = RECT {
        left: 0,
        top: 0,
        right: scale(DEFAULT_CLIENT_WIDTH),
        bottom: scale(DEFAULT_CLIENT_HEIGHT),
    };
    unsafe {
        let _ = windows::Win32::UI::HiDpi::AdjustWindowRectExForDpi(
            &mut bounds,
            style,
            false,
            ex_style,
            dpi,
        );
    }
    let window_width = bounds.right - bounds.left;
    let window_height = bounds.bottom - bounds.top;
    let (x, y) = centered_origin(owner, window_width, window_height);
    let title = wide(&format!("Terminal Image {}", image.id));
    let window = match unsafe {
        WindowsAndMessaging::CreateWindowExW(
            ex_style,
            preview_class(),
            PCWSTR(title.as_ptr()),
            style,
            x,
            y,
            window_width,
            window_height,
            Some(owner),
            None,
            GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    } {
        Ok(window) => window,
        Err(error) => {
            log::error!("terminal image preview window failed: {error}");
            return;
        }
    };

    let state = Box::new(PreviewState {
        owner: owner_key,
        width,
        height,
        pixels,
    });
    unsafe {
        WindowsAndMessaging::SetWindowLongPtrW(
            window,
            WindowsAndMessaging::GWLP_USERDATA,
            Box::into_raw(state) as isize,
        );
    }
    previews().insert(owner_key, window.0 as isize);
    dress_window(window);
    unsafe {
        let _ = WindowsAndMessaging::ShowWindow(window, WindowsAndMessaging::SW_SHOW);
        let _ = WindowsAndMessaging::SetForegroundWindow(window);
    }
}

fn preview_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let class = WNDCLASSW {
            lpfnWndProc: Some(preview_proc),
            hInstance: unsafe { GetModuleHandleW(None) }
                .map(|module| HINSTANCE(module.0))
                .unwrap_or_default(),
            hCursor: unsafe {
                WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_ARROW)
            }
            .unwrap_or_default(),
            hbrBackground: HBRUSH(unsafe { GetStockObject(BLACK_BRUSH) }.0),
            lpszClassName: w!("LingXiaTerminalImagePreview"),
            ..Default::default()
        };
        unsafe {
            WindowsAndMessaging::RegisterClassW(&class);
        }
    });
    w!("LingXiaTerminalImagePreview")
}

unsafe extern "system" fn preview_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WindowsAndMessaging::WM_KEYDOWN if wparam.0 as u16 == VK_ESCAPE.0 => {
            unsafe {
                let _ = WindowsAndMessaging::DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_NCDESTROY => {
            let raw = unsafe {
                WindowsAndMessaging::SetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA, 0)
            };
            if raw != 0 {
                let state = unsafe { Box::from_raw(raw as *mut PreviewState) };
                let mut open = previews();
                if open.get(&state.owner).copied() == Some(hwnd.0 as isize) {
                    open.remove(&state.owner);
                }
            }
            unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn paint(hwnd: HWND) {
    let mut paint = PAINTSTRUCT::default();
    let dc = unsafe { BeginPaint(hwnd, &mut paint) };
    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
        let _ = FillRect(dc, &client, HBRUSH(GetStockObject(BLACK_BRUSH).0));
    }
    let raw =
        unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA) };
    if raw != 0 {
        let state = unsafe { &*(raw as *const PreviewState) };
        let target = aspect_fit(
            state.width,
            state.height,
            client.right.max(0),
            client.bottom.max(0),
        );
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: state.width as i32,
                biHeight: -(state.height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        unsafe {
            let _ = SetStretchBltMode(dc, HALFTONE);
            StretchDIBits(
                dc,
                target.left,
                target.top,
                target.right - target.left,
                target.bottom - target.top,
                0,
                0,
                state.width as i32,
                state.height as i32,
                Some(state.pixels.as_ptr().cast()),
                &info,
                DIB_RGB_COLORS,
                SRCCOPY,
            );
        }
    }
    unsafe {
        let _ = EndPaint(hwnd, &paint);
    }
}

fn aspect_fit(image_width: u32, image_height: u32, width: i32, height: i32) -> RECT {
    if image_width == 0 || image_height == 0 || width <= 0 || height <= 0 {
        return RECT::default();
    }
    let image_ratio = image_width as f64 / image_height as f64;
    let client_ratio = width as f64 / height as f64;
    let (draw_width, draw_height) = if image_ratio > client_ratio {
        (width, (width as f64 / image_ratio).round() as i32)
    } else {
        ((height as f64 * image_ratio).round() as i32, height)
    };
    let left = (width - draw_width) / 2;
    let top = (height - draw_height) / 2;
    RECT {
        left,
        top,
        right: left + draw_width,
        bottom: top + draw_height,
    }
}

fn centered_origin(owner: HWND, width: i32, height: i32) -> (i32, i32) {
    let mut owner_rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetWindowRect(owner, &mut owner_rect);
    }
    let monitor = unsafe {
        windows::Win32::Graphics::Gdi::MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST)
    };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::GetMonitorInfoW(monitor, &mut info);
    }
    let work = info.rcWork;
    let x = owner_rect.left + (owner_rect.right - owner_rect.left - width) / 2;
    let y = owner_rect.top + (owner_rect.bottom - owner_rect.top - height) / 2;
    (
        x.clamp(work.left, (work.right - width).max(work.left)),
        y.clamp(work.top, (work.bottom - height).max(work.top)),
    )
}

fn dress_window(hwnd: HWND) {
    let (background, foreground, dark) = super::windows_shell_frame_colors();
    let colorref = |rgb: u32| ((rgb & 0xff) << 16) | (rgb & 0xff00) | ((rgb >> 16) & 0xff);
    let background = colorref(background);
    let foreground = colorref(foreground);
    let dark = u32::from(dark);
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR,
            (&background as *const u32).cast(),
            std::mem::size_of_val(&background) as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_TEXT_COLOR,
            (&foreground as *const u32).cast(),
            std::mem::size_of_val(&foreground) as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&dark as *const u32).cast(),
            std::mem::size_of_val(&dark) as u32,
        );
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspect_fit_letterboxes_both_orientations() {
        assert_eq!(
            aspect_fit(1600, 900, 800, 800),
            RECT {
                left: 0,
                top: 175,
                right: 800,
                bottom: 625,
            }
        );
        assert_eq!(
            aspect_fit(900, 1600, 800, 800),
            RECT {
                left: 175,
                top: 0,
                right: 625,
                bottom: 800,
            }
        );
    }
}
