use crate::{LogLevel, WebViewError, WebViewInputError, WebViewScriptError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

/// Outcome of handling a scheme request.
#[derive(Debug)]
pub enum SchemeOutcome {
    /// Handler produced a response.
    Handled(WebResourceResponse),
    /// Handler intentionally declined the request.
    PassThrough,
}

/// Async scheme handler signature.
pub(crate) type AsyncSchemeFuture = Pin<Box<dyn Future<Output = SchemeOutcome> + Send + 'static>>;
pub(crate) type AsyncSchemeHandler =
    Arc<dyn Fn(http::Request<Vec<u8>>) -> AsyncSchemeFuture + Send + Sync>;

/// Navigation policy decision returned by the navigation handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationPolicy {
    /// Allow the WebView to navigate to this URL.
    Allow,
    /// Cancel the navigation. The handler is responsible for any side effects
    /// (e.g., opening the URL externally via `AppRuntime::open_url()`).
    Cancel,
}

/// New-window policy decision returned by the new-window handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewWindowPolicy {
    /// Load the URL in the current WebView (replaces current page).
    LoadInSelf,
    /// Cancel the new-window request without doing anything.
    Cancel,
}

pub type NavigationHandler = Box<dyn Fn(&str) -> NavigationPolicy + Send + Sync>;
pub type NewWindowHandler = Box<dyn Fn(&str) -> NewWindowPolicy + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadRequest {
    /// Final download URL reported by the platform callback.
    pub url: String,
    /// Request user-agent if available on this platform.
    pub user_agent: Option<String>,
    /// `Content-Disposition` response header if exposed by the platform.
    pub content_disposition: Option<String>,
    /// Response MIME type if exposed by the platform.
    pub mime_type: Option<String>,
    /// Response content length if known.
    pub content_length: Option<u64>,
    /// Platform-suggested filename (may be absent).
    pub suggested_filename: Option<String>,
    /// Source page URL that initiated the download when available.
    pub source_page_url: Option<String>,
    /// Cookie header string for `url` when available.
    pub cookie: Option<String>,
}

/// Download callback.
///
/// In browser profile, registering this callback makes download requests flow through the host
/// app callback path instead of in-WebView download UI.
pub type DownloadHandler = Box<dyn Fn(DownloadRequest) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WebViewCookieSameSite {
    Lax,
    Strict,
    None,
}

impl WebViewCookieSameSite {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lax => "lax",
            Self::Strict => "strict",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebViewCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub host_only: bool,
    #[serde(default)]
    pub secure: bool,
    #[serde(default)]
    pub http_only: bool,
    #[serde(default)]
    pub session: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_unix_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_site: Option<WebViewCookieSameSite>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebViewCookieSetRequest {
    #[serde(default)]
    pub url: String,
    pub name: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default = "default_cookie_path")]
    pub path: String,
    #[serde(default)]
    pub secure: bool,
    #[serde(default)]
    pub http_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_unix_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_site: Option<WebViewCookieSameSite>,
}

fn default_cookie_path() -> String {
    "/".to_string()
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChooserRequest {
    /// Accepted MIME types / extensions requested by the page.
    pub accept_types: Vec<String>,
    /// Whether multiple files may be selected.
    pub allow_multiple: bool,
    /// Whether directories may be selected.
    pub allow_directories: bool,
    /// Whether the page requested capture/live media.
    pub capture: bool,
    /// Source page URL that initiated the chooser when available.
    pub source_page_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChooserFile {
    pub path: Option<String>,
    pub uri: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChooserResponse {
    Cancel,
    Error(String),
    Files(Vec<FileChooserFile>),
}

/// Body source for WebResourceResponse
#[derive(Debug)]
pub enum WebResourceBody {
    /// Serve data from a regular file path on disk
    Path(PathBuf),
    /// Serve data from a system pipe (read end)
    Pipe(SystemPipeReader),
    /// Serve data directly from memory
    Bytes(Vec<u8>),
}

/// Cross‑platform system pipe reader (read end)
#[derive(Debug)]
pub struct SystemPipeReader {
    #[cfg(unix)]
    fd: std::os::fd::RawFd,
    #[cfg(windows)]
    handle: std::os::windows::io::RawHandle,
}

impl SystemPipeReader {
    /// Consume and return the raw file descriptor (Unix).
    /// Caller becomes responsible for closing it.
    #[cfg(unix)]
    pub fn into_raw_fd(self) -> std::os::fd::RawFd {
        self.fd
    }

    /// Construct from a raw file descriptor (Unix).
    ///
    /// # Safety
    ///
    /// Caller guarantees that `fd` is a valid read end of a pipe file descriptor.
    #[cfg(unix)]
    pub unsafe fn from_raw_fd(fd: std::os::fd::RawFd) -> Self {
        Self { fd }
    }

    /// Convert into a File for reading (consumes self).
    #[cfg(unix)]
    pub fn into_file(self) -> std::fs::File {
        use std::os::fd::FromRawFd;
        unsafe { std::fs::File::from_raw_fd(self.into_raw_fd()) }
    }

    /// Consume and return the raw handle (Windows).
    /// Caller becomes responsible for closing it.
    #[cfg(windows)]
    pub fn into_raw_handle(self) -> std::os::windows::io::RawHandle {
        self.handle
    }

    /// Construct from a raw handle (Windows).
    ///
    /// # Safety
    ///
    /// Caller guarantees that `handle` is a valid readable OS handle.
    #[cfg(windows)]
    pub unsafe fn from_raw_handle(handle: std::os::windows::io::RawHandle) -> Self {
        Self { handle }
    }

    /// Convert into a File for reading (consumes self).
    #[cfg(windows)]
    pub fn into_file(self) -> std::fs::File {
        use std::os::windows::io::FromRawHandle;
        unsafe { std::fs::File::from_raw_handle(self.into_raw_handle()) }
    }
}

/// Interface for controlling WebView (100% copy from lxapp)
#[async_trait]
pub trait WebViewController: Send + Sync {
    /// Load a URL in the WebView
    fn load_url(&self, url: &str) -> Result<(), WebViewError>;

    /// Load HTML data into the WebView.
    fn load_data(&self, request: LoadDataRequest<'_>) -> Result<(), WebViewError>;

    /// Execute JavaScript in the WebView without observing its return value.
    fn exec_js(&self, js: &str) -> Result<(), WebViewError>;

    /// Evaluate JavaScript in the WebView and return the decoded JSON value.
    ///
    /// Implementations are required to be both CSP-safe (no `(0,eval)` /
    /// `new Function` — pages whose CSP omits `'unsafe-eval'` must still
    /// work) and `await`-aware (top-level `await` in the user expression
    /// resolves before the future returns). Platforms achieve this by
    /// dispatching through the native await-capable API
    /// (`callAsyncJavaScript:` on Apple, `LingXiaProxy.resolveEval` JS
    /// bridge on Android/Harmony).
    async fn eval_js(&self, js: &str) -> Result<serde_json::Value, WebViewScriptError>;

    /// Return the platform WebView's current URL.
    async fn current_url(&self) -> Result<Option<String>, WebViewError> {
        Err(WebViewError::WebView(
            "current_url is not implemented for this platform".to_string(),
        ))
    }

    /// Post a message to the WebView
    fn post_message(&self, message: &str) -> Result<(), WebViewError>;

    /// Clear browsing data from the WebView
    fn clear_browsing_data(&self) -> Result<(), WebViewError>;

    /// Set the user agent string for the WebView
    fn set_user_agent(&self, ua: &str) -> Result<(), WebViewError>;

    /// Reload the current WebView document.
    fn reload(&self) -> Result<(), WebViewError> {
        Err(WebViewError::WebView(
            "reload is not implemented for this platform".to_string(),
        ))
    }

    /// Navigate back in WebView history.
    fn go_back(&self) -> Result<(), WebViewError> {
        Err(WebViewError::WebView(
            "go_back is not implemented for this platform".to_string(),
        ))
    }

    /// Navigate forward in WebView history.
    fn go_forward(&self) -> Result<(), WebViewError> {
        Err(WebViewError::WebView(
            "go_forward is not implemented for this platform".to_string(),
        ))
    }

    /// List HTTP cookies from the platform WebView cookie store.
    async fn list_cookies(&self) -> Result<Vec<WebViewCookie>, WebViewError> {
        Err(WebViewError::WebView(
            "cookie store is not implemented for this platform".to_string(),
        ))
    }

    /// Set an HTTP cookie through the platform WebView cookie store.
    async fn set_cookie(&self, _request: WebViewCookieSetRequest) -> Result<(), WebViewError> {
        Err(WebViewError::WebView(
            "cookie store is not implemented for this platform".to_string(),
        ))
    }

    /// Delete an HTTP cookie from the platform WebView cookie store.
    async fn delete_cookie(
        &self,
        _name: &str,
        _domain: &str,
        _path: &str,
    ) -> Result<(), WebViewError> {
        Err(WebViewError::WebView(
            "cookie store is not implemented for this platform".to_string(),
        ))
    }

    /// Clear all HTTP cookies from the platform WebView cookie store.
    async fn clear_cookies(&self) -> Result<(), WebViewError> {
        Err(WebViewError::WebView(
            "cookie store is not implemented for this platform".to_string(),
        ))
    }

    /// Clear data owned by the current website without clearing the shared
    /// browser profile. Platforms report whether their network cache supports
    /// site-scoped removal.
    async fn clear_site_data(
        &self,
        _url: &str,
        _options: ClearSiteDataOptions,
    ) -> Result<ClearSiteDataResult, WebViewError> {
        Err(WebViewError::WebView(
            "site-scoped data clearing is not implemented for this platform".to_string(),
        ))
    }

    /// Capture a PNG screenshot of the WebView's visible content.
    /// Returns raw PNG-encoded bytes ready to be base64'd over the wire.
    async fn take_screenshot(&self) -> Result<Vec<u8>, WebViewError> {
        Err(WebViewError::WebView(
            "screenshot is not implemented for this platform".to_string(),
        ))
    }

    /// Begin recording network requests/responses into a bounded per-webview
    /// buffer, retrievable via [`Self::network_entries`]. Dev-tooling only;
    /// implemented on platforms whose WebView exposes an inspection protocol
    /// (currently Windows/WebView2 via the Chrome DevTools Protocol).
    async fn start_network_capture(&self) -> Result<(), WebViewError> {
        Err(WebViewError::WebView(
            "network capture is not implemented for this platform".to_string(),
        ))
    }

    /// Stop recording network traffic. Captured entries are kept until
    /// [`Self::clear_network_capture`] or the webview is torn down.
    async fn stop_network_capture(&self) -> Result<(), WebViewError> {
        Err(WebViewError::WebView(
            "network capture is not implemented for this platform".to_string(),
        ))
    }

    /// Snapshot the captured network entries (oldest first). `dropped` counts
    /// entries evicted from the ring buffer since the last clear.
    async fn network_entries(&self) -> Result<NetworkCaptureSnapshot, WebViewError> {
        Err(WebViewError::WebView(
            "network capture is not implemented for this platform".to_string(),
        ))
    }

    /// Drop all captured entries (leaves capture enabled if it was on).
    async fn clear_network_capture(&self) -> Result<(), WebViewError> {
        Err(WebViewError::WebView(
            "network capture is not implemented for this platform".to_string(),
        ))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ClearSiteDataOptions {
    pub cache: bool,
    pub site_data: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ClearSiteDataResult {
    pub cache_cleared: bool,
    pub site_data_cleared: bool,
}

/// One captured network request and its response (when it completed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEntry {
    /// Protocol request id, stable across the request/response events.
    pub request_id: String,
    pub url: String,
    pub method: String,
    /// Resource kind reported by the engine (document, xhr, fetch, script,
    /// image, ...), when available.
    pub resource_type: Option<String>,
    pub request_headers: Vec<(String, String)>,
    /// Request payload (POST body) as reported by the engine, when present.
    pub request_body: Option<String>,
    pub status: Option<u16>,
    pub response_headers: Vec<(String, String)>,
    pub mime_type: Option<String>,
    pub response_body: NetworkBody,
    pub from_cache: bool,
    /// Populated when the request failed (engine error text) instead of
    /// producing a response.
    pub failed: Option<String>,
    /// Wall-clock start time (Unix epoch seconds), when the engine reports it.
    pub wall_time: Option<f64>,
    /// Monotonic engine timestamps (seconds), for ordering and durations.
    pub started: f64,
    pub finished: Option<f64>,
}

impl NetworkEntry {
    /// Request duration in milliseconds, once the response has completed.
    pub fn duration_ms(&self) -> Option<f64> {
        self.finished
            .filter(|finished| *finished >= self.started)
            .map(|finished| (finished - self.started) * 1000.0)
    }
}

/// Response body of a captured entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NetworkBody {
    /// No body captured yet (in flight) or the response had none.
    #[default]
    None,
    /// UTF-8 text body.
    Text { text: String },
    /// Base64-encoded binary body.
    Base64 { base64: String },
    /// Body deliberately not captured (e.g. over the size cap, or evicted
    /// before it could be read); `reason` says which.
    Skipped { reason: String },
}

/// A point-in-time view of the capture buffer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkCaptureSnapshot {
    pub entries: Vec<NetworkEntry>,
    /// Entries evicted from the ring buffer since the last clear (buffer
    /// full). Surfaced so truncation is never silent.
    pub dropped: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClickOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypeOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    #[serde(default)]
    pub replace: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FillOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PressOptions;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScrollOptions;

#[async_trait]
pub trait WebViewInputController: WebViewController {
    async fn click(
        &self,
        _selector: &str,
        _options: ClickOptions,
    ) -> Result<(), WebViewInputError> {
        Err(WebViewInputError::Unsupported(
            "input control is not implemented for this platform",
        ))
    }

    async fn type_text(
        &self,
        _selector: &str,
        _text: &str,
        _options: TypeOptions,
    ) -> Result<(), WebViewInputError> {
        Err(WebViewInputError::Unsupported(
            "input control is not implemented for this platform",
        ))
    }

    async fn fill(
        &self,
        _selector: &str,
        _text: &str,
        _options: FillOptions,
    ) -> Result<(), WebViewInputError> {
        Err(WebViewInputError::Unsupported(
            "input control is not implemented for this platform",
        ))
    }

    async fn press(&self, _key: &str, _options: PressOptions) -> Result<(), WebViewInputError> {
        Err(WebViewInputError::Unsupported(
            "input control is not implemented for this platform",
        ))
    }

    async fn scroll(
        &self,
        _dx: f64,
        _dy: f64,
        _options: ScrollOptions,
    ) -> Result<(), WebViewInputError> {
        Err(WebViewInputError::Unsupported(
            "input control is not implemented for this platform",
        ))
    }

    async fn scroll_to(
        &self,
        _selector: &str,
        _options: ScrollOptions,
    ) -> Result<(), WebViewInputError> {
        Err(WebViewInputError::Unsupported(
            "input control is not implemented for this platform",
        ))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LoadDataRequest<'a> {
    pub data: &'a str,
    pub base_url: &'a str,
    pub history_url: Option<&'a str>,
}

impl<'a> LoadDataRequest<'a> {
    pub fn new(data: &'a str, base_url: &'a str) -> Self {
        Self {
            data,
            base_url,
            history_url: None,
        }
    }

    pub fn with_history_url(mut self, history_url: &'a str) -> Self {
        self.history_url = Some(history_url);
        self
    }
}

/// Normalized category for a main-frame page load failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadErrorKind {
    Dns,
    Network,
    Timeout,
    Security,
    Cancelled,
    InvalidUrl,
    NotFound,
    Unknown,
}

/// Error reported when a main-frame page load fails (DNS, network, TLS, etc.).
///
/// The webview crate is responsible only for delivering this event.
/// What to display is entirely up to the caller.
#[derive(Debug, Clone)]
pub struct LoadError {
    /// URL that failed to load, if the platform exposes it.
    pub url: Option<String>,
    /// Cross-platform error category for application logic and UI.
    pub kind: LoadErrorKind,
    /// Human-readable description from the platform.
    pub description: String,
}

/// WebView delegate trait - focused on WebView events only
pub trait WebViewDelegate: Send + Sync {
    /// Called when the page starts loading
    fn on_page_started(&self);

    /// Called when the page finishes loading
    fn on_page_finished(&self);

    /// Called after a successful top-level navigation with its final URL.
    /// Default is a no-op for platforms that do not expose the URL here.
    fn on_navigation_finished(&self, _url: &str) {}

    /// Called when a main-frame page load fails (e.g. DNS failure, network unreachable, TLS error).
    ///
    /// Only fires for the main document; sub-resource errors are ignored.
    /// Default is a no-op so existing implementations do not need to change.
    fn on_load_error(&self, _error: &LoadError) {}

    /// Called when the document title changes (where the platform reports
    /// it; currently Windows/WebView2). Default is a no-op so existing
    /// implementations do not need to change.
    fn on_title_changed(&self, _title: &str) {}

    /// Called when the page favicon changes (where the platform reports it;
    /// currently Windows/WebView2). `png_bytes` holds the favicon encoded
    /// as PNG; an empty vector means the page has no favicon. Default is a
    /// no-op so existing implementations do not need to change.
    fn on_favicon_changed(&self, _png_bytes: Vec<u8>) {}

    /// Called when the webview's session history changes, with the new
    /// back/forward availability. Platforms whose host layer observes this
    /// separately (e.g. macOS KVO on `canGoBack`) do not call it. Default is
    /// a no-op so existing implementations do not need to change.
    fn on_history_changed(&self, _can_go_back: bool, _can_go_forward: bool) {}

    /// Called when the webview's document URL changes — including history
    /// navigations, redirects, and same-document updates. Platforms whose
    /// host layer observes this separately (e.g. macOS KVO on `url`) do not
    /// call it. Default is a no-op so existing implementations do not need
    /// to change.
    fn on_url_changed(&self, _url: &str) {}

    /// Handles a postMessage from the page View(WebView)
    fn handle_post_message(&self, msg: String);

    /// Handles a native-component message posted by the page through the
    /// embedded-component channel (`window.NativeComponentBridge`), where
    /// the platform routes it in-process (currently Windows/WebView2).
    /// `message_json` is the raw component message (`component.mount`,
    /// `component.update`, ...). Default is a no-op so existing
    /// implementations do not need to change.
    fn handle_native_component_message(&self, message_json: &str) {
        let _ = message_json;
    }

    /// Receive log from WebView
    fn log(&self, level: LogLevel, message: &str);
}

/// Represents an HTTP response whose body is provided by a file path, pipe, or in-memory bytes.
#[derive(Debug)]
pub struct WebResourceResponse {
    parts: http::response::Parts,
    body: WebResourceBody,
}

impl From<Option<WebResourceResponse>> for SchemeOutcome {
    fn from(value: Option<WebResourceResponse>) -> Self {
        match value {
            Some(response) => SchemeOutcome::Handled(response),
            None => SchemeOutcome::PassThrough,
        }
    }
}

impl WebResourceResponse {
    /// Borrow the response parts (status, headers, etc.).
    pub fn parts(&self) -> &http::response::Parts {
        &self.parts
    }

    /// Consume the struct and return the owned parts and file path.
    pub fn into_parts(self) -> (http::response::Parts, WebResourceBody) {
        (self.parts, self.body)
    }
}

/// Convenience conversion from (Parts, PathBuf)
impl From<(http::response::Parts, PathBuf)> for WebResourceResponse {
    fn from(value: (http::response::Parts, PathBuf)) -> Self {
        WebResourceResponse {
            parts: value.0,
            body: WebResourceBody::Path(value.1),
        }
    }
}

/// Convenience conversion from (Parts, SystemPipeReader)
impl From<(http::response::Parts, SystemPipeReader)> for WebResourceResponse {
    fn from(value: (http::response::Parts, SystemPipeReader)) -> Self {
        WebResourceResponse {
            parts: value.0,
            body: WebResourceBody::Pipe(value.1),
        }
    }
}

/// Convenience conversion from (Parts, Vec<u8>)
impl From<(http::response::Parts, Vec<u8>)> for WebResourceResponse {
    fn from(value: (http::response::Parts, Vec<u8>)) -> Self {
        WebResourceResponse {
            parts: value.0,
            body: WebResourceBody::Bytes(value.1),
        }
    }
}

impl WebResourceResponse {
    fn response_parts_with_status(status: u16) -> http::response::Parts {
        let response = match http::Response::builder().status(status).body(()) {
            Ok(response) => response,
            Err(_) => http::Response::new(()),
        };
        let (parts, _) = response.into_parts();
        parts
    }

    /// Create a response serving a file from disk (status 200).
    pub fn file(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let content_length = std::fs::metadata(&path).ok().map(|m| m.len());
        let mut parts = Self::response_parts_with_status(200);
        if let Some(len) = content_length {
            parts
                .headers
                .insert(http::header::CONTENT_LENGTH, http::HeaderValue::from(len));
        }
        Self {
            parts,
            body: WebResourceBody::Path(path),
        }
    }

    /// Create a response serving in-memory bytes (status 200).
    pub fn bytes(data: impl Into<Vec<u8>>) -> Self {
        let data = data.into();
        let len = data.len();
        let mut parts = Self::response_parts_with_status(200);
        parts
            .headers
            .insert(http::header::CONTENT_LENGTH, http::HeaderValue::from(len));
        Self {
            parts,
            body: WebResourceBody::Bytes(data),
        }
    }

    /// Create a response serving data from a system pipe (status 200).
    pub fn stream(reader: SystemPipeReader) -> Self {
        let parts = Self::response_parts_with_status(200);
        Self {
            parts,
            body: WebResourceBody::Pipe(reader),
        }
    }

    /// Set the Content-Type header (builder pattern).
    pub fn mime(mut self, content_type: &str) -> Self {
        if let Ok(value) = http::HeaderValue::from_str(content_type) {
            self.parts.headers.insert(http::header::CONTENT_TYPE, value);
        }
        self
    }

    /// Set the HTTP status code (builder pattern).
    pub fn status(mut self, code: u16) -> Self {
        self.parts.status = http::StatusCode::from_u16(code).unwrap_or(self.parts.status);
        self
    }

    /// Add a response header (builder pattern).
    pub fn header(mut self, name: &str, value: &str) -> Self {
        if let (Ok(header_name), Ok(header_value)) = (
            name.parse::<http::header::HeaderName>(),
            http::HeaderValue::from_str(value),
        ) {
            self.parts.headers.insert(header_name, header_value);
        }
        self
    }

    /// Add CORS header `Access-Control-Allow-Origin: null` (builder pattern).
    pub fn cors(self) -> Self {
        self.header("access-control-allow-origin", "null")
    }
}
