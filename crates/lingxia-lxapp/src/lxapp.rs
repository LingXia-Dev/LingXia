use dashmap::DashMap;
use http::Uri as HttpUri;
use lingxia_platform::Platform;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_platform::traits::ui::UIUpdate;
#[cfg(feature = "js-appservice")]
use rong::{JSContext, JSResult, Source, error::HostError};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tokio::time;
use uuid::Uuid;

use self::navbar::NavigationBarState;
use crate::appservice::LxAppWorkers;
use crate::error::LxAppError;
use crate::page::config::{OrientationConfig, PageConfig};
use crate::page::{PageInstance, PageInstanceId, ViewCallOptions, WebTagInstance};
use crate::startup::LxAppStartupOptions;
use crate::update::UpdateManager;
use crate::{error, info, warn};
use security::NetworkSecurity;

pub mod config;
use config::{LxAppConfig, LxAppLogicEntry, LxAppPageEntry};
mod content;
pub(crate) mod metadata;
pub mod navbar;
mod page_instance_host;
mod runtime_bootstrap;
mod runtime_ops;
mod runtime_registry;
mod scheme;
mod security;
mod surface;
pub use security::LxAppSecurityPrivilege;
pub mod tabbar;
pub mod uri;
pub(crate) mod version;
use crate::lifecycle::AppServiceEvent;
pub use crate::page::runtime::{
    CloseReason, CreatePageInstanceRequest, CreatedPageInstance, PageDefinition, PageInstanceEvent,
    PageOwner, PageQueryInput, PageTarget, PresentationKind, ResolvedPage, SceneId,
};
use crate::page::runtime::{
    PageInstanceLifecycleState, PageInstanceRuntimeRecord, transition_page_instance_lifecycle,
};
pub use lingxia_platform::traits::ui::{SurfaceKind, SurfacePosition};
pub use lingxia_surface::Role as SurfaceRole;
pub use lingxia_update::ReleaseType;
use lingxia_webview::WebTag;
use lingxia_webview::runtime::destroy_webview;
pub use runtime_bootstrap::init;
pub use runtime_ops::{
    close_lxapp, create_page_instance, dispose_page_instance, dispose_page_instance_by_id,
    ensure_builtin_lxapp, ensure_lxapp, get_current_lxapp, installed_lxapp_path, is_lxapp_open,
    is_pull_down_refresh_enabled, list_lxapps, mark_lxapp_active, notify_lxapp_host_visibility,
    notify_page_host_visibility, notify_page_instance, notify_page_instance_by_id, on_low_memory,
    open_lxapp, restart_lxapp, touch_page_instance_by_id, uninstall_lxapp,
};
pub use runtime_registry::{find_page_by_instance_id, get_locale, get_platform, try_get};
pub(crate) use runtime_registry::{get, get_lxapps_manager};
pub(crate) use surface::SurfaceRecords;
pub use surface::{
    PageSurface, PageSurfaceRequest, PageSurfaceTarget, register_surface_close_observer,
};
use version::Version;

/// Constants for lxapp storage layout
pub(crate) const LINGXIA_DIR: &str = "lingxia";
pub(crate) const LXAPPS_DIR: &str = "lxapps";
pub(crate) const PLUGINS_DIR: &str = "plugins";
pub(crate) const STORAGE_DIR: &str = "storage";
pub(crate) const USER_DATA_DIR: &str = "userdata";
pub(crate) const USER_CACHE_DIR: &str = "usercache";
pub(crate) const TEMP_DIR: &str = "temp";

const LXAPPS_DB_FILE: &str = "lxapps.redb";
const DEFAULT_VERSION: &str = "0.0.1";

const LXAPP_STACK_MAX: usize = 5;
const PAGE_STACK_MAX: usize = 10;

/// Configured worker/stack count override. Must be set before runtime initialization.
static NUM_WORKERS: OnceLock<usize> = OnceLock::new();
static LXAPP_SOURCE_OVERRIDES: OnceLock<Mutex<HashMap<String, LxAppBundleSource>>> =
    OnceLock::new();
static TRANSIENT_FILE_GRANTS: OnceLock<DashMap<(String, LxAppSessionId, String), PathBuf>> =
    OnceLock::new();
static TRANSIENT_FILE_REFERENCE_GRANTS: OnceLock<DashMap<(String, LxAppSessionId, String), ()>> =
    OnceLock::new();

#[derive(Debug, Clone, Copy)]
enum TransientPathKind {
    File,
    Directory,
}

fn normalize_transient_path(path: &Path, kind: TransientPathKind) -> Result<PathBuf, LxAppError> {
    let normalized = std::fs::canonicalize(path).map_err(|e| {
        LxAppError::ResourceNotFound(format!("transient path {}: {}", path.display(), e))
    })?;
    let metadata = std::fs::metadata(&normalized)?;
    let valid = match kind {
        TransientPathKind::File => metadata.is_file(),
        TransientPathKind::Directory => metadata.is_dir(),
    };
    if !valid {
        return Err(LxAppError::InvalidParameter(format!(
            "invalid transient path kind: {}",
            normalized.display()
        )));
    }
    Ok(normalized)
}

fn normalize_transient_file_reference(reference: &str) -> Result<String, LxAppError> {
    let normalized = reference.trim();
    let scheme = normalized
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase());
    if normalized.is_empty()
        || normalized.chars().any(char::is_control)
        || !matches!(scheme.as_deref(), Some("content" | "datashare" | "file"))
    {
        return Err(LxAppError::InvalidParameter(
            "invalid transient file reference".to_string(),
        ));
    }
    Ok(normalized.to_string())
}

/// Set the number of JS workers (and lxapp navigation stack capacity).
///
/// Must be called **before** [`init()`]. Defaults to [`LXAPP_STACK_MAX`] (5) if not set.
/// A value of 0 is clamped to 1.
pub fn set_num_workers(n: usize) {
    let n = n.max(1);
    if NUM_WORKERS.set(n).is_err() {
        warn!("set_num_workers: value already set, ignoring");
    }
}

/// Read the configured worker count, falling back to `LXAPP_STACK_MAX`.
fn get_num_workers() -> usize {
    NUM_WORKERS.get().copied().unwrap_or(LXAPP_STACK_MAX)
}

/// Register an lxapp whose pages/logic are bundled at `<appid>/...` inside the
/// platform asset root (Android `assets/`, iOS bundle, etc.). The on-disk asset
/// prefix is always the appid — no separate `asset_root` argument.
pub fn register_builtin_asset_bundle(appid: impl Into<String>) {
    register_lxapp_bundle_source(appid, LxAppBundleSource::BuiltinAssets);
}

/// Register a content-less builtin lxapp host. The LxApp is created with default
/// empty config (no pages/plugins/logic). A later [`register_builtin_asset_bundle`]
/// call for the same appid upgrades to a disk-backed bundle — used by shell-runtime
/// to swap in the real shell webui on macOS.
pub fn register_synthetic_lxapp(appid: impl Into<String>) {
    register_lxapp_bundle_source(appid, LxAppBundleSource::Synthetic);
}

pub fn register_dev_bundle_source(appid: impl Into<String>, root: impl Into<PathBuf>) {
    register_lxapp_bundle_source(appid, LxAppBundleSource::DevPath { root: root.into() });
}

fn register_lxapp_bundle_source(appid: impl Into<String>, source: LxAppBundleSource) {
    let appid = appid.into();
    let registry = LXAPP_SOURCE_OVERRIDES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = registry.lock().unwrap_or_else(|e| e.into_inner());
    guard.insert(appid, source);
}

fn lxapp_bundle_source_for(appid: &str) -> Option<LxAppBundleSource> {
    LXAPP_SOURCE_OVERRIDES
        .get()
        .and_then(|registry| registry.lock().ok())
        .and_then(|guard| guard.get(appid).cloned())
}

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
    pub(crate) executor: Arc<LxAppWorkers>,

    /// Pending delayed-destroy timers keyed by appid
    pending_destroy: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

impl LxApps {
    fn new(runtime: Platform, executor: Arc<LxAppWorkers>, capacity: usize) -> Self {
        info!("LxApps manager initialized with {} workers", capacity);
        let runtime = Arc::new(runtime);

        Self {
            lxapps: DashMap::new(),
            runtime,
            executor,
            lxapp_stack: Mutex::new(VecDeque::with_capacity(capacity)),
            pending_destroy: Mutex::new(HashMap::new()),
        }
    }

    /// Ensure an LxApp instance exists for the given appid.
    pub(crate) fn ensure_lxapp(
        &self,
        appid: String,
        release_type: ReleaseType,
    ) -> Result<Arc<LxApp>, LxAppError> {
        let has_pending_update = metadata::downloaded_get(&appid, release_type)
            .map(|opt| opt.is_some())
            .unwrap_or(false);

        if has_pending_update {
            // Tear down any existing instance before applying new files
            self.destroy_lxapp(&appid);
            if let Err(e) =
                UpdateManager::apply_downloaded_update(self.runtime.clone(), &appid, release_type)
            {
                error!(
                    "Failed to apply downloaded update before opening app: {}",
                    e
                )
                .with_appid(appid.clone());
            }
        } else if let Some(app_arc) = self.lxapps.get(&appid) {
            return Ok(app_arc.clone());
        }

        // Create new LxApp
        let new_lxapp = Arc::new(LxApp::new(
            appid.clone(),
            self.runtime.clone(),
            self.executor.clone(),
            release_type,
        )?);

        // Insert into collection and return
        self.lxapps.insert(appid, new_lxapp.clone());
        Ok(new_lxapp)
    }

    /// Completely destroy an LxApp (shutdown + removal from manager and stack).
    fn destroy_lxapp_with_options(&self, appid: &str, skip_hide: bool) {
        if let Some(app_arc) = self.lxapps.get(appid) {
            let _ = app_arc.shutdown_with_options(skip_hide);
        }
        self.remove_from_stack(appid);
        self.lxapps.remove(appid);
    }

    /// Completely destroy an LxApp with normal hide behavior.
    fn destroy_lxapp(&self, appid: &str) {
        self.destroy_lxapp_with_options(appid, false);
    }

    /// Recreate the LxApp instance for a given appid with a brand new instance.
    /// Used by restart to force a fresh session and runtime state.
    fn recreate_lxapp(
        &self,
        appid: String,
        release_type: ReleaseType,
    ) -> Result<Arc<LxApp>, LxAppError> {
        // Close handshake is handled by restart state machine; avoid a second hide while recreating.
        self.destroy_lxapp_with_options(&appid, true);

        // Delegate to ensure_lxapp so pending downloaded updates are applied
        // consistently (same path as cold-start navigation).
        self.ensure_lxapp(appid, release_type)
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
            // UI/JSContext/PageInstance/WebView/AppService are cleaned up deterministically.
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
            std::mem::drop(crate::executor::spawn(async move {
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
            }));
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
        let max = get_num_workers();
        if let Ok(mut stack) = self.lxapp_stack.lock() {
            if stack.len() < max {
                stack.push_back(appid);
            } else {
                warn!(
                    "LxApp navigation stack is full (capacity: {}). Cannot push app: {}",
                    max, appid
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
        let max = get_num_workers();
        if let Ok(stack) = self.lxapp_stack.lock() {
            stack.len() >= max
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
    pub(crate) pages: Mutex<HashMap<String, PageInstance>>,

    /// Runtime page instances keyed by stable instance id.
    pub(crate) pages_by_id: Mutex<HashMap<String, PageInstance>>,

    /// Runtime metadata and lifecycle state keyed by page instance id.
    page_instance_runtime: Mutex<HashMap<String, PageInstanceRuntimeRecord>>,

    /// Delayed dispose timers for hidden page instances.
    page_instance_dispose_timers: Mutex<HashMap<String, oneshot::Sender<()>>>,

    /// PageInstance navigation stack for tracking page navigation history within this app
    /// Stores all pages for navigation history
    pub(crate) page_stack: Mutex<VecDeque<String>>,

    /// Time when this app was last active
    /// Used for LRU (Least Recently Used) eviction when memory is low
    pub(crate) last_active_time: Instant,

    /// Network security configuration for HTTPS domain filtering
    /// Manages which domains this app is allowed to access
    network_security: NetworkSecurity,

    /// TabBar runtime state
    /// Contains TabBar configuration and dynamic state (badges, red dots, visibility)
    pub tabbar: Option<tabbar::TabBar>,

    /// Startup options for the app
    pub(crate) startup_options: LxAppStartupOptions,

    /// Dynamic page surfaces created by lx.surface.open.
    pub(crate) surfaces: Mutex<SurfaceRecords>,

    /// App-level orientation override (runtime + persisted)
    pub(crate) orientation_override: Option<OrientationConfig>,
}

impl LxAppState {
    fn new() -> Self {
        Self {
            pages: Mutex::new(HashMap::new()),
            pages_by_id: Mutex::new(HashMap::new()),
            page_instance_runtime: Mutex::new(HashMap::new()),
            page_instance_dispose_timers: Mutex::new(HashMap::new()),
            page_stack: Mutex::new(VecDeque::with_capacity(PAGE_STACK_MAX)),
            last_active_time: Instant::now(),
            network_security: NetworkSecurity::new(),
            tabbar: None,
            startup_options: LxAppStartupOptions::default(),
            surfaces: Mutex::new(SurfaceRecords::new()),
            orientation_override: None,
        }
    }
}

/// Represents a single lxapplication
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LxAppBundleSource {
    Installed,
    DevPath {
        root: PathBuf,
    },
    /// Pages/logic bundled at `<appid>/...` inside the platform asset root.
    BuiltinAssets,
    /// Content-less host. `LxAppConfig` stays at default (empty pages/plugins,
    /// `logic_enabled() == false`). Used for SDK-internal hosts with no UI bundle.
    Synthetic,
}

pub struct LxApp {
    // Immutable data - initialized once and never changed
    pub appid: String,
    pub runtime: Arc<Platform>,
    pub lxapp_dir: PathBuf,
    pub(crate) bundle_source: LxAppBundleSource,
    pub storage_file_path: PathBuf,
    pub user_data_dir: PathBuf,
    pub user_cache_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub fingermark: String,
    pub is_home_lxapp: bool,
    pub(crate) release_type: ReleaseType,
    pub(crate) config: LxAppConfig,
    pub(crate) executor: Arc<LxAppWorkers>,
    home_update_check_dispatched: AtomicBool,
    pending_restart_request: AtomicBool,

    /// Current runtime session of this app (id + status)
    pub(crate) session: LxAppSession,

    // Mutable state - protected by mutex for fine-grained locking
    pub(crate) state: Mutex<LxAppState>,

    page_creation_lock: Mutex<()>,

    // Scripts injected into every page owned by this LxApp on page load.
    page_scripts: Mutex<Vec<Arc<str>>>,
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

impl LxAppSessionStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Opening => "opening",
            Self::Opened => "opened",
            Self::Closing => "closing",
            Self::Restarting => "restarting",
        }
    }
}

/// A single runtime session of a LxApp: id + status.
pub(crate) struct LxAppSession {
    pub(crate) id: LxAppSessionId,
    status: AtomicU8,
}

#[derive(Debug, Clone, Serialize)]
pub struct LxAppRuntimeInfo {
    pub appid: String,
    pub app_name: String,
    pub version: String,
    pub release_type: String,
    pub session_id: u64,
    pub status: String,
    pub is_home: bool,
    pub current_page: Option<String>,
    pub initial_route: String,
    pub pages_count: usize,
    pub page_entries: Vec<LxAppRuntimePageInfo>,
    pub page_stack: Vec<String>,
    pub lxapp_dir: String,
    pub data_dir: String,
    pub cache_dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LxAppRuntimePageInfo {
    pub name: String,
    pub path: String,
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

/// Session helpers and lifecycle utilities for LxApp.
impl LxApp {
    /// Helper to clone Arc<Self> from within methods needing Arc
    pub(crate) fn clone_arc(&self) -> Arc<LxApp> {
        // All LxApp instances are stored as Arc in the global manager; retrieve by appid
        crate::lxapp::get(self.appid.clone())
    }

    pub(crate) fn status(&self) -> LxAppSessionStatus {
        self.session.status()
    }

    pub fn session_id(&self) -> LxAppSessionId {
        self.session.id
    }

    pub(crate) fn sync_host_ui(&self) {
        if let Err(err) = self.runtime.update_navbar_ui(self.appid.clone()) {
            warn!("Failed to update host NavigationBar UI: {}", err).with_appid(self.appid.clone());
        }
        if let Err(err) = self.runtime.update_tabbar_ui(self.appid.clone()) {
            warn!("Failed to update host TabBar UI: {}", err).with_appid(self.appid.clone());
        }
    }

    pub fn grant_transient_file_access(&self, path: &Path) -> Result<uri::LxUri, LxAppError> {
        self.grant_transient_path_access(path, TransientPathKind::File)
    }

    pub fn grant_transient_file_reference(&self, reference: &str) -> Result<String, LxAppError> {
        let normalized = normalize_transient_file_reference(reference)?;
        TRANSIENT_FILE_REFERENCE_GRANTS
            .get_or_init(DashMap::new)
            .insert(
                (self.appid.clone(), self.session_id(), normalized.clone()),
                (),
            );
        Ok(normalized)
    }

    pub fn has_transient_file_reference(&self, reference: &str) -> bool {
        let Ok(normalized) = normalize_transient_file_reference(reference) else {
            return false;
        };
        TRANSIENT_FILE_REFERENCE_GRANTS
            .get_or_init(DashMap::new)
            .contains_key(&(self.appid.clone(), self.session_id(), normalized))
    }

    pub fn register_temp_file(&self, path: &Path) -> Result<uri::LxUri, LxAppError> {
        self.cleanup_temp_size(Some(path))?;
        let uri = self.grant_transient_file_access(path)?;
        Ok(uri)
    }

    pub fn temp_output_path(
        &self,
        category: &str,
        ext: Option<&str>,
    ) -> Result<PathBuf, LxAppError> {
        let category = category
            .chars()
            .map(|ch| match ch {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
                _ => '_',
            })
            .collect::<String>();
        let dir = self.temp_dir.join(category);
        std::fs::create_dir_all(&dir).map_err(|e| {
            LxAppError::IoError(format!("Failed to create temp output directory: {}", e))
        })?;
        let mut name = Uuid::new_v4().simple().to_string();
        if let Some(ext) = ext
            .map(str::trim)
            .map(|value| value.trim_start_matches('.'))
            .filter(|value| !value.is_empty())
        {
            name.push('.');
            name.push_str(ext);
        }
        Ok(dir.join(name))
    }

    pub fn grant_transient_directory_access(&self, path: &Path) -> Result<uri::LxUri, LxAppError> {
        self.grant_transient_path_access(path, TransientPathKind::Directory)
    }

    fn grant_transient_path_access(
        &self,
        path: &Path,
        kind: TransientPathKind,
    ) -> Result<uri::LxUri, LxAppError> {
        let normalized = normalize_transient_path(path, kind)?;
        let token = Uuid::new_v4().simple().to_string();
        TRANSIENT_FILE_GRANTS.get_or_init(DashMap::new).insert(
            (self.appid.clone(), self.session_id(), token.clone()),
            normalized,
        );
        uri::LxUri::from_str(&format!(
            "{}://{}/{}",
            uri::LX_SCHEME,
            uri::HOST_TEMP,
            token
        ))
        .map_err(LxAppError::InvalidParameter)
    }

    fn resolve_transient_file(&self, token: &str) -> Option<PathBuf> {
        TRANSIENT_FILE_GRANTS
            .get_or_init(DashMap::new)
            .get(&(self.appid.clone(), self.session_id(), token.to_string()))
            .map(|entry| entry.value().clone())
    }

    pub(crate) fn clear_transient_files(&self) {
        let appid = self.appid.clone();
        let session_id = self.session_id();
        if let Some(grants) = TRANSIENT_FILE_GRANTS.get() {
            grants.retain(|key, _| key.0 != appid || key.1 != session_id);
        }
        if let Some(grants) = TRANSIENT_FILE_REFERENCE_GRANTS.get() {
            grants.retain(|key, _| key.0 != appid || key.1 != session_id);
        }
        if !self.temp_dir.as_os_str().is_empty() {
            let _ = std::fs::remove_dir_all(&self.temp_dir);
        }
    }

    fn cleanup_temp_size(&self, keep: Option<&Path>) -> Result<(), LxAppError> {
        if self.temp_dir.as_os_str().is_empty() {
            return Ok(());
        }
        let Some(keep) = keep else {
            return Ok(());
        };
        let incoming = lingxia_service::storage::path_size(keep);
        lingxia_service::storage::ensure_temp_quota(&self.temp_dir, keep, incoming)
            .map_err(|err| LxAppError::ResourceExhausted(err.detail().to_string()))
    }

    fn status_name(&self) -> &'static str {
        self.status().as_str()
    }

    pub fn release_type(&self) -> ReleaseType {
        self.release_type
    }

    pub fn app_data_dir(&self) -> PathBuf {
        self.runtime.app_data_dir()
    }

    pub fn page_entries(&self) -> Vec<LxAppRuntimePageInfo> {
        self.config
            .page_entries()
            .into_iter()
            .map(|LxAppPageEntry { name, path }| LxAppRuntimePageInfo { name, path })
            .collect()
    }

    pub fn runtime_info(&self) -> LxAppRuntimeInfo {
        let info = self.get_lxapp_info();
        let page_entries = self.page_entries();
        LxAppRuntimeInfo {
            appid: self.appid.clone(),
            app_name: info.app_name,
            version: info.version,
            release_type: info.release_type,
            session_id: self.session_id(),
            status: self.status_name().to_string(),
            is_home: self.is_home_lxapp,
            current_page: self.peek_current_page(),
            initial_route: self.initial_route(),
            pages_count: page_entries.len(),
            page_entries,
            page_stack: self.get_page_stack(),
            lxapp_dir: self.lxapp_dir.to_string_lossy().into_owned(),
            data_dir: self.user_data_dir.to_string_lossy().into_owned(),
            cache_dir: self.user_cache_dir.to_string_lossy().into_owned(),
        }
    }

    pub async fn eval_logic(&self, script: String) -> Result<serde_json::Value, LxAppError> {
        let json = self
            .executor
            .eval_app_service(self.clone_arc(), script)
            .await?;
        serde_json::from_str(&json).map_err(LxAppError::from)
    }

    pub(crate) fn set_status(&self, s: LxAppSessionStatus) {
        self.session.set_status(s);
    }

    pub(crate) fn cas_status(&self, from: LxAppSessionStatus, to: LxAppSessionStatus) -> bool {
        self.session.cas_status(from, to)
    }

    pub(crate) fn trigger_home_update_check_once(&self) {
        if !self.is_home_lxapp {
            return;
        }
        if self
            .home_update_check_dispatched
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            UpdateManager::spawn_release_lxapp_update_check(self.appid.clone());
        }
    }

    pub(crate) fn has_pending_restart_request(&self) -> bool {
        self.pending_restart_request.load(Ordering::SeqCst)
    }

    fn cancel_page_instance_dispose_timer(&self, id: &PageInstanceId) {
        self.cancel_page_instance_dispose_timer_by_id(id.as_str());
    }

    fn cancel_page_instance_dispose_timer_by_id(&self, id: &str) {
        if let Ok(state) = self.state.lock()
            && let Some(cancel) = state
                .page_instance_dispose_timers
                .lock()
                .unwrap()
                .remove(id)
        {
            let _ = cancel.send(());
        }
    }

    fn cancel_all_page_instance_dispose_timers(&self) {
        if let Ok(state) = self.state.lock() {
            let mut timers = state.page_instance_dispose_timers.lock().unwrap();
            for (_id, cancel) in timers.drain() {
                let _ = cancel.send(());
            }
        }
    }

    fn schedule_page_instance_dispose_timer(
        &self,
        id: &PageInstanceId,
        dispose_ttl: Duration,
    ) -> Result<(), LxAppError> {
        // When the TTL fires, the page is being reclaimed by the SDK because
        // it stayed hidden too long — not because the consumer asked for it.
        // Always carry `Reclaimed` so JS-side close listeners can distinguish
        // SDK-initiated cleanup from a user/programmatic close.
        let reclaim_reason = CloseReason::Reclaimed;
        if dispose_ttl.is_zero() {
            return self.dispose_page_instance_internal(id, reclaim_reason, false);
        }

        self.cancel_page_instance_dispose_timer(id);

        let (tx, rx) = oneshot::channel();
        if let Ok(state) = self.state.lock() {
            state
                .page_instance_dispose_timers
                .lock()
                .unwrap()
                .insert(id.to_string(), tx);
        }

        let appid = self.appid.clone();
        let page_instance_id = id.to_string();
        std::mem::drop(crate::executor::spawn(async move {
            let sleep = time::sleep(dispose_ttl);
            tokio::pin!(sleep);
            tokio::pin!(rx);
            tokio::select! {
                _ = &mut sleep => {}
                _ = &mut rx => return,
            }

            let Some(app) = crate::lxapp::try_get(&appid) else {
                return;
            };
            let Some(id) = PageInstanceId::parse(page_instance_id.clone()) else {
                return;
            };
            if let Err(err) = app.dispose_page_instance_internal(&id, reclaim_reason, false) {
                warn!(
                    "Delayed dispose failed for page instance {}: {}",
                    page_instance_id, err
                )
                .with_appid(appid);
            }
        }));

        Ok(())
    }

    fn refresh_page_instance_dispose_ttl(&self, id: &PageInstanceId) -> Result<(), LxAppError> {
        let (lifecycle, dispose_ttl) = {
            let state = self.state.lock().unwrap();
            let records = state.page_instance_runtime.lock().unwrap();
            let record = records.get(id.as_str()).ok_or_else(|| {
                LxAppError::ResourceNotFound(format!("page instance id: {}", id.as_str()))
            })?;
            (record.lifecycle, record.dispose_ttl)
        };

        if lifecycle != PageInstanceLifecycleState::Hidden {
            self.cancel_page_instance_dispose_timer(id);
            return Ok(());
        }

        if let Some(ttl) = dispose_ttl {
            self.schedule_page_instance_dispose_timer(id, ttl)?;
        } else {
            self.cancel_page_instance_dispose_timer(id);
        }

        Ok(())
    }

    // AppService state subscriptions removed for simplicity; rely on FIFO ordering.
    /// Shutdown this LxApp completely. Idempotent.
    ///
    /// Order:
    /// 1) Mark Closing to suppress page terminations
    /// 2) Close UI window
    /// 3) Break PageInstance↔WebView delegate links and clear pages
    /// 4) Destroy platform WebViews
    /// 5) Clear page stack and surfaces
    /// 6) Send TerminateAppSvc (receiver handles teardown)
    pub fn shutdown_with_options(&self, skip_hide: bool) -> Result<(), LxAppError> {
        // Mark closing to suppress TerminatePage from PageInstance drops
        self.set_status(LxAppSessionStatus::Closing);
        self.clear_transient_files();
        self.cancel_all_page_instance_dispose_timers();
        self.close_all_surfaces(CloseReason::AppClosed);
        crate::lifecycle::key_events::clear(&self.appid, self.session.id);

        // Close UI window
        if !skip_hide {
            let _ = self
                .runtime
                .hide_lxapp(self.appid.clone(), self.session.id)
                .map_err(LxAppError::from);
        }

        // Collect current pages
        let (page_webtags, page_instance_ids): (Vec<WebTag>, Vec<String>) = {
            let state = self.state.lock().unwrap();
            let pages_by_id = state.pages_by_id.lock().unwrap();
            (
                pages_by_id.values().map(|page| page.webtag()).collect(),
                pages_by_id
                    .values()
                    .map(|page| page.instance_id_string())
                    .collect(),
            )
        };
        crate::view_call::cancel_view_calls_for_page_instances(
            &page_instance_ids,
            "PageInstance removed while waiting for view response",
        );

        // Break PageInstance <-> WebView links early and detach WebViews, then drop pages by clearing the map
        if let Ok(state) = self.state.lock() {
            for (_k, page) in state.pages.lock().unwrap().iter() {
                page.detach_webview();
            }
        }
        if let Ok(state) = self.state.lock() {
            state.pages.lock().unwrap().clear();
            state.pages_by_id.lock().unwrap().clear();
            state.page_instance_runtime.lock().unwrap().clear();
        }
        for webtag in &page_webtags {
            destroy_webview(webtag);
        }
        let _ = self.clear_page_stack();
        // Terminate AppService (receiver handles its own state)
        let _ = self.executor.terminate_app_svc(self.clone_arc());
        Ok(())
    }

    pub fn shutdown(&self) -> Result<(), LxAppError> {
        self.shutdown_with_options(false)
    }

    fn _new(
        appid: String,
        runtime: Arc<Platform>,
        executor: Arc<LxAppWorkers>,
        release_type: ReleaseType,
    ) -> Self {
        let session = LxAppSession::new();
        let bundle_source = lxapp_bundle_source_for(&appid).unwrap_or(LxAppBundleSource::Installed);
        Self {
            appid,
            runtime,
            lxapp_dir: PathBuf::new(),
            bundle_source,
            storage_file_path: PathBuf::new(),
            user_data_dir: PathBuf::new(),
            user_cache_dir: PathBuf::new(),
            temp_dir: PathBuf::new(),
            fingermark: String::new(),
            is_home_lxapp: false,
            release_type,
            config: LxAppConfig::default(),
            executor,
            home_update_check_dispatched: AtomicBool::new(false),
            pending_restart_request: AtomicBool::new(false),
            session,
            state: Mutex::new(LxAppState::new()),
            page_creation_lock: Mutex::new(()),
            page_scripts: Mutex::new(Vec::new()),
        }
    }

    /// Create a new regular mini-app (not home app)
    pub(crate) fn new(
        appid: String,
        runtime: Arc<Platform>,
        executor: Arc<LxAppWorkers>,
        release_type: ReleaseType,
    ) -> Result<Self, LxAppError> {
        let mut app = Self::_new(appid, runtime, executor, release_type);
        app.setup().inspect_err(|e| {
            error!("Setup failed: {}", e).with_appid(&app.appid);
        })?;
        Ok(app)
    }

    /// Create a new LxApp instance marked as the home lxapp
    fn new_as_home(
        appid: String,
        runtime: Arc<Platform>,
        executor: Arc<LxAppWorkers>,
    ) -> Result<Self, LxAppError> {
        let mut app = Self::_new(appid, runtime, executor, ReleaseType::Release);

        // Mark as home lxapp
        app.is_home_lxapp = true;

        app.setup().inspect_err(|e| {
            error!("Setup failed for home app: {}", e).with_appid(&app.appid);
        })?;
        Ok(app)
    }

    /// Initialize paths and directories for the lxapp
    fn initialize_paths(&mut self) -> Result<(), LxAppError> {
        // Load metadata if available to determine version and install path
        let meta = metadata::get(&self.appid, self.release_type).ok().flatten();
        self.fingermark = meta
            .as_ref()
            .map(|record| record.fingermark.clone())
            .unwrap_or_else(|| lxapp_fingermark(&self.appid, self.release_type));
        let dir_name = self.fingermark.clone();
        // Set up app directory (default path)
        let base_dir = self
            .runtime
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(LXAPPS_DIR);
        self.lxapp_dir = base_dir.join(&dir_name);

        match &self.bundle_source {
            LxAppBundleSource::Installed => {
                if let Some(install_path) = meta
                    .as_ref()
                    .map(|record| record.install_path.trim())
                    .filter(|path| !path.is_empty())
                {
                    self.lxapp_dir = PathBuf::from(install_path);
                }
            }
            LxAppBundleSource::DevPath { root } => {
                info!("Using dev path for lxapp bundle: {}", root.display())
                    .with_appid(self.appid.clone());
                self.lxapp_dir = root.clone();
            }
            LxAppBundleSource::BuiltinAssets | LxAppBundleSource::Synthetic => {
                self.lxapp_dir = self
                    .runtime
                    .app_data_dir()
                    .join(LINGXIA_DIR)
                    .join("builtin")
                    .join(&dir_name);
            }
        }

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

        // Set up LingXia-managed user cache directory. This is intentionally under app data,
        // not the OS cache directory, because LingXia owns usercache cleanup policy.
        let cache_base_dir = self
            .runtime
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(USER_CACHE_DIR);

        self.user_cache_dir = cache_base_dir.join(&dir_name);
        if !self.user_cache_dir.exists() {
            std::fs::create_dir_all(&self.user_cache_dir).map_err(|e| {
                LxAppError::IoError(format!("Failed to create cache directory: {}", e))
            })?;
        }

        let temp_base_dir = self
            .runtime
            .app_cache_dir()
            .join(LINGXIA_DIR)
            .join(LXAPPS_DIR)
            .join(TEMP_DIR)
            .join(&dir_name);
        let _ = std::fs::create_dir_all(&temp_base_dir);
        if let Ok(entries) = std::fs::read_dir(&temp_base_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let stale = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name != self.session_id().to_string());
                if stale && path.is_dir() {
                    let _ = std::fs::remove_dir_all(path);
                }
            }
        }
        self.temp_dir = temp_base_dir.join(self.session_id().to_string());
        if !self.temp_dir.exists() {
            std::fs::create_dir_all(&self.temp_dir).map_err(|e| {
                LxAppError::IoError(format!("Failed to create temp directory: {}", e))
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
        self.read_json("lxapp.json").map(|app_json| {
            self.config = LxAppConfig::from_value(app_json)
                .map_err(|e| LxAppError::InvalidJsonFile(format!("lxapp.json: {}", e)))?;

            {
                let mut state = self.state.lock().unwrap();
                state
                    .network_security
                    .set_domains(self.config.trusted_domains());
            }

            // Initialize TabBar state if config has TabBar
            if let Some(tabbar_config) = &self.config.tabBar {
                let mut state = self.state.lock().unwrap();
                // Convert icon paths to absolute paths using the lxapp directory as base
                state.tabbar = Some(tabbar_config.with_absolute_paths(&self.lxapp_dir));
            }

            Ok(())
        })?
    }

    /// Initialize paths and load configuration
    fn setup(&mut self) -> Result<(), LxAppError> {
        self.initialize_paths()?;
        if matches!(self.bundle_source, LxAppBundleSource::Synthetic) {
            // No `lxapp.json` to read. The default `LxAppConfig.logic = None` resolves to
            // `Some("logic.js")` (documented default for normal lxapps); force it off so
            // `logic_enabled()` / `logic_entry_source` don't spin up JS workers we have
            // no source for.
            self.config.logic = Some(LxAppLogicEntry::Enabled(false));
        } else {
            self.load_config()?;
        }
        Ok(())
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

    pub fn logic_enabled(&self) -> bool {
        self.config.logic_entry().is_some()
    }

    #[cfg(feature = "js-appservice")]
    pub async fn logic_entry_source(&self, ctx: &JSContext) -> JSResult<Option<Source>> {
        let Some(entry) = self.config.logic_entry() else {
            return Ok(None);
        };
        if Path::new(&entry).extension().and_then(|ext| ext.to_str()) != Some("js") {
            return Err(HostError::new(
                rong::error::E_NOT_SUPPORTED,
                format!("lxapp logic entry must be a .js file: {}", entry),
            )
            .into());
        }

        match &self.bundle_source {
            LxAppBundleSource::Installed | LxAppBundleSource::DevPath { .. } => {
                let source_path = self.lxapp_dir.join(&entry);
                Source::from_path(ctx, &source_path).await.map(Some)
            }
            LxAppBundleSource::Synthetic => unreachable!(
                "synthetic lxapp {} forces logic=false at setup(); logic_entry() must be None",
                self.appid
            ),
            LxAppBundleSource::BuiltinAssets => {
                let asset_path = format!(
                    "{}/{}",
                    self.appid.trim_end_matches('/'),
                    entry.trim_start_matches('/')
                );
                let mut reader = self.runtime.read_asset(&asset_path).map_err(|err| {
                    HostError::new(
                        rong::error::E_NOT_FOUND,
                        format!("builtin lxapp logic not found: {} ({})", asset_path, err),
                    )
                })?;
                let mut data = Vec::new();
                reader.read_to_end(&mut data).map_err(|err| {
                    HostError::new(
                        rong::error::E_IO,
                        format!(
                            "failed to read builtin lxapp logic: {} ({})",
                            asset_path, err
                        ),
                    )
                })?;
                Ok(Some(Source::from_bytes(data).with_name(asset_path)))
            }
        }
    }

    pub fn get_app_orientation(&self) -> OrientationConfig {
        let state = self.state.lock().unwrap();
        state.orientation_override.unwrap_or_default()
    }

    pub fn set_app_orientation(&self, orientation: OrientationConfig) {
        let orientation = OrientationConfig::normalize(orientation.mode, orientation.rotation);
        let mut state = self.state.lock().unwrap();
        state.orientation_override = Some(orientation);
    }

    /// Get resolved orientation for a page; falls back to app-level config.
    pub fn get_page_orientation(&self, path: &str) -> OrientationConfig {
        let app_orientation = self.get_app_orientation();
        let page_override = self
            .get_page(path)
            .and_then(|page| page.get_orientation_override())
            .unwrap_or_default();
        page_override.apply(app_orientation)
    }

    // Reads binary data from the specified relative path
    fn read_bytes(&self, relative_path: &str) -> Result<Vec<u8>, LxAppError> {
        if matches!(self.bundle_source, LxAppBundleSource::Synthetic) {
            return Err(LxAppError::ResourceNotFound(format!(
                "{relative_path}: synthetic lxapp host {} has no on-disk content",
                self.appid
            )));
        }
        let file_path = match crate::plugin::resolve_plugin_resource_path_from_internal_path(
            &self.runtime,
            &self.config.plugins,
            relative_path,
        )? {
            Some(path) => path,
            None => {
                if matches!(self.bundle_source, LxAppBundleSource::BuiltinAssets) {
                    let asset_path = format!(
                        "{}/{}",
                        self.appid.trim_end_matches('/'),
                        relative_path.trim_start_matches('/')
                    );
                    let mut reader = self.runtime.read_asset(&asset_path).map_err(|e| {
                        LxAppError::ResourceNotFound(format!(
                            "{relative_path}:{e} (asset: {asset_path})"
                        ))
                    })?;
                    let mut data = Vec::new();
                    reader.read_to_end(&mut data).map_err(|e| {
                        LxAppError::ResourceNotFound(format!(
                            "{relative_path}:{e} (asset: {asset_path})"
                        ))
                    })?;
                    return Ok(data);
                }
                self.lxapp_dir.join(relative_path)
            }
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

    /// Resolve an "allowed" lxapp path (package dir, user data, user cache) to a physical path.
    ///
    /// This implementation uses logical mapping and prefix validation to ensure the path
    /// stays within the app's sandbox, without requiring the file to exist on disk.
    pub fn resolve_accessible_path(&self, path: &str) -> Result<PathBuf, LxAppError> {
        let path = path.trim();
        if path.is_empty() {
            return Err(LxAppError::ResourceNotFound("empty path".to_string()));
        }

        // 1. Handle lx:// URIs (Internal helper already does logical joining and ".." check)
        if path.starts_with("lx://") {
            let lx_uri = uri::LxUri::from_str(path)
                .map_err(|e| LxAppError::InvalidParameter(format!("invalid lx uri: {}", e)))?;
            return self.resolve_lx_path_uri(&lx_uri);
        }

        // 2. Prevent directory traversal for any input
        if path.split('/').any(|s| s == "..") {
            return Err(LxAppError::ResourceNotFound(
                "directory traversal not allowed".to_string(),
            ));
        }

        let path_ref = Path::new(path);

        // 3. Handle Relative path: search in order user data -> user cache -> package
        if !path_ref.is_absolute() && !path.contains(':') {
            let rel = path.trim_start_matches('/');

            // In a simple logical resolve, we prioritize user data for relative paths
            // or we could stick to a specific root. Here we check existence only for
            // relative path "discovery" if we want to maintain the old search behavior,
            // otherwise we default to a specific root.

            // To keep it simple and predictable for "creation", relative paths
            // without lx:// prefix are resolved against the app bundle root by default.
            return Ok(self.lxapp_dir.join(rel));
        }

        // 4. Handle Absolute paths: Must start with one of the trusted roots.
        //
        // On Apple platforms, the same sandbox path may appear with different
        // spellings (for example `/var/...` vs `/private/var/...`). When the
        // target exists, compare canonicalized paths as well so chooser-returned
        // absolute paths remain accessible.
        let trusted_roots = [
            (&self.lxapp_dir, "app bundle"),
            (&self.user_data_dir, "user data"),
            (&self.user_cache_dir, "user cache"),
            (&self.temp_dir, "temp"),
        ];

        let resolved_target = std::fs::canonicalize(path_ref).ok();

        for (root, _name) in trusted_roots {
            if root.as_os_str().is_empty() {
                continue;
            }
            if path_ref.starts_with(root) {
                return Ok(path_ref.to_path_buf());
            }
            if let (Some(target), Ok(canonical_root)) =
                (resolved_target.as_ref(), std::fs::canonicalize(root))
                && target.starts_with(&canonical_root)
            {
                return Ok(target.to_path_buf());
            }
        }

        // Also check if it's under the parents of userdata/usercache to support the
        // full path directory structure if needed (though usually not recommended for JS)
        for root in [&self.user_data_dir, &self.user_cache_dir] {
            if let Some(parent) = root.parent() {
                if path_ref.starts_with(parent) {
                    return Ok(path_ref.to_path_buf());
                }
                if let (Some(target), Ok(canonical_parent)) =
                    (resolved_target.as_ref(), std::fs::canonicalize(parent))
                    && target.starts_with(&canonical_parent)
                {
                    return Ok(target.to_path_buf());
                }
            }
        }

        Err(LxAppError::ResourceNotFound(format!(
            "Access denied: {}",
            path
        )))
    }

    pub fn to_uri(&self, path: &Path) -> Option<uri::LxUri> {
        if !self.temp_dir.as_os_str().is_empty() && path.starts_with(&self.temp_dir) {
            return self.register_temp_file(path).ok();
        }
        uri::try_convert_path_to_uri(path, self)
    }

    fn resolve_lx_path_uri(&self, lx_uri: &uri::LxUri) -> Result<PathBuf, LxAppError> {
        let uri = HttpUri::from_str(lx_uri.as_str())
            .map_err(|_| LxAppError::InvalidParameter("invalid lx uri".to_string()))?;

        if uri.scheme_str() != Some(uri::LX_SCHEME) {
            return Err(LxAppError::InvalidParameter(
                "invalid lx uri scheme".to_string(),
            ));
        }

        match uri.host() {
            Some(uri::HOST_TEMP) => {
                if uri.query().is_some() {
                    return Err(LxAppError::ResourceNotFound(lx_uri.as_str().to_string()));
                }
                let token = uri.path().trim_matches('/');
                if token.is_empty() || token.contains('/') || token.contains('\\') {
                    return Err(LxAppError::ResourceNotFound(lx_uri.as_str().to_string()));
                }
                self.resolve_transient_file(token).ok_or_else(|| {
                    LxAppError::ResourceNotFound(format!(
                        "temporary file grant not found: {}",
                        lx_uri.as_str()
                    ))
                })
            }
            Some(uri::HOST_USER_CACHE) | Some(uri::HOST_USER_DATA) => {
                let base_dir = match uri.host() {
                    Some(uri::HOST_USER_CACHE) => &self.user_cache_dir,
                    Some(uri::HOST_USER_DATA) => &self.user_data_dir,
                    _ => unreachable!(),
                };

                let decoded_path = uri::decode_lx_path(uri.path());
                let rel = decoded_path.trim_matches('/');
                if rel.is_empty() {
                    return Ok(base_dir.clone());
                }
                if uri::has_invalid_segment(rel) || rel.contains(':') || rel.contains('\\') {
                    return Err(LxAppError::ResourceNotFound(lx_uri.as_str().to_string()));
                }

                Ok(base_dir.join(rel))
            }
            Some(uri::HOST_LXAPP) => {
                let decoded_path = uri::decode_lx_path(uri.path());
                let raw = decoded_path.trim_start_matches('/');
                let (appid, rest) = raw
                    .split_once('/')
                    .ok_or_else(|| LxAppError::ResourceNotFound(lx_uri.as_str().to_string()))?;
                if appid != self.appid.as_str() {
                    return Err(LxAppError::ResourceNotFound(lx_uri.as_str().to_string()));
                }

                let rel = rest.trim_matches('/');
                if rel.is_empty() {
                    return Err(LxAppError::ResourceNotFound(lx_uri.as_str().to_string()));
                }
                if uri::has_invalid_segment(rel) || rel.contains(':') || rel.contains('\\') {
                    return Err(LxAppError::ResourceNotFound(lx_uri.as_str().to_string()));
                }

                Ok(self.lxapp_dir.join(rel))
            }
            _ => Err(LxAppError::ResourceNotFound(format!(
                "unsupported lx uri host: {}",
                lx_uri.as_str()
            ))),
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

    pub fn is_opened(&self) -> bool {
        matches!(self.status(), LxAppSessionStatus::Opened)
    }

    /// Register a script to inject on every page load within this LxApp.
    ///
    /// Use this for app-specific scripts (e.g. browser context-menu).
    /// For scripts that should run in *all* apps, use [`add_global_page_script`].
    pub fn add_page_script(&self, js: impl Into<String>) {
        if let Ok(mut scripts) = self.page_scripts.lock() {
            scripts.push(Arc::from(js.into()));
        }
    }

    /// Snapshot page scripts for a new PageInstance: global scripts + this app's scripts.
    pub(crate) fn page_scripts_snapshot(&self) -> Vec<Arc<str>> {
        let mut scripts = crate::page::global_page_scripts_snapshot();
        if let Ok(app_scripts) = self.page_scripts.lock() {
            scripts.extend(app_scripts.iter().cloned());
        }
        scripts
    }

    /// Check if a domain is allowed for network access
    pub fn is_domain_allowed(&self, domain: &str) -> bool {
        self.state
            .lock()
            .unwrap()
            .network_security
            .is_domain_allowed(domain)
    }

    /// Check whether this lxapp declares a high-risk security privilege.
    ///
    /// Intended for privileged host APIs such as automation/devtools. Ordinary
    /// host capabilities such as camera/media/location should continue to rely
    /// on the host app and platform permission flow.
    pub fn has_security_privilege(&self, privilege: &LxAppSecurityPrivilege) -> bool {
        self.config.has_security_privilege(privilege)
    }

    /// Get a page by path
    pub fn get_page(&self, path: &str) -> Option<PageInstance> {
        self.state
            .lock()
            .unwrap()
            .pages
            .lock()
            .unwrap()
            .get(path)
            .cloned()
    }

    pub fn get_page_by_instance_id(&self, id: &PageInstanceId) -> Option<PageInstance> {
        self.get_page_by_instance_id_str(id.as_str())
    }

    pub fn get_page_by_instance_id_str(&self, id: &str) -> Option<PageInstance> {
        self.state
            .lock()
            .unwrap()
            .pages_by_id
            .lock()
            .unwrap()
            .get(id)
            .cloned()
    }

    pub fn page_instance_id_for_path(&self, path: &str) -> Option<String> {
        self.get_page(path).map(|page| page.instance_id_string())
    }

    pub fn initial_route(&self) -> String {
        self.config.get_initial_route()
    }

    /// Ensure the JS app service worker is running for this app.
    pub fn ensure_app_service_running(&self) -> Result<(), LxAppError> {
        self.executor.create_app_svc(self.clone_arc())
    }

    fn remove_registered_headless_page_if_current(&self, path: &str, page: &PageInstance) {
        if let Ok(state) = self.state.lock() {
            let id = page.instance_id_string();
            let mut pages = state.pages.lock().unwrap();
            if pages
                .get(path)
                .is_some_and(|current| current.instance_id_string() == id)
            {
                pages.remove(path);
            }
            state.pages_by_id.lock().unwrap().remove(id.as_str());
        }
    }

    pub fn ensure_headless_page_service(&self, path: &str) -> Result<PageInstance, LxAppError> {
        let _creation_guard = self.page_creation_lock.lock().unwrap();
        if let Some(page) = self.get_page(path) {
            return Ok(page);
        }

        let candidate = PageInstance::new_headless(self.appid.clone(), path.to_string(), self);
        let page = {
            let state = self.state.lock().unwrap();
            let mut pages = state.pages.lock().unwrap();

            if let Some(page) = pages.get(path) {
                page.clone()
            } else {
                state
                    .pages_by_id
                    .lock()
                    .unwrap()
                    .entry(candidate.instance_id_string())
                    .or_insert_with(|| candidate.clone());
                pages.insert(path.to_string(), candidate.clone());
                candidate
            }
        };
        drop(_creation_guard);

        let (ack_tx, ack_rx) = oneshot::channel::<Result<(), String>>();
        if let Err(err) =
            self.executor
                .create_page_svc_with_ack(self.clone_arc(), path.to_string(), None, ack_tx)
        {
            page.mark_webview_ready(Err(err.to_string()));
            self.remove_registered_headless_page_if_current(path, &page);
            return Err(err);
        }

        let page_clone = page.clone();
        let lxapp = self.clone_arc();
        let path = path.to_string();
        crate::executor::spawn(async move {
            let result = match ack_rx.await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(e),
                Err(err) => Err(err.to_string()),
            };
            if result.is_err() {
                lxapp.remove_registered_headless_page_if_current(&path, &page_clone);
            }
            page_clone.mark_webview_ready(result);
        });

        Ok(page)
    }

    /// Check if pull-to-refresh is enabled for a specific page
    pub fn is_pull_down_refresh_enabled(&self, path: &str) -> bool {
        self.get_page(path)
            .map(|page| page.is_pull_down_refresh_enabled())
            .unwrap_or(false)
    }

    /// Get navigation bar state for a page; returns default if page not found.
    pub fn get_navbar_state(&self, path: &str) -> NavigationBarState {
        let resolved_path = self
            .find_page_path(
                path.split('?')
                    .next()
                    .unwrap_or(path)
                    .split('#')
                    .next()
                    .unwrap_or(path),
            )
            .unwrap_or_else(|| path.to_string());

        self.get_page(path)
            .or_else(|| self.get_page(&resolved_path))
            .and_then(|page| page.get_navbar_state())
            .unwrap_or_default()
    }

    pub(crate) fn open(&self, options: LxAppStartupOptions) -> Result<(), LxAppError> {
        if self.logic_enabled() && !crate::js_appservice_supported() {
            return Err(LxAppError::UnsupportedOperation(
                "this host app was built without JS AppService runtime".to_string(),
            ));
        }

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

        // Ensure the target app's JS worker is created and mapped before creating pages.
        // View-only lxapps (`logic: false`) skip this path.
        if let Err(e) = self.executor.create_app_svc(self.clone_arc()) {
            error!("Failed to trigger app service: {}", e).with_appid(self.appid.clone());
        }

        // Create native PageInstance + WebView
        let page = self.get_or_create_page(&startup_options.path);
        page.set_query(startup_options.query.clone());

        // Open UI
        self.runtime.show_lxapp(
            self.appid.clone(),
            startup_options.path.clone(),
            self.session.id,
            startup_options.open_mode,
            startup_options.panel_id.clone(),
        )?;

        #[cfg(target_os = "windows")]
        {
            let surface = match startup_options.open_mode {
                lingxia_platform::traits::app_runtime::LxAppOpenMode::Panel => {
                    PresentationKind::Panel
                }
                lingxia_platform::traits::app_runtime::LxAppOpenMode::Normal => {
                    PresentationKind::Window
                }
            };
            let query = (!startup_options.query.is_empty())
                .then(|| PageQueryInput::Raw(startup_options.query.clone()));
            self.create_page_instance(
                PageOwner::Scene(SceneId("system".to_string())),
                PageTarget::Path(startup_options.path),
                query,
                surface,
                None,
            )?;
            if !matches!(
                startup_options.open_mode,
                lingxia_platform::traits::app_runtime::LxAppOpenMode::Panel
            ) {
                self.sync_host_ui();
            }
        }
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
                    get_num_workers(),
                    appid
                );
                return Ok(());
            }

            let app = manager.ensure_lxapp(appid.clone(), options.release_type)?;
            app.open(options)?;
        }
        Ok(())
    }

    /// Navigates back to the previous LxApp in the history stack.
    pub fn navigate_back(&self) -> Result<(), LxAppError> {
        // The on_lxapp_closed delegate will then handle removing it from the navigation stack.
        // The underlying UI framework should detect the app closure and automatically display the new app at the top of the stack.
        self.runtime
            .hide_lxapp(self.appid.clone(), self.session.id)?;
        Ok(())
    }

    /// Restarts the current LxApp with cleanup + reopen.
    /// This offloads the sequence to the service executor to avoid blocking JS worker.
    pub fn restart(&self) -> Result<(), LxAppError> {
        let from_session = self.session.id;
        let current_status = self.status();

        match current_status {
            // If restart is requested during Opening (e.g. applyUpdate in onLaunch),
            // queue it and consume once on_lxapp_opened finalizes status=Opened.
            LxAppSessionStatus::Opening
            | LxAppSessionStatus::Closed
            | LxAppSessionStatus::Closing => {
                self.pending_restart_request.store(true, Ordering::SeqCst);
                return Ok(());
            }
            LxAppSessionStatus::Opened => {}
            LxAppSessionStatus::Restarting => return Ok(()),
        }

        // Prevent overlapping restarts from races with other state transitions.
        if !self.cas_status(LxAppSessionStatus::Opened, LxAppSessionStatus::Restarting) {
            let current = self.status();
            if current == LxAppSessionStatus::Opening {
                self.pending_restart_request.store(true, Ordering::SeqCst);
            }
            return Ok(());
        }
        self.pending_restart_request.store(false, Ordering::SeqCst);

        if let Err(e) = self.runtime.hide_lxapp(self.appid.clone(), from_session) {
            error!(
                "Restart transition: failed to request close for session {}: {}",
                from_session, e
            )
            .with_appid(self.appid.clone());
        }

        // Always relaunch to initial route after restart.
        // Wait for the current session to report Closed (or timeout) before recreate+open,
        // so close/open callbacks do not race on the same appid.
        let relaunch_path = self.config.get_initial_route();
        let appid = self.appid.clone();
        let release_type = self.release_type;
        std::mem::drop(crate::executor::spawn(async move {
            let wait_deadline = Instant::now() + Duration::from_millis(1500);
            loop {
                let Some(current) = crate::lxapp::try_get(&appid) else {
                    break;
                };

                if current.session_id() != from_session {
                    return;
                }

                if current.status() == LxAppSessionStatus::Closed {
                    break;
                }

                if Instant::now() >= wait_deadline {
                    warn!(
                        "Restart transition: close wait timeout for session {}, forcing recreate",
                        from_session
                    )
                    .with_appid(appid.clone());
                    break;
                }

                time::sleep(Duration::from_millis(20)).await;
            }

            // 1) Replace LxApp instance in manager with a brand new one for this appid.
            if let Some(manager) = get_lxapps_manager() {
                let new_app = match manager.recreate_lxapp(appid.clone(), release_type) {
                    Ok(app) => app,
                    Err(e) => {
                        error!("Failed to recreate lxapp after restart: {}", e)
                            .with_appid(appid.clone());
                        return;
                    }
                };

                // 2) Initialize startup options for the new app session and open it.
                let options =
                    LxAppStartupOptions::new(&relaunch_path).set_release_type(release_type);
                if let Err(e) = new_app.open(options) {
                    error!("Failed to start lxapp after restart: {}", e);
                }
            }
            // Status will be driven back to Opened by on_lxapp_opened delegate after reopen.
        }));
        Ok(())
    }

    pub fn get_lxapp_info(&self) -> config::LxAppInfo {
        let mut info = self.config.get_lxapp_info(self.release_type.as_str());
        // Resolve the icon path relative to the lxapp directory, mirroring the
        // tabbar icon handling. Empty = the lxapp declared no icon.
        if !info.icon.is_empty() {
            info.icon = self
                .lxapp_dir
                .join(&info.icon)
                .to_string_lossy()
                .into_owned();
        }
        info
    }
}

/// Compute a stable hash id for lxapp-scoped data separation.
/// Includes lxappid + release_type + device_fingerprint to ensure isolation across variants and devices.
pub(crate) fn lxapp_fingermark(lxappid: &str, release_type: ReleaseType) -> String {
    // Fingermark uses appid + release_type + device fingerprint (version excluded)
    let device_fp = match crate::provider::get_provider().get_fingerprint() {
        Ok(fp) => fp,
        Err(e) => {
            warn!("Device fingerprint unavailable: {}", e);
            String::new()
        }
    };
    let combined = format!("{}|{}|{}", lxappid, release_type.as_str(), device_fp);
    let mut hasher = DefaultHasher::new();
    combined.hash(&mut hasher);
    format!("{:x}", hasher.finish())
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
