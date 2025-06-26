use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use miniapp::{AppRuntime, AssetFileEntry, DeviceInfo, MiniAppError, WebViewController};

use crate::{App, WebViewInner};

static RUNTIME: OnceLock<Arc<SimpleAppRuntime>> = OnceLock::new();

/// Simplified AppRuntime implementation
pub struct SimpleAppRuntime {
    app: App,
    webviews: Mutex<HashMap<(String, String), Arc<WebViewInner>>>,
}

impl SimpleAppRuntime {
    /// Create a new SimpleAppRuntime instance
    pub fn new(app: App) -> Self {
        Self {
            app,
            webviews: Mutex::new(HashMap::new()),
        }
    }

    /// Initialize the global runtime instance
    pub fn init(app: App) -> Result<Arc<SimpleAppRuntime>, &'static str> {
        let runtime = Arc::new(SimpleAppRuntime::new(app));
        RUNTIME.set(runtime.clone()).map_err(|_| "Runtime already initialized")?;
        Ok(runtime)
    }

    /// Get the global runtime instance
    pub fn get() -> Option<&'static Arc<SimpleAppRuntime>> {
        RUNTIME.get()
    }

    /// Get a WebView instance from the registry
    pub fn get_webview(&self, appid: &str, path: &str) -> Option<Arc<WebViewInner>> {
        if let Ok(webviews) = self.webviews.lock() {
            webviews
                .get(&(appid.to_string(), path.to_string()))
                .cloned()
        } else {
            None
        }
    }

    /// Register a WebView instance
    pub fn put_webview(&self, appid: String, path: String, webview: Arc<WebViewInner>) -> bool {
        if let Ok(mut webviews) = self.webviews.lock() {
            webviews.insert((appid, path), webview);
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
        // Create the actual platform-specific WebView using WebViewInner::create
        let webview_inner = Arc::new(WebViewInner::create(&appid, &path)?);

        // Register the WebView
        if self.put_webview(appid, path, webview_inner.clone()) {
            // Return WebViewInner directly since it implements WebViewController
            Ok(webview_inner)
        } else {
            Err(MiniAppError::WebView(
                "Failed to register WebView".to_string(),
            ))
        }
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
