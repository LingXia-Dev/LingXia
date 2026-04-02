mod runtime;

pub use runtime::{
    BUILTIN_BROWSER_APPID, BrowserAddressAction, BrowserAddressInputContext,
    BrowserAddressInputError, BrowserAddressInputRequest, BrowserAddressInputResponse,
    BrowserAddressInputTrigger, BrowserAddressNavigation, BrowserAddressState,
    BrowserAddressSuggestion, BrowserAddressValueKind, BrowserNavigationPolicyDecision,
    BrowserNavigationPolicyRequest, BrowserNavigationPolicyResponse, BrowserNavigationTarget,
    BrowserTabInfo,
};

pub use lxapp::LxAppError;

#[cfg_attr(not(any(target_os = "android", target_env = "ohos")), allow(dead_code))]
pub(crate) fn classify_navigation_json(request_json: &str) -> Option<String> {
    runtime::handle_browser_navigation_policy_json(request_json)
}

#[doc(hidden)]
pub fn install_tab_page_finished_script(js: impl Into<String>) {
    runtime::install_browser_tab_page_finished_script(js);
}

#[doc(hidden)]
pub fn install_runtime() {
    lxapp::register_page_resolver(runtime::browser_logic_page_path_for_tab_path);
    lingxia_transfer::runtime::register_browser_tab_path_resolver(runtime::browser_tab_path_for_id);
    lingxia_transfer::runtime::register_browser_retry_handler(
        runtime::retry_browser_owned_download,
    );
}

#[doc(hidden)]
pub fn register_internal_page(
    route: impl Into<String>,
    entry_asset: impl Into<String>,
) -> Result<(), LxAppError> {
    runtime::register_browser_internal_page(route, entry_asset)
}

pub fn open(url: &str, tab_id: Option<&str>) -> Result<String, LxAppError> {
    runtime::open_internal_browser_tab(url, tab_id)
}

pub fn open_for_app(
    appid: &str,
    session_id: u64,
    url: &str,
    tab_id: Option<&str>,
) -> Result<String, LxAppError> {
    runtime::open_internal_browser_tab_for_owner(appid, session_id, url, tab_id)
}

pub fn close(tab_id: &str) -> Result<(), LxAppError> {
    runtime::close_browser_tab(tab_id)
}

pub fn tab_path(tab_id: &str) -> String {
    runtime::browser_tab_path_for_id(tab_id)
}

pub fn update_tab(tab_id: &str, current_url: Option<&str>, title: Option<&str>) -> bool {
    runtime::browser_update_tab_info(tab_id, current_url, title)
}

pub fn start_download(
    tab_id: &str,
    url: &str,
    user_agent: Option<&str>,
    suggested_filename: Option<&str>,
    source_page_url: Option<&str>,
    cookie: Option<&str>,
) -> Result<(), LxAppError> {
    runtime::start_native_browser_download(
        tab_id,
        url,
        user_agent,
        suggested_filename,
        source_page_url,
        cookie,
    )
}

#[doc(hidden)]
pub fn register_bundled_app() {
    runtime::register_builtin_browser_asset_bundle();
}

#[doc(hidden)]
pub fn warmup() {
    if let Err(err) = runtime::warmup_builtin_browser_runtime() {
        lxapp::warn!("[InternalBrowser] warmup failed: {}", err);
    }
}
