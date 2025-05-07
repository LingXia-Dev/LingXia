use http::{Response, StatusCode};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::Instant;

use crate::app::AppController;
use crate::error::MiniAppError;
use crate::log::{LogLevel, Logging};
use crate::miniapp::config::{AppConfig, PageConfig};
use crate::page::{self, Pages};

mod config;
mod ipc;
mod scheme;
mod tabbar;

/// Platform-specific capabilities for MiniApp
pub trait MiniAppRuntime: Send + Sync {
    /// Read asset file from platform-specific location
    fn read_asset(&self, path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>>;

    /// Get data directory for the app
    fn get_data_dir(&self) -> Option<String>;

    /// Get cache directory for the app
    fn get_cache_dir(&self) -> Option<String>;

    /// Log message to platform-specific logging system
    fn log(&self, level: LogLevel, message: &str);

    /// Switch to another page within the same mini app
    ///
    /// # Arguments
    /// * `app_id` - The ID of the mini app whose page needs switching
    /// * `path` - The target path to navigate to within the mini app
    fn switch_page(&self, app_id: &str, path: &str) -> Result<(), Box<dyn std::error::Error>>;

    /// Open a mini app in platform-specific way
    ///
    /// # Arguments
    /// * `app_id` - The ID of the mini app to open
    /// * `path` - The initial path to navigate to within the mini app
    fn open_miniapp(&self, app_id: &str, path: &str) -> Result<(), Box<dyn std::error::Error>>;

    /// Close mini app in platform-specific way
    fn close_miniapp(&self, app_id: &str) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct MiniAppOld {
    pub(crate) runtime: Box<dyn MiniAppRuntime>,
    apps: HashMap<String, Arc<Mutex<page::Pages>>>, // appid -> PageManager
    last_active_times: HashMap<String, Instant>,    // appid -> last active time
    max_apps: usize,                                // Maximum number of apps allowed
}

impl MiniAppOld {
    pub fn on_miniapp_opened(&mut self, appid: String) {
        // If the app is already loaded, just update its active time
        // if self.apps.contains_key(&appid) {
        //     self.last_active_times.insert(appid, Instant::now());
        //     return;
        // }
        //
        // // If we've reached the maximum number of apps, destroy the least active one
        // if self.apps.len() >= self.max_apps {
        //     self.destroy_least_active_miniapp();
        // }
        //
        // // Create a new PageManager for this app
        // self.apps.insert(
        //     appid.clone(),
        //     Arc::new(Mutex::new(page::PageManager::new(None))),
        // );
        // self.last_active_times.insert(appid, Instant::now());
    }

    /**
     * Called when a mini app is closed.
     * This primarily updates the last active time for the app to help with memory management.
     */
    pub fn on_miniapp_closed(&mut self, appid: String) {
        // Only update the time if the app exists
        if self.apps.contains_key(&appid) {
            self.last_active_times.insert(appid, Instant::now());
        }
    }

    /// Called when a new page is created for the given appid and path
    pub fn on_page_created(&mut self, appid: String, path: String) {
        // A page is a tab page if it's in the tab bar configuration(demo code)
        // let is_tab_page = if let Ok(tab_config) =
        //     serde_json::from_str::<serde_json::Value>(DEFAULT_TAB_BAR_CONFIG)
        // {
        //     tab_config
        //         .get("list")
        //         .and_then(|v| v.as_array())
        //         .map(|list| {
        //             list.iter().any(|item| {
        //                 item.get("pagePath")
        //                     .and_then(|v| v.as_str())
        //                     .map(|p| p == path)
        //                     .unwrap_or(false)
        //             })
        //         })
        //         .unwrap_or(false)
        // } else {
        //     false
        // };

        // self.info(
        //     &appid,
        //     format!("insert page {}, is_tab_page: {}", path, is_tab_page),
        // );

        // let page_manager = self
        //     .apps
        //     .entry(appid.clone())
        //     .or_insert_with(|| Arc::new(Mutex::new(page::PageManager::new(None))));

        // demo code
        let url = if appid == "home" {
            let path_str = if path.is_empty() { "index.html" } else { &path };
            format!("lingxia://home/{}", path_str)
        } else {
            "https://www.bing.com".to_string()
        };

        // pc.load_url(url);

        // #[cfg(debug_assertions)]
        // pc.set_devtools(true);

        // Initialize the page
        // let mut page_manager = page_manager.lock().unwrap();
        // page_manager.push_page_controller(path, is_tab_page, pc);
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
        // if let Some(controller) = self.find_page_controller(&appid, &path) {
        //     // Get IPC script content and inject it
        //     if let Err(e) = controller.evaluate_javascript(ipc::get_ipc_script()) {
        //         // self.error(&appid, e.to_string());
        //     }
        // }
    }

    /// Called when the page finishes loading
    pub fn on_page_finished(&self, _appid: String, _path: String) {
        // ... implementation ...
    }

    /// Called when the page showed in the view
    pub fn on_page_show(&self, appid: String, path: String) {
        // self.info(&appid, format!("Page show: {}", path));

        // Mark the page as active when it's shown
        if let Some(page_manager) = self.apps.get(&appid) {
            let mut page_manager = page_manager.lock().unwrap();
            page_manager.mark_active(&path);
        }
    }

    /// Handle back press event
    /// Returns true if the event was handled, false otherwise
    pub fn on_back_pressed(&self, app_id: &str) -> bool {
        // self.info(app_id, "Back pressed, closing mini app");

        if let Some(page_manager_arc) = self.apps.get(app_id) {
            let mut page_manager = page_manager_arc.lock().unwrap();
            // match page_manager.pop_from_current_stack() {
            // Some(previous_path) => {
            // self.info(
            //     app_id,
            //     format!("Popped page, requesting switch back to: {}", previous_path),
            // );
            // Tell the platform to switch the view *without* changing the tab state
            // if let Err(e) = self.runtime.switch_page(app_id, &previous_path) {
            // self.error(
            //     app_id,
            //     format!(
            //         "Failed to request page switch back to {}: {}",
            //         previous_path, e
            //     ),
            // );
            // Still considered handled as state was popped
            // }
            true // Back press was handled by popping a page state
        // }
        // None => {
        // self.error(app_id, "No page to pop from current stack");
        // false
        // }
        } else {
            // self.error(app_id, "No page manager found for the given app id");
            false
        }
    }
}

/// Manages a collection of mini applications
pub struct MiniApps {
    // Collection of mini apps, keyed by app ID
    miniapps: HashMap<String, Arc<RwLock<MiniApp>>>,
    // Reference to the app controller
    controller: Arc<dyn AppController>,
    // Maximum number of apps allowed in memory
    max_apps: usize,
}

impl MiniApps {
    fn new<T: AppController + 'static>(controller: T) -> Self {
        Self {
            miniapps: HashMap::new(),
            controller: Arc::new(controller),
            max_apps: 5,
        }
    }

    pub fn on_low_memory() {
        // TODO: Destroy the least active app
    }
}

/// Represents a single mini application
/// TODO: rename to MiniApp after refactoring
pub struct MiniApp {
    pub(crate) appid: String,

    // Collection of pages in this app
    pages: Pages,

    // Time when this app was last active
    last_active_time: Instant,

    // Reference to the app controller
    pub(crate) controller: Arc<dyn AppController>,

    // Directory for miniapp-specific data
    data_dir: PathBuf,
    // Directory for miniapp-specific cache
    cache_dir: PathBuf,
}

impl MiniApp {
    fn new(appid: String, controller: Arc<dyn AppController>) -> Self {
        // TODO: build dir based on vendor and customer id
        let data_dir = controller.app_data_dir();
        let cache_dir = controller.app_cache_dir();

        // TODO: read app.json
        Self {
            pages: Pages::new(None, false),
            last_active_time: Instant::now(),
            appid,
            controller,
            data_dir,
            cache_dir,
        }
    }

    // Reads binary data from the specified relative path
    fn read_bytes(&self, relative_path: &str) -> Result<Vec<u8>, MiniAppError> {
        let file_path = self.data_dir.join(relative_path);

        // Try to read from the filesystem
        fs::read(file_path)
            .map_err(|e| MiniAppError::ResourceNotFound(format!("{}:{}", relative_path, e)))
    }

    /// Reads text content from the specified relative path
    fn read_text(&self, relative_path: &str) -> Result<String, MiniAppError> {
        self.read_bytes(relative_path)
            .map(|content| String::from_utf8_lossy(&content).to_string())
    }

    /// Reads and parses JSON content from the specified relative path
    fn read_json(&self, relative_path: &str) -> Result<serde_json::Value, MiniAppError> {
        self.read_text(relative_path).and_then(|content| {
            serde_json::from_str(&content)
                .map_err(|_| MiniAppError::InvalidJsonFile(relative_path.to_string()))
        })
    }
}

pub trait AppUiDelegate {
    /// Get tabbar configuration for mini app
    fn get_tab_bar_config(&self) -> Result<String, MiniAppError>;

    /// Get page configuration for a specific page
    fn get_page_config(&self, path: &str) -> Result<String, MiniAppError>;

    /// Called when mini app is opened
    fn on_miniapp_opened(&self);

    /// Called when mini app is closed
    fn on_miniapp_closed(&self);

    /// Called when a page is created
    fn on_page_created(&self, path: String);

    /// Called when the page starts loading
    fn on_page_started(&self, path: String);

    /// Called when the page finishes loading
    fn on_page_finished(&self, path: String);

    /// Called when the page showed in the view
    fn on_page_show(&self, path: String);

    /// Handle back button press
    fn on_back_pressed(&self) -> bool;

    /// Determines whether to override URL loading in the page.
    ///
    /// # Arguments
    /// * `url` - The URL being requested
    ///
    /// # Returns
    /// * `true` - To intercept and handle the URL loading
    /// * `false` - To allow the page to continue loading the URL
    fn should_override_url_loading(&self, url: String) -> bool;

    /// Handles a postMessage from the page's JavaScript context
    fn handle_post_message(&self, path: String, msg: String);

    /// Handles an HTTP request from the page
    fn handle_request(&self, req: http::Request<Vec<u8>>) -> Option<http::Response<Vec<u8>>>;

    /// Receive log from WebView
    fn log(&self, path: &str, level: LogLevel, message: &str);
}

impl AppUiDelegate for MiniApp {
    fn get_tab_bar_config(&self) -> Result<String, MiniAppError> {
        // Read app.json and parse it using AppConfig
        let app_config_value = self.read_json("app.json")?;
        let app_config = AppConfig::from_value(app_config_value)
            .map_err(|e| MiniAppError::InvalidJsonFile(format!("app.json: {}", e)))?;

        // Handle TabBar configuration
        if let Some(tab_bar) = &app_config.tabBar {
            // Only return tabbar JSON if it's valid (has between 2-5 items)
            if tab_bar.is_valid() {
                // Convert relative paths to absolute paths
                tab_bar
                    .to_json_with_absolute_paths(&self.data_dir)
                    .map_err(|e| {
                        MiniAppError::InvalidJsonFile(format!("Failed to serialize TabBar: {}", e))
                    })
            } else {
                // Not enough items or too many items, return empty JSON object
                Ok("{}".to_string())
            }
        } else {
            // TabBar is optional, return a valid empty tabbar JSON
            Ok("{}".to_string())
        }
    }

    fn get_page_config(&self, path: &str) -> Result<String, MiniAppError> {
        // Handle different possible path formats:
        // 1. "pages/home/index.html" -> "pages/home/index.json"
        // 2. "pages/home/index" -> "pages/home/index.json"
        // 3. "pages/home" -> "pages/home.json"
        let page_config_path = if path.contains('.') {
            // Has extension: replace it with .json
            let pos = path.rfind('.').unwrap();
            format!("{}.json", &path[0..pos])
        } else {
            // No extension: append .json
            format!("{}.json", path)
        };

        // Try to read page-specific configuration first from the direct path
        let result = self.read_json(&page_config_path);

        // If that fails, try the legacy format: "pages/{path}/page.json"
        let result = if result.is_err() && !path.starts_with("pages/") {
            self.read_json(&format!("pages/{}/page.json", path))
        } else {
            result
        };

        // Process the configuration or use default
        match result {
            Ok(page_config_value) => {
                let page_config = PageConfig::from_value(page_config_value).map_err(|e| {
                    MiniAppError::InvalidJsonFile(format!("{}:{}", page_config_path, e))
                })?;

                serde_json::to_string(&page_config).map_err(|e| {
                    MiniAppError::InvalidJsonFile(format!("Failed to serialize PageConfig: {}", e))
                })
            }
            Err(_) => {
                // Fallback to default page config
                let default_config = PageConfig::default();
                serde_json::to_string(&default_config).map_err(|e| {
                    MiniAppError::InvalidJsonFile(format!(
                        "Failed to serialize default PageConfig: {}",
                        e
                    ))
                })
            }
        }
    }

    fn on_miniapp_opened(&self) {
        todo!()
    }

    fn on_miniapp_closed(&self) {
        todo!()
    }

    fn on_page_created(&self, path: String) {
        todo!()
    }

    fn on_page_started(&self, path: String) {
        todo!()
    }

    fn on_page_finished(&self, path: String) {
        todo!()
    }

    fn on_page_show(&self, path: String) {
        todo!()
    }

    fn on_back_pressed(&self) -> bool {
        todo!()
    }

    fn should_override_url_loading(&self, url: String) -> bool {
        todo!()
    }

    fn handle_post_message(&self, path: String, msg: String) {
        todo!()
    }

    fn handle_request(&self, req: http::Request<Vec<u8>>) -> Option<http::Response<Vec<u8>>> {
        todo!()
    }

    fn log(&self, path: &str, level: LogLevel, message: &str) {
        todo!()
    }
}

// Global instance of MiniApps
static MINIAPPS: OnceLock<RwLock<MiniApps>> = OnceLock::new();

/// Initialize the MiniApps singleton
pub fn init<T: AppController + 'static>(controller: T) {
    let _ = MINIAPPS.set(RwLock::new(MiniApps::new(controller)));
}

/// Get or initialize a specific MiniApp instance by appid
///
/// This function provides a get-or-create semantic for MiniApp instances.
/// If the MiniApp with the given appid exists, it returns a reference to it.
/// If it doesn't exist, it creates a new one with default settings and returns a reference.
///
/// # Arguments
/// * `appid` - The ID of the mini app to get or create
///
/// # Returns
/// A thread-safe reference to the MiniAppUnit
pub fn get_or_init_miniapp(appid: String) -> Arc<RwLock<MiniApp>> {
    let mut miniapps = MINIAPPS
        .get()
        .expect("MiniApps not initialized")
        .write()
        .unwrap();

    let controller = miniapps.controller.clone();

    // Use entry API to atomically get or insert
    miniapps
        .miniapps
        .entry(appid.clone())
        .or_insert_with(|| {
            let unit = MiniApp::new(appid, controller);
            Arc::new(RwLock::new(unit))
        })
        .clone()
}
