//! Windows host-window API owned by the Windows SDK layer.
//!
//! This module is the boundary used by LingXia Windows runtime code for
//! custom chrome, grouped host windows, native panels, and HWND-bound host
//! callbacks. `lingxia-webview` remains the lower WebView2 surface provider.

pub use lingxia_webview::platform::windows::lingxia_host::{
    HostWindowCreatedHandler, WindowsChromeAttachedLayout, WindowsChromeAttachedState,
    WindowsChromeCommand, WindowsChromeHit, WindowsChromePanel, WindowsChromePanelLayout,
    WindowsChromePanelLayoutInput, WindowsChromeRenderer, WindowsChromeState, WindowsFrameButton,
    WindowsHostPanelContent, WindowsHostPanelInputHandler, WindowsHostPanelKeyEvent,
    WindowsHostPanelTab, WindowsPanelPosition, WindowsWebViewHostWindow, WindowsWindowLayout,
    add_webview_host_window_created_handler, clear_host_panel_input_handler,
    find_webview_host_window, hide_host_panel, invalidate_host_panel, is_panel_visible,
    request_webview_host_window_layout, restore_presented_group_main, set_default_window_size,
    set_host_panel_input_handler, set_host_panel_maximized, set_host_panel_tabs,
    set_webview_chrome_event_handler, set_webview_close_handler, set_webview_window_layout,
    set_windows_chrome_renderer, show_interactive_host_panel, update_host_panel_body,
};
