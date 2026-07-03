mod automation;
mod chooser;
mod downloads;
mod internal_pages;
mod policy;
mod tabs;
mod types;
mod webview;

pub use lingxia_webview::{WebViewCookie, WebViewCookieSameSite, WebViewCookieSetRequest};
pub use policy::{extract_url_scheme, is_lingxia_startup_url};
use std::sync::Arc;
use std::time::Duration;
pub use types::{
    BrowserAddressAction, BrowserAddressInputContext, BrowserAddressInputError,
    BrowserAddressInputRequest, BrowserAddressInputResponse, BrowserAddressInputTrigger,
    BrowserAddressNavigation, BrowserAddressState, BrowserAddressSuggestion,
    BrowserAddressValueKind, BrowserAutomationError, BrowserElementInfo, BrowserNativeInputHost,
    BrowserNavigationPolicyDecision, BrowserNavigationPolicyRequest,
    BrowserNavigationPolicyResponse, BrowserNavigationTarget, BrowserRect, BrowserTabInfo,
    BrowserWaitCondition, BrowserWaitResult,
};

pub use lxapp::LxAppError;

pub const BUILTIN_BROWSER_APPID: &str = "app.lingxia.browser";

pub fn classify_navigation(
    request: BrowserNavigationPolicyRequest,
) -> BrowserNavigationPolicyResponse {
    policy::handle_browser_navigation_policy(request)
}

pub fn classify_navigation_json(request_json: &str) -> Option<String> {
    policy::handle_browser_navigation_policy_json(request_json)
}

#[doc(hidden)]
pub fn register_startup_page_script(js: impl Into<String>) {
    internal_pages::register_browser_startup_page_script(js);
}

#[doc(hidden)]
pub fn install_runtime() {
    lxapp::register_page_resolver(internal_pages::browser_logic_page_path_for_tab_path);
    lingxia_transfer::runtime::register_browser_tab_path_resolver(tabs::browser_tab_path_for_id);
    lingxia_transfer::runtime::register_browser_retry_handler(
        downloads::retry_browser_owned_download,
    );
}

#[doc(hidden)]
pub fn register_internal_page(
    route: impl Into<String>,
    entry_asset: impl Into<String>,
) -> Result<(), LxAppError> {
    internal_pages::register_browser_internal_page(route, entry_asset)
}

pub fn open(url: &str, tab_id: Option<&str>) -> Result<String, LxAppError> {
    tabs::open_internal_browser_tab(url, tab_id)
}

pub fn open_for_app(
    appid: &str,
    session_id: u64,
    url: &str,
    tab_id: Option<&str>,
) -> Result<String, LxAppError> {
    tabs::open_internal_browser_tab_for_owner(appid, session_id, url, tab_id, false, false)
}

/// Open an aside tab in the shared in-app browser: same as [`open_for_app`]
/// except the chrome hides its address bar while this tab is active.
pub fn open_aside_for_app(
    appid: &str,
    session_id: u64,
    url: &str,
    tab_id: Option<&str>,
) -> Result<String, LxAppError> {
    tabs::open_internal_browser_tab_for_owner(appid, session_id, url, tab_id, false, true)
}

/// Open a standalone browser tab (no tab strip) for a docked aside browser.
/// New-window requests from this tab load inline in the same WebView rather
/// than spawning a new main-area tab.
pub fn open_standalone_for_app(
    appid: &str,
    session_id: u64,
    url: &str,
    tab_id: Option<&str>,
) -> Result<String, LxAppError> {
    tabs::open_internal_browser_tab_for_owner(appid, session_id, url, tab_id, true, false)
}

/// Whether `tab_id` was opened as an aside — chrome hides the address bar
/// while such a tab is active.
pub fn tab_is_aside(tab_id: &str) -> bool {
    tabs::is_aside_tab(tab_id)
}

/// Whether `tab_id` is a standalone (no-tab-strip) browser, e.g. a docked
/// aside tab. Standalone tabs are independent of the main tab model, so
/// shells exclude them from their main tab listings.
pub fn tab_is_standalone(tab_id: &str) -> bool {
    tabs::is_standalone_tab(tab_id)
}

pub fn close(tab_id: &str) -> Result<(), LxAppError> {
    tabs::close_browser_tab(tab_id)
}

/// Discard a tab's WebView to free memory while keeping its sidebar entry.
pub fn discard(tab_id: &str) -> Result<(), LxAppError> {
    tabs::discard_browser_tab(tab_id)
}

/// Recreate a discarded tab's WebView, reload its URL, and activate it.
pub fn reactivate(tab_id: &str) -> Result<(), LxAppError> {
    tabs::reactivate_browser_tab(tab_id)
}

/// Sync the Rust-side active tab when the SDK switches to an already-live tab.
pub fn mark_active(tab_id: &str) {
    tabs::mark_browser_tab_active(tab_id)
}

pub fn tabs() -> Vec<BrowserTabInfo> {
    tabs::browser_tabs()
}

pub fn current_tab() -> Option<BrowserTabInfo> {
    tabs::browser_current_tab()
}

pub fn activate(tab_id: &str) -> Result<BrowserTabInfo, BrowserAutomationError> {
    tabs::browser_activate_tab(tab_id)
}

/// Registers a process-wide observer invoked whenever the browser tab set
/// or tab metadata changes: tab opened/closed, active tab switched, or a
/// tab's URL/title updated. Intended for shell UIs that mirror the tab
/// list (e.g. sidebar tab rows); the previous handler (if any) is replaced.
///
/// The callback may fire from arbitrary runtime threads (webview UI
/// threads included) and must not block; query [`tabs`]/[`current_tab`]
/// from it to read the new state.
pub fn set_tabs_changed_handler(handler: Arc<dyn Fn() + Send + Sync>) {
    tabs::set_tabs_changed_handler(handler);
}

/// PNG-encoded favicon of `tab_id`'s current page, if the platform webview
/// reported one (see `WebViewDelegate::on_favicon_changed`). Kept out of
/// [`BrowserTabInfo`] so the serialized tab projection stays byte-free;
/// shell sidebars query it per tab when mirroring the tab list.
pub fn tab_favicon(tab_id: &str) -> Option<Arc<Vec<u8>>> {
    tabs::browser_tab_favicon(tab_id)
}

pub fn register_native_input_host(host: Arc<dyn BrowserNativeInputHost>) -> bool {
    automation::register_native_input_host(host)
}

pub async fn evaluate_javascript(
    tab_id: &str,
    js: &str,
) -> Result<serde_json::Value, BrowserAutomationError> {
    automation::browser_evaluate_javascript(tab_id, js).await
}

pub async fn take_screenshot(tab_id: &str) -> Result<Vec<u8>, BrowserAutomationError> {
    automation::browser_take_screenshot(tab_id).await
}

pub async fn current_url(tab_id: &str) -> Result<Option<String>, BrowserAutomationError> {
    automation::browser_current_url(tab_id).await
}

pub fn reload(tab_id: &str) -> Result<(), BrowserAutomationError> {
    automation::browser_reload(tab_id)
}

pub fn go_back(tab_id: &str) -> Result<(), BrowserAutomationError> {
    automation::browser_go_back(tab_id)
}

pub fn go_forward(tab_id: &str) -> Result<(), BrowserAutomationError> {
    automation::browser_go_forward(tab_id)
}

pub async fn list_cookies(tab_id: &str) -> Result<Vec<WebViewCookie>, BrowserAutomationError> {
    automation::browser_list_cookies(tab_id).await
}

pub async fn list_all_cookies(tab_id: &str) -> Result<Vec<WebViewCookie>, BrowserAutomationError> {
    automation::browser_list_all_cookies(tab_id).await
}

pub async fn set_cookie(
    tab_id: &str,
    request: WebViewCookieSetRequest,
) -> Result<(), BrowserAutomationError> {
    automation::browser_set_cookie(tab_id, request).await
}

pub async fn delete_cookie(
    tab_id: &str,
    name: &str,
    domain: &str,
    path: &str,
) -> Result<(), BrowserAutomationError> {
    automation::browser_delete_cookie(tab_id, name, domain, path).await
}

pub async fn clear_cookies(tab_id: &str) -> Result<(), BrowserAutomationError> {
    automation::browser_clear_cookies(tab_id).await
}

pub async fn query(
    tab_id: &str,
    selector: &str,
) -> Result<BrowserElementInfo, BrowserAutomationError> {
    automation::browser_query(tab_id, selector).await
}

pub async fn query_with_max_text(
    tab_id: &str,
    selector: &str,
    max_text_chars: Option<usize>,
) -> Result<BrowserElementInfo, BrowserAutomationError> {
    automation::browser_query_with_max_text(tab_id, selector, max_text_chars).await
}

pub async fn wait(
    tab_id: &str,
    condition: BrowserWaitCondition,
    timeout: Duration,
) -> Result<BrowserWaitResult, BrowserAutomationError> {
    automation::browser_wait(tab_id, condition, timeout).await
}

pub async fn wait_for_url(
    tab_id: &str,
    url: &str,
    timeout: Duration,
) -> Result<BrowserWaitResult, BrowserAutomationError> {
    automation::browser_wait_for_url(tab_id, url, timeout).await
}

pub async fn wait_for_url_contains(
    tab_id: &str,
    text: &str,
    timeout: Duration,
) -> Result<BrowserWaitResult, BrowserAutomationError> {
    automation::browser_wait_for_url_contains(tab_id, text, timeout).await
}

pub async fn wait_for_navigation(
    tab_id: &str,
    timeout: Duration,
    wait_until_complete: bool,
) -> Result<BrowserWaitResult, BrowserAutomationError> {
    automation::browser_wait_for_navigation(tab_id, timeout, wait_until_complete).await
}

pub async fn click(tab_id: &str, selector: &str) -> Result<(), BrowserAutomationError> {
    automation::browser_click(tab_id, selector).await
}

pub async fn type_text(
    tab_id: &str,
    selector: &str,
    text: &str,
) -> Result<(), BrowserAutomationError> {
    automation::browser_type_text(tab_id, selector, text).await
}

pub async fn fill(tab_id: &str, selector: &str, text: &str) -> Result<(), BrowserAutomationError> {
    automation::browser_fill(tab_id, selector, text).await
}

pub async fn press(tab_id: &str, key: &str) -> Result<(), BrowserAutomationError> {
    automation::browser_press(tab_id, key).await
}

pub async fn scroll(tab_id: &str, dx: f64, dy: f64) -> Result<(), BrowserAutomationError> {
    automation::browser_scroll(tab_id, dx, dy).await
}

pub async fn scroll_to(tab_id: &str, selector: &str) -> Result<(), BrowserAutomationError> {
    automation::browser_scroll_to(tab_id, selector).await
}

pub fn tab_path(tab_id: &str) -> String {
    tabs::browser_tab_path_for_id(tab_id)
}

pub fn update_tab(tab_id: &str, current_url: Option<&str>, title: Option<&str>) -> bool {
    tabs::browser_update_tab_info(tab_id, current_url, title)
}

pub fn start_download(
    tab_id: &str,
    url: &str,
    user_agent: Option<&str>,
    suggested_filename: Option<&str>,
    source_page_url: Option<&str>,
    cookie: Option<&str>,
) -> Result<(), LxAppError> {
    downloads::start_native_browser_download(
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
    tabs::register_builtin_browser_host();
}

#[doc(hidden)]
pub fn warmup() {
    if let Err(err) = internal_pages::warmup_builtin_browser_runtime() {
        lxapp::warn!("[InternalBrowser] warmup failed: {}", err);
    }
}
