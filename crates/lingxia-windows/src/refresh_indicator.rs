//! Pull-to-refresh indicator: a small rounded pill with three pulsing dots,
//! shown at the top-center of the current page while a refresh is in flight.
//! This is the Windows counterpart of the macOS `MacPullToRefreshHelper`.
//!
//! This is host UI, so it lives in the Windows host SDK, not in
//! `lingxia-webview`. The webview only reports where its content is on screen
//! (via `active_content_screen_rect`); this module owns the overlay window and
//! its GDI animation, and plugs into the platform through the
//! [`set_windows_refresh_indicator_handler`] seam.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::webview_host::post_to_window_thread;
use crate::webview_host::{WindowsContentRect, active_content_screen_rect};
use lingxia_platform::set_windows_refresh_indicator_handler;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateRoundRectRgn, CreateSolidBrush, DeleteObject, Ellipse, EndPaint, FillRect,
    HGDIOBJ, InvalidateRect, PAINTSTRUCT, SelectObject, SetWindowRgn,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging;
use windows::core::{PCWSTR, w};

/// Registers the refresh-indicator show/hide callback with the platform. Called
/// once at host startup.
pub(crate) fn install() {
    set_windows_refresh_indicator_handler(Arc::new(set_refresh));
}

/// Logical pill size and dot geometry (scaled by the host DPI).
const PILL_WIDTH: i32 = 58;
const PILL_HEIGHT: i32 = 24;
const DOT_RADIUS: i32 = 3;
const DOT_SPACING: i32 = 11;
const CORNER_RADIUS: i32 = 12;
const TOP_MARGIN: i32 = 12;

const TIMER_ID: usize = 0x5252; // 'RR'
const TIMER_INTERVAL_MS: u32 = 80;
const FRAME_COUNT: u32 = 24;

const CLASS_NAME: PCWSTR = w!("LingXiaRefreshIndicator");

struct IndicatorWindow {
    hwnd: isize,
    host: isize,
}

static INDICATOR: Mutex<Option<IndicatorWindow>> = Mutex::new(None);
/// Desired visibility, toggled on the caller's thread; the UI-thread create
/// step honors it so a hide racing ahead of a queued show wins.
static ACTIVE: AtomicBool = AtomicBool::new(false);
static CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);
static FRAME: AtomicU32 = AtomicU32::new(0);

fn hwnd_from_handle(handle: isize) -> HWND {
    HWND(handle as *mut c_void)
}

fn hwnd_handle(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

/// Shows or hides the indicator. Safe to call from any thread; the window work
/// is marshalled onto the host window's UI thread.
fn set_refresh(show: bool) {
    ACTIVE.store(show, Ordering::SeqCst);
    if show {
        let Some(rect) = active_content_screen_rect() else {
            return;
        };
        post_to_window_thread(rect.host_window, Box::new(move || ui_show(rect)));
    } else {
        let host = INDICATOR
            .lock()
            .ok()
            .and_then(|slot| slot.as_ref().map(|indicator| indicator.host));
        if let Some(host) = host {
            post_to_window_thread(host, Box::new(ui_hide));
        }
    }
}

fn ui_show(rect: WindowsContentRect) {
    if !ACTIVE.load(Ordering::SeqCst) {
        return;
    }
    let host = hwnd_from_handle(rect.host_window);
    let hwnd = {
        let mut slot = match INDICATOR.lock() {
            Ok(slot) => slot,
            Err(_) => return,
        };
        match slot.as_ref() {
            Some(indicator)
                if unsafe {
                    WindowsAndMessaging::IsWindow(Some(hwnd_from_handle(indicator.hwnd)))
                }
                .as_bool() =>
            {
                hwnd_from_handle(indicator.hwnd)
            }
            _ => {
                let Some(hwnd) = create_indicator(host, rect.dpi) else {
                    return;
                };
                unsafe {
                    WindowsAndMessaging::SetTimer(Some(hwnd), TIMER_ID, TIMER_INTERVAL_MS, None);
                }
                *slot = Some(IndicatorWindow {
                    hwnd: hwnd_handle(hwnd),
                    host: rect.host_window,
                });
                hwnd
            }
        }
    };
    position_indicator(hwnd, &rect);
}

fn ui_hide() {
    let indicator = INDICATOR.lock().ok().and_then(|mut slot| slot.take());
    if let Some(indicator) = indicator {
        let hwnd = hwnd_from_handle(indicator.hwnd);
        unsafe {
            let _ = WindowsAndMessaging::KillTimer(Some(hwnd), TIMER_ID);
            let _ = WindowsAndMessaging::DestroyWindow(hwnd);
        }
    }
}

fn pill_size(dpi: u32) -> (i32, i32, f64) {
    let scale = if dpi == 0 { 1.0 } else { dpi as f64 / 96.0 };
    let width = (PILL_WIDTH as f64 * scale).round() as i32;
    let height = (PILL_HEIGHT as f64 * scale).round() as i32;
    (width, height, scale)
}

fn create_indicator(host: HWND, dpi: u32) -> Option<HWND> {
    ensure_class();
    let (width, height, scale) = pill_size(dpi);
    let instance = unsafe { GetModuleHandleW(None) }
        .ok()
        .map(|module| HINSTANCE(module.0))?;
    // A top-level popup *owned* by the host (not a WS_CHILD): an owned window
    // always renders above its owner's child windows, so the pill sits over the
    // WebView2 content, which a sibling child window cannot reliably do.
    let hwnd = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_TOOLWINDOW | WindowsAndMessaging::WS_EX_NOACTIVATE,
            CLASS_NAME,
            w!(""),
            WindowsAndMessaging::WS_POPUP,
            0,
            0,
            width,
            height,
            Some(host),
            None,
            Some(instance),
            None,
        )
    }
    .ok()?;
    // Clip to a rounded pill so the corners reveal the content behind it.
    let diameter = ((CORNER_RADIUS as f64 * scale).round() as i32 * 2).max(2);
    unsafe {
        let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, diameter, diameter);
        let _ = SetWindowRgn(hwnd, Some(region), true);
    }
    Some(hwnd)
}

fn position_indicator(hwnd: HWND, rect: &WindowsContentRect) {
    let (width, height, scale) = pill_size(rect.dpi);
    let area_width = rect.width.max(width);
    let left = rect.left + (area_width - width) / 2;
    let top = rect.top + (TOP_MARGIN as f64 * scale).round() as i32;
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd,
            Some(WindowsAndMessaging::HWND_TOP),
            left,
            top,
            width,
            height,
            WindowsAndMessaging::SWP_SHOWWINDOW | WindowsAndMessaging::SWP_NOACTIVATE,
        );
    }
}

fn ensure_class() {
    if CLASS_REGISTERED.swap(true, Ordering::SeqCst) {
        return;
    }
    let instance = unsafe { GetModuleHandleW(None) }
        .ok()
        .map(|module| HINSTANCE(module.0))
        .unwrap_or_default();
    let class = WindowsAndMessaging::WNDCLASSW {
        style: WindowsAndMessaging::CS_HREDRAW | WindowsAndMessaging::CS_VREDRAW,
        lpfnWndProc: Some(indicator_proc),
        hInstance: instance,
        hCursor: unsafe { WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_ARROW) }
            .unwrap_or_default(),
        lpszClassName: CLASS_NAME,
        ..Default::default()
    };
    unsafe {
        WindowsAndMessaging::RegisterClassW(&class);
    }
}

unsafe extern "system" fn indicator_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // A panic must never cross the C ABI boundary (it aborts the process), so
    // contain it and fall back to default handling.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        indicator_proc_inner(hwnd, msg, wparam, lparam)
    })) {
        Ok(result) => result,
        Err(_) => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn indicator_proc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WindowsAndMessaging::WM_TIMER && wparam.0 == TIMER_ID {
        FRAME.store(
            (FRAME.load(Ordering::Relaxed) + 1) % FRAME_COUNT,
            Ordering::Relaxed,
        );
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
        return LRESULT(0);
    }
    if msg == WindowsAndMessaging::WM_ERASEBKGND {
        return LRESULT(1);
    }
    if msg == WindowsAndMessaging::WM_PAINT {
        paint_indicator(hwnd);
        return LRESULT(0);
    }
    unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn paint_indicator(hwnd: HWND) {
    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    let mut paint = PAINTSTRUCT::default();
    let frame = FRAME.load(Ordering::Relaxed);
    unsafe {
        let hdc = BeginPaint(hwnd, &mut paint);
        if hdc.is_invalid() {
            return;
        }
        // Pill background (rounded by the window region).
        let bg = CreateSolidBrush(COLORREF(0x00FAF8F6));
        FillRect(hdc, &client, bg);
        let _ = DeleteObject(HGDIOBJ(bg.0));

        let width = client.right - client.left;
        let height = client.bottom - client.top;
        let dpi = GetDpiForWindow(hwnd);
        let scale = if dpi == 0 { 1.0 } else { dpi as f64 / 96.0 };
        let radius = (DOT_RADIUS as f64 * scale).round() as i32;
        let spacing = (DOT_SPACING as f64 * scale).round() as i32;
        let center_y = height / 2;
        let center_x = width / 2;
        for i in 0..3 {
            let brightness = dot_brightness(frame, i);
            let lerp = |light: i32, dark: i32| -> i32 {
                (light as f64 + (dark - light) as f64 * brightness).round() as i32
            };
            let r = lerp(205, 90).clamp(0, 255) as u32;
            let g = lerp(205, 90).clamp(0, 255) as u32;
            let b = lerp(212, 104).clamp(0, 255) as u32;
            let brush = CreateSolidBrush(COLORREF((b << 16) | (g << 8) | r));
            let previous = SelectObject(hdc, HGDIOBJ(brush.0));
            let cx = center_x + (i - 1) * spacing;
            let _ = Ellipse(
                hdc,
                cx - radius,
                center_y - radius,
                cx + radius,
                center_y + radius,
            );
            SelectObject(hdc, previous);
            let _ = DeleteObject(HGDIOBJ(brush.0));
        }
        let _ = EndPaint(hwnd, &paint);
    }
}

/// Triangle-wave brightness in `0.0..=1.0` for dot `i`, phase-shifted so the
/// three dots ripple.
fn dot_brightness(frame: u32, i: i32) -> f64 {
    let phase = (frame + (i as u32) * (FRAME_COUNT / 3)) % FRAME_COUNT;
    let t = phase as f64 / FRAME_COUNT as f64;
    let triangle = 1.0 - (t * 2.0 - 1.0).abs();
    0.3 + 0.7 * triangle
}
