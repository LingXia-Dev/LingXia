/// Browser runtime access.
///
/// The browser runtime is independent from the standard JS LxApp runtime.
/// `shell-runtime` is one optional UI presentation for that capability.

#[cfg(feature = "browser-runtime")]
pub(crate) const APP_ID: &str = lingxia_browser::BUILTIN_BROWSER_APPID;
#[cfg(not(feature = "browser-runtime"))]
pub(crate) const APP_ID: &str = "";

pub(crate) fn register_bundled_app() {
    #[cfg(feature = "browser-runtime")]
    lingxia_browser::register_bundled_app();
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
        Err(lxapp::LxAppError::UnsupportedOperation(
            "browser not available (browser feature disabled)".to_string(),
        ))
    }
}

pub(crate) fn close(tab_id: &str) -> Result<(), lxapp::LxAppError> {
    #[cfg(feature = "browser-runtime")]
    return lingxia_browser::close(tab_id);
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = tab_id;
        Err(lxapp::LxAppError::UnsupportedOperation(
            "browser not available (browser feature disabled)".to_string(),
        ))
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
        Err(lxapp::LxAppError::UnsupportedOperation(
            "browser not available (browser feature disabled)".to_string(),
        ))
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) fn resolve_input_json(request_json: &str) -> Option<String> {
    #[cfg(feature = "shell-runtime")]
    return lingxia_shell::resolve_input_json(request_json);
    #[cfg(not(feature = "shell-runtime"))]
    {
        let _ = request_json;
        None
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

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn should_hide_url(raw: &str) -> bool {
    #[cfg(feature = "shell-runtime")]
    return lingxia_shell::should_hide_url(raw);
    #[cfg(not(feature = "shell-runtime"))]
    {
        let _ = raw;
        false
    }
}

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn panel_item_for_id(panel_id: &str) -> Option<(String, String)> {
    #[cfg(feature = "shell-runtime")]
    return lingxia_shell::panel_item_for_id(panel_id);
    #[cfg(not(feature = "shell-runtime"))]
    {
        let _ = panel_id;
        None
    }
}

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn panels_config_json() -> Option<String> {
    #[cfg(feature = "shell-runtime")]
    return lingxia_shell::panels_config_json();
    #[cfg(not(feature = "shell-runtime"))]
    None
}

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn open_panel_lxapp(panel_id: &str, appid: &str, path: &str) {
    #[cfg(feature = "shell-runtime")]
    lingxia_shell::open_panel_lxapp(panel_id, appid, path);
    #[cfg(not(feature = "shell-runtime"))]
    let _ = (panel_id, appid, path);
}

pub(crate) fn register_builtin_runtime() {
    #[cfg(all(feature = "browser-runtime", not(feature = "shell-runtime")))]
    {
        use std::sync::OnceLock;
        static REGISTERED: OnceLock<()> = OnceLock::new();
        REGISTERED.get_or_init(lingxia_browser::install_runtime);
    }
    #[cfg(feature = "shell-runtime")]
    lingxia_shell::register_runtime();
}

pub(crate) fn register_builtin_assets() {
    #[cfg(all(feature = "browser-runtime", not(feature = "shell-runtime")))]
    {
        use std::sync::OnceLock;
        static REGISTERED: OnceLock<()> = OnceLock::new();
        REGISTERED.get_or_init(lingxia_browser::register_bundled_app);
    }
    #[cfg(feature = "shell-runtime")]
    lingxia_shell::register_bundled_assets();
}

pub(crate) fn warmup() {
    #[cfg(all(feature = "browser-runtime", not(feature = "shell-runtime")))]
    lingxia_browser::warmup();
    #[cfg(feature = "shell-runtime")]
    lingxia_shell::warmup();
}
