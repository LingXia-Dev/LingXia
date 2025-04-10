use http::{Response, StatusCode};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Instant;

use crate::log::{LogLevel, Logging};
use crate::page::{self, PageController};

mod ipc;
mod scheme;

// Global instance of MiniApp
static MINI_APP: OnceLock<Mutex<MiniApp>> = OnceLock::new();

/// Platform-specific capabilities for MiniApp
pub trait MiniAppRuntime: Send + Sync {
    /// Read asset file from platform-specific location
    fn read_asset(&self, path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>>;

    /// Open mini app in platform-specific way
    fn open_miniapp(&self, app_id: &str, path: &str) -> Result<(), Box<dyn std::error::Error>>;

    /// Log message to platform-specific logging system
    fn log(&self, level: LogLevel, message: &str);

    /// Get platform-specific data directory
    fn get_data_dir(&self) -> Option<String>;

    /// Get platform-specific cache directory
    fn get_cache_dir(&self) -> Option<String>;
}

/// Initializes the MiniApp with the given platform implementation
pub fn init(platform: Box<dyn MiniAppRuntime>) {
    MINI_APP.get_or_init(|| {
        Mutex::new(MiniApp {
            runtime: platform,
            apps: HashMap::new(),
            last_active_times: HashMap::new(),
            max_apps: 5,
        })
    });
}

/// called when MiniApp system destroied by App
/// currently, it's planceholder
pub fn exit() {}

/// Returns a reference to the initialized MiniApp.
/// Panics if MiniApp has not been initialized.
pub fn get() -> &'static Mutex<MiniApp> {
    MINI_APP.get().expect("MiniApp has not been initialized")
}

pub struct MiniApp {
    pub(crate) runtime: Box<dyn MiniAppRuntime>,
    apps: HashMap<String, Arc<Mutex<page::PageManager>>>, // appid -> PageManager
    last_active_times: HashMap<String, Instant>,          // appid -> last active time
    max_apps: usize,                                      // Maximum number of apps allowed
}

impl MiniApp {
    /// Returns a reference to the PageManager for the given appid
    pub fn get_page_manager(&self, appid: &str) -> Option<&Arc<Mutex<page::PageManager>>> {
        self.apps.get(appid)
    }

    pub fn on_miniapp_loaded(&mut self, appid: String) {
        // If the app is already loaded, just update its active time
        if self.apps.contains_key(&appid) {
            self.last_active_times.insert(appid, Instant::now());
            return;
        }

        // If we've reached the maximum number of apps, destroy the least active one
        if self.apps.len() >= self.max_apps {
            self.destroy_least_active_miniapp();
        }

        // Create a new PageManager for this app
        self.apps.insert(
            appid.clone(),
            Arc::new(Mutex::new(page::PageManager::new(None))),
        );
        self.last_active_times.insert(appid, Instant::now());
    }

    pub fn on_miniapp_hidden(&mut self, appid: String) {
        // Only update the time if the app exists
        if self.apps.contains_key(&appid) {
            self.last_active_times.insert(appid, Instant::now());
        }
    }

    /// Handles low memory event (global, no appid needed)
    pub fn on_low_memory(&mut self) {
        // Destroy the least active app
        self.destroy_least_active_miniapp();
    }

    /// Called when a new page is created for the given appid and path
    pub fn on_page_created(&mut self, appid: String, path: String, pc: Arc<dyn PageController>) {
        let page_manager = self
            .apps
            .entry(appid.clone())
            .or_insert_with(|| Arc::new(Mutex::new(page::PageManager::new(None))));

        let url = if appid == "home" {
            let path_str = if path.is_empty() { "index.html" } else { &path };
            format!("lingxia://home/{}", path_str)
        } else {
            "https://www.bing.com".to_string()
        };

        pc.load_url(url);

        #[cfg(debug_assertions)]
        pc.set_devtools(true);

        // Initialize or update the page for the given path
        // update: on_page_show, on page show: page finsihed, reload(from java)
        let mut page_manager = page_manager.lock().unwrap();
        page_manager.mark_active(&path);
        page_manager.push_page_controller(path, pc);
    }

    /// Finds a PageController by appid and path
    pub fn find_page_controller(&self, appid: &str, path: &str) -> Option<Arc<dyn PageController>> {
        if let Some(page_manager) = self.apps.get(appid) {
            let page_manager = page_manager.lock().unwrap();
            page_manager.find_page_controller(path)
        } else {
            None
        }
    }

    /// Determines whether to override URL loading in the page.
    ///
    /// # Arguments
    /// * `appid` - The identifier of the mini application
    /// * `url` - The URL being requested
    ///
    /// # Returns
    /// * `true` - To intercept and handle the URL loading
    /// * `false` - To allow the page to continue loading the URL
    pub fn should_override_url_loading(&self, _appid: String, url: String) -> bool {
        // Extract scheme from URL
        let scheme = if let Some(scheme_end) = url.find("://") {
            &url[..scheme_end]
        } else {
            return false; // Invalid URL, don't override
        };

        // Handle lingxia scheme or block non-https schemes
        match scheme {
            "lingxia" => true, // Always intercept lingxia scheme
            "https" => false,  // Allow http/https URLs
            _ => true,         // Block all other schemes
        }
    }

    /// Handles a postMessage from the page's JavaScript context
    pub fn handle_post_message(&self, appid: String, _path: String, msg: String) {
        self.info(&appid, format!("Handling message for WebView: {}", msg));

        let message: Value = match serde_json::from_str(&msg) {
            Ok(v) => v,
            Err(e) => {
                self.error(&appid, format!("Failed to parse message: {}", e));
                return;
            }
        };

        let message_type = message.get("type").and_then(Value::as_str);

        match message_type {
            Some("OPEN_MINIAPP") => {
                self.info(&appid, "Handling OPEN_MINIAPP message");
                if let Some(data) = message.get("data") {
                    if let Some(app_id) = data.get("appId").and_then(Value::as_str) {
                        let path = data.get("path").and_then(Value::as_str).unwrap_or("");
                        if let Err(e) = self.runtime.open_miniapp(app_id, path) {
                            self.error(&appid, format!("Failed to open miniapp: {}", e));
                        }
                    }
                }
            }
            _ => {
                self.error(
                    &appid,
                    format!("Unknown message type: {}", message_type.unwrap_or("None")),
                );
            }
        }
    }

    /// Handles an HTTP request from the page
    pub fn handle_request(
        &self,
        _appid: String,
        req: http::Request<Vec<u8>>,
    ) -> Option<http::Response<Vec<u8>>> {
        let uri = req.uri();
        let scheme = uri.scheme_str().unwrap_or("");

        // Don't intercept http/https requests
        if scheme == "http" || scheme == "https" {
            return None;
        }

        // Handle different schemes
        Some(match scheme {
            "lingxia" => scheme::lingxia_handler(self.runtime.as_ref(), req),
            _ => Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header("Content-Type", "text/plain")
                .body(format!("Unknown scheme: {}", scheme).into_bytes())
                .unwrap(),
        })
    }

    /// Called when the page starts loading
    pub fn on_page_started(&self, appid: String, path: String) {
        // Find the corresponding controller
        if let Some(controller) = self.find_page_controller(&appid, &path) {
            // Get IPC script content and inject it
            if let Err(e) = controller.evaluate_javascript(ipc::get_ipc_script()) {
                self.error(&appid, e.to_string());
            }
        }
    }

    /// Called when the page finishes loading
    pub fn on_page_finished(&self, _appid: String, _path: String) {
        // ... implementation ...
    }

    /// Called when the page showed in the view
    pub fn on_page_show(&self, appid: String, path: String) {
        self.info(&appid, format!("Page show: {}", path));
    }
}

impl MiniApp {
    /// Destroys the least active app
    fn destroy_least_active_miniapp(&mut self) {
        let least_active_appid = self
            .last_active_times
            .iter()
            .min_by_key(|(_, time)| *time)
            .map(|(appid, _)| appid.clone());

        if let Some(appid) = least_active_appid {
            // Remove from both maps - PageManager's Drop trait will handle cleanup
            self.apps.remove(&appid);
            self.last_active_times.remove(&appid);
        }
    }
}
