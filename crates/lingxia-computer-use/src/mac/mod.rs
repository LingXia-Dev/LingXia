//! macOS desktop backend.
//!
//! Coordinates are **global display points with a top-left origin** — the
//! native space of `CGWindowList`, `CGEvent`, and the Accessibility API, so
//! window bounds, pointer input, and AX rects are all in the same units. This
//! differs from the Windows backend, which is in physical pixels; on a Retina
//! display one point is `scale` pixels, and screenshots therefore come back
//! `scale`× larger than the point bounds they cover. Each [`Display`] and
//! [`Window`] carries its `scale`/`dpi` so a caller can convert when needed.

use crate::error::{Error, Result};
use crate::model::{Capabilities, Display, Doctor, Permissions, Rect, Window, WindowQuery};
use objc2_core_foundation::CGRect;
use objc2_core_graphics::{
    CGDirectDisplayID, CGDisplayBounds, CGDisplayCopyDisplayMode, CGDisplayMode,
    CGGetActiveDisplayList, CGMainDisplayID, CGPreflightPostEventAccess,
    CGPreflightScreenCaptureAccess, CGRequestPostEventAccess, CGRequestScreenCaptureAccess,
    CGWindowListCopyWindowInfo, CGWindowListOption, kCGWindowAlpha, kCGWindowBounds,
    kCGWindowIsOnscreen, kCGWindowLayer, kCGWindowName, kCGWindowNumber, kCGWindowOwnerName,
    kCGWindowOwnerPID,
};

mod ax;
mod axui;
mod capture;
mod cf;
mod clipboard;
mod input;
mod keymap;
mod process;
mod window_ops;

pub use ax::{
    collapse as ax_collapse, expand as ax_expand, focus as ax_focus, hit_test as ax_hit_test,
    invoke as ax_invoke, query as ax_query, scroll_into_view as ax_scroll_into_view,
    select as ax_select, set_value as ax_set_value, tree as ax_tree, wait as ax_wait,
};
pub use capture::{pixel, screenshot, wait_pixel};
pub use clipboard::{
    clear as clipboard_clear, get as clipboard_get, paste as clipboard_paste, set as clipboard_set,
};
pub use input::{
    key_down, key_press, key_type, key_up, pointer_click, pointer_down, pointer_drag, pointer_move,
    pointer_scroll, pointer_up,
};
pub use process::{app_launch, app_quit, process_kill, process_list};
pub use window_ops::{
    activate as window_activate, close as window_close, focus as window_focus,
    maximize as window_maximize, minimize as window_minimize, move_to as window_move,
    move_to_display as window_move_display, raise as window_raise, resize as window_resize,
    restore as window_restore, set_always_on_top as window_set_always_on_top,
    status as window_status,
};

/// Convert a `CGRect` (points) to the model's integer `Rect`.
pub(crate) fn rect_to(r: CGRect) -> Rect {
    Rect {
        x: r.origin.x.round() as i32,
        y: r.origin.y.round() as i32,
        w: r.size.width.round() as i32,
        h: r.size.height.round() as i32,
    }
}

/// Parse a window id (decimal, as emitted by `windows()`, or `0x…` hex) back
/// into a `CGWindowID`.
pub(crate) fn parse_window_id(id: &str) -> Result<u32> {
    let parsed = if let Some(hex) = id.strip_prefix("0x").or_else(|| id.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16)
    } else {
        id.parse::<u32>()
    };
    parsed.map_err(|_| Error::Usage(format!("invalid window id '{id}'")))
}

/// Best-effort macOS product version (e.g. "14.5") via sysctl.
fn os_version() -> String {
    let mut buf = [0u8; 64];
    let mut len = buf.len();
    let name = c"kern.osproductversion";
    let rc = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            buf.as_mut_ptr() as *mut libc::c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 || len == 0 {
        return String::new();
    }
    String::from_utf8_lossy(&buf[..len.saturating_sub(1)]).into_owned()
}

/// Live permission grants for this process (no prompt).
pub fn permissions() -> Permissions {
    Permissions {
        accessibility: axui::is_trusted(),
        screen_recording: CGPreflightScreenCaptureAccess(),
        input: CGPreflightPostEventAccess(),
    }
}

/// Prompt for any permission not yet granted, then re-report. macOS shows the
/// system dialog / adds the app to the relevant list; the user still approves.
pub fn request_permissions() -> Permissions {
    if !CGPreflightScreenCaptureAccess() {
        let _ = CGRequestScreenCaptureAccess();
    }
    if !CGPreflightPostEventAccess() {
        let _ = CGRequestPostEventAccess();
    }
    if !axui::is_trusted() {
        let _ = axui::prompt_trusted();
    }
    permissions()
}

pub fn doctor() -> Doctor {
    Doctor {
        backend: "macos".to_string(),
        os: "macos".to_string(),
        os_version: os_version(),
        permissions: permissions(),
        capabilities: Capabilities {
            displays: true,
            windows: true,
            screenshot: true,
            // CGWindowListCreateImage composites the target window's own
            // backing store, so occluded regions still come through.
            window_screenshot_occlusion_independent: true,
            pixel: true,
            pointer: true,
            key: true,
            window_management: true,
            clipboard: true,
            ax_tree: true,
            ..Capabilities::default()
        },
    }
}

// ============================ displays ============================

pub fn displays() -> Result<Vec<Display>> {
    let mut ids = [0 as CGDirectDisplayID; 16];
    let mut count: u32 = 0;
    let err = unsafe { CGGetActiveDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut count) };
    if err.0 != 0 {
        return Err(Error::Unavailable(format!(
            "CGGetActiveDisplayList failed ({})",
            err.0
        )));
    }
    let main = CGMainDisplayID();
    let mut out = Vec::with_capacity(count as usize);
    for (i, &id) in ids.iter().take(count as usize).enumerate() {
        let bounds = CGDisplayBounds(id);
        // Backing scale = the current mode's pixel width over its point width.
        // `CGDisplayPixelsWide` reports the *scaled* (point) width on HiDPI modes,
        // so it can't distinguish a 1× from a Retina 2× display; the mode can.
        let scale = display_scale(id);
        out.push(Display {
            id: format!("display-{}", i + 1),
            primary: id == main,
            bounds: rect_to(bounds),
            // macOS does not expose a per-display work area through Quartz; the
            // menu bar / Dock insets would require NSScreen.visibleFrame. Report
            // the full bounds and let callers inset if they care.
            work_area: rect_to(bounds),
            scale,
            dpi: (72.0 * scale).round() as u32,
        });
    }
    Ok(out)
}

/// A display's backing scale factor (2.0 on Retina), from its current mode.
fn display_scale(id: CGDirectDisplayID) -> f64 {
    match CGDisplayCopyDisplayMode(id) {
        Some(mode) => {
            let points = CGDisplayMode::width(Some(&mode)) as f64;
            let pixels = CGDisplayMode::pixel_width(Some(&mode)) as f64;
            if points > 0.0 {
                (pixels / points).max(1.0)
            } else {
                1.0
            }
        }
        None => 1.0,
    }
}

/// The display whose bounds contain a rect's top-left, else the first display.
pub(crate) fn display_for_rect(displays: &[Display], r: &Rect) -> (String, u32, f64) {
    for d in displays {
        if r.x >= d.bounds.x
            && r.y >= d.bounds.y
            && r.x < d.bounds.x + d.bounds.w
            && r.y < d.bounds.y + d.bounds.h
        {
            return (d.id.clone(), d.dpi, d.scale);
        }
    }
    displays
        .first()
        .map(|d| (d.id.clone(), d.dpi, d.scale))
        .unwrap_or_else(|| (String::new(), 72, 1.0))
}

// ============================ windows ============================

/// The pid of the frontmost GUI application, for `focused` reporting and for
/// directing keyboard input at the active app.
pub(crate) fn frontmost_pid() -> Option<i32> {
    use objc2_app_kit::NSWorkspace;
    NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .map(|app| app.processIdentifier())
}

/// List on-screen OS windows (the public `desktop windows` surface).
pub fn windows(query: &WindowQuery) -> Result<Vec<Window>> {
    enumerate(query, true)
}

/// Locate a single window by `CGWindowID`, including minimized/off-screen ones
/// (`OptionAll`). Used by window operations, which must still reach a window a
/// user has minimized — those drop out of the on-screen list.
pub(crate) fn window_record(wid: u32) -> Option<Window> {
    enumerate(&WindowQuery::default(), false)
        .ok()?
        .into_iter()
        .find(|w| w.id == wid.to_string())
}

/// Enumerate windows. `only_onscreen` picks `OptionOnScreenOnly` (visible
/// windows, front-to-back) vs `OptionAll` (also minimized/off-screen).
fn enumerate(query: &WindowQuery, only_onscreen: bool) -> Result<Vec<Window>> {
    if query.is_malformed() {
        return Ok(Vec::new());
    }
    let option = if only_onscreen {
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements
    } else {
        CGWindowListOption::OptionAll | CGWindowListOption::ExcludeDesktopElements
    };
    let Some(info) = CGWindowListCopyWindowInfo(option, 0) else {
        return Err(Error::Unavailable("CGWindowListCopyWindowInfo failed".into()));
    };
    let array = (&*info as *const objc2_core_foundation::CFArray).cast::<std::ffi::c_void>();
    let displays = displays().unwrap_or_default();
    let front = frontmost_pid();

    let mut out = Vec::new();
    let mut focused_taken = false;
    unsafe {
        let n = cf::array_count(array);
        for z in 0..n {
            let dict = cf::array_get(array, z);
            if dict.is_null() {
                continue;
            }
            // Layer 0 is the normal window layer; skip the desktop (<0). Higher
            // layers (menu bar, Dock, status items) are kept but flagged
            // always_on_top, matching the Windows topmost notion.
            let layer = cf::dict_i64(dict, kCGWindowLayer).unwrap_or(0);
            if layer < 0 {
                continue;
            }
            // Skip fully transparent helper windows.
            if cf::dict_f64(dict, kCGWindowAlpha).is_some_and(|a| a <= 0.0) {
                continue;
            }
            let Some(number) = cf::dict_i64(dict, kCGWindowNumber) else {
                continue;
            };
            let Some(bounds) = cf::dict_rect(dict, kCGWindowBounds) else {
                continue;
            };
            let rect = rect_to(bounds);
            if rect.w <= 0 || rect.h <= 0 {
                continue;
            }
            let onscreen = cf::dict_i64(dict, kCGWindowIsOnscreen).map(|v| v != 0);
            let pid = cf::dict_i64(dict, kCGWindowOwnerPID).unwrap_or(0) as u32;
            let process = cf::dict_string(dict, kCGWindowOwnerName).unwrap_or_default();
            // kCGWindowName (the title) is redacted unless the process holds the
            // Screen Recording permission; it is often empty.
            let title = cf::dict_string(dict, kCGWindowName).unwrap_or_default();

            let raw = RawWindow {
                number: number as u32,
                title: &title,
                process: &process,
                pid,
            };
            if !matches_query(&raw, query) {
                continue;
            }

            let (display_id, dpi, scale) = display_for_rect(&displays, &rect);
            // First window (frontmost, since the list is front-to-back) owned by
            // the frontmost app is the focused one.
            let focused = !focused_taken && front == Some(pid as i32);
            if focused {
                focused_taken = true;
            }
            // In the all-windows view, an off-screen window is treated as
            // minimized (its most common cause).
            let visible = onscreen.unwrap_or(true);
            out.push(Window {
                id: number.to_string(),
                title,
                process,
                pid,
                bounds: rect,
                display_id,
                scale,
                dpi,
                visible,
                focused: focused && visible,
                minimized: !visible,
                maximized: false,
                always_on_top: layer > 0,
                z: z as u32,
            });
        }
    }
    Ok(out)
}

struct RawWindow<'a> {
    number: u32,
    title: &'a str,
    process: &'a str,
    pid: u32,
}

fn matches_query(w: &RawWindow, q: &WindowQuery) -> bool {
    if q.is_malformed() {
        return false;
    }
    if q.is_empty() {
        return true;
    }
    if let Some(pid) = q.pid {
        return w.pid == pid;
    }
    let ci = |needle: &str, hay: &str| hay.to_lowercase().contains(&needle.to_lowercase());
    // macOS has no window "class"; treat class: as a process-name match so the
    // grammar stays uniform.
    if let Some(t) = &q.title {
        return ci(t, w.title);
    }
    if let Some(c) = &q.class {
        return ci(c, w.process);
    }
    if let Some(p) = &q.process {
        return ci(p, w.process);
    }
    if let Some(text) = &q.text {
        return ci(text, w.title) || ci(text, w.process) || w.number.to_string() == *text;
    }
    true
}

/// Poll `windows()` until one matches, or time out (exit 5).
pub fn wait_window(query: &WindowQuery, visible: Option<bool>, timeout_ms: u64) -> Result<Window> {
    // Only visible, on-screen windows are enumerated, so `--state hidden` can
    // never be satisfied; reject it up front rather than spinning to a timeout.
    if visible == Some(false) {
        return Err(Error::Usage(
            "wait window --state hidden is unsupported: only visible windows are enumerated".into(),
        ));
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        if let Ok(found) = windows(query)
            && let Some(w) = found
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
