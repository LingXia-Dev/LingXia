//! Windows platform bootstrap for pure Rust host apps.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use lingxia_platform::traits::app_runtime::AppRuntime;
pub use lingxia_platform::{Platform, PlatformError, set_windows_app_exit_handler};
use lingxia_webview::WebTag;

static WINDOWS_APP_VISIBILITY: LazyLock<Mutex<HashMap<String, bool>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Initializes the LingXia runtime for a Windows host process.
///
/// Installs logging and the WebView2 user-data directory before running the
/// common platform bootstrap. Returns the home app id on success.
pub fn init(platform: Platform) -> Option<String> {
    crate::logging::init();
    lingxia_webview::platform::windows::set_webview_user_data_dir(
        platform.app_cache_dir().join("webview2"),
    );
    install_lifecycle_bridge();
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

/// Overrides the initial outer size, in pixels, of Windows host windows
/// created after this call, in particular the main window of the host app.
///
/// Call before [`init`] (the first window is created when the home lxapp
/// opens). The first call wins; later calls and non-positive dimensions are
/// ignored. Without an override windows open at the built-in 1024x768.
pub fn set_default_window_size(width: i32, height: i32) {
    lingxia_windows_host::set_default_window_size(width, height);
}

/// Resizes the top-level window of `appid` so its content (client) area is
/// exactly `width` x `height` physical pixels, accounting for the caption,
/// borders, and any attached menu bar.
///
/// Resolves the lxapp's current page webview and resizes the window
/// presenting it (attached surfaces resolve to their group host window).
pub fn resize_app_window_content(appid: &str, width: i32, height: i32) -> Result<(), String> {
    let webview = current_page_webview(appid)?;
    lingxia_windows_host::resize_host_window_content(&webview.webtag(), width, height)
        .map_err(|err| err.to_string())
}

fn current_page_webview(appid: &str) -> Result<std::sync::Arc<lingxia_webview::WebView>, String> {
    let app = lxapp::try_get(appid).ok_or_else(|| format!("lxapp is not active: {appid}"))?;
    let page = app.current_page().map_err(|err| err.to_string())?;
    page.webview()
        .ok_or_else(|| "page WebView is not ready".to_string())
}

fn install_lifecycle_bridge() {
    lingxia_windows_host::set_webview_visibility_handler(Arc::new(|webtag, visible| {
        on_webview_visibility_changed(webtag, visible);
    }));
}

fn on_webview_visibility_changed(webtag: &WebTag, visible: bool) {
    let (appid, path) = webtag.extract_parts();
    if appid.is_empty() || path.is_empty() {
        return;
    }

    if let Err(err) = lxapp::notify_page_host_visibility(&appid, &path, visible) {
        log::debug!(
            "Windows page visibility event ignored for {} visible={}: {}",
            webtag,
            visible,
            err
        );
    }

    let app_event = {
        let Ok(mut visibility) = WINDOWS_APP_VISIBILITY.lock() else {
            return;
        };
        match visibility.insert(appid.clone(), visible) {
            Some(previous) if previous != visible => Some(visible),
            _ => None,
        }
    };

    if let Some(visible) = app_event
        && let Err(err) = lxapp::notify_lxapp_host_visibility(&appid, visible)
    {
        log::debug!(
            "Windows app visibility event ignored for {} visible={}: {}",
            appid,
            visible,
            err
        );
    }
}
