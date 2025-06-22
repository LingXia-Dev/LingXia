use miniapp::{MiniAppError, WebViewController};

#[derive(Debug)]
pub struct WebViewInner {
    webtag: String,
}

unsafe impl Send for WebViewInner {}
unsafe impl Sync for WebViewInner {}

impl WebViewInner {
    /// Create a new WebView instance for HarmonyOS
    pub fn create(appid: &str, path: &str) -> Result<Self, MiniAppError> {
        Ok(WebViewInner {
            webtag: appid.to_string(),
        })
    }
}

impl WebViewController for WebViewInner {
    fn load_url(&self, _url: String) -> Result<(), MiniAppError> {
        Ok(())
    }

    fn evaluate_javascript(&self, _js: String) -> Result<(), MiniAppError> {
        Ok(())
    }

    fn set_devtools(&self, _enabled: bool) -> Result<(), MiniAppError> {
        Ok(())
    }

    fn clear_browsing_data(&self) -> Result<(), MiniAppError> {
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
