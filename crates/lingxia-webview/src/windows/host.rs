//! Win32 host-window mechanics for Windows WebView2 surfaces.
//!
//! These modules are not shell UI. They own generic host-window behavior
//! needed to present WebView2 content on Windows: HWND lifecycle, grouping,
//! and custom chrome renderer hooks. A few HWND-bound mechanics are kept as
//! hidden bridge internals for the Windows host SDK because they must run
//! inside the WebView host window procedure.

use super::*;

mod api;
mod groups;
mod renderer;
mod window;

pub use api::{
    HostWindowCreatedHandler, WindowsHostPanelInputHandler, WindowsHostPanelKeyEvent,
    WindowsPanelPosition, WindowsWebViewContentWindow, WindowsWebViewHandler,
    WindowsWebViewHostWindow, WindowsWebViewWindowSnapshot, WindowsWindowLayout,
    add_webview_host_window_created_handler, clear_host_panel_input_handler,
    clear_webview_group_override, clear_webview_os_frame, set_webview_group_override,
    set_webview_os_frame,
    find_webview_content_window, find_webview_handler, find_webview_host_window, hide_host_panel,
    invalidate_host_panel, is_panel_visible, post_to_window_thread,
    request_webview_host_window_layout, restore_presented_group_main, set_default_window_size,
    set_host_panel_input_handler, set_host_panel_maximized, set_host_panel_tabs,
    set_webview_chrome_event_handler, set_webview_close_handler, set_webview_devtools_enabled,
    set_webview_user_data_dir, set_webview_window_layout, show_interactive_host_panel,
    update_host_panel_body,
};
pub(crate) use api::{
    WINDOW_HOST_PANEL_INPUT_HANDLERS, WM_LINGXIA_RUN_CALLBACK, configured_webview_user_data_dir,
    invoke_chrome_event_handler, invoke_close_handler, invoke_host_window_created_handler,
    remove_chrome_event_handler, remove_close_handler, run_posted_window_callback,
};
pub use renderer::{
    WindowsChromeAttachedLayout, WindowsChromeAttachedState, WindowsChromeCommand,
    WindowsChromeHit, WindowsChromePanel, WindowsChromePanelLayout, WindowsChromePanelLayoutInput,
    WindowsChromeRenderer, WindowsChromeState, WindowsFrameButton, WindowsHostPanelContent,
    WindowsHostPanelTab, set_windows_chrome_renderer,
};
pub use window::{WindowsCardDecorator, set_windows_card_decorator};

pub(crate) use api::*;
pub(crate) use groups::*;
pub(crate) use renderer::*;
pub(crate) use window::*;
