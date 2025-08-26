use dashmap::DashMap;
use rong::FromJSObj;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use crate::app::AppConfig;
use crate::error::LxAppError;
use crate::executor::LxAppExecutor;
pub use crate::page::PageState;
use crate::page::{Page, Pages};
use crate::{AppRuntime, error, info};
use security::NetworkSecurity; // Import the new logging macros

pub mod config;
use config::LxAppConfig;
mod content;
mod install;
mod scheme;
mod security;
pub mod tabbar;
mod version;

/// Constants for lxapp storage layout
const LINGXIA_DIR: &str = "lingxia";
const LXAPPS_DIR: &str = "lxapps";
const VERSIONS_DIR: &str = "versions";
const STORAGE_DIR: &str = "storage";
const USER_DATA_DIR: &str = "userdata";
const USER_CACHE_DIR: &str = "usercache";

const DEFAULT_USER_ID: &str = "default";
const DEFAULT_VERSION: &str = "0.0.1";

/// Manages a collection of mini applications
pub struct LxApps {
    /// Collection of mini apps, keyed by app ID
    /// Uses DashMap for thread-safe concurrent access
    lxapps: DashMap<String, Arc<LxApp>>,

    /// Reference to the platform-specific app runtime
    /// Provides file system access, UI callbacks, etc.
    runtime: Arc<dyn AppRuntime>,

    /// Maximum number of apps allowed in memory simultaneously
    /// When exceeded, least recently used apps are destroyed
    max_apps: usize,

    /// Reference to the executor
    /// Handles async task execution for mini apps
    pub(crate) executor: Arc<LxAppExecutor>,

    /// Current user ID (hashed for privacy)
    /// Used to generate directory names for user-specific storage
    user_id: Mutex<String>,
}

impl LxApps {
    fn new<T: AppRuntime + 'static>(
        runtime: T,
        executor: Arc<LxAppExecutor>,
        max_apps: usize,
    ) -> Self {
        let runtime = Arc::new(runtime);

        // Initialize with default user ID
        let user_id = Mutex::new(DEFAULT_USER_ID.to_string());

        Self {
            lxapps: DashMap::new(),
            runtime,
            max_apps,
            executor,
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

    /// Get or initialize a specific LxApp instance by appid
    fn get_or_init_lxapp(&self, appid: String) -> Arc<LxApp> {
        // If the lxapp already exists, return it directly
        if let Some(app_arc) = self.lxapps.get(&appid) {
            return app_arc.clone();
        }

        // Check if we've reached the maximum number of apps
        if self.lxapps.len() >= self.max_apps {
            // Find and remove the least active app to make room
            self.destroy_least_active_lxapp();
        }

        // Create new LxApp
        let new_lxapp = Arc::new(LxApp::new(
            appid.clone(),
            self.runtime.clone(),
            self.executor.clone(),
        ));

        // Insert into collection and return
        self.lxapps.insert(appid, new_lxapp.clone());
        new_lxapp
    }

    /// Destroys the least recently active mini app to free up memory
    fn destroy_least_active_lxapp(&self) {
        if self.lxapps.is_empty() {
            return;
        }

        // Find the least active app that isn't the home app
        let least_active = self
            .lxapps
            .iter()
            .filter_map(|entry| {
                let (appid, app_arc) = entry.pair();
                let state = app_arc.state.lock().unwrap();
                // Skip home app from destruction
                if app_arc.home_lxapp {
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
            if let Err(e) = self.executor.terminate_app_svc(appid.clone()) {
                error!("Failed to terminate app service: {}", e).with_appid(appid.clone());
            }

            self.lxapps.remove(&appid);
        }
    }

    /// Uninstall a mini app by removing its files and version record
    pub fn uninstall_lxapp(&self, appid: &str) -> Result<(), LxAppError> {
        // Log operation
        info!("Uninstalling lxapp").with_appid(appid);

        // Get or create the lxapp instance
        let app_arc = self.get_or_init_lxapp(appid.to_string());

        // Call the uninstall method on the LxApp instance
        let result = app_arc.uninstall();

        // If successful, remove the app from the collection
        if result.is_ok() {
            self.lxapps.remove(appid);
        }

        result
    }
}

/// Mutable state of a LxApp that requires synchronization
pub(crate) struct LxAppState {
    /// Collection of pages in this app with their current states
    /// Manages page lifecycle (show/hide/destroy)
    pub pages: Pages,

    /// Time when this app was last active
    /// Used for LRU (Least Recently Used) eviction when memory is low
    pub last_active_time: Instant,

    /// Debug mode override (can be enabled at runtime)
    /// When true, enables additional logging and debugging features
    debug: bool,

    /// Whether the app is currently opened or closed
    /// Controls app lifecycle and resource allocation
    pub opened: bool,

    /// Network security configuration for HTTPS domain filtering
    /// Manages which domains this app is allowed to access
    network_security: NetworkSecurity,

    /// TabBar runtime state
    /// Contains TabBar configuration and dynamic state (badges, red dots, visibility)
    pub tabbar: Option<tabbar::TabBar>,
}

impl LxAppState {
    fn new() -> Self {
        Self {
            pages: Pages::new(),
            last_active_time: Instant::now(),
            debug: false,
            opened: false,
            network_security: NetworkSecurity::new(),
            tabbar: None,
        }
    }
}

/// Represents a single mini application
pub struct LxApp {
    // Immutable data - initialized once and never changed
    pub appid: String,
    pub runtime: Arc<dyn AppRuntime>,
    pub lxapp_dir: PathBuf,
    pub storage_dir: PathBuf,
    pub user_data_dir: PathBuf,
    pub user_cache_dir: PathBuf,
    pub home_lxapp: bool,
    pub version: String,
    pub(crate) config: LxAppConfig,
    pub(crate) executor: Arc<LxAppExecutor>,

    // Mutable state - protected by mutex for fine-grained locking
    pub(crate) state: Mutex<LxAppState>,
}

#[derive(FromJSObj)]
pub struct LxAppNavigator {
    #[rename = "appId"]
    pub appid: String,
    pub path: String,
}

impl LxApp {
    fn _new(appid: String, runtime: Arc<dyn AppRuntime>, executor: Arc<LxAppExecutor>) -> Self {
        Self {
            appid,
            runtime,
            lxapp_dir: PathBuf::new(),
            storage_dir: PathBuf::new(),
            user_data_dir: PathBuf::new(),
            user_cache_dir: PathBuf::new(),
            home_lxapp: false,
            version: String::new(),
            config: LxAppConfig::default(),
            executor,
            state: Mutex::new(LxAppState::new()),
        }
    }

    /// Create a new regular mini-app (not home app)
    fn new(appid: String, runtime: Arc<dyn AppRuntime>, executor: Arc<LxAppExecutor>) -> Self {
        let mut app = Self::_new(appid, runtime, executor);
        if let Err(e) = app.setup() {
            error!("Setup failed: {}", e).with_appid(&app.appid);
        }

        app
    }

    /// Create a new LxApp instance marked as the home mini app
    fn new_as_home(
        appid: String,
        runtime: Arc<dyn AppRuntime>,
        executor: Arc<LxAppExecutor>,
    ) -> Self {
        let mut app = Self::_new(appid, runtime, executor);

        // Mark as home lxapp
        app.home_lxapp = true;

        if let Err(e) = app.setup() {
            error!("Setup failed for home app: {}", e).with_appid(&app.appid);
        }

        app
    }

    /// Initialize paths and directories for the lxapp
    fn initialize_paths(&mut self) -> Result<(), LxAppError> {
        // Get the app's version
        self.version = self.read_version();

        // Calculate the directory name based on appid, user and whether this is a home app
        let dir_name = if self.home_lxapp {
            // Home mini app uses appid directly as directory name
            self.appid.clone()
        } else {
            let user_id = LXAPPS_MANAGER.get().unwrap().get_user_id();
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
                LxAppError::IoError(format!("Failed to create lx apps directory: {}", e))
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
                LxAppError::IoError(format!("Failed to create user data directory: {}", e))
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
                LxAppError::IoError(format!("Failed to create cache directory: {}", e))
            })?;
        }

        Ok(())
    }

    /// Load and parse lxapp.json configuration
    pub fn load_config(&mut self) -> Result<(), LxAppError> {
        let lxapp_json_path = self.lxapp_dir.join("lxapp.json");
        info!(
            " [{}] Loading lxapp.json from: {}",
            self.appid,
            lxapp_json_path.display()
        );

        // Load app configuration if it exists
        self.read_json("lxapp.json")
            .map(|app_json| {
                self.config = LxAppConfig::from_value(app_json)
                    .map_err(|e| LxAppError::InvalidJsonFile(format!("lxapp.json: {}", e)))?;

                // Initialize TabBar state if config has TabBar
                if let Some(tabbar_config) = self.config.get_tab_bar(self) {
                    let mut state = self.state.lock().unwrap();
                    state.tabbar = Some(tabbar_config.clone());
                    // Ensure page stacks match TabBar configuration
                    state.pages.ensure_stacks_for_tabbar(Some(&tabbar_config));
                }

                Ok(())
            })
            .inspect_err(|_| {
                let mut state = self.state.lock().unwrap();
                state.debug = true;
            })?
    }

    /// Initialize paths and load configuration
    fn setup(&mut self) -> Result<(), LxAppError> {
        self.initialize_paths()?;
        self.load_config()?;
        Ok(())
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
    fn read_bytes(&self, relative_path: &str) -> Result<Vec<u8>, LxAppError> {
        let file_path = self.lxapp_dir.join(relative_path);

        // Try to read from the filesystem
        fs::read(file_path)
            .map_err(|e| LxAppError::ResourceNotFound(format!("{}:{}", relative_path, e)))
    }

    /// Reads text content from the specified relative path
    fn read_text(&self, relative_path: &str) -> Result<String, LxAppError> {
        self.read_bytes(relative_path)
            .map(|content| String::from_utf8_lossy(&content).to_string())
    }

    /// Reads and parses JSON content from the specified relative path
    fn read_json(&self, relative_path: &str) -> Result<serde_json::Value, LxAppError> {
        self.read_text(relative_path).and_then(|content| {
            serde_json::from_str(&content)
                .map_err(|_| LxAppError::InvalidJsonFile(relative_path.to_string()))
        })
    }

    pub fn is_debug_enabled(&self) -> bool {
        self.state.lock().unwrap().debug || self.config.is_debug_enabled()
    }

    pub fn is_opened(&self) -> bool {
        self.state.lock().unwrap().opened
    }

    /// Check if a domain is allowed for network access
    pub fn is_domain_allowed(&self, domain: &str) -> bool {
        self.state
            .lock()
            .unwrap()
            .network_security
            .is_domain_allowed(domain)
    }

    /// Get a page by path
    pub fn get_page(&self, path: &str) -> Option<Page> {
        let state = self.state.lock().unwrap();
        state.pages.get_page(path).cloned()
    }

    /// This method should only be called when page is in Created state
    pub(crate) fn setup_page(&self, page: &Page, path: &str) {
        let state = page.get_page_state();
        if state != PageState::Created {
            return;
        }

        // Load HTML - this might fail on HarmonyOS if WebView isn't ready yet
        let html_data = self.generate_page_html(path);
        match page.load_html(
            String::from_utf8_lossy(&html_data).to_string(),
            format!("lx://{}/{}", self.appid, path),
        ) {
            Ok(_) => {
                // HTML loaded successfully
                page.set_page_state(PageState::Loading);
            }
            Err(e) => {
                error!("Failed to load HTML: {}", e)
                    .with_appid(self.appid.clone())
                    .with_path(path.to_string());
            }
        }
    }

    pub fn navigator_to_lxapp(&self, to: LxAppNavigator) -> Result<(), LxAppError> {
        // ignore if appid is the same
        if self.appid == to.appid {
            return Ok(());
        }

        if let Some(manager) = LXAPPS_MANAGER.get() {
            let app = manager.get_or_init_lxapp(to.appid.clone());
            // Always call open_lxapp to allow SDK/UI layer to handle switching
            app.runtime.open_lxapp(to.appid, to.path)?;
        }
        Ok(())
    }

    /// Uninstalls the mini app by removing its version record and directories
    ///
    /// # Returns
    /// * `Ok(())` - If the mini app was uninstalled successfully
    /// * `Err(LxAppError)` - If there was an error during uninstallation
    pub fn uninstall(&self) -> Result<(), LxAppError> {
        // Don't allow uninstalling the home app
        if self.home_lxapp {
            return Err(LxAppError::UnsupportedOperation(
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

    /// Get app configuration
    pub fn get_config(&self) -> &config::LxAppConfig {
        &self.config
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
fn prepare_directory_structure<T: AppRuntime + ?Sized>(runtime: &T) -> Result<(), LxAppError> {
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

// Global instance of LxApps manager
static LXAPPS_MANAGER: OnceLock<Arc<LxApps>> = OnceLock::new();

/// Initialize the LxApps singleton
/// Returns an Option of home_app_id on success.
pub fn init<R: AppRuntime + 'static>(runtime: R) -> Option<String> {
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
            let home_lxapp_appid = config.home_lxapp_appid.clone();
            let home_lxapp_version = &config.home_lxapp_version;
            let max_apps = config.max_allowed_lxapps;

            if !install::is_installed(runtime_arc.as_ref(), &home_lxapp_appid) {
                if let Err(e) = install::install_home_lxapp(
                    runtime_arc.as_ref(),
                    &home_lxapp_appid,
                    home_lxapp_version,
                ) {
                    error!("Failed to install home LxApp: {}", e);
                    return None;
                }
            }

            let executor = crate::executor::LxAppExecutor::init(max_apps);

            // Create the home LxApp instance
            let mut home_lxapp = LxApp::new_as_home(
                home_lxapp_appid.clone(),
                runtime_arc.clone(),
                executor.clone(),
            );

            // Check if home mini app needs updating after loading its configuration
            if home_lxapp.is_debug_enabled() || home_lxapp.should_update(home_lxapp_version) {
                if let Err(e) = install::install_home_lxapp(
                    runtime_arc.as_ref(),
                    &home_lxapp_appid,
                    home_lxapp_version,
                ) {
                    error!("Failed to install home LxApp: {}", e);
                    return None;
                }
                if let Err(e) = home_lxapp.load_config() {
                    error!("Home LxApp failed to load config: {}", e);
                }
            }

            // Create LxApps manager
            let lxapps_manager = Arc::new(LxApps::new(runtime_arc.clone(), executor, max_apps));

            // Add home lxapp to the manager
            let home_lxapp_arc = Arc::new(home_lxapp);
            lxapps_manager
                .lxapps
                .insert(home_lxapp_appid.clone(), home_lxapp_arc.clone());

            // Set global instance
            if LXAPPS_MANAGER.set(lxapps_manager).is_err() {
                error!("LxApps manager singleton had been initialized by another instance");
                return None;
            }

            info!("LxApps initialized successfully");
            Some(home_lxapp_appid)
        }

        Err(e) => {
            // Provide more detailed error messages for different error types
            let error_message = match e {
                LxAppError::InvalidParameter(msg) => {
                    format!("Configuration validation failed: {}", msg)
                }
                LxAppError::InvalidJsonFile(msg) => {
                    format!("Invalid app.json file: {}", msg)
                }
                LxAppError::IoError(msg) => {
                    format!("I/O error while reading configuration: {}", msg)
                }
                _ => format!("Failed to load app configuration: {}", e),
            };

            error!("{}", error_message);
            None
        }
    }
}

/// Get a specific LxApp instance by appid
///
/// If the LxApp with the given appid exists, it returns a reference to it.
///
/// # Arguments
/// * `appid` - The ID of the mini app to get or create
///
/// # Returns
/// A thread-safe reference to the LxApp
///
/// # Panics
/// Panics if `LxApps` is not initialized or LxApp doesn't exist
pub fn get(appid: String) -> Arc<LxApp> {
    let manager = LXAPPS_MANAGER.get().expect("LxApps not initialized");
    if let Some(app_arc) = manager.lxapps.get(&appid) {
        return app_arc.clone();
    }
    panic!("Not found lxapp '{}'", appid);
}
