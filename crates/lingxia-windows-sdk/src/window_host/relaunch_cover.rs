//! A reLaunch swaps the whole host window: the old one leaves before the new one has its
//! first frame or, on a simulated device, its frame. The screen as last shown stays over the
//! swap until then.

use std::time::{Duration, Instant};

use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, BitBlt,
    CAPTUREBLT, ClientToScreen, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDC, ReleaseDC, SRCCOPY, SelectObject,
};
use windows::Win32::System::LibraryLoader;
use windows::Win32::UI::WindowsAndMessaging::{
    self, GetWindowThreadProcessId, MSG, PM_REMOVE, WNDCLASSW, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use super::{
    NAV_SLIDE_HOLD, active_host_window, hwnd_handle, is_window_visible, window_is_device_framed,
};

/// Long enough for the swap to begin before the cover can lift.
const MIN_HOLD: Duration = Duration::from_millis(60);
/// A page that never presents must not freeze the screen.
const MAX_HOLD: Duration = Duration::from_millis(2000);

/// Covers the active host, with its device frame, with a capture of it, then lifts the cover once the
/// host that replaced it is shown, framed when the old one was, and its GPU page has a frame.
pub(crate) fn cover_relaunch() {
    let Some(host) = active_host_window() else {
        return;
    };
    if !is_window_visible(host) || unsafe { WindowsAndMessaging::IsIconic(host) }.as_bool() {
        return;
    }
    // A simulated device's bezel leaves with the old host too.
    let by_frame = frame_area(host).is_some();
    let Some((origin, width, height)) = frame_area(host).or_else(|| client_area(host)) else {
        return;
    };
    let clear = occluders(
        host,
        RECT {
            left: origin.x,
            top: origin.y,
            right: origin.x + width,
            bottom: origin.y + height,
        },
    );
    let framed = window_is_device_framed(host);
    let old = hwnd_handle(host);
    let (shown_tx, shown_rx) = std::sync::mpsc::channel();
    // Its own pumping thread: a window dies with the thread that made it, and the old page's
    // thread may end before the new page is ready.
    let spawned = std::thread::Builder::new()
        .name("lingxia-relaunch-cover".into())
        .spawn(move || {
            let overlay = mount(origin, width, height, &clear);
            let _ = shown_tx.send(overlay.is_some());
            let Some(overlay) = overlay else {
                return;
            };
            let started = Instant::now();
            while started.elapsed() < MAX_HOLD {
                let mut msg = MSG::default();
                unsafe {
                    while WindowsAndMessaging::PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE)
                        .as_bool()
                    {
                        let _ = WindowsAndMessaging::TranslateMessage(&msg);
                        WindowsAndMessaging::DispatchMessageW(&msg);
                    }
                }
                std::thread::sleep(Duration::from_millis(8));
                // A window moved under the cover: the capture no longer matches the screen.
                if moved(origin, by_frame)
                    || (started.elapsed() >= MIN_HOLD && replaced(old, framed))
                {
                    break;
                }
            }
            unsafe {
                let _ = WindowsAndMessaging::DestroyWindow(overlay);
            }
        });
    if spawned.is_ok() {
        // The swap must not begin before the cover is on screen.
        let _ = shown_rx.recv_timeout(Duration::from_millis(500));
    }
}

fn client_area(host: HWND) -> Option<(POINT, i32, i32)> {
    let mut client = RECT::default();
    let mut origin = POINT { x: 0, y: 0 };
    unsafe {
        WindowsAndMessaging::GetClientRect(host, &mut client).ok()?;
        ClientToScreen(host, &mut origin).ok().ok()?;
    }
    let (width, height) = (client.right - client.left, client.bottom - client.top);
    (width > 0 && height > 0).then_some((origin, width, height))
}

fn frame_area(host: HWND) -> Option<(POINT, i32, i32)> {
    #[cfg(feature = "device-frame")]
    {
        let frame = crate::device_frame::device_frame_window(hwnd_handle(host))?;
        let mut rect = RECT::default();
        unsafe {
            WindowsAndMessaging::GetWindowRect(super::hwnd_from_handle(frame), &mut rect).ok()?;
        }
        let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
        (width > 0 && height > 0).then_some((
            POINT {
                x: rect.left,
                y: rect.top,
            },
            width,
            height,
        ))
    }
    #[cfg(not(feature = "device-frame"))]
    {
        let _ = host;
        None
    }
}

fn moved(origin: POINT, by_frame: bool) -> bool {
    active_host_window()
        .filter(|host| is_window_visible(*host))
        .and_then(|host| {
            if by_frame {
                frame_area(host)
            } else {
                client_area(host)
            }
        })
        .is_some_and(|(now, _, _)| now.x != origin.x || now.y != origin.y)
}

fn frame_shown(host: HWND) -> bool {
    #[cfg(feature = "device-frame")]
    {
        crate::device_frame::device_frame_shown(hwnd_handle(host))
    }
    #[cfg(not(feature = "device-frame"))]
    {
        window_is_device_framed(host)
    }
}

/// Other apps' windows above `host` over `area`, in screen coordinates: the cover leaves them clear, or it
/// would freeze their pixels.
fn occluders(host: HWND, area: RECT) -> Vec<RECT> {
    let mut found = Vec::new();
    let ours = std::process::id();
    let mut window = unsafe { WindowsAndMessaging::GetTopWindow(None) }.ok();
    while let Some(current) = window.filter(|current| *current != host) {
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(current, Some(&mut pid)) };
        let mut rect = RECT::default();
        if pid != ours
            && is_window_visible(current)
            && !cloaked(current)
            && unsafe { WindowsAndMessaging::GetWindowRect(current, &mut rect) }.is_ok()
            && rect.left < area.right
            && area.left < rect.right
            && rect.top < area.bottom
            && area.top < rect.bottom
        {
            found.push(rect);
        }
        window =
            unsafe { WindowsAndMessaging::GetWindow(current, WindowsAndMessaging::GW_HWNDNEXT) }
                .ok();
    }
    found
}

fn cloaked(window: HWND) -> bool {
    let mut cloaked = 0u32;
    unsafe {
        windows::Win32::Graphics::Dwm::DwmGetWindowAttribute(
            window,
            windows::Win32::Graphics::Dwm::DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        )
    }
    .is_ok()
        && cloaked != 0
}

fn replaced(old: isize, framed: bool) -> bool {
    let Some(host) = active_host_window() else {
        return false;
    };
    hwnd_handle(host) != old
        && is_window_visible(host)
        && (!framed || frame_shown(host))
        && !NAV_SLIDE_HOLD.get().is_some_and(|hold| hold())
}

/// A topmost, click-through layered window showing what the screen shows at `origin`.
fn mount(origin: POINT, width: i32, height: i32, clear: &[RECT]) -> Option<HWND> {
    static CLASS: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    CLASS.get_or_init(|| {
        let class = WNDCLASSW {
            lpfnWndProc: Some(cover_proc),
            lpszClassName: w!("LingXiaRelaunchCover"),
            ..Default::default()
        };
        unsafe {
            WindowsAndMessaging::RegisterClassW(&class);
        }
    });
    let overlay = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TRANSPARENT
                | WindowsAndMessaging::WS_EX_NOACTIVATE
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_TOPMOST,
            w!("LingXiaRelaunchCover"),
            PCWSTR::null(),
            WS_POPUP,
            origin.x,
            origin.y,
            width,
            height,
            None,
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    }
    .ok()?;
    if !paint(overlay, origin, width, height, clear) {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(overlay);
        }
        return None;
    }
    unsafe {
        let _ = WindowsAndMessaging::ShowWindow(overlay, WindowsAndMessaging::SW_SHOWNOACTIVATE);
        let _ = windows::Win32::Graphics::Dwm::DwmFlush();
    }
    Some(overlay)
}

unsafe extern "system" fn cover_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn paint(overlay: HWND, origin: POINT, width: i32, height: i32, clear: &[RECT]) -> bool {
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
    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let mut bits = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(dc);
            ReleaseDC(None, screen);
            return false;
        };
        let old = SelectObject(dc, bitmap.into());
        // CAPTUREBLT takes the composed screen, GPU and layered windows included.
        let captured = BitBlt(
            dc,
            0,
            0,
            width,
            height,
            Some(screen),
            origin.x,
            origin.y,
            SRCCOPY | CAPTUREBLT,
        )
        .is_ok();
        ReleaseDC(None, screen);
        let _ = windows::Win32::Graphics::Gdi::GdiFlush();
        // SAFETY: the DIB holds `width × height` pixels and lives until the end of this block.
        let pixels = std::slice::from_raw_parts_mut(bits as *mut u32, (width * height) as usize);
        // A capture carries no alpha: opaque, then clear (premultiplied zero) where other
        // apps lie over the area.
        for pixel in pixels.iter_mut() {
            *pixel |= 0xff00_0000;
        }
        for rect in clear {
            let left = (rect.left - origin.x).clamp(0, width);
            let right = (rect.right - origin.x).clamp(0, width);
            let top = (rect.top - origin.y).clamp(0, height);
            let bottom = (rect.bottom - origin.y).clamp(0, height);
            for y in top..bottom {
                let row = (y * width) as usize;
                pixels[row + left as usize..row + right as usize].fill(0);
            }
        }
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let source = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let shown = captured
            && WindowsAndMessaging::UpdateLayeredWindow(
                overlay,
                None,
                Some(&origin),
                Some(&size),
                Some(dc),
                Some(&source),
                COLORREF(0),
                Some(&blend),
                WindowsAndMessaging::ULW_ALPHA,
            )
            .is_ok();
        SelectObject(dc, old);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(dc);
        shown
    }
}
