use miniapp::{MiniAppError, WebViewController};

#[derive(Clone, Debug)]
pub struct WebViewInner {}

impl WebViewInner {
    /// Create a new WebView - empty implementation
    pub(crate) fn create(_appid: &str, _path: &str) -> Result<Self, MiniAppError> {
        todo!();
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
