use dashmap::DashMap;
use lingxia_platform::{AppRuntime, Platform, PopupPresenter, PopupRequest};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tokio::time;

use self::navbar::NavigationBarState;
use crate::PageLifecycleEvent;
use crate::app::AppConfig;
use crate::cache::LxAppCache;
use crate::error::LxAppError;
use crate::executor::LxAppExecutor;
use crate::page::Page;
use crate::startup::LxAppStartupOptions;
use crate::update::UpdateManager;
use crate::{error, info, warn};
use security::NetworkSecurity;

pub mod config;
use config::LxAppConfig;
mod content;
pub(crate) mod metadata;
pub use metadata::ReleaseType;
pub mod navbar;
pub mod page_config;
mod scheme;
mod security;
pub mod tabbar;
pub(crate) mod uri;
pub(crate) mod version;
use crate::event::AppServiceEvent;
use lingxia_webview::{WebTag, destroy_webview};

/// Constants for lxapp storage layout
pub(crate) const LINGXIA_DIR: &str = "lingxia";
pub(crate) const LXAPPS_DIR: &str = "lxapps";
pub(crate) const PLUGINS_DIR: &str = "plugins";
pub(crate) const STORAGE_DIR: &str = "storage";
pub(crate) const USER_DATA_DIR: &str = "userdata";
pub(crate) const USER_CACHE_DIR: &str = "usercache";

const LXAPPS_DB_FILE: &str = "lxapps.redb";
const DEFAULT_VERSION: &str = "0.0.1";

const LXAPP_STACK_MAX: usize = 5;
const PAGE_STACK_MAX: usize = 10;

/// Manages a collection of lxapp applications
pub struct LxApps {
    /// Collection of lxapps, keyed by app ID
    /// Uses DashMap for thread-safe concurrent access
    lxapps: DashMap<String, Arc<LxApp>>,

    /// LxApp navigation stack for tracking app navigation history
    /// Uses VecDeque for efficient push/pop operations
    lxapp_stack: Mutex<VecDeque<String>>,

    /// Reference to the platform-specific app runtime
    /// Provides file system access, UI callbacks, etc.
    runtime: Arc<Platform>,

    /// Reference to the executor
    /// Handles async task execution for lxapp apps
    pub(crate) executor: Arc<LxAppExecutor>,

    /// Pending delayed-destroy timers keyed by appid
    pending_destroy: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

impl LxApps {
    fn new(runtime: Platform, executor: Arc<LxAppExecutor>) -> Self {
        let runtime = Arc::new(runtime);

        Self {
            lxapps: DashMap::new(),
            runtime,
            executor,
            lxapp_stack: Mutex::new(VecDeque::with_capacity(LXAPP_STACK_MAX)),
            pending_destroy: Mutex::new(HashMap::new()),
        }
    }

    /// Get or initialize a specific LxApp instance by appid
    fn get_or_init_lxapp(&self, appid: String, release_type: ReleaseType) -> Arc<LxApp> {
        let has_pending_update = metadata::downloaded_get(&appid, release_type)
            .map(|opt| opt.is_some())
            .unwrap_or(false);

        if has_pending_update {
            // Tear down any existing instance before applying new files
            self.destroy_lxapp(&appid);
            let _ =
                UpdateManager::apply_downloaded_update(self.runtime.clone(), &appid, release_type);
        } else if let Some(app_arc) = self.lxapps.get(&appid) {
            return app_arc.clone();
        }

        // Create new LxApp
        let new_lxapp = Arc::new(LxApp::new(
            appid.clone(),
            self.runtime.clone(),
            self.executor.clone(),
            release_type,
        ));

        // Insert into collection and return
        self.lxapps.insert(appid, new_lxapp.clone());
        new_lxapp
    }

    /// Completely destroy an LxApp (shutdown + removal from manager and stack).
    fn destroy_lxapp(&self, appid: &str) {
        if let Some(app_arc) = self.lxapps.get(appid) {
            let _ = app_arc.shutdown();
        }
        self.remove_from_stack(appid);
        self.lxapps.remove(appid);
    }

    /// Recreate the LxApp instance for a given appid with a brand new instance.
    /// Used by restart to force a fresh session and runtime state.
    fn recreate_lxapp(&self, appid: String, release_type: ReleaseType) -> Arc<LxApp> {
        // Clean out old instance + stack entries before inserting a fresh one.
        self.destroy_lxapp(&appid);

        // Delegate to get_or_init_lxapp so pending downloaded updates are applied
        // consistently (same path as cold-start navigation).
        self.get_or_init_lxapp(appid, release_type)
    }

    /// Finds and evicts the least recently used LxApp to free up memory.
    /// The least recently used app is determined by the front of the navigation stack.
    fn evict_lru_lxapp(&self) {
        let appid_to_destroy = {
            if let Ok(stack) = self.lxapp_stack.lock() {
                stack.front().cloned()
            } else {
                None
            }
        };

        if let Some(appid_to_destroy) = appid_to_destroy {
            // Check if it's the home app
            if let Some(app_arc) = self.lxapps.get(&appid_to_destroy)
                && app_arc.is_home_lxapp
            {
                warn!("Cannot evict the home lxapp").with_appid(appid_to_destroy);
                return;
            }

            info!("Evicting least recently used lxapp").with_appid(appid_to_destroy.clone());

            // Explicitly shutdown the app before removing it from the map so that
            // UI/JSContext/Page/WebView/AppService are cleaned up deterministically.
            self.destroy_lxapp(&appid_to_destroy);
        }
    }

    /// Schedule a delayed destroy for an app; cancel on reopen.
    pub(crate) fn schedule_delayed_destroy(self: &Arc<Self>, appid: String) {
        // cancel existing timer if present
        if let Ok(mut map) = self.pending_destroy.lock()
            && let Some(cancel) = map.remove(&appid)
        {
            let _ = cancel.send(());

            let (tx, rx) = oneshot::channel();
            map.insert(appid.clone(), tx);

            let mgr_weak = Arc::downgrade(self);
            let task_appid = appid.clone();
            let _ = rong::service_executor::spawn_async(async move {
                let sleep = time::sleep(Duration::from_secs(1800));
                tokio::pin!(rx);
                tokio::pin!(sleep);
                tokio::select! {
                    _ = &mut sleep => {},
                    _ = &mut rx => return, // cancelled
                }

                if let Some(mgr) = mgr_weak.upgrade() {
                    info!("Delayed destroy triggered after inactivity")
                        .with_appid(task_appid.clone());
                    mgr.destroy_lxapp(&task_appid);
                    if let Ok(mut guard) = mgr.pending_destroy.lock() {
                        guard.remove(&task_appid);
                    }
                }
            });
        }
    }

    /// Cancel any pending delayed destroy for the given app.
    pub(crate) fn cancel_delayed_destroy(&self, appid: &str) {
        if let Ok(mut map) = self.pending_destroy.lock()
            && let Some(cancel) = map.remove(appid)
        {
            let _ = cancel.send(());
        }
    }

    /// Pushes an app onto the back of the navigation stack.
    /// This signifies that it is the most recently used app.
    /// If the stack is already at full capacity, the operation is aborted and a warning is logged.
    pub(crate) fn push_lxapp_stack(&self, appid: String) {
        if let Ok(mut stack) = self.lxapp_stack.lock() {
            if stack.len() < LXAPP_STACK_MAX {
                stack.push_back(appid);
            } else {
                warn!(
                    "LxApp navigation stack is full (capacity: {}). Cannot push app: {}",
                    LXAPP_STACK_MAX, appid
                );
            }
        }
    }

    /// Peek at the top app on the navigation stack without removing it
    fn peek_lxapp_stack(&self) -> Option<String> {
        if let Ok(stack) = self.lxapp_stack.lock() {
            stack.back().cloned()
        } else {
            None
        }
    }

    /// Remove a specific app from the navigation stack
    pub(crate) fn remove_from_stack(&self, appid: &str) {
        if let Ok(mut stack) = self.lxapp_stack.lock() {
            stack.retain(|id| id != appid);
        }
    }

    /// Check if the navigation stack is full
    fn is_lxapp_stack_full(&self) -> bool {
        if let Ok(stack) = self.lxapp_stack.lock() {
            stack.len() >= LXAPP_STACK_MAX
        } else {
            // If the lock is poisoned, it's safer to consider it full
            // to prevent further pushes.
            true
        }
    }
}

/// Mutable state of a LxApp that requires synchronization
pub(crate) struct LxAppState {
    /// Collection of pages in this app with their current states
    /// Manages page lifecycle (show/hide/destroy)
    pub(crate) pages: Mutex<HashMap<String, Page>>,

    /// Page navigation stack for tracking page navigation history within this app
    /// Stores all pages for navigation history
    pub(crate) page_stack: Mutex<VecDeque<String>>,

    /// Time when this app was last active
    /// Used for LRU (Least Recently Used) eviction when memory is low
    pub(crate) last_active_time: Instant,

    /// Debug mode override (can be enabled at runtime)
    /// When true, enables additional logging and debugging features
    debug: bool,

    /// Network security configuration for HTTPS domain filtering
    /// Manages which domains this app is allowed to access
    network_security: NetworkSecurity,

    /// TabBar runtime state
    /// Contains TabBar configuration and dynamic state (badges, red dots, visibility)
    pub tabbar: Option<tabbar::TabBar>,

    /// Startup options for the app
    pub(crate) startup_options: LxAppStartupOptions,

    /// Currently displayed popup page (if any)
    pub(crate) current_popup: Option<String>,
}

impl LxAppState {
    fn new() -> Self {
        Self {
            pages: Mutex::new(HashMap::new()),
            page_stack: Mutex::new(VecDeque::with_capacity(PAGE_STACK_MAX)),
            last_active_time: Instant::now(),
            debug: false,
            network_security: NetworkSecurity::new(),
            tabbar: None,
            startup_options: LxAppStartupOptions::default(),
            current_popup: None,
        }
    }
}

/// Represents a single lxapplication
pub struct LxApp {
    // Immutable data - initialized once and never changed
    pub appid: String,
    pub runtime: Arc<Platform>,
    pub lxapp_dir: PathBuf,
    pub storage_file_path: PathBuf,
    pub user_data_dir: PathBuf,
    pub user_cache_dir: PathBuf,
    pub fingermark: String,
    pub is_home_lxapp: bool,
    pub(crate) release_type: ReleaseType,
    pub(crate) config: LxAppConfig,
    pub(crate) executor: Arc<LxAppExecutor>,

    /// Current runtime session of this app (id + status)
    pub(crate) session: LxAppSession,

    // Mutable state - protected by mutex for fine-grained locking
    pub(crate) state: Mutex<LxAppState>,
    // Per-app cache for network and media artifacts
    cache: Option<LxAppCache>,
}

/// Unique id for a single LxApp runtime session within the process.
pub(crate) type LxAppSessionId = u64;

/// Lifecycle status of a LxApp session (replacing LxAppStatus).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum LxAppSessionStatus {
    Closed = 0,
    Opening = 1,
    Opened = 2,
    Closing = 3,
    Restarting = 4,
}

/// A single runtime session of a LxApp: id + status.
pub(crate) struct LxAppSession {
    pub(crate) id: LxAppSessionId,
    status: AtomicU8,
}

impl LxAppSession {
    pub(crate) fn new() -> Self {
        // Process-wide monotonically increasing session id.
        use std::sync::atomic::AtomicU64;
        static SESSION_SEQ: AtomicU64 = AtomicU64::new(1);
        let id = SESSION_SEQ.fetch_add(1, Ordering::Relaxed);
        Self {
            id,
            status: AtomicU8::new(LxAppSessionStatus::Closed as u8),
        }
    }

    pub(crate) fn status(&self) -> LxAppSessionStatus {
        match self.status.load(Ordering::SeqCst) {
            1 => LxAppSessionStatus::Opening,
            2 => LxAppSessionStatus::Opened,
            3 => LxAppSessionStatus::Closing,
            4 => LxAppSessionStatus::Restarting,
            _ => LxAppSessionStatus::Closed,
        }
    }

    pub(crate) fn set_status(&self, s: LxAppSessionStatus) {
        self.status.store(s as u8, Ordering::SeqCst);
    }

    pub(crate) fn cas_status(&self, from: LxAppSessionStatus, to: LxAppSessionStatus) -> bool {
        let current = self.status.load(Ordering::SeqCst);
        if current == from as u8 {
            self.status.store(to as u8, Ordering::SeqCst);
            true
        } else {
            false
        }
    }
}

/// Service identity of a LxApp, used for registry and comparisons.
impl LxApp {
    /// Helper to clone Arc<Self> from within methods needing Arc
    pub(crate) fn clone_arc(&self) -> Arc<LxApp> {
        // All LxApp instances are stored as Arc in the global manager; retrieve by appid
        crate::lxapp::get(self.appid.clone())
    }

    pub(crate) fn status(&self) -> LxAppSessionStatus {
        self.session.status()
    }

    pub(crate) fn set_status(&self, s: LxAppSessionStatus) {
        self.session.set_status(s);
    }

    pub(crate) fn cas_status(&self, from: LxAppSessionStatus, to: LxAppSessionStatus) -> bool {
        self.session.cas_status(from, to)
    }
    // AppService state subscriptions removed for simplicity; rely on FIFO ordering.
    /// Shutdown this LxApp completely. Idempotent.
    ///
    /// Order:
    /// 1) Mark Closing to suppress page terminations
    /// 2) Close UI window
    /// 3) Break Page↔WebView delegate links and clear pages
    /// 4) Destroy platform WebViews
    /// 5) Clear page stack and popup
    /// 6) Send TerminateAppSvc (receiver handles teardown)
    pub fn shutdown(&self) -> Result<(), LxAppError> {
        // Mark closing to suppress TerminatePage from Page drops
        self.set_status(LxAppSessionStatus::Closing);

        // Close UI window
        let _ = self
            .runtime
            .hide_lxapp(self.appid.clone())
            .map_err(LxAppError::from);

        // Collect current pages
        let page_paths: Vec<String> = {
            let state = self.state.lock().unwrap();
            state.pages.lock().unwrap().keys().cloned().collect()
        };

        // Break Page <-> WebView links early and detach WebViews, then drop pages by clearing the map
        if let Ok(state) = self.state.lock() {
            for (_k, page) in state.pages.lock().unwrap().iter() {
                page.detach_webview();
            }
        }
        if let Ok(state) = self.state.lock() {
            state.pages.lock().unwrap().clear();
        }
        for p in &page_paths {
            destroy_webview(&WebTag::new(&self.appid, p, Some(self.session.id)));
        }
        let _ = self.clear_page_stack();
        if let Ok(mut state) = self.state.lock() {
            state.current_popup = None;
        }

        // Terminate AppService (receiver handles its own state)
        let _ = self.executor.terminate_app_svc(self.clone_arc());
        Ok(())
    }

    fn _new(
        appid: String,
        runtime: Arc<Platform>,
        executor: Arc<LxAppExecutor>,
        release_type: ReleaseType,
    ) -> Self {
        let session = LxAppSession::new();
        Self {
            appid,
            runtime,
            lxapp_dir: PathBuf::new(),
            storage_file_path: PathBuf::new(),
            user_data_dir: PathBuf::new(),
            user_cache_dir: PathBuf::new(),
            fingermark: String::new(),
            is_home_lxapp: false,
            release_type,
            config: LxAppConfig::default(),
            executor,
            session,
            state: Mutex::new(LxAppState::new()),
            cache: None,
        }
    }

    /// Create a new regular mini-app (not home app)
    pub(crate) fn new(
        appid: String,
        runtime: Arc<Platform>,
        executor: Arc<LxAppExecutor>,
        release_type: ReleaseType,
    ) -> Self {
        let mut app = Self::_new(appid, runtime, executor, release_type);
        if let Err(e) = app.setup() {
            error!("Setup failed: {}", e).with_appid(&app.appid);
        }

        app
    }

    /// Create a new LxApp instance marked as the home lxapp
    fn new_as_home(appid: String, runtime: Arc<Platform>, executor: Arc<LxAppExecutor>) -> Self {
        let mut app = Self::_new(appid, runtime, executor, ReleaseType::Release);

        // Mark as home lxapp
        app.is_home_lxapp = true;

        if let Err(e) = app.setup() {
            error!("Setup failed for home app: {}", e).with_appid(&app.appid);
        }

        app
    }

    /// Initialize paths and directories for the lxapp
    fn initialize_paths(&mut self) -> Result<(), LxAppError> {
        // Load metadata if available to determine version and install path
        let meta = metadata::get(&self.appid, self.release_type).ok().flatten();
        self.fingermark = meta
            .as_ref()
            .map(|record| record.fingermark.clone())
            .unwrap_or_else(|| lxapp_fingermark(&self.appid, self.release_type));
        // Determine directory name from fingerprint
        let dir_name = self.fingermark.clone();

        // Set up app directory
        let base_dir = self
            .runtime
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(LXAPPS_DIR);

        self.lxapp_dir = base_dir.join(&dir_name);

        // Compute storage file path: <data>/lingxia/storage/<fingermark>.redb
        self.storage_file_path = self
            .runtime
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(STORAGE_DIR)
            .join(format!("{}.redb", self.fingermark));

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
                if let Some(tabbar_config) = &self.config.tabBar {
                    let mut state = self.state.lock().unwrap();
                    // Convert icon paths to absolute paths using the lxapp directory as base
                    state.tabbar = Some(tabbar_config.with_absolute_paths(&self.lxapp_dir));
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
        // Initialize per-app cache directly using the app's cache dir
        self.cache = Some(
            LxAppCache::new(self.user_cache_dir.clone())
                .map_err(|e| LxAppError::IoError(e.to_string()))?,
        );
        Ok(())
    }

    pub fn cache(&self) -> Result<&LxAppCache, LxAppError> {
        self.cache
            .as_ref()
            .ok_or_else(|| LxAppError::IoError("cache not initialized".to_string()))
    }

    /// Get the current installed version of this app variant from storage
    pub fn current_version(&self) -> String {
        metadata::get(&self.appid, self.release_type)
            .ok()
            .flatten()
            .map(|record| record.version_string())
            .filter(|version| !version.is_empty())
            .unwrap_or_else(|| DEFAULT_VERSION.to_string())
    }

    // Reads binary data from the specified relative path
    fn read_bytes(&self, relative_path: &str) -> Result<Vec<u8>, LxAppError> {
        let file_path = match crate::plugin::resolve_plugin_resource_path_from_internal_path(
            &self.runtime,
            &self.config.plugins,
            relative_path,
        )? {
            Some(path) => path,
            None => self.lxapp_dir.join(relative_path),
        };

        // Try to read from the filesystem
        fs::read(&file_path).map_err(|e| {
            LxAppError::ResourceNotFound(format!(
                "{}:{} (resolved: {})",
                relative_path,
                e,
                file_path.display()
            ))
        })
    }

    /// Resolve an "allowed" lxapp path (package dir, user data, user cache) to a canonical path.
    ///
    /// Order:
    /// 1) Absolute path: validate it lies under a trusted root (package, user data, user cache,
    ///    plus their parents for full-path scenarios); return canonical path or error
    /// 2) Relative path: check under user data, then user cache, then package dir; return match or error
    ///
    /// Note: paths containing `.` or `..` segments are rejected.
    pub fn resolve_accessible_path(&self, path: &str) -> Result<PathBuf, LxAppError> {
        if path.trim().is_empty() {
            return Err(LxAppError::ResourceNotFound("empty path".to_string()));
        }
        if path.split('/').any(|s| s == "." || s == "..") {
            return Err(LxAppError::ResourceNotFound(
                "dot segment not allowed".to_string(),
            ));
        }

        let path_ref = Path::new(path);
        let _bundle_root = self
            .lxapp_dir
            .canonicalize()
            .unwrap_or_else(|_| self.lxapp_dir.clone());

        // Relative path: search in order user data -> user cache -> package
        if !path_ref.is_absolute() && !path.contains(':') {
            let rel = path.trim_matches('/');
            let search_roots: [&Path; 3] =
                [&self.user_data_dir, &self.user_cache_dir, &self.lxapp_dir];
            for root in search_roots
                .into_iter()
                .filter(|dir| !dir.as_os_str().is_empty())
            {
                let candidate = root.join(rel);
                let root_canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
                if let Ok(canonical) = candidate.canonicalize()
                    && canonical.starts_with(&root_canon)
                {
                    return Ok(canonical);
                }
            }
            return Err(LxAppError::ResourceNotFound(path.to_string()));
        }

        let candidate = if path_ref.is_absolute() || path.contains(':') {
            PathBuf::from(path)
        } else {
            Path::new("/").join(path_ref)
        };

        let canonical = candidate
            .canonicalize()
            .map_err(|_| LxAppError::ResourceNotFound(path.to_string()))?;

        let mut trusted_roots: Vec<PathBuf> = Vec::new();
        for dir in [&self.lxapp_dir, &self.user_cache_dir, &self.user_data_dir] {
            if !dir.as_os_str().is_empty()
                && let Ok(c) = dir.canonicalize()
            {
                trusted_roots.push(c);
            }
        }
        // Also allow parents of user data/cache roots to support full-path scenarios
        for dir in [&self.user_cache_dir, &self.user_data_dir] {
            if let Some(parent) = dir.parent()
                && let Ok(c) = parent.canonicalize()
            {
                trusted_roots.push(c);
            }
        }
        // Note: storage path is intentionally not added to allowed static roots.

        if trusted_roots.iter().any(|root| canonical.starts_with(root)) {
            Ok(canonical)
        } else {
            Err(LxAppError::ResourceNotFound(path.to_string()))
        }
    }

    /// Reads text content from the specified relative path
    fn read_text(&self, relative_path: &str) -> Result<String, LxAppError> {
        self.read_bytes(relative_path)
            .map(|content| String::from_utf8_lossy(&content).to_string())
    }

    /// Reads and parses JSON content from the specified relative path
    pub(crate) fn read_json(&self, relative_path: &str) -> Result<serde_json::Value, LxAppError> {
        self.read_text(relative_path).and_then(|content| {
            serde_json::from_str(&content)
                .map_err(|_| LxAppError::InvalidJsonFile(relative_path.to_string()))
        })
    }

    pub fn is_debug_enabled(&self) -> bool {
        self.state.lock().unwrap().debug || self.config.is_debug_enabled()
    }

    pub fn is_opened(&self) -> bool {
        matches!(self.status(), LxAppSessionStatus::Opened)
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
        self.state
            .lock()
            .unwrap()
            .pages
            .lock()
            .unwrap()
            .get(path)
            .cloned()
    }

    /// Check if pull-to-refresh is enabled for a specific page
    pub fn is_pull_down_refresh_enabled(&self, path: &str) -> bool {
        self.get_page(path)
            .map(|page| page.is_pull_down_refresh_enabled())
            .unwrap_or(false)
    }

    /// Get navigation bar state for a page; returns default if page not found.
    pub fn get_navbar_state(&self, path: &str) -> NavigationBarState {
        self.get_page(path)
            .and_then(|page| page.get_navbar_state())
            .unwrap_or_default()
    }

    /// Open a new runtime session for this LxApp instance:
    /// - Apply any downloaded update for this release_type (if present)
    /// - Record startup options
    /// - Ensure AppService exists
    /// - Create native Page/WebView
    /// - Open the UI window
    fn open(&self, options: LxAppStartupOptions) -> Result<(), LxAppError> {
        let mut startup_options = options;

        // Record startup options on this instance
        // Resolve path early so we can keep native/view/AppService consistent.
        let raw_url = if startup_options.path.is_empty() {
            self.config.get_initial_route()
        } else {
            startup_options.path.clone()
        };

        let resolved = crate::route::resolve_route(self, &raw_url).unwrap_or_else(|e| {
            error!("Failed to resolve startup url '{}': {}", raw_url, e)
                .with_appid(self.appid.clone());
            crate::route::ResolvedRoute {
                original: raw_url.clone(),
                query: None,
                target: crate::route::RouteTarget::Normal {
                    path: raw_url.clone(),
                },
            }
        });

        startup_options.path = resolved.internal_path();
        if startup_options.query.is_empty()
            && let Some(query) = resolved.query.clone()
        {
            startup_options.query = query;
        }

        self.state.lock().unwrap().startup_options = startup_options.clone();

        // Ensure the target app's JS worker is created and mapped before creating pages
        if let Err(e) = self.executor.create_app_svc(self.clone_arc()) {
            error!("Failed to trigger app service: {}", e).with_appid(self.appid.clone());
        }

        // Create native Page + WebView
        let page = self.get_or_create_page(&startup_options.path);
        page.set_query(startup_options.query.clone());

        // Open UI
        self.runtime
            .show_lxapp(self.appid.clone(), startup_options.path)?;
        Ok(())
    }

    /// Navigates to another LxApp (forward navigation).
    ///
    /// If the provided path is empty, it will navigate to the target app's initial route.
    /// If the navigation stack is already full, this operation will be ignored.
    ///
    /// This is a forward navigation that will push the target app onto the navigation stack.
    /// The initial state of the target app is controlled by the `options` parameter.
    /// If the app navigation stack is full, this operation will be ignored.
    ///
    /// # Arguments
    ///
    /// * `appid` - The ID of the target `LxApp` to navigate to.
    /// * `options` - The startup options for the target app.
    pub fn navigate_to(
        &self,
        appid: String,
        options: LxAppStartupOptions,
    ) -> Result<(), LxAppError> {
        if let Some(manager) = get_lxapps_manager() {
            // Cancel any pending destroy for the target app since it is about to be opened.
            manager.cancel_delayed_destroy(&appid);

            if manager.is_lxapp_stack_full() {
                warn!(
                    "LxApp navigation stack is full (capacity: {}). Cannot navigate to app: {}",
                    LXAPP_STACK_MAX, appid
                );
                return Ok(());
            }

            let app = manager.get_or_init_lxapp(appid.clone(), options.release_type);
            app.open(options)?;
        }
        Ok(())
    }

    /// Navigates back to the previous LxApp in the history stack.
    pub fn navigate_back(&self) -> Result<(), LxAppError> {
        // The on_lxapp_closed delegate will then handle removing it from the navigation stack.
        // The underlying UI framework should detect the app closure and automatically display the new app at the top of the stack.
        self.runtime.hide_lxapp(self.appid.clone())?;
        Ok(())
    }

    /// Restarts the current LxApp with cleanup + reopen.
    /// This offloads the sequence to the service executor to avoid blocking JS worker.
    pub fn restart(&self) -> Result<(), LxAppError> {
        // Prevent overlapping restarts
        if !self.cas_status(LxAppSessionStatus::Opened, LxAppSessionStatus::Restarting) {
            return Ok(());
        }
        // Always relaunch to initial route after restart
        let relaunch_path = self.config.get_initial_route();
        let appid = self.appid.clone();
        let release_type = self.release_type;
        let _ = rong::service_executor::spawn_async(async move {
            // 1) Replace LxApp instance in manager with a brand new one for this appid.
            if let Some(manager) = get_lxapps_manager() {
                let new_app = manager.recreate_lxapp(appid.clone(), release_type);

                // 2) Initialize startup options for the new app session and open it.
                let options =
                    LxAppStartupOptions::new(&relaunch_path).set_release_type(release_type);
                if let Err(e) = new_app.open(options) {
                    error!("Failed to start lxapp after restart: {}", e);
                }
            }
            // Status will be driven back to Opened by on_lxapp_opened delegate after reopen.
        });
        Ok(())
    }

    /// Show popup content rendered via WebView.
    ///
    /// This will ensure the target page is created, query parameters applied, lifecycle
    /// callbacks dispatched, and then delegate to the platform popup presenter.
    pub fn show_popup(self: &Arc<Self>, mut request: PopupRequest) -> Result<(), LxAppError> {
        // Ensure only one popup is active at a time.
        self.hide_popup()?;

        request.app_id = self.appid.clone();

        let resolved = crate::route::resolve_route(self, &request.path)?;
        let path = resolved.internal_path();
        let query_str = resolved.query.unwrap_or_default();

        let popup_page = self.get_or_create_page(&path);

        popup_page.mark_active();

        if !query_str.is_empty() {
            popup_page.set_query(query_str);
        }

        if !request.width_ratio.is_nan() {
            request.width_ratio = request.width_ratio.clamp(0.0, 1.0);
        }
        if !request.height_ratio.is_nan() {
            request.height_ratio = request.height_ratio.clamp(0.0, 1.0);
        }

        popup_page.dispatch_lifecycle_event(PageLifecycleEvent::OnLoad);

        request.path = path.clone();

        self.runtime.show_popup(request).map_err(LxAppError::from)?;

        if let Ok(mut state) = self.state.lock() {
            state.current_popup = Some(path);
        }

        Ok(())
    }

    /// Hide the currently displayed popup, if any.
    pub fn hide_popup(self: &Arc<Self>) -> Result<(), LxAppError> {
        let popup_path = {
            let mut state = self.state.lock().unwrap();
            state.current_popup.take()
        };

        if let Some(path) = popup_path {
            if let Some(page) = self.get_page(&path) {
                page.dispatch_lifecycle_event(PageLifecycleEvent::OnHide);
                page.dispatch_lifecycle_event(PageLifecycleEvent::OnUnload);
            }

            self.runtime
                .hide_popup(&self.appid)
                .map_err(LxAppError::from)?;
        }

        Ok(())
    }

    pub fn get_lxapp_info(&self) -> config::LxAppInfo {
        self.config.get_lxapp_info()
    }

    /// Get existing page or create a new one.
    /// PageSvc creation + HTML load are handled inside Page::new once WebView is ready.
    pub fn get_or_create_page(&self, url: &str) -> Page {
        let resolved = crate::route::resolve_route(self, url).unwrap_or_else(|e| {
            error!("Failed to resolve page url '{}': {}", url, e).with_appid(self.appid.clone());
            let (path, query) = crate::startup::split_path_query(url);
            crate::route::ResolvedRoute {
                original: url.to_string(),
                query,
                target: crate::route::RouteTarget::Normal { path },
            }
        });

        let path = resolved.internal_path();
        let query = resolved.query;

        {
            let state = self.state.lock().unwrap();
            if let Some(page) = state.pages.lock().unwrap().get(&path) {
                if let Some(query) = query.clone() {
                    page.set_query(query);
                }
                return page.clone();
            }
        }

        let appid = self.appid.clone();

        // Gate HTML load when create_page_svc is false; caller will unblock after creating PageSvc.
        let lxapp_arc = self.clone_arc();
        let page = Page::new(appid.clone(), path.to_string(), self, move |page| {
            let lxapp_arc = lxapp_arc.clone();
            let page_clone = page.clone();
            async move {
                // Ensure PageSvc exists before loading HTML (for both regular and plugin pages)
                let (ack_tx, ack_rx) = oneshot::channel();
                if let Err(e) = lxapp_arc.executor.create_page_svc_with_ack(
                    lxapp_arc.clone(),
                    page_clone.path(),
                    ack_tx,
                ) {
                    return Err(e.to_string());
                }

                ack_rx
                    .await
                    .map_err(|e| format!("Page service creation ack failed: {}", e))?;

                page_clone
                    .load_html()
                    .map_err(|e| format!("Failed to load HTML for page: {}", e))
            }
        });

        // Insert the new page first to ensure it's protected
        {
            let state = self.state.lock().unwrap();
            state
                .pages
                .lock()
                .unwrap()
                .insert(path.clone(), page.clone());
        }

        self.evict_inactive_pages_if_needed();

        if let Some(query) = query {
            page.set_query(query);
        }

        page
    }

    /// Check if we need to evict pages before creating new ones
    /// Evict when page count exceeds: tabbar_items + PAGE_STACK_MAX
    fn should_evict_pages(&self) -> bool {
        let state = self.state.lock().unwrap();
        let page_count = state.pages.lock().unwrap().len();

        let max_allowed = if let Some(ref tabbar) = state.tabbar {
            tabbar.list.len() + PAGE_STACK_MAX
        } else {
            PAGE_STACK_MAX
        };

        page_count > max_allowed
    }

    /// Evict least recently used pages when memory is full
    fn evict_inactive_pages_if_needed(&self) {
        if !self.should_evict_pages() {
            return;
        }

        let state = self.state.lock().unwrap();
        let mut pages = state.pages.lock().unwrap();

        // Find the least recently used page (excluding current page in stack)
        let current_page = state.page_stack.lock().unwrap().back().cloned();

        let mut oldest_time: Option<Instant> = None;
        let mut oldest_path: Option<String> = None;

        for (path, page) in pages.iter() {
            if Some(path) == current_page.as_ref() {
                continue; // Don't evict current page
            }

            // Don't evict tabbar pages
            if page.is_tabbar_page() {
                info!("Skipping tabbar page for eviction: {}", path).with_appid(self.appid.clone());
                continue;
            }

            if let Some(last_active) = page.get_last_active_time()
                && oldest_time.is_none_or(|old| last_active < old)
            {
                oldest_time = Some(last_active);
                oldest_path = Some(path.clone());
            }
        }

        // Remove the oldest page
        if let Some(path) = oldest_path.clone() {
            // First, ask AppService to remove the PageSvc for this path (object-identity safe)
            let _ = self
                .executor
                .terminate_page_svc(self.clone_arc(), path.clone())
                .map_err(|e| {
                    warn!("Failed to request page termination: {}", e)
                        .with_appid(self.appid.clone())
                        .with_path(path.clone())
                });

            // Then remove from native registry
            if let Some(_removed_page) = pages.remove(&path) {
                info!("Evicted inactive page: {}", path).with_appid(self.appid.clone());
            } else {
                warn!("Failed to evict page (not found): {}", path).with_appid(self.appid.clone());
            }
        }
    }

    /// Check if the page stack is considered full
    /// Returns true when stack size reaches PAGE_STACK_MAX
    pub(crate) fn is_page_stack_full(&self) -> bool {
        self.get_page_stack_size() >= PAGE_STACK_MAX
    }

    /// Clear the page navigation stack
    /// This removes all pages from the navigation history
    pub(crate) fn clear_page_stack(&self) -> Result<(), LxAppError> {
        let state = self.state.lock().unwrap();
        state.page_stack.lock().unwrap().clear();
        Ok(())
    }

    /// Add a page to the navigation stack.
    pub(crate) fn push_to_page_stack(&self, path: &str) -> Result<(), LxAppError> {
        let state = self.state.lock().unwrap();
        let mut stack = state.page_stack.lock().unwrap();

        // If stack is full, do nothing
        if stack.len() >= PAGE_STACK_MAX {
            return Ok(());
        }

        // Add to the back of the stack (most recent)
        stack.push_back(path.to_string());

        Ok(())
    }

    /// Remove the most recent page from the navigation stack
    /// Returns the path of the removed page, or None if stack is empty
    pub(crate) fn pop_from_page_stack(&self) -> Option<String> {
        let state = self.state.lock().unwrap();
        state.page_stack.lock().unwrap().pop_back()
    }

    /// Get the current page stack size
    pub(crate) fn get_page_stack_size(&self) -> usize {
        self.state.lock().unwrap().page_stack.lock().unwrap().len()
    }

    /// Get a copy of the current page stack
    /// Returns a vector of page paths in stack order (oldest to newest)
    pub fn get_page_stack(&self) -> Vec<String> {
        self.state
            .lock()
            .unwrap()
            .page_stack
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect()
    }

    /// Peek at the current page path without removing it from the stack
    /// Returns None if the stack is empty
    pub fn peek_current_page(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap()
            .page_stack
            .lock()
            .unwrap()
            .back()
            .cloned()
    }
}

/// Compute a stable hash id for lxapp-scoped data separation.
/// Includes lxappid + release_type + device_fingerprint to ensure isolation across variants and devices.
pub(crate) fn lxapp_fingermark(lxappid: &str, release_type: ReleaseType) -> String {
    // Fingermark uses appid + release_type + device fingerprint (version excluded)
    let device_fp = crate::provider::get_provider()
        .get_fingerprint()
        .unwrap_or_default();
    info!("Device Fingerprint: {}", device_fp);
    let combined = format!("{}|{}|{}", lxappid, release_type.as_str(), device_fp);
    let mut hasher = DefaultHasher::new();
    combined.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

impl LxApp {
    /// Notify the AppService (logic.js layer) with a built-in event and optional JSON payload.
    pub fn appservice_notify(
        &self,
        event: AppServiceEvent,
        payload_json: Option<String>,
    ) -> Result<(), LxAppError> {
        self.executor
            .call_app_service_event(self.clone_arc(), event, payload_json)
    }
}

impl Drop for LxApp {
    fn drop(&mut self) {
        // Don't destroy home app
        if self.is_home_lxapp {
            return;
        }
        // At this point all strong Arc references have been released. Explicit shutdown
        // should have been invoked via restart, navigate_back, or LRU eviction paths.
        // Avoid calling shutdown() here to prevent accidentally targeting a newer
        // instance with the same appid after restart.
        info!("Dropping LxApp").with_appid(self.appid.clone());
    }
}

/// Prepares the base directory structure for lxapps
fn prepare_directory_structure(runtime: Arc<Platform>) -> Result<(), LxAppError> {
    let data_dir = runtime.app_data_dir();
    let cache_dir = runtime.app_cache_dir();

    // Create required directories
    let dirs = [
        data_dir.join(LINGXIA_DIR).join(LXAPPS_DIR),
        data_dir.join(LINGXIA_DIR).join(PLUGINS_DIR),
        data_dir.join(LINGXIA_DIR).join(USER_DATA_DIR),
        data_dir.join(LINGXIA_DIR).join(STORAGE_DIR),
        cache_dir.join(LINGXIA_DIR).join(LXAPPS_DIR),
    ];

    for dir in &dirs {
        fs::create_dir_all(dir)?;
    }

    let metadata_path = data_dir.join(LINGXIA_DIR).join(LXAPPS_DB_FILE);
    metadata::init(metadata_path)
}

// Global instance of LxApps manager
static LXAPPS_MANAGER: OnceLock<Arc<LxApps>> = OnceLock::new();

/// Initialize the LxApps singleton
/// Returns an Option of home_app_id on success.
pub fn init(runtime: Platform) -> Option<String> {
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

    // Initialize WebView manager
    lingxia_webview::init_webview_manager();

    let runtime_arc = Arc::new(runtime.clone());

    // Prepare directory structure
    if let Err(e) = prepare_directory_structure(runtime_arc.clone()) {
        error!("Failed to prepare directory structure: {}", e);
        return None;
    }

    match AppConfig::load(runtime_arc.clone()) {
        Ok(config) => {
            let home_lxapp_appid = config.home_lxapp_appid.clone();
            let home_lxapp_version = &config.home_lxapp_version;

            // Check if installation is needed before creating LxApp
            // This ensures lxapp.json is only loaded once
            let needs_install = !metadata::exists(&home_lxapp_appid, ReleaseType::Release)
                .unwrap_or(false)
                || metadata::get(&home_lxapp_appid, ReleaseType::Release)
                    .ok()
                    .flatten()
                    .map(|rec| rec.version_string())
                    .as_deref()
                    != Some(home_lxapp_version);

            if needs_install {
                if let Err(e) = crate::update::UpdateManager::install_from_assets(
                    runtime_arc.clone(),
                    &home_lxapp_appid,
                    home_lxapp_version,
                ) {
                    error!("Failed to install home LxApp: {}", e);
                    return None;
                }
            }

            let executor = LxAppExecutor::init(LXAPP_STACK_MAX);

            // Create LxApps manager BEFORE creating home_lxapp
            // This makes get_platform() available as early as possible
            let lxapps_manager = Arc::new(LxApps::new(runtime, executor.clone()));

            // Set global instance early so get_platform() works
            if LXAPPS_MANAGER.set(lxapps_manager.clone()).is_err() {
                error!("LxApps manager singleton had been initialized by another instance");
                return None;
            }

            // Create the home LxApp instance (loads lxapp.json once)
            let mut home_lxapp = LxApp::new_as_home(
                home_lxapp_appid.clone(),
                runtime_arc.clone(),
                executor.clone(),
            );

            let initial_route = home_lxapp.config.get_initial_route();
            home_lxapp.state.lock().unwrap().startup_options.path = initial_route;

            // In debug mode, always reinstall from assets for hot-reload during development
            if home_lxapp.is_debug_enabled() {
                if let Err(e) = crate::update::UpdateManager::install_from_assets(
                    runtime_arc.clone(),
                    &home_lxapp_appid,
                    home_lxapp_version,
                ) {
                    error!("Failed to reinstall home LxApp in debug mode: {}", e);
                }
                // Reload config after reinstall
                let _ = home_lxapp.load_config();
            }

            // Add home lxapp to the manager
            let home_lxapp_arc = Arc::new(home_lxapp);
            lxapps_manager
                .lxapps
                .insert(home_lxapp_appid.clone(), home_lxapp_arc.clone());

            // Pre-create JS worker for home lxapp
            if let Err(e) = home_lxapp_arc
                .executor
                .create_app_svc(home_lxapp_arc.clone())
            {
                error!("Failed to trigger home app service: {}", e)
                    .with_appid(home_lxapp_appid.clone());
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

/// Get access to the LxApps manager for navigation stack operations
pub(crate) fn get_lxapps_manager() -> Option<Arc<LxApps>> {
    LXAPPS_MANAGER.get().cloned()
}

/// Get the platform runtime instance.
/// Returns None if the SDK has not been initialized.
pub fn get_platform() -> Option<Arc<Platform>> {
    LXAPPS_MANAGER.get().map(|manager| manager.runtime.clone())
}

/// Get the system locale string.
/// Returns "en-US" as default if the SDK has not been initialized.
pub fn get_locale() -> String {
    LXAPPS_MANAGER
        .get()
        .map(|manager| manager.runtime.get_system_locale().to_string())
        .unwrap_or_else(|| "en-US".to_string())
}

/// Try to get a specific LxApp instance by lxappid
pub fn try_get(appid: &str) -> Option<Arc<LxApp>> {
    LXAPPS_MANAGER
        .get()
        .and_then(|manager| manager.lxapps.get(appid).map(|lxapp| lxapp.clone()))
}

/// Internal helper: get LxApp by appid, panics if not found.
/// Only for use within lingxia-lxapp where LxApp is known to exist.
pub(crate) fn get(appid: String) -> Arc<LxApp> {
    try_get(&appid).expect("LxApp not found")
}

/// Triggers memory cleanup for LxApps.
/// This function should be called by the platform when the system is under memory pressure.
pub fn on_low_memory() {
    if let Some(manager) = LXAPPS_MANAGER.get() {
        info!("on_low_memory triggered, evicting least recently used app.");
        manager.evict_lru_lxapp();
    }
}

/// Get the current lxapp from the navigation stack and its current page path
/// Returns (appid, current_page_path) or empty strings if not found
pub fn get_current_lxapp() -> (String, String) {
    if let Some(manager) = LXAPPS_MANAGER.get()
        && let Some(current_appid) = manager.peek_lxapp_stack()
        && let Some(lxapp) = manager.lxapps.get(&current_appid)
    {
        let current_path = lxapp.peek_current_page().unwrap_or_default();
        info!("Peek {}:{} from lxapp stack", current_appid, current_path);
        return (current_appid, current_path);
    }
    (String::new(), String::new())
}

/// Check if pull-to-refresh is enabled for a specific page
/// Returns false if the app or page is not found
pub fn is_pull_down_refresh_enabled(appid: &str, path: &str) -> bool {
    try_get(appid)
        .map(|lxapp| lxapp.is_pull_down_refresh_enabled(path))
        .unwrap_or(false)
}

/// Check whether a given appid is currently opened (in-memory and marked opened).
pub fn is_lxapp_open(lxappid: &str) -> bool {
    if let Some(manager) = LXAPPS_MANAGER.get()
        && let Some(app) = manager.lxapps.get(lxappid)
    {
        return app.is_opened();
    }
    false
}
