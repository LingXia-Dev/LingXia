//! Public Windows API surface: window/panel entry points,
//! handler registries, and the exported layout/event types.

use super::*;

mod callbacks;
mod layout;
mod panel;
mod surface;

pub use callbacks::{
    HostWindowCreatedHandler, add_webview_host_window_created_handler,
    clear_host_panel_input_handler, post_to_window_thread, set_host_panel_input_handler,
    set_webview_chrome_event_handler, set_webview_close_handler, set_webview_user_data_dir,
};
pub(crate) use callbacks::{
    WINDOW_HOST_PANEL_INPUT_HANDLERS, WM_LINGXIA_RUN_CALLBACK, configured_webview_user_data_dir,
    invoke_chrome_event_handler, invoke_close_handler, invoke_host_window_created_handler,
    remove_chrome_event_handler, remove_close_handler, run_posted_window_callback,
};
pub use layout::*;
pub use panel::*;
pub use surface::*;

/// Built-in initial outer size of top-level webview host windows, used when
/// no process-wide override was installed via [`set_default_window_size`].
const BUILTIN_DEFAULT_WINDOW_SIZE: (i32, i32) = (1024, 768);

/// Process-wide override of the initial outer window size, set at most once
/// before the first window is created (see [`set_default_window_size`]).
static DEFAULT_WINDOW_SIZE: OnceLock<(i32, i32)> = OnceLock::new();

/// Overrides the initial outer size, in pixels, of top-level webview host
/// windows created after this call, in particular the main window of a
/// host app. Attached surfaces (panels, presented main children) are
/// re-laid out by their group and are unaffected in practice.
///
/// Call once during host bootstrap, before the first webview window is
/// created. The first call wins; later calls and non-positive dimensions
/// are ignored. Without an override windows open at 1024x768.
pub fn set_default_window_size(width: i32, height: i32) {
    if width <= 0 || height <= 0 {
        log::warn!("ignoring non-positive default window size {width}x{height}");
        return;
    }
    if DEFAULT_WINDOW_SIZE.set((width, height)).is_err() {
        log::warn!("default window size already set; ignoring {width}x{height}");
    }
}

/// Initial outer size for newly created top-level webview host windows.
pub(crate) fn default_window_size() -> (i32, i32) {
    DEFAULT_WINDOW_SIZE
        .get()
        .copied()
        .unwrap_or(BUILTIN_DEFAULT_WINDOW_SIZE)
}

/// Geometry of a webview's own content window, for host integrations that
/// place native child controls (embedded components) over the rendered
/// page. Unlike [`WindowsWebViewWindowSnapshot`], `window` is always the
/// webview's own window: the correct parent for overlay children, never
/// the group host.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowsWebViewContentWindow {
    /// Raw handle of the webview's own window (parent for overlay children).
    pub window: isize,
    /// WebView2 content origin within the window's client area, physical px.
    pub content_left: i32,
    pub content_top: i32,
    /// WebView2 content size, physical px.
    pub content_width: i32,
    pub content_height: i32,
    /// Physical pixels per CSS pixel (window DPI / 96).
    pub scale: f64,
}

/// Resolves the content-window geometry for `webtag`, or `None` while the
/// webview has no registered window yet (not shown/attached). Pure registry
/// and Win32 reads; safe to call from any thread.
pub fn find_webview_content_window(webtag: &WebTag) -> Option<WindowsWebViewContentWindow> {
    let hwnd = window_handle_for_key(webtag.key())?;
    let mut client = RECT::default();
    unsafe {
        WindowsAndMessaging::GetClientRect(hwnd, &mut client).ok()?;
    }
    let content = controller_bounds_for_window(hwnd, webtag.key(), client);
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) };
    let scale = if dpi == 0 { 1.0 } else { dpi as f64 / 96.0 };
    Some(WindowsWebViewContentWindow {
        window: hwnd_handle(hwnd),
        content_left: content.left,
        content_top: content.top,
        content_width: (content.right - content.left).max(0),
        content_height: (content.bottom - content.top).max(0),
        scale,
    })
}

/// Top-level host window currently presenting a webview surface.
///
/// For a standalone webview this is its own window. For attached main
/// surfaces and panels, this resolves to the host group host that actually
/// owns window chrome, menus, sizing, and host-level presentation effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsWebViewHostWindow {
    /// Raw host `HWND` as an integer handle.
    pub window: isize,
}

pub(crate) fn webview_host_hwnd(webtag: &WebTag) -> StdResult<HWND> {
    let hwnd = window_handle_for_key(webtag.key()).ok_or_else(|| {
        WebViewError::WebView(format!("no window registered for {}", webtag.key()))
    })?;
    match window_attachment(webtag.key()) {
        Some(WindowAttachment {
            group_key,
            kind: WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. },
        }) => host_handle_for_group(&group_key).ok_or_else(|| {
            WebViewError::WebView(format!("no host window for Windows host group {group_key}"))
        }),
        _ => Ok(hwnd),
    }
}

/// Resolves the host window currently presenting `webtag`.
///
/// This is generic webview hosting state; host integrations can use the handle
/// with [`post_to_window_thread`] for host-window UI work without knowing
/// how LingXia host groups attach child webviews internally.
pub fn find_webview_host_window(webtag: &WebTag) -> StdResult<WindowsWebViewHostWindow> {
    webview_host_hwnd(webtag).map(|window| WindowsWebViewHostWindow {
        window: hwnd_handle(window),
    })
}

/// Requests a layout pass for a WebView host window previously surfaced by
/// this crate. This is intentionally only a host-window primitive: callers
/// decide why layout is needed (for example after attaching native chrome).
pub fn request_webview_host_window_layout(window: WindowsWebViewHostWindow) -> bool {
    if !is_window_handle_valid(window.window) {
        return false;
    }
    unsafe {
        WindowsAndMessaging::PostMessageW(
            Some(hwnd_from_handle(window.window)),
            WM_LINGXIA_LAYOUT,
            WPARAM::default(),
            LPARAM::default(),
        )
        .is_ok()
    }
}
