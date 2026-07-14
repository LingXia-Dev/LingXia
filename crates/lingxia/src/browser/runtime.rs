//! Internal adapter for the optional `lingxia-browser` runtime.
#![cfg_attr(target_os = "windows", allow(dead_code))]

#[cfg(feature = "browser-runtime")]
pub(crate) const APP_ID: &str = lingxia_browser::BUILTIN_BROWSER_APPID;
#[cfg(not(feature = "browser-runtime"))]
pub(crate) const APP_ID: &str = "";

#[cfg(not(feature = "browser-runtime"))]
fn unavailable<T>() -> Result<T, lxapp::LxAppError> {
    Err(lxapp::LxAppError::UnsupportedOperation(
        "browser not available (browser feature disabled)".to_string(),
    ))
}

pub(crate) fn register_bundled_app() {
    #[cfg(feature = "browser-runtime")]
    lingxia_browser::register_bundled_app();
}

#[cfg(all(feature = "browser-runtime", not(feature = "browser-shell")))]
pub(crate) fn install_runtime_once() {
    use std::sync::OnceLock;
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(lingxia_browser::install_runtime);
}

#[cfg(all(feature = "browser-runtime", not(feature = "browser-shell")))]
pub(crate) fn register_bundled_app_once() {
    use std::sync::OnceLock;
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(lingxia_browser::register_bundled_app);
}

#[cfg(all(feature = "browser-runtime", not(feature = "browser-shell")))]
pub(crate) fn warmup() {
    lingxia_browser::warmup();
}

pub(crate) fn open_for_app(
    appid: &str,
    session_id: u64,
    url: &str,
    tab_id: Option<&str>,
) -> Result<String, lxapp::LxAppError> {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::open_for_app(appid, session_id, url, tab_id);
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = (appid, session_id, url, tab_id);
        unavailable()
    }
}

/// Open an aside tab in the shared in-app browser: self chrome minus the
/// address bar.
pub(crate) fn open_aside_for_app(
    appid: &str,
    session_id: u64,
    url: &str,
    tab_id: Option<&str>,
) -> Result<String, lxapp::LxAppError> {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::open_aside_for_app(appid, session_id, url, tab_id);
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = (appid, session_id, url, tab_id);
        unavailable()
    }
}

/// Whether `tab_id` was opened as an aside (chrome hides its address bar).
pub(crate) fn tab_is_aside(tab_id: &str) -> bool {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::tab_is_aside(tab_id);
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = tab_id;
        false
    }
}

/// Open a standalone (no-tab-strip) browser tab for a docked aside browser.
/// New-window requests load inline in the same WebView.
#[cfg(any(target_os = "ios", target_os = "macos", target_os = "windows"))]
pub(crate) fn open_standalone_for_app(
    appid: &str,
    session_id: u64,
    url: &str,
    tab_id: Option<&str>,
    data_mode: lingxia_webview::WebViewDataMode,
) -> Result<String, lxapp::LxAppError> {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::open_standalone_for_app(appid, session_id, url, tab_id, data_mode);
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = (appid, session_id, url, tab_id, data_mode);
        unavailable()
    }
}

/// Whether the optional browser runtime is compiled in.
#[cfg(target_os = "windows")]
pub(crate) fn runtime_enabled() -> bool {
    cfg!(feature = "browser-runtime")
}

/// Crate-internal projection of a browser tab for shell UI policy
/// (sidebar rows, presentation targets).
#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
pub(crate) struct BrowserTabSummary {
    pub(crate) tab_id: String,
    pub(crate) path: String,
    pub(crate) session_id: u64,
    pub(crate) title: Option<String>,
    pub(crate) current_url: Option<String>,
    /// PNG favicon of the tab's current page, when the webview reported one.
    pub(crate) favicon_png: Option<std::sync::Arc<Vec<u8>>>,
}

#[cfg(all(target_os = "windows", feature = "browser-runtime"))]
fn tab_summary_from_info(info: lingxia_browser::BrowserTabInfo) -> BrowserTabSummary {
    let favicon_png = lingxia_browser::tab_favicon(&info.tab_id);
    BrowserTabSummary {
        tab_id: info.tab_id,
        path: info.path,
        session_id: info.session_id,
        title: info.title,
        current_url: info.current_url,
        favicon_png,
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn tabs() -> Vec<BrowserTabSummary> {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::tabs()
        .into_iter()
        .map(tab_summary_from_info)
        .collect();
    #[cfg(not(feature = "browser-runtime"))]
    Vec::new()
}

#[cfg(target_os = "windows")]
pub(crate) fn tab_summary(tab_id: &str) -> Option<BrowserTabSummary> {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::tabs()
        .into_iter()
        .find(|tab| tab.tab_id == tab_id)
        .map(tab_summary_from_info);
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = tab_id;
        None
    }
}

/// Marks `tab_id` as the active browser tab. Returns `false` when the tab
/// does not exist (or the browser runtime is disabled).
#[cfg(target_os = "windows")]
pub(crate) fn activate(tab_id: &str) -> bool {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::activate(tab_id).is_ok();
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = tab_id;
        false
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn set_tabs_changed_handler(handler: std::sync::Arc<dyn Fn() + Send + Sync>) {
    #[cfg(feature = "browser-runtime")]
    lingxia_browser::set_tabs_changed_handler(handler);
    #[cfg(not(feature = "browser-runtime"))]
    let _ = handler;
}

/// Navigates an existing browser tab to `url`. Uses the global tab scope so
/// the runtime tab id is kept as-is (owner scoping would derive a new id).
#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android",
    target_env = "ohos"
))]
pub(crate) fn navigate(tab_id: &str, url: &str) -> Result<(), lxapp::LxAppError> {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::open(url, Some(tab_id)).map(|_| ());
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = (tab_id, url);
        unavailable()
    }
}

/// Navigates the tab's webview back in history. Returns `false` when the
/// tab is gone or the browser runtime is disabled.
#[cfg(target_os = "windows")]
pub(crate) fn go_back(tab_id: &str) -> bool {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::go_back(tab_id).is_ok();
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = tab_id;
        false
    }
}

/// Navigates the tab's webview forward in history.
#[cfg(target_os = "windows")]
pub(crate) fn go_forward(tab_id: &str) -> bool {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::go_forward(tab_id).is_ok();
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = tab_id;
        false
    }
}

/// Reloads the tab's webview.
#[cfg(target_os = "windows")]
pub(crate) fn reload(tab_id: &str) -> bool {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::reload(tab_id).is_ok();
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = tab_id;
        false
    }
}

pub(crate) fn close(tab_id: &str) -> Result<(), lxapp::LxAppError> {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::close(tab_id);
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = tab_id;
        unavailable()
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) fn discard(tab_id: &str) -> Result<(), lxapp::LxAppError> {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::discard(tab_id);
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = tab_id;
        unavailable()
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) fn reactivate(tab_id: &str) -> Result<(), lxapp::LxAppError> {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::reactivate(tab_id);
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = tab_id;
        unavailable()
    }
}

pub(crate) fn mark_active(tab_id: &str) {
    #[cfg(feature = "browser-runtime")]
    lingxia_browser::mark_active(tab_id);
    #[cfg(not(feature = "browser-runtime"))]
    let _ = tab_id;
}

pub(crate) fn tab_path(tab_id: &str) -> String {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::tab_path(tab_id);
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = tab_id;
        String::new()
    }
}

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn update_tab(tab_id: &str, current_url: Option<&str>, title: Option<&str>) -> bool {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::update_tab(tab_id, current_url, title);
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = (tab_id, current_url, title);
        false
    }
}

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn download(
    tab_id: &str,
    url: &str,
    user_agent: Option<&str>,
    suggested_filename: Option<&str>,
    source_page_url: Option<&str>,
    cookie: Option<&str>,
) -> Result<(), lxapp::LxAppError> {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::start_download(
        tab_id,
        url,
        user_agent,
        suggested_filename,
        source_page_url,
        cookie,
    );
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = (
            tab_id,
            url,
            user_agent,
            suggested_filename,
            source_page_url,
            cookie,
        );
        unavailable()
    }
}

#[cfg_attr(not(any(target_os = "android", target_env = "ohos")), allow(dead_code))]
pub(crate) fn classify_navigation_json(request_json: &str) -> Option<String> {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::classify_navigation_json(request_json);
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = request_json;
        None
    }
}
