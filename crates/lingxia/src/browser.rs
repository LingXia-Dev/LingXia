/// All access to `lingxia-shell` and `lingxia-browser` goes through this module.
/// When the `shell` feature is disabled every function is a no-op / returns an empty value.

/// Returns a bitmask of host app capabilities.
/// Bit 0 (0x1) = shell (browser, downloads, settings, panels).
pub(crate) fn app_capabilities() -> u32 {
    #[cfg(feature = "shell")]
    return 0x1;
    #[cfg(not(feature = "shell"))]
    0
}

#[cfg(feature = "shell")]
pub(crate) const APP_ID: &str = lingxia_shell::APP_ID;
#[cfg(not(feature = "shell"))]
pub(crate) const APP_ID: &str = "";

pub(crate) fn register_bundled_app() {
    #[cfg(feature = "shell")]
    lingxia_browser::register_bundled_app();
}

pub(crate) fn open_for_app(
    appid: &str,
    session_id: u64,
    url: &str,
    tab_id: Option<&str>,
) -> Result<String, lxapp::LxAppError> {
    #[cfg(feature = "shell")]
    return lingxia_shell::open_for_app(appid, session_id, url, tab_id);
    #[cfg(not(feature = "shell"))]
    {
        let _ = (appid, session_id, url, tab_id);
        Err(lxapp::LxAppError::UnsupportedOperation(
            "browser not available (shell feature disabled)".to_string(),
        ))
    }
}

pub(crate) fn close(tab_id: &str) -> Result<(), lxapp::LxAppError> {
    #[cfg(feature = "shell")]
    return lingxia_shell::close(tab_id);
    #[cfg(not(feature = "shell"))]
    {
        let _ = tab_id;
        Err(lxapp::LxAppError::UnsupportedOperation(
            "browser not available (shell feature disabled)".to_string(),
        ))
    }
}

pub(crate) fn tab_path(tab_id: &str) -> String {
    #[cfg(feature = "shell")]
    return lingxia_shell::tab_path(tab_id);
    #[cfg(not(feature = "shell"))]
    {
        let _ = tab_id;
        String::new()
    }
}

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn update_tab(tab_id: &str, current_url: Option<&str>, title: Option<&str>) -> bool {
    #[cfg(feature = "shell")]
    return lingxia_shell::update_tab(tab_id, current_url, title);
    #[cfg(not(feature = "shell"))]
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
    #[cfg(feature = "shell")]
    return lingxia_shell::download(tab_id, url, user_agent, suggested_filename, source_page_url, cookie);
    #[cfg(not(feature = "shell"))]
    {
        let _ = (tab_id, url, user_agent, suggested_filename, source_page_url, cookie);
        Err(lxapp::LxAppError::UnsupportedOperation(
            "browser not available (shell feature disabled)".to_string(),
        ))
    }
}

pub(crate) fn resolve_input_json(request_json: &str) -> Option<String> {
    #[cfg(feature = "shell")]
    return lingxia_shell::resolve_input_json(request_json);
    #[cfg(not(feature = "shell"))]
    {
        let _ = request_json;
        None
    }
}

#[cfg_attr(not(any(target_os = "android", target_env = "ohos")), allow(dead_code))]
pub(crate) fn classify_navigation_json(request_json: &str) -> Option<String> {
    #[cfg(feature = "shell")]
    return lingxia_shell::classify_navigation_json(request_json);
    #[cfg(not(feature = "shell"))]
    {
        let _ = request_json;
        None
    }
}

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn should_hide_url(raw: &str) -> bool {
    #[cfg(feature = "shell")]
    return lingxia_shell::should_hide_url(raw);
    #[cfg(not(feature = "shell"))]
    {
        let _ = raw;
        false
    }
}

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn panel_item_for_id(panel_id: &str) -> Option<(String, String)> {
    #[cfg(feature = "shell")]
    return lingxia_shell::panel_item_for_id(panel_id);
    #[cfg(not(feature = "shell"))]
    {
        let _ = panel_id;
        None
    }
}

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn panels_config_json() -> Option<String> {
    #[cfg(feature = "shell")]
    return lingxia_shell::panels_config_json();
    #[cfg(not(feature = "shell"))]
    None
}

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn open_panel_lxapp(panel_id: &str, appid: &str, path: &str) {
    #[cfg(feature = "shell")]
    lingxia_shell::open_panel_lxapp(panel_id, appid, path);
    #[cfg(not(feature = "shell"))]
    let _ = (panel_id, appid, path);
}

pub(crate) fn register_builtin() {
    #[cfg(feature = "shell")]
    {
        use std::sync::OnceLock;
        static REGISTERED: OnceLock<()> = OnceLock::new();
        REGISTERED.get_or_init(lingxia_shell::register);
    }
}

pub(crate) fn warmup() {
    #[cfg(feature = "shell")]
    lingxia_shell::warmup();
}
