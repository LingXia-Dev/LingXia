use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use miniapp::{AppRuntime, AssetFileEntry, DeviceInfo, MiniAppError, WebViewController};

use crate::{App, WebViewInner};

static RUNTIME: OnceLock<Arc<SimpleAppRuntime>> = OnceLock::new();

/// WebTag newtype for better type safety
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WebTag(String);

impl WebTag {
    pub fn new(appid: &str, path: &str) -> Self {
        Self(format!("{}-{}", appid, path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Extract appid from WebTag
    pub fn extract_appid(&self) -> String {
        self.0
            .split_once('-')
            .map(|(appid, _)| appid.to_string())
            .unwrap()
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

impl From<(String, String)> for WebTag {
    fn from((appid, path): (String, String)) -> Self {
        Self::new(&appid, &path)
    }
}

impl From<(&str, &str)> for WebTag {
    fn from((appid, path): (&str, &str)) -> Self {
        Self::new(appid, path)
    }
}

impl From<&str> for WebTag {
    fn from(webtag_str: &str) -> Self {
        Self(webtag_str.to_string())
    }
}

impl From<String> for WebTag {
    fn from(webtag_str: String) -> Self {
        Self(webtag_str)
    }
}

impl std::fmt::Display for WebTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Simplified AppRuntime implementation
pub struct SimpleAppRuntime {
    app: App,
    webviews: Mutex<HashMap<WebTag, Arc<Mutex<WebViewInner>>>>,
}

impl SimpleAppRuntime {
    /// Initialize the global runtime instance
    pub fn init(app: App) -> Arc<SimpleAppRuntime> {
        let runtime = Arc::new(SimpleAppRuntime {
            app,
            webviews: Mutex::new(HashMap::new()),
        });

        // Set global runtime, ignore error if already initialized
        let _ = RUNTIME.set(runtime.clone());
        runtime
    }

    /// Get the global runtime instance
    pub fn get() -> Option<&'static Arc<SimpleAppRuntime>> {
        RUNTIME.get()
    }

    /// Get a WebView instance from the registry
    pub fn get_webview(&self, appid: &str, path: &str) -> Option<Arc<Mutex<WebViewInner>>> {
        if let Ok(webviews) = self.webviews.lock() {
            let webtag = WebTag::new(appid, path);
            webviews.get(&webtag).cloned()
        } else {
            None
        }
    }

    /// Get a WebView by WebTag
    pub fn get_webview_by_tag(&self, webtag: &WebTag) -> Option<Arc<Mutex<WebViewInner>>> {
        if let Ok(webviews) = self.webviews.lock() {
            webviews.get(webtag).cloned()
        } else {
            None
        }
    }

    /// Register a WebView instance
    pub fn put_webview(&self, appid: String, path: String, webview: WebViewInner) -> bool {
        if let Ok(mut webviews) = self.webviews.lock() {
            let webtag = WebTag::new(&appid, &path);
            webviews.insert(webtag, Arc::new(Mutex::new(webview)));
            true
        } else {
            false
        }
    }

    /// Register a WebView instance by WebTag
    pub fn put_webview_by_tag(&self, webtag: WebTag, webview: WebViewInner) -> bool {
        if let Ok(mut webviews) = self.webviews.lock() {
            webviews.insert(webtag, Arc::new(Mutex::new(webview)));
            true
        } else {
            false
        }
    }
}

impl AppRuntime for SimpleAppRuntime {
    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, MiniAppError> {
        self.app.read_asset(path)
    }

    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, MiniAppError>> + 'a> {
        // Convert from our AssetFileEntry to miniapp's AssetFileEntry
        let iter = self.app.asset_dir_iter(asset_dir);
        Box::new(iter.map(|result| {
            result.map(|entry| AssetFileEntry {
                path: entry.path,
                reader: entry.reader,
            })
        }))
    }

    fn app_data_dir(&self) -> PathBuf {
        self.app.app_data_dir()
    }

    fn app_cache_dir(&self) -> PathBuf {
        self.app.app_cache_dir()
    }

    fn device_info(&self) -> DeviceInfo {
        self.app.device_info()
    }

    fn create_webview(
        &self,
        appid: String,
        path: String,
    ) -> Result<Arc<dyn WebViewController>, MiniAppError> {
        // Create WebView and store it in global runtime
        let webtag = WebTag::new(&appid, &path);
        let webview_inner = WebViewInner::create(&appid, &path)?;

        // Store WebView in global runtime
        if let Some(global_runtime) = SimpleAppRuntime::get() {
            if !global_runtime.put_webview_by_tag(webtag, webview_inner) {
                log::error!("Failed to store WebView in runtime: {}-{}", appid, path);
                return Err(MiniAppError::WebView(
                    "Failed to store WebView in runtime".to_string(),
                ));
            } else {
                log::info!("WebView stored in runtime: {}-{}", appid, path);
            }
        } else {
            log::error!(
                "Global runtime not available for WebView storage: {}-{}",
                appid,
                path
            );
            return Err(MiniAppError::WebView(
                "Global runtime not available".to_string(),
            ));
        }

        // Create a new WebView instance to return (since the previous one was moved)
        let return_webview = WebViewInner::create(&appid, &path)?;
        Ok(Arc::new(return_webview))
    }

    fn open_miniapp(&self, appid: String, path: String) -> Result<(), MiniAppError> {
        self.app.open_miniapp(&appid, &path)
    }

    fn close_miniapp(&self, appid: String) -> Result<(), MiniAppError> {
        self.app.close_miniapp(&appid)
    }

    fn switch_page(&self, appid: String, path: String) -> Result<(), MiniAppError> {
        self.app.switch_page(&appid, &path)
    }
}
