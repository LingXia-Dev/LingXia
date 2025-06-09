use miniapp::{MiniAppError, WebViewController};
use super::ffi;

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

        Ok(WebViewInner {
            swift_webview_ptr,
        })
    }

    pub(crate) fn get_swift_webview_ptr(&self) -> usize {
        self.swift_webview_ptr
    }
}

impl WebViewController for WebViewInner {
    fn load_url(&self, _url: String) -> Result<(), MiniAppError> {
        Ok(())
    }

    fn evaluate_javascript(&self, _js: String) -> Result<(), MiniAppError> {
        Ok(())
    }

    fn clear_browsing_data(&self) -> Result<(), MiniAppError> {
        Ok(())
    }

    fn set_devtools(&self, _enabled: bool) -> Result<(), MiniAppError> {
        Ok(())
    }

    fn post_message(&self, _message: String) -> Result<(), MiniAppError> {
        Ok(())
    }

    fn set_user_agent(&self, _ua: String) -> Result<(), MiniAppError> {
        Ok(())
    }

    fn set_scroll_listener_enabled(
        &self,
        _enabled: bool,
        _throttle_ms: Option<u64>,
    ) -> Result<(), MiniAppError> {
        Ok(())
    }
}
