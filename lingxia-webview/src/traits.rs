use crate::{LogLevel, WebViewError};
use std::path::PathBuf;

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

    /// Enable or disable scroll event listener with optional throttle time
    /// When enabled, scroll events will be sent to the native layer
    fn set_scroll_listener_enabled(
        &self,
        enabled: bool,
        throttle_ms: Option<u64>,
    ) -> Result<(), WebViewError>;
}

/// WebView delegate trait - focused on WebView events only
pub trait WebViewDelegate: Send + Sync {
    /// Called when the page starts loading
    fn on_page_started(&self);

    /// Called when the page finishes loading
    fn on_page_finished(&self);

    /// Called when Scroll changed
    fn on_page_scroll_changed(
        &self,
        scroll_x: i32,
        scroll_y: i32,
        max_scroll_x: i32,
        max_scroll_y: i32,
    );

    /// Handles a postMessage from the page View(WebView)
    fn handle_post_message(&self, msg: String);

    /// Handles an HTTP request from the page
    fn handle_request(&self, req: http::Request<Vec<u8>>) -> Option<WebResourceResponse>;

    /// Receive log from WebView
    fn log(&self, level: LogLevel, message: &str);
}

/// Represents an HTTP response whose body data is stored in a file on disk.
#[derive(Debug)]
pub struct WebResourceResponse {
    parts: http::response::Parts,
    file_path: PathBuf,
}

impl WebResourceResponse {
    /// Create a new WebResourceResponse from response parts and an absolute file path.
    pub fn new(parts: http::response::Parts, file_path: PathBuf) -> Self {
        Self { parts, file_path }
    }

    /// Borrow the response parts (status, headers, etc.).
    pub fn parts(&self) -> &http::response::Parts {
        &self.parts
    }

    /// Consume the struct and return the owned parts and file path.
    pub fn into_parts(self) -> (http::response::Parts, PathBuf) {
        (self.parts, self.file_path)
    }

    /// Borrow the file path where the response body is stored.
    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }
}
