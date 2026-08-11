//! Windows desktop backend. All coordinates are global virtual-screen physical
//! pixels; the process is made per-monitor DPI aware on first use so reads are
//! not virtualized.

#[cfg(feature = "window")]
use crate::error::{Error, Result};
#[cfg(feature = "diagnostics")]
use crate::model::{Capabilities, Doctor, Permissions};
#[cfg(feature = "window")]
use crate::model::{Display, Rect, Window, WindowQuery};
#[cfg(any(feature = "diagnostics", feature = "window"))]
use std::sync::Once;
#[cfg(feature = "input")]
use windows::Win32::Foundation::POINT;
#[cfg(feature = "window")]
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, RECT, TRUE};
#[cfg(feature = "window")]
use windows::core::{BOOL, PWSTR};

#[cfg(feature = "ax")]
mod ax;
#[cfg(feature = "snapshot")]
mod capture;
#[cfg(feature = "clipboard")]
mod clipboard;
#[cfg(feature = "input")]
mod input;
#[cfg(feature = "supervision")]
mod pip;
#[cfg(feature = "process")]
mod process;
#[cfg(feature = "snapshot")]
mod wgc;
#[cfg(feature = "window")]
mod window_ops;
#[cfg(feature = "ax")]
pub use ax::{
    collapse as ax_collapse, expand as ax_expand, focus as ax_focus, hit_test as ax_hit_test,
    invoke as ax_invoke, query as ax_query, scroll_into_view as ax_scroll_into_view,
    select as ax_select, set_value as ax_set_value, tree as ax_tree, wait as ax_wait,
};
#[cfg(feature = "snapshot")]
pub use capture::{pixel, screenshot, wait_pixel};
#[cfg(feature = "clipboard")]
pub use clipboard::{
    clear as clipboard_clear, get as clipboard_get, paste as clipboard_paste, set as clipboard_set,
};
#[cfg(feature = "input")]
pub use input::{
    key_down, key_press, key_type, key_up, pointer_click, pointer_down, pointer_drag, pointer_move,
    pointer_scroll, pointer_up,
};
#[cfg(feature = "supervision")]
pub use pip::{dismiss as pip_dismiss, note_activity as pip_note_activity};
#[cfg(feature = "app")]
pub use process::{app_launch, app_quit};
#[cfg(feature = "process")]
pub use process::{process_kill, process_list};
#[cfg(feature = "window")]
pub use window_ops::{
    activate as window_activate, close as window_close, focus as window_focus,
    maximize as window_maximize, minimize as window_minimize, move_to as window_move,
    move_to_display as window_move_display, raise as window_raise, resize as window_resize,
    restore as window_restore, set_always_on_top as window_set_always_on_top,
    status as window_status,
};

/// Parse a "0x…"-style window id back into an `HWND`.
#[cfg(feature = "window")]
pub(crate) fn parse_hwnd(id: &str) -> Result<HWND> {
    let hex = id
        .strip_prefix("0x")
        .or_else(|| id.strip_prefix("0X"))
        .unwrap_or(id);
    let raw = isize::from_str_radix(hex, 16)
        .map_err(|_| Error::Usage(format!("invalid window id '{id}'")))?;
    let hwnd = HWND(raw as *mut core::ffi::c_void);
    ensure_automatable_hwnd(hwnd)?;
    Ok(hwnd)
}
#[cfg(feature = "window")]
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
#[cfg(feature = "window")]
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MONITORINFOEXW, MonitorFromRect,
};
#[cfg(feature = "window")]
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
#[cfg(any(feature = "diagnostics", feature = "window"))]
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
#[cfg(feature = "window")]
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
#[cfg(feature = "window")]
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GetClassNameW, GetForegroundWindow, GetWindowLongW, GetWindowRect,
    GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, IsZoomed, WS_EX_TOPMOST,
};
#[cfg(feature = "input")]
use windows::Win32::UI::WindowsAndMessaging::{GA_ROOT, GetAncestor, WindowFromPoint};

#[cfg(feature = "window")]
const MONITORINFOF_PRIMARY: u32 = 1;

#[cfg(feature = "window")]
fn direct_target_class_allowed(class: &str) -> bool {
    #[cfg(feature = "supervision")]
    {
        !pip::is_viewer_class(class)
    }
    #[cfg(not(feature = "supervision"))]
    {
        let _ = class;
        true
    }
}

#[cfg(feature = "window")]
pub(crate) fn is_viewer_hwnd(hwnd: HWND) -> bool {
    unsafe {
        let mut class = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut class);
        len > 0
            && !direct_target_class_allowed(&String::from_utf16_lossy(
                &class[..len.max(0) as usize],
            ))
    }
}

#[cfg(feature = "window")]
pub(crate) fn ensure_automatable_hwnd(hwnd: HWND) -> Result<()> {
    if is_viewer_hwnd(hwnd) {
        Err(Error::NotFound(
            "the activity viewer is not a desktop target".into(),
        ))
    } else {
        Ok(())
    }
}

/// Make the process per-monitor DPI aware once, so window/monitor rects come
/// back in true physical pixels instead of being virtualized.
#[cfg(any(feature = "diagnostics", feature = "window"))]
pub(crate) fn ensure_dpi_aware() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    });
}

#[cfg(feature = "window")]
pub(crate) fn rect_to(r: RECT) -> Rect {
    Rect {
        x: r.left,
        y: r.top,
        w: r.right - r.left,
        h: r.bottom - r.top,
    }
}

/// Windows needs no per-process TCC-style grants for these APIs, so all
/// permissions read as granted.
#[cfg(feature = "diagnostics")]
pub fn permissions() -> Permissions {
    Permissions::all_granted()
}

#[cfg(feature = "diagnostics")]
pub fn request_permissions() -> Permissions {
    Permissions::all_granted()
}

#[cfg(feature = "diagnostics")]
pub fn doctor() -> Doctor {
    ensure_dpi_aware();
    Doctor {
        backend: "windows".to_string(),
        os: "windows".to_string(),
        os_version: os_version(),
        permissions: permissions(),
        capabilities: Capabilities {
            displays: cfg!(feature = "window"),
            windows: cfg!(feature = "window"),
            screenshot: cfg!(feature = "snapshot"),
            window_screenshot_occlusion_independent: cfg!(feature = "snapshot"),
            pixel: cfg!(feature = "snapshot"),
            window_management: cfg!(feature = "window"),
            pointer: cfg!(feature = "input"),
            key: cfg!(feature = "input"),
            clipboard: cfg!(feature = "clipboard"),
            ax_tree: cfg!(feature = "ax"),
            ..Capabilities::default()
        },
    }
}

#[cfg(feature = "diagnostics")]
fn os_version() -> String {
    // Best-effort; avoids a version-shim dependency.
    std::env::var("OS").unwrap_or_default()
}

// ============================ displays ============================

#[cfg(feature = "window")]
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

#[cfg(feature = "window")]
fn monitor_dpi(hmon: HMONITOR) -> u32 {
    let mut dpi_x: u32 = 96;
    let mut dpi_y: u32 = 96;
    unsafe {
        let _ = GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
    }
    dpi_x
}

#[cfg(feature = "window")]
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

#[cfg(feature = "window")]
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

#[cfg(feature = "window")]
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

#[cfg(feature = "window")]
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

/// The top-level window that would receive pointer input at this physical
/// desktop point. Resolve it immediately before SendInput so owned popups and
/// overlapping topmost windows win over a caller's stale proposed window id.
#[cfg(feature = "input")]
pub(crate) fn input_window_at_point(x: i32, y: i32) -> Option<Window> {
    ensure_dpi_aware();
    unsafe {
        let hit = WindowFromPoint(POINT { x, y });
        if hit.0.is_null() {
            return None;
        }
        let root = GetAncestor(hit, GA_ROOT);
        if root.0.is_null() {
            return None;
        }
        if is_viewer_hwnd(root) {
            // The panel is WS_EX_TRANSPARENT and the real event passes through
            // it. Never let the viewer become its own capture target; falling
            // back to the acted-on display is safe if hit-testing reports it.
            return None;
        }
        window_ops::window_info(root).ok()
    }
}

/// Poll `windows()` until one matches (and, if given, matches `visible`), or
/// time out (exit 5).
#[cfg(feature = "window")]
pub fn wait_window(query: &WindowQuery, visible: Option<bool>, timeout_ms: u64) -> Result<Window> {
    // Enumeration only surfaces visible, uncloaked, non-zero-area top-level
    // windows, so every candidate is visible. `--state hidden` is therefore
    // unsatisfiable; reject it up front rather than spinning to a timeout.
    if visible == Some(false) {
        return Err(Error::Usage(
            "wait window --state hidden is unsupported: only visible windows are enumerated".into(),
        ));
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        let found = windows(query)?;
        if let Some(w) = found
            .into_iter()
            .find(|w| visible.is_none_or(|v| w.visible == v))
        {
            return Ok(w);
        }
        if std::time::Instant::now() >= deadline {
            return Err(Error::Timeout("timed out waiting for window".into()));
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
}

#[cfg(feature = "window")]
fn matches_query(r: &Raw, q: &WindowQuery) -> bool {
    if q.is_malformed() {
        return false;
    }
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

#[cfg(feature = "window")]
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
        if !direct_target_class_allowed(&class) {
            return TRUE;
        }

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

#[cfg(feature = "window")]
pub(crate) fn process_name(pid: u32) -> String {
    if pid == 0 {
        return String::new();
    }
    unsafe {
        let handle: HANDLE = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
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

#[cfg(all(test, feature = "supervision"))]
mod tests {
    use super::{direct_target_class_allowed, pip};

    #[test]
    fn direct_ids_can_never_target_the_viewer_class() {
        assert!(!direct_target_class_allowed(pip::VIEWER_CLASS));
        assert!(direct_target_class_allowed("OrdinaryProductWindow"));
        assert!(direct_target_class_allowed("LingXiaActivityViewerTarget"));
    }
}
