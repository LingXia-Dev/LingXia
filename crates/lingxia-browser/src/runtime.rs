use http::{Request, Response, StatusCode, Uri};
use lingxia_platform::traits::app_runtime::{AppRuntime, OpenUrlRequest, OpenUrlTarget};
use lingxia_platform::traits::file::FileDialogFilter;
use lingxia_platform::traits::file::{ChooseDirectoryRequest, ChooseFileRequest, FileService};
use lingxia_transfer as downloads;
use lingxia_webview::runtime::{
    destroy_webview as destroy_managed_webview, find_webview as find_managed_webview,
};
use lingxia_webview::{
    DownloadRequest, FileChooserFile, FileChooserRequest, FileChooserResponse, LogLevel,
    NavigationPolicy, NewWindowPolicy, WebTag, WebView, WebViewBuilder, WebViewController,
    WebViewCookie, WebViewCookieSetRequest, WebViewDelegate, WebViewError, WebViewInputError,
    WebViewScriptError, WebViewSession,
};
use lxapp::{LxApp, LxAppError, Page, publish_app_event};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub const BUILTIN_BROWSER_APPID: &str = "app.lingxia.browser";
const DEFAULT_QUERY_TEXT_LIMIT: usize = 4096;
const INTERNAL_TAB_PATH_PREFIX: &str = "/tabs/";

/// Register a startup-time script that should run after each browser page load.
///
/// Browser pages all belong to the single built-in browser LxApp, so startup-time
/// registration is enough: warmup drains these scripts into that app's page-script list.
pub(crate) fn register_browser_startup_page_script(js: impl Into<String>) {
    let scripts = BROWSER_STARTUP_PAGE_SCRIPTS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut guard) = scripts.lock() {
        guard.push(js.into());
    }
}

/// Register a browser-internal route and the packaged HTML entry that implements it.
///
/// Example: `register_browser_internal_page("settings", "pages/settings/index.html")`.
/// Runtime routing then resolves `lingxia://settings` through this registry instead of
/// assuming a file layout from the host name.
pub(crate) fn register_browser_internal_page(
    route: impl Into<String>,
    entry_asset: impl Into<String>,
) -> Result<(), LxAppError> {
    let route = normalize_internal_page_route_key(&route.into())?;
    let entry_asset = normalize_internal_page_entry_asset(&entry_asset.into())?;
    let pages = BROWSER_INTERNAL_PAGES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = pages.lock().unwrap_or_else(|e| e.into_inner());
    guard.insert(route, BrowserInternalPageRegistration { entry_asset });
    Ok(())
}

/// Take all registered scripts, leaving the registry empty.
/// Subsequent calls return an empty Vec.
fn take_browser_startup_page_scripts() -> Vec<String> {
    BROWSER_STARTUP_PAGE_SCRIPTS
        .get()
        .and_then(|m| m.lock().ok())
        .map(|mut guard| std::mem::take(&mut *guard))
        .unwrap_or_default()
}
const LINGXIA_SCHEME: &str = "lingxia";
const BROWSER_IN_WEBVIEW_SCHEMES: &[&str] = &["http", "https", "lx", "lingxia"];
const BROWSER_NON_EXTERNAL_SCHEMES: &[&str] = &["about", "data", "blob", "javascript", "file"];
const BROWSER_LINGXIA_ASSET_HOSTS: &[&str] = &[
    "lxapp",
    "plugin",
    "usercache",
    "userdata",
    "assets",
    "proxy",
];

#[derive(Clone, Debug)]
struct BrowserInternalPageRegistration {
    entry_asset: String,
}

#[derive(Clone, Debug)]
enum InternalPageTarget {
    StartupPage { page_path: String },
    Registered(BrowserInternalPageRegistration),
}

// Internal browser tab model:
// 1) All tabs are hosted by the built-in browser lxapp (BUILTIN_BROWSER_APPID).
// 2) Callers may provide a stable tab key; the core resolves that key against an
//    explicit scope and maps it to a canonical runtime UUID tab id.
// 3) One canonical runtime tab id maps to one page path: /tabs/{tab_id}.
// 4) One canonical runtime tab id owns one managed WebView instance lifecycle.

fn normalize_browser_target_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= "http://".len() && trimmed[..7].eq_ignore_ascii_case("http://") {
        format!("https://{}", &trimmed[7..])
    } else {
        trimmed.to_string()
    }
}

fn normalize_url_for_wait_compare(raw: &str) -> String {
    let normalized = normalize_browser_target_url(raw);
    let trimmed = normalized.trim();
    let Ok(uri) = trimmed.parse::<http::Uri>() else {
        return trimmed.to_string();
    };
    let Some(scheme) = uri.scheme_str().map(str::to_ascii_lowercase) else {
        return trimmed.to_string();
    };
    if !matches!(scheme.as_str(), "http" | "https") {
        return trimmed.to_string();
    }
    let Some(host) = uri.host() else {
        return trimmed.to_string();
    };
    let host = host.to_ascii_lowercase();
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host
    };
    let port = uri
        .port()
        .map(|port| format!(":{}", port.as_str()))
        .unwrap_or_default();
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("/");
    format!("{scheme}://{host}{port}{path_and_query}")
}

fn normalize_internal_page_route_key(raw: &str) -> Result<String, LxAppError> {
    let trimmed = raw.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "browser internal route must not be empty".to_string(),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(LxAppError::InvalidParameter(format!(
            "invalid browser internal route '{}'",
            raw.trim()
        )));
    }
    Ok(trimmed)
}

fn normalize_internal_page_entry_asset(raw: &str) -> Result<String, LxAppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "browser internal page entry asset must not be empty".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn browser_internal_pages() -> HashMap<String, BrowserInternalPageRegistration> {
    BROWSER_INTERNAL_PAGES
        .get()
        .and_then(|m| m.lock().ok())
        .map(|guard| guard.clone())
        .unwrap_or_default()
}

fn browser_internal_page_for_host(host: &str) -> Option<BrowserInternalPageRegistration> {
    let route = normalize_internal_page_route_key(host).ok()?;
    browser_internal_pages().remove(&route)
}

fn internal_page_target_for_host(startup_path: &str, host: &str) -> Option<InternalPageTarget> {
    match host {
        "" => Some(InternalPageTarget::StartupPage {
            page_path: startup_path.to_string(),
        }),
        _ => browser_internal_page_for_host(host)
            .map(InternalPageTarget::Registered)
            .or_else(|| {
                (host == "newtab").then(|| InternalPageTarget::StartupPage {
                    page_path: startup_path.to_string(),
                })
            }),
    }
}

fn internal_page_target_entry_path(target: &InternalPageTarget) -> &str {
    match target {
        InternalPageTarget::StartupPage { page_path } => page_path.as_str(),
        InternalPageTarget::Registered(registration) => registration.entry_asset.as_str(),
    }
}

fn internal_page_target_for_url(startup_path: &str, url: &str) -> Option<InternalPageTarget> {
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
    internal_page_target_for_host(startup_path, &host)
}

fn is_browser_lingxia_asset_host(host: &str) -> bool {
    BROWSER_LINGXIA_ASSET_HOSTS.contains(&host)
}

fn extensions_for_accept_token(value: &str) -> Vec<&'static str> {
    match value {
        "image/*" => vec![
            "png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "heic", "heif",
        ],
        "audio/*" => vec!["mp3", "wav", "aac", "m4a", "ogg", "flac"],
        "video/*" => vec!["mp4", "mov", "m4v", "webm", "mkv", "avi"],
        "text/*" => vec!["txt", "md", "csv", "log"],
        "application/pdf" => vec!["pdf"],
        "application/zip" => vec!["zip"],
        "application/json" => vec!["json"],
        "text/plain" => vec!["txt"],
        "text/csv" => vec!["csv"],
        "text/markdown" => vec!["md"],
        "application/msword" => vec!["doc"],
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => vec!["docx"],
        "application/vnd.ms-excel" => vec!["xls"],
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => vec!["xlsx"],
        "application/vnd.ms-powerpoint" => vec!["ppt"],
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => vec!["pptx"],
        _ => Vec::new(),
    }
}

fn file_filters_from_accept_types(accept_types: &[String]) -> Vec<FileDialogFilter> {
    let mut extensions: Vec<String> = accept_types
        .iter()
        .flat_map(|raw| raw.split(','))
        .map(str::trim)
        .filter_map(|value| {
            if value.is_empty() {
                return None;
            }
            if let Some(stripped) = value.strip_prefix('.') {
                return (!stripped.is_empty()).then(|| stripped.to_ascii_lowercase());
            }
            if value.contains('/') {
                return None;
            }
            Some(value.to_ascii_lowercase())
        })
        .collect();

    for accept_type in accept_types
        .iter()
        .flat_map(|raw| raw.split(','))
        .map(str::trim)
    {
        if accept_type.is_empty() {
            continue;
        }
        extensions.extend(
            extensions_for_accept_token(&accept_type.to_ascii_lowercase())
                .into_iter()
                .map(str::to_string),
        );
    }

    extensions.sort();
    extensions.dedup();

    if extensions.is_empty() {
        Vec::new()
    } else {
        vec![FileDialogFilter {
            name: Some("Files".to_string()),
            extensions,
        }]
    }
}

async fn browser_choose_files(
    owner: Arc<LxApp>,
    request: FileChooserRequest,
) -> FileChooserResponse {
    if request.allow_directories {
        return match owner
            .runtime
            .choose_directory(ChooseDirectoryRequest {
                title: Some("Choose folder".to_string()),
                default_path: None,
            })
            .await
        {
            Ok(result) if !result.canceled && !result.paths.is_empty() => {
                FileChooserResponse::Files(
                    result
                        .paths
                        .into_iter()
                        .map(|value| FileChooserFile {
                            path: (!value.contains("://")).then_some(value.clone()),
                            uri: value.contains("://").then_some(value),
                        })
                        .collect(),
                )
            }
            Ok(_) => FileChooserResponse::Cancel,
            Err(err) => {
                publish_browser_file_chooser_failed_event(&request, &err.to_string());
                lxapp::warn!(
                    "[InternalBrowser] file chooser directory request failed: {}",
                    err
                );
                FileChooserResponse::Error(err.to_string())
            }
        };
    }

    match owner
        .runtime
        .choose_file(ChooseFileRequest {
            multiple: request.allow_multiple,
            filters: file_filters_from_accept_types(&request.accept_types),
            title: Some("Choose file".to_string()),
            default_path: None,
        })
        .await
    {
        Ok(result) if !result.canceled && !result.paths.is_empty() => FileChooserResponse::Files(
            result
                .paths
                .into_iter()
                .map(|value| FileChooserFile {
                    path: (!value.contains("://")).then_some(value.clone()),
                    uri: value.contains("://").then_some(value),
                })
                .collect(),
        ),
        Ok(_) => FileChooserResponse::Cancel,
        Err(err) => {
            publish_browser_file_chooser_failed_event(&request, &err.to_string());
            lxapp::warn!("[InternalBrowser] file chooser request failed: {}", err);
            FileChooserResponse::Error(err.to_string())
        }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
    pub right: f64,
    pub bottom: f64,
    pub center_x: f64,
    pub center_y: f64,
    pub viewport_width: f64,
    pub viewport_height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserElementInfo {
    pub exists: bool,
    pub visible: bool,
    pub enabled: bool,
    pub editable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub text_truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub value_truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<BrowserRect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BrowserWaitCondition {
    Loaded,
    SelectorExists {
        selector: String,
    },
    SelectorVisible {
        selector: String,
    },
    SelectorHidden {
        selector: String,
    },
    SelectorEditable {
        selector: String,
    },
    JsTrue {
        js: String,
    },
    UrlEquals {
        url: String,
    },
    UrlContains {
        text: String,
    },
    Navigation {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        initial_url: Option<String>,
        #[serde(default)]
        wait_until_complete: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserWaitResult {
    pub elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<BrowserElementInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

pub trait BrowserNativeInputHost: Send + Sync {
    fn prepare_for_input(&self, tab_id: &str) -> Result<(), String>;
}

#[derive(Debug, thiserror::Error)]
pub enum BrowserAutomationError {
    #[error("browser tab not found: {0}")]
    TabNotFound(String),
    #[error("browser tab webview not found: {0}")]
    WebViewNotFound(String),
    #[error(transparent)]
    Script(#[from] WebViewScriptError),
    #[error(transparent)]
    Input(#[from] WebViewInputError),
    #[error(transparent)]
    WebView(#[from] WebViewError),
    #[error("native input host is not registered")]
    NativeInputHostMissing,
    #[error("native input error: {0}")]
    NativeInput(String),
    #[error("timed out after {timeout_ms}ms waiting for {condition}")]
    WaitTimeout { condition: String, timeout_ms: u64 },
}

fn default_true() -> bool {
    true
}

fn is_false(value: &bool) -> bool {
    !*value
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
/// - Registered `lingxia://<route>` values resolve via the browser internal-page registry.
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

pub(crate) fn handle_browser_navigation_policy_json(request_json: &str) -> Option<String> {
    let request: BrowserNavigationPolicyRequest = serde_json::from_str(request_json).ok()?;
    serde_json::to_string(&handle_browser_navigation_policy(request)).ok()
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
static BROWSER_TAB_COUNTER: AtomicU64 = AtomicU64::new(1);
static BROWSER_CREATE_TOKEN: AtomicU64 = AtomicU64::new(1);
static BROWSER_LOAD_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
static BROWSER_STARTUP_PAGE_INIT_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
static BROWSER_STARTUP_PAGE_SCRIPTS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static BROWSER_INTERNAL_PAGES: OnceLock<Mutex<HashMap<String, BrowserInternalPageRegistration>>> =
    OnceLock::new();
static BROWSER_NATIVE_INPUT_HOST: OnceLock<Arc<dyn BrowserNativeInputHost>> = OnceLock::new();
static BROWSER_ACTIVE_TAB_ID: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn lock_state() -> MutexGuard<'static, BrowserState> {
    BROWSER_STATE
        .get_or_init(|| {
            Mutex::new(BrowserState {
                tabs: HashMap::new(),
            })
        })
        .lock()
        .unwrap_or_else(|e| {
            lxapp::warn!("[InternalBrowser] recovered poisoned browser state mutex");
            e.into_inner()
        })
}

fn lock_active_tab() -> MutexGuard<'static, Option<String>> {
    BROWSER_ACTIVE_TAB_ID
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn set_active_browser_tab(tab_id: &str) {
    *lock_active_tab() = Some(tab_id.to_string());
}

pub fn register_native_input_host(host: Arc<dyn BrowserNativeInputHost>) -> bool {
    BROWSER_NATIVE_INPUT_HOST.set(host).is_ok()
}

fn native_input_host() -> Option<&'static Arc<dyn BrowserNativeInputHost>> {
    BROWSER_NATIVE_INPUT_HOST.get()
}

async fn prepare_browser_tab_for_input(tab_id: &str) -> Result<(), BrowserAutomationError> {
    if let Some(host) = native_input_host() {
        let mut last_error = None;
        for _ in 0..10 {
            match host.prepare_for_input(tab_id) {
                Ok(()) => return Ok(()),
                Err(error) => last_error = Some(error),
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        return Err(BrowserAutomationError::NativeInput(
            last_error.unwrap_or_else(|| format!("failed to prepare browser tab: {tab_id}")),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum BrowserTabScope<'a> {
    Global,
    OwnerSession {
        owner_appid: &'a str,
        owner_session_id: u64,
    },
}

fn generate_tab_id() -> String {
    loop {
        let candidate = format!(
            "tab-{}",
            BROWSER_TAB_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        if !lock_state().tabs.contains_key(&candidate) {
            return candidate;
        }
    }
}

fn validate_requested_tab_key(input: &str) -> Result<String, LxAppError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "tab_id is required".to_string(),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(LxAppError::InvalidParameter(
            "tab_id must contain only ASCII letters, digits, '-' or '_'".to_string(),
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn normalize_runtime_tab_id(input: &str) -> Option<String> {
    validate_requested_tab_key(input).ok()
}

fn resolve_tab_scope_seed(scope: BrowserTabScope<'_>, stable_tab_key: &str) -> String {
    match scope {
        BrowserTabScope::Global => format!("global:{stable_tab_key}"),
        BrowserTabScope::OwnerSession {
            owner_appid,
            owner_session_id,
        } => format!("owner:{owner_appid}:{owner_session_id}:{stable_tab_key}"),
    }
}

fn deterministic_tab_suffix(seed: &str) -> String {
    const FNV_OFFSET_A: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    fn fnv1a64(bytes: &[u8], offset: u64, prime: u64) -> u64 {
        let mut hash = offset;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(prime);
        }
        hash
    }

    format!(
        "{:08x}",
        fnv1a64(seed.as_bytes(), FNV_OFFSET_A, FNV_PRIME) as u32
    )
}

fn resolve_browser_tab_id(
    requested_tab_key: Option<&str>,
    scope: BrowserTabScope<'_>,
) -> Result<String, LxAppError> {
    match requested_tab_key {
        Some(tab_key) => {
            let stable_tab_key = validate_requested_tab_key(tab_key)?;
            match scope {
                BrowserTabScope::Global => Ok(stable_tab_key),
                BrowserTabScope::OwnerSession { .. } => {
                    let seed = resolve_tab_scope_seed(scope, &stable_tab_key);
                    Ok(format!(
                        "{}-{}",
                        stable_tab_key,
                        deterministic_tab_suffix(&seed)
                    ))
                }
            }
        }
        None => Ok(generate_tab_id()),
    }
}

fn next_browser_create_token() -> u64 {
    BROWSER_CREATE_TOKEN.fetch_add(1, Ordering::Relaxed)
}

fn publish_browser_download_event(event_name: &str, payload: serde_json::Value) {
    let payload_str = Some(payload.to_string());
    let _ = publish_app_event(BUILTIN_BROWSER_APPID, event_name, payload_str);
}

fn publish_browser_file_chooser_failed_event(request: &FileChooserRequest, error: &str) {
    let payload = serde_json::json!({
        "error": error,
        "acceptTypes": request.accept_types,
        "allowMultiple": request.allow_multiple,
        "allowDirectories": request.allow_directories,
        "capture": request.capture,
        "sourcePageUrl": request.source_page_url,
    });
    let _ = publish_app_event(
        BUILTIN_BROWSER_APPID,
        "FileChooserFailed",
        Some(payload.to_string()),
    );
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
    tab_id: String,
    page_path: String,
    session_id: u64,
}

impl WebViewDelegate for BrowserTabDelegate {
    fn on_page_started(&self) {
        match browser_resolve_delegate_page(&self.tab_id, &self.page_path, self.session_id) {
            Ok(page) => page.notify_page_started(),
            Err(err) => {
                lxapp::warn!(
                    "[InternalBrowser] Failed to resolve delegate page for tab {} on start: {}",
                    self.tab_id,
                    err
                );
            }
        }
    }

    fn on_page_finished(&self) {
        match browser_resolve_delegate_page(&self.tab_id, &self.page_path, self.session_id) {
            Ok(page) => page.handle_loaded(),
            Err(err) => {
                lxapp::warn!(
                    "[InternalBrowser] Failed to resolve delegate page for tab {} on finish: {}",
                    self.tab_id,
                    err
                );
            }
        }
    }

    fn handle_post_message(&self, msg: String) {
        match browser_resolve_delegate_page(&self.tab_id, &self.page_path, self.session_id) {
            Ok(page) => {
                if let Err(err) = page.handle_incoming_message_json(&msg) {
                    lxapp::warn!(
                        "[InternalBrowser] Failed to handle bridge message for tab {}: {}",
                        self.tab_id,
                        err
                    );
                }
            }
            Err(err) => {
                lxapp::warn!(
                    "[InternalBrowser] Failed to resolve delegate page for tab {}: {}",
                    self.tab_id,
                    err
                );
            }
        }
    }

    fn log(&self, level: LogLevel, message: &str) {
        let log_level = match level {
            LogLevel::Error => lxapp::log::LogLevel::Error,
            LogLevel::Warn => lxapp::log::LogLevel::Warn,
            LogLevel::Info => lxapp::log::LogLevel::Info,
            LogLevel::Debug | LogLevel::Verbose => lxapp::log::LogLevel::Debug,
        };
        lxapp::log::LogBuilder::new(lxapp::log::LogTag::WebViewConsole, message)
            .with_level(log_level)
            .with_path(&self.page_path)
            .with_appid(BUILTIN_BROWSER_APPID.to_string());
    }
}

/// Ensure the browser lxapp has a headless startup Page + a live PageSvc.
///
/// Idempotent: if the page already exists in the browser lxapp's page map, returns it directly.
/// Otherwise creates a headless Page (nonce, no WebView), registers it, starts the AppSvc,
/// and asynchronously awaits the PageSvc ack before signalling the page as "ready".
fn ensure_browser_startup_page(browser: &Arc<LxApp>) -> Result<Page, LxAppError> {
    let startup_path = browser.initial_route();

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
    if let Err(e) = browser.ensure_app_service_running() {
        lxapp::warn!("[InternalBrowser] Failed to start app service: {}", e);
    }

    browser.ensure_headless_page_service(&startup_path)
}

fn ensure_internal_tab_page(owner: &Arc<LxApp>, path: &str) -> Result<Page, LxAppError> {
    owner.ensure_headless_page_service(path)
}

fn default_internal_page_target(browser: &Arc<LxApp>) -> InternalPageTarget {
    internal_page_target_for_host(&browser.initial_route(), "")
        .expect("startup page target must always resolve")
}

fn current_internal_page_target_for_tab(browser: &Arc<LxApp>, tab_id: &str) -> InternalPageTarget {
    let url = {
        let state = lock_state();
        state
            .tabs
            .get(tab_id)
            .and_then(|tab| tab.current_url.as_ref().or(tab.pending_url.as_ref()))
            .cloned()
    };
    url.as_deref()
        .and_then(|value| internal_page_target_for_url(&browser.initial_route(), value))
        .unwrap_or_else(|| default_internal_page_target(browser))
}

fn ensure_internal_tab_page_for_target(
    browser: &Arc<LxApp>,
    tab_path: &str,
    target: &InternalPageTarget,
) -> Result<(Arc<LxApp>, Page), LxAppError> {
    let owner = ensure_browser_lxapp()?;
    ensure_browser_startup_page(&owner)?;
    let page = ensure_internal_tab_page(&owner, tab_path)?;
    let _ = browser;
    let _ = target;
    Ok((owner, page))
}

fn detach_internal_tab_pages_except(tab_path: &str, keep_appid: &str) {
    if let Some(browser) = lxapp::try_get(BUILTIN_BROWSER_APPID)
        && browser.appid != keep_appid
        && let Some(page) = browser.get_page(tab_path)
    {
        page.detach_webview();
    }
}

fn bind_internal_tab_page(
    browser: &Arc<LxApp>,
    tab_path: &str,
    session_id: u64,
    target: &InternalPageTarget,
) -> Result<Page, LxAppError> {
    let (owner, page) = ensure_internal_tab_page_for_target(browser, tab_path, target)?;
    detach_internal_tab_pages_except(tab_path, &owner.appid);
    if let Ok(webview) = browser_find_webview(tab_path, session_id) {
        page.attach_webview(webview);
    }
    Ok(page)
}

fn browser_resolve_delegate_context(
    tab_id: &str,
    tab_path: &str,
    session_id: u64,
) -> Result<(Arc<LxApp>, Page), LxAppError> {
    let browser = ensure_browser_lxapp()?;
    let target = current_internal_page_target_for_tab(&browser, tab_id);
    let page = bind_internal_tab_page(&browser, tab_path, session_id, &target)?;
    Ok((browser, page))
}

fn browser_resolve_delegate_page(
    tab_id: &str,
    tab_path: &str,
    session_id: u64,
) -> Result<Page, LxAppError> {
    browser_resolve_delegate_context(tab_id, tab_path, session_id).map(|(_, page)| page)
}

fn rewrite_internal_page_asset_request(
    owner: &LxApp,
    target: &InternalPageTarget,
    req: Request<Vec<u8>>,
) -> Result<Request<Vec<u8>>, LxAppError> {
    let (mut parts, body) = req.into_parts();
    let req_uri = parts.uri.clone();
    let entry_asset = internal_page_target_entry_path(target);
    let base_dir = entry_asset
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .unwrap_or("");
    let asset_rel = req_uri.path().trim_start_matches('/');
    let asset_path = if asset_rel.eq_ignore_ascii_case("favicon.ico") {
        "public/favicon.ico".to_string()
    } else if asset_rel.is_empty() {
        entry_asset.to_string()
    } else if base_dir.is_empty() {
        asset_rel.to_string()
    } else {
        format!("{base_dir}/{asset_rel}")
    };

    let mut rewritten = format!("lx://lxapp/{}/{}", owner.appid, asset_path);
    if let Some(query) = req_uri.query() {
        rewritten.push('?');
        rewritten.push_str(query);
    }
    let uri = Uri::try_from(rewritten).map_err(|err| {
        LxAppError::InvalidParameter(format!("invalid internal asset uri: {err}"))
    })?;
    parts.uri = uri;
    Ok(Request::from_parts(parts, body))
}

/// Attach the given tab WebView to its headless page and load a lingxia:// URL into it.
/// Waits for the PageSvc to be ready first.
///
/// `page_url`: the `lx://` URL to load. `None` loads the default startup/newtab page;
/// `Some(url)` loads a specific internal browser page (e.g. `lx://lxapp/.../downloads`).
async fn browser_attach_tab_page(
    webview: Arc<WebView>,
    page_path: &str,
    session_id: u64,
    tab_id: &str,
    page_url: Option<&str>,
) -> Result<(), LxAppError> {
    let browser = ensure_browser_lxapp()?;
    let target = match page_url {
        Some(url) => {
            internal_page_target_for_url(&browser.initial_route(), url).ok_or_else(|| {
                LxAppError::ResourceNotFound(format!(
                    "browser internal route not registered for url: {}",
                    url
                ))
            })?
        }
        None => default_internal_page_target(&browser),
    };
    let page = bind_internal_tab_page(&browser, page_path, session_id, &target)?;

    // Wait until PageSvc signals ready (ack from JS worker).
    if let Err(e) = page.wait_webview_ready().await {
        lxapp::warn!(
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
    let tab_path_owned = path.to_string();
    let tab_id_owned = tab_id.to_string();

    // Ensure the JS worker and browser startup page exist before creating the tab WebView.
    ensure_browser_startup_page(&browser_owner)?;

    let startup_path = browser_owner.initial_route();
    let owner_for_lingxia = browser_owner.clone();
    let startup_path_for_lingxia = startup_path.clone();
    let tab_id_for_lx = tab_id.to_string();
    let tab_path_for_lx = tab_path_owned.clone();
    let tab_path_for_lingxia = tab_path_owned.clone();
    let tab_id_for_lingxia = tab_id_owned.clone();
    let runtime_for_nav = browser_owner.runtime.clone();
    let owner_appid_for_nav = browser_owner.appid.clone();
    let owner_session_for_nav = browser_owner.session_id();
    let runtime_for_new_window = browser_owner.runtime.clone();
    let owner_appid_for_new_window = browser_owner.appid.clone();
    let owner_session_for_new_window = browser_owner.session_id();
    let owner_for_download = browser_owner.clone();
    let owner_for_file_chooser = browser_owner.clone();
    let session = WebViewBuilder::browser(webtag)
        .delegate(Arc::new(BrowserTabDelegate {
            tab_id: tab_id_owned.clone(),
            page_path: tab_path_owned.clone(),
            session_id,
        }))
        .on_scheme("lx", move |req| {
            let tab_id = tab_id_for_lx.clone();
            let tab_path = tab_path_for_lx.clone();
            async move {
                match browser_resolve_delegate_context(&tab_id, &tab_path, session_id) {
                    Ok((owner, page)) => owner.handle_lingxia_request(&page, req).into(),
                    Err(err) => {
                        lxapp::warn!(
                            "[InternalBrowser] Failed to resolve lx:// owner for tab {}: {}",
                            tab_id,
                            err
                        );
                        None.into()
                    }
                }
            }
        })
        .on_scheme(LINGXIA_SCHEME, move |req| {
            let browser_owner = owner_for_lingxia.clone();
            let tab_page_path = tab_path_for_lingxia.clone();
            let tab_session_id = session_id;
            let startup_path = startup_path_for_lingxia.clone();
            let tab_id = tab_id_for_lingxia.clone();
            async move {
                // Map `lingxia://` hosts to browser internal pages.
                let host = req.uri().host().unwrap_or("").to_ascii_lowercase();
                if is_browser_lingxia_asset_host(&host) {
                    let target = current_internal_page_target_for_tab(&browser_owner, &tab_id);
                    let page = match bind_internal_tab_page(
                        &browser_owner,
                        &tab_page_path,
                        tab_session_id,
                        &target,
                    ) {
                        Ok(page) => page,
                        Err(err) => {
                            lxapp::warn!(
                                "[InternalBrowser] Failed to bind asset page for tab {} host {}: {}",
                                tab_id,
                                host,
                                err
                            );
                            return None.into();
                        }
                    };
                    return browser_owner.handle_lingxia_request(&page, req).into();
                }
                let Some(target) = internal_page_target_for_host(&startup_path, &host) else {
                    lxapp::warn!(
                        "[InternalBrowser] Unregistered browser internal route host={}",
                        host
                    );
                    return None.into();
                };
                let page = match bind_internal_tab_page(
                    &browser_owner,
                    &tab_page_path,
                    tab_session_id,
                    &target,
                ) {
                    Ok(page) => page,
                    Err(err) => {
                        lxapp::warn!(
                            "[InternalBrowser] Failed to bind internal page for tab {} host {}: {}",
                            tab_id,
                            host,
                            err
                        );
                        return None.into();
                    }
                };
                let owner = browser_owner.clone();
                // Serve page HTML (with bridge nonce) for the document root.
                let req_path = req.uri().path();
                if req_path == "/" || req_path.is_empty() {
                    let nonce = page.bridge_nonce();
                    let html =
                        owner.generate_page_html(internal_page_target_entry_path(&target), nonce.as_deref());
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
                // Route sub-resources relative to the browser internal page bundle.
                match rewrite_internal_page_asset_request(&owner, &target, req) {
                    Ok(rewritten) => owner.handle_lingxia_request(&page, rewritten).into(),
                    Err(err) => {
                        lxapp::warn!(
                            "[InternalBrowser] Failed to rewrite internal asset request for host {}: {}",
                            host,
                            err
                        );
                        None.into()
                    }
                }
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
            rong::RongExecutor::global().spawn(async move {
                browser_download_resource(owner, tab_id, request).await;
            });
        })
        .on_file_chooser(move |request| {
            let owner = owner_for_file_chooser.clone();
            async move { browser_choose_files(owner, request).await }
        })
        .create();
    let path_owned = path.to_string();
    let tab_id_owned = tab_id.to_string();

    rong::RongExecutor::global().spawn(async move {
        browser_on_webview_ready(path_owned, session_id, tab_id_owned, create_token, session).await;
    });
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
            lxapp::warn!(
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
                    if let Err(e) = browser_attach_tab_page(
                        webview.clone(),
                        &path,
                        session_id,
                        &tab_id,
                        Some(url.as_str()),
                    )
                    .await
                    {
                        lxapp::warn!(
                            "[InternalBrowser] Failed to attach startup page for internal tab {}: {}",
                            tab_id,
                            e
                        );
                        browser_clear_pending_if_token_matches(&tab_id, session_id, create_token);
                        let _ = webview.load_url("about:blank");
                    } else {
                        browser_commit_navigation_if_token_matches(
                            &tab_id,
                            session_id,
                            create_token,
                            Some(&url),
                        );
                    }
                } else {
                    // Direct URL load — no bridge handshake needed, just navigate.
                    if let Err(e) = webview.load_url(&url) {
                        lxapp::warn!(
                            "[InternalBrowser] Failed to load URL for tab {}: {}",
                            tab_id,
                            e
                        );
                        browser_clear_pending_if_token_matches(&tab_id, session_id, create_token);
                    } else {
                        browser_commit_navigation_if_token_matches(
                            &tab_id,
                            session_id,
                            create_token,
                            Some(&url),
                        );
                    }
                }
            } else {
                // Startup page: attach WebView to shared startup Page, then load with nonce.
                if let Err(e) =
                    browser_attach_tab_page(webview.clone(), &path, session_id, &tab_id, None).await
                {
                    lxapp::warn!(
                        "[InternalBrowser] Failed to load startup page for tab {}: {}",
                        tab_id,
                        e
                    );
                    let _ = webview.load_url("about:blank");
                } else {
                    browser_commit_navigation_if_token_matches(
                        &tab_id,
                        session_id,
                        create_token,
                        None,
                    );
                }
            }
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
    let cancel_rx = downloads::runtime::register_active_download(&task_id);
    let task = downloads::runtime::DownloadTask::for_browser(
        request,
        downloads::runtime::browser_download_root(&owner.runtime.app_data_dir()),
        Some(rong::get_user_agent()),
    )
    .with_browser_persistence(owner.runtime.app_data_dir(), task_id.clone());
    let tab_id_for_event = tab_id.clone();

    let result = downloads::runtime::run_browser_download_task(
        task,
        &task_id,
        &tab_id_for_event,
        cancel_rx,
        |event_name, payload| {
            if let Err(err) = downloads::runtime::record_bridge_event(
                &owner.runtime.app_data_dir(),
                event_name,
                &payload,
            ) {
                lxapp::warn!(
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
    downloads::runtime::unregister_active_download(&task_id);
    if let Err(err) = result {
        if err.error == "Download paused" {
            return;
        }
        lxapp::warn!(
            "[InternalBrowser] download task failed tab_id={} url={} reason={}",
            tab_id,
            err.url,
            err.error
        );
    }
}

fn map_lxapp_error_to_downloads(err: LxAppError) -> downloads::DownloadsError {
    match err {
        LxAppError::InvalidParameter(message) => {
            downloads::DownloadsError::InvalidParameter(message)
        }
        LxAppError::ResourceNotFound(message) => {
            downloads::DownloadsError::ResourceNotFound(message)
        }
        LxAppError::UnsupportedOperation(message) => {
            downloads::DownloadsError::UnsupportedOperation(message)
        }
        LxAppError::IoError(message)
        | LxAppError::Runtime(message)
        | LxAppError::ChannelError(message)
        | LxAppError::ResourceExhausted(message)
        | LxAppError::Bridge(message)
        | LxAppError::RongJS(message)
        | LxAppError::PluginNotConfigured(message)
        | LxAppError::PluginDownloadFailed(message)
        | LxAppError::InvalidJsonFile(message)
        | LxAppError::WebView(message) => downloads::DownloadsError::Runtime(message),
        LxAppError::RongJSHost { code, message, .. } => {
            downloads::DownloadsError::Runtime(format!("{code}: {message}"))
        }
    }
}

pub(crate) fn retry_browser_owned_download(task_id: &str) -> downloads::Result<()> {
    let owner = ensure_browser_lxapp().map_err(map_lxapp_error_to_downloads)?;
    let app_data_dir = owner.runtime.app_data_dir();
    let record = downloads::runtime::get_record(&app_data_dir, task_id)?.ok_or_else(|| {
        downloads::DownloadsError::ResourceNotFound(format!("download not found: {task_id}"))
    })?;
    if !matches!(
        record.status,
        downloads::DownloadStatus::Failed | downloads::DownloadStatus::Paused
    ) {
        return Err(downloads::DownloadsError::UnsupportedOperation(
            "download is not retryable".to_string(),
        ));
    }
    if !record.retry {
        return Err(downloads::DownloadsError::UnsupportedOperation(
            "download cannot be retried".to_string(),
        ));
    }
    if downloads::runtime::has_active_download(task_id) {
        return Err(downloads::DownloadsError::UnsupportedOperation(
            "download is already active".to_string(),
        ));
    }

    let request_context = downloads::runtime::get_request_context(&app_data_dir, task_id)?
        .ok_or_else(|| {
            downloads::DownloadsError::UnsupportedOperation(
                "download retry context is unavailable".to_string(),
            )
        })?;

    if matches!(
        record.owner.kind,
        downloads::user_cache::DownloadOwnerKind::LxApp
    ) {
        let task_id_owned = task_id.to_string();
        let app_data_dir_clone = app_data_dir.clone();
        let owner_appid = record.owner.appid.clone();
        let url = record.url.clone();
        let headers = request_context.headers.clone();
        let user_agent = request_context.user_agent.clone();
        let target_path = PathBuf::from(&record.target_path);
        let behavior = request_context.behavior;

        rong::RongExecutor::global().spawn(async move {
            let persistence = downloads::user_cache::DownloadPersistence::new(
                app_data_dir_clone.clone(),
                task_id_owned.clone(),
                downloads::user_cache::DownloadOwner {
                    kind: downloads::user_cache::DownloadOwnerKind::LxApp,
                    appid: owner_appid,
                    page_path: None,
                    tab_id: None,
                },
                true,
            );
            let result = downloads::user_cache::download_to_path_with_behavior(
                Some(persistence),
                target_path,
                downloads::user_cache::UserCacheDownloadRequest { url, headers },
                user_agent,
                behavior,
                |_| {},
            )
            .await;
            if let Err(err) = result {
                if err.error == "Download paused" {
                    return;
                }
                lxapp::warn!(
                    "[Downloads] retry download task failed task_id={} url={} reason={}",
                    task_id_owned,
                    err.url,
                    err.error
                );
            }
        });

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
    let cancel_rx = downloads::runtime::register_active_download(task_id);
    let task = downloads::runtime::DownloadTask::for_browser(
        request,
        downloads::runtime::browser_download_root(&app_data_dir),
        Some(rong::get_user_agent()),
    )
    .with_target_path(PathBuf::from(&record.target_path))
    .with_browser_persistence(app_data_dir.clone(), task_id.to_string())
    .with_behavior(request_context.behavior);
    let owner_clone = owner.clone();
    let task_id_owned = task_id.to_string();
    let tab_id = record.tab_id.clone();

    rong::RongExecutor::global().spawn(async move {
        let result = downloads::runtime::run_browser_download_task(
            task,
            &task_id_owned,
            &tab_id,
            cancel_rx,
            |event_name, payload| {
                if let Err(err) = downloads::runtime::record_bridge_event(
                    &owner_clone.runtime.app_data_dir(),
                    event_name,
                    &payload,
                ) {
                    lxapp::warn!(
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
        downloads::runtime::unregister_active_download(&task_id_owned);
        if let Err(err) = result {
            if err.error == "Download paused" {
                return;
            }
            lxapp::warn!(
                "[InternalBrowser] retry download task failed task_id={} url={} reason={}",
                task_id_owned,
                err.url,
                err.error
            );
        }
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Owner resolution (used by FFI bridge layer)
// ---------------------------------------------------------------------------

fn resolve_owner_lxapp(owner_appid: &str, owner_session_id: u64) -> Result<Arc<LxApp>, LxAppError> {
    let owner_appid = owner_appid.trim();
    if owner_appid.is_empty() || owner_session_id == 0 {
        return Err(LxAppError::InvalidParameter(
            "owner_appid and owner_session_id are required".to_string(),
        ));
    }

    let owner = lxapp::try_get(owner_appid).ok_or_else(|| {
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

pub(crate) fn register_builtin_browser_asset_bundle() {
    lxapp::register_builtin_asset_bundle(BUILTIN_BROWSER_APPID, BUILTIN_BROWSER_APPID);
}

pub(crate) fn warmup_builtin_browser_runtime() -> Result<(), LxAppError> {
    let browser = ensure_browser_lxapp()?;

    // Drain startup scripts registered before the browser LxApp existed
    // (e.g. shell's context-menu JS)
    // into the LxApp's page_scripts so they are picked up by Page::handle_loaded().
    // take_ ensures idempotency — repeated warmup calls won't duplicate scripts.
    for js in take_browser_startup_page_scripts() {
        browser.add_page_script(js);
    }

    let _ = ensure_browser_startup_page(&browser)?;
    Ok(())
}

/// Ensure browser lxapp instance exists in manager.
fn ensure_browser_lxapp() -> Result<Arc<LxApp>, LxAppError> {
    let _load_guard = BROWSER_LOAD_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    if let Some(browser) = lxapp::try_get(BUILTIN_BROWSER_APPID) {
        return Ok(browser);
    }

    lxapp::ensure_builtin_lxapp(BUILTIN_BROWSER_APPID)
}

fn browser_tab_path_for_runtime_id(tab_id: &str) -> String {
    format!("{INTERNAL_TAB_PATH_PREFIX}{tab_id}")
}

pub(crate) fn browser_tab_path_for_id(tab_id: &str) -> String {
    normalize_runtime_tab_id(tab_id)
        .map(|tab_id| browser_tab_path_for_runtime_id(&tab_id))
        .unwrap_or_else(|| INTERNAL_TAB_PATH_PREFIX.to_string())
}

fn browser_internal_page_path_for_url(browser: &LxApp, url: &str) -> Option<String> {
    let target = internal_page_target_for_url(&browser.initial_route(), url)?;
    Some(
        browser
            .find_page_path(internal_page_target_entry_path(&target))
            .unwrap_or_else(|| internal_page_target_entry_path(&target).to_string()),
    )
}

pub(crate) fn browser_logic_page_path_for_tab_path(
    browser: &LxApp,
    tab_path: &str,
) -> Option<String> {
    let tab_id = tab_path.strip_prefix(INTERNAL_TAB_PATH_PREFIX)?;
    let normalized = normalize_runtime_tab_id(tab_id)?;
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
        path: browser_tab_path_for_runtime_id(tab_id),
        session_id: state.session_id,
        current_url: state.current_url.clone(),
        title: state.title.clone(),
    }
}

pub fn browser_tab_info(tab_id: &str) -> Option<BrowserTabInfo> {
    let normalized = normalize_runtime_tab_id(tab_id)?;
    let state = lock_state();
    state
        .tabs
        .get(&normalized)
        .map(|tab| build_tab_info(&normalized, tab))
}

pub fn browser_tabs() -> Vec<BrowserTabInfo> {
    let state = lock_state();
    let mut tabs: Vec<BrowserTabInfo> = state
        .tabs
        .iter()
        .map(|(tab_id, tab)| build_tab_info(tab_id, tab))
        .collect();
    tabs.sort_by(|a, b| a.tab_id.cmp(&b.tab_id));
    tabs
}

pub fn browser_current_tab() -> Option<BrowserTabInfo> {
    if let Some(tab_id) = lock_active_tab().clone()
        && let Some(info) = browser_tab_info(&tab_id)
    {
        return Some(info);
    }
    browser_tabs().into_iter().next()
}

pub fn browser_activate_tab(tab_id: &str) -> Result<BrowserTabInfo, BrowserAutomationError> {
    let normalized_tab_id = normalize_runtime_tab_id(tab_id)
        .ok_or_else(|| BrowserAutomationError::TabNotFound(tab_id.to_string()))?;
    let info = browser_tab_info(&normalized_tab_id)
        .ok_or_else(|| BrowserAutomationError::TabNotFound(tab_id.to_string()))?;
    set_active_browser_tab(&normalized_tab_id);
    Ok(info)
}

fn browser_tab_webview(tab_id: &str) -> Result<Arc<WebView>, BrowserAutomationError> {
    let normalized_tab_id = normalize_runtime_tab_id(tab_id)
        .ok_or_else(|| BrowserAutomationError::TabNotFound(tab_id.to_string()))?;
    let session_id = {
        let state = lock_state();
        state
            .tabs
            .get(&normalized_tab_id)
            .map(|tab| tab.session_id)
            .ok_or_else(|| BrowserAutomationError::TabNotFound(tab_id.to_string()))?
    };
    let path = browser_tab_path_for_runtime_id(&normalized_tab_id);
    let webtag = WebTag::new(BUILTIN_BROWSER_APPID, &path, Some(session_id));
    find_managed_webview(&webtag)
        .ok_or_else(|| BrowserAutomationError::WebViewNotFound(tab_id.to_string()))
}

fn build_browser_query_script(
    selector: &str,
    max_text_chars: Option<usize>,
) -> Result<String, BrowserAutomationError> {
    let selector_json = serde_json::to_string(selector)
        .map_err(|err| BrowserAutomationError::NativeInput(format!("invalid selector: {err}")))?;
    let max_text_json = serde_json::to_string(&max_text_chars).map_err(|err| {
        BrowserAutomationError::NativeInput(format!("invalid query limit: {err}"))
    })?;
    Ok(format!(
        r#"
(() => {{
  const selector = {selector_json};
  const maxText = {max_text_json};
  const truncate = (value) => {{
    const text = String(value ?? "");
    if (typeof maxText === "number" && maxText >= 0 && text.length > maxText) {{
      return {{ value: text.slice(0, maxText), truncated: true }};
    }}
    return {{ value: text, truncated: false }};
  }};
  if (typeof selector !== "string" || selector.trim() === "") {{
    throw new Error("selector must not be empty");
  }}
  let el;
  try {{
    el = document.querySelector(selector);
  }} catch (err) {{
    throw new Error("invalid selector: " + String(err && err.message ? err.message : err));
  }}
  if (!el) {{
    return {{
      exists: false,
      visible: false,
      enabled: false,
      editable: false
    }};
  }}

  const rect = el.getBoundingClientRect();
  const style = window.getComputedStyle(el);
  const disabled = !!el.disabled || el.getAttribute("aria-disabled") === "true";
  const tag = (el.tagName || "").toLowerCase();
  const inputType = tag === "input" ? String(el.type || "text").toLowerCase() : "";
  const blockedInputTypes = new Set([
    "button", "checkbox", "color", "file", "hidden", "image", "radio",
    "range", "reset", "submit"
  ]);
  const editable = !!el.isContentEditable ||
    (tag === "textarea" && !disabled && !el.readOnly) ||
    (tag === "input" && !disabled && !el.readOnly && !blockedInputTypes.has(inputType));
  const visible = rect.width > 0 &&
    rect.height > 0 &&
    rect.bottom > 0 &&
    rect.right > 0 &&
    rect.top < window.innerHeight &&
    rect.left < window.innerWidth &&
    style.visibility !== "hidden" &&
    style.display !== "none" &&
    Number(style.opacity || "1") !== 0;
  const hasValue = "value" in el;
  const text = truncate(el.innerText || el.textContent || "");
  const value = hasValue ? truncate(el.value ?? "") : null;
  return {{
    exists: true,
    visible,
    enabled: !disabled,
    editable,
    text: text.value,
    text_truncated: text.truncated,
    value: value ? value.value : null,
    value_truncated: value ? value.truncated : false,
    rect: {{
      left: rect.left,
      top: rect.top,
      width: rect.width,
      height: rect.height,
      right: rect.right,
      bottom: rect.bottom,
      center_x: rect.left + (rect.width / 2),
      center_y: rect.top + (rect.height / 2),
      viewport_width: window.innerWidth,
      viewport_height: window.innerHeight
    }}
  }};
}})()
"#
    ))
}

fn browser_tab_current_url(tab_id: &str) -> Result<Option<String>, BrowserAutomationError> {
    let normalized_tab_id = normalize_runtime_tab_id(tab_id)
        .ok_or_else(|| BrowserAutomationError::TabNotFound(tab_id.to_string()))?;
    let state = lock_state();
    state
        .tabs
        .get(&normalized_tab_id)
        .map(|tab| tab.current_url.clone().or_else(|| tab.pending_url.clone()))
        .ok_or_else(|| BrowserAutomationError::TabNotFound(tab_id.to_string()))
}

async fn browser_live_current_url(tab_id: &str) -> Result<Option<String>, BrowserAutomationError> {
    let state_url = browser_tab_current_url(tab_id)?;
    let webview = browser_tab_webview(tab_id)?;

    match webview.current_url().await {
        Ok(Some(url)) => {
            let _ = browser_update_tab_info(tab_id, Some(url.as_str()), None);
            Ok(Some(url))
        }
        Ok(None) => Ok(state_url),
        Err(_) => Ok(state_url),
    }
}

pub async fn browser_current_url(tab_id: &str) -> Result<Option<String>, BrowserAutomationError> {
    browser_live_current_url(tab_id).await
}

fn wait_condition_label(condition: &BrowserWaitCondition) -> String {
    match condition {
        BrowserWaitCondition::Loaded => "loaded".to_string(),
        BrowserWaitCondition::SelectorExists { selector } => format!("selector exists {selector}"),
        BrowserWaitCondition::SelectorVisible { selector } => {
            format!("selector visible {selector}")
        }
        BrowserWaitCondition::SelectorHidden { selector } => {
            format!("selector hidden {selector}")
        }
        BrowserWaitCondition::SelectorEditable { selector } => {
            format!("selector editable {selector}")
        }
        BrowserWaitCondition::JsTrue { .. } => "js returns true".to_string(),
        BrowserWaitCondition::UrlEquals { url } => format!("url equals {url}"),
        BrowserWaitCondition::UrlContains { text } => format!("url contains {text}"),
        BrowserWaitCondition::Navigation { .. } => "navigation".to_string(),
    }
}

fn duration_ms_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

struct BrowserWaitCheck {
    matched: bool,
    current_url: Option<String>,
    element: Option<BrowserElementInfo>,
    value: Option<serde_json::Value>,
}

async fn check_wait_condition(
    tab_id: &str,
    condition: &BrowserWaitCondition,
) -> Result<BrowserWaitCheck, BrowserAutomationError> {
    match condition {
        BrowserWaitCondition::Loaded => {
            let value = browser_evaluate_javascript(tab_id, "document.readyState").await?;
            let matched = value.as_str() == Some("complete");
            Ok(BrowserWaitCheck {
                matched,
                current_url: browser_live_current_url(tab_id).await?,
                element: None,
                value: Some(value),
            })
        }
        BrowserWaitCondition::SelectorExists { selector } => {
            let element = browser_query(tab_id, selector).await?;
            Ok(BrowserWaitCheck {
                matched: element.exists,
                current_url: browser_live_current_url(tab_id).await?,
                element: Some(element),
                value: None,
            })
        }
        BrowserWaitCondition::SelectorVisible { selector } => {
            let element = browser_query(tab_id, selector).await?;
            Ok(BrowserWaitCheck {
                matched: element.exists && element.visible,
                current_url: browser_live_current_url(tab_id).await?,
                element: Some(element),
                value: None,
            })
        }
        BrowserWaitCondition::SelectorHidden { selector } => {
            let element = browser_query(tab_id, selector).await?;
            Ok(BrowserWaitCheck {
                matched: !element.exists || !element.visible,
                current_url: browser_live_current_url(tab_id).await?,
                element: Some(element),
                value: None,
            })
        }
        BrowserWaitCondition::SelectorEditable { selector } => {
            let element = browser_query(tab_id, selector).await?;
            Ok(BrowserWaitCheck {
                matched: element.exists && element.visible && element.enabled && element.editable,
                current_url: browser_live_current_url(tab_id).await?,
                element: Some(element),
                value: None,
            })
        }
        BrowserWaitCondition::JsTrue { js } => {
            let value = browser_evaluate_javascript(tab_id, js).await?;
            Ok(BrowserWaitCheck {
                matched: value.as_bool().unwrap_or(false),
                current_url: browser_live_current_url(tab_id).await?,
                element: None,
                value: Some(value),
            })
        }
        BrowserWaitCondition::UrlEquals { url } => {
            let current_url = browser_live_current_url(tab_id).await?;
            let expected = normalize_url_for_wait_compare(url);
            Ok(BrowserWaitCheck {
                matched: current_url
                    .as_deref()
                    .is_some_and(|url| normalize_url_for_wait_compare(url) == expected),
                current_url,
                element: None,
                value: None,
            })
        }
        BrowserWaitCondition::UrlContains { text } => {
            let current_url = browser_live_current_url(tab_id).await?;
            Ok(BrowserWaitCheck {
                matched: current_url
                    .as_deref()
                    .is_some_and(|url| url.contains(text.as_str())),
                current_url,
                element: None,
                value: None,
            })
        }
        BrowserWaitCondition::Navigation {
            initial_url,
            wait_until_complete,
        } => {
            let current_url = browser_live_current_url(tab_id).await?;
            let changed = current_url.as_deref().map(normalize_url_for_wait_compare)
                != initial_url.as_deref().map(normalize_url_for_wait_compare);
            let loaded = if changed && *wait_until_complete {
                browser_evaluate_javascript(tab_id, "document.readyState")
                    .await?
                    .as_str()
                    == Some("complete")
            } else {
                true
            };
            Ok(BrowserWaitCheck {
                matched: changed && loaded,
                current_url,
                element: None,
                value: None,
            })
        }
    }
}

pub async fn browser_evaluate_javascript(
    tab_id: &str,
    js: &str,
) -> Result<serde_json::Value, BrowserAutomationError> {
    browser_tab_webview(tab_id)?
        .evaluate_javascript(js)
        .await
        .map_err(BrowserAutomationError::from)
}

pub fn browser_reload(tab_id: &str) -> Result<(), BrowserAutomationError> {
    browser_tab_webview(tab_id)?.reload()?;
    Ok(())
}

pub fn browser_go_back(tab_id: &str) -> Result<(), BrowserAutomationError> {
    browser_tab_webview(tab_id)?.go_back()?;
    Ok(())
}

pub fn browser_go_forward(tab_id: &str) -> Result<(), BrowserAutomationError> {
    browser_tab_webview(tab_id)?.go_forward()?;
    Ok(())
}

pub async fn browser_list_cookies(
    tab_id: &str,
) -> Result<Vec<WebViewCookie>, BrowserAutomationError> {
    let current_url = browser_live_current_url(tab_id).await?;
    let cookies = browser_tab_webview(tab_id)?
        .list_cookies()
        .await
        .map_err(BrowserAutomationError::from)?;
    Ok(
        match current_url
            .as_deref()
            .and_then(cookie_filter_context_for_url)
        {
            Some((host, path)) => cookies
                .into_iter()
                .filter(|cookie| cookie_matches_url(cookie, &host, &path))
                .collect(),
            None => cookies,
        },
    )
}

pub async fn browser_set_cookie(
    tab_id: &str,
    mut request: WebViewCookieSetRequest,
) -> Result<(), BrowserAutomationError> {
    if request.url.trim().is_empty() {
        request.url = browser_live_current_url(tab_id).await?.ok_or_else(|| {
            BrowserAutomationError::NativeInput(
                "cookie url is required when tab has no current URL".to_string(),
            )
        })?;
    }
    browser_tab_webview(tab_id)?
        .set_cookie(request)
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_delete_cookie(
    tab_id: &str,
    name: &str,
    domain: &str,
    path: &str,
) -> Result<(), BrowserAutomationError> {
    browser_tab_webview(tab_id)?
        .delete_cookie(name, domain, path)
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_clear_cookies(tab_id: &str) -> Result<(), BrowserAutomationError> {
    browser_tab_webview(tab_id)?
        .clear_cookies()
        .await
        .map_err(BrowserAutomationError::from)
}

fn cookie_filter_context_for_url(url: &str) -> Option<(String, String)> {
    let uri = url.parse::<http::Uri>().ok()?;
    let host = normalize_cookie_host(uri.host()?);
    if host.is_empty() {
        None
    } else {
        let path = uri
            .path_and_query()
            .map(|value| value.path())
            .filter(|value| !value.is_empty())
            .unwrap_or("/")
            .to_string();
        Some((host, path))
    }
}

fn cookie_matches_url(cookie: &WebViewCookie, host: &str, path: &str) -> bool {
    let domain = normalize_cookie_host(cookie.domain.trim_start_matches('.'));
    if domain.is_empty() {
        return false;
    }
    let domain_matches = if cookie.host_only {
        host == domain
    } else {
        host == domain || host.ends_with(&format!(".{domain}"))
    };
    if !domain_matches {
        return false;
    }
    let cookie_path = if cookie.path.trim().is_empty() {
        "/"
    } else {
        cookie.path.as_str()
    };
    if cookie_path == "/" || path == cookie_path {
        return true;
    }
    if cookie_path.ends_with('/') {
        return path.starts_with(cookie_path);
    }
    path.strip_prefix(cookie_path)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

fn normalize_cookie_host(host: &str) -> String {
    host.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase()
}

pub async fn browser_query(
    tab_id: &str,
    selector: &str,
) -> Result<BrowserElementInfo, BrowserAutomationError> {
    browser_query_with_max_text(tab_id, selector, Some(DEFAULT_QUERY_TEXT_LIMIT)).await
}

pub async fn browser_query_with_max_text(
    tab_id: &str,
    selector: &str,
    max_text_chars: Option<usize>,
) -> Result<BrowserElementInfo, BrowserAutomationError> {
    let script = build_browser_query_script(selector, max_text_chars)?;
    let value = browser_evaluate_javascript(tab_id, &script).await?;
    serde_json::from_value(value).map_err(|err| {
        BrowserAutomationError::NativeInput(format!("failed to decode element info: {err}"))
    })
}

pub async fn browser_wait(
    tab_id: &str,
    condition: BrowserWaitCondition,
    timeout: Duration,
) -> Result<BrowserWaitResult, BrowserAutomationError> {
    let started = Instant::now();
    let timeout_ms = duration_ms_u64(timeout);

    loop {
        let check = check_wait_condition(tab_id, &condition).await?;
        if check.matched {
            return Ok(BrowserWaitResult {
                elapsed_ms: duration_ms_u64(started.elapsed()),
                current_url: check.current_url,
                element: check.element,
                value: check.value,
            });
        }

        if started.elapsed() >= timeout {
            return Err(BrowserAutomationError::WaitTimeout {
                condition: wait_condition_label(&condition),
                timeout_ms,
            });
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

pub async fn browser_wait_for_url(
    tab_id: &str,
    url: &str,
    timeout: Duration,
) -> Result<BrowserWaitResult, BrowserAutomationError> {
    browser_wait(
        tab_id,
        BrowserWaitCondition::UrlEquals {
            url: url.to_string(),
        },
        timeout,
    )
    .await
}

pub async fn browser_wait_for_url_contains(
    tab_id: &str,
    text: &str,
    timeout: Duration,
) -> Result<BrowserWaitResult, BrowserAutomationError> {
    browser_wait(
        tab_id,
        BrowserWaitCondition::UrlContains {
            text: text.to_string(),
        },
        timeout,
    )
    .await
}

pub async fn browser_wait_for_navigation(
    tab_id: &str,
    timeout: Duration,
    wait_until_complete: bool,
) -> Result<BrowserWaitResult, BrowserAutomationError> {
    let initial_url = browser_live_current_url(tab_id).await?;
    browser_wait(
        tab_id,
        BrowserWaitCondition::Navigation {
            initial_url,
            wait_until_complete,
        },
        timeout,
    )
    .await
}

pub async fn browser_click(tab_id: &str, selector: &str) -> Result<(), BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    browser_tab_webview(tab_id)?
        .click(selector, lingxia_webview::ClickOptions::default())
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_fill(
    tab_id: &str,
    selector: &str,
    text: &str,
) -> Result<(), BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    browser_tab_webview(tab_id)?
        .fill(selector, text, lingxia_webview::FillOptions::default())
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_type_text(
    tab_id: &str,
    selector: &str,
    text: &str,
) -> Result<(), BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    browser_tab_webview(tab_id)?
        .type_text(selector, text, lingxia_webview::TypeOptions::default())
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_press(tab_id: &str, key: &str) -> Result<(), BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    browser_tab_webview(tab_id)?
        .press(key, lingxia_webview::PressOptions::default())
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_scroll(tab_id: &str, dx: f64, dy: f64) -> Result<(), BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    browser_tab_webview(tab_id)?
        .scroll(dx, dy, lingxia_webview::ScrollOptions::default())
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_scroll_to(tab_id: &str, selector: &str) -> Result<(), BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    browser_tab_webview(tab_id)?
        .scroll_to(selector, lingxia_webview::ScrollOptions::default())
        .await
        .map_err(BrowserAutomationError::from)
}

pub(crate) fn browser_update_tab_info(
    tab_id: &str,
    current_url: Option<&str>,
    title: Option<&str>,
) -> bool {
    let Some(normalized) = normalize_runtime_tab_id(tab_id) else {
        return false;
    };
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

pub(crate) fn start_native_browser_download(
    tab_id: &str,
    url: &str,
    user_agent: Option<&str>,
    suggested_filename: Option<&str>,
    source_page_url: Option<&str>,
    cookie: Option<&str>,
) -> Result<(), LxAppError> {
    let normalized_tab_id = normalize_runtime_tab_id(tab_id).ok_or_else(|| {
        LxAppError::InvalidParameter("tab_id must be a valid runtime browser tab id".to_string())
    })?;

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

    rong::RongExecutor::global().spawn({
        let owner = owner.clone();
        let tab_id = normalized_tab_id.clone();
        async move {
            browser_download_resource(owner, tab_id, request).await;
        }
    });

    Ok(())
}

fn browser_commit_navigation_if_token_matches(
    tab_id: &str,
    session_id: u64,
    create_token: u64,
    current_url: Option<&str>,
) {
    let mut state = lock_state();
    if let Some(tab) = state.tabs.get_mut(tab_id)
        && tab.session_id == session_id
        && tab.create_token == create_token
    {
        tab.pending_url = None;
        tab.current_url = normalize_optional_string(current_url);
    }
}

fn browser_clear_pending_url(tab_id: &str) {
    let mut state = lock_state();
    if let Some(tab) = state.tabs.get_mut(tab_id) {
        tab.pending_url = None;
    }
}

fn open_internal_browser_tab_with_scope(
    url: &str,
    requested_tab_key: Option<&str>,
    scope: BrowserTabScope<'_>,
) -> Result<String, LxAppError> {
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
    let tab_id = resolve_browser_tab_id(requested_tab_key, scope)?;
    let path = browser_tab_path_for_runtime_id(&tab_id);
    let session_id = browser_session_id;
    let mut create_token: Option<u64> = None;
    let mut is_new_tab = false;

    {
        let mut state = lock_state();
        if let Some(existing) = state.tabs.get_mut(&tab_id) {
            existing.session_id = session_id;
            if has_target_url {
                existing.pending_url = Some(normalized_target_url.clone());
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
                    current_url: None,
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
        set_active_browser_tab(&tab_id);
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
            Err(e) => {
                browser_clear_pending_url(&tab_id);
                return Err(e);
            }
        }
    }

    set_active_browser_tab(&tab_id);
    Ok(tab_id)
}

pub(crate) fn open_internal_browser_tab(
    url: &str,
    tab_id: Option<&str>,
) -> Result<String, LxAppError> {
    open_internal_browser_tab_with_scope(url, tab_id, BrowserTabScope::Global)
}

pub(crate) fn open_internal_browser_tab_for_owner(
    owner_appid: &str,
    owner_session_id: u64,
    url: &str,
    tab_id: Option<&str>,
) -> Result<String, LxAppError> {
    let _owner = resolve_owner_lxapp(owner_appid, owner_session_id)?;
    open_internal_browser_tab_with_scope(
        url,
        tab_id,
        BrowserTabScope::OwnerSession {
            owner_appid,
            owner_session_id,
        },
    )
}

pub fn browser_tab_exists(tab_id: &str) -> bool {
    let Some(normalized) = normalize_runtime_tab_id(tab_id) else {
        return false;
    };
    lock_state().tabs.contains_key(&normalized)
}

pub(crate) fn close_browser_tab(tab_id: &str) -> Result<(), LxAppError> {
    let normalized = normalize_runtime_tab_id(tab_id).ok_or_else(|| {
        LxAppError::InvalidParameter("tab_id must be a valid runtime browser tab id".to_string())
    })?;

    let removed = {
        let mut state = lock_state();
        state.tabs.remove(&normalized)
    };
    if let Some(tab) = removed {
        let tab_path = browser_tab_path_for_runtime_id(&normalized);
        // Detach only when this tab currently backs the startup page bridge.
        // Closing a background tab must not break the active tab bridge.
        if let Ok(browser) = ensure_browser_lxapp() {
            let startup_path = browser.initial_route();
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
            if let Some(page) = browser.get_page(&tab_path) {
                page.detach_webview();
            }
            browser.remove_pages(std::slice::from_ref(&tab_path));
        }
        browser_destroy_webview(&tab_path, tab.session_id);
    }
    let active_matches_closed = lock_active_tab().as_deref() == Some(normalized.as_str());
    if active_matches_closed {
        let next = browser_tabs().into_iter().next().map(|tab| tab.tab_id);
        *lock_active_tab() = next;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static TEST_BROWSER_INTERNAL_PAGES: Once = Once::new();

    fn register_test_browser_internal_pages() {
        TEST_BROWSER_INTERNAL_PAGES.call_once(|| {
            register_browser_internal_page("downloads", "pages/downloads/index.html").unwrap();
            register_browser_internal_page("settings", "pages/settings/index.html").unwrap();
        });
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
    fn normalize_url_for_wait_compare_canonicalizes_browser_urls() {
        assert_eq!(
            normalize_url_for_wait_compare("https://Example.com"),
            "https://example.com/"
        );
        assert_eq!(
            normalize_url_for_wait_compare("http://example.com"),
            "https://example.com/"
        );
        assert_eq!(
            normalize_url_for_wait_compare("https://[::1]:8443/path?q=1"),
            "https://[::1]:8443/path?q=1"
        );
    }

    #[test]
    fn cookie_filter_context_handles_ipv6_urls() {
        assert_eq!(
            cookie_filter_context_for_url("https://[::1]:8443/path?q=1"),
            Some(("::1".to_string(), "/path".to_string()))
        );
    }

    #[test]
    fn cookie_matches_url_respects_host_only_and_path_rules() {
        let host_only = WebViewCookie {
            name: "a".to_string(),
            value: "1".to_string(),
            domain: "example.com".to_string(),
            path: "/foo".to_string(),
            host_only: true,
            secure: false,
            http_only: false,
            session: true,
            expires_unix_ms: None,
            same_site: None,
        };
        assert!(cookie_matches_url(&host_only, "example.com", "/foo/bar"));
        assert!(!cookie_matches_url(
            &host_only,
            "sub.example.com",
            "/foo/bar"
        ));
        assert!(!cookie_matches_url(&host_only, "example.com", "/foobar"));

        let domain_cookie = WebViewCookie {
            host_only: false,
            domain: ".example.com".to_string(),
            ..host_only
        };
        assert!(cookie_matches_url(
            &domain_cookie,
            "sub.example.com",
            "/foo/bar"
        ));
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
    fn registered_internal_page_route_resolves_to_entry_asset() {
        register_test_browser_internal_pages();
        let target = internal_page_target_for_url("pages/newtab/index.html", "lingxia://settings")
            .expect("settings route should resolve");
        assert_eq!(
            internal_page_target_entry_path(&target),
            "pages/settings/index.html"
        );
    }

    #[test]
    fn unknown_internal_page_route_does_not_resolve() {
        register_test_browser_internal_pages();
        assert!(
            internal_page_target_for_url("pages/newtab/index.html", "lingxia://unknown").is_none()
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

    #[test]
    fn lingxia_asset_hosts_delegate_to_lx_handler() {
        assert!(is_browser_lingxia_asset_host("lxapp"));
        assert!(is_browser_lingxia_asset_host("assets"));
        assert!(is_browser_lingxia_asset_host("plugin"));
        assert!(!is_browser_lingxia_asset_host("settings"));
        assert!(!is_browser_lingxia_asset_host("downloads"));
    }

    #[test]
    fn stable_browser_tab_ids_are_deterministic_per_scope() {
        let global_a = resolve_browser_tab_id(Some("settings"), BrowserTabScope::Global).unwrap();
        let global_b = resolve_browser_tab_id(Some("settings"), BrowserTabScope::Global).unwrap();
        let owner_a = resolve_browser_tab_id(
            Some("settings"),
            BrowserTabScope::OwnerSession {
                owner_appid: "app.demo",
                owner_session_id: 1,
            },
        )
        .unwrap();
        let owner_b = resolve_browser_tab_id(
            Some("settings"),
            BrowserTabScope::OwnerSession {
                owner_appid: "app.demo",
                owner_session_id: 2,
            },
        )
        .unwrap();

        assert_eq!(global_a, global_b);
        assert_ne!(global_a, owner_a);
        assert_ne!(owner_a, owner_b);
    }

    #[test]
    fn stable_browser_tab_ids_reject_invalid_keys() {
        let result = resolve_browser_tab_id(Some("settings/main"), BrowserTabScope::Global);
        assert!(matches!(result, Err(LxAppError::InvalidParameter(_))));
    }

    #[test]
    fn runtime_tab_id_lookup_normalizes_stable_keys() {
        assert_eq!(
            normalize_runtime_tab_id("settings"),
            Some("settings".to_string())
        );
        assert_eq!(
            normalize_runtime_tab_id("SeTtings"),
            Some("settings".to_string())
        );
        assert!(normalize_runtime_tab_id("settings/main").is_none());
    }
}
