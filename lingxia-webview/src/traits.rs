use crate::{LogLevel, WebViewError};
use std::path::PathBuf;

/// Body source for WebResourceResponse
#[derive(Debug)]
pub enum WebResourceBody {
    /// Serve data from a regular file path on disk
    Path(PathBuf),
    /// Serve data from a system pipe (read end)
    Pipe(SystemPipeReader),
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
    fn load_url(&self, url: String) -> Result<(), WebViewError>;

    /// Load HTML data into the WebView
    ///
    /// # Arguments
    /// * `data` - The HTML content to load
    /// * `base_url` - Base URL for resolving relative paths in the HTML
    /// * `history_url` - Optional URL to use for history (defaults to base_url if None)
    fn load_data(
        &self,
        data: String,
        base_url: String,
        history_url: Option<String>,
    ) -> Result<(), WebViewError>;

    /// Evaluate JavaScript in the WebView
    fn evaluate_javascript(&self, js: String) -> Result<(), WebViewError>;

    /// Post a message to the WebView
    fn post_message(&self, message: String) -> Result<(), WebViewError> {
        // Escape the JSON message for safe JavaScript injection
        // Since message is already JSON, we need to escape it properly for JS string literal
        let escaped_message = message
            .replace('\\', "\\\\") // Escape backslashes first
            .replace('"', "\\\"") // Escape double quotes
            .replace('\n', "\\n") // Escape newlines
            .replace('\r', "\\r") // Escape carriage returns
            .replace('\t', "\\t"); // Escape tabs

        // Call the global receiver function defined in webview-bridge.js
        let js_code = format!(
            "if (typeof window.__LingXiaRecvMessage === 'function') {{ \
                window.__LingXiaRecvMessage(\"{}\"); \
            }} else {{ \
                console.warn('[LingXia] __LingXiaRecvMessage not available'); \
            }}",
            escaped_message
        );

        // Use evaluateJavaScript to send the message to the WebView
        self.evaluate_javascript(js_code)
    }

    /// Clear browsing data from the WebView
    fn clear_browsing_data(&self) -> Result<(), WebViewError>;

    /// Set the user agent string for the WebView
    fn set_user_agent(&self, ua: String) -> Result<(), WebViewError>;
}

/// WebView delegate trait - focused on WebView events only
pub trait WebViewDelegate: Send + Sync {
    /// Called when the page starts loading
    fn on_page_started(&self);

    /// Called when the page finishes loading
    fn on_page_finished(&self);

    /// Handles a postMessage from the page View(WebView)
    fn handle_post_message(&self, msg: String);

    /// Handles an HTTP request from the page
    fn handle_request(&self, req: http::Request<Vec<u8>>) -> Option<WebResourceResponse>;

    /// Receive log from WebView
    fn log(&self, level: LogLevel, message: &str);
}

/// Represents an HTTP response whose body is provided by either a file path or a system pipe.
#[derive(Debug)]
pub struct WebResourceResponse {
    parts: http::response::Parts,
    body: WebResourceBody,
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
