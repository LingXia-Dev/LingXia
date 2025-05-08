use http::{Response, StatusCode};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::Instant;

use crate::app::{AppConfig, AppController};
use crate::error::MiniAppError;
use crate::log::{LogLevel, LogTag, Logging};
use crate::page::{self, Pages};
use config::{MiniAppConfig, PageConfig};
use security::NetworkSecurity;

mod config;
mod install;
mod scheme;
mod security;
mod tabbar;

/// Constants for miniapp storage layout
const LINGXIA_DIR: &str = "lingxia";
const MINIAPPS_DIR: &str = "miniapps";
const VERSIONS_DIR: &str = "versions";
const STORAGE_DIR: &str = "storage";
const USERID_FILE: &str = "userid.txt";
const DEFAULT_USER_ID: &str = "default";
const DEFAULT_VERSION: &str = "0.0.1";

pub struct MiniAppOld {
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
pub struct MiniApp {
    pub(crate) appid: String,

    // Collection of pages in this app
    pages: Pages,

    // Time when this app was last active
    last_active_time: Instant,

    // Reference to the app controller
    pub(crate) controller: Arc<dyn AppController>,

    // Directory for miniapp files (HTML, JS, CSS, etc.)
    app_dir: PathBuf,

    // Directory for miniapp-specific storage (database, etc.)
    storage_dir: PathBuf,

    // Directory for miniapp-specific cache
    cache_dir: PathBuf,

    // whether it's home mini app
    home_miniapp: bool,

    // Current version of the mini app
    version: String,

    config: MiniAppConfig,

    // Network security configuration
    network_security: NetworkSecurity,
}

impl MiniApp {
    /// Create a new regular mini-app (not home app)
    fn new(appid: String, controller: Arc<dyn AppController>) -> Self {
        let mut app = Self {
            appid,
            pages: Pages::new(),
            last_active_time: Instant::now(),
            controller,
            app_dir: PathBuf::new(),
            storage_dir: PathBuf::new(),
            cache_dir: PathBuf::new(),
            home_miniapp: false,
            version: String::new(),
            network_security: NetworkSecurity::new(),
            config: MiniAppConfig::default(),
        };

        if let Err(e) = app.setup() {
            app.error("system", format!("Failed to setup app: {}", e));
        }

        app
    }

    /// Create a new MiniApp instance marked as the home mini app
    fn new_as_home(appid: String, controller: Arc<dyn AppController>) -> Self {
        let mut app = Self {
            appid,
            pages: Pages::new(),
            last_active_time: Instant::now(),
            controller,
            app_dir: PathBuf::new(),
            storage_dir: PathBuf::new(),
            cache_dir: PathBuf::new(),
            home_miniapp: true,
            version: String::new(),
            network_security: NetworkSecurity::new(),
            config: MiniAppConfig::default(),
        };

        if let Err(e) = app.setup() {
            app.error("system", format!("Failed to setup home app: {}", e));
        }

        app
    }

    // Setup will initialize paths and load config
    fn setup(&mut self) -> Result<(), MiniAppError> {
        // Get the app's version
        self.version = self.get_version();

        // Calculate the directory name based on appid, user and whether this is a home app
        let dir_name = if self.home_miniapp {
            // Home mini app uses appid directly as directory name
            self.appid.clone()
        } else {
            // Regular mini app uses a hash based on app_id and user_id
            let user_id = get_user_id(self.controller.as_ref());
            generate_app_hash(&self.appid, &user_id)
        };

        // Set up app directory
        let base_dir = self
            .controller
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(MINIAPPS_DIR);

        self.app_dir = base_dir.join(&dir_name);
        if !self.app_dir.exists() {
            std::fs::create_dir_all(&self.app_dir).map_err(|e| {
                MiniAppError::IoError(format!("Failed to create app directory: {}", e))
            })?;
        }

        // Set up storage directory
        let storage_base_dir = self
            .controller
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(STORAGE_DIR);

        self.storage_dir = storage_base_dir.join(&dir_name);
        if !self.storage_dir.exists() {
            std::fs::create_dir_all(&self.storage_dir).map_err(|e| {
                MiniAppError::IoError(format!("Failed to create storage directory: {}", e))
            })?;
        }

        // Set up cache directory
        let cache_base_dir = self
            .controller
            .app_cache_dir()
            .join(LINGXIA_DIR)
            .join(MINIAPPS_DIR);

        self.cache_dir = cache_base_dir.join(&dir_name);
        if !self.cache_dir.exists() {
            std::fs::create_dir_all(&self.cache_dir).map_err(|e| {
                MiniAppError::IoError(format!("Failed to create cache directory: {}", e))
            })?;
        }

        // Load app configuration if it exists
        if let Ok(app_json) = self.read_json("app.json") {
            self.config = MiniAppConfig::from_value(app_json)
                .map_err(|e| MiniAppError::InvalidJsonFile(format!("app.json: {}", e)))?;

            // Configure Pages based on app config
            self.pages.set_has_tabbar(self.config.has_tab_bar());
        }

        Ok(())
    }

    /// Get the version of this app from storage
    fn get_version(&self) -> String {
        let version_path = self
            .controller
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(VERSIONS_DIR)
            .join(format!("{}.txt", self.appid));

        if version_path.exists() {
            if let Ok(content) = fs::read_to_string(&version_path) {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }

        // Return default version
        DEFAULT_VERSION.to_string()
    }

    // Reads binary data from the specified relative path
    fn read_bytes(&self, relative_path: &str) -> Result<Vec<u8>, MiniAppError> {
        let file_path = self.app_dir.join(relative_path);

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

/// Generates a hash string based on app ID and user ID
fn generate_app_hash(app_id: &str, user_id: &str) -> String {
    // Combine app_id and user_id
    let combined = format!("{}_{}", app_id, user_id);

    // Calculate hash using standard library's DefaultHasher
    let mut hasher = DefaultHasher::new();
    combined.hash(&mut hasher);
    let result = hasher.finish();

    // Convert to hex string
    format!("{:x}", result)
}

/// Gets the current user ID from storage or returns the default
fn get_user_id<T: AppController + ?Sized>(controller: &T) -> String {
    let userid_path = controller
        .app_data_dir()
        .join(LINGXIA_DIR)
        .join(USERID_FILE);

    if userid_path.exists() {
        if let Ok(content) = fs::read_to_string(&userid_path) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    // Return default user ID
    DEFAULT_USER_ID.to_string()
}

/// Sets the current user ID in storage
fn set_user_id<T: AppController + ?Sized>(
    controller: &T,
    user_id: &str,
) -> Result<(), MiniAppError> {
    let lingxia_dir = controller.app_data_dir().join(LINGXIA_DIR);
    if !lingxia_dir.exists() {
        fs::create_dir_all(&lingxia_dir)?;
    }

    let userid_path = lingxia_dir.join(USERID_FILE);
    fs::write(userid_path, user_id)?;

    Ok(())
}

/// Prepares the base directory structure for mini apps
fn prepare_directory_structure<T: AppController + ?Sized>(
    controller: &T,
) -> Result<(), MiniAppError> {
    let data_dir = controller.app_data_dir();
    let cache_dir = controller.app_cache_dir();

    // Create required directories
    let dirs = [
        data_dir.join(LINGXIA_DIR).join(MINIAPPS_DIR),
        data_dir.join(LINGXIA_DIR).join(VERSIONS_DIR),
        data_dir.join(LINGXIA_DIR).join(STORAGE_DIR),
        cache_dir.join(LINGXIA_DIR).join(MINIAPPS_DIR),
    ];

    for dir in &dirs {
        fs::create_dir_all(dir)?;
    }

    // Ensure user ID file exists with default value if needed
    let userid_path = data_dir.join(LINGXIA_DIR).join(USERID_FILE);
    if !userid_path.exists() {
        if let Some(parent) = userid_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(userid_path, DEFAULT_USER_ID)?;
    }

    Ok(())
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
        let app_config = MiniAppConfig::from_value(app_config_value)
            .map_err(|e| MiniAppError::InvalidJsonFile(format!("app.json: {}", e)))?;

        // Handle TabBar configuration
        if let Some(tab_bar) = &app_config.tabBar {
            // Only return tabbar JSON if it's valid (has between 2-5 items)
            if tab_bar.is_valid() {
                // Convert relative paths to absolute paths
                tab_bar
                    .to_json_with_absolute_paths(&self.app_dir)
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

    fn on_page_started(&self, _path: String) {
        // TODO
    }

    fn on_page_finished(&self, _path: String) {
        // TODO
    }

    fn on_page_show(&self, path: String) {
        todo!()
    }

    fn on_back_pressed(&self) -> bool {
        todo!()
    }

    // Determines whether to override URL loading in the page.
    fn should_override_url_loading(&self, url: String) -> bool {
        // Extract scheme from URL
        let scheme = if let Some(scheme_end) = url.find("://") {
            &url[..scheme_end]
        } else {
            return false; // Invalid URL, don't override
        };

        // Handle lingxia scheme or block non-https schemes
        match scheme {
            "lingxia" => true, // Always intercept lingxia scheme
            "https" => false,  // Allow https URLs (they'll be checked in handle_request)
            _ => true,         // Block all other schemes
        }
    }

    fn handle_post_message(&self, path: String, msg: String) {
        todo!()
    }

    fn handle_request(&self, req: http::Request<Vec<u8>>) -> Option<http::Response<Vec<u8>>> {
        let uri = req.uri();
        let scheme = uri.scheme_str().unwrap_or("");

        // Use pattern matching for different URI schemes
        match scheme {
            // HTTPS requests - check domain whitelist and static resource types
            "https" => self.https_handler(req),

            // Lingxia scheme for internal app assets
            "lingxia" => self.lingxia_handler(req),

            // Reject all other schemes with 400 Bad Request
            _ => Some(
                Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header("Content-Type", "text/plain")
                    .body(format!("Unsupported scheme: {}", scheme).into_bytes())
                    .unwrap(),
            ),
        }
    }

    fn log(&self, path: &str, level: LogLevel, message: &str) {
        self.write_log(path, level, LogTag::WebViewConsole, message);
    }
}

// Global instance of MiniApps
static MINIAPPS: OnceLock<RwLock<MiniApps>> = OnceLock::new();

/// Initialize the MiniApps singleton
pub fn init<T: AppController + 'static>(controller: T) {
    let controller_arc = Arc::new(controller);

    // Prepare the directory structure
    if let Err(e) = prepare_directory_structure(controller_arc.as_ref()) {
        controller_arc.log(
            "system",
            LogLevel::Error,
            &format!("Failed to prepare directory structure: {}", e),
        );
        return;
    }

    match AppConfig::load(controller_arc.as_ref()) {
        Ok(config) => {
            let home_mini_app_id = &config.home_mini_app_id;

            // Check if the home mini app is installed
            if !install::is_installed(controller_arc.as_ref(), home_mini_app_id) {
                let home_mini_app_version = &config.home_mini_app_version;

                // Copy home mini app files from assets and update version
                if let Err(e) = install::install_home_miniapp(
                    controller_arc.as_ref(),
                    home_mini_app_id,
                    home_mini_app_version,
                ) {
                    controller_arc.log(
                        "system",
                        LogLevel::Error,
                        &format!("Failed to install home mini app: {}", e),
                    );
                    return;
                }
            }

            // Now create the MiniApp instance and call setup
            let home_miniapp =
                MiniApp::new_as_home(home_mini_app_id.clone(), controller_arc.clone());

            // Initialize MiniApps collection
            let mut miniapps = MiniApps::new(controller_arc.clone());

            // Wrap the home miniapp in Arc<RwLock<>> and add it to the collection
            let home_miniapp_arc = Arc::new(RwLock::new(home_miniapp));

            // Add home mini app to the collection
            miniapps
                .miniapps
                .insert(home_mini_app_id.clone(), home_miniapp_arc);

            if MINIAPPS.set(RwLock::new(miniapps)).is_err() {
                controller_arc.log(
                    "system",
                    LogLevel::Error,
                    "MiniApps singleton had been initialized by another instance",
                );
            } else {
                controller_arc.log(
                    "system",
                    LogLevel::Info,
                    "MiniApps initialized successfully",
                );
            }
        }

        Err(e) => {
            // Provide more detailed error messages for different error types
            let error_message = match e {
                MiniAppError::InvalidParameter(msg) => {
                    format!("Configuration validation failed: {}", msg)
                }
                MiniAppError::InvalidJsonFile(msg) => {
                    format!("Invalid app.json file: {}", msg)
                }
                MiniAppError::IoError(msg) => {
                    format!("I/O error while reading configuration: {}", msg)
                }
                _ => format!("Failed to load app configuration: {}", e),
            };

            controller_arc.log("system", LogLevel::Error, &error_message);
        }
    }
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
///
/// # Panics
/// Panics if `MiniApps` is not initialized
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
