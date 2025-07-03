use dashmap::DashMap;
use http::{Response, StatusCode};
use rong::FromJSObj;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use crate::app::AppConfig;
use crate::appservice::{self, MiniAppServiceManager};
use crate::error::MiniAppError;
use crate::log::{self, LogLevel, LogTag};
pub use crate::page::PageState;
use crate::page::{Page, Pages};
use crate::{AppRuntime, error, info};
use config::{MiniAppConfig, PageConfig};
use security::NetworkSecurity; // Import the new logging macros

mod config;
mod content;
mod install;
mod scheme;
mod security;
mod tabbar;
mod version;

/// Constants for miniapp storage layout
const LINGXIA_DIR: &str = "lingxia";
const LXAPPS_DIR: &str = "lxapps";
const VERSIONS_DIR: &str = "versions";
const STORAGE_DIR: &str = "storage";
const USER_DATA_DIR: &str = "userdata";
const USER_CACHE_DIR: &str = "usercache";

const DEFAULT_USER_ID: &str = "default";
const DEFAULT_VERSION: &str = "0.0.1";

/// Manages a collection of mini applications
pub struct MiniApps {
    /// Collection of mini apps, keyed by app ID
    /// Uses DashMap for thread-safe concurrent access
    miniapps: DashMap<String, Arc<MiniApp>>,

    /// Reference to the platform-specific app runtime
    /// Provides file system access, UI callbacks, etc.
    runtime: Arc<dyn AppRuntime>,

    /// Maximum number of apps allowed in memory simultaneously
    /// When exceeded, least recently used apps are destroyed
    max_apps: usize,

    /// Reference to the app service manager
    /// Handles background services for mini apps
    svc_manager: Arc<Mutex<MiniAppServiceManager>>,

    /// Current user ID (hashed for privacy)
    /// Used to generate directory names for user-specific storage
    user_id: Mutex<String>,
}

impl MiniApps {
    fn new<T: AppRuntime + 'static>(
        runtime: T,
        svc_manager: Arc<Mutex<MiniAppServiceManager>>,
        max_apps: usize,
    ) -> Self {
        let runtime = Arc::new(runtime);

        // Initialize with default user ID
        let user_id = Mutex::new(DEFAULT_USER_ID.to_string());

        Self {
            miniapps: DashMap::new(),
            runtime,
            max_apps,
            svc_manager,
            user_id,
        }
    }

    /// Set the current user ID for all LingXia apps
    ///
    /// This will affect the directory structure for new lx apps.
    /// Existing lx apps will continue to use their current directories.
    ///
    /// The user ID is used to generate hashed directory names for privacy protection.
    /// Each user gets isolated storage and cache directories.
    ///
    /// # Arguments
    /// * `new_user_id` - The new user ID to use (will be hashed for directory names)
    /// ```
    pub fn set_user_id(&self, new_user_id: String) {
        if let Ok(mut user_id) = self.user_id.lock() {
            *user_id = new_user_id;
        }
    }

    /// Get the current user ID
    fn get_user_id(&self) -> String {
        match self.user_id.lock() {
            Ok(user_id) => user_id.clone(),
            Err(_) => {
                // If lock is poisoned, return default
                DEFAULT_USER_ID.to_string()
            }
        }
    }

    /// Get or initialize a specific MiniApp instance by appid
    pub fn get_or_init_miniapp(&self, appid: String) -> Arc<MiniApp> {
        // If the miniapp already exists, return it directly
        if let Some(app_arc) = self.miniapps.get(&appid) {
            return app_arc.clone();
        }

        // Check if we've reached the maximum number of apps
        if self.miniapps.len() >= self.max_apps {
            // Find and remove the least active app to make room
            self.destroy_least_active_miniapp();
        }

        // Create new MiniApp
        let new_miniapp = Arc::new(MiniApp::new(
            appid.clone(),
            self.runtime.clone(),
            self.svc_manager.clone(),
        ));

        // Insert into collection and return
        self.miniapps.insert(appid, new_miniapp.clone());
        new_miniapp
    }

    /// Destroys the least recently active mini app to free up memory
    fn destroy_least_active_miniapp(&self) {
        if self.miniapps.is_empty() {
            return;
        }

        // Find the least active app that isn't the home app
        let least_active = self
            .miniapps
            .iter()
            .filter_map(|entry| {
                let (appid, app_arc) = entry.pair();
                let state = app_arc.state.lock().unwrap();
                // Skip home app from destruction
                if app_arc.home_miniapp {
                    None
                } else {
                    Some((appid.clone(), state.last_active_time))
                }
            })
            .min_by(|(_, time1), (_, time2)| time1.cmp(time2));

        // If we found a non-home app, remove it
        if let Some((appid, _)) = least_active {
            info!("Destroying least active mini app").with_appid(appid.clone());

            // Clean up app service before removing the app
            if let Ok(mut svc_manager) = self.svc_manager.lock() {
                if let Err(e) = svc_manager.terminate_app_svc(appid.clone()) {
                    error!("Failed to terminate app service: {}", e).with_appid(appid.clone());
                }
            }

            self.miniapps.remove(&appid);
        }
    }

    /// Uninstall a mini app by removing its files and version record
    pub fn uninstall_miniapp(&self, appid: &str) -> Result<(), MiniAppError> {
        // Log operation
        info!("Uninstalling mini app").with_appid(appid);

        // Get or create the miniapp instance
        let app_arc = self.get_or_init_miniapp(appid.to_string());

        // Call the uninstall method on the MiniApp instance
        let result = app_arc.uninstall();

        // If successful, remove the app from the collection
        if result.is_ok() {
            self.miniapps.remove(appid);
        }

        result
    }
}

/// Mutable state of a MiniApp that requires synchronization
pub(crate) struct MiniAppState {
    /// Collection of pages in this app with their current states
    /// Manages page lifecycle (show/hide/destroy)
    pages: Pages,

    /// Time when this app was last active
    /// Used for LRU (Least Recently Used) eviction when memory is low
    last_active_time: Instant,

    /// Debug mode override (can be enabled at runtime)
    /// When true, enables additional logging and debugging features
    debug: bool,

    /// Whether the app is currently opened or closed
    /// Controls app lifecycle and resource allocation
    opened: bool,

    /// Network security configuration for HTTPS domain filtering
    /// Manages which domains this app is allowed to access
    network_security: NetworkSecurity,
}

impl MiniAppState {
    fn new() -> Self {
        Self {
            pages: Pages::new(),
            last_active_time: Instant::now(),
            debug: false,
            opened: false,
            network_security: NetworkSecurity::new(),
        }
    }
}

/// Represents a single mini application
pub struct MiniApp {
    // Immutable data - initialized once and never changed
    pub appid: String,
    pub runtime: Arc<dyn AppRuntime>,
    pub lxapp_dir: PathBuf,
    pub user_data_dir: PathBuf,
    pub storage_dir: PathBuf,
    pub user_cache_dir: PathBuf,
    pub home_miniapp: bool,
    pub version: String,
    config: MiniAppConfig,
    svc_manager: Arc<Mutex<MiniAppServiceManager>>,

    // Mutable state - protected by mutex for fine-grained locking
    pub(crate) state: Mutex<MiniAppState>,
}

#[derive(FromJSObj)]
pub struct MiniAppNavigator {
    #[rename = "appId"]
    pub appid: String,
    pub path: String,
}

impl MiniApp {
    fn _new(
        appid: String,
        runtime: Arc<dyn AppRuntime>,
        svc_manager: Arc<Mutex<MiniAppServiceManager>>,
    ) -> Self {
        Self {
            appid,
            runtime,
            lxapp_dir: PathBuf::new(),
            storage_dir: PathBuf::new(),
            user_data_dir: PathBuf::new(),
            user_cache_dir: PathBuf::new(),
            home_miniapp: false,
            version: String::new(),
            config: MiniAppConfig::default(),
            svc_manager,
            state: Mutex::new(MiniAppState::new()),
        }
    }

    /// Create a new regular mini-app (not home app)
    fn new(
        appid: String,
        runtime: Arc<dyn AppRuntime>,
        svc_manager: Arc<Mutex<MiniAppServiceManager>>,
    ) -> Self {
        let mut app = Self::_new(appid, runtime, svc_manager);
        if let Err(e) = app.setup() {
            error!("Setup failed: {}", e).with_appid(&app.appid);
        }

        app
    }

    /// Create a new MiniApp instance marked as the home mini app
    fn new_as_home(
        appid: String,
        runtime: Arc<dyn AppRuntime>,
        svc_manager: Arc<Mutex<MiniAppServiceManager>>,
    ) -> Self {
        let mut app = Self::_new(appid, runtime, svc_manager);

        // Mark as home miniapp
        app.home_miniapp = true;

        if let Err(e) = app.setup() {
            error!("Setup failed for home app: {}", e).with_appid(&app.appid);
        }

        app
    }

    // Setup will initialize paths and load config
    fn setup(&mut self) -> Result<(), MiniAppError> {
        // Get the app's version
        self.version = self.read_version();

        // Calculate the directory name based on appid, user and whether this is a home app
        let dir_name = if self.home_miniapp {
            // Home mini app uses appid directly as directory name
            self.appid.clone()
        } else {
            let user_id = MINIAPPS_MANAGER.get().unwrap().get_user_id();
            generate_app_hash(&self.appid, &user_id)
        };

        // Set up app directory
        let base_dir = self
            .runtime
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(LXAPPS_DIR);

        self.lxapp_dir = base_dir.join(&dir_name);
        if !self.lxapp_dir.exists() {
            std::fs::create_dir_all(&self.lxapp_dir).map_err(|e| {
                MiniAppError::IoError(format!("Failed to create lx apps directory: {}", e))
            })?;
        }

        self.storage_dir = self
            .runtime
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(STORAGE_DIR);

        // Set up userdata directory
        let userdata_base_dir = self
            .runtime
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(USER_DATA_DIR);

        self.user_data_dir = userdata_base_dir.join(&dir_name);
        if !self.user_data_dir.exists() {
            std::fs::create_dir_all(&self.user_data_dir).map_err(|e| {
                MiniAppError::IoError(format!("Failed to create user data directory: {}", e))
            })?;
        }

        // Set up cache directory
        let cache_base_dir = self
            .runtime
            .app_cache_dir()
            .join(LINGXIA_DIR)
            .join(USER_CACHE_DIR);

        self.user_cache_dir = cache_base_dir.join(&dir_name);
        if !self.user_cache_dir.exists() {
            std::fs::create_dir_all(&self.user_cache_dir).map_err(|e| {
                MiniAppError::IoError(format!("Failed to create cache directory: {}", e))
            })?;
        }

        // Load app configuration if it exists
        self.read_json("app.json")
            .map(|app_json| {
                self.config = MiniAppConfig::from_value(app_json)
                    .map_err(|e| MiniAppError::InvalidJsonFile(format!("app.json: {}", e)))?;

                // Set tabbar items in the state
                let mut state = self.state.lock().unwrap();
                state.pages.set_tabbar_items(self.config.get_tab_pages());
                Ok(())
            })
            .inspect_err(|_| {
                let mut state = self.state.lock().unwrap();
                state.debug = true;
            })?
    }

    /// Get the version of this app from storage
    fn read_version(&self) -> String {
        let version_path = self
            .runtime
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
        let file_path = self.lxapp_dir.join(relative_path);

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

    pub fn is_debug_enabled(&self) -> bool {
        self.state.lock().unwrap().debug || self.config.is_debug_enabled()
    }

    pub fn is_opened(&self) -> bool {
        self.state.lock().unwrap().opened
    }

    /// Get a page by path
    pub fn get_page(&self, path: &str) -> Option<Page> {
        let state = self.state.lock().unwrap();
        state.pages.get_page(path).cloned()
    }

    /// This method should only be called when page is in Created state
    fn setup_page(&self, page: &Page, path: &str) {
        let state = page.get_page_state();
        if state != PageState::Created {
            return;
        }

        // Enable devtools if debug mode is enabled
        if self.is_debug_enabled() {
            if let Err(e) = page.webview_controller().set_devtools(true) {
                error!("Failed to enable devtools: {}", e)
                    .with_appid(self.appid.clone())
                    .with_path(path.to_string());
            }
        }

        // Load HTML - this might fail on HarmonyOS if WebView isn't ready yet
        let html_data = self.generate_page_html(path);
        match page.load_html(
            String::from_utf8_lossy(&html_data).to_string(),
            "lx://./".to_string(),
        ) {
            Ok(_) => {
                // HTML loaded successfully
                page.set_page_state(PageState::Loaded);
            }
            Err(e) => {
                error!("Failed to load HTML: {}", e)
                    .with_appid(self.appid.clone())
                    .with_path(path.to_string());
            }
        }
    }

    pub fn navigator_to_miniapp(&self, to: MiniAppNavigator) -> Result<(), MiniAppError> {
        // ignore if appid is the same
        if self.appid == to.appid {
            return Ok(());
        }

        if let Some(manager) = MINIAPPS_MANAGER.get() {
            let app = manager.get_or_init_miniapp(to.appid.clone());
            if !app.is_opened() {
                app.runtime.open_miniapp(to.appid, to.path)?;
            }
        }
        Ok(())
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
            .runtime
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(VERSIONS_DIR)
            .join(format!("{}.txt", self.appid));

        if version_path.exists() {
            fs::remove_file(&version_path)?
        }

        //  Remove the app directory
        if self.lxapp_dir.exists() {
            fs::remove_dir_all(&self.lxapp_dir)?;
        }

        // Remove the storage directory
        if self.storage_dir.exists() {
            fs::remove_dir_all(&self.storage_dir)?;
        }

        //  Remove the cache directory
        if self.user_cache_dir.exists() {
            fs::remove_dir_all(&self.user_cache_dir)?;
        }

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

/// Prepares the base directory structure for mini apps
fn prepare_directory_structure<T: AppRuntime + ?Sized>(runtime: &T) -> Result<(), MiniAppError> {
    let data_dir = runtime.app_data_dir();
    let cache_dir = runtime.app_cache_dir();

    // Create required directories
    let dirs = [
        data_dir.join(LINGXIA_DIR).join(LXAPPS_DIR),
        data_dir.join(LINGXIA_DIR).join(VERSIONS_DIR),
        data_dir.join(LINGXIA_DIR).join(USER_DATA_DIR),
        data_dir.join(LINGXIA_DIR).join(STORAGE_DIR),
        cache_dir.join(LINGXIA_DIR).join(LXAPPS_DIR),
    ];

    for dir in &dirs {
        fs::create_dir_all(dir)?;
    }

    Ok(())
}

pub trait AppUiDelegate {
    /// Get tabbar configuration for mini app
    fn get_tab_bar_config(self: &Arc<Self>) -> Result<String, MiniAppError>;

    /// Get page configuration for a specific page
    fn get_page_config(self: &Arc<Self>, path: &str) -> Result<String, MiniAppError>;

    /// Called when mini app is opened
    fn on_miniapp_opened(self: Arc<Self>, path: String);

    /// Called when mini app is closed
    fn on_miniapp_closed(self: &Arc<Self>);

    /// Called when the page starts loading
    fn on_page_started(self: &Arc<Self>, path: String);

    /// Called when the page finishes loading
    fn on_page_finished(self: &Arc<Self>, path: String);

    /// Called when the page showed in the view
    fn on_page_show(self: &Arc<Self>, path: String);

    // Called when Scroll changed
    fn on_page_scroll_changed(
        self: &Arc<Self>,
        path: String,
        scroll_x: i32,
        scroll_y: i32,
        max_scroll_x: i32,
        max_scroll_y: i32,
    );

    /// Handle back button press
    /// Return true to indicate the back press had been handled
    fn on_back_pressed(self: &Arc<Self>) -> bool;

    /// Determines whether to override URL loading in the page.
    ///
    /// # Arguments
    /// * `url` - The URL being requested
    ///
    /// # Returns
    /// * `true` - To intercept and handle the URL loading
    /// * `false` - To allow the page to continue loading the URL
    fn should_override_url_loading(self: &Arc<Self>, url: String) -> bool;

    /// Handles a postMessage from the page View(WebView)
    fn handle_post_message(self: &Arc<Self>, path: String, msg: String);

    /// Handles an HTTP request from the page
    fn handle_request(
        self: &Arc<Self>,
        req: http::Request<Vec<u8>>,
    ) -> Option<http::Response<Vec<u8>>>;

    /// Receive log from WebView
    fn log(self: &Arc<Self>, path: &str, level: LogLevel, message: &str);
}

impl AppUiDelegate for MiniApp {
    fn get_tab_bar_config(self: &Arc<Self>) -> Result<String, MiniAppError> {
        // Handle TabBar configuration
        if let Some(tab_bar_json) = self.config.get_tabbar_json_with_base_path(&self.lxapp_dir) {
            // crate::info!("TabBar: {}", tab_bar_json);
            Ok(tab_bar_json)
        } else {
            // TabBar is optional or invalid, return a valid empty tabbar JSON
            Ok("{}".to_string())
        }
    }

    fn get_page_config(self: &Arc<Self>, path: &str) -> Result<String, MiniAppError> {
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

    fn on_miniapp_opened(self: Arc<Self>, path: String) {
        info!("Mini app opened")
            .with_appid(self.appid.clone())
            .with_path(path.clone());

        // Use the Arc<Self> directly instead of looking it up in the global manager
        if let Ok(mut svc_manager) = self.svc_manager.lock() {
            if let Err(e) = svc_manager.create_app_svc(self.clone()) {
                error!("Failed to trigger app service: {}", e).with_appid(self.appid.clone());
            }
            if let Err(e) = svc_manager.app_svc(self.appid.clone(), "onLaunch".to_string(), None) {
                error!("Failed to trigger onLaunch service: {}", e).with_appid(self.appid.clone());
            }
        }

        // Create the page for the given path if it doesn't exist
        // This path is typically the initial_route.
        let mut state = self.state.lock().unwrap();
        if let Err(e) = state.pages.get_or_create_page(
            self.appid.clone(),
            path.clone(),
            self.runtime.clone(),
            self.svc_manager.clone(),
        ) {
            error!("Failed to create page for initial_route: {}", e)
                .with_appid(self.appid.clone())
                .with_path(path.clone());
        }
        state.opened = true;
    }

    fn on_miniapp_closed(self: &Arc<Self>) {
        self.state.lock().unwrap().opened = false;

        // Update last active time
        self.state.lock().unwrap().last_active_time = Instant::now();

        // Log the app closing event
        info!("Mini app closed").with_appid(self.appid.clone());
    }

    fn on_page_started(self: &Arc<Self>, path: String) {
        if let Ok(manager) = self.svc_manager.lock() {
            let _ = manager.invoke_page_function(
                self.appid.clone(),
                path.clone(),
                "onLoad".to_string(),
                None,
            );
        }
    }

    fn on_page_finished(self: &Arc<Self>, path: String) {
        if let Ok(manager) = self.svc_manager.lock() {
            let _ = manager.invoke_page_function(
                self.appid.clone(),
                path.clone(),
                "onReady".to_string(),
                None,
            );
        }
    }

    fn on_page_show(self: &Arc<Self>, path: String) {
        info!("on_page_show called for path: {}", path)
            .with_appid(self.appid.clone())
            .with_path(path.clone());

        // Get the page (should already exist)
        let page = {
            let state = self.state.lock().unwrap();
            state.pages.get_page(&path).cloned()
        };

        let page = match page {
            Some(page) => page,
            None => {
                error!("Page not found: {}", path)
                    .with_appid(self.appid.clone())
                    .with_path(path.clone());
                return;
            }
        };

        // Setup page if it hasn't been setup yet
        self.setup_page(&page, &path);

        // Navigate to the new page and get the previous page if there was a switch
        let previous_page = self
            .state
            .lock()
            .unwrap()
            .pages
            .navigate_to_page(path.clone());

        if let Ok(manager) = self.svc_manager.lock() {
            // Call onHide for the previous page if there was a page switch
            if let Some(prev_path) = previous_page {
                if let Err(e) = manager.invoke_page_function(
                    self.appid.clone(),
                    prev_path.clone(),
                    "onHide".to_string(),
                    None,
                ) {
                    error!("Failed to call onHide for page {}: {}", prev_path, e)
                        .with_appid(self.appid.clone());
                }
            }

            // Call onShow for the new page
            if let Err(e) = manager.invoke_page_function(
                self.appid.clone(),
                path.clone(),
                "onShow".to_string(),
                None,
            ) {
                error!("Failed to call onShow: {}", e)
                    .with_appid(self.appid.clone())
                    .with_path(path.clone());
            }
        }

        // precreate webviews for other tab pages
        let current_state = page.get_page_state();
        let should_precreate =
            self.config.is_initial_route(&path) && current_state == PageState::Loaded;

        if should_precreate {
            let tab_pages = self.config.get_tab_pages();
            for p in tab_pages {
                if p == path {
                    continue;
                }

                // Create new page and setup content
                let mut state = self.state.lock().unwrap();
                if let Ok(page) = state.pages.create_page(
                    self.appid.clone(),
                    p.clone(),
                    self.runtime.clone(),
                    self.svc_manager.clone(),
                ) {
                    drop(state); // Release lock before setup

                    // On HarmonyOS, setup_page might fail if WebView isn't ready yet
                    // This is OK - the page will be setup when onPageShow is called for that tab
                    self.setup_page(&page, &p);
                }
            }
        }

        // Mark page as shown (at the end of on_page_show)
        page.set_page_state(PageState::Showed);
    }

    fn on_page_scroll_changed(
        self: &Arc<Self>,
        _path: String,
        scroll_x: i32,
        scroll_y: i32,
        max_scroll_x: i32,
        max_scroll_y: i32,
    ) {
        // safe division to avoid division by zero
        let scroll_percent_x = if max_scroll_x > 0 {
            (scroll_x as f64 / max_scroll_x as f64 * 100.0) as i32
        } else {
            0
        };

        let scroll_percent_y = if max_scroll_y > 0 {
            (scroll_y as f64 / max_scroll_y as f64 * 100.0) as i32
        } else {
            0
        };

        info!(
            "Scroll: x={}/{} ({}%), y={}/{} ({}%)",
            scroll_x, max_scroll_x, scroll_percent_x, scroll_y, max_scroll_y, scroll_percent_y
        );
    }

    fn on_back_pressed(self: &Arc<Self>) -> bool {
        info!("Backbutton pressed").with_appid(self.appid.clone());

        // Try to pop the current page from the stack
        if let Some(previous_page) = self.state.lock().unwrap().pages.pop_from_current_stack() {
            // it's at top tab page
            if self.config.is_initial_route(&previous_page)
                || self.config.is_tab_page(&previous_page)
            {
                return false;
            }

            info!("Popped page, switching back to: {}", previous_page)
                .with_appid(self.appid.clone());

            // Request to switch to the previous page
            if let Err(e) = self
                .runtime
                .switch_page(self.appid.clone(), previous_page.clone())
            {
                error!("Failed to switch to page {}: {}", previous_page, e)
                    .with_appid(self.appid.clone());
            }

            // Return true to indicate we handled the back press
            true
        } else {
            // No page to pop, return false to allow default back behavior
            false
        }
    }

    // Determines whether to override URL loading in the page.
    fn should_override_url_loading(self: &Arc<Self>, url: String) -> bool {
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

    fn handle_post_message(self: &Arc<Self>, path: String, msg: String) {
        let incoming = appservice::bridge::IncomingMessage::from_json_str(&msg).unwrap();

        if let Ok(manager) = self.svc_manager.lock() {
            if let Err(e) =
                manager.handle_view_message(self.appid.clone(), path, Arc::new(incoming))
            {
                error!("Failed to create app service: {}", e).with_appid(self.appid.clone());
            }
        }
    }

    fn handle_request(
        self: &Arc<Self>,
        req: http::Request<Vec<u8>>,
    ) -> Option<http::Response<Vec<u8>>> {
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

    fn log(self: &Arc<Self>, path: &str, level: LogLevel, message: &str) {
        log::LogBuilder::new(LogTag::WebViewConsole, message)
            .with_level(level)
            .with_path(path)
            .with_appid(self.appid.clone());
    }
}

// Global instance of MiniApps manager
static MINIAPPS_MANAGER: OnceLock<Arc<MiniApps>> = OnceLock::new();

/// Initialize the MiniApps singleton
/// Returns an Option of (home_app_id, initial_route) on success.
pub fn init<R: AppRuntime + 'static>(runtime: R) -> Option<(String, String)> {
    // Set up panic hook to capture panic information
    std::panic::set_hook(Box::new(|panic_info| {
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic message".to_string()
        };

        error!("RUST PANIC: {} at {}", message, location);
    }));

    let runtime_arc = Arc::new(runtime);

    // Prepare the directory structure
    if let Err(e) = prepare_directory_structure(runtime_arc.as_ref()) {
        error!("Failed to prepare directory structure: {}", e);
        return None;
    }

    match AppConfig::load(runtime_arc.as_ref()) {
        Ok(config) => {
            let home_miniapp_id = config.home_mini_app_id.clone();
            let home_miniapp_version = &config.home_mini_app_version;
            let max_apps = config.max_allowed_miniapps;

            if !install::is_installed(runtime_arc.as_ref(), &home_miniapp_id) {
                if let Err(e) = install::install_home_miniapp(
                    runtime_arc.as_ref(),
                    &home_miniapp_id,
                    home_miniapp_version,
                ) {
                    error!("Failed to install home MiniApp: {}", e);
                    return None;
                }
            }

            let svc_manager = appservice::init(max_apps);

            // Create the home MiniApp instance
            let home_miniapp = MiniApp::new_as_home(
                home_miniapp_id.clone(),
                runtime_arc.clone(),
                svc_manager.clone(),
            );

            // Check if home mini app needs updating after loading its configuration
            if home_miniapp.is_debug_enabled() || home_miniapp.should_update(home_miniapp_version) {
                if let Err(e) = install::install_home_miniapp(
                    runtime_arc.as_ref(),
                    &home_miniapp_id,
                    home_miniapp_version,
                ) {
                    error!("Failed to install home MiniApp: {}", e);

                    return None;
                }
            }

            // Get the initial route from the configured home_miniapp
            let initial_route = home_miniapp.config.get_initial_route();

            // Create MiniApps manager
            let miniapps_manager =
                Arc::new(MiniApps::new(runtime_arc.clone(), svc_manager, max_apps));

            // Add home miniapp to the manager
            let home_miniapp_arc = Arc::new(home_miniapp);
            miniapps_manager
                .miniapps
                .insert(home_miniapp_id.clone(), home_miniapp_arc.clone());

            // Set global instance
            if MINIAPPS_MANAGER.set(miniapps_manager).is_err() {
                error!("MiniApps manager singleton had been initialized by another instance");
                return None;
            }

            info!("MiniApps initialized successfully");
            Some((home_miniapp_id, initial_route))
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

            error!("{}", error_message);
            None
        }
    }
}

/// Get a specific MiniApp instance by appid
///
/// If the MiniApp with the given appid exists, it returns a reference to it.
///
/// # Arguments
/// * `appid` - The ID of the mini app to get or create
///
/// # Returns
/// A thread-safe reference to the MiniApp
///
/// # Panics
/// Panics if `MiniApps` is not initialized or MiniApp doesn't exist
pub fn get(appid: String) -> Arc<MiniApp> {
    let manager = MINIAPPS_MANAGER.get().expect("MiniApps not initialized");
    if let Some(app_arc) = manager.miniapps.get(&appid) {
        return app_arc.clone();
    }
    panic!("Not found miniapp {}", appid);
}
