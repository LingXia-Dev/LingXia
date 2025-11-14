use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

#[cfg(target_os = "android")]
use crate::android::WebViewInner;

#[cfg(any(target_os = "ios", target_os = "macos"))]
use crate::apple::WebViewInner;

#[cfg(all(target_os = "linux", target_env = "ohos"))]
use crate::harmony::WebViewInner;

use crate::{WebViewController, WebViewDelegate, WebViewError};

/// WebView type that includes inner implementation and delegate
pub struct WebView {
    pub(crate) inner: WebViewInner,
    appid: String,
    path: String,
    // Hold a strong reference to the delegate; PageInner::drop removes it to break cycles
    delegate: RwLock<Option<Arc<dyn WebViewDelegate>>>,
}

impl WebView {
    pub(crate) fn new(inner: WebViewInner, appid: String, path: String) -> Self {
        Self {
            inner,
            appid,
            path,
            delegate: RwLock::new(None),
        }
    }

    /// Get the appid
    pub fn appid(&self) -> &str {
        &self.appid
    }

    /// Get the path
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Get the webtag (computed from appid and path)
    pub fn webtag(&self) -> WebTag {
        WebTag::new(&self.appid, &self.path)
    }

    /// Set delegate for this WebView
    pub fn set_delegate(&self, delegate: Arc<dyn WebViewDelegate>) {
        if let Ok(mut guard) = self.delegate.write() {
            *guard = Some(delegate);
        }
    }

    /// Get delegate for this WebView
    pub fn get_delegate(&self) -> Option<Arc<dyn WebViewDelegate>> {
        self.delegate.read().ok().and_then(|guard| guard.clone())
    }

    /// Remove delegate for this WebView
    pub fn remove_delegate(&self) {
        if let Ok(mut guard) = self.delegate.write() {
            *guard = None;
        }
    }

    /// Get platform-specific pointer for interop (Apple platforms only)
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    pub fn get_swift_webview_ptr(&self) -> usize {
        self.inner.get_swift_webview_ptr()
    }

    /// Get Java WebView reference (Android only)
    #[cfg(target_os = "android")]
    pub fn get_java_webview(&self) -> &jni::objects::GlobalRef {
        self.inner.get_java_webview()
    }
}

impl WebViewController for WebView {
    fn load_url(&self, url: String) -> Result<(), WebViewError> {
        self.inner.load_url(url)
    }

    fn load_data(
        &self,
        data: String,
        base_url: String,
        history_url: Option<String>,
    ) -> Result<(), WebViewError> {
        self.inner.load_data(data, base_url, history_url)
    }

    fn evaluate_javascript(&self, js: String) -> Result<(), WebViewError> {
        self.inner.evaluate_javascript(js)
    }

    fn post_message(&self, message: String) -> Result<(), WebViewError> {
        self.inner.post_message(message)
    }

    fn clear_browsing_data(&self) -> Result<(), WebViewError> {
        self.inner.clear_browsing_data()
    }

    fn set_user_agent(&self, ua: String) -> Result<(), WebViewError> {
        self.inner.set_user_agent(ua)
    }

    fn set_scroll_listener_enabled(
        &self,
        enabled: bool,
        throttle_ms: Option<u64>,
    ) -> Result<(), WebViewError> {
        self.inner.set_scroll_listener_enabled(enabled, throttle_ms)
    }
}

/// Global WebView instances storage
static WEBVIEW_INSTANCES: OnceLock<Arc<Mutex<HashMap<String, Arc<WebView>>>>> = OnceLock::new();

/// WebView identifier combining appid and path
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WebTag(String);

impl std::fmt::Display for WebTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl WebTag {
    pub fn new(appid: &str, path: &str) -> Self {
        Self(format!("{}-{}", appid, path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Extract appid from the webtag
    pub fn extract_appid(&self) -> String {
        self.0.split('-').next().unwrap_or("").to_string()
    }

    /// Extract appid and path from WebTag
    /// This will always succeed since WebTag is constructed with a valid format
    pub fn extract_parts(&self) -> (String, String) {
        self.0
            .split_once('-')
            .map(|(appid, path)| (appid.to_string(), path.to_string()))
            .unwrap()
    }
}

impl From<&str> for WebTag {
    fn from(webtag_str: &str) -> Self {
        Self(webtag_str.to_string())
    }
}

/// Initialize WebView manager
pub fn init_webview_manager() {
    let _ = WEBVIEW_INSTANCES.set(Arc::new(Mutex::new(HashMap::new())));
}

/// Create a WebView instance asynchronously with channel sender
pub fn create_webview(webtag: &WebTag, sender: Sender<Result<Arc<WebView>, WebViewError>>) {
    let (appid, path) = webtag.extract_parts();
    log::info!("Creating WebView for webtag: {}", webtag.as_str());

    // Get or initialize the global instances map
    let instances = WEBVIEW_INSTANCES.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));

    // Check if WebView already exists
    if let Ok(webviews) = instances.lock() {
        if let Some(existing_webview) = webviews.get(webtag.as_str()) {
            log::info!("WebView already exists, reusing: {}", webtag.as_str());
            let _ = sender.send(Ok(existing_webview.clone()));
            return;
        }
    }

    // Delegate WebView creation to the platform-specific implementation
    WebViewInner::create(&appid, &path, sender);
}

pub(crate) fn register_webview(webview: Arc<WebView>) {
    if let Some(instances) = WEBVIEW_INSTANCES.get() {
        if let Ok(mut webviews) = instances.lock() {
            let webtag = webview.webtag();
            webviews.insert(webtag.as_str().to_string(), webview.clone());
            log::info!("WebView created and stored: {}", webtag.as_str());
        }
    }
}

/// Find WebView by WebTag
pub fn find_webview(webtag: &WebTag) -> Option<Arc<WebView>> {
    if let Some(instances) = WEBVIEW_INSTANCES.get() {
        if let Ok(webviews) = instances.lock() {
            webviews.get(webtag.as_str()).cloned()
        } else {
            None
        }
    } else {
        None
    }
}

/// Set delegate for a WebView by WebTag
pub fn set_webview_delegate(webtag: &WebTag, delegate: Arc<dyn WebViewDelegate>) -> bool {
    if let Some(webview) = find_webview(webtag) {
        webview.set_delegate(delegate);
        true
    } else {
        false
    }
}

/// Get delegate from WebView by webtag (for internal use by platform implementations)
pub fn get_webview_delegate(webtag: &WebTag) -> Option<Arc<dyn WebViewDelegate>> {
    if let Some(webview) = find_webview(webtag) {
        webview.get_delegate()
    } else {
        None
    }
}

/// Destroy a WebView instance by WebTag and remove it from global storage
pub fn destroy_webview(webtag: &WebTag) {
    if let Some(instances) = WEBVIEW_INSTANCES.get() {
        if let Ok(mut webviews) = instances.lock() {
            webviews.remove(webtag.as_str());
        }
    }
}
