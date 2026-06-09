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

#[cfg(all(feature = "browser-runtime", not(feature = "shell-runtime")))]
pub(crate) fn install_runtime_once() {
    use std::sync::OnceLock;
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(lingxia_browser::install_runtime);
}

#[cfg(all(feature = "browser-runtime", not(feature = "shell-runtime")))]
pub(crate) fn register_bundled_app_once() {
    use std::sync::OnceLock;
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(lingxia_browser::register_bundled_app);
}

#[cfg(all(feature = "browser-runtime", not(feature = "shell-runtime")))]
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

pub(crate) fn close(tab_id: &str) -> Result<(), lxapp::LxAppError> {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::close(tab_id);
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = tab_id;
        unavailable()
    }
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
