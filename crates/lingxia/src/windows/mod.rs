//! Windows platform bootstrap for pure Rust host apps.

mod shell;
mod terminal_panel;

use std::path::Path;

use lingxia_platform::traits::app_runtime::AppRuntime;
pub use lingxia_platform::{Platform, PlatformError, set_windows_app_exit_handler};

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
