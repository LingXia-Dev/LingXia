//! macOS desktop backend.
//!
//! Coordinates are **global display points with a top-left origin** — the
//! native space of `CGWindowList`, `CGEvent`, and the Accessibility API, so
//! window bounds, pointer input, and AX rects are all in the same units. This
//! differs from the Windows backend, which is in physical pixels; on a Retina
//! display one point is `scale` pixels, and screenshots therefore come back
//! `scale`× larger than the point bounds they cover. Each [`Display`] and
//! [`Window`] carries its `scale`/`dpi` so a caller can convert when needed.

#[cfg(feature = "window")]
use crate::error::{Error, Result};
#[cfg(feature = "diagnostics")]
use crate::model::{Capabilities, Doctor, Permissions};
#[cfg(feature = "window")]
use crate::model::{Display, Rect, Window, WindowQuery};
#[cfg(feature = "window")]
use objc2_core_foundation::CGRect;
#[cfg(feature = "window")]
use objc2_core_graphics::{
    CGDirectDisplayID, CGDisplayBounds, CGDisplayCopyDisplayMode, CGDisplayMode,
    CGGetActiveDisplayList, CGMainDisplayID, CGWindowListCopyWindowInfo, CGWindowListOption,
    kCGWindowAlpha, kCGWindowBounds, kCGWindowIsOnscreen, kCGWindowLayer, kCGWindowName,
    kCGWindowNumber, kCGWindowOwnerName, kCGWindowOwnerPID,
};
#[cfg(feature = "diagnostics")]
use objc2_core_graphics::{
    CGPreflightPostEventAccess, CGPreflightScreenCaptureAccess, CGRequestPostEventAccess,
    CGRequestScreenCaptureAccess,
};

#[cfg(feature = "ax")]
mod ax;
#[cfg(any(feature = "diagnostics", feature = "window"))]
mod ax_permission;
#[cfg(feature = "window")]
#[cfg_attr(not(feature = "ax"), allow(dead_code))]
mod axui;
#[cfg(feature = "snapshot")]
mod capture;
#[cfg(feature = "window")]
mod cf;
#[cfg(feature = "clipboard")]
mod clipboard;
#[cfg(feature = "input")]
mod input;
#[cfg(feature = "input")]
mod keymap;
#[cfg(feature = "supervision")]
mod pip;
#[cfg(feature = "process")]
mod process;
#[cfg(feature = "window")]
mod window_ops;

// Borderless panels do not display their title, but WindowServer still exposes
// it as kCGWindowName when Screen Recording metadata is available. Every host
// uses the same deliberately collision-resistant sentinel so one product does
// not enumerate and automate another product's activity viewer.
#[cfg(feature = "window")]
const VIEWER_WINDOW_SENTINEL: &str = "__lingxia_activity_viewer_8d61c5b2_v1__";

#[cfg(feature = "window")]
fn current_viewer_window() -> u32 {
    #[cfg(feature = "supervision")]
    {
        pip::window_number()
    }
    #[cfg(not(feature = "supervision"))]
    {
        0
    }
}

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

/// Convert a `CGRect` (points) to the model's integer `Rect`.
#[cfg(feature = "window")]
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
#[cfg(feature = "window")]
pub(crate) fn parse_window_id(id: &str) -> Result<u32> {
    let parsed = if let Some(hex) = id.strip_prefix("0x").or_else(|| id.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16)
    } else {
        id.parse::<u32>()
    };
    parsed.map_err(|_| Error::Usage(format!("invalid window id '{id}'")))
}

/// Best-effort macOS product version (e.g. "14.5") via sysctl.
#[cfg(feature = "diagnostics")]
#[cfg(feature = "diagnostics")]
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

/// The bundled app the OS will attribute a grant to. Bare binaries have no
/// bundle URL and must fall back to the terminal that macOS actually records
/// in its privacy database.
#[cfg(feature = "window")]
pub(crate) fn responsible_app_name() -> Option<String> {
    use objc2_app_kit::NSRunningApplication;

    let app = NSRunningApplication::currentApplication();
    app.bundleURL()?;
    app.localizedName()
        .map(|name| name.to_string())
        .filter(|name| !name.is_empty())
}

#[cfg(feature = "diagnostics")]
pub fn permissions() -> Permissions {
    Permissions {
        accessibility: ax_permission::is_trusted(),
        screen_recording: CGPreflightScreenCaptureAccess(),
        input: CGPreflightPostEventAccess(),
    }
}

/// Prompt for any permission not yet granted, then re-report. macOS shows the
/// system dialog / adds the app to the relevant list; the user still approves.
#[cfg(feature = "diagnostics")]
pub fn request_permissions() -> Permissions {
    if !CGPreflightScreenCaptureAccess() {
        let _ = CGRequestScreenCaptureAccess();
    }
    if !CGPreflightPostEventAccess() {
        let _ = CGRequestPostEventAccess();
    }
    if !ax_permission::is_trusted() {
        let _ = ax_permission::prompt_trusted();
    }
    permissions()
}

#[cfg(feature = "diagnostics")]
pub fn doctor() -> Doctor {
    Doctor {
        backend: "macos".to_string(),
        os: "macos".to_string(),
        os_version: os_version(),
        permissions: permissions(),
        capabilities: Capabilities {
            displays: cfg!(feature = "window"),
            windows: cfg!(feature = "window"),
            screenshot: cfg!(feature = "snapshot"),
            // CGWindowListCreateImage composites the target window's own
            // backing store, so occluded regions still come through.
            window_screenshot_occlusion_independent: cfg!(feature = "snapshot"),
            pixel: cfg!(feature = "snapshot"),
            pointer: cfg!(feature = "input"),
            key: cfg!(feature = "input"),
            window_management: cfg!(feature = "window"),
            clipboard: cfg!(feature = "clipboard"),
            ax_tree: cfg!(feature = "ax"),
            ..Capabilities::default()
        },
    }
}

// ============================ displays ============================

#[cfg(feature = "window")]
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
            work_area: display_work_area(id, bounds),
            scale,
            dpi: (72.0 * scale).round() as u32,
        });
    }
    Ok(out)
}

/// Return `NSScreen.visibleFrame` in the same top-left coordinate space as
/// `CGDisplayBounds`. AppKit is main-thread-only, so non-main callers retain the
/// safe full-display fallback.
#[cfg(feature = "window")]
fn display_work_area(id: CGDirectDisplayID, bounds: CGRect) -> Rect {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;
    use objc2_foundation::{NSNumber, NSString};

    let Some(mtm) = MainThreadMarker::new() else {
        return rect_to(bounds);
    };
    let screens = NSScreen::screens(mtm);
    let screen_number = NSString::from_str("NSScreenNumber");
    let Some(screen) = screens.iter().find(|screen| {
        screen
            .deviceDescription()
            .objectForKey(&screen_number)
            .and_then(|value| value.downcast_ref::<NSNumber>().map(NSNumber::as_u32))
            == Some(id)
    }) else {
        return rect_to(bounds);
    };
    let frame = screen.frame();
    let visible = screen.visibleFrame();
    let left = visible.origin.x - frame.origin.x;
    let top = (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
    rect_to(CGRect::new(
        objc2_core_foundation::CGPoint::new(bounds.origin.x + left, bounds.origin.y + top),
        visible.size,
    ))
}

/// A display's backing scale factor (2.0 on Retina), from its current mode.
#[cfg(feature = "window")]
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
#[cfg(feature = "window")]
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
#[cfg(feature = "window")]
pub(crate) fn frontmost_pid() -> Option<i32> {
    use objc2_app_kit::NSWorkspace;
    NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .map(|app| app.processIdentifier())
}

/// List on-screen OS windows (the public `desktop windows` surface).
#[cfg(feature = "window")]
pub fn windows(query: &WindowQuery) -> Result<Vec<Window>> {
    enumerate(query, true)
}

/// Locate a single window by `CGWindowID`, including minimized/off-screen ones
/// (`OptionAll`). Used by window operations, which must still reach a window a
/// user has minimized — those drop out of the on-screen list.
#[cfg(feature = "window")]
pub(crate) fn window_record(wid: u32) -> Option<Window> {
    enumerate(&WindowQuery::default(), false)
        .ok()?
        .into_iter()
        .find(|w| w.id == wid.to_string())
}

/// Enumerate windows. `only_onscreen` picks `OptionOnScreenOnly` (visible
/// windows, front-to-back) vs `OptionAll` (also minimized/off-screen).
#[cfg(feature = "window")]
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
        return Err(Error::Unavailable(
            "CGWindowListCopyWindowInfo failed".into(),
        ));
    };
    let array = (&*info as *const objc2_core_foundation::CFArray).cast::<std::ffi::c_void>();
    let displays = displays().unwrap_or_default();
    let front = frontmost_pid();
    let viewer_window = current_viewer_window();

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
            // The exact number always protects this process. The shared title
            // also protects viewers owned by other LingXia products when macOS
            // has not redacted window names from CGWindow metadata.
            let title = cf::dict_string(dict, kCGWindowName).unwrap_or_default();
            if is_viewer_window(number as u32, viewer_window, &title) {
                continue;
            }
            let Some(bounds) = cf::dict_rect(dict, kCGWindowBounds) else {
                continue;
            };
            let rect = rect_to(bounds);
            if rect.w <= 0 || rect.h <= 0 {
                continue;
            }
            let onscreen = cf::dict_bool(dict, kCGWindowIsOnscreen);
            let pid = cf::dict_i64(dict, kCGWindowOwnerPID).unwrap_or(0) as u32;
            let process = cf::dict_string(dict, kCGWindowOwnerName).unwrap_or_default();
            // kCGWindowName (the title) is redacted unless the process holds the
            // Screen Recording permission; it is often empty.

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
            // First normal window owned by the frontmost app is the focused
            // one. Status items and other elevated helper windows can precede
            // the key window in this list, but they do not own keyboard focus.
            let focused = layer == 0 && !focused_taken && front == Some(pid as i32);
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

#[cfg(feature = "window")]
fn is_viewer_window(number: u32, viewer_window: u32, title: &str) -> bool {
    (viewer_window != 0 && number == viewer_window) || title == VIEWER_WINDOW_SENTINEL
}

#[cfg(feature = "window")]
struct RawWindow<'a> {
    number: u32,
    title: &'a str,
    process: &'a str,
    pid: u32,
}

#[cfg(feature = "window")]
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
#[cfg(feature = "window")]
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

#[cfg(all(test, feature = "window"))]
mod tests {
    use super::{VIEWER_WINDOW_SENTINEL, is_viewer_window};

    #[test]
    fn viewer_is_excluded_only_after_its_window_exists() {
        assert!(!is_viewer_window(42, 0, "ordinary window"));
        assert!(is_viewer_window(42, 42, ""));
        assert!(!is_viewer_window(41, 42, "ordinary window"));
        assert!(
            is_viewer_window(900, 42, VIEWER_WINDOW_SENTINEL),
            "another product's viewer is excluded by its shared identity"
        );
    }
}
