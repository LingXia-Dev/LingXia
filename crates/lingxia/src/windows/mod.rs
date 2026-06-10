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
    lxapp::open_lxapp(appid, lxapp::LxAppStartupOptions::new(""))
        .map(|_| ())
        .map_err(|err| err.to_string())
}

/// Sets the Windows taskbar and title-bar icon from an image file on disk.
pub fn set_app_icon_from_path(path: &Path) -> Result<(), String> {
    lingxia_webview::platform::windows::set_app_icon_from_path(path).map_err(|err| err.to_string())
}
