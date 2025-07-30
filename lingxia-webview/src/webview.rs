use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use miniapp::{LxAppError, WebViewController};

use crate::WebViewInner;

/// Global WebView instances storage
static WEBVIEW_INSTANCES: OnceLock<Arc<Mutex<HashMap<String, Arc<WebViewInner>>>>> =
    OnceLock::new();

/// WebView identifier combining appid and path (internal concept)
fn make_webtag(appid: &str, path: &str) -> String {
    format!("{}-{}", appid, path)
}

/// Initialize WebView manager
pub fn init_webview_manager() {
    let _ = WEBVIEW_INSTANCES.set(Arc::new(Mutex::new(HashMap::new())));
}

/// Create a WebView instance
pub fn create_webview(
    appid: String,
    path: String,
) -> Result<Arc<dyn WebViewController>, LxAppError> {
    let webtag = make_webtag(&appid, &path);

    // Get or initialize the global instances map
    let instances = WEBVIEW_INSTANCES.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));

    // Check if WebView already exists
    if let Ok(webviews) = instances.lock() {
        if let Some(existing_webview) = webviews.get(&webtag) {
            log::info!("WebView already exists, reusing: {}-{}", appid, path);
            return Ok(existing_webview.clone());
        }
    }

    // Create new WebView only if it doesn't exist
    let webview_inner = WebViewInner::create(&appid, &path)?;
    let webview = Arc::new(webview_inner);

    // Store WebView in HashMap
    if let Ok(mut webviews) = instances.lock() {
        webviews.insert(webtag, webview.clone());
        log::info!("WebView created and stored: {}-{}", appid, path);
    } else {
        return Err(LxAppError::WebView(
            "Failed to acquire webviews lock".to_string(),
        ));
    }

    // Return the same WebView instance that was stored
    Ok(webview)
}

/// Find WebView by appid and path
pub fn find_webview(appid: &str, path: &str) -> Option<Arc<WebViewInner>> {
    let webtag = make_webtag(appid, path);

    if let Some(instances) = WEBVIEW_INSTANCES.get() {
        if let Ok(webviews) = instances.lock() {
            webviews.get(&webtag).cloned()
        } else {
            None
        }
    } else {
        None
    }
}
