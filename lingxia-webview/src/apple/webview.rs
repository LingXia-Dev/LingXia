use super::ffi;
use miniapp::{MiniAppError, WebViewController};

#[derive(Clone, Debug)]
pub struct WebViewInner {
    swift_webview_ptr: usize,
}

unsafe impl Send for WebViewInner {}
unsafe impl Sync for WebViewInner {}

impl WebViewInner {
    /// Create a new WebView by calling Swift to create and get pointer as usize
    pub(crate) fn create(appid: &str, path: &str) -> Result<Self, MiniAppError> {
        let swift_webview_ptr = ffi::create_webview_ptr(appid, path);

        if swift_webview_ptr == 0 {
            return Err(MiniAppError::WebView(format!(
                "Failed to create WebView for appid={}, path={}",
                appid, path
            )));
        }

        Ok(WebViewInner { swift_webview_ptr })
    }

    pub(crate) fn get_swift_webview_ptr(&self) -> usize {
        self.swift_webview_ptr
    }
}

impl WebViewController for WebViewInner {
    fn load_url(&self, url: String) -> Result<(), MiniAppError> {
        if ffi::webview_load_url(self.swift_webview_ptr, &url) {
            Ok(())
        } else {
            Err(MiniAppError::WebView(format!(
                "Failed to load URL: {}",
                url
            )))
        }
    }

    fn evaluate_javascript(&self, js: String) -> Result<(), MiniAppError> {
        if ffi::webview_evaluate_javascript(self.swift_webview_ptr, &js) {
            Ok(())
        } else {
            Err(MiniAppError::WebView(
                "Failed to evaluate JavaScript".to_string(),
            ))
        }
    }

    fn clear_browsing_data(&self) -> Result<(), MiniAppError> {
        if ffi::webview_clear_browsing_data(self.swift_webview_ptr) {
            Ok(())
        } else {
            Err(MiniAppError::WebView(
                "Failed to clear browsing data".to_string(),
            ))
        }
    }

    fn set_devtools(&self, enabled: bool) -> Result<(), MiniAppError> {
        if ffi::webview_set_devtools(self.swift_webview_ptr, enabled) {
            Ok(())
        } else {
            Err(MiniAppError::WebView("Failed to set devtools".to_string()))
        }
    }

    fn post_message(&self, message: String) -> Result<(), MiniAppError> {
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

    fn set_user_agent(&self, ua: String) -> Result<(), MiniAppError> {
        if ffi::webview_set_user_agent(self.swift_webview_ptr, &ua) {
            Ok(())
        } else {
            Err(MiniAppError::WebView(
                "Failed to set user agent".to_string(),
            ))
        }
    }

    fn set_scroll_listener_enabled(
        &self,
        enabled: bool,
        throttle_ms: Option<u64>,
    ) -> Result<(), MiniAppError> {
        let throttle = throttle_ms.unwrap_or(100); // Default 100ms throttle
        if ffi::webview_set_scroll_listener_enabled(self.swift_webview_ptr, enabled, throttle) {
            Ok(())
        } else {
            Err(MiniAppError::WebView(
                "Failed to set scroll listener".to_string(),
            ))
        }
    }
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        // Call Swift to destroy the WebView
        if self.swift_webview_ptr != 0 {
            ffi::webview_destroy(self.swift_webview_ptr);
            self.swift_webview_ptr = 0;
        }
    }
}
