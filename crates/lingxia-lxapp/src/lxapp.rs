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
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tokio::time;
use uuid::Uuid;

use self::navbar::NavigationBarState;
use self::page_chrome::{
    AppearancePreference, EffectivePageChromeLayout, LxAppAppearanceState, ResolvedAppearance,
    TabBarPresentation, TabBarVisibilityPreference, VisibilityPreference,
};
use crate::appservice::LxAppWorkers;
use crate::error::LxAppError;
use crate::page::config::{OrientationConfig, PageConfig};
use crate::page::{PageInstance, PageInstanceId, ViewCallOptions};
use crate::startup::LxAppStartupOptions;
use crate::update::UpdateManager;
use crate::{debug, error, info, warn};
use security::NetworkSecurity;

pub mod config;
pub mod host_class;
use config::{LxAppConfig, LxAppLogicEntry, LxAppPageEntry};
mod content;
pub(crate) mod metadata;
pub mod navbar;
pub mod page_chrome;
mod page_instance_host;
mod runtime_bootstrap;
mod runtime_ops;
pub(crate) mod runtime_registry;
mod scheme;
pub(crate) mod security;
mod surface;
pub use security::{LxAppSecurityPrivilege, is_public_network_address};
pub mod tabbar;
pub mod uri;
pub(crate) mod version;
use crate::lifecycle::AppServiceEvent;
pub use crate::page::runtime::{
    CloseReason, CreatePageInstanceRequest, CreatedPageInstance, PageDefinition, PageInstanceEvent,
    PageInstanceRuntimeInfo, PageOwner, PageQueryInput, PageTarget, PresentationKind, ResolvedPage,
    SceneId,
};
use crate::page::runtime::{
    PageInstanceLifecycleState, PageInstanceRuntimeRecord, transition_page_instance_lifecycle,
};
pub use lingxia_platform::traits::ui::{SurfaceKind, SurfacePosition};
pub use lingxia_surface::Role as SurfaceRole;
pub use lingxia_update::ReleaseType;
use lingxia_webview::runtime::destroy_webview;
pub use runtime_bootstrap::dev_session_active as is_dev_session;
pub use runtime_bootstrap::init;
pub use runtime_bootstrap::runner_active as is_runner;
pub use runtime_bootstrap::{automation_auto_grant, set_automation_auto_grant};
pub use runtime_ops::{
    close_lxapp, create_page_instance, dispose_page_instance, dispose_page_instance_by_id,
    ensure_builtin_lxapp, ensure_host_surface_owner, ensure_lxapp, get_current_lxapp,
    installed_lxapp_path, is_lxapp_open, is_pull_down_refresh_enabled, list_lxapps,
    mark_lxapp_active, notify_lxapp_host_visibility, notify_page_host_visibility,
    notify_page_instance, notify_page_instance_by_id, on_low_memory, open_lxapp,
    refresh_auto_appearances, restart_lxapp, touch_page_instance_by_id, uninstall_lxapp,
};
pub(crate) use runtime_registry::get_lxapps_manager;
pub use runtime_registry::{
    find_page_by_instance_id, get_display_language, get_platform, set_display_language, try_get,
};
pub(crate) use surface::SurfaceRecords;
pub use surface::{
    HostMainSurfaceRegistration, HostSurfaceMenuExecution, LxAppRuntimeSurfaceInfo,
    ManagedNativeSurface, PageSurface, PageSurfaceRequest, PageSurfaceTarget, UrlCallbackSurface,
    UrlCallbackWaitError, register_surface_active_main_observer, register_surface_close_observer,
    register_surface_context_observer, register_surface_visibility_observer,
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
type PendingPageServiceRestart = (PageInstance, oneshot::Receiver<Result<(), String>>);
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
/// call for the same appid upgrades to a disk-backed bundle — used by browser-shell
/// to swap in the real browser shell webui on macOS.
pub fn register_synthetic_lxapp(appid: impl Into<String>) {
    register_lxapp_bundle_source(appid, LxAppBundleSource::Synthetic);
}

/// SDK-internal, content-less owner for a desktop host surface graph when the
/// product does not configure a home lxapp.
pub const HOST_SURFACE_OWNER_APP_ID: &str = "app.lingxia.host-surface-owner";

pub fn register_dev_bundle_source(appid: impl Into<String>, root: impl Into<PathBuf>) {
    register_lxapp_bundle_source(appid, LxAppBundleSource::DevPath { root: root.into() });
}

fn register_lxapp_bundle_source(appid: impl Into<String>, source: LxAppBundleSource) {
    let appid = appid.into();
    let registry = LXAPP_SOURCE_OVERRIDES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = registry.lock().unwrap_or_else(|e| e.into_inner());
    guard.insert(appid, source);
}

/// Whether `appid`'s bundle is managed by the update system. Answers for an
/// appid with no live instance too — first install runs before one exists.
pub(crate) fn is_ota_managed_appid(appid: &str) -> bool {
    !matches!(
        lxapp_bundle_source_for(appid),
        Some(LxAppBundleSource::DevPath { .. })
    )
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
    pending_destroy: Mutex<HashMap<String, PendingDestroy>>,
    next_destroy_generation: AtomicU64,
}

struct PendingDestroy {
    generation: u64,
    cancel: oneshot::Sender<()>,
}

fn replace_pending_destroy(
    pending: &mut HashMap<String, PendingDestroy>,
    appid: String,
    replacement: PendingDestroy,
) {
    if let Some(previous) = pending.insert(appid, replacement) {
        let _ = previous.cancel.send(());
    }
}

fn claim_pending_destroy(
    pending: &mut HashMap<String, PendingDestroy>,
    appid: &str,
    generation: u64,
) -> bool {
    if pending
        .get(appid)
        .is_some_and(|entry| entry.generation == generation)
    {
        pending.remove(appid);
        true
    } else {
        false
    }
}

fn first_evictable_appid(
    stack: &[String],
    mut is_evictable: impl FnMut(&str) -> bool,
) -> Option<String> {
    stack.iter().find(|appid| is_evictable(appid)).cloned()
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
            next_destroy_generation: AtomicU64::new(1),
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
        new_lxapp.bind_arc();

        // Publish with the map entry API. Two concurrent cold opens must both
        // receive the same LxApp instance; otherwise each instance could claim
        // a different shell region and defeat the one-app/one-region invariant.
        match self.lxapps.entry(appid) {
            dashmap::mapref::entry::Entry::Occupied(entry) => Ok(entry.get().clone()),
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                entry.insert(new_lxapp.clone());
                Ok(new_lxapp)
            }
        }
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
    /// Selects the first non-home live app from the least-recently-used end.
    fn evict_lru_lxapp(&self) {
        let candidates = {
            let Ok(stack) = self.lxapp_stack.lock() else {
                return;
            };
            stack.iter().cloned().collect::<Vec<_>>()
        };
        let Some(appid_to_destroy) = first_evictable_appid(&candidates, |appid| {
            self.lxapps.get(appid).is_some_and(|app| !app.is_home_lxapp)
        }) else {
            warn!("No non-home lxapp is available for eviction");
            return;
        };

        info!("Evicting least recently used lxapp").with_appid(appid_to_destroy.clone());

        // Explicitly shutdown the app before removing it from the map so that
        // UI/JSContext/PageInstance/WebView/AppService are cleaned up deterministically.
        self.destroy_lxapp(&appid_to_destroy);
    }

    /// Schedule a delayed destroy for an app; cancel on reopen.
    pub(crate) fn schedule_delayed_destroy(self: &Arc<Self>, appid: String) {
        let generation = self.next_destroy_generation.fetch_add(1, Ordering::Relaxed);
        let (cancel, rx) = oneshot::channel();
        {
            let mut pending = self
                .pending_destroy
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            replace_pending_destroy(
                &mut pending,
                appid.clone(),
                PendingDestroy { generation, cancel },
            );
        }

        let mgr_weak = Arc::downgrade(self);
        std::mem::drop(crate::executor::spawn(async move {
            let sleep = time::sleep(Duration::from_secs(1800));
            tokio::pin!(rx);
            tokio::pin!(sleep);
            tokio::select! {
                _ = &mut sleep => {},
                _ = &mut rx => return,
            }

            if let Some(mgr) = mgr_weak.upgrade() {
                let should_destroy = {
                    let mut pending = mgr
                        .pending_destroy
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    claim_pending_destroy(&mut pending, &appid, generation)
                };
                if should_destroy {
                    info!("Delayed destroy triggered after inactivity").with_appid(appid.clone());
                    mgr.destroy_lxapp(&appid);
                }
            }
        }));
    }

    /// Cancel any pending delayed destroy for the given app.
    pub(crate) fn cancel_delayed_destroy(&self, appid: &str) {
        let mut pending = self
            .pending_destroy
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(entry) = pending.remove(appid) {
            let _ = entry.cancel.send(());
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

    /// Whether an app is anywhere on the navigation stack.
    pub(crate) fn stack_contains(&self, appid: &str) -> bool {
        self.lxapp_stack
            .lock()
            .map(|stack| stack.iter().any(|id| id == appid))
            .unwrap_or(false)
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
    /// Runtime page instances keyed by stable instance id — the single owner
    /// of every live PageInstance (stack pages, pins, isolated surfaces).
    pub(crate) pages_by_id: Mutex<HashMap<String, PageInstance>>,

    /// Path-pinned singleton instances: tab pages and headless services.
    /// These survive off-stack and resolve by path when no stack entry does.
    pub(crate) path_pins: Mutex<HashMap<String, String>>,

    /// Runtime metadata and lifecycle state keyed by page instance id.
    page_instance_runtime: Mutex<HashMap<String, PageInstanceRuntimeRecord>>,

    /// Delayed dispose timers for hidden page instances.
    page_instance_dispose_timers: Mutex<HashMap<String, oneshot::Sender<()>>>,
    /// Pending in-place resets for pages that left the stack, keyed by page
    /// instance id. Cancelled when the page is navigated to again.
    page_reset_timers: Mutex<HashMap<String, oneshot::Sender<()>>>,

    /// PageInstance navigation stack: instance ids, oldest → newest. The
    /// instance id is the page's identity; its path is route metadata read
    /// from the instance itself.
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

    /// Lxapp-scoped appearance and the latest committed Page Chrome revision.
    pub(crate) appearance: LxAppAppearanceState,
    pub(crate) page_chrome_revision: u64,
    pub(crate) page_chrome_layouts: HashMap<String, EffectivePageChromeLayout>,

    /// Startup options for the app
    pub(crate) startup_options: LxAppStartupOptions,

    /// Shell region currently owned by this live lxapp presentation. This is
    /// claimed atomically before platform presentation starts and released only
    /// by a real close; hide/show keeps the claim.
    open_region: Option<LxAppOpenRegion>,

    /// Dynamic page surfaces created by lx.openSurface.
    pub(crate) surfaces: Mutex<SurfaceRecords>,

    /// App-level orientation override (runtime + persisted)
    pub(crate) orientation_override: Option<OrientationConfig>,

    /// App-declared actions surfaced by the host's secondary action affordance.
    more_actions: LxAppMoreActionState,
}

impl LxAppState {
    fn new() -> Self {
        Self {
            pages_by_id: Mutex::new(HashMap::new()),
            path_pins: Mutex::new(HashMap::new()),
            page_instance_runtime: Mutex::new(HashMap::new()),
            page_instance_dispose_timers: Mutex::new(HashMap::new()),
            page_reset_timers: Mutex::new(HashMap::new()),
            page_stack: Mutex::new(VecDeque::with_capacity(PAGE_STACK_MAX)),
            last_active_time: Instant::now(),
            network_security: NetworkSecurity::new(),
            tabbar: None,
            appearance: LxAppAppearanceState::default(),
            page_chrome_revision: 0,
            page_chrome_layouts: HashMap::new(),
            startup_options: LxAppStartupOptions::default(),
            open_region: None,
            surfaces: Mutex::new(SurfaceRecords::new()),
            orientation_override: None,
            more_actions: LxAppMoreActionState::default(),
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
    app_launch_dispatched: AtomicBool,
    pending_restart_request: AtomicBool,
    /// Session being torn down for a restart, or 0. Page instances must not be
    /// (re)created on it; the recreated instance starts fresh at 0.
    restart_closing_session: AtomicU64,

    /// Current runtime session of this app (id + status)
    pub(crate) session: LxAppSession,

    // Mutable state - protected by mutex for fine-grained locking
    pub(crate) state: Mutex<LxAppState>,

    /// Serializes presentation opens so a failed cold open cannot release a
    /// same-region claim that a concurrent reopen has already made live.
    presentation_open_lock: Mutex<()>,

    /// Serializes public appearance/navbar/tabbar mutations per lxapp.
    pub(crate) page_chrome_mutation_lock: tokio::sync::Mutex<()>,

    self_weak: OnceLock<Weak<LxApp>>,

    // Scripts injected as soon as a page document starts loading.
    document_start_scripts: Mutex<Vec<Arc<str>>>,

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
    /// Whether the app is on the runtime navigation stack — i.e. open from
    /// the user's perspective. A hidden (capsule-closed) app keeps an
    /// "opened" session but is not on the stack.
    pub in_stack: bool,
    pub is_home: bool,
    pub current_page: Option<String>,
    pub initial_route: String,
    pub pages_count: usize,
    pub page_entries: Vec<LxAppRuntimePageInfo>,
    pub page_stack: Vec<String>,
    pub tab_bar: Option<LxAppRuntimeTabBarInfo>,
    pub navigation_bar: Option<LxAppRuntimeNavigationBarInfo>,
    pub lxapp_dir: String,
    pub data_dir: String,
    pub cache_dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LxAppRuntimeTabBarInfo {
    pub presentation: TabBarPresentation,
    pub visibility: TabBarVisibilityPreference,
    pub route_visible: bool,
    pub effective_visible: bool,
    pub selected_index: i32,
    pub runtime_style: LxAppRuntimeTabBarStyleInfo,
    pub items: Vec<LxAppRuntimeTabBarItemInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LxAppRuntimeTabBarStyleInfo {
    pub foreground_color: Option<String>,
    pub selected_foreground_color: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LxAppRuntimeNavigationBarInfo {
    pub title: String,
    pub home_button: VisibilityPreference,
    pub home_button_visible: bool,
    pub runtime_style: LxAppRuntimeNavigationBarStyleInfo,
}

#[derive(Debug, Clone, Serialize)]
pub struct LxAppRuntimeNavigationBarStyleInfo {
    pub background_color: Option<String>,
    pub foreground_color: Option<String>,
    pub divider_color: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LxAppRuntimeTabBarItemInfo {
    pub index: usize,
    pub text: Option<String>,
    pub icon_path: Option<String>,
    pub badge: Option<String>,
    pub red_dot: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LxAppRuntimePageInfo {
    pub name: String,
    pub path: String,
}

/// One app-declared action after its icon path has been resolved inside the
/// lxapp sandbox. The callback remains in the Logic context and is addressed by
/// the snapshot generation plus this item's index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LxAppMoreAction {
    pub label: String,
    pub icon_path: String,
}

/// Immutable native-facing snapshot used to build a More action menu/sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LxAppMoreActions {
    pub generation: u64,
    pub items: Vec<LxAppMoreAction>,
}

/// Three host lifecycle actions plus these app actions fill a two-row,
/// five-column capsule menu.
pub const LXAPP_MORE_ACTION_LIMIT: usize = 7;

#[derive(Debug, Default)]
struct LxAppMoreActionState {
    generation: u64,
    items: Vec<LxAppMoreAction>,
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
        self.status
            .compare_exchange(from as u8, to as u8, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}

/// Session helpers and lifecycle utilities for LxApp.
impl LxApp {
    /// Helper to clone Arc<Self> from within methods needing Arc
    pub(crate) fn clone_arc(&self) -> Arc<LxApp> {
        self.self_weak
            .get()
            .and_then(Weak::upgrade)
            .expect("LxApp Arc binding missing")
    }

    pub(crate) fn bind_arc(self: &Arc<Self>) {
        let _ = self.self_weak.set(Arc::downgrade(self));
    }

    pub(crate) fn status(&self) -> LxAppSessionStatus {
        self.session.status()
    }

    pub fn session_id(&self) -> LxAppSessionId {
        self.session.id
    }

    pub fn sync_host_ui(&self) {
        let revision = self.next_page_chrome_revision();
        if let Err(err) = self.runtime.update_navbar_ui(self.appid.clone()) {
            warn!("Failed to update host NavigationBar UI: {}", err).with_appid(self.appid.clone());
        }
        if let Err(err) = self.runtime.update_tabbar_ui(self.appid.clone()) {
            warn!("Failed to update host TabBar UI: {}", err).with_appid(self.appid.clone());
        }
        if let Ok(page) = self.current_page() {
            let appearance = self.appearance_state().resolved;
            let app = self.clone_arc();
            std::mem::drop(crate::executor::spawn(async move {
                if let Err(err) = app
                    .publish_realized_page_chrome(&page, revision, appearance)
                    .await
                {
                    warn!("Failed to publish Page Chrome View snapshot: {}", err)
                        .with_appid(app.appid.clone());
                }
            }));
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

    /// Whether this lxapp's code was supplied by the host build (or its local
    /// development source), rather than installed or updated independently.
    pub fn is_host_bundled(&self) -> bool {
        matches!(
            self.bundle_source,
            LxAppBundleSource::BuiltinAssets | LxAppBundleSource::DevPath { .. }
        )
    }

    /// Whether `lx.process` is actually reachable from this lxapp — the answer
    /// `lx.supports` reports, so the query and the module's presence cannot
    /// disagree. False wherever the feature is not compiled in.
    pub fn process_supported(&self) -> bool {
        #[cfg(feature = "process")]
        {
            self.process_access_enabled()
        }
        #[cfg(not(feature = "process"))]
        {
            false
        }
    }

    #[cfg(feature = "process")]
    pub(crate) fn process_access_enabled(&self) -> bool {
        if !self.is_home_lxapp || !lingxia_app_context::process_enabled() {
            return false;
        }
        let privilege = LxAppSecurityPrivilege::new("process")
            .expect("process is a valid security privilege id");
        self.has_security_privilege(&privilege)
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
        let tab_bar = self.get_tabbar().map(|tabbar| LxAppRuntimeTabBarInfo {
            presentation: tabbar.presentation,
            visibility: tabbar.visibility,
            route_visible: tabbar.route_visible,
            effective_visible: tabbar.is_effectively_visible(),
            selected_index: tabbar.selected_index,
            runtime_style: LxAppRuntimeTabBarStyleInfo {
                foreground_color: tabbar
                    .runtime_style
                    .foreground_color
                    .map(|color| color.to_string()),
                selected_foreground_color: tabbar
                    .runtime_style
                    .selected_foreground_color
                    .map(|color| color.to_string()),
            },
            items: tabbar
                .items
                .into_iter()
                .enumerate()
                .map(|(index, item)| LxAppRuntimeTabBarItemInfo {
                    index,
                    text: item.text,
                    icon_path: item.icon_path,
                    badge: item.badge,
                    red_dot: item.has_red_dot,
                })
                .collect(),
        });
        let navigation_bar = self.peek_current_page_path().map(|path| {
            let state = self.get_navbar_state(&path);
            LxAppRuntimeNavigationBarInfo {
                title: state.title().to_string(),
                home_button: state.home_button,
                home_button_visible: state.home_button_visible(),
                runtime_style: LxAppRuntimeNavigationBarStyleInfo {
                    background_color: state
                        .runtime_style
                        .background_color
                        .map(|color| color.to_string()),
                    foreground_color: state
                        .runtime_style
                        .foreground_color
                        .map(|color| color.to_string()),
                    divider_color: state
                        .runtime_style
                        .divider_color
                        .map(|color| color.to_string()),
                },
            }
        });
        // On the navigation stack = open from the user's perspective. A
        // capsule-closed app keeps its "opened" session (stateful hide) but
        // leaves the stack, so hosts must read `in_stack` — not `status` —
        // for open-app lists.
        let in_stack = crate::lxapp::get_lxapps_manager()
            .map(|manager| manager.stack_contains(&self.appid))
            .unwrap_or(false);
        LxAppRuntimeInfo {
            appid: self.appid.clone(),
            app_name: info.app_name,
            version: info.version,
            release_type: info.release_type,
            session_id: self.session_id(),
            status: self.status_name().to_string(),
            in_stack,
            is_home: self.is_home_lxapp,
            current_page: self.peek_current_page_path(),
            initial_route: self.initial_route(),
            pages_count: page_entries.len(),
            page_entries,
            page_stack: self.get_page_stack_paths(),
            tab_bar,
            navigation_bar,
            lxapp_dir: self.lxapp_dir.to_string_lossy().into_owned(),
            data_dir: self.user_data_dir.to_string_lossy().into_owned(),
            cache_dir: self.user_cache_dir.to_string_lossy().into_owned(),
        }
    }

    /// Atomically replace this lxapp's app-declared More actions.
    pub fn replace_more_actions(&self, generation: u64, mut items: Vec<LxAppMoreAction>) {
        items.truncate(LXAPP_MORE_ACTION_LIMIT);
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.more_actions = LxAppMoreActionState { generation, items };
    }

    /// Clear actions only when the shutting-down Logic context still owns the
    /// current generation. A newer context must not be erased by an older one.
    pub fn clear_more_actions_if_generation(&self, generation: u64) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.more_actions.generation == generation {
            state.more_actions.generation = generation.saturating_add(1);
            state.more_actions.items.clear();
        }
    }

    pub fn more_actions(&self) -> LxAppMoreActions {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        LxAppMoreActions {
            generation: state.more_actions.generation,
            items: state.more_actions.items.clone(),
        }
    }

    pub fn more_actions_json(&self) -> String {
        serde_json::to_string(&self.more_actions())
            .unwrap_or_else(|_| r#"{"generation":0,"items":[]}"#.to_string())
    }

    /// Validate a native selection against the currently displayed generation,
    /// then enqueue its callback on this app's Logic thread.
    pub fn activate_more_action(&self, generation: u64, index: usize) -> bool {
        let valid = {
            let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            state.more_actions.generation == generation && index < state.more_actions.items.len()
        };
        if !valid {
            return false;
        }
        crate::publish_app_event(
            &self.appid,
            &format!("lx.moreActions:{generation}:{index}"),
            None,
        )
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

    /// Whether this lxapp's bundle is managed by the update system. A
    /// dev-served bundle is served live from a local `dist`, so there is no
    /// installed package to check or replace.
    pub(crate) fn is_ota_managed(&self) -> bool {
        !matches!(self.bundle_source, LxAppBundleSource::DevPath { .. })
    }

    pub(crate) fn trigger_home_update_check_once(&self) {
        if !self.is_home_lxapp {
            return;
        }
        if matches!(self.bundle_source, LxAppBundleSource::DevPath { .. }) {
            return;
        }
        if self
            .home_update_check_dispatched
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            UpdateManager::spawn_lxapp_update_check(self.appid.clone(), self.release_type);
        }
    }

    pub(crate) fn has_pending_restart_request(&self) -> bool {
        self.pending_restart_request.load(Ordering::SeqCst)
    }

    /// True while `session_id` is this instance's restart-closing session.
    pub fn is_restart_closing_session(&self, session_id: u64) -> bool {
        session_id != 0 && self.restart_closing_session.load(Ordering::SeqCst) == session_id
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

    /// How long to wait past a back/replace transition before tearing down the
    /// page that left. Long enough for the native container's pop animation to
    /// finish, so the outgoing page is never blanked while it is on screen.
    const PAGE_RESET_DELAY: Duration = Duration::from_millis(500);

    /// Schedules the teardown of a page that just left the stack.
    ///
    /// Leaving a page ends its instance: the next entry must see fresh `data`
    /// and a fresh document, which is what `onLoad` has always promised. The
    /// teardown is deliberately lazy — the Logic service dies and the document
    /// is parked blank, but nothing is rebuilt until an entry asks for it.
    /// Rebuilding speculatively would run page code off-screen (view mount
    /// hooks, native components, media) and queue Logic work for pages nobody
    /// is returning to.
    pub(crate) fn schedule_page_reset(&self, page: &PageInstance) {
        let instance_id = page.instance_id_string();
        self.cancel_page_reset(&instance_id);
        {
            let _transition = page.reset_transition_guard();
            page.mark_reset_pending();
        }

        let (tx, rx) = oneshot::channel();
        if let Ok(state) = self.state.lock() {
            state
                .page_reset_timers
                .lock()
                .unwrap()
                .insert(instance_id.clone(), tx);
        }

        let appid = self.appid.clone();
        std::mem::drop(crate::executor::spawn(async move {
            let sleep = time::sleep(Self::PAGE_RESET_DELAY);
            tokio::pin!(sleep);
            tokio::pin!(rx);
            tokio::select! {
                _ = &mut sleep => {}
                _ = &mut rx => return,
            }

            let Some(app) = crate::lxapp::try_get(&appid) else {
                return;
            };
            app.cancel_page_reset(&instance_id);
            // The instance can be back on the stack already: a re-entry inside
            // the delay window, which `flush_page_reset` will service, or an
            // entry that landed between the pop and its `onLoad`. Either way
            // the reset stays owed and is claimed there, not here.
            if app
                .get_page_stack()
                .iter()
                .any(|entry| entry == &instance_id)
            {
                return;
            }
            // Resolve by id: a same-path sibling may still be live on the
            // stack, and its presence must not shield this instance from its
            // own teardown.
            let Some(page) = app.get_page_by_instance_id_str(&instance_id) else {
                return;
            };
            let _transition = page.reset_transition_guard();
            if page.take_reset_pending() {
                app.teardown_page(&page);
            }
        }));
    }

    /// Cancels a pending reset, reporting whether one was outstanding.
    pub(crate) fn cancel_page_reset(&self, instance_id: &str) -> bool {
        let Ok(state) = self.state.lock() else {
            return false;
        };
        let cancel = state.page_reset_timers.lock().unwrap().remove(instance_id);
        match cancel {
            Some(cancel) => {
                let _ = cancel.send(());
                true
            }
            None => false,
        }
    }

    /// Settles the reset a page owes at the moment an entry lands on it.
    ///
    /// Entering a page again inside the delay window must still give the user
    /// a fresh instance — the deferred teardown is claimed here if the timer
    /// has not run, and the rebuild the teardown left owing is started. The
    /// entry's own `onLoad` is already requested by the caller; the fresh
    /// document's handshake releases it.
    pub(crate) fn flush_page_reset(&self, page: &PageInstance) {
        let _ = self.flush_page_reset_awaited(page);
    }

    /// Like [`Self::flush_page_reset`], but hands back a receiver that
    /// resolves once the rebuilt Logic service is registered — for in-Logic
    /// navigation, which must look the target's service up immediately after
    /// the flush. `None` means no rebuild was owed.
    pub(crate) fn flush_page_reset_awaited(
        &self,
        page: &PageInstance,
    ) -> Option<oneshot::Receiver<Result<(), String>>> {
        let _transition = page.reset_transition_guard();
        if page.take_reset_pending() {
            self.cancel_page_reset(&page.instance_id_string());
            self.teardown_page(page);
        }
        if page.take_reset_awaiting_entry() {
            Some(self.rebuild_page_on_entry(page))
        } else {
            None
        }
    }

    /// Ends the instance that left the stack, keeping its WebView warm.
    ///
    /// Order matters: cancelling bridge work first drops in-flight view calls
    /// (they would otherwise hang to their 15s timeout against a document that
    /// is about to be replaced), parking swaps the document for an inert blank
    /// one, and terminating closes the old service's channels and page event
    /// bus. Nothing is created here — the rebuild belongs to the next entry.
    fn teardown_page(&self, page: &PageInstance) {
        debug!(
            "Tearing down left page (instance {})",
            page.instance_id_string()
        )
        .with_appid(self.appid.clone())
        .with_path(page.path());
        page.prepare_for_service_restart();
        page.park_view();
        if let Err(err) = self.executor.terminate_page_svc(
            self.clone_arc(),
            page.path().to_string(),
            Some(page.instance_id_string()),
        ) {
            warn!(
                "Failed to terminate page service for {}: {}",
                page.path(),
                err
            )
            .with_appid(self.appid.clone());
        }
    }

    /// Rebuilds a torn-down page for the entry now standing on it: a fresh
    /// Logic service first — bound to this instance id — then the document,
    /// so the new
    /// document's handshake finds the new service.
    ///
    /// The returned receiver resolves as soon as the service is registered;
    /// the document reload continues independently.
    fn rebuild_page_on_entry(&self, page: &PageInstance) -> oneshot::Receiver<Result<(), String>> {
        debug!(
            "Rebuilding page for entry (instance {})",
            page.instance_id_string()
        )
        .with_appid(self.appid.clone())
        .with_path(page.path());
        let (done_tx, done_rx) = oneshot::channel::<Result<(), String>>();
        let path = page.path().to_string();
        let (ack_tx, ack_rx) = oneshot::channel::<Result<(), String>>();
        if let Err(err) = self.executor.create_page_svc_with_ack(
            self.clone_arc(),
            path.clone(),
            Some(page.instance_id_string()),
            ack_tx,
        ) {
            warn!("Failed to recreate page service for {}: {}", path, err)
                .with_appid(self.appid.clone());
            let _ = done_tx.send(Err(err.to_string()));
            return done_rx;
        }

        let appid = self.appid.clone();
        let page = page.clone();
        std::mem::drop(crate::executor::spawn(async move {
            match ack_rx.await {
                Ok(Ok(())) => {
                    let _ = done_tx.send(Ok(()));
                    // load_html, not WebView::reload: the document came from
                    // loadHTMLString with a logical base URL, and a reload would
                    // fetch that URL's raw source, losing the bridge config and
                    // nonce.
                    if let Err(err) = page.load_html() {
                        warn!("Failed to reload {} for re-entry: {}", path, err).with_appid(appid);
                    }
                }
                Ok(Err(err)) => {
                    warn!("Page service rebuild failed for {}: {}", path, err).with_appid(appid);
                    let _ = done_tx.send(Err(err));
                }
                Err(_) => {}
            }
        }));
        done_rx
    }

    fn cancel_all_page_resets(&self) {
        if let Ok(state) = self.state.lock() {
            let mut timers = state.page_reset_timers.lock().unwrap();
            for (_id, cancel) in timers.drain() {
                let _ = cancel.send(());
            }
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
        self.cancel_all_page_bridge_work();
        self.clear_transient_files();
        self.cancel_all_page_instance_dispose_timers();
        self.cancel_all_page_resets();
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
        let pages = {
            let state = self.state.lock().unwrap();
            state
                .pages_by_id
                .lock()
                .unwrap()
                .values()
                .cloned()
                .collect::<Vec<_>>()
        };
        let page_webtags = pages.iter().map(|page| page.webtag()).collect::<Vec<_>>();
        let page_instance_ids = pages
            .iter()
            .map(|page| page.instance_id_string())
            .collect::<Vec<_>>();
        crate::view_call::cancel_view_calls_for_page_instances(
            &page_instance_ids,
            "PageInstance removed while waiting for view response",
        );

        // Break PageInstance <-> WebView links early and detach WebViews, then drop pages by clearing the map
        for page in pages {
            page.detach_webview();
        }
        if let Ok(mut state) = self.state.lock() {
            state.pages_by_id.lock().unwrap().clear();
            if let Ok(mut pins) = state.path_pins.lock() {
                pins.clear();
            }
            state.page_instance_runtime.lock().unwrap().clear();
            state.page_chrome_layouts.clear();
        }
        for webtag in &page_webtags {
            destroy_webview(webtag);
        }
        let _ = self.clear_page_stack();
        // Terminate AppService (receiver handles its own state)
        let _ = self.executor.terminate_app_svc(self.clone_arc());
        self.app_launch_dispatched.store(false, Ordering::SeqCst);
        self.clear_open_region();
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
        // A dev-sourced bundle is, by definition, a developer build: it's served
        // live from a local `dist` and is never installed or OTA-updated. Derive
        // the channel from the source so update gating (release-only) and scope
        // keys stay consistent, whatever channel the caller requested.
        let release_type = match bundle_source {
            LxAppBundleSource::DevPath { .. } => ReleaseType::Developer,
            _ => release_type,
        };
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
            app_launch_dispatched: AtomicBool::new(false),
            pending_restart_request: AtomicBool::new(false),
            restart_closing_session: AtomicU64::new(0),
            session,
            state: Mutex::new(LxAppState::new()),
            presentation_open_lock: Mutex::new(()),
            page_chrome_mutation_lock: tokio::sync::Mutex::new(()),
            self_weak: OnceLock::new(),
            document_start_scripts: Mutex::new(Vec::new()),
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
        let mut app = Self::_new(appid, runtime, executor, crate::host_channel());

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

            let manifest_preference = self.config.appearance;
            let saved_preference = lingxia_service::settings::lxapp_appearance(
                &self.runtime.app_data_dir(),
                &self.appid,
            )
            .map_err(|error| LxAppError::IoError(error.to_string()))?
            .and_then(|value| value.parse::<AppearancePreference>().ok());
            let preference = saved_preference.unwrap_or(manifest_preference);
            let resolved = match preference {
                AppearancePreference::Light => ResolvedAppearance::Light,
                AppearancePreference::Dark => ResolvedAppearance::Dark,
                AppearancePreference::Auto => {
                    if self.runtime.host_appearance_dark() {
                        ResolvedAppearance::Dark
                    } else {
                        ResolvedAppearance::Light
                    }
                }
            };
            self.state.lock().unwrap().appearance = LxAppAppearanceState {
                preference,
                resolved,
                revision: 0,
            };
            self.runtime
                .apply_lxapp_appearance(&self.appid, resolved.is_dark())?;
            self.document_start_scripts.lock().unwrap().push(Arc::from(
                page_chrome::bootstrap_script(&EffectivePageChromeLayout::default(), resolved),
            ));

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
    /// Installed resources use logical mapping and prefix validation. Built-in package
    /// resources are materialized into the app cache for native filesystem consumers.
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

        let path_ref = Path::new(path);

        // 2. A network URL is the one wrong answer worth naming. It trips the
        // traversal rule below purely because it contains a colon, and being
        // told a URL is directory traversal sends the caller hunting for a path
        // bug that is not there. Native chrome cannot fetch, so the remedy is
        // always the same and belongs in the message.
        if let Some(scheme) = uri::network_scheme(path) {
            return Err(LxAppError::InvalidParameter(format!(
                "{scheme} URLs are not supported here: download the file first \
                 (for example with lx.downloadFile) and pass the returned lx:// path"
            )));
        }

        // 3. Prevent traversal for relative logical paths on every platform,
        // and catch native parent components in absolute chooser paths.
        if path_ref
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
            || (!path_ref.is_absolute() && uri::has_invalid_segment(path))
        {
            return Err(LxAppError::ResourceNotFound(
                "directory traversal not allowed".to_string(),
            ));
        }

        // 4. Handle Relative path: search in order user data -> user cache -> package
        if !path_ref.is_absolute() && !path.contains(':') {
            let rel = path.trim_start_matches('/');

            // In a simple logical resolve, we prioritize user data for relative paths
            // or we could stick to a specific root. Here we check existence only for
            // relative path "discovery" if we want to maintain the old search behavior,
            // otherwise we default to a specific root.

            // To keep it simple and predictable for "creation", relative paths
            // without lx:// prefix are resolved against the app bundle root by default.
            if matches!(self.bundle_source, LxAppBundleSource::BuiltinAssets) {
                return self.materialize_builtin_resource(rel);
            }
            return Ok(self.lxapp_dir.join(rel));
        }

        // 5. Handle Absolute paths: Must start with one of the trusted roots.
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
                if let Some(target) = resolved_target.as_ref()
                    && let Ok(canonical_root) = std::fs::canonicalize(root)
                {
                    if target.starts_with(&canonical_root) {
                        return Ok(target.to_path_buf());
                    }
                    continue;
                }
                return Ok(path_ref.to_path_buf());
            }
            if let (Some(target), Ok(canonical_root)) =
                (resolved_target.as_ref(), std::fs::canonicalize(root))
                && target.starts_with(&canonical_root)
            {
                return Ok(target.to_path_buf());
            }
        }

        Err(LxAppError::ResourceNotFound(format!(
            "Access denied: {}",
            path
        )))
    }

    fn materialize_builtin_resource(&self, relative: &str) -> Result<PathBuf, LxAppError> {
        let data = self.read_bytes(relative)?;
        let destination = self.user_cache_dir.join("native-resources").join(relative);
        let parent = destination.parent().ok_or_else(|| {
            LxAppError::InvalidParameter(format!(
                "native resource has no parent: {}",
                destination.display()
            ))
        })?;
        fs::create_dir_all(parent).map_err(|err| {
            LxAppError::IoError(format!("failed to create {}: {err}", parent.display()))
        })?;
        fs::write(&destination, data).map_err(|err| {
            LxAppError::IoError(format!("failed to write {}: {err}", destination.display()))
        })?;
        Ok(destination)
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

                if matches!(self.bundle_source, LxAppBundleSource::BuiltinAssets) {
                    self.materialize_builtin_resource(rel)
                } else {
                    Ok(self.lxapp_dir.join(rel))
                }
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

    pub(crate) fn document_start_scripts_snapshot(&self) -> Vec<Arc<str>> {
        self.document_start_scripts
            .lock()
            .map(|scripts| scripts.clone())
            .unwrap_or_default()
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
            .is_domain_allowed_in(domain, crate::is_dev_session())
    }

    /// Check whether this lxapp declares a high-risk security privilege.
    ///
    /// Intended for privileged host APIs such as automation/devtools. Ordinary
    /// host capabilities such as camera/media/location should continue to rely
    /// on the host app and platform permission flow.
    pub fn has_security_privilege(&self, privilege: &LxAppSecurityPrivilege) -> bool {
        self.config.has_security_privilege(privilege)
    }

    /// Resolve a path to its live page instance.
    ///
    /// Identity lives in the instance id; a path is route metadata. The path
    /// resolves to, in order: the topmost stack entry on that route, the
    /// path-pinned singleton (tab pages, headless services), and finally an
    /// off-stack cached instance (a page that left the stack and is parked
    /// for re-entry). Surface-isolated instances never resolve by path.
    pub fn get_page(&self, path: &str) -> Option<PageInstance> {
        let state = self.state.lock().ok()?;
        let pages_by_id = state.pages_by_id.lock().ok()?;

        if let Ok(stack) = state.page_stack.lock() {
            for id in stack.iter().rev() {
                if let Some(page) = pages_by_id.get(id)
                    && page.path() == path
                {
                    return Some(page.clone());
                }
            }
        }

        if let Ok(pins) = state.path_pins.lock()
            && let Some(id) = pins.get(path)
            && let Some(page) = pages_by_id.get(id)
        {
            return Some(page.clone());
        }

        pages_by_id
            .values()
            .filter(|page| !page.is_isolated() && page.path() == path)
            .max_by_key(|page| page.get_last_active_time())
            .cloned()
    }

    /// Whether the route has a live surface-isolated instance. Those never
    /// resolve by bare path, so a path-keyed report naming one is unaddressable
    /// rather than wrong — the owning surface drives that instance through
    /// `notify_page_instance`.
    pub(crate) fn has_isolated_page(&self, path: &str) -> bool {
        let Ok(state) = self.state.lock() else {
            return false;
        };
        let Ok(pages_by_id) = state.pages_by_id.lock() else {
            return false;
        };
        pages_by_id
            .values()
            .any(|page| page.is_isolated() && page.path() == path)
    }

    /// The path's pinned singleton instance, when one is registered.
    pub(crate) fn pinned_page(&self, path: &str) -> Option<PageInstance> {
        let state = self.state.lock().ok()?;
        let id = state.path_pins.lock().ok()?.get(path)?.clone();
        state.pages_by_id.lock().ok()?.get(&id).cloned()
    }

    /// The most recently active off-stack instance on the route — the warm
    /// re-entry candidate. Instances currently on the stack are excluded.
    pub(crate) fn most_recent_off_stack_page(&self, path: &str) -> Option<PageInstance> {
        let state = self.state.lock().ok()?;
        let stack_ids: std::collections::HashSet<String> =
            state.page_stack.lock().ok()?.iter().cloned().collect();
        state
            .pages_by_id
            .lock()
            .ok()?
            .values()
            .filter(|page| {
                !page.is_isolated()
                    && page.path() == path
                    && !stack_ids.contains(&page.instance_id_string())
            })
            .max_by_key(|page| page.get_last_active_time())
            .cloned()
    }

    /// Pin a page instance as the path's singleton (tab pages, headless
    /// services): it stays resolvable by path while off the stack.
    pub(crate) fn pin_page_path(&self, page: &PageInstance) {
        if let Ok(state) = self.state.lock()
            && let Ok(mut pins) = state.path_pins.lock()
        {
            pins.insert(page.path(), page.instance_id_string());
        }
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

    pub(crate) fn cancel_all_page_bridge_work(&self) {
        let pages = {
            let state = self.state.lock().unwrap();
            state
                .pages_by_id
                .lock()
                .unwrap()
                .values()
                .cloned()
                .collect::<Vec<_>>()
        };
        for page in pages {
            page.cancel_bridge_work();
        }
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

    /// Dispatch `App.onLaunch` once for the current worker-backed app instance.
    /// The worker creation and event messages share one FIFO queue, so callers
    /// may invoke this immediately after `ensure_app_service_running`.
    pub fn ensure_app_launch_dispatched(&self) -> Result<(), LxAppError> {
        if self
            .app_launch_dispatched
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(());
        }
        if let Err(error) = self.appservice_notify(AppServiceEvent::OnLaunch, None) {
            self.app_launch_dispatched.store(false, Ordering::SeqCst);
            return Err(error);
        }
        Ok(())
    }

    /// Restart the app service without closing the host surface or recreating
    /// the LxApp instance. Dev runners use this for an in-place lxapp restart:
    /// logic is recreated, while the existing window/frame stays put.
    pub fn restart_app_service_in_place(&self) -> Result<(), LxAppError> {
        self.executor.restart_app_svc(self.clone_arc())?;
        self.app_launch_dispatched.store(false, Ordering::SeqCst);
        // Re-run onLaunch so app-service state (globalData, network init) is
        // rebuilt. onLaunch normally fires only during open(); an in-place
        // restart skips that lifecycle, so fire it explicitly. It is enqueued
        // before the page reload, so the reloaded page's first globalData read
        // (which arrives only after the page re-loads) observes the fresh state.
        self.ensure_app_launch_dispatched()
    }

    /// Reload the current page's WebView in place (without recreating the host
    /// window), so a dev "restart" reloads the page rather than flashing the
    /// screen. Errors when the page stack is empty or its WebView is not ready.
    pub fn reload_current_page(&self) -> Result<(), LxAppError> {
        self.current_page()?
            .webview()
            .ok_or_else(|| LxAppError::WebView("page WebView is not ready".to_string()))?
            .reload()
            .map_err(LxAppError::from)
    }

    /// In-place restart: recreate the JS app service, rebuild the live page
    /// services, then regenerate HTML in their retained WebViews — without closing or
    /// recreating the host surface.
    /// The steps belong together: restarting the app service alone leaves the
    /// page bound to the terminated worker, and reloading without rebuilding the
    /// page services drops the page's bridge messages ("page service not
    /// loaded"). Reloading waits for every new PageSvc acknowledgement so a
    /// fast WebView cannot send its ready handshake before the service exists.
    pub fn restart_in_place(&self) -> Result<(), LxAppError> {
        let pending = self.begin_in_place_restart()?;
        let appid = self.appid.clone();
        std::mem::drop(crate::executor::spawn(async move {
            if let Err(error) = Self::finish_in_place_restart(pending).await {
                error!("Failed to finish in-place lxapp restart: {error}").with_appid(appid);
            }
        }));
        Ok(())
    }

    /// Awaitable form used by devtools, which must not report success before
    /// the retained page has completed the new document load.
    pub async fn restart_in_place_and_wait(&self) -> Result<(), LxAppError> {
        let pending = self.begin_in_place_restart()?;
        Self::finish_in_place_restart(pending).await
    }

    fn begin_in_place_restart(&self) -> Result<Vec<PendingPageServiceRestart>, LxAppError> {
        self.restart_app_service_in_place()?;
        let pages: Vec<PageInstance> = {
            let state = self
                .state
                .lock()
                .map_err(|_| LxAppError::Runtime("lxapp state lock poisoned".to_string()))?;
            let pages_by_id = state
                .pages_by_id
                .lock()
                .map_err(|_| LxAppError::Runtime("page registry lock poisoned".to_string()))?;
            pages_by_id
                .values()
                .filter(|page| !page.is_isolated())
                .cloned()
                .collect()
        };
        let mut pending = Vec::with_capacity(pages.len());
        for page in pages {
            {
                let _transition = page.reset_transition_guard();
                page.prepare_for_service_restart();
            }
            let (ack_tx, ack_rx) = oneshot::channel::<Result<(), String>>();
            self.executor.create_page_svc_with_ack(
                self.clone_arc(),
                page.path().to_string(),
                Some(page.instance_id_string()),
                ack_tx,
            )?;
            pending.push((page, ack_rx));
        }
        Ok(pending)
    }

    async fn finish_in_place_restart(
        pending: Vec<PendingPageServiceRestart>,
    ) -> Result<(), LxAppError> {
        let mut pages = Vec::with_capacity(pending.len());
        for (page, ack_rx) in pending {
            ack_rx
                .await
                .map_err(|_| LxAppError::Runtime("page service restart cancelled".to_string()))?
                .map_err(LxAppError::Runtime)?;
            pages.push(page);
        }
        for page in pages {
            // These pages originate from loadHTMLString + a logical base URL.
            // WebView::reload would request that base URL's raw source and skip
            // generate_page_html, losing the bridge config and nonce.
            page.load_html()?;
        }
        Ok(())
    }

    /// Clears this lxapp's user cache directory, recreating it empty. Dev
    /// runners and the shell "clean cache" action use this before an in-place
    /// restart.
    pub fn clear_user_cache(&self) -> Result<(), LxAppError> {
        if self.user_cache_dir.exists() {
            std::fs::remove_dir_all(&self.user_cache_dir).map_err(|err| {
                LxAppError::IoError(format!(
                    "failed to remove {}: {err}",
                    self.user_cache_dir.display()
                ))
            })?;
        }
        std::fs::create_dir_all(&self.user_cache_dir).map_err(|err| {
            LxAppError::IoError(format!(
                "failed to recreate {}: {err}",
                self.user_cache_dir.display()
            ))
        })
    }

    /// Remove a page instance whose setup failed: it never became usable and
    /// must not stay resolvable.
    fn remove_failed_page(&self, page: &PageInstance) {
        let id = page.instance_id_string();
        if let Ok(state) = self.state.lock() {
            let _ =
                self.executor
                    .terminate_page_svc(self.clone_arc(), page.path(), Some(id.clone()));
            state.pages_by_id.lock().unwrap().remove(id.as_str());
            // A failed setup can land after the entry pushed the instance; a
            // dangling stack slot would wedge current_page() and navigation.
            if let Ok(mut stack) = state.page_stack.lock() {
                stack.retain(|entry| entry != &id);
            }
            if let Ok(mut pins) = state.path_pins.lock() {
                pins.retain(|_, pinned| pinned != &id);
            }
            state
                .page_instance_runtime
                .lock()
                .unwrap()
                .remove(id.as_str());
            if let Some(cancel) = state
                .page_instance_dispose_timers
                .lock()
                .unwrap()
                .remove(id.as_str())
            {
                let _ = cancel.send(());
            }
        }

        page.cancel_bridge_work();
        page.detach_webview();
        destroy_webview(&page.webtag());
    }

    pub fn ensure_headless_page_service(&self, path: &str) -> Result<PageInstance, LxAppError> {
        if let Some(page) = self.get_page(path) {
            return Ok(page);
        }

        let candidate = PageInstance::new_headless(self.appid.clone(), path.to_string(), self);
        // Headless services are path-pinned singletons like tab pages.
        let page = {
            let state = self.state.lock().unwrap();
            let mut pages_by_id = state.pages_by_id.lock().unwrap();
            let existing = pages_by_id
                .values()
                .find(|page| !page.is_isolated() && page.path() == path)
                .cloned();
            if let Some(page) = existing {
                page
            } else {
                pages_by_id.insert(candidate.instance_id_string(), candidate.clone());
                candidate
            }
        };
        self.pin_page_path(&page);

        let (ack_tx, ack_rx) = oneshot::channel::<Result<(), String>>();
        if let Err(err) = self.executor.create_page_svc_with_ack(
            self.clone_arc(),
            path.to_string(),
            Some(page.instance_id_string()),
            ack_tx,
        ) {
            page.mark_webview_ready(Err(err.to_string()));
            self.remove_failed_page(&page);
            return Err(err);
        }

        let page_clone = page.clone();
        let lxapp = self.clone_arc();
        crate::executor::spawn(async move {
            let result = match ack_rx.await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(e),
                Err(err) => Err(err.to_string()),
            };
            if result.is_err() {
                lxapp.remove_failed_page(&page_clone);
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
        let _open_guard = self
            .presentation_open_lock
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let requested_region = LxAppOpenRegion::from(options.open_mode);
        let claimed = self.claim_open_region(requested_region)?;
        let began_opening =
            self.cas_status(LxAppSessionStatus::Closed, LxAppSessionStatus::Opening);
        let result = self.open_claimed(options);
        if result.is_err() {
            if began_opening {
                let _ = self.cas_status(LxAppSessionStatus::Opening, LxAppSessionStatus::Closed);
            }
            if claimed {
                // A platform can fail after it has attached the controller (for
                // example while finalizing a Windows page instance). Roll back the
                // cold presentation before releasing the region claim; otherwise
                // another role could open while the first View is still visible.
                let _ = self.runtime.hide_lxapp(self.appid.clone(), self.session.id);
                self.release_open_region(requested_region);
            }
        }
        result
    }

    fn open_claimed(&self, options: LxAppStartupOptions) -> Result<(), LxAppError> {
        if self.logic_enabled() && !crate::js_appservice_supported() {
            return Err(LxAppError::UnsupportedOperation(
                "this host app was built without JS AppService runtime".to_string(),
            ));
        }

        let mut startup_options = options;

        // Record startup options on this instance
        // Resolve path early so we can keep native/view/AppService consistent.
        let raw_url = startup_options.resolved_url(self)?;
        startup_options.page = None;

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
        self.executor.create_app_svc(self.clone_arc())?;

        // Create native PageInstance + WebView
        let page = self.get_or_create_page(&startup_options.path);
        page.set_query(startup_options.query.clone());

        // Open UI
        let title = self.get_lxapp_info().app_name;
        self.runtime.show_lxapp(
            self.appid.clone(),
            title,
            startup_options.path.clone(),
            page.webtag().key().to_string(),
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
                self.set_active_main();
                self.sync_host_ui();
            }
        }
        Ok(())
    }

    /// Claim the app's one live shell region. Returning `true` means this call
    /// created the claim and therefore owns rollback if platform open fails.
    fn claim_open_region(&self, requested: LxAppOpenRegion) -> Result<bool, LxAppError> {
        let mut state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        match state.open_region {
            None => {
                state.open_region = Some(requested);
                Ok(true)
            }
            Some(current) if current == requested => Ok(false),
            Some(current) => Err(LxAppError::SurfaceConflict(format!(
                "lxapp '{}' is already open as {}; close it before opening as {}",
                self.appid,
                current.as_str(),
                requested.as_str()
            ))),
        }
    }

    fn release_open_region(&self, expected: LxAppOpenRegion) {
        let mut state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        if state.open_region == Some(expected) {
            state.open_region = None;
        }
    }

    pub(crate) fn clear_open_region(&self) {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .open_region = None;
    }

    fn current_open_region(&self) -> Option<LxAppOpenRegion> {
        self.state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .open_region
    }

    /// Host surface id currently used for an aside presentation.
    pub fn open_panel_id(&self) -> Option<String> {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        (state.open_region == Some(LxAppOpenRegion::Aside))
            .then(|| state.startup_options.panel_id.trim().to_string())
            .filter(|panel_id| !panel_id.is_empty())
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

        // Mark the session restart-closing so a premature reopen can't pre-create
        // its pages on the dying worker; the recreated instance starts clean.
        self.restart_closing_session
            .store(from_session, Ordering::SeqCst);

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

/// The shell region an OPEN lxapp currently occupies. One lxapp lives in
/// exactly one region (main or aside); the shell never silently copies or
/// moves an instance between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LxAppOpenRegion {
    Main,
    Aside,
}

impl LxAppOpenRegion {
    /// Stable API spelling used in diagnostics and surface metadata.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Aside => "aside",
        }
    }
}

impl From<lingxia_platform::traits::app_runtime::LxAppOpenMode> for LxAppOpenRegion {
    fn from(mode: lingxia_platform::traits::app_runtime::LxAppOpenMode) -> Self {
        match mode {
            lingxia_platform::traits::app_runtime::LxAppOpenMode::Panel => Self::Aside,
            lingxia_platform::traits::app_runtime::LxAppOpenMode::Normal => Self::Main,
        }
    }
}

/// `None` means the app owns no live shell presentation. Region ownership is
/// independent from visibility: hide/show keeps the claim; close releases it.
pub fn open_region(appid: &str) -> Option<LxAppOpenRegion> {
    let app = runtime_registry::try_get(appid)?;
    app.current_open_region()
}

#[cfg(test)]
mod delayed_destroy_tests {
    use super::*;
    use tokio::sync::oneshot::error::TryRecvError;

    #[test]
    fn first_timer_is_registered_and_replacement_is_cancelled() {
        let mut pending = HashMap::new();
        let (first_cancel, mut first_rx) = oneshot::channel();
        replace_pending_destroy(
            &mut pending,
            "app".to_string(),
            PendingDestroy {
                generation: 1,
                cancel: first_cancel,
            },
        );

        assert_eq!(pending.get("app").map(|entry| entry.generation), Some(1));
        assert!(matches!(first_rx.try_recv(), Err(TryRecvError::Empty)));

        let (second_cancel, mut second_rx) = oneshot::channel();
        replace_pending_destroy(
            &mut pending,
            "app".to_string(),
            PendingDestroy {
                generation: 2,
                cancel: second_cancel,
            },
        );

        assert_eq!(first_rx.try_recv(), Ok(()));
        assert!(matches!(second_rx.try_recv(), Err(TryRecvError::Empty)));
        assert_eq!(pending.get("app").map(|entry| entry.generation), Some(2));
    }

    #[test]
    fn only_current_timer_can_claim_delayed_destroy() {
        let mut pending = HashMap::new();
        let (cancel, _rx) = oneshot::channel();
        replace_pending_destroy(
            &mut pending,
            "app".to_string(),
            PendingDestroy {
                generation: 2,
                cancel,
            },
        );

        assert!(!claim_pending_destroy(&mut pending, "app", 1));
        assert!(pending.contains_key("app"));
        assert!(claim_pending_destroy(&mut pending, "app", 2));
        assert!(!pending.contains_key("app"));
    }

    #[test]
    fn lru_eviction_skips_home_and_stale_entries() {
        let stack = vec![
            "home".to_string(),
            "stale".to_string(),
            "app-b".to_string(),
            "app-c".to_string(),
        ];

        assert_eq!(
            first_evictable_appid(&stack, |appid| matches!(appid, "app-b" | "app-c")),
            Some("app-b".to_string())
        );
        assert_eq!(first_evictable_appid(&stack, |_| false), None);
    }

    #[test]
    fn session_status_compare_exchange_has_one_winner() {
        const CONTENDERS: usize = 16;
        let session = Arc::new(LxAppSession::new());
        session.set_status(LxAppSessionStatus::Opened);
        let barrier = Arc::new(std::sync::Barrier::new(CONTENDERS));

        let handles = (0..CONTENDERS)
            .map(|_| {
                let session = session.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    session.cas_status(LxAppSessionStatus::Opened, LxAppSessionStatus::Restarting)
                })
            })
            .collect::<Vec<_>>();

        let winners = handles
            .into_iter()
            .map(|handle| usize::from(handle.join().unwrap()))
            .sum::<usize>();
        assert_eq!(winners, 1);
        assert_eq!(session.status(), LxAppSessionStatus::Restarting);
    }
}
