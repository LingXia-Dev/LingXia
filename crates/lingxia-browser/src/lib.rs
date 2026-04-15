mod runtime;

pub use runtime::{
    BUILTIN_BROWSER_APPID, BrowserAddressAction, BrowserAddressInputContext,
    BrowserAddressInputError, BrowserAddressInputRequest, BrowserAddressInputResponse,
    BrowserAddressInputTrigger, BrowserAddressNavigation, BrowserAddressState,
    BrowserAddressSuggestion, BrowserAddressValueKind, BrowserAutomationError,
    BrowserNativeInputHost, BrowserNavigationPolicyDecision, BrowserNavigationPolicyRequest,
    BrowserNavigationPolicyResponse, BrowserNavigationTarget, BrowserTabInfo,
};
use std::sync::Arc;

pub use lxapp::LxAppError;

pub fn classify_navigation(
    request: BrowserNavigationPolicyRequest,
) -> BrowserNavigationPolicyResponse {
    runtime::handle_browser_navigation_policy(request)
}

pub fn classify_navigation_json(request_json: &str) -> Option<String> {
    runtime::handle_browser_navigation_policy_json(request_json)
}

#[doc(hidden)]
pub fn register_startup_page_script(js: impl Into<String>) {
    runtime::register_browser_startup_page_script(js);
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

pub fn tabs() -> Vec<BrowserTabInfo> {
    runtime::browser_tabs()
}

pub fn register_native_input_host(host: Arc<dyn BrowserNativeInputHost>) -> bool {
    runtime::register_native_input_host(host)
}

pub async fn evaluate_javascript(
    tab_id: &str,
    js: &str,
) -> Result<serde_json::Value, BrowserAutomationError> {
    runtime::browser_evaluate_javascript(tab_id, js).await
}

pub async fn click(tab_id: &str, selector: &str) -> Result<(), BrowserAutomationError> {
    runtime::browser_click(tab_id, selector).await
}

pub async fn type_text(
    tab_id: &str,
    selector: &str,
    text: &str,
) -> Result<(), BrowserAutomationError> {
    runtime::browser_type_text(tab_id, selector, text).await
}

pub async fn press(tab_id: &str, key: &str) -> Result<(), BrowserAutomationError> {
    runtime::browser_press(tab_id, key).await
}

pub async fn scroll(tab_id: &str, dx: f64, dy: f64) -> Result<(), BrowserAutomationError> {
    runtime::browser_scroll(tab_id, dx, dy).await
}

pub async fn scroll_to(tab_id: &str, selector: &str) -> Result<(), BrowserAutomationError> {
    runtime::browser_scroll_to(tab_id, selector).await
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
