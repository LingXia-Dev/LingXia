//! Windows platform bootstrap for pure Rust host apps.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};

use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_platform::traits::ui::{
    SurfaceContent, SurfaceKind, SurfacePosition, SurfacePresenter, SurfaceRequest, SurfaceRole,
};
pub use lingxia_platform::{Platform, PlatformError, set_windows_app_exit_handler};
use lingxia_webview::{NavigationPolicy, WebTag, WebViewController, WebViewDataMode};

static WINDOWS_APP_VISIBLE_WEBTAGS: LazyLock<Mutex<HashMap<String, HashSet<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub const RUNNER_WEB_APP_ID: &str = "__lingxia_runner_web__";
pub const RUNNER_WEB_SESSION_ID: u64 = 1;

/// Initializes the LingXia runtime for a Windows host process.
///
/// Installs logging and the WebView2 user-data directory before running the
/// common platform bootstrap. Returns the home app id on success.
pub fn init(platform: Platform) -> crate::Result<Option<String>> {
    crate::logging::init();
    lingxia_webview::platform::windows::set_webview_user_data_dir(
        platform.app_cache_dir().join("webview2"),
    );
    install_lifecycle_bridge();
    install_url_surface_bridge();
    install_aside_panel_bridge();
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

/// Opens a host-owned URL as a chrome-free main surface.
pub fn open_web_surface(url: &str) -> Result<(), String> {
    let url = url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("web surface URL must use http:// or https://".to_string());
    }
    let platform = crate::runtime::platform().map_err(|error| error.to_string())?;
    platform
        .present_surface(SurfaceRequest {
            id: "runner-web-main".to_string(),
            app_id: RUNNER_WEB_APP_ID.to_string(),
            path: url.to_string(),
            session_id: RUNNER_WEB_SESSION_ID,
            page_instance_id: String::new(),
            content: SurfaceContent::Url,
            kind: SurfaceKind::Window,
            width: 0.0,
            height: 0.0,
            width_ratio: 0.0,
            height_ratio: 0.0,
            position: SurfacePosition::Center,
            role: SurfaceRole::Main,
            ephemeral_web_data: false,
            url_callback: false,
        })
        .map_err(|error| error.to_string())
}

/// Overrides the initial outer size, in pixels, of Windows host windows
/// created after this call, in particular the main window of the host app.
///
/// Call before [`init`] (the first window is created when the home lxapp
/// opens). The first call wins; later calls and non-positive dimensions are
/// ignored. Without an override windows open at the built-in 1024x768.
pub fn set_default_window_size(width: i32, height: i32) {
    lingxia_windows_contract::set_default_window_size(width, height);
}

/// Resizes the top-level window of `appid` so its content (client) area is
/// exactly `width` x `height` physical pixels, accounting for the caption,
/// borders, and any attached menu bar.
///
/// Resolves the lxapp's current page webview and resizes the window
/// presenting it (attached surfaces resolve to their group host window).
pub fn resize_app_window_content(appid: &str, width: i32, height: i32) -> Result<(), String> {
    let webview = current_page_webview(appid)?;
    lingxia_windows_contract::resize_host_window_content(&webview.webtag(), width, height)
        .map_err(|err| err.to_string())
}

/// Feed the host window's logical (DIP) width to `appid`'s adaptive surface
/// graph so the size class (and therefore the aside cap) tracks the real
/// window. Without this the graph stays at its seed width — permanently
/// `Medium` (max 1 aside) — so a second aside evicts the first even on a wide
/// window. Called on window create/resize. Returns `false` if the app is not
/// active (e.g. during teardown).
pub fn set_surface_width(appid: &str, width: f64) -> bool {
    lxapp::try_get(appid)
        .map(|app| app.set_surface_width(width))
        .unwrap_or(false)
}

pub fn set_surface_layout_metrics(appid: &str, width: f64, sidebar_width: f64) -> bool {
    lxapp::try_get(appid)
        .map(|app| app.set_surface_layout_metrics(width, sidebar_width))
        .unwrap_or(false)
}

pub fn set_surface_sidebar_width(appid: &str, width: f64) -> bool {
    lxapp::try_get(appid)
        .map(|app| app.set_surface_sidebar_width(width))
        .unwrap_or(false)
}

pub fn set_surface_viewport(appid: &str, width: f64, height: f64) -> bool {
    lxapp::try_get(appid)
        .map(|app| app.set_surface_viewport(width, height))
        .unwrap_or(false)
}

fn current_page_webview(appid: &str) -> Result<std::sync::Arc<lingxia_webview::WebView>, String> {
    let app = lxapp::try_get(appid).ok_or_else(|| format!("lxapp is not active: {appid}"))?;
    let page = app.current_page().map_err(|err| err.to_string())?;
    page.webview()
        .ok_or_else(|| "page WebView is not ready".to_string())
}

fn install_lifecycle_bridge() {
    lingxia_windows_contract::set_webview_visibility_handler(Arc::new(|webtag, visible| {
        on_webview_visibility_changed(webtag, visible);
    }));
}

fn install_aside_panel_bridge() {
    lingxia_platform::install_windows_aside_panel_bridge();
}

fn install_url_surface_bridge() {
    lingxia_platform::set_windows_url_surface_handler(Arc::new(|request| {
        if request.content != SurfaceContent::Url {
            return None;
        }
        // The data mode is orthogonal to browser behavior, so ephemeral (auth
        // handoff) and ordinary URL surfaces share the same tab delegate and
        // error UI.
        let data_mode = if request.ephemeral_web_data {
            WebViewDataMode::Ephemeral
        } else {
            WebViewDataMode::ProfileDefault
        };
        // With the browser engine compiled in, a URL surface is a
        // standalone managed browser tab (downloads, new-window policy:
        // window.open from a docked aside opens a sibling aside tab) - macOS
        // DockedBrowser parity. Without it (plain builds), a browser-profile
        // WebView2 renders the URL directly.
        #[cfg(feature = "browser-runtime")]
        if request.app_id != RUNNER_WEB_APP_ID
            && let Some(resolved) = resolve_url_surface_as_browser_tab(request, data_mode)
        {
            return Some(resolved);
        }
        // `teardown_surface` destroys this webview by its webtag, so no cleanup hook.
        let webtag = WebTag::new(&request.app_id, &request.path, Some(request.session_id));
        let url = request.path.clone();
        let url_callback = request.url_callback;
        let session = lingxia_webview::WebViewBuilder::browser(webtag)
            .data_mode(data_mode)
            .on_navigation(move |request| url_surface_navigation_policy(url_callback, &request.url))
            .create();
        std::mem::drop(crate::task::spawn(async move {
            match session.wait_ready().await {
                Ok(webview) => {
                    if let Err(err) = webview.load_url(&url) {
                        log::error!("URL surface failed to load {url}: {err}");
                    }
                }
                Err(err) => log::error!("URL surface webview create failed for {url}: {err}"),
            }
        }));
        Some(lingxia_platform::WindowsUrlSurfaceWebTag {
            app_id: request.app_id.clone(),
            path: request.path.clone(),
            session_id: request.session_id,
            cleanup: None,
        })
    }));
}

fn url_uses_scheme(url: &str, expected: &str) -> bool {
    url.trim()
        .split_once(':')
        .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case(expected))
}

fn url_surface_navigation_policy(url_callback: bool, url: &str) -> NavigationPolicy {
    if url_callback && url_uses_scheme(url, "file") {
        NavigationPolicy::Cancel
    } else {
        NavigationPolicy::Allow
    }
}

#[cfg(feature = "browser-runtime")]
fn resolve_url_surface_as_browser_tab(
    request: &lingxia_platform::traits::ui::SurfaceRequest,
    data_mode: WebViewDataMode,
) -> Option<lingxia_platform::WindowsUrlSurfaceWebTag> {
    let tab_id = crate::browser::open_standalone_for_app(
        &request.app_id,
        request.session_id,
        &request.path,
        None,
        data_mode,
        request.url_callback,
    )
    .inspect_err(|err| log::warn!("URL surface browser tab failed for {}: {err}", request.path))
    .ok()?;
    let tab = crate::browser::tab_summary(&tab_id)?;
    let close_tab_id = tab_id.clone();
    Some(lingxia_platform::WindowsUrlSurfaceWebTag {
        app_id: crate::browser::APP_ID.to_string(),
        path: tab.path,
        session_id: tab.session_id,
        cleanup: Some(Arc::new(move || {
            let _ = crate::browser::close(&close_tab_id);
        })),
    })
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

    let app_event = update_app_visible_webtags(&appid, webtag.key(), visible);

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

fn update_app_visible_webtags(appid: &str, webtag_key: &str, visible: bool) -> Option<bool> {
    let Ok(mut visible_webtags) = WINDOWS_APP_VISIBLE_WEBTAGS.lock() else {
        return None;
    };
    let webtags = visible_webtags.entry(appid.to_string()).or_default();
    let was_visible = !webtags.is_empty();
    if visible {
        webtags.insert(webtag_key.to_string());
    } else {
        webtags.remove(webtag_key);
    }
    let is_visible = !webtags.is_empty();
    if !is_visible {
        visible_webtags.remove(appid);
    }
    (was_visible != is_visible).then_some(is_visible)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_visibility_is_aggregated_across_webtags() {
        WINDOWS_APP_VISIBLE_WEBTAGS.lock().unwrap().clear();

        assert_eq!(
            update_app_visible_webtags("app", "app:main", true),
            Some(true)
        );
        assert_eq!(update_app_visible_webtags("app", "app:panel", true), None);
        assert_eq!(update_app_visible_webtags("app", "app:panel", false), None);
        assert_eq!(
            update_app_visible_webtags("app", "app:main", false),
            Some(false)
        );
    }

    #[test]
    fn standard_url_surface_policy_keeps_file_for_browsers_only() {
        assert_eq!(
            url_surface_navigation_policy(false, "file:///C:/Temp/report.html"),
            NavigationPolicy::Allow
        );
        assert_eq!(
            url_surface_navigation_policy(true, "FILE:/C:/Temp/report.html"),
            NavigationPolicy::Cancel
        );
        assert_eq!(
            url_surface_navigation_policy(true, "http://127.0.0.1:18080/authorize"),
            NavigationPolicy::Allow
        );
    }
}
