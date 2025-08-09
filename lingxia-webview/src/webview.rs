use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};

use lxapp::{LxAppError, WebViewController};

use crate::WebViewInner;

/// Global WebView instances storage
static WEBVIEW_INSTANCES: OnceLock<Arc<Mutex<HashMap<String, Arc<WebViewInner>>>>> =
    OnceLock::new();

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
pub fn create_webview(
    appid: String,
    path: String,
    sender: Sender<Result<Arc<dyn WebViewController>, LxAppError>>,
) {
    let webtag = WebTag::new(&appid, &path);

    // Get or initialize the global instances map
    let instances = WEBVIEW_INSTANCES.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));

    // Check if WebView already exists
    if let Ok(webviews) = instances.lock() {
        if let Some(existing_webview) = webviews.get(webtag.as_str()) {
            log::info!("WebView already exists, reusing: {}-{}", appid, path);
            let _ = sender.send(Ok(existing_webview.clone()));
            return;
        }
    }

    // Delegate WebView creation to the platform-specific implementation
    WebViewInner::create(&appid, &path, sender);
}

pub(crate) fn register_webview(webtag: &WebTag, webview: Arc<WebViewInner>) {
    if let Some(instances) = WEBVIEW_INSTANCES.get() {
        if let Ok(mut webviews) = instances.lock() {
            webviews.insert(webtag.as_str().to_string(), webview);
            log::info!("WebView created and stored: {}", webtag.as_str());
        }
    }
}

/// Find WebView by appid and path
pub fn find_webview(appid: &str, path: &str) -> Option<Arc<WebViewInner>> {
    let webtag = WebTag::new(appid, path);
    find_webview_by_tag(&webtag)
}

/// Find WebView by WebTag
pub fn find_webview_by_tag(webtag: &WebTag) -> Option<Arc<WebViewInner>> {
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
