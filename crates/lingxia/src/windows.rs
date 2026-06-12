//! Windows platform bootstrap for pure Rust host apps.

mod shell;
mod terminal_panel;

use std::path::Path;

use lingxia_platform::traits::app_runtime::AppRuntime;
pub use lingxia_platform::{Platform, PlatformError, set_windows_app_exit_handler};
pub use lingxia_webview::platform::windows::{
    WindowsAppMenu, WindowsAppMenuCommandHandler, WindowsAppMenuEntry, WindowsAppMenuItem,
    WindowsDeviceFrame, WindowsDeviceFrameToolbar, set_webview_devtools_enabled,
    set_windows_app_menu,
    set_windows_app_menu_command_handler,
};

/// Initializes the LingXia runtime for a Windows host process.
///
/// Installs logging, the WebView2 user-data directory, and the Windows shell
/// chrome handlers before running the common platform bootstrap. Returns the
/// home app id on success.
pub fn init(platform: Platform) -> Option<String> {
    crate::logging::init();
    lingxia_webview::platform::windows::set_webview_user_data_dir(
        platform.app_cache_dir().join("webview2"),
    );
    shell::install();
    crate::init_with_platform(platform)
}

/// Opens the home lxapp identified by `appid` in the main window.
///
/// Call this after [`init`] returned the home app id.
pub fn open_home_app(appid: &str) -> Result<(), String> {
    shell::set_shell_owner_appid(appid);
    lxapp::open_lxapp(appid, lxapp::LxAppStartupOptions::new(""))
        .map(|_| ())
        .map_err(|err| err.to_string())
}

/// Sets the Windows taskbar and title-bar icon from an image file on disk.
pub fn set_app_icon_from_path(path: &Path) -> Result<(), String> {
    lingxia_webview::platform::windows::set_app_icon_from_path(path).map_err(|err| err.to_string())
}

/// Overrides the initial outer size, in pixels, of webview host windows
/// created after this call — in particular the main window of the host app.
///
/// Call before [`init`] (the first window is created when the home lxapp
/// opens). The first call wins; later calls and non-positive dimensions are
/// ignored. Without an override windows open at the built-in 1024x768.
pub fn set_default_window_size(width: i32, height: i32) {
    lingxia_webview::platform::windows::set_default_window_size(width, height);
}

/// Opens the WebView2 DevTools window for the current page of `appid`.
///
/// Resolves the lxapp's current foreground page webview and dispatches
/// `ICoreWebView2::OpenDevToolsWindow` on its UI thread. Requires DevTools
/// to be enabled (the default; see
/// [`set_webview_devtools_enabled`]).
pub fn open_current_page_devtools(appid: &str) -> Result<(), String> {
    let webview = current_page_webview(appid)?;
    lingxia_webview::platform::windows::open_webview_devtools(&webview.webtag())
        .map_err(|err| err.to_string())
}

/// Resizes the top-level window of `appid` so its content (client) area is
/// exactly `width` x `height` physical pixels, accounting for the caption,
/// borders, and any attached menu bar.
///
/// Resolves the lxapp's current page webview and resizes the window
/// presenting it (attached surfaces resolve to their group host window).
pub fn resize_app_window_content(appid: &str, width: i32, height: i32) -> Result<(), String> {
    let webview = current_page_webview(appid)?;
    lingxia_webview::platform::windows::resize_webview_window_content(
        &webview.webtag(),
        width,
        height,
    )
    .map_err(|err| err.to_string())
}

/// Presents (or clears, with `None`) a simulated-device frame around the
/// top-level window of `appid`: the window becomes a borderless "screen" at
/// exactly the frame's screen size, with rounded corners and a layered
/// bezel-and-shadow companion window glued behind it. While framed, the
/// window has no caption or menu bar; the installed app menu (see
/// [`set_windows_app_menu`]) is offered from the bezel's right-click menu,
/// and dragging the bezel moves the window.
pub fn set_app_window_device_frame(
    appid: &str,
    frame: Option<WindowsDeviceFrame>,
) -> Result<(), String> {
    let webview = current_page_webview(appid)?;
    lingxia_webview::platform::windows::set_webview_device_frame(&webview.webtag(), frame)
        .map_err(|err| err.to_string())
}

fn current_page_webview(appid: &str) -> Result<std::sync::Arc<lingxia_webview::WebView>, String> {
    let app = lxapp::try_get(appid).ok_or_else(|| format!("lxapp is not active: {appid}"))?;
    let page = app.current_page().map_err(|err| err.to_string())?;
    page.webview()
        .ok_or_else(|| "page WebView is not ready".to_string())
}
