//! Internal adapter for shell-specific browser UI behavior.

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn resolve_input_json(request_json: &str) -> Option<String> {
    #[cfg(feature = "browser-shell")]
    return lingxia_browser_shell::resolve_input_json(request_json);
    #[cfg(not(feature = "browser-shell"))]
    {
        let _ = request_json;
        None
    }
}

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn should_hide_url(raw: &str) -> bool {
    #[cfg(feature = "browser-shell")]
    return lingxia_browser_shell::should_hide_url(raw);
    #[cfg(not(feature = "browser-shell"))]
    {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return true;
        }
        let lowered = trimmed.to_ascii_lowercase();
        if lowered.starts_with("lx:")
            || lowered.starts_with("data:")
            || lowered.starts_with("javascript:")
            || lowered.starts_with("blob:")
            || lowered == "about:blank"
        {
            return true;
        }
        let host = lowered
            .strip_prefix("lingxia://")
            .map(|rest| rest.split('/').next().unwrap_or(""))
            .unwrap_or("__not_lingxia__");
        host.is_empty() || host == "newtab"
    }
}

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn panel_item_for_id(panel_id: &str) -> Option<(String, String)> {
    #[cfg(feature = "browser-shell")]
    return lingxia_browser_shell::panel_item_for_id(panel_id);
    #[cfg(not(feature = "browser-shell"))]
    {
        let _ = panel_id;
        None
    }
}

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn panels_config_json() -> Option<String> {
    #[cfg(feature = "browser-shell")]
    return lingxia_browser_shell::panels_config_json();
    #[cfg(not(feature = "browser-shell"))]
    None
}

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn open_panel_lxapp(panel_id: &str, appid: &str, path: &str) {
    #[cfg(feature = "browser-shell")]
    lingxia_browser_shell::open_panel_lxapp(panel_id, appid, path);
    #[cfg(not(feature = "browser-shell"))]
    let _ = (panel_id, appid, path);
}

#[cfg(feature = "browser-shell")]
pub(crate) fn register_runtime() {
    lingxia_browser_shell::register_runtime();
}

#[cfg(feature = "browser-shell")]
pub(crate) fn register_bundled_assets() {
    lingxia_browser_shell::register_bundled_assets();
}

#[cfg(feature = "browser-shell")]
pub(crate) fn warmup() {
    lingxia_browser_shell::warmup();
}
