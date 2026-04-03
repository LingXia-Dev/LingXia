use std::sync::OnceLock;

pub(crate) const APP_ID: &str = lingxia_shell::APP_ID;

pub(crate) fn resolve_input_json(request_json: &str) -> Option<String> {
    lingxia_shell::resolve_input_json(request_json)
}

#[cfg_attr(not(any(target_os = "android", target_env = "ohos")), allow(dead_code))]
pub(crate) fn classify_navigation_json(request_json: &str) -> Option<String> {
    lingxia_shell::classify_navigation_json(request_json)
}

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn should_hide_url(raw: &str) -> bool {
    lingxia_shell::should_hide_url(raw)
}

pub(crate) fn open_for_app(
    appid: &str,
    session_id: u64,
    url: &str,
    tab_id: Option<&str>,
) -> Result<String, lxapp::LxAppError> {
    lingxia_shell::open_for_app(appid, session_id, url, tab_id)
}

pub(crate) fn close(tab_id: &str) -> Result<(), lxapp::LxAppError> {
    lingxia_shell::close(tab_id)
}

pub(crate) fn tab_path(tab_id: &str) -> String {
    lingxia_shell::tab_path(tab_id)
}

#[cfg_attr(not(any(target_os = "ios", target_os = "macos")), allow(dead_code))]
pub(crate) fn update_tab(tab_id: &str, current_url: Option<&str>, title: Option<&str>) -> bool {
    lingxia_shell::update_tab(tab_id, current_url, title)
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
    lingxia_shell::download(
        tab_id,
        url,
        user_agent,
        suggested_filename,
        source_page_url,
        cookie,
    )
}

pub(crate) fn register_builtin() {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(lingxia_shell::register);
}

pub(crate) fn warmup() {
    lingxia_shell::warmup();
}
