use http::{Response, StatusCode};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::Instant;

use crate::app::{AppConfig, AppController, switch_page};
use crate::appservice::{self, MiniAppServiceManager};
use crate::error::MiniAppError;
use crate::log::{LogLevel, LogTag, Logging};
use crate::page::{Pages, WebViewController};
use config::{MiniAppConfig, PageConfig};
use security::NetworkSecurity;

mod config;
mod install;
mod scheme;
mod security;
mod tabbar;
mod version;

/// Constants for miniapp storage layout
const LINGXIA_DIR: &str = "lingxia";
const MINIAPPS_DIR: &str = "miniapps";
const VERSIONS_DIR: &str = "versions";
const STORAGE_DIR: &str = "storage";
const USERID_FILE: &str = "userid.txt";
const DEFAULT_USER_ID: &str = "default";
const DEFAULT_VERSION: &str = "0.0.1";

/// Manages a collection of mini applications
pub struct MiniApps {
    // Collection of mini apps, keyed by app ID
    miniapps: HashMap<String, Arc<RwLock<MiniApp>>>,
    // Reference to the app controller
    controller: Arc<dyn AppController>,
    // Maximum number of apps allowed in memory
    max_apps: usize,
    // Reference to the app service manager
    svc_manager: Arc<Mutex<MiniAppServiceManager>>,
}

impl MiniApps {
    fn new<T: AppController + 'static>(
        controller: T,
        svc_manager: Arc<Mutex<MiniAppServiceManager>>,
        max_apps: usize,
    ) -> Self {
        let controller = Arc::new(controller);

        Self {
            miniapps: HashMap::new(),
            controller,
            max_apps,
            svc_manager,
        }
    }

    /// Destroys the least recently active mini app to free up memory
    fn destroy_least_active_miniapp(&mut self) {
        if self.miniapps.is_empty() {
            return;
        }

        // Find the least active app that isn't the home app
        let least_active = self
            .miniapps
            .iter()
            .filter_map(|(appid, app_arc)| {
                let app = app_arc.read().unwrap();
                // Skip home app from destruction
                if app.home_miniapp {
                    None
                } else {
                    Some((appid.clone(), app.last_active_time))
                }
            })
            .min_by(|(_, time1), (_, time2)| time1.cmp(time2));

        // If we found a non-home app, remove it
        if let Some((appid, _)) = least_active {
            self.controller.log(
                LogLevel::Info,
                &format!("Destroying least active mini app: {}", appid),
            );

            // Clean up app service before removing the app
            if let Ok(mut manager) = self.svc_manager.lock() {
                if let Err(e) = manager.terminate_app_svc(appid.clone()) {
                    self.controller.log(
                        LogLevel::Error,
                        &format!("Failed to terminate app service for {}: {}", appid, e),
                    );
                }
            }

            self.miniapps.remove(&appid);
        }
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

    // Reference to the app service manager
    svc_manager: Arc<Mutex<MiniAppServiceManager>>,
}

impl MiniApp {
    /// Create a new regular mini-app (not home app)
    fn new(
        appid: String,
        controller: Arc<dyn AppController>,
        svc_manager: Arc<Mutex<MiniAppServiceManager>>,
    ) -> Self {
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
            svc_manager,
        };

        if let Err(e) = app.setup() {
            app.error("system", format!("Failed to setup app: {}", e));
        }

        app
    }

    /// Create a new MiniApp instance marked as the home mini app
    fn new_as_home(
        appid: String,
        controller: Arc<dyn AppController>,
        svc_manager: Arc<Mutex<MiniAppServiceManager>>,
    ) -> Self {
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
            svc_manager,
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

            // Set tab bar items in the Pages manager
            self.pages.set_tabbar_items(self.config.get_tab_pages());
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

    /// Uninstalls the mini app by removing its version record and directories
    ///
    /// # Returns
    /// * `Ok(())` - If the mini app was uninstalled successfully
    /// * `Err(MiniAppError)` - If there was an error during uninstallation
    pub fn uninstall(&self) -> Result<(), MiniAppError> {
        // Don't allow uninstalling the home app
        if self.home_miniapp {
            return Err(MiniAppError::UnsupportedOperation(
                "Cannot uninstall the home mini app".to_string(),
            ));
        }

        //  Remove the version record file
        let version_path = self
            .controller
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(VERSIONS_DIR)
            .join(format!("{}.txt", self.appid));

        if version_path.exists() {
            fs::remove_file(&version_path)?
        }

        //  Remove the app directory
        if self.app_dir.exists() {
            fs::remove_dir_all(&self.app_dir)?;
        }

        // Remove the storage directory
        if self.storage_dir.exists() {
            fs::remove_dir_all(&self.storage_dir)?;
        }

        //  Remove the cache directory
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir)?;
        }

        self.info(
            "system",
            format!("Mini app {} uninstalled successfully", self.appid),
        );
        Ok(())
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
    fn on_miniapp_closed(&mut self);

    /// Called when a page is created
    fn on_page_created(&mut self, path: String);

    /// Called when the page starts loading
    fn on_page_started(&self, path: String);

    /// Called when the page finishes loading
    fn on_page_finished(&self, path: String);

    /// Called when the page showed in the view
    fn on_page_show(&mut self, path: String);

    /// Handle back button press
    /// Return true to indicate the back press had been handled
    fn on_back_pressed(&mut self) -> bool;

    /// Determines whether to override URL loading in the page.
    ///
    /// # Arguments
    /// * `url` - The URL being requested
    ///
    /// # Returns
    /// * `true` - To intercept and handle the URL loading
    /// * `false` - To allow the page to continue loading the URL
    fn should_override_url_loading(&self, url: String) -> bool;

    /// Handles a postMessage from the page View(WebView)
    fn handle_post_message(&self, path: String, msg: String);

    /// Handles an HTTP request from the page
    fn handle_request(&mut self, req: http::Request<Vec<u8>>) -> Option<http::Response<Vec<u8>>>;

    /// Receive log from WebView
    fn log(&self, path: &str, level: LogLevel, message: &str);
}

impl AppUiDelegate for MiniApp {
    fn get_tab_bar_config(&self) -> Result<String, MiniAppError> {
        // Handle TabBar configuration
        if let Some(tab_bar_json) = self.config.get_tabbar_json_with_base_path(&self.app_dir) {
            // self.info("TabBar", &tab_bar_json);
            Ok(tab_bar_json)
        } else {
            // TabBar is optional or invalid, return a valid empty tabbar JSON
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
        // Log the app opening event
        self.info("AppUiDelegate", format!("Mini app {} opened", self.appid));

        // Initialize app service for home app
        if let Ok(mut manager) = self.svc_manager.lock() {
            if let Err(e) = manager.create_app_svc(self.appid.clone(), self.app_dir.clone()) {
                self.error(
                    "AppUiDelegate",
                    format!("Failed to triger app service: {}", e),
                );
            }
            if let Err(e) = manager.app_svc(self.appid.clone(), "onLaunch".to_string(), None) {
                self.error(
                    "AppUiDelegate",
                    format!("Failed to triger onLaunch service: {}", e),
                );
            }
        }
    }

    fn on_miniapp_closed(&mut self) {
        // Update last active time
        self.last_active_time = Instant::now();

        // Log the app closing event
        self.info("AppUiDelegate", format!("Mini app {} closed", self.appid));
    }

    fn on_page_created(&mut self, path: String) {
        let url = format!("lx://{}", path.clone());
        let appid_clone = self.appid.clone();
        let controller_clone = self.controller.clone();

        // Create the page first
        let page = self.pages.create_page(
            appid_clone,
            path.clone(),
            controller_clone,
            self.svc_manager.clone(),
        );

        // Store the result of loading the URL
        let url_load_result = page.load_url(&url);

        // Check if debug mode is enabled in the app config
        let debug_enabled = self.config.is_debug_enabled();

        // Enable devtools if debug mode is enabled in config
        let devtools_result = if debug_enabled {
            page.set_devtools(true)
        } else {
            Ok(())
        };

        // Now we can use self again as the mutable borrow has ended
        if let Err(e) = url_load_result {
            self.error(&path, format!("Failed to load URL {}: {}", url, e));
        }

        if let Err(e) = devtools_result {
            self.error(&path, format!("Failed to enable devtools: {}", e));
        }

        self.info("AppUiDelegate", format!("Page {} created", path));
    }

    fn on_page_started(&self, _path: String) {
        // TODO
    }

    fn on_page_finished(&self, _path: String) {
        // TODO
    }

    fn on_page_show(&mut self, path: String) {
        self.pages.navigate_to_page(path);
    }

    fn on_back_pressed(&mut self) -> bool {
        self.info("AppUiDelegate", "Backbutton pressed");

        // Try to pop the current page from the stack
        if let Some(previous_page) = self.pages.pop_from_current_stack() {
            // it's at top tab page
            if self.config.is_initial_route(&previous_page)
                || self.config.is_tab_page(&previous_page)
            {
                return false;
            }

            self.info(
                "AppUiDelegate",
                format!("Popped page, switching back to: {}", previous_page),
            );

            // Request to switch to the previous page
            if let Err(e) = switch_page(&self.controller, &self.appid, &previous_page) {
                self.error(
                    "AppUiDelegate",
                    format!("Failed to switch to page {}: {}", previous_page, e),
                );
            }

            // Return true to indicate we handled the back press
            true
        } else {
            // No page to pop, return false to allow default back behavior
            false
        }
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
            "lx" => true,     // Always intercept lingxia scheme
            "https" => false, // Allow https URLs (they'll be checked in handle_request)
            _ => true,        // Block all other schemes
        }
    }

    fn handle_post_message(&self, path: String, msg: String) {
        let incoming = appservice::bridge::IncomingMessage::from_json_str(&msg).unwrap();

        if let Ok(manager) = self.svc_manager.lock() {
            if let Err(e) = manager.handle_view_message(self.appid.clone(), path, Arc::new(incoming)) {
                self.error(
                    "AppUiDelegate",
                    format!("Failed to create app service: {}", e),
                );
            }
        }
    }

    fn handle_request(&mut self, req: http::Request<Vec<u8>>) -> Option<http::Response<Vec<u8>>> {
        let uri = req.uri();
        let scheme = uri.scheme_str().unwrap_or("");

        // Use pattern matching for different URI schemes
        match scheme {
            // HTTPS requests - check domain whitelist and static resource types
            "https" => self.https_handler(req),

            // Lingxia scheme for internal app assets
            "lx" => self.lingxia_handler(req),

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
/// Returns an Option of (home_app_id, initial_route) on success.
pub fn init<T: AppController + 'static>(controller: T) -> Option<(String, String)> {
    let controller_arc = Arc::new(controller);

    // Prepare the directory structure
    if let Err(e) = prepare_directory_structure(controller_arc.as_ref()) {
        controller_arc.log(
            LogLevel::Error,
            &format!("Failed to prepare directory structure: {}", e),
        );
        return None;
    }

    match AppConfig::load(controller_arc.as_ref()) {
        Ok(config) => {
            let home_miniapp_id = config.home_mini_app_id.clone();
            let home_miniapp_version = &config.home_mini_app_version;
            let max_apps = config.max_allowed_miniapps;

            if !install::is_installed(controller_arc.as_ref(), &home_miniapp_id) {
                if let Err(e) = install::install_home_miniapp(
                    controller_arc.as_ref(),
                    &home_miniapp_id,
                    home_miniapp_version,
                ) {
                    controller_arc.log(
                        LogLevel::Error,
                        &format!("Failed to install home MiniApp: {}", e),
                    );

                    return None;
                }
            }

            let svc_manager = appservice::init(controller_arc.clone(), max_apps);

            // Now create the MiniApp instance and call setup
            // new_as_home itself calls setup(), which loads its app.json.
            let home_miniapp = MiniApp::new_as_home(
                home_miniapp_id.clone(),
                controller_arc.clone(),
                svc_manager.clone(),
            );

            // Check if home mini app needs updating after loading its configuration
            if home_miniapp.config.is_debug_enabled()
                || home_miniapp.should_update(home_miniapp_version)
            {
                if let Err(e) = install::install_home_miniapp(
                    controller_arc.as_ref(),
                    &home_miniapp_id,
                    home_miniapp_version,
                ) {
                    controller_arc.log(
                        LogLevel::Error,
                        &format!("Failed to install home MiniApp: {}", e),
                    );

                    return None;
                }
            }

            // Get the initial route from the now-configured home_miniapp
            let initial_route = home_miniapp.config.get_initial_route();

            // Initialize MiniApps collection
            let mut miniapps = MiniApps::new(controller_arc.clone(), svc_manager, max_apps);

            // Wrap the home miniapp in Arc<RwLock<>> and add it to the collection
            let home_miniapp_arc = Arc::new(RwLock::new(home_miniapp));

            // Add home mini app to the collection
            miniapps
                .miniapps
                .insert(home_miniapp_id.clone(), home_miniapp_arc);

            if MINIAPPS.set(RwLock::new(miniapps)).is_err() {
                controller_arc.log(
                    LogLevel::Error,
                    "MiniApps singleton had been initialized by another instance",
                );
                None
            } else {
                controller_arc.log(LogLevel::Info, "MiniApps initialized successfully");
                Some((home_miniapp_id, initial_route))
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

            controller_arc.log(LogLevel::Error, &error_message);
            None
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

    // If the miniapp already exists, return it directly
    if let Some(app_arc) = miniapps.miniapps.get(&appid) {
        return app_arc.clone();
    }

    // Check if we've reached the maximum number of apps
    if miniapps.miniapps.len() >= miniapps.max_apps {
        // Destroy the least active app to make room
        miniapps.destroy_least_active_miniapp();
    }

    let controller = miniapps.controller.clone();

    // Create new MiniApp with app_service_manager
    let unit = MiniApp::new(appid.clone(), controller, miniapps.svc_manager.clone());
    let app_arc = Arc::new(RwLock::new(unit));

    // Insert into collection and return
    miniapps.miniapps.insert(appid, app_arc.clone());
    app_arc
}

/// Uninstall a mini app by removing its files and version record
///
/// This function uninstalls a specified mini app, removing all its files and data.
/// It protects the home mini app from being uninstalled.
///
/// # Arguments
/// * `appid` - The ID of the mini app to uninstall
///
/// # Returns
/// * `Ok(())` - If the mini app was uninstalled successfully
/// * `Err(MiniAppError)` - If there was an error during uninstallation
///
/// # Panics
/// Panics if `MiniApps` is not initialized
pub fn uninstall_miniapp(appid: &str) -> Result<(), MiniAppError> {
    let miniapps = MINIAPPS
        .get()
        .expect("MiniApps not initialized")
        .read()
        .unwrap();

    // Get controller to log operation
    let controller = miniapps.controller.clone();
    controller.log(LogLevel::Info, &format!("Uninstalling mini app: {}", appid));

    // Get or create the miniapp instance
    let app_arc = get_or_init_miniapp(appid.to_string());

    // Call the uninstall method on the MiniApp instance
    let result = app_arc.write().unwrap().uninstall();

    // If successful, remove the app from the collection
    if result.is_ok() {
        // Drop read lock and acquire write lock to modify collection
        drop(miniapps);
        let mut miniapps_write = MINIAPPS
            .get()
            .expect("MiniApps not initialized")
            .write()
            .unwrap();

        miniapps_write.miniapps.remove(appid);
    }

    result
}
