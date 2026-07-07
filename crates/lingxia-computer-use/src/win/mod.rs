//! Windows desktop backend. All coordinates are global virtual-screen physical
//! pixels; the process is made per-monitor DPI aware on first use so reads are
//! not virtualized.

use crate::error::{Error, Result};
use crate::model::{Capabilities, Display, Doctor, Rect, Window, WindowQuery};
use std::sync::Once;
use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, RECT, TRUE};

mod capture;
mod clipboard;
mod input;
mod window_ops;
pub use clipboard::{
    clear as clipboard_clear, get as clipboard_get, paste as clipboard_paste, set as clipboard_set,
};
pub use capture::{pixel, screenshot};
pub use input::{
    key_down, key_press, key_type, key_up, pointer_click, pointer_down, pointer_drag, pointer_move,
    pointer_scroll, pointer_up,
};
pub use window_ops::{
    activate as window_activate, close as window_close, focus as window_focus,
    maximize as window_maximize, minimize as window_minimize, move_to as window_move,
    move_to_display as window_move_display, raise as window_raise, resize as window_resize,
    restore as window_restore, set_always_on_top as window_set_always_on_top,
    status as window_status,
};

/// Parse a "0x…"-style window id back into an `HWND`.
pub(crate) fn parse_hwnd(id: &str) -> Result<HWND> {
    let hex = id
        .strip_prefix("0x")
        .or_else(|| id.strip_prefix("0X"))
        .unwrap_or(id);
    let raw = isize::from_str_radix(hex, 16)
        .map_err(|_| Error::Usage(format!("invalid window id '{id}'")))?;
    Ok(HWND(raw as *mut core::ffi::c_void))
}
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromRect, HDC, HMONITOR, MONITORINFO,
    MONITORINFOEXW, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::HiDpi::{
    GetDpiForMonitor, SetProcessDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, MDT_EFFECTIVE_DPI,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowLongW, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, IsZoomed, GWL_EXSTYLE, WS_EX_TOPMOST,
};

const MONITORINFOF_PRIMARY: u32 = 1;

/// Make the process per-monitor DPI aware once, so window/monitor rects come
/// back in true physical pixels instead of being virtualized.
pub(crate) fn ensure_dpi_aware() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    });
}

pub(crate) fn rect_to(r: RECT) -> Rect {
    Rect {
        x: r.left,
        y: r.top,
        w: r.right - r.left,
        h: r.bottom - r.top,
    }
}

pub fn doctor() -> Doctor {
    ensure_dpi_aware();
    Doctor {
        backend: "windows".to_string(),
        os: "windows".to_string(),
        os_version: os_version(),
        capabilities: Capabilities {
            displays: true,
            windows: true,
            screenshot: true,
            window_screenshot_occlusion_independent: true,
            pixel: true,
            window_management: true,
            pointer: true,
            key: true,
            clipboard: true,
            ..Capabilities::default()
        },
    }
}

fn os_version() -> String {
    // Best-effort; avoids a version-shim dependency.
    std::env::var("OS").unwrap_or_default()
}

// ============================ displays ============================

pub fn displays() -> Result<Vec<Display>> {
    ensure_dpi_aware();
    let mut out: Vec<Display> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(monitor_enum_proc),
            LPARAM(&mut out as *mut _ as isize),
        );
    }
    Ok(out)
}

fn monitor_dpi(hmon: HMONITOR) -> u32 {
    let mut dpi_x: u32 = 96;
    let mut dpi_y: u32 = 96;
    unsafe {
        let _ = GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
    }
    dpi_x
}

unsafe extern "system" fn monitor_enum_proc(
    hmon: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    unsafe {
        let out = &mut *(lparam.0 as *mut Vec<Display>);
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        let ok = GetMonitorInfoW(hmon, &mut mi.monitorInfo as *mut MONITORINFO);
        if ok.as_bool() {
            let dpi = monitor_dpi(hmon);
            let primary = mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0;
            out.push(Display {
                id: format!("display-{}", out.len() + 1),
                primary,
                bounds: rect_to(mi.monitorInfo.rcMonitor),
                work_area: rect_to(mi.monitorInfo.rcWork),
                scale: dpi as f64 / 96.0,
                dpi,
            });
        }
    }
    TRUE
}

pub(crate) fn display_id_for_rect(displays: &[Display], r: &RECT) -> (String, u32, f64) {
    let hmon = unsafe { MonitorFromRect(r, MONITOR_DEFAULTTONEAREST) };
    let dpi = monitor_dpi(hmon);
    // Match by containment of the window's top-left against known bounds.
    for d in displays {
        if r.left >= d.bounds.x
            && r.top >= d.bounds.y
            && r.left < d.bounds.x + d.bounds.w
            && r.top < d.bounds.y + d.bounds.h
        {
            return (d.id.clone(), d.dpi, d.scale);
        }
    }
    (
        displays.first().map(|d| d.id.clone()).unwrap_or_default(),
        dpi,
        dpi as f64 / 96.0,
    )
}

// ============================ windows ============================

struct Raw {
    hwnd: HWND,
    title: String,
    class: String,
    pid: u32,
    rect: RECT,
    minimized: bool,
    maximized: bool,
    topmost: bool,
}

pub fn windows(query: &WindowQuery) -> Result<Vec<Window>> {
    ensure_dpi_aware();
    let mut raw: Vec<Raw> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(window_enum_proc), LPARAM(&mut raw as *mut _ as isize));
    }
    let displays = displays().unwrap_or_default();
    let foreground = unsafe { GetForegroundWindow() };

    let mut out = Vec::new();
    for (z, r) in raw.into_iter().enumerate() {
        if !matches_query(&r, query) {
            continue;
        }
        let (display_id, dpi, scale) = display_id_for_rect(&displays, &r.rect);
        out.push(Window {
            id: format!("0x{:X}", r.hwnd.0 as isize),
            title: r.title,
            process: process_name(r.pid),
            pid: r.pid,
            bounds: rect_to(r.rect),
            display_id,
            scale,
            dpi,
            visible: true,
            focused: r.hwnd == foreground,
            minimized: r.minimized,
            maximized: r.maximized,
            always_on_top: r.topmost,
            z: z as u32,
        });
    }
    Ok(out)
}

fn matches_query(r: &Raw, q: &WindowQuery) -> bool {
    if q.is_empty() {
        return true;
    }
    if let Some(pid) = q.pid {
        return r.pid == pid;
    }
    let hay_ci = |needle: &str, hay: &str| hay.to_lowercase().contains(&needle.to_lowercase());
    if let Some(t) = &q.title {
        return hay_ci(t, &r.title);
    }
    if let Some(c) = &q.class {
        return hay_ci(c, &r.class);
    }
    if let Some(p) = &q.process {
        return hay_ci(p, &process_name(r.pid));
    }
    if let Some(text) = &q.text {
        return hay_ci(text, &r.title)
            || hay_ci(text, &r.class)
            || hay_ci(text, &process_name(r.pid));
    }
    true
}

unsafe extern "system" fn window_enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let out = &mut *(lparam.0 as *mut Vec<Raw>);

        if !IsWindowVisible(hwnd).as_bool() {
            return TRUE;
        }
        // Skip DWM-cloaked windows (UWP ghosts, virtual-desktop hidden).
        let mut cloaked: u32 = 0;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as *mut core::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
        if cloaked != 0 {
            return TRUE;
        }

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return TRUE;
        }
        // Skip zero-area windows.
        if rect.right - rect.left <= 0 || rect.bottom - rect.top <= 0 {
            return TRUE;
        }

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut title_buf);
        let title = String::from_utf16_lossy(&title_buf[..len.max(0) as usize]);

        let mut class_buf = [0u16; 256];
        let clen = windows::Win32::UI::WindowsAndMessaging::GetClassNameW(hwnd, &mut class_buf);
        let class = String::from_utf16_lossy(&class_buf[..clen.max(0) as usize]);

        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let topmost = ex & WS_EX_TOPMOST.0 != 0;

        out.push(Raw {
            hwnd,
            title,
            class,
            pid,
            rect,
            minimized: IsIconic(hwnd).as_bool(),
            maximized: IsZoomed(hwnd).as_bool(),
            topmost,
        });
    }
    TRUE
}

pub(crate) fn process_name(pid: u32) -> String {
    if pid == 0 {
        return String::new();
    }
    unsafe {
        let handle: HANDLE =
            match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                Ok(h) => h,
                Err(_) => return String::new(),
            };
        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        if ok.is_err() {
            return String::new();
        }
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        full.rsplit(['\\', '/'])
            .next()
            .unwrap_or(&full)
            .trim_end_matches(".exe")
            .to_string()
    }
}
