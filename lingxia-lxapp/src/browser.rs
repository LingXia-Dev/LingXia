use crate::bridge::IncomingMessage;
use crate::page::Page;
use crate::{LxApp, LxAppError, publish_app_event};
use http::{Response, StatusCode};
use lingxia_platform::traits::app_runtime::{AppRuntime, OpenUrlRequest, OpenUrlTarget};
use lingxia_webview::runtime::{
    destroy_webview as destroy_managed_webview, find_webview as find_managed_webview,
};
use lingxia_webview::{
    DownloadRequest, LogLevel, NavigationPolicy, NewWindowPolicy, WebTag, WebView, WebViewBuilder,
    WebViewController, WebViewDelegate, WebViewSession,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use tokio::sync::{broadcast, oneshot};
use uuid::Uuid;

use crate::download;
use crate::download::{DownloadEvent, DownloadRecord, DownloadStatus, DownloadsSnapshot};

pub const BUILTIN_BROWSER_APPID: &str = "app.lingxia.browser";
const INTERNAL_TAB_PATH_PREFIX: &str = "/tabs/";
const DEFAULT_BROWSER_PREFERRED_SCHEME: &str = "https";
const LINGXIA_SCHEME: &str = "lingxia";
const BROWSER_IN_WEBVIEW_SCHEMES: &[&str] = &["http", "https", "lx", "lingxia"];
const BROWSER_NON_EXTERNAL_SCHEMES: &[&str] = &["about", "data", "blob", "javascript", "file"];

// Internal browser tab model:
// 1) All tabs are hosted by the built-in browser lxapp (BUILTIN_BROWSER_APPID).
// 2) One tab id maps to one page path: /tabs/{tab_id}.
// 3) One tab owns one managed WebView instance lifecycle (reopen same tab id reuses it).

pub(crate) fn normalize_browser_target_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= "http://".len() && trimmed[..7].eq_ignore_ascii_case("http://") {
        format!("https://{}", &trimmed[7..])
    } else {
        trimmed.to_string()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BrowserAddressInputTrigger {
    Edit,
    #[default]
    Submit,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserAddressAction {
    Navigate,
    Suggest,
    Reject,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserAddressValueKind {
    Empty,
    Url,
    SearchQuery,
    Invalid,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserNavigationTarget {
    CurrentTab,
    NewTab,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserNavigationPolicyDecision {
    InWebview,
    OpenExternal,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserNavigationPolicyRequest {
    pub raw_url: String,
    #[serde(default)]
    pub has_user_gesture: bool,
    #[serde(default = "default_true")]
    pub is_main_frame: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserNavigationPolicyResponse {
    pub decision: BrowserNavigationPolicyDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrowserAddressInputContext {
    #[serde(default)]
    pub preferred_scheme: Option<String>,
    #[serde(default)]
    pub current_url: Option<String>,
    #[serde(default)]
    pub tab_id: Option<String>,
    #[serde(default)]
    pub allow_search_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserAddressInputRequest {
    pub raw_input: String,
    #[serde(default)]
    pub trigger: BrowserAddressInputTrigger,
    #[serde(default)]
    pub context: BrowserAddressInputContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserAddressState {
    pub raw_input: String,
    pub normalized_input: String,
    pub display_text: String,
    pub value_kind: BrowserAddressValueKind,
    pub canonical_url: Option<String>,
    pub inferred_scheme: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserAddressNavigation {
    pub url: String,
    pub target: BrowserNavigationTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserAddressSuggestion {
    pub kind: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub fill_text: String,
    pub navigation: Option<BrowserAddressNavigation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserAddressInputError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserAddressInputResponse {
    pub action: BrowserAddressAction,
    pub state: BrowserAddressState,
    pub navigation: Option<BrowserAddressNavigation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestions: Option<Vec<BrowserAddressSuggestion>>,
    pub error: Option<BrowserAddressInputError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserTabInfo {
    pub tab_id: String,
    pub path: String,
    pub session_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
struct BrowserUrlResolution {
    url: String,
    inferred_scheme: Option<String>,
}

fn default_true() -> bool {
    true
}

fn normalize_browser_preferred_scheme(raw: Option<&str>) -> String {
    let candidate = raw
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_BROWSER_PREFERRED_SCHEME);
    let lowered = candidate.to_ascii_lowercase();
    match lowered.as_str() {
        "http" | "https" => lowered,
        _ => DEFAULT_BROWSER_PREFERRED_SCHEME.to_string(),
    }
}

fn extract_host_from_authority(authority: &str) -> Option<&str> {
    let authority = authority.rsplit('@').next()?;
    if authority.is_empty() {
        return None;
    }

    if let Some(rest) = authority.strip_prefix('[') {
        let end = rest.find(']')?;
        let host = &rest[..end];
        if host.is_empty() {
            return None;
        }
        let suffix = &rest[end + 1..];
        if suffix.is_empty() {
            return Some(host);
        }
        if !suffix.starts_with(':') || suffix.len() == 1 {
            return None;
        }
        if suffix[1..].chars().all(|c| c.is_ascii_digit()) {
            return Some(host);
        }
        return None;
    }

    let host = match authority.rsplit_once(':') {
        Some((host, port))
            if !host.is_empty() && !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) =>
        {
            host
        }
        Some(_) => return None,
        _ => authority,
    };

    if host.is_empty() { None } else { Some(host) }
}

fn is_probable_web_host_without_scheme(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    if host.contains('.') {
        return true;
    }
    host.parse::<IpAddr>().is_ok()
}

fn resolve_browser_http_url(raw: &str, preferred_scheme: &str) -> Option<BrowserUrlResolution> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().any(|c| c.is_whitespace()) {
        return None;
    }

    let (candidate, inferred_scheme) = if trimmed.contains("://") {
        (trimmed.to_string(), None)
    } else {
        (
            format!("{preferred_scheme}://{trimmed}"),
            Some(preferred_scheme.to_string()),
        )
    };

    let (scheme_raw, rest) = candidate.split_once("://")?;
    let scheme = scheme_raw.to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https") {
        return None;
    }
    if rest.is_empty() || rest.starts_with('/') || rest.starts_with('?') || rest.starts_with('#') {
        return None;
    }

    let authority = rest
        .split(|c| matches!(c, '/' | '?' | '#'))
        .next()
        .unwrap_or_default();
    let host = extract_host_from_authority(authority)?;
    if host.trim().is_empty() || host.chars().any(|c| c.is_whitespace()) {
        return None;
    }
    if inferred_scheme.is_some() && !is_probable_web_host_without_scheme(host) {
        return None;
    }

    Some(BrowserUrlResolution {
        url: format!("{scheme}://{rest}"),
        inferred_scheme,
    })
}

fn classify_browser_address_value(raw: &str) -> BrowserAddressValueKind {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        BrowserAddressValueKind::Empty
    } else if trimmed.contains(char::is_whitespace) || !trimmed.contains("://") {
        BrowserAddressValueKind::SearchQuery
    } else {
        BrowserAddressValueKind::Invalid
    }
}

pub(crate) fn extract_url_scheme(raw: &str) -> Option<String> {
    let (scheme, _) = raw.split_once(':')?;
    if scheme.is_empty() {
        return None;
    }
    let is_valid = scheme
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    if !is_valid {
        return None;
    }
    Some(scheme.to_ascii_lowercase())
}

/// Whether a `lingxia://` URL maps to the startup/newtab page or another internal browser page.
///
/// - `lingxia://newtab` (or bare `lingxia://`) → `StartupPage`
/// - `lingxia://downloads`, `lingxia://settings`, … → `InternalPage`
///
/// Returns `None` if `url` is not a `lingxia://` URL.
fn is_lingxia_startup_url(url: &str) -> Option<bool> {
    if extract_url_scheme(url).as_deref() != Some(LINGXIA_SCHEME) {
        return None;
    }
    let host = url
        .splitn(2, "://")
        .nth(1)
        .unwrap_or("")
        .split('/')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    Some(host.is_empty() || host == "newtab")
}

fn scheme_in_list(scheme: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(scheme))
}

fn browser_policy_response(
    decision: BrowserNavigationPolicyDecision,
    reason: Option<&str>,
) -> BrowserNavigationPolicyResponse {
    BrowserNavigationPolicyResponse {
        decision,
        reason: reason.map(str::to_string),
    }
}

/// Classify browser navigation requests into:
/// - `in_webview`: keep loading in current webview.
/// - `open_external`: cancel in-webview load and open externally.
/// - `deny`: cancel navigation.
///
/// Security model:
/// - `http/https/lx` stay in webview.
/// - Potential external schemes require user gesture + main-frame navigation.
/// - Non-external internal schemes (`javascript:`, `data:`, etc.) are denied.
pub(crate) fn handle_browser_navigation_policy(
    request: BrowserNavigationPolicyRequest,
) -> BrowserNavigationPolicyResponse {
    let trimmed = request.raw_url.trim();
    if trimmed.is_empty() {
        return browser_policy_response(BrowserNavigationPolicyDecision::Deny, Some("empty"));
    }

    if trimmed.chars().any(|c| c.is_whitespace()) {
        return browser_policy_response(
            BrowserNavigationPolicyDecision::Deny,
            Some("whitespace_url"),
        );
    }

    let Some(scheme) = extract_url_scheme(trimmed) else {
        return browser_policy_response(
            BrowserNavigationPolicyDecision::Deny,
            Some("missing_scheme"),
        );
    };

    if scheme_in_list(&scheme, BROWSER_IN_WEBVIEW_SCHEMES) {
        return browser_policy_response(BrowserNavigationPolicyDecision::InWebview, None);
    }

    if scheme_in_list(&scheme, BROWSER_NON_EXTERNAL_SCHEMES) {
        return browser_policy_response(
            BrowserNavigationPolicyDecision::Deny,
            Some("non_external_scheme"),
        );
    }

    if !request.is_main_frame {
        return browser_policy_response(
            BrowserNavigationPolicyDecision::Deny,
            Some("non_main_frame_external"),
        );
    }

    if !request.has_user_gesture {
        return browser_policy_response(
            BrowserNavigationPolicyDecision::Deny,
            Some("gesture_required"),
        );
    }

    browser_policy_response(BrowserNavigationPolicyDecision::OpenExternal, None)
}

pub fn handle_browser_navigation_policy_json(request_json: &str) -> Option<String> {
    let request: BrowserNavigationPolicyRequest = serde_json::from_str(request_json).ok()?;
    serde_json::to_string(&handle_browser_navigation_policy(request)).ok()
}

/// Handle browser address-bar input with a minimal synchronous pipeline.
///
/// Current scope:
/// - `submit` resolves navigable http/https-style input into a current-tab navigation target.
/// - `edit` only returns the `Suggest` action shape; real autocomplete/search suggestions are not wired yet.
/// - `new_tab`, async suggestion providers, and search fallback remain protocol-level extension points.
pub fn handle_browser_address_input(
    request: BrowserAddressInputRequest,
) -> BrowserAddressInputResponse {
    let preferred_scheme =
        normalize_browser_preferred_scheme(request.context.preferred_scheme.as_deref());
    let trimmed = request.raw_input.trim();

    // Handle `lingxia://` browser-internal URLs (e.g. lingxia://newtab, lingxia://downloads).
    if extract_url_scheme(trimmed).as_deref() == Some(LINGXIA_SCHEME) {
        let url = trimmed.to_string();
        let action = match request.trigger {
            BrowserAddressInputTrigger::Submit => BrowserAddressAction::Navigate,
            BrowserAddressInputTrigger::Edit => BrowserAddressAction::Suggest,
        };
        let state = BrowserAddressState {
            raw_input: request.raw_input,
            normalized_input: url.clone(),
            display_text: url.clone(),
            value_kind: BrowserAddressValueKind::Url,
            canonical_url: Some(url.clone()),
            inferred_scheme: None,
        };
        let navigation = BrowserAddressNavigation {
            url,
            target: BrowserNavigationTarget::CurrentTab,
        };
        return BrowserAddressInputResponse {
            action,
            state,
            navigation: matches!(action, BrowserAddressAction::Navigate).then_some(navigation),
            suggestions: None,
            error: None,
        };
    }

    if let Some(resolved) = resolve_browser_http_url(trimmed, &preferred_scheme) {
        let BrowserUrlResolution {
            url: resolved_url,
            inferred_scheme,
        } = resolved;

        let action = match request.trigger {
            BrowserAddressInputTrigger::Submit => BrowserAddressAction::Navigate,
            BrowserAddressInputTrigger::Edit => BrowserAddressAction::Suggest,
        };

        let display_text = resolved_url.clone();
        let state = BrowserAddressState {
            raw_input: request.raw_input,
            normalized_input: display_text.clone(),
            display_text,
            value_kind: BrowserAddressValueKind::Url,
            canonical_url: Some(resolved_url.clone()),
            inferred_scheme,
        };

        let navigation = BrowserAddressNavigation {
            url: resolved_url,
            target: BrowserNavigationTarget::CurrentTab,
        };

        return BrowserAddressInputResponse {
            action,
            state,
            navigation: matches!(action, BrowserAddressAction::Navigate).then_some(navigation),
            suggestions: None,
            error: None,
        };
    }

    let value_kind = classify_browser_address_value(trimmed);
    let normalized = trimmed.to_string();
    let state = BrowserAddressState {
        raw_input: request.raw_input,
        display_text: normalized.clone(),
        normalized_input: normalized,
        value_kind,
        canonical_url: None,
        inferred_scheme: None,
    };

    let should_suggest = matches!(request.trigger, BrowserAddressInputTrigger::Edit)
        || (matches!(value_kind, BrowserAddressValueKind::SearchQuery)
            && request.context.allow_search_fallback);

    if should_suggest {
        return BrowserAddressInputResponse {
            action: BrowserAddressAction::Suggest,
            state,
            navigation: None,
            suggestions: None,
            error: None,
        };
    }

    let error = match value_kind {
        BrowserAddressValueKind::Empty => BrowserAddressInputError {
            code: "empty_input".to_string(),
            message: "Address input is empty".to_string(),
        },
        BrowserAddressValueKind::SearchQuery => BrowserAddressInputError {
            code: "search_fallback_unavailable".to_string(),
            message: "Search fallback is not enabled for this browser input".to_string(),
        },
        BrowserAddressValueKind::Invalid | BrowserAddressValueKind::Url => {
            BrowserAddressInputError {
                code: "invalid_url".to_string(),
                message: "Address input is not a supported URL".to_string(),
            }
        }
    };

    BrowserAddressInputResponse {
        action: BrowserAddressAction::Reject,
        state,
        navigation: None,
        suggestions: None,
        error: Some(error),
    }
}

pub fn handle_browser_address_input_json(request_json: &str) -> Option<String> {
    let request: BrowserAddressInputRequest = serde_json::from_str(request_json).ok()?;
    serde_json::to_string(&handle_browser_address_input(request)).ok()
}

/// Return `true` if the URL should be hidden from the browser address bar.
///
/// Centralises all platform address-bar filtering in one place.
/// Platforms call this and, if `true`, show an empty address field instead.
pub fn browser_url_is_hidden(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return true;
    }
    let lowered = trimmed.to_ascii_lowercase();
    // Internal transport scheme and non-displayable URL types.
    if lowered.starts_with("lx:")
        || lowered.starts_with("data:")
        || lowered.starts_with("javascript:")
        || lowered.starts_with("blob:")
        || lowered == "about:blank"
    {
        return true;
    }
    // Browser-managed pages (newtab). Other lingxia:// pages are shown as-is.
    matches!(is_lingxia_startup_url(trimmed), Some(true))
}

#[derive(Clone)]
struct BrowserTabState {
    session_id: u64,
    /// Monotonic token to identify the current create lifecycle of this tab.
    /// Used to ignore stale async callbacks when tab gets recreated quickly.
    create_token: u64,
    /// URL queued for loading while WebView creation is in-flight.
    pending_url: Option<String>,
    current_url: Option<String>,
    title: Option<String>,
}

struct BrowserState {
    // tab_id -> tab lifecycle state (single WebView lifecycle per tab_id)
    tabs: HashMap<String, BrowserTabState>,
}

static BROWSER_STATE: OnceLock<Mutex<BrowserState>> = OnceLock::new();
static BROWSER_CREATE_TOKEN: AtomicU64 = AtomicU64::new(1);
static BROWSER_LOAD_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
static BROWSER_STARTUP_PAGE_INIT_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

fn lock_state() -> MutexGuard<'static, BrowserState> {
    BROWSER_STATE
        .get_or_init(|| {
            Mutex::new(BrowserState {
                tabs: HashMap::new(),
            })
        })
        .lock()
        .unwrap_or_else(|e| {
            crate::warn!("[InternalBrowser] recovered poisoned browser state mutex");
            e.into_inner()
        })
}

fn sanitize_tab_id(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

fn generate_tab_id() -> String {
    Uuid::new_v4().to_string()
}

fn next_browser_create_token() -> u64 {
    BROWSER_CREATE_TOKEN.fetch_add(1, Ordering::Relaxed)
}

pub fn set_download_dir(lxapp: &LxApp, path: impl Into<PathBuf>) -> Result<(), LxAppError> {
    let path = path.into();
    if path.as_os_str().is_empty() {
        return Err(LxAppError::InvalidParameter(
            "download directory cannot be empty".to_string(),
        ));
    }
    crate::settings::set_download_dir(&lxapp.app_data_dir(), Some(path))
}

pub fn reset_download_dir(lxapp: &LxApp) -> Result<(), LxAppError> {
    crate::settings::set_download_dir(&lxapp.app_data_dir(), None::<&Path>)
}

pub fn download_dir(lxapp: &LxApp) -> Result<PathBuf, LxAppError> {
    Ok(crate::download::manager::download_root(
        &lxapp.app_data_dir(),
    ))
}

fn publish_browser_download_event(event_name: &str, payload: serde_json::Value) {
    let payload_str = Some(payload.to_string());
    let _ = publish_app_event(BUILTIN_BROWSER_APPID, event_name, payload_str);
}

// ---------------------------------------------------------------------------
// Browser startup page bridge: delegate + headless page setup
// ---------------------------------------------------------------------------

/// WebView delegate for browser tab WebViews.
///
/// All tab WebViews share a single headless startup Page (and its PageSvc).
/// This delegate routes postMessage, page-started, and page-finished events
/// from the currently active tab WebView to that shared startup Page.
struct BrowserTabDelegate {
    appid: String,
    page_path: String,
}

impl WebViewDelegate for BrowserTabDelegate {
    fn on_page_started(&self) {
        if let Some(browser) = crate::lxapp::try_get(&self.appid) {
            if let Some(page) = browser.get_page(&self.page_path) {
                page.notify_page_started();
            }
        }
    }

    fn on_page_finished(&self) {
        if let Some(browser) = crate::lxapp::try_get(&self.appid) {
            if let Some(page) = browser.get_page(&self.page_path) {
                page.notify_page_finished();
            }
        }
    }

    fn handle_post_message(&self, msg: String) {
        match IncomingMessage::from_json_str(&msg) {
            Ok(incoming) => {
                if let Some(browser) = crate::lxapp::try_get(&self.appid) {
                    if let Some(page) = browser.get_page(&self.page_path) {
                        if let Err(e) = page.bridge().handle_incoming(&page, Arc::new(incoming)) {
                            crate::warn!(
                                "[InternalBrowser] Failed to handle bridge message: {}",
                                e
                            );
                        }
                    } else {
                        crate::warn!(
                            "[InternalBrowser] Failed to handle bridge message: page not found {}",
                            self.page_path
                        );
                    }
                }
            }
            Err(e) => {
                crate::warn!("[InternalBrowser] Invalid postMessage JSON: {}", e);
            }
        }
    }

    fn log(&self, level: LogLevel, message: &str) {
        let log_level = match level {
            LogLevel::Error => crate::log::LogLevel::Error,
            LogLevel::Warn => crate::log::LogLevel::Warn,
            LogLevel::Info => crate::log::LogLevel::Info,
            LogLevel::Debug | LogLevel::Verbose => crate::log::LogLevel::Debug,
        };
        crate::log::LogBuilder::new(crate::log::LogTag::WebViewConsole, message)
            .with_level(log_level)
            .with_path(&self.page_path)
            .with_appid(self.appid.clone());
    }
}

/// Ensure the browser lxapp has a headless startup Page + a live PageSvc.
///
/// Idempotent: if the page already exists in the browser lxapp's page map, returns it directly.
/// Otherwise creates a headless Page (nonce, no WebView), registers it, starts the AppSvc,
/// and asynchronously awaits the PageSvc ack before signalling the page as "ready".
fn ensure_browser_startup_page(browser: &Arc<LxApp>) -> Result<Page, LxAppError> {
    let startup_path = browser.config.get_initial_route();

    // Return existing page if already registered (idempotent).
    if let Some(page) = browser.get_page(&startup_path) {
        return Ok(page);
    }

    // Serialize one-time startup page initialization to avoid duplicate CreatePage races.
    let _startup_guard = BROWSER_STARTUP_PAGE_INIT_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    // Another task may have finished initialization while we were waiting on the lock.
    if let Some(page) = browser.get_page(&startup_path) {
        return Ok(page);
    }

    // Ensure the JS app service worker is running for this browser lxapp.
    if let Err(e) = browser.executor.create_app_svc(browser.clone_arc()) {
        crate::warn!("[InternalBrowser] Failed to start app service: {}", e);
    }

    // Create a headless Page — nonce allocated, no WebView yet.
    let page = Page::new_headless(
        BUILTIN_BROWSER_APPID.to_string(),
        startup_path.clone(),
        browser,
    );
    browser.register_page(page.clone());

    // Request PageSvc creation; wait for the ack and then mark the page ready.
    let (ack_tx, ack_rx) = oneshot::channel::<()>();
    browser
        .executor
        .create_page_svc_with_ack(browser.clone_arc(), startup_path, ack_tx)?;

    let page_clone = page.clone();
    if let Err(e) = rong::bg::spawn(async move {
        let result = ack_rx.await.map_err(|e| e.to_string());
        page_clone.mark_webview_ready(result);
    }) {
        crate::warn!(
            "[InternalBrowser] Failed to spawn startup page ack task: {}",
            e
        );
        page.mark_webview_ready(Err(e.to_string()));
    }

    Ok(page)
}

fn ensure_browser_tab_page(browser: &Arc<LxApp>, path: &str) -> Result<Page, LxAppError> {
    if let Some(page) = browser.get_page(path) {
        return Ok(page);
    }

    let page = Page::new_headless(BUILTIN_BROWSER_APPID.to_string(), path.to_string(), browser);
    browser.register_page(page.clone());

    let (ack_tx, ack_rx) = oneshot::channel::<()>();
    browser
        .executor
        .create_page_svc_with_ack(browser.clone_arc(), path.to_string(), ack_tx)?;

    let page_clone = page.clone();
    if let Err(e) = rong::bg::spawn(async move {
        let result = ack_rx.await.map_err(|err| err.to_string());
        page_clone.mark_webview_ready(result);
    }) {
        crate::warn!("[InternalBrowser] Failed to spawn tab page ack task: {}", e);
        page.mark_webview_ready(Err(e.to_string()));
    }

    Ok(page)
}

/// Attach the given tab WebView to its headless page and load a lingxia:// URL into it.
/// Waits for the PageSvc to be ready first.
///
/// `page_url`: the `lx://` URL to load. `None` loads the default startup/newtab page;
/// `Some(url)` loads a specific internal browser page (e.g. `lx://lxapp/.../downloads`).
async fn browser_attach_tab_page(
    webview: Arc<WebView>,
    page_path: &str,
    tab_id: &str,
    page_url: Option<&str>,
) -> Result<(), LxAppError> {
    let browser = ensure_browser_lxapp()?;
    let page = browser
        .get_page(page_path)
        .ok_or_else(|| LxAppError::Runtime(format!("browser tab page not found: {}", page_path)))?;

    // Wait until PageSvc signals ready (ack from JS worker).
    if let Err(e) = page.wait_webview_ready().await {
        crate::warn!(
            "[InternalBrowser] Tab PageSvc not ready for tab {}: {}",
            tab_id,
            e
        );
    }

    // Attach this tab's WebView so bridge responses are delivered here.
    page.attach_webview(webview.clone());

    // Load the requested URL (or `lingxia://newtab` for the default startup page).
    let url_to_load = page_url
        .map(|u| u.to_string())
        .unwrap_or_else(|| format!("{}://newtab", LINGXIA_SCHEME));
    webview
        .load_url(&url_to_load)
        .map_err(|e| LxAppError::WebView(e.to_string()))
}

// ---------------------------------------------------------------------------
// WebView helpers — thin wrappers around lingxia-webview cross-platform API
// ---------------------------------------------------------------------------

fn browser_webtag(path: &str, session_id: u64) -> WebTag {
    WebTag::new(BUILTIN_BROWSER_APPID, path, Some(session_id))
}

fn browser_create_webview(
    path: &str,
    session_id: u64,
    tab_id: &str,
    create_token: u64,
) -> Result<(), LxAppError> {
    let webtag = browser_webtag(path, session_id);
    let tab_id_for_download = tab_id.to_string();
    let browser_owner = ensure_browser_lxapp()?;

    // Ensure the JS worker and browser startup page exist before creating the tab WebView.
    ensure_browser_startup_page(&browser_owner)?;
    let tab_page = ensure_browser_tab_page(&browser_owner, path)?;

    let startup_path = browser_owner.config.get_initial_route();
    let owner_for_lx = browser_owner.clone();
    let owner_for_lingxia = browser_owner.clone();
    let startup_path_for_lingxia = startup_path.clone();
    let tab_page_for_lx = tab_page.clone();
    let tab_page_for_lingxia = tab_page.clone();
    let runtime_for_nav = browser_owner.runtime.clone();
    let owner_appid_for_nav = browser_owner.appid.clone();
    let owner_session_for_nav = browser_owner.session_id();
    let runtime_for_new_window = browser_owner.runtime.clone();
    let owner_appid_for_new_window = browser_owner.appid.clone();
    let owner_session_for_new_window = browser_owner.session_id();
    let owner_for_download = browser_owner.clone();
    let session = WebViewBuilder::browser(webtag)
        .delegate(Arc::new(BrowserTabDelegate {
            appid: BUILTIN_BROWSER_APPID.to_string(),
            page_path: path.to_string(),
        }))
        .on_scheme("lx", move |req| {
            let owner = owner_for_lx.clone();
            let page = tab_page_for_lx.clone();
            async move { owner.lingxia_handler(&page, req).into() }
        })
        .on_scheme(LINGXIA_SCHEME, move |req| {
            let owner = owner_for_lingxia.clone();
            let page = tab_page_for_lingxia.clone();
            let startup_path = startup_path_for_lingxia.clone();
            async move {
                // Map the `lingxia://` host to the browser page path.
                // `lingxia://newtab` → startup path; `lingxia://X` → `/X`.
                let host = req.uri().host().unwrap_or("").to_ascii_lowercase();
                let page_path = if host.is_empty() || host == "newtab" {
                    startup_path.clone()
                } else {
                    format!("pages/{host}/index.html")
                };
                // Serve page HTML (with bridge nonce) for the document root.
                let req_path = req.uri().path();
                if req_path == "/" || req_path.is_empty() {
                    let nonce = page.bridge_nonce();
                    let html = owner.generate_page_html(&page_path, nonce.as_deref());
                    let response = Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "text/html; charset=utf-8")
                        .header("Access-Control-Allow-Origin", "null")
                        .body(())
                        .unwrap_or_else(|_| {
                            Response::builder()
                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                .body(())
                                .expect("Failed to build fallback lingxia response")
                        });
                    let (parts, _) = response.into_parts();
                    return Some((parts, html).into()).into();
                }
                // Sub-paths (assets, API) fall through to the standard handler.
                owner.lingxia_handler(&page, req).into()
            }
        })
        .on_navigation(move |url| {
            // Keep internal lx:// and lingxia:// browser pages inside this WebView.
            if matches!(extract_url_scheme(url).as_deref(), Some("lx" | "lingxia")) {
                return NavigationPolicy::Allow;
            }
            // Android callback currently only provides URL string, so user-gesture/main-frame
            // metadata is unavailable here. Keep web links in-webview and dispatch custom
            // schemes to host runtime for OS handler resolution.
            let decision = handle_browser_navigation_policy(BrowserNavigationPolicyRequest {
                raw_url: url.to_string(),
                has_user_gesture: true,
                is_main_frame: true,
            });
            match decision.decision {
                BrowserNavigationPolicyDecision::InWebview => NavigationPolicy::Allow,
                BrowserNavigationPolicyDecision::OpenExternal => {
                    let _ = runtime_for_nav.open_url(OpenUrlRequest {
                        owner_appid: owner_appid_for_nav.clone(),
                        owner_session_id: owner_session_for_nav,
                        url: url.to_string(),
                        target: OpenUrlTarget::External,
                    });
                    NavigationPolicy::Cancel
                }
                BrowserNavigationPolicyDecision::Deny => NavigationPolicy::Cancel,
            }
        })
        .on_new_window(move |url| {
            let normalized = normalize_browser_target_url(url);
            let _ = runtime_for_new_window.open_url(OpenUrlRequest {
                owner_appid: owner_appid_for_new_window.clone(),
                owner_session_id: owner_session_for_new_window,
                url: normalized,
                target: OpenUrlTarget::NewBrowserTab,
            });
            NewWindowPolicy::Cancel
        })
        .on_download(move |request| {
            let tab_id = tab_id_for_download.clone();
            let owner = owner_for_download.clone();
            if let Err(e) = rong::bg::spawn(async move {
                browser_download_resource(owner, tab_id, request).await;
            }) {
                crate::warn!("[InternalBrowser] spawn download task failed: {}", e);
            }
        })
        .create();
    let path_owned = path.to_string();
    let tab_id_owned = tab_id.to_string();

    if let Err(e) = rong::bg::spawn(async move {
        browser_on_webview_ready(path_owned, session_id, tab_id_owned, create_token, session).await;
    }) {
        return Err(LxAppError::Runtime(format!(
            "failed to spawn browser webview ready task: {}",
            e
        )));
    }
    Ok(())
}

async fn browser_on_webview_ready(
    path: String,
    session_id: u64,
    tab_id: String,
    create_token: u64,
    session: WebViewSession,
) {
    let webview = match session.wait_ready().await {
        Ok(webview) => webview,
        Err(e) => {
            crate::warn!(
                "[InternalBrowser] Failed to create webview for tab {}: {}",
                tab_id,
                e
            );
            browser_remove_tab_if_token_matches(&tab_id, session_id, create_token);
            return;
        }
    };
    let tab_state = browser_tab_create_state(&tab_id, session_id, create_token);
    match tab_state {
        TabCreateState::Missing => {
            // Tab was closed while creation was in-flight.
            browser_destroy_webview(&path, session_id);
            return;
        }
        TabCreateState::Stale => {
            // A newer create lifecycle already took ownership of this tab id.
            // Destroy the orphaned webview from this old create cycle.
            browser_destroy_webview(&path, session_id);
            return;
        }
        TabCreateState::Active { pending_url } => {
            if let Some(url) = pending_url {
                // Internal browser pages (`lingxia://X`) need the startup bridge attached
                // so they can communicate with the JS app service worker.
                let is_browser_internal =
                    extract_url_scheme(&url).as_deref() == Some(LINGXIA_SCHEME);
                if is_browser_internal {
                    if let Err(e) =
                        browser_attach_tab_page(webview.clone(), &path, &tab_id, Some(&url)).await
                    {
                        crate::warn!(
                            "[InternalBrowser] Failed to attach startup page for internal tab {}: {}",
                            tab_id,
                            e
                        );
                        let _ = webview.load_url("about:blank");
                    }
                } else {
                    // Direct URL load — no bridge handshake needed, just navigate.
                    if let Err(e) = webview.load_url(&url) {
                        crate::warn!(
                            "[InternalBrowser] Failed to load URL for tab {}: {}",
                            tab_id,
                            e
                        );
                    }
                }
            } else {
                // Startup page: attach WebView to shared startup Page, then load with nonce.
                if let Err(e) = browser_attach_tab_page(webview.clone(), &path, &tab_id, None).await
                {
                    crate::warn!(
                        "[InternalBrowser] Failed to load startup page for tab {}: {}",
                        tab_id,
                        e
                    );
                    let _ = webview.load_url("about:blank");
                }
            }
            browser_clear_pending_if_token_matches(&tab_id, session_id, create_token);
        }
    }
}

#[derive(Debug)]
enum TabCreateState {
    Active { pending_url: Option<String> },
    Missing,
    Stale,
}

fn browser_tab_create_state(tab_id: &str, session_id: u64, create_token: u64) -> TabCreateState {
    let state = lock_state();
    match state.tabs.get(tab_id) {
        Some(tab) if tab.session_id == session_id && tab.create_token == create_token => {
            TabCreateState::Active {
                pending_url: tab.pending_url.clone(),
            }
        }
        Some(_) => TabCreateState::Stale,
        None => TabCreateState::Missing,
    }
}

fn browser_remove_tab_if_token_matches(tab_id: &str, session_id: u64, create_token: u64) {
    let mut state = lock_state();
    let should_remove = state
        .tabs
        .get(tab_id)
        .map(|tab| tab.session_id == session_id && tab.create_token == create_token)
        .unwrap_or(false);
    if should_remove {
        state.tabs.remove(tab_id);
    }
}

fn browser_clear_pending_if_token_matches(tab_id: &str, session_id: u64, create_token: u64) {
    let mut state = lock_state();
    if let Some(tab) = state.tabs.get_mut(tab_id)
        && tab.session_id == session_id
        && tab.create_token == create_token
    {
        tab.pending_url = None;
    }
}

fn browser_find_webview(path: &str, session_id: u64) -> Result<Arc<WebView>, LxAppError> {
    let webtag = browser_webtag(path, session_id);
    find_managed_webview(&webtag).ok_or_else(|| {
        LxAppError::ResourceNotFound(format!("browser webview not found: {}", webtag.as_str()))
    })
}

fn browser_load_url(path: &str, session_id: u64, url: &str) -> Result<(), LxAppError> {
    let webview = browser_find_webview(path, session_id)?;
    webview
        .load_url(url)
        .map_err(|e| LxAppError::WebView(e.to_string()))
}

fn browser_destroy_webview(path: &str, session_id: u64) {
    let webtag = browser_webtag(path, session_id);
    // Remove from global registry (triggers platform-specific cleanup on Drop).
    destroy_managed_webview(&webtag);
}

async fn browser_download_resource(owner: Arc<LxApp>, tab_id: String, request: DownloadRequest) {
    let task_id = Uuid::new_v4().to_string();
    let cancel_rx = download::register_active_download(&task_id);
    let task = crate::download::manager::DownloadTask::for_browser(
        request,
        crate::download::manager::browser_download_root(&owner.runtime.app_data_dir()),
        Some(rong::get_user_agent()),
    )
    .with_browser_persistence(owner.runtime.app_data_dir(), task_id.clone());
    let tab_id_for_event = tab_id.clone();

    let result = crate::download::manager::run_browser_download_task(
        task,
        &task_id,
        &tab_id_for_event,
        cancel_rx,
        |event_name, payload| {
            if let Err(err) = download::record_bridge_event(&owner.runtime.app_data_dir(), event_name, &payload) {
                crate::warn!(
                    "[InternalBrowser] failed to record download event task_id={} event={} error={}",
                    task_id,
                    event_name,
                    err
                );
            }
            publish_browser_download_event(event_name, payload);
        },
    )
    .await;
    download::unregister_active_download(&task_id);
    if let Err(err) = result {
        crate::warn!(
            "[InternalBrowser] download task failed tab_id={} url={} reason={}",
            tab_id,
            err.url,
            err.error
        );
    }
}

pub fn downloads_snapshot(lxapp: &LxApp) -> Result<DownloadsSnapshot, LxAppError> {
    download::snapshot(&lxapp.app_data_dir())
}

pub fn subscribe_downloads(
    lxapp: &LxApp,
) -> Result<broadcast::Receiver<DownloadEvent>, LxAppError> {
    download::subscribe(&lxapp.app_data_dir())
}

pub fn download_record(lxapp: &LxApp, task_id: &str) -> Result<Option<DownloadRecord>, LxAppError> {
    download::get_record(&lxapp.app_data_dir(), task_id)
}

pub fn clear_completed_downloads(lxapp: &LxApp) -> Result<u64, LxAppError> {
    download::clear_completed(&lxapp.app_data_dir())
}

pub fn remove_download(lxapp: &LxApp, task_id: &str) -> Result<(), LxAppError> {
    let removed = download::remove(&lxapp.app_data_dir(), task_id)?;
    if removed.is_none() {
        return Err(LxAppError::ResourceNotFound(format!(
            "download not found: {}",
            task_id
        )));
    }
    Ok(())
}

pub fn cancel_download(lxapp: &LxApp, task_id: &str) -> Result<(), LxAppError> {
    if download::cancel_active_download(task_id) {
        crate::info!("[Downloads] cancel requested for task_id={}", task_id);
        return Ok(());
    }

    let record = download::get_record(&lxapp.app_data_dir(), task_id)?
        .ok_or_else(|| LxAppError::ResourceNotFound(format!("download not found: {}", task_id)))?;
    if record.status != DownloadStatus::Downloading {
        return Err(LxAppError::UnsupportedOperation(
            "download is not active".to_string(),
        ));
    }
    Err(LxAppError::UnsupportedOperation(
        "download can no longer be canceled".to_string(),
    ))
}

pub fn cancel_active_download_signal(task_id: &str) -> bool {
    download::cancel_active_download(task_id)
}

pub fn retry_download(task_id: &str) -> Result<(), LxAppError> {
    let owner = ensure_browser_lxapp()?;
    let app_data_dir = owner.runtime.app_data_dir();
    let record = download::get_record(&app_data_dir, task_id)?
        .ok_or_else(|| LxAppError::ResourceNotFound(format!("download not found: {task_id}")))?;
    if record.status != DownloadStatus::Failed {
        return Err(LxAppError::UnsupportedOperation(
            "download is not retryable".to_string(),
        ));
    }
    if !record.retry {
        return Err(LxAppError::UnsupportedOperation(
            "download cannot be retried".to_string(),
        ));
    }
    if download::has_active_download(task_id) {
        return Err(LxAppError::UnsupportedOperation(
            "download is already active".to_string(),
        ));
    }

    let request_context =
        download::get_request_context(&app_data_dir, task_id)?.ok_or_else(|| {
            LxAppError::UnsupportedOperation("download retry context is unavailable".to_string())
        })?;

    if matches!(
        record.owner.kind,
        crate::download::manager::DownloadOwnerKind::LxDownloadFile
    ) {
        let user_cache_dir = PathBuf::from(&record.target_path)
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                LxAppError::UnsupportedOperation(
                    "download retry target path has no parent directory".to_string(),
                )
            })?;
        let task_id_owned = task_id.to_string();
        let app_data_dir_clone = app_data_dir.clone();
        let owner_appid = record.owner.appid.clone();
        let url = record.url.clone();
        let headers = request_context.headers.clone();
        let user_agent = request_context.user_agent.clone();

        rong::bg::spawn(async move {
            let persistence = crate::download::manager::DownloadPersistence::new(
                app_data_dir_clone.clone(),
                task_id_owned.clone(),
                crate::download::manager::DownloadOwner {
                    kind: crate::download::manager::DownloadOwnerKind::LxDownloadFile,
                    appid: owner_appid,
                    page_path: None,
                    tab_id: None,
                },
                true,
            );
            let result = crate::download::manager::download_to_user_cache(
                Some(persistence),
                &user_cache_dir,
                crate::download::manager::UserCacheDownloadRequest { url, headers },
                user_agent,
                |_| {},
            )
            .await;
            if let Err(err) = result {
                crate::warn!(
                    "[Downloads] retry download task failed task_id={} url={} reason={}",
                    task_id_owned,
                    err.url,
                    err.error
                );
            }
        })
        .map_err(|e| LxAppError::Runtime(format!("failed to spawn download retry task: {e}")))?;

        return Ok(());
    }

    let request = DownloadRequest {
        url: record.url.clone(),
        user_agent: request_context.user_agent.clone(),
        content_disposition: None,
        mime_type: record.mime_type.clone(),
        content_length: record.total_bytes,
        suggested_filename: request_context
            .suggested_filename
            .clone()
            .or_else(|| Some(record.file_name.clone())),
        source_page_url: request_context.source_page_url.clone(),
        cookie: request_context.cookie.clone(),
    };
    let cancel_rx = download::register_active_download(task_id);
    let task = crate::download::manager::DownloadTask::for_browser(
        request,
        crate::download::manager::browser_download_root(&app_data_dir),
        Some(rong::get_user_agent()),
    )
    .with_target_path(PathBuf::from(&record.target_path))
    .with_browser_persistence(app_data_dir.clone(), task_id.to_string());
    let owner_clone = owner.clone();
    let task_id_owned = task_id.to_string();
    let tab_id = record.tab_id.clone();

    rong::bg::spawn(async move {
        let result = crate::download::manager::run_browser_download_task(
            task,
            &task_id_owned,
            &tab_id,
            cancel_rx,
            |event_name, payload| {
                if let Err(err) = download::record_bridge_event(
                    &owner_clone.runtime.app_data_dir(),
                    event_name,
                    &payload,
                ) {
                    crate::warn!(
                        "[InternalBrowser] failed to record retry download event task_id={} event={} error={}",
                        task_id_owned,
                        event_name,
                        err
                    );
                }
                publish_browser_download_event(event_name, payload);
            },
        )
        .await;
        download::unregister_active_download(&task_id_owned);
        if let Err(err) = result {
            crate::warn!(
                "[InternalBrowser] retry download task failed task_id={} url={} reason={}",
                task_id_owned,
                err.url,
                err.error
            );
        }
    })
    .map_err(|e| LxAppError::Runtime(format!("failed to spawn browser retry task: {e}")))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Browser startup page
// ---------------------------------------------------------------------------

/// Generate the browser startup page HTML with bridge injection.
///
/// Reads the startup page from the registered browser LxApp, applies bridge
/// config and CSS injection (same pipeline as normal lxapp pages), and returns
/// the HTML bytes together with the `lx://` base URL for asset resolution.
pub fn generate_browser_startup_html() -> Result<(Vec<u8>, String), LxAppError> {
    // Lazy-load browser lxapp on first use
    let browser = ensure_browser_lxapp()?;
    let startup_page = browser.config.get_initial_route();
    if startup_page.is_empty() {
        return Err(LxAppError::InvalidParameter(format!(
            "{} has no startup page configured in lxapp.json",
            BUILTIN_BROWSER_APPID
        )));
    }
    let html = browser.generate_page_html(&startup_page, None);
    let base_url = format!("lx://lxapp/{}/{}", BUILTIN_BROWSER_APPID, startup_page);
    Ok((html, base_url))
}

// ---------------------------------------------------------------------------
// Owner resolution (used by FFI bridge layer)
// ---------------------------------------------------------------------------

pub fn resolve_owner_lxapp(
    owner_appid: &str,
    owner_session_id: u64,
) -> Result<Arc<LxApp>, LxAppError> {
    let owner_appid = owner_appid.trim();
    if owner_appid.is_empty() || owner_session_id == 0 {
        return Err(LxAppError::InvalidParameter(
            "owner_appid and owner_session_id are required".to_string(),
        ));
    }

    let owner = crate::try_get(owner_appid).ok_or_else(|| {
        LxAppError::ResourceNotFound(format!(
            "owner lxapp not found for browser tab operation: {}",
            owner_appid
        ))
    })?;

    if owner.session_id() != owner_session_id {
        return Err(LxAppError::InvalidParameter(format!(
            "owner session mismatch for {}: expected {}, got {}",
            owner_appid,
            owner.session_id(),
            owner_session_id
        )));
    }

    Ok(owner)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

fn browser_assets_installed() -> bool {
    crate::lxapp::metadata::get(BUILTIN_BROWSER_APPID, crate::lxapp::ReleaseType::Release)
        .ok()
        .flatten()
        .is_some_and(|record| {
            let install_path_str = record.install_path.trim();
            let install_path = Path::new(install_path_str);
            !install_path_str.is_empty()
                && install_path.is_dir()
                && install_path.join("lxapp.json").is_file()
        })
}

/// Try to install built-in browser assets during SDK initialization.
/// Missing packaged assets are tolerated.
pub(crate) fn preload_builtin_browser_assets(runtime: Arc<lingxia_platform::Platform>) {
    if browser_assets_installed() {
        return;
    }

    if let Err(e) = crate::update::UpdateManager::install_from_assets(
        runtime,
        BUILTIN_BROWSER_APPID,
        crate::SDK_RUNTIME_VERSION,
    ) {
        crate::warn!("Built-in browser assets not available: {}", e);
    }
}

/// Ensure browser lxapp instance exists in manager.
fn ensure_browser_lxapp() -> Result<Arc<LxApp>, LxAppError> {
    let _load_guard = BROWSER_LOAD_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    if let Some(browser) = crate::try_get(BUILTIN_BROWSER_APPID) {
        return Ok(browser);
    }

    let manager = crate::lxapp::get_lxapps_manager()
        .ok_or_else(|| LxAppError::Runtime("LxApps manager not initialized".to_string()))?;
    Ok(manager.ensure_lxapp(
        BUILTIN_BROWSER_APPID.to_string(),
        crate::lxapp::metadata::ReleaseType::Release,
    ))
}

pub fn browser_tab_path_for_id(tab_id: &str) -> String {
    // All browser tab pages are routed under the built-in browser lxapp.
    format!("{INTERNAL_TAB_PATH_PREFIX}{}", sanitize_tab_id(tab_id))
}

fn browser_internal_page_path_for_url(browser: &LxApp, url: &str) -> Option<String> {
    if extract_url_scheme(url).as_deref() != Some(LINGXIA_SCHEME) {
        return None;
    }
    let host = url
        .splitn(2, "://")
        .nth(1)
        .unwrap_or("")
        .split('/')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if host.is_empty() || host == "newtab" {
        return Some(browser.config.get_initial_route());
    }
    let page_path = format!("pages/{host}/index.html");
    Some(browser.find_page_path(&page_path).unwrap_or(page_path))
}

pub(crate) fn browser_logic_page_path_for_tab_path(
    browser: &LxApp,
    tab_path: &str,
) -> Option<String> {
    let tab_id = tab_path.strip_prefix(INTERNAL_TAB_PATH_PREFIX)?;
    let normalized = sanitize_tab_id(tab_id);
    if normalized.is_empty() {
        return None;
    }
    let target_url = {
        let state = lock_state();
        let tab = state.tabs.get(&normalized)?;
        tab.current_url
            .as_ref()
            .or(tab.pending_url.as_ref())
            .cloned()?
    };
    browser_internal_page_path_for_url(browser, &target_url)
}

fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    let text = value.unwrap_or_default().trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn build_tab_info(tab_id: &str, state: &BrowserTabState) -> BrowserTabInfo {
    BrowserTabInfo {
        tab_id: tab_id.to_string(),
        path: browser_tab_path_for_id(tab_id),
        session_id: state.session_id,
        current_url: state.current_url.clone(),
        title: state.title.clone(),
    }
}

pub fn browser_tab_info(tab_id: &str) -> Option<BrowserTabInfo> {
    let normalized = sanitize_tab_id(tab_id);
    if normalized.is_empty() {
        return None;
    }
    let state = lock_state();
    state
        .tabs
        .get(&normalized)
        .map(|tab| build_tab_info(&normalized, tab))
}

pub fn browser_tab_info_json(tab_id: &str) -> Option<String> {
    browser_tab_info(tab_id).and_then(|info| serde_json::to_string(&info).ok())
}

pub fn browser_tab_infos() -> Vec<BrowserTabInfo> {
    let state = lock_state();
    let mut result = state
        .tabs
        .iter()
        .map(|(tab_id, tab)| build_tab_info(tab_id, tab))
        .collect::<Vec<_>>();
    result.sort_by(|a, b| a.tab_id.cmp(&b.tab_id));
    result
}

pub fn browser_tab_infos_json() -> String {
    serde_json::to_string(&browser_tab_infos()).unwrap_or_else(|_| "[]".to_string())
}

pub fn browser_update_tab_info(
    tab_id: &str,
    current_url: Option<&str>,
    title: Option<&str>,
) -> bool {
    let normalized = sanitize_tab_id(tab_id);
    if normalized.is_empty() {
        return false;
    }
    let mut state = lock_state();
    let Some(tab) = state.tabs.get_mut(&normalized) else {
        return false;
    };
    if current_url.is_some() {
        tab.current_url = normalize_optional_string(current_url);
    }
    if title.is_some() {
        tab.title = normalize_optional_string(title);
    }
    true
}

pub fn start_native_browser_download(
    tab_id: &str,
    url: &str,
    user_agent: Option<&str>,
    suggested_filename: Option<&str>,
    source_page_url: Option<&str>,
    cookie: Option<&str>,
) -> Result<(), LxAppError> {
    let normalized_tab_id = sanitize_tab_id(tab_id);
    if normalized_tab_id.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "tab_id is required".to_string(),
        ));
    }

    let normalized_url = url.trim();
    if normalized_url.is_empty() {
        return Err(LxAppError::InvalidParameter("url is required".to_string()));
    }
    if !matches!(
        extract_url_scheme(normalized_url).as_deref(),
        Some("http" | "https")
    ) {
        return Err(LxAppError::InvalidParameter(
            "browser download url must be http(s)".to_string(),
        ));
    }

    let source_page_url = normalize_optional_string(source_page_url)
        .or_else(|| browser_tab_info(&normalized_tab_id).and_then(|info| info.current_url));
    if !browser_tab_exists(&normalized_tab_id) {
        return Err(LxAppError::ResourceNotFound(format!(
            "browser tab not found: {}",
            normalized_tab_id
        )));
    }

    let owner = ensure_browser_lxapp()?;
    let request = DownloadRequest {
        url: normalized_url.to_string(),
        user_agent: normalize_optional_string(user_agent),
        content_disposition: None,
        mime_type: None,
        content_length: None,
        suggested_filename: normalize_optional_string(suggested_filename),
        source_page_url,
        cookie: normalize_optional_string(cookie),
    };

    rong::bg::spawn({
        let owner = owner.clone();
        let tab_id = normalized_tab_id.clone();
        async move {
            browser_download_resource(owner, tab_id, request).await;
        }
    })
    .map_err(|e| LxAppError::Runtime(format!("failed to spawn browser download task: {e}")))?;

    Ok(())
}

pub fn open_internal_browser_tab(url: &str, tab_id: Option<&str>) -> Result<String, LxAppError> {
    let browser = ensure_browser_lxapp()?;
    let browser_session_id = browser.session_id();

    let raw_url = url.trim();

    // `lingxia://newtab` (and bare `lingxia://`) → startup page (no URL).
    // Other `lingxia://` pages stay as-is and are served by the lingxia:// scheme handler.
    let effective_url: String = match is_lingxia_startup_url(raw_url) {
        Some(true) => String::new(),
        _ => raw_url.to_string(),
    };
    let target_url = effective_url.as_str();

    let normalized_target_url = normalize_browser_target_url(target_url);
    let has_target_url = !normalized_target_url.is_empty();
    let tab_id = tab_id
        .map(sanitize_tab_id)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(generate_tab_id);
    let path = browser_tab_path_for_id(&tab_id);
    let session_id = browser_session_id;
    let mut create_token: Option<u64> = None;
    let mut is_new_tab = false;

    {
        let mut state = lock_state();
        if let Some(existing) = state.tabs.get_mut(&tab_id) {
            existing.session_id = session_id;
            if has_target_url {
                existing.pending_url = Some(normalized_target_url.clone());
                existing.current_url = Some(normalized_target_url.clone());
            }
        } else {
            is_new_tab = true;
            let token = next_browser_create_token();
            create_token = Some(token);
            state.tabs.insert(
                tab_id.clone(),
                BrowserTabState {
                    session_id,
                    create_token: token,
                    pending_url: if has_target_url {
                        Some(normalized_target_url.clone())
                    } else {
                        None
                    },
                    current_url: if has_target_url {
                        Some(normalized_target_url.clone())
                    } else {
                        None
                    },
                    title: None,
                },
            );
        }
    }

    if is_new_tab {
        let token = create_token.expect("create_token must exist for new tab");
        if let Err(e) = browser_create_webview(&path, session_id, &tab_id, token) {
            lock_state().tabs.remove(&tab_id);
            return Err(e);
        }
        return Ok(tab_id);
    }

    // Existing tab — load target URL if provided.
    if has_target_url {
        match browser_load_url(&path, session_id, &normalized_target_url) {
            Ok(()) => {
                if let Some(s) = lock_state().tabs.get_mut(&tab_id) {
                    s.pending_url = None;
                    s.current_url = Some(normalized_target_url.clone());
                }
            }
            Err(LxAppError::ResourceNotFound(_)) => {
                // WebView may still be creating on another thread; keep pending_url for replay.
            }
            Err(e) => return Err(e),
        }
    }

    Ok(tab_id)
}

pub fn close_internal_browser_tab(lxapp: &LxApp, tab_id: &str) -> Result<(), LxAppError> {
    let _ = lxapp;
    close_browser_tab(tab_id)
}

pub fn browser_tab_exists(tab_id: &str) -> bool {
    let normalized = sanitize_tab_id(tab_id);
    if normalized.is_empty() {
        return false;
    }
    lock_state().tabs.contains_key(&normalized)
}

pub fn close_browser_tab(tab_id: &str) -> Result<(), LxAppError> {
    let normalized = sanitize_tab_id(tab_id);
    if normalized.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "tab_id is required".to_string(),
        ));
    }

    let removed = {
        let mut state = lock_state();
        state.tabs.remove(&normalized)
    };
    if let Some(tab) = removed {
        let tab_path = browser_tab_path_for_id(&normalized);
        // Detach only when this tab currently backs the startup page bridge.
        // Closing a background tab must not break the active tab bridge.
        if let Ok(browser) = ensure_browser_lxapp() {
            let startup_path = browser.config.get_initial_route();
            if let Some(page) = browser.get_page(&startup_path) {
                let startup_webview = page.webview();
                let closing_tab_webview = browser_find_webview(&tab_path, tab.session_id).ok();
                if let (Some(startup_webview), Some(closing_tab_webview)) =
                    (startup_webview, closing_tab_webview)
                    && Arc::ptr_eq(&startup_webview, &closing_tab_webview)
                {
                    page.detach_webview();
                }
            }
        }
        browser_destroy_webview(&tab_path, tab.session_id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_without_scheme_navigates_with_https() {
        let response = handle_browser_address_input(BrowserAddressInputRequest {
            raw_input: "example.com/docs".to_string(),
            trigger: BrowserAddressInputTrigger::Submit,
            context: BrowserAddressInputContext::default(),
        });

        assert_eq!(response.action, BrowserAddressAction::Navigate);
        assert_eq!(
            response.navigation.as_ref().map(|value| value.url.as_str()),
            Some("https://example.com/docs")
        );
        assert_eq!(response.state.value_kind, BrowserAddressValueKind::Url);
        assert_eq!(response.state.inferred_scheme.as_deref(), Some("https"));
    }

    #[test]
    fn submit_keeps_http_fragments() {
        let response = handle_browser_address_input(BrowserAddressInputRequest {
            raw_input: "http://example.com/path?q=1#frag".to_string(),
            trigger: BrowserAddressInputTrigger::Submit,
            context: BrowserAddressInputContext::default(),
        });

        assert_eq!(response.action, BrowserAddressAction::Navigate);
        assert_eq!(
            response.navigation.as_ref().map(|value| value.url.as_str()),
            Some("http://example.com/path?q=1#frag")
        );
        assert_eq!(response.state.inferred_scheme, None);
    }

    #[test]
    fn submit_supports_localhost() {
        let response = handle_browser_address_input(BrowserAddressInputRequest {
            raw_input: "localhost:3000".to_string(),
            trigger: BrowserAddressInputTrigger::Submit,
            context: BrowserAddressInputContext::default(),
        });

        assert_eq!(response.action, BrowserAddressAction::Navigate);
        assert_eq!(
            response.navigation.as_ref().map(|value| value.url.as_str()),
            Some("https://localhost:3000")
        );
    }

    #[test]
    fn edit_search_query_returns_suggest_action() {
        let response = handle_browser_address_input(BrowserAddressInputRequest {
            raw_input: "openai docs".to_string(),
            trigger: BrowserAddressInputTrigger::Edit,
            context: BrowserAddressInputContext::default(),
        });

        assert_eq!(response.action, BrowserAddressAction::Suggest);
        assert_eq!(
            response.state.value_kind,
            BrowserAddressValueKind::SearchQuery
        );
        assert!(response.navigation.is_none());
    }

    #[test]
    fn submit_search_query_rejects_when_fallback_is_disabled() {
        let response = handle_browser_address_input(BrowserAddressInputRequest {
            raw_input: "openai".to_string(),
            trigger: BrowserAddressInputTrigger::Submit,
            context: BrowserAddressInputContext::default(),
        });

        assert_eq!(response.action, BrowserAddressAction::Reject);
        assert_eq!(
            response.error.as_ref().map(|value| value.code.as_str()),
            Some("search_fallback_unavailable")
        );
        assert_eq!(
            response.state.value_kind,
            BrowserAddressValueKind::SearchQuery
        );
    }

    #[test]
    fn normalize_browser_target_url_upgrades_http_case_insensitively() {
        assert_eq!(
            normalize_browser_target_url("  HTTP://Example.com/path?q=1 "),
            "https://Example.com/path?q=1"
        );
        assert_eq!(
            normalize_browser_target_url("http://example.com"),
            "https://example.com"
        );
        assert_eq!(
            normalize_browser_target_url("https://example.com"),
            "https://example.com"
        );
    }

    #[test]
    fn browser_nav_policy_allows_lark_with_gesture() {
        let response = handle_browser_navigation_policy(BrowserNavigationPolicyRequest {
            raw_url: "lark://client/auth?code=1".to_string(),
            has_user_gesture: true,
            is_main_frame: true,
        });

        assert_eq!(
            response.decision,
            BrowserNavigationPolicyDecision::OpenExternal
        );
    }

    #[test]
    fn browser_nav_policy_denies_lark_without_gesture() {
        let response = handle_browser_navigation_policy(BrowserNavigationPolicyRequest {
            raw_url: "lark://client/auth?code=1".to_string(),
            has_user_gesture: false,
            is_main_frame: true,
        });

        assert_eq!(response.decision, BrowserNavigationPolicyDecision::Deny);
        assert_eq!(response.reason.as_deref(), Some("gesture_required"));
    }

    #[test]
    fn browser_nav_policy_allows_unknown_custom_scheme_with_gesture() {
        let response = handle_browser_navigation_policy(BrowserNavigationPolicyRequest {
            raw_url: "customxyz://hello".to_string(),
            has_user_gesture: true,
            is_main_frame: true,
        });

        assert_eq!(
            response.decision,
            BrowserNavigationPolicyDecision::OpenExternal
        );
    }

    #[test]
    fn browser_nav_policy_denies_non_external_scheme() {
        let response = handle_browser_navigation_policy(BrowserNavigationPolicyRequest {
            raw_url: "javascript:alert(1)".to_string(),
            has_user_gesture: true,
            is_main_frame: true,
        });

        assert_eq!(response.decision, BrowserNavigationPolicyDecision::Deny);
        assert_eq!(response.reason.as_deref(), Some("non_external_scheme"));
    }

    #[test]
    fn browser_nav_policy_denies_external_in_subframe() {
        let response = handle_browser_navigation_policy(BrowserNavigationPolicyRequest {
            raw_url: "lark://client/auth".to_string(),
            has_user_gesture: true,
            is_main_frame: false,
        });

        assert_eq!(response.decision, BrowserNavigationPolicyDecision::Deny);
        assert_eq!(response.reason.as_deref(), Some("non_main_frame_external"));
    }

    #[test]
    fn lingxia_newtab_is_startup_url() {
        assert_eq!(is_lingxia_startup_url("lingxia://newtab"), Some(true));
        assert_eq!(is_lingxia_startup_url("lingxia://"), Some(true));
        assert_eq!(is_lingxia_startup_url("lingxia://downloads"), Some(false));
        assert_eq!(is_lingxia_startup_url("https://example.com"), None);
    }

    #[test]
    fn address_input_submit_lingxia_newtab_navigates() {
        let response = handle_browser_address_input(BrowserAddressInputRequest {
            raw_input: "lingxia://newtab".to_string(),
            trigger: BrowserAddressInputTrigger::Submit,
            context: BrowserAddressInputContext::default(),
        });
        assert_eq!(response.action, BrowserAddressAction::Navigate);
        assert_eq!(response.state.value_kind, BrowserAddressValueKind::Url);
        assert_eq!(
            response.navigation.as_ref().map(|n| n.url.as_str()),
            Some("lingxia://newtab")
        );
    }

    #[test]
    fn address_input_submit_lingxia_downloads_navigates() {
        let response = handle_browser_address_input(BrowserAddressInputRequest {
            raw_input: "lingxia://downloads".to_string(),
            trigger: BrowserAddressInputTrigger::Submit,
            context: BrowserAddressInputContext::default(),
        });
        assert_eq!(response.action, BrowserAddressAction::Navigate);
        assert_eq!(
            response.navigation.as_ref().map(|n| n.url.as_str()),
            Some("lingxia://downloads")
        );
    }

    #[test]
    fn browser_nav_policy_allows_lingxia_in_webview() {
        // `lingxia://` is served natively by the browser scheme handler — stay in-webview.
        let response = handle_browser_navigation_policy(BrowserNavigationPolicyRequest {
            raw_url: "lingxia://settings".to_string(),
            has_user_gesture: false,
            is_main_frame: true,
        });
        assert_eq!(
            response.decision,
            BrowserNavigationPolicyDecision::InWebview
        );
    }
}
