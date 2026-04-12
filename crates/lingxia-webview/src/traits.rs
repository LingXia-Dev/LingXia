use crate::{LogLevel, WebViewError};
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
}

/// Interface for controlling WebView (100% copy from lxapp)
pub trait WebViewController: Send + Sync {
    /// Load a URL in the WebView
    fn load_url(&self, url: &str) -> Result<(), WebViewError>;

    /// Load HTML data into the WebView.
    fn load_data(&self, request: LoadDataRequest<'_>) -> Result<(), WebViewError>;

    /// Evaluate JavaScript in the WebView
    fn evaluate_javascript(&self, js: &str) -> Result<(), WebViewError>;

    /// Post a message to the WebView
    fn post_message(&self, message: &str) -> Result<(), WebViewError>;

    /// Clear browsing data from the WebView
    fn clear_browsing_data(&self) -> Result<(), WebViewError>;

    /// Set the user agent string for the WebView
    fn set_user_agent(&self, ua: &str) -> Result<(), WebViewError>;
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

    /// Called when a main-frame page load fails (e.g. DNS failure, network unreachable, TLS error).
    ///
    /// Only fires for the main document; sub-resource errors are ignored.
    /// Default is a no-op so existing implementations do not need to change.
    fn on_load_error(&self, _error: &LoadError) {}

    /// Handles a postMessage from the page View(WebView)
    fn handle_post_message(&self, msg: String);

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
