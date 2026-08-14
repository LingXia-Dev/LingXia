pub(crate) mod config;
pub(crate) mod definition;
pub(crate) mod runtime;

pub use definition::{register_page_resolver, resolve_page_path};

use crate::bridge::{IncomingMessage, PageBridge};
use crate::lifecycle::PageLifecycleEvent;
use crate::lxapp::{self, LxAppSessionStatus, navbar::NavigationBarState};
use crate::page::config::{OrientationOverride, PageConfig};
use crate::plugin;
use crate::startup::parse_query_string;
use crate::{LxApp, LxAppError, debug, error, info};
use base64::Engine;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use lingxia_log::{LogBuilder, LogLevel as LxLogLevel, LogTag};
use lingxia_platform::traits::app_runtime::{
    AnimationType, AppRuntime, OpenUrlRequest, OpenUrlTarget,
};
use lingxia_webview::runtime::destroy_webview;
use lingxia_webview::{
    LoadDataRequest, LogLevel, NavigationOutcome, NavigationPolicy, NewWindowPolicy, WebTag,
    WebView, WebViewBuilder, WebViewController, WebViewDelegate,
};
use ring::rand::{SecureRandom, SystemRandom};

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::watch;

/// Global scripts injected into every page across all LxApps on page load.
///
/// For per-app scripts, use [`LxApp::add_page_script`] instead.
static GLOBAL_PAGE_SCRIPTS: OnceLock<Mutex<Vec<Arc<str>>>> = OnceLock::new();

/// Register a script to inject on every page load across all LxApps.
///
/// Call at app startup, before any pages are created.
/// For per-app scripts, use [`LxApp::add_page_script`] instead.
pub fn add_global_page_script(js: impl Into<String>) {
    let scripts = GLOBAL_PAGE_SCRIPTS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut guard) = scripts.lock() {
        guard.push(Arc::from(js.into()));
    }
}

pub(crate) fn global_page_scripts_snapshot() -> Vec<Arc<str>> {
    GLOBAL_PAGE_SCRIPTS
        .get()
        .and_then(|m| m.lock().ok())
        .map(|guard| guard.clone())
        .unwrap_or_default()
}

/// Fired at most once per process: the home lxapp delivered its first
/// `OnReady` (first render finished). Hosts dismiss the startup splash
/// overlay on this signal.
static HOME_FIRST_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn notify_home_first_ready_once(appid: &str) {
    if lingxia_app_context::home_app_id() != Some(appid) {
        return;
    }
    if HOME_FIRST_READY.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }

    // Hold the signal until the cover has been up long enough. A page that
    // renders in 200ms would otherwise flash the splash, which reads worse
    // than not having one. The platform-side timeout still caps the wait.
    let remaining = lingxia_app_context::splash_min_duration()
        .saturating_sub(lingxia_app_context::since_startup());
    if remaining.is_zero() {
        signal_home_first_ready();
        return;
    }
    std::mem::drop(crate::executor::spawn(async move {
        tokio::time::sleep(remaining).await;
        signal_home_first_ready();
    }));
}

fn signal_home_first_ready() {
    if let Some(platform) = lxapp::runtime_registry::get_platform() {
        use lingxia_platform::traits::ui::UIUpdate;
        platform.notify_home_first_ready();
    }
}

type WebviewReadyReceiver = Arc<Mutex<watch::Receiver<Option<Result<(), String>>>>>;

const DEFAULT_VIEW_CALL_TIMEOUT: Duration = Duration::from_secs(15);

/// Inner state of a page that can be shared across threads
#[derive(Clone)]
pub(crate) struct PageInstanceInner {
    id: PageInstanceId,
    appid: String,
    path: String,
    webtag: WebTag,

    // Reference to the WebView (optional, set when WebView is ready)
    webview: Arc<Mutex<Option<Arc<WebView>>>>,

    // Time when this page was last active
    last_active_time: Arc<Mutex<Instant>>,

    // state of PageInstance
    state: Arc<Mutex<PageState>>,

    // Per-page bridge nonce (used to validate the View<->Logic wiring)
    bridge_nonce: Arc<Mutex<Option<String>>>,
    bridge: PageBridge,

    // notify when WebView wiring is ready (delegate set & setup ran)
    webview_ready_tx: watch::Sender<Option<Result<(), String>>>,
    webview_ready_rx: WebviewReadyReceiver,

    // Runtime-owned scripts installed at the earliest page-start callback.
    document_start_scripts: Vec<Arc<str>>,

    // Scripts injected on every page load (global + app-level, snapshotted at creation).
    page_scripts: Vec<Arc<str>>,

    // Async notification: bumped on every handle_loaded().
    loaded_tx: watch::Sender<u64>,

    // Canonical attempt-correlation fold over typed navigation events.
    navigation_progress: Arc<std::sync::Mutex<lingxia_webview::NavigationProgress>>,
}

/// A page runs on three independent clocks, and conflating them is what let
/// `onReady` fire against a document that had not re-rendered: a logical entry
/// owes one `onLoad`, the container's visibility owes `onShow`/`onHide`, and
/// each rendered document owes one `onReady`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EntryPhase {
    /// No entry is pending — nothing has been navigated to since the last unload.
    Idle,
    /// An entry is pending; `onLoad` fires once render has started and, for
    /// Logic-enabled pages, the bridge is up.
    LoadOwed,
    /// `onLoad` was delivered for the current entry.
    Loaded,
}

/// Where a page is in the reset it owes after leaving the stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PageReset {
    /// Nothing owed — the ordinary state of a live page.
    None,
    /// The instance ended; its service and document are still the old ones.
    Pending,
    /// Rebuilt for a future entry. The document is fresh but nobody is on the
    /// page, so its bridge handshake must not boot a lifecycle of its own.
    AwaitingEntry,
}

/// What the container wants, and whether `onShow` has caught up with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Visibility {
    Hidden,
    /// Visible to the container, but `onShow` waits for `onLoad` first.
    ShowOwed,
    Shown,
}

#[derive(Clone, Debug)]
pub struct PageState {
    // PageInstance(webview) render status
    render_status: PageRenderStatus,
    // page lifecycle event
    event: Option<PageLifecycleEvent>,
    /// How far the current logical entry has got.
    entry: EntryPhase,
    /// What the container wants, and whether `onShow` has caught up with it.
    visibility: Visibility,
    /// Tracks whether the current WebView document has completed its bridge handshake.
    bridge_ready: bool,
    /// Logic-enabled pages must wait for AppService bridge readiness before onLoad.
    requires_bridge_ready: bool,
    /// Whether the current document has had its `onReady`.
    ready_dispatched: bool,
    /// The reset this page owes after leaving the stack.
    reset: PageReset,
    // Navigation bar state
    pub(crate) navbar_state: NavigationBarState,
    // A malformed page config owns this page's load outcome; it must not
    // silently fall back to chrome defaults.
    config_load_error: Option<String>,
    // Pull-to-refresh enabled flag
    pub(crate) enable_pull_down_refresh: bool,
    // PageInstance orientation overrides
    pub(crate) orientation_override: OrientationOverride,
    // Query parameters
    pub(crate) query: serde_json::Value,
}

/// Automation-facing readiness snapshot for a page instance.
#[derive(Clone, Debug, Serialize)]
pub struct PageAutomationState {
    pub webview_attached: bool,
    pub webview_ready: bool,
    pub webview_error: Option<String>,
    pub bridge_ready: bool,
    pub render_state: &'static str,
    pub lifecycle: &'static str,
    pub ready: bool,
    pub query: Value,
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum PageRenderStatus {
    Unstarted,
    Started,
    Finished,
}

/// Navigation type for page navigation within LxApp
/// This enum defines the different types of navigation actions that can be performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationType {
    /// Launch navigation - open entry page (clear stack)
    Launch = 0,
    /// Forward navigation - push new page
    Forward = 1,
    /// Backward navigation - pop to previous page
    Backward = 2,
    /// Replace navigation - replace current page
    Replace = 3,
    /// Switch tab navigation - switch between tab pages
    SwitchTab = 4,
}

impl NavigationType {
    /// Convert navigation type to an appropriate animation type for platform runtimes
    pub fn to_animation(self) -> AnimationType {
        match self {
            NavigationType::Forward => AnimationType::Forward,
            NavigationType::Backward => AnimationType::Backward,
            _ => AnimationType::None,
        }
    }
}

/// Represents a single page in a mini app
#[derive(Clone)]
pub struct PageInstance {
    // Use Arc to share the inner state across threads
    inner: Arc<PageInstanceInner>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PageInstanceId(String);

pub(crate) enum WebTagInstance {
    PageInstanceId,
}

impl PageInstanceId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parse(raw: impl Into<String>) -> Option<Self> {
        let value = raw.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }
        uuid::Uuid::parse_str(trimmed)
            .ok()
            .map(|id| Self(id.hyphenated().to_string()))
    }
}

impl Default for PageInstanceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PageInstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Options for Rust-side calls into `window.LingXiaBridge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewCallOptions {
    timeout: Duration,
}

impl Default for ViewCallOptions {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_VIEW_CALL_TIMEOUT,
        }
    }
}

impl ViewCallOptions {
    /// Create default call options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the response timeout for this call.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Return the configured timeout.
    pub fn timeout(self) -> Duration {
        self.timeout
    }
}

fn serialize_view_call_params<P>(params: &P) -> Result<Option<Value>, LxAppError>
where
    P: Serialize + ?Sized,
{
    let value = serde_json::to_value(params)?;
    if value.is_null() {
        return Ok(None);
    }
    Ok(Some(value))
}

fn decode_view_call_result<R>(method: &str, value: Value) -> Result<R, LxAppError>
where
    R: DeserializeOwned,
{
    serde_json::from_value(value).map_err(|err| {
        LxAppError::Bridge(format!(
            "Failed to decode view response for '{}': {}",
            method, err
        ))
    })
}

impl PageInstance {
    /// Reconstruct a PageInstance from a shared inner (used by scheme handler closures).
    pub(crate) fn from_inner(inner: Arc<PageInstanceInner>) -> Self {
        Self { inner }
    }

    fn generate_bridge_nonce() -> String {
        let rng = SystemRandom::new();
        let mut bytes = [0u8; 16];
        // If entropy fails (unlikely), fall back to a time-based token.
        if rng.fill(&mut bytes).is_err() {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                .to_le_bytes();
            bytes.copy_from_slice(&nanos[..16]);
        }
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Build PageState from JSON config
    /// PageConfig is the single source of truth for configuration.
    fn build_page_state(lxapp: &lxapp::LxApp, path: &str) -> PageState {
        let (page_config, config_load_error) = if lxapp.logic_enabled() {
            match PageConfig::from_json(lxapp, path) {
                Ok(config) => (config, None),
                Err(error) => {
                    error!("Page config load failed for {}: {}", path, error)
                        .with_appid(lxapp.appid.clone())
                        .with_path(path.to_string());
                    (PageConfig::default(), Some(error.to_string()))
                }
            }
        } else {
            // When logic is disabled, page.json is intentionally ignored.
            // In this mode pages talk directly to Rust without JS/page config.
            (PageConfig::default(), None)
        };
        PageState {
            event: None,
            render_status: PageRenderStatus::Unstarted,
            entry: EntryPhase::Idle,
            visibility: Visibility::Hidden,
            bridge_ready: false,
            requires_bridge_ready: lxapp.logic_enabled(),
            ready_dispatched: false,
            reset: PageReset::None,
            navbar_state: page_config.create_navbar_state(),
            config_load_error,
            enable_pull_down_refresh: page_config.is_pull_down_refresh_enabled(),
            orientation_override: page_config.get_orientation_override(),
            query: serde_json::json!({}),
        }
    }

    /// Create a new page in pending state (WebView creation in progress)
    pub(crate) fn new<F, Fut>(appid: String, path: String, lxapp: &LxApp, setup_callback: F) -> Self
    where
        F: Fn(&PageInstance) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
    {
        // AppRuntime presents and navigates stack pages by app, path, and
        // session. Explicit surface-owned pages opt into per-instance tags.
        Self::new_with_webtag_instance(appid, path, lxapp, None, setup_callback)
    }

    pub(crate) fn new_with_webtag_instance<F, Fut>(
        appid: String,
        path: String,
        lxapp: &LxApp,
        webtag_instance: Option<WebTagInstance>,
        setup_callback: F,
    ) -> Self
    where
        F: Fn(&PageInstance) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
    {
        // Build page state from LxApp configuration
        let page_state = Self::build_page_state(lxapp, &path);
        let id = PageInstanceId::new();
        let webtag = webtag_instance
            .as_ref()
            .map(|instance| {
                let instance_id = match instance {
                    WebTagInstance::PageInstanceId => id.as_str(),
                };
                WebTag::new(
                    &appid,
                    &format!("{path}#{instance_id}"),
                    Some(lxapp.session.id),
                )
            })
            .unwrap_or_else(|| WebTag::new(&appid, &path, Some(lxapp.session.id)));
        let bridge_nonce = Self::generate_bridge_nonce();
        let lxapp_arc = lxapp.clone_arc();
        let (ready_tx, ready_rx) = watch::channel(None);
        let (loaded_tx, _) = watch::channel(0u64);
        let inner = Arc::new(PageInstanceInner {
            navigation_progress: Arc::new(std::sync::Mutex::new(Default::default())),
            id,
            appid: appid.clone(),
            path: path.clone(),
            webtag: webtag.clone(),
            last_active_time: Arc::new(Mutex::new(Instant::now())),
            state: Arc::new(Mutex::new(page_state)),
            webview: Arc::new(Mutex::new(None)),
            bridge_nonce: Arc::new(Mutex::new(Some(bridge_nonce))),
            bridge: PageBridge::new(lxapp_arc.clone(), lxapp_arc.executor.clone()),
            webview_ready_tx: ready_tx,
            webview_ready_rx: Arc::new(Mutex::new(ready_rx)),
            document_start_scripts: lxapp.document_start_scripts_snapshot(),
            page_scripts: lxapp.page_scripts_snapshot(),
            loaded_tx,
        });

        // Capture weak ref before moving inner into page
        let page_weak_for_lx = Arc::downgrade(&inner);

        let page = Self { inner };

        // Initiate WebView creation with scheme handlers
        // Register closure-based scheme handlers so lingxia-webview
        // doesn't need to know about lxapp business logic.
        // Captures for navigation handler (no PageInstanceInner ref → no circular ref)
        let runtime_for_nav = lxapp.runtime.clone();
        let appid_for_nav = appid.clone();
        let session_id_for_nav = lxapp.session_id();

        // Captures for new-window handler
        let runtime_for_new_window = lxapp.runtime.clone();
        let appid_for_new_window = appid.clone();
        let session_id_for_new_window = lxapp.session_id();

        let session = WebViewBuilder::strict(webtag)
            .delegate(Arc::new(page.clone()))
            .on_scheme("lx", move |req| {
                let page_weak_for_lx = page_weak_for_lx.clone();
                async move {
                    let Some(inner) = page_weak_for_lx.upgrade() else {
                        return None.into();
                    };
                    let page = PageInstance::from_inner(inner);
                    let lxapp = page.owning_lxapp();
                    if lxapp.status() == LxAppSessionStatus::Closed {
                        return None.into();
                    }
                    lxapp.handle_lingxia_request(&page, req).into()
                }
            })
            .on_navigation(move |request| {
                let url = request.url.as_str();
                let scheme = url.split(':').next().unwrap_or("");
                match scheme {
                    // lx:// pages and inline content are always allowed
                    "lx" | "data" | "blob" => NavigationPolicy::Allow,
                    _ => {
                        // Strict mode: https/http/about and external schemes (tel:, mailto:, etc.)
                        // must go through openURL so the host app controls navigation.
                        // about: is silently cancelled (no legitimate use in strict lxapp pages).
                        if scheme != "about" {
                            let _ = runtime_for_nav.open_url(OpenUrlRequest {
                                owner_appid: appid_for_nav.clone(),
                                owner_session_id: session_id_for_nav,
                                url: url.to_string(),
                                target: OpenUrlTarget::External,
                            });
                        }
                        NavigationPolicy::Cancel
                    }
                }
            })
            .on_new_window(move |url| {
                let _ = runtime_for_new_window.open_url(OpenUrlRequest {
                    owner_appid: appid_for_new_window.clone(),
                    owner_session_id: session_id_for_new_window,
                    url: url.to_string(),
                    target: OpenUrlTarget::SelfTarget,
                });
                NewWindowPolicy::Cancel
            })
            .create();

        // Spawn task to wait for WebView creation completion
        // Keep a strong reference to ensure page stays alive during WebView creation
        let page_for_task = page.clone();
        let appid_clone = appid.clone();
        let path_clone = path.clone();

        crate::executor::spawn(async move {
            match session.wait_ready().await {
                Ok(webview_controller) => {
                    // First attach WebView to page
                    page_for_task.attach_webview(webview_controller.clone());

                    // Call setup callback - let external code handle the rest
                    let result = setup_callback(&page_for_task).await;

                    // Mark ready after setup completes so waiters are released only once page is usable.
                    page_for_task.mark_webview_ready(result);
                }
                Err(e) => {
                    if page_for_task.is_unloaded() {
                        info!("Cancelled WebView creation for unloaded page: {}", e)
                            .with_appid(appid_clone)
                            .with_path(path_clone);
                    } else {
                        error!("Failed to create WebView: {}", e)
                            .with_appid(appid_clone)
                            .with_path(path_clone);
                    }
                    page_for_task.mark_webview_ready(Err(e.to_string()));
                }
            }
        });

        page
    }

    /// Create a headless page (nonce allocated, no WebView created).
    ///
    /// Used for a shared logical page that can be attached to externally
    /// managed WebViews one at a time.
    pub(crate) fn new_headless(appid: String, path: String, lxapp: &LxApp) -> Self {
        let page_state = Self::build_page_state(lxapp, &path);
        let id = PageInstanceId::new();
        let bridge_nonce = Self::generate_bridge_nonce();
        let webtag = WebTag::new(&appid, &path, Some(lxapp.session.id));
        let lxapp_arc = lxapp.clone_arc();
        let (ready_tx, ready_rx) = watch::channel(None);
        let (loaded_tx, _) = watch::channel(0u64);
        let inner = Arc::new(PageInstanceInner {
            navigation_progress: Arc::new(std::sync::Mutex::new(Default::default())),
            id,
            appid,
            path,
            webtag,
            last_active_time: Arc::new(Mutex::new(Instant::now())),
            state: Arc::new(Mutex::new(page_state)),
            webview: Arc::new(Mutex::new(None)),
            bridge_nonce: Arc::new(Mutex::new(Some(bridge_nonce))),
            bridge: PageBridge::new(lxapp_arc.clone(), lxapp_arc.executor.clone()),
            webview_ready_tx: ready_tx,
            webview_ready_rx: Arc::new(Mutex::new(ready_rx)),
            document_start_scripts: lxapp.document_start_scripts_snapshot(),
            page_scripts: lxapp.page_scripts_snapshot(),
            loaded_tx,
        });
        Self { inner }
    }

    pub fn bridge_nonce(&self) -> Option<String> {
        self.inner.bridge_nonce.lock().ok().and_then(|v| v.clone())
    }

    pub fn instance_id(&self) -> PageInstanceId {
        self.inner.id.clone()
    }

    pub fn instance_id_string(&self) -> String {
        self.inner.id.to_string()
    }

    /// The webview tag identifying this page instance's view (also the
    /// page key passed to [`crate::NativeComponentHost::on_page_destroyed`]).
    pub fn webtag(&self) -> WebTag {
        self.inner.webtag.clone()
    }

    pub(crate) fn bridge(&self) -> PageBridge {
        self.inner.bridge.clone()
    }

    fn owning_lxapp(&self) -> Arc<LxApp> {
        self.inner.bridge.lxapp()
    }

    pub(crate) fn cancel_bridge_work(&self) {
        self.inner.bridge.cancel_page_work(self);
    }

    /// Records that this instance ended and owes a reset.
    pub(crate) fn mark_reset_pending(&self) {
        if let Ok(mut state) = self.inner.state.lock() {
            state.reset = PageReset::Pending;
        }
    }

    /// Claims the pending reset, if this page still owes one.
    pub(crate) fn take_reset_pending(&self) -> bool {
        let Ok(mut state) = self.inner.state.lock() else {
            return false;
        };
        if state.reset == PageReset::Pending {
            state.reset = PageReset::AwaitingEntry;
            return true;
        }
        false
    }

    pub(crate) fn is_reset_pending(&self) -> bool {
        self.inner
            .state
            .lock()
            .is_ok_and(|state| state.reset == PageReset::Pending)
    }

    /// Prepare a retained WebView for a freshly-created PageSvc.
    ///
    /// An in-place app-service restart keeps the native page instance, so the
    /// normal attach path cannot clear the old worker's bridge/lifecycle state.
    pub(crate) fn prepare_for_service_restart(&self) {
        self.cancel_bridge_work();
        if let Ok(mut state) = self.inner.state.lock() {
            Self::reset_webview_lifecycle_state(&mut state);
        }
    }

    /// Attach WebView to this page (called when WebView is ready)
    pub fn attach_webview(&self, webview: Arc<WebView>) {
        let mut should_reset_lifecycle = false;
        if let Ok(mut webview_guard) = self.inner.webview.lock() {
            let current_webview_is_same = webview_guard
                .as_ref()
                .map(|current| Arc::ptr_eq(current, &webview));
            should_reset_lifecycle =
                Self::should_reset_lifecycle_on_attach(current_webview_is_same);
            *webview_guard = Some(webview);
        }
        if should_reset_lifecycle && let Ok(mut state) = self.inner.state.lock() {
            Self::reset_webview_lifecycle_state(&mut state);
        }
    }

    pub fn handle_incoming_message_json(&self, msg: &str) -> Result<(), LxAppError> {
        let incoming = IncomingMessage::from_json_str(msg)
            .map_err(|err| LxAppError::Bridge(format!("Invalid bridge message JSON: {}", err)))?;
        self.inner.bridge.handle_incoming(self, Arc::new(incoming))
    }

    /// Get complete page state
    pub fn get_page_state(&self) -> Option<PageState> {
        self.inner.state.lock().ok().map(|state| state.clone())
    }

    // Only the entry clock rewinds here. `onReady` belongs to the rendered
    // document, so a cached instance that re-enters without a document reload
    // has nothing new to be ready about.
    fn request_on_load(state: &mut PageState) {
        state.entry = EntryPhase::LoadOwed;
        state.reset = PageReset::None;
        if state.visibility == Visibility::Shown {
            state.visibility = Visibility::ShowOwed;
        }
    }

    fn should_reset_lifecycle_on_attach(current_webview_is_same: Option<bool>) -> bool {
        matches!(current_webview_is_same, Some(false))
    }

    fn reset_webview_lifecycle_state(state: &mut PageState) {
        let is_currently_visible = state.event == Some(PageLifecycleEvent::OnShow);
        state.render_status = PageRenderStatus::Unstarted;
        state.visibility = if is_currently_visible {
            Visibility::ShowOwed
        } else {
            Visibility::Hidden
        };
        state.entry = EntryPhase::Idle;
        state.bridge_ready = false;
        state.ready_dispatched = false;
    }

    fn collect_ready_lifecycle_events(
        state: &mut PageState,
        events_to_fire: &mut Vec<(PageLifecycleEvent, Option<String>)>,
    ) {
        if state.entry == EntryPhase::LoadOwed
            && (!state.requires_bridge_ready || state.bridge_ready)
            && !matches!(state.render_status, PageRenderStatus::Unstarted)
        {
            let query = serde_json::to_string(&state.query).ok();
            events_to_fire.push((PageLifecycleEvent::OnLoad, query));
            state.entry = EntryPhase::Loaded;
        }

        // Weixin ordering for the first visible load is Load -> Show -> Ready.
        if state.entry == EntryPhase::Loaded && state.visibility == Visibility::ShowOwed {
            events_to_fire.push((PageLifecycleEvent::OnShow, None));
            state.visibility = Visibility::Shown;
            state.event = Some(PageLifecycleEvent::OnShow);
        }

        if state.entry == EntryPhase::Loaded
            && state.render_status == PageRenderStatus::Finished
            && !state.ready_dispatched
        {
            events_to_fire.push((PageLifecycleEvent::OnReady, None));
            state.ready_dispatched = true;
        }
    }

    fn collect_hidden_or_unloaded_lifecycle_event(
        state: &mut PageState,
        event: PageLifecycleEvent,
        events_to_fire: &mut Vec<(PageLifecycleEvent, Option<String>)>,
    ) {
        debug_assert!(matches!(
            event,
            PageLifecycleEvent::OnHide | PageLifecycleEvent::OnUnload
        ));

        state.visibility = Visibility::Hidden;
        if event == PageLifecycleEvent::OnUnload && state.entry == EntryPhase::LoadOwed {
            // Cancel an entry that never got its `onLoad`. One already
            // delivered stays delivered: the PageInstance can be reused after
            // `onUnload` without a fresh bridge handshake, and only real
            // document teardown rewinds the entry through reset.
            state.entry = EntryPhase::Idle;
        }
        if state.event != Some(event) {
            events_to_fire.push((event, None));
            state.event = Some(event);
        }
    }

    fn lifecycle_cancels_bridge_work(event: PageLifecycleEvent) -> bool {
        event == PageLifecycleEvent::OnUnload
    }

    fn fire_lifecycle_events(&self, events_to_fire: Vec<(PageLifecycleEvent, Option<String>)>) {
        if events_to_fire.is_empty() {
            return;
        }

        let lxapp = self.owning_lxapp();
        let appid = self.appid();
        let path = self.path();

        if events_to_fire
            .iter()
            .any(|(event, _)| *event == PageLifecycleEvent::OnReady)
        {
            notify_home_first_ready_once(&appid);
        }

        for (event, query) in events_to_fire {
            // Keep the in-process native-component host in sync with
            // the page lifecycle: hidden AND unloaded pages hide their
            // overlays and pause playback (an unloaded page's webview
            // may stay cached and be revived by a later navigation, so
            // its components only pause here — they are torn down with
            // the page instance through `on_page_destroyed` when it is
            // disposed). Mirrors the platform managers' inactive/
            // active/destroyed handling.
            match event {
                PageLifecycleEvent::OnShow => {
                    crate::native_component::notify_page_visibility(self.webtag().key(), true);
                }
                PageLifecycleEvent::OnHide | PageLifecycleEvent::OnUnload => {
                    crate::native_component::notify_page_visibility(self.webtag().key(), false);
                }
                _ => {}
            }

            if let Err(e) = lxapp.executor.call_page_service_event(
                lxapp.clone(),
                path.clone(),
                Some(self.instance_id_string()),
                event,
                query,
            ) {
                error!("Failed to call {}: {}", String::from(event), e)
                    .with_appid(appid.clone())
                    .with_path(path.clone());
            }
        }
    }

    pub(crate) fn notify_bridge_ready(&self) {
        let mut events_to_fire: Vec<(PageLifecycleEvent, Option<String>)> = Vec::new();
        {
            let mut state = self.inner.state.lock().unwrap();
            if state.bridge_ready {
                return;
            }

            state.bridge_ready = true;
            // A page whose document was rebuilt for a future entry has no
            // entry to serve: booting here would deliver `onLoad` (with the
            // previous entry's query) and `onReady` to nobody, and leave the
            // real entry with a second `onLoad` and no `onReady`. Pages that
            // handshake while live — surfaces, an app-service restart — still
            // boot from here.
            if state.entry == EntryPhase::Idle && state.reset == PageReset::None {
                Self::request_on_load(&mut state);
            }
            Self::collect_ready_lifecycle_events(&mut state, &mut events_to_fire);
        }
        self.fire_lifecycle_events(events_to_fire);
    }

    fn notify_render_started_inner(&self) {
        let mut events_to_fire: Vec<(PageLifecycleEvent, Option<String>)> = Vec::new();
        {
            let mut state = self.inner.state.lock().unwrap();
            state.render_status = PageRenderStatus::Started;
            Self::collect_ready_lifecycle_events(&mut state, &mut events_to_fire);
        }
        self.fire_lifecycle_events(events_to_fire);
    }

    fn notify_render_finished_after_scripts(&self) {
        let mut events_to_fire: Vec<(PageLifecycleEvent, Option<String>)> = Vec::new();
        {
            let mut state = self.inner.state.lock().unwrap();
            state.render_status = PageRenderStatus::Finished;
            Self::collect_ready_lifecycle_events(&mut state, &mut events_to_fire);
        }
        self.fire_lifecycle_events(events_to_fire);
    }

    pub(crate) fn dispatch_lifecycle_event(&self, event: PageLifecycleEvent) {
        // Central lifecycle state machine for a single WebView-backed PageInstance.
        // Sources of events:
        // - First-time creation: WebView/LXPort ready requests onLoad (AppService side)
        // - Re-navigation with new query (navigateTo): native manually requests onLoad
        // - Render completion: WebView delegate triggers onReady after page scripts are injected
        // - Visibility changes: native triggers onShow/onHide
        // Goals (Weixin semantics adapted to a single WebView instance):
        // - onLoad carries query and may occur multiple times across logical navigations
        //   (first-time + each navigateTo with new params)
        // - onReady fires once for each logical navigation after render has finished
        // - onShow fires each time the page becomes visible (after a hide), without query

        if Self::lifecycle_cancels_bridge_work(event) {
            self.cancel_bridge_work();
        }

        // An entry never inherits the instance that left: if this page was
        // popped and its deferred reset has not run yet, complete it here,
        // before the entry's own onLoad is queued against the fresh state.
        if event == PageLifecycleEvent::OnLoad {
            self.owning_lxapp().flush_page_reset(self);
        }

        // A collection of events to fire after the lock is released.
        let mut events_to_fire: Vec<(PageLifecycleEvent, Option<String>)> = Vec::new();

        // acquire lock, update state, determine events to fire
        // The lock must be released before calling the executor to avoid deadlocks,
        // in case the JS code calls back into Rust and needs to access page state.
        {
            let mut state = self.inner.state.lock().unwrap();

            // OnPullDownRefresh is a simple event that fires immediately without state tracking
            if event == PageLifecycleEvent::OnPullDownRefresh {
                events_to_fire.push((event, None));
            }
            // OnHide and OnUnload are handled exclusively and do not trigger the main event cascade.
            else if event == PageLifecycleEvent::OnHide || event == PageLifecycleEvent::OnUnload {
                Self::collect_hidden_or_unloaded_lifecycle_event(
                    &mut state,
                    event,
                    &mut events_to_fire,
                );
            } else {
                // This logic handles the Load -> Show -> Ready cascade.

                // Update raw status based on the incoming event.
                if event == PageLifecycleEvent::OnShow && state.visibility != Visibility::Shown {
                    state.visibility = Visibility::ShowOwed;
                }

                if event == PageLifecycleEvent::OnLoad {
                    Self::request_on_load(&mut state);
                }

                Self::collect_ready_lifecycle_events(&mut state, &mut events_to_fire);
            }
        }

        //  Fire the collected events outside of the lock to prevent deadlocks.
        self.fire_lifecycle_events(events_to_fire);
    }

    /// Get navbar state (read-only)
    pub fn get_navbar_state(&self) -> Option<NavigationBarState> {
        self.inner
            .state
            .lock()
            .ok()
            .map(|state| state.navbar_state.clone())
    }

    /// Get navbar state with mutable access (internal use)
    pub fn get_navbar_state_mut<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut NavigationBarState) -> R,
    {
        self.inner
            .state
            .lock()
            .ok()
            .map(|mut state| f(&mut state.navbar_state))
    }

    /// Get page orientation overrides from state
    pub fn get_orientation_override(&self) -> Option<OrientationOverride> {
        self.inner
            .state
            .lock()
            .ok()
            .map(|state| state.orientation_override)
    }

    /// Get WebView if available
    pub fn webview(&self) -> Option<Arc<WebView>> {
        if let Ok(webview_guard) = self.inner.webview.lock() {
            webview_guard.clone()
        } else {
            None
        }
    }

    /// Return a non-blocking readiness snapshot suitable for devtools.
    pub fn automation_state(&self) -> PageAutomationState {
        let webview_attached = self.webview().is_some();
        let webview_result = self
            .inner
            .webview_ready_rx
            .lock()
            .ok()
            .and_then(|rx| rx.borrow().clone());
        let (webview_ready, webview_error) = match webview_result {
            Some(Ok(())) => (true, None),
            Some(Err(error)) => (false, Some(error)),
            None => (false, None),
        };
        let state = self.inner.state.lock().ok();
        let bridge_ready = state.as_ref().is_some_and(|state| state.bridge_ready);
        let render_state = match state.as_ref().map(|state| state.render_status) {
            Some(PageRenderStatus::Started) => "loading",
            Some(PageRenderStatus::Finished) => "finished",
            _ => "unstarted",
        };
        let lifecycle = state
            .as_ref()
            .and_then(|state| state.event.map(|event| event.as_str()))
            .unwrap_or("unknown");
        let ready = state.as_ref().is_some_and(|state| state.ready_dispatched);
        let query = state
            .as_ref()
            .map(|state| state.query.clone())
            .unwrap_or_else(|| serde_json::json!({}));
        PageAutomationState {
            webview_attached,
            webview_ready,
            webview_error,
            bridge_ready,
            render_state,
            lifecycle,
            ready,
            query,
        }
    }

    pub(crate) fn is_unloaded(&self) -> bool {
        self.inner
            .state
            .lock()
            .is_ok_and(|state| state.event == Some(PageLifecycleEvent::OnUnload))
    }

    pub(crate) fn mark_webview_ready(&self, result: Result<(), String>) {
        // Ignore errors; receiver will handle missing updates.
        let _ = self.inner.webview_ready_tx.send(Some(result));
    }

    /// Notify that the page's WebView started loading (mirrors WebViewDelegate::on_page_started).
    /// Used by external delegates to forward events to a shared page.
    pub fn notify_page_started(&self) {
        if !self.inner.document_start_scripts.is_empty()
            && let Some(webview) = self.webview()
        {
            for js in &self.inner.document_start_scripts {
                if let Err(error) = webview.exec_js(js) {
                    crate::error!("document-start script injection failed: {}", error)
                        .with_appid(self.inner.appid.clone())
                        .with_path(self.inner.path.clone());
                }
            }
        }
        self.notify_render_started_inner();
    }

    pub async fn wait_webview_ready(&self) -> Result<(), String> {
        let rx = {
            // Clone receiver so concurrent waiters don't block each other.
            self.inner
                .webview_ready_rx
                .lock()
                .map(|r| r.clone())
                .map_err(|_| "webview ready receiver poisoned".to_string())?
        };

        // Fast-path: already has a value.
        if let Some(res) = rx.borrow().clone() {
            return res;
        }

        let mut rx = rx;
        while rx.changed().await.is_ok() {
            if let Some(res) = rx.borrow().clone() {
                return res;
            }
        }

        Err("webview ready channel closed before result".to_string())
    }

    async fn handle_loaded_async(&self) {
        if !self.inner.page_scripts.is_empty()
            && let Some(webview) = self.webview()
        {
            for js in &self.inner.page_scripts {
                if let Err(e) = webview.exec_js(js) {
                    crate::error!("page script injection failed: {}", e)
                        .with_appid(self.inner.appid.clone())
                        .with_path(self.inner.path.clone());
                }
            }
        }

        self.notify_render_finished_after_scripts();
        self.inner.loaded_tx.send_modify(|v| *v = v.wrapping_add(1));
    }

    /// Unified page-loaded handler. Call from any delegate (lxapp or external)
    /// when the WebView finishes a navigation.
    ///
    /// Script injection is awaited before `OnReady` and loaded notifications.
    pub fn handle_loaded(&self) {
        let page = self.clone();
        std::mem::drop(crate::executor::spawn(async move {
            page.handle_loaded_async().await;
        }));
    }

    /// Subscribe to page-loaded events.
    ///
    /// The receiver is notified each time `handle_loaded` completes
    /// (scripts are already injected at that point).
    pub fn subscribe_loaded(&self) -> watch::Receiver<u64> {
        self.inner.loaded_tx.subscribe()
    }

    /// Detach and drop the WebView held by this page.
    /// This breaks PageInstance -> WebView strong reference and triggers platform Drop when
    /// combined with registry removal.
    pub fn detach_webview(&self) {
        self.cancel_bridge_work();
        if let Ok(mut webview_guard) = self.inner.webview.lock() {
            // Drop the Arc by taking it out
            let _ = webview_guard.take();
        }
        if let Ok(mut state) = self.inner.state.lock() {
            Self::reset_webview_lifecycle_state(&mut state);
        }
    }

    /// Get the WebView controller for this page (returns None if not ready)
    pub(crate) fn webview_controller(&self) -> Option<Arc<dyn WebViewController>> {
        if let Some(webview) = self.webview() {
            Some(webview as Arc<dyn WebViewController>)
        } else {
            None
        }
    }

    /// Load HTML content into this page's WebView
    pub(crate) fn load_html(&self) -> Result<(), LxAppError> {
        let lxapp = self.owning_lxapp();
        let path = self.path();
        let config_load_error = self
            .inner
            .state
            .lock()
            .ok()
            .and_then(|state| state.config_load_error.clone());
        let html_string = if let Some(message) = config_load_error {
            lingxia_webview::render_load_error_page(lingxia_webview::LoadErrorPage {
                title: "Page configuration error",
                message: &message,
                retry_label: "Retry",
                retry_url: &self.base_url(),
            })
        } else {
            String::from_utf8_lossy(
                &lxapp.generate_page_html(&path, self.bridge_nonce().as_deref()),
            )
            .into_owned()
        };
        let base_url = self.base_url();

        if let Some(controller) = self.webview_controller() {
            controller
                .load_data(LoadDataRequest::new(&html_string, &base_url))
                .map_err(|e| LxAppError::WebView(e.to_string()))
        } else {
            Err(LxAppError::WebView("WebView not ready".to_string()))
        }
    }

    /// Returns the appid of this page
    pub fn path(&self) -> String {
        self.inner.path.clone()
    }

    /// Returns the appid of this page
    pub fn appid(&self) -> String {
        self.inner.appid.clone()
    }

    /// Returns the base URL used when loading this page's HTML.
    pub fn base_url(&self) -> String {
        if let Some((plugin_name, page_path)) = plugin::parse_plugin_page_path(&self.path()) {
            if page_path.is_empty() {
                return format!("lx://plugin/{}", plugin_name);
            }
            return format!("lx://plugin/{}/{}", plugin_name, page_path);
        }
        format!("lx://lxapp/{}/{}", self.appid(), self.path())
    }

    /// Update the last active time to now
    pub(crate) fn mark_active(&self) {
        if let Ok(mut time) = self.inner.last_active_time.lock() {
            *time = Instant::now();
        }
    }

    /// Get the last active time for LRU eviction
    pub(crate) fn get_last_active_time(&self) -> Option<Instant> {
        self.inner.last_active_time.lock().ok().map(|time| *time)
    }

    /// Check if pull-to-refresh is enabled for this page
    pub fn is_pull_down_refresh_enabled(&self) -> bool {
        self.inner
            .state
            .lock()
            .ok()
            .map(|state| state.enable_pull_down_refresh)
            .unwrap_or(false)
    }

    /// Check if this page is a TabBar page
    pub fn is_tabbar_page(&self) -> bool {
        let lxapp = self.owning_lxapp();
        match lxapp.get_tabbar() {
            Some(tab_bar) => tab_bar.is_tabbar_page(&self.inner.path),
            None => false,
        }
    }

    pub fn navigate_to(
        &self,
        target_page: PageInstance,
        nav_type: NavigationType,
    ) -> Result<PageInstance, LxAppError> {
        let lxapp = self.owning_lxapp();

        // Normalize through LxApp to ensure consistent canonical paths (e.g. plugin routes).
        let target_url =
            crate::append_page_query(target_page.path(), &target_page.automation_state().query)
                .map_err(LxAppError::InvalidParameter)?;
        let target_page = lxapp.get_or_create_page(&target_url);
        self.navigate_to_internal(target_page, nav_type, &lxapp)
    }

    /// Internal navigation logic shared by regular and plugin navigation
    fn navigate_to_internal(
        &self,
        target_page: PageInstance,
        nav_type: NavigationType,
        lxapp: &Arc<LxApp>,
    ) -> Result<PageInstance, LxAppError> {
        let path = target_page.path();
        let target_url =
            crate::append_page_query(path.clone(), &target_page.automation_state().query)
                .map_err(LxAppError::InvalidParameter)?;
        let mut target_page = target_page;
        let is_tabbar_page = lxapp
            .get_tabbar()
            .is_some_and(|tabbar| tabbar.is_tabbar_page(&path));
        let is_tab_switch = nav_type == NavigationType::SwitchTab
            || (nav_type == NavigationType::Launch && is_tabbar_page);
        let is_initial_route = path == lxapp.config.get_initial_route();

        // 2. Handle page stack modifications
        match nav_type {
            NavigationType::Launch | NavigationType::SwitchTab => {
                if nav_type == NavigationType::Launch {
                    let stack_paths = lxapp.get_page_stack();
                    for stack_path in &stack_paths {
                        if let Some(page) = lxapp.get_page(stack_path) {
                            page.dispatch_lifecycle_event(PageLifecycleEvent::OnUnload);
                            page.detach_webview();
                        }
                        destroy_webview(&WebTag::new(
                            &lxapp.appid,
                            stack_path,
                            Some(lxapp.session.id),
                        ));
                    }
                    lxapp.remove_pages(&stack_paths);
                    target_page = lxapp.get_or_create_page(&target_url);
                }
                lxapp.clear_page_stack()?;
            }
            NavigationType::Replace => {
                // Replacing drops the current entry, so the target can only
                // collide with what remains below it — and a collision means
                // two stack slots sharing one instance, same as a duplicate
                // navigateTo.
                if lxapp
                    .get_page_stack()
                    .iter()
                    .rev()
                    .skip(1)
                    .any(|entry| entry == &path)
                {
                    return Err(LxAppError::InvalidParameter(format!(
                        "redirectTo target '{path}' is already on the page stack. \
                         A page can only appear once; navigate back to it instead."
                    )));
                }
                lxapp.pop_from_page_stack();
            }
            NavigationType::Forward => {
                if lxapp.is_page_stack_full() {
                    info!("PageInstance stack is full, cannot navigate forward.");
                    return Ok(target_page);
                }
                // A page instance is keyed by its path, so the same route
                // cannot be on the stack twice: both entries would share one
                // instance, and popping either would end the instance the
                // other still shows. Reject it rather than pretend.
                if lxapp.get_page_stack().iter().any(|entry| entry == &path) {
                    return Err(LxAppError::InvalidParameter(format!(
                        "navigateTo target '{path}' is already on the page stack. \
                         A page can only appear once; use lx.redirectTo to replace \
                         the current page, or navigate to a different route."
                    )));
                }
            }
            NavigationType::Backward => {
                return Err(LxAppError::UnsupportedOperation(
                    "should use navigate_back".to_string(),
                ));
            }
        }

        // 3. Handle UI state based on navigation type (TabBar, NavBar)
        lxapp.with_tabbar_mut(|t| t.set_visible(is_tab_switch));
        if is_tab_switch
            && let Some(Some(index)) = lxapp.with_tabbar_mut(|t| t.find_index_by_path(&path))
        {
            lxapp.with_tabbar_mut(|t| {
                t.set_selected_index(index);
            });
        } else if !is_tabbar_page {
            // Navigating to a non-tab page: no tabbar item may stay
            // highlighted (the lxapp tab itself still is).
            lxapp.with_tabbar_mut(|t| {
                t.clear_selected_index();
            });
        }
        lxapp.push_to_page_stack(&path)?;

        // Set navbar state AFTER page creation to avoid being overwritten
        let stack_size = lxapp.get_page_stack_size();
        let show_back_button = stack_size > 1;
        let show_home_button = stack_size <= 1 && !is_tabbar_page && !is_initial_route;
        target_page.get_navbar_state_mut(|navbar| {
            let allow_buttons = navbar.show_navbar;
            navbar.set_back_button_visibility(show_back_button && allow_buttons);
            navbar.set_home_button_visibility(show_home_button && allow_buttons);
        });

        lxapp.sync_host_ui();

        // 5. Dispatch lifecycle events for current and target pages
        match nav_type {
            NavigationType::Replace => {
                self.dispatch_lifecycle_event(PageLifecycleEvent::OnUnload);
                // Replacing a page with itself never takes it off screen, so
                // resetting would reload the document under the user — a white
                // frame on every redirect. The entry re-runs `onLoad` with the
                // new query against the instance that is already there.
                if target_page.instance_id_string() != self.instance_id_string() {
                    lxapp.schedule_page_reset(self);
                }
            }
            NavigationType::Launch => {}
            _ => {
                self.dispatch_lifecycle_event(PageLifecycleEvent::OnHide);
            }
        }

        // Request onLoad for the target page; the lifecycle state machine will gate:
        // - If first-time render hasn't started yet, the request is kept until render starts.
        // - If the WebView has rendered before (re-navigation), OnLoad is accepted immediately.
        target_page.dispatch_lifecycle_event(PageLifecycleEvent::OnLoad);

        // 6. Perform the native navigation
        (*lxapp.runtime)
            .navigate(self.appid(), path, nav_type.to_animation())
            .map_err(LxAppError::from)?;

        lxapp.sync_host_ui();

        // Do not dispatch OnReady here. WebViewDelegate::on_page_finished() will do it.

        Ok(target_page)
    }

    pub fn navigate_back(&self, delta: u32) -> Result<(), LxAppError> {
        let lxapp = self.owning_lxapp();
        let stack_size = lxapp.get_page_stack_size();

        // Ensure at least one page remains
        if stack_size <= 1 {
            return Ok(());
        }

        let mut pages_to_pop = delta;
        // Prevent popping all pages
        if pages_to_pop as usize >= stack_size {
            pages_to_pop = (stack_size - 1) as u32;
        }

        if pages_to_pop == 0 {
            return Ok(());
        }

        for _ in 0..pages_to_pop {
            if let Some(path) = lxapp.pop_from_page_stack()
                && let Some(page) = lxapp.get_page(path.as_str())
            {
                page.dispatch_lifecycle_event(PageLifecycleEvent::OnUnload);
                // `onUnload` means the instance ended. The WebView is retained
                // for a warm re-entry, so reset the service and the document
                // behind it — otherwise the next entry inherits this one's
                // `data` and its DOM, popups included.
                lxapp.schedule_page_reset(&page);
            }
        }

        if let Some(path) = lxapp.peek_current_page() {
            // Forward navigation clears selected_index on detail pages, so
            // Back must restore selection as well as visibility.
            let is_tabbar_page = lxapp
                .with_tabbar_mut(|tabbar| restore_tabbar_after_back(tabbar, &path))
                .unwrap_or(false);

            // Update NavBar back button visibility based on the new stack size
            let new_stack_size = lxapp.get_page_stack_size();
            if let Some(dest_page) = lxapp.get_page(&path) {
                let is_initial_route = path == lxapp.config.get_initial_route();
                let show_home_button = new_stack_size <= 1 && !is_tabbar_page && !is_initial_route;
                dest_page.get_navbar_state_mut(|navbar| {
                    let allow_buttons = navbar.show_navbar;
                    navbar.set_back_button_visibility(new_stack_size > 1 && allow_buttons);
                    navbar.set_home_button_visibility(show_home_button && allow_buttons);
                });
            }

            (*lxapp.runtime).navigate(
                self.appid(),
                path.clone(),
                NavigationType::Backward.to_animation(),
            )?;
            // Reveal lifecycle for the destination. Platforms with native
            // page containers (iOS/Android/Harmony) call `on_page_show`
            // from the container when the page surfaces; the windowed
            // runtime has no such callback, and `dispatch_lifecycle_event`
            // de-dupes when both paths fire.
            if let Some(dest_page) = lxapp.get_page(&path) {
                dest_page.dispatch_lifecycle_event(PageLifecycleEvent::OnShow);
                dest_page.mark_active();
            }
            lxapp.sync_host_ui();
            Ok(())
        } else {
            Err(LxAppError::UnsupportedOperation(
                "PageInstance stack is empty after pop".to_string(),
            ))
        }
    }

    pub(crate) fn set_query(&self, query_str: String) {
        if let Ok(query_value) = parse_query_string(&query_str) {
            self.inner.state.lock().unwrap().query = query_value;
        }
    }

    /// Call a JavaScript function in the page's logic service
    ///
    /// # Arguments
    /// * `name` - Function name to call
    /// * `arg` - JSON string containing function arguments
    ///
    /// # Returns
    /// `Ok(())` if successful, `Err(LxAppError)` if execution fails
    pub fn call_js(&self, name: String, arg: String) -> Result<(), LxAppError> {
        let lxapp = self.owning_lxapp();
        lxapp.executor.call_page_service(
            lxapp.clone(),
            self.path(),
            Some(self.instance_id_string()),
            name,
            Some(arg),
        )
    }

    /// Call a View method on this page without a payload and deserialize the response.
    pub async fn call_view<R>(&self, method: &str) -> Result<R, LxAppError>
    where
        R: DeserializeOwned,
    {
        self.call_view_in(method, ViewCallOptions::default()).await
    }

    /// Call a View method on this page without a payload using explicit call options.
    pub async fn call_view_in<R>(
        &self,
        method: &str,
        options: ViewCallOptions,
    ) -> Result<R, LxAppError>
    where
        R: DeserializeOwned,
    {
        let value = self.call_view_json_in(method, options).await?;
        decode_view_call_result(method, value)
    }

    /// Call a View method on this page with a typed payload and deserialize the response.
    pub async fn call_view_with<P, R>(&self, method: &str, params: &P) -> Result<R, LxAppError>
    where
        P: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        self.call_view_with_in(method, params, ViewCallOptions::default())
            .await
    }

    /// Call a View method on this page with a typed payload using explicit call options.
    pub async fn call_view_with_in<P, R>(
        &self,
        method: &str,
        params: &P,
        options: ViewCallOptions,
    ) -> Result<R, LxAppError>
    where
        P: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let value = self.call_view_json_with_in(method, params, options).await?;
        decode_view_call_result(method, value)
    }

    /// Call a View method on this page and return the raw JSON response.
    pub async fn call_view_json(&self, method: &str) -> Result<Value, LxAppError> {
        self.call_view_json_in(method, ViewCallOptions::default())
            .await
    }

    /// Call a View method on this page and return the raw JSON response using explicit options.
    pub async fn call_view_json_in(
        &self,
        method: &str,
        options: ViewCallOptions,
    ) -> Result<Value, LxAppError> {
        self.call_view_json_value(method, None, options).await
    }

    /// Call a View method on this page with a typed payload and return the raw JSON response.
    pub async fn call_view_json_with<P>(
        &self,
        method: &str,
        params: &P,
    ) -> Result<Value, LxAppError>
    where
        P: Serialize + ?Sized,
    {
        self.call_view_json_with_in(method, params, ViewCallOptions::default())
            .await
    }

    /// Call a View method on this page with a typed payload and return the raw JSON response.
    pub async fn call_view_json_with_in<P>(
        &self,
        method: &str,
        params: &P,
        options: ViewCallOptions,
    ) -> Result<Value, LxAppError>
    where
        P: Serialize + ?Sized,
    {
        self.call_view_json_value(method, serialize_view_call_params(params)?, options)
            .await
    }

    async fn call_view_json_value(
        &self,
        method: &str,
        params: Option<Value>,
        options: ViewCallOptions,
    ) -> Result<Value, LxAppError> {
        let pending = crate::view_call::call_view(self, method, params)?;
        crate::view_call::await_pending_view_call(pending, options.timeout()).await
    }
}

/// Owned form of a `NavigationOutcome`, so the progress lock is released
/// before the handlers run.
enum ClassifiedNavigation {
    Started,
    Loaded,
    Failed { description: String, kind: String },
    Superseded,
}

impl WebViewDelegate for PageInstance {
    fn on_navigation_event(&self, event: lingxia_webview::NavigationEvent) {
        let outcome = {
            let mut progress = self
                .inner
                .navigation_progress
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            // Borrowing the event out of the guard would hold the lock across
            // the handlers below, which re-enter page state.
            match progress.classify(&event) {
                NavigationOutcome::Started { .. } => ClassifiedNavigation::Started,
                NavigationOutcome::Loaded { .. } => ClassifiedNavigation::Loaded,
                NavigationOutcome::Failed { error } => ClassifiedNavigation::Failed {
                    description: error.description.clone(),
                    kind: format!("{:?}", error.kind),
                },
                NavigationOutcome::Superseded => ClassifiedNavigation::Superseded,
            }
        };
        match outcome {
            ClassifiedNavigation::Started => self.notify_page_started(),
            ClassifiedNavigation::Loaded => self.handle_loaded(),
            ClassifiedNavigation::Failed { description, kind } => {
                error!("page load failed: {} ({})", description, kind)
                    .with_appid(self.inner.appid.clone())
                    .with_path(self.inner.path.clone());
            }
            ClassifiedNavigation::Superseded => {}
        }
    }

    /// Handles a postMessage from the WebView
    fn handle_post_message(&self, msg: String) {
        if let Some((level, message)) = decode_console_envelope(&msg) {
            self.log(level, &message);
            return;
        }
        if let Some(component_payload) = decode_native_component_envelope(&msg) {
            self.handle_native_component_message(component_payload);
            return;
        }

        match IncomingMessage::from_json_str(&msg) {
            Ok(incoming) => {
                if let Err(e) = self.bridge().handle_incoming(self, Arc::new(incoming)) {
                    if self.is_unloaded() || self.webview().is_none() {
                        debug!("Dropping view message after page unload")
                            .with_appid(self.inner.appid.clone())
                            .with_path(self.inner.path.clone());
                    } else {
                        error!("Failed to handle view message: {}", e)
                            .with_appid(self.inner.appid.clone())
                            .with_path(self.inner.path.clone());
                    }
                }
            }
            Err(e) => {
                error!("Invalid postMessage JSON: {}", e)
                    .with_appid(self.inner.appid.clone())
                    .with_path(self.inner.path.clone());
            }
        }
    }

    /// Routes embedded native-component messages from the view to the
    /// registered in-process host (Windows; other platforms deliver
    /// component traffic through their own channels and never hit this).
    fn handle_native_component_message(&self, message_json: String) {
        crate::native_component::dispatch_component_message(self, &message_json);
    }

    /// Receive log from WebView
    fn log(&self, level: LogLevel, message: &str) {
        // Convert lingxia_webview::LogLevel to lingxia_log::LogLevel
        let log_level = match level {
            LogLevel::Error => LxLogLevel::Error,
            LogLevel::Warn => LxLogLevel::Warn,
            LogLevel::Info => LxLogLevel::Info,
            LogLevel::Debug => LxLogLevel::Debug,
            LogLevel::Verbose => LxLogLevel::Debug, // Map Verbose to Debug
        };

        LogBuilder::new(LogTag::WebViewConsole, message)
            .with_level(log_level)
            .with_path(&self.inner.path)
            .with_appid(self.inner.appid.clone());
    }
}

fn decode_console_envelope(msg: &str) -> Option<(LogLevel, String)> {
    let json = serde_json::from_str::<Value>(msg).ok()?;
    json.get("__lingxia_console__")
        .and_then(Value::as_bool)
        .filter(|enabled| *enabled)?;
    let level = match json.get("level").and_then(Value::as_str) {
        Some("error") => LogLevel::Error,
        Some("warn") => LogLevel::Warn,
        Some("debug") => LogLevel::Debug,
        Some("info") => LogLevel::Info,
        Some("verbose") => LogLevel::Verbose,
        _ => LogLevel::Info,
    };
    let message = json.get("message").and_then(Value::as_str)?.to_string();
    Some((level, message))
}

fn decode_native_component_envelope(msg: &str) -> Option<String> {
    let json = serde_json::from_str::<Value>(msg).ok()?;
    json.get("__lingxia_native_component__")
        .and_then(Value::as_bool)
        .filter(|enabled| *enabled)?;
    json.get("payload")
        .and_then(Value::as_str)
        .map(str::to_string)
}

impl Drop for PageInstanceInner {
    fn drop(&mut self) {
        // Native components mounted by this page (if any) go down with it.
        crate::native_component::notify_page_destroyed(self.webtag.key());

        // Destroy WebView if it exists
        if let Ok(mut webview) = self.webview.lock()
            && let Some(_webview_controller) = webview.take()
        {
            // WebView will be automatically destroyed when controller is dropped
            info!("WebView destroyed for page")
                .with_appid(self.appid.clone())
                .with_path(self.path.clone());
        }
    }
}

fn restore_tabbar_after_back(tabbar: &mut crate::lxapp::tabbar::TabBar, path: &str) -> bool {
    let index = tabbar.find_index_by_path(path);
    tabbar.set_visible(index.is_some());
    if let Some(index) = index {
        tabbar.set_selected_index(index);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, serde::Deserialize)]
    struct ViewReply {
        ok: bool,
    }

    fn test_page_state() -> PageState {
        PageState {
            event: None,
            render_status: PageRenderStatus::Unstarted,
            entry: EntryPhase::Idle,
            visibility: Visibility::Hidden,
            bridge_ready: false,
            requires_bridge_ready: true,
            ready_dispatched: false,
            reset: PageReset::None,
            navbar_state: NavigationBarState::default(),
            config_load_error: None,
            enable_pull_down_refresh: false,
            orientation_override: OrientationOverride::default(),
            query: serde_json::json!({}),
        }
    }

    #[test]
    fn serialize_view_call_params_skips_null() {
        assert_eq!(serialize_view_call_params(&()).unwrap(), None);
        assert_eq!(
            serialize_view_call_params(&serde_json::json!({ "topic": "status" })).unwrap(),
            Some(serde_json::json!({ "topic": "status" }))
        );
    }

    #[test]
    fn decode_view_call_result_deserializes_typed_payload() {
        let reply: ViewReply =
            decode_view_call_result("example.echo", serde_json::json!({ "ok": true })).unwrap();
        assert!(reply.ok);
    }

    #[test]
    fn decode_view_call_result_reports_method_name() {
        let err = decode_view_call_result::<ViewReply>("example.echo", serde_json::json!({}))
            .unwrap_err();

        match err {
            LxAppError::Bridge(message) => {
                assert!(message.contains("example.echo"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn view_call_options_default_timeout_is_positive() {
        assert!(ViewCallOptions::default().timeout() > Duration::ZERO);
    }

    #[test]
    fn back_to_tab_page_restores_the_selected_item() {
        let mut tabbar: crate::lxapp::tabbar::TabBar = serde_json::from_value(serde_json::json!({
            "items": [
                { "pagePath": "pages/home/index" },
                { "pagePath": "pages/api/index" }
            ]
        }))
        .unwrap();
        tabbar.clear_selected_index();
        tabbar.set_visible(false);

        assert!(restore_tabbar_after_back(&mut tabbar, "pages/api/index"));
        assert!(tabbar.is_effectively_visible());
        assert_eq!(tabbar.get_selected_index(), 1);
    }

    #[test]
    fn lifecycle_cascade_waits_for_bridge_ready_before_on_load() {
        let mut state = test_page_state();
        let mut events = Vec::new();

        PageInstance::request_on_load(&mut state);
        state.render_status = PageRenderStatus::Started;
        PageInstance::collect_ready_lifecycle_events(&mut state, &mut events);

        assert!(events.is_empty());
        assert_eq!(state.entry, EntryPhase::LoadOwed);
        assert_ne!(state.entry, EntryPhase::Loaded);

        state.bridge_ready = true;
        PageInstance::collect_ready_lifecycle_events(&mut state, &mut events);

        assert_eq!(
            events,
            vec![(PageLifecycleEvent::OnLoad, Some("{}".to_string()))]
        );
        assert_ne!(state.entry, EntryPhase::LoadOwed);
        assert_eq!(state.entry, EntryPhase::Loaded);
    }

    #[test]
    fn lifecycle_cascade_allows_on_load_without_bridge_for_native_only_pages() {
        let mut state = test_page_state();
        let mut events = Vec::new();

        state.requires_bridge_ready = false;
        PageInstance::request_on_load(&mut state);
        state.render_status = PageRenderStatus::Started;
        PageInstance::collect_ready_lifecycle_events(&mut state, &mut events);

        assert_eq!(
            events,
            vec![(PageLifecycleEvent::OnLoad, Some("{}".to_string()))]
        );
        assert_ne!(state.entry, EntryPhase::LoadOwed);
        assert_eq!(state.entry, EntryPhase::Loaded);
    }

    #[test]
    fn lifecycle_cascade_orders_load_show_ready() {
        let mut state = test_page_state();
        let mut events = Vec::new();

        state.entry = EntryPhase::LoadOwed;
        state.bridge_ready = true;
        state.visibility = Visibility::ShowOwed;
        state.render_status = PageRenderStatus::Finished;
        PageInstance::collect_ready_lifecycle_events(&mut state, &mut events);

        assert_eq!(
            events,
            vec![
                (PageLifecycleEvent::OnLoad, Some("{}".to_string())),
                (PageLifecycleEvent::OnShow, None),
                (PageLifecycleEvent::OnReady, None),
            ]
        );
        assert_ne!(state.entry, EntryPhase::LoadOwed);
        assert_eq!(state.entry, EntryPhase::Loaded);
        assert_eq!(state.visibility, Visibility::Shown);
        assert!(state.ready_dispatched);
    }

    #[test]
    fn reentering_a_cached_page_does_not_refire_ready_without_a_reload() {
        let mut state = test_page_state();
        state.bridge_ready = true;
        state.visibility = Visibility::ShowOwed;
        state.render_status = PageRenderStatus::Finished;
        PageInstance::request_on_load(&mut state);
        PageInstance::collect_ready_lifecycle_events(&mut state, &mut Vec::new());
        assert!(state.ready_dispatched);

        // Re-entry without a document reload: load and show repeat, ready does not.
        let mut events = Vec::new();
        state.visibility = Visibility::ShowOwed;
        PageInstance::request_on_load(&mut state);
        PageInstance::collect_ready_lifecycle_events(&mut state, &mut events);

        assert_eq!(
            events,
            vec![
                (PageLifecycleEvent::OnLoad, Some("{}".to_string())),
                (PageLifecycleEvent::OnShow, None),
            ]
        );
    }

    #[test]
    fn a_reset_page_fires_ready_again_for_the_new_document() {
        let mut state = test_page_state();
        state.bridge_ready = true;
        state.visibility = Visibility::ShowOwed;
        state.render_status = PageRenderStatus::Finished;
        PageInstance::request_on_load(&mut state);
        PageInstance::collect_ready_lifecycle_events(&mut state, &mut Vec::new());

        // A reset reloads the document, so the next render is ready-worthy.
        PageInstance::reset_webview_lifecycle_state(&mut state);
        state.bridge_ready = true;
        state.visibility = Visibility::ShowOwed;
        state.render_status = PageRenderStatus::Finished;
        PageInstance::request_on_load(&mut state);
        let mut events = Vec::new();
        PageInstance::collect_ready_lifecycle_events(&mut state, &mut events);

        assert!(events.contains(&(PageLifecycleEvent::OnReady, None)));
    }

    #[test]
    fn a_document_rebuilt_for_a_later_entry_does_not_boot_itself() {
        let mut state = test_page_state();
        state.reset = PageReset::AwaitingEntry;
        state.entry = EntryPhase::Idle;

        // The bridge-ready auto-request is what boots surfaces and app-service
        // restarts; a page rebuilt for an entry that has not happened yet must
        // stay put instead of firing onLoad at nobody.
        assert!(!(state.entry == EntryPhase::Idle && state.reset == PageReset::None));

        // The real entry clears it and takes over.
        PageInstance::request_on_load(&mut state);
        assert_eq!(state.reset, PageReset::None);
        assert_eq!(state.entry, EntryPhase::LoadOwed);
    }

    #[test]
    fn unload_cancels_a_pending_entry_but_not_a_delivered_one() {
        let mut owed = test_page_state();
        owed.entry = EntryPhase::LoadOwed;
        PageInstance::collect_hidden_or_unloaded_lifecycle_event(
            &mut owed,
            PageLifecycleEvent::OnUnload,
            &mut Vec::new(),
        );
        assert_eq!(owed.entry, EntryPhase::Idle);

        // A delivered entry stays delivered, so a reused instance does not
        // re-request `onLoad` off the back of its existing bridge.
        let mut delivered = test_page_state();
        delivered.entry = EntryPhase::Loaded;
        PageInstance::collect_hidden_or_unloaded_lifecycle_event(
            &mut delivered,
            PageLifecycleEvent::OnUnload,
            &mut Vec::new(),
        );
        assert_eq!(delivered.entry, EntryPhase::Loaded);
    }

    #[test]
    fn hiding_a_shown_page_owes_on_show_again_on_re_entry() {
        let mut state = test_page_state();
        state.bridge_ready = true;
        state.visibility = Visibility::ShowOwed;
        state.render_status = PageRenderStatus::Finished;
        PageInstance::request_on_load(&mut state);
        PageInstance::collect_ready_lifecycle_events(&mut state, &mut Vec::new());
        assert_eq!(state.visibility, Visibility::Shown);

        let mut events = Vec::new();
        PageInstance::collect_hidden_or_unloaded_lifecycle_event(
            &mut state,
            PageLifecycleEvent::OnHide,
            &mut events,
        );
        assert_eq!(state.visibility, Visibility::Hidden);

        // Becoming visible again owes another onShow, with no second onLoad:
        // the entry never ended.
        state.visibility = Visibility::ShowOwed;
        let mut events = Vec::new();
        PageInstance::collect_ready_lifecycle_events(&mut state, &mut events);
        assert_eq!(events, vec![(PageLifecycleEvent::OnShow, None)]);
    }

    #[test]
    fn reset_webview_lifecycle_state_does_not_keep_stale_hidden_show_request() {
        let mut state = test_page_state();
        state.event = Some(PageLifecycleEvent::OnHide);
        state.visibility = Visibility::ShowOwed;
        state.entry = EntryPhase::LoadOwed;
        state.bridge_ready = true;
        state.entry = EntryPhase::Loaded;
        state.visibility = Visibility::Shown;
        state.ready_dispatched = true;
        state.render_status = PageRenderStatus::Finished;

        PageInstance::reset_webview_lifecycle_state(&mut state);

        assert_eq!(state.visibility, Visibility::Hidden);
        assert_ne!(state.entry, EntryPhase::LoadOwed);
        assert!(!state.bridge_ready);
        assert_ne!(state.entry, EntryPhase::Loaded);
        assert_ne!(state.visibility, Visibility::Shown);
        assert!(!state.ready_dispatched);
        assert_eq!(state.render_status, PageRenderStatus::Unstarted);
    }

    #[test]
    fn reset_webview_lifecycle_state_preserves_current_visible_intent() {
        let mut state = test_page_state();
        state.event = Some(PageLifecycleEvent::OnShow);

        PageInstance::reset_webview_lifecycle_state(&mut state);

        assert_ne!(state.visibility, Visibility::Hidden);
    }

    #[test]
    fn on_unload_preserves_bridge_ready_for_reusable_page_instance() {
        let mut state = test_page_state();
        let mut events = Vec::new();

        state.visibility = Visibility::ShowOwed;
        state.entry = EntryPhase::LoadOwed;
        state.bridge_ready = true;

        PageInstance::collect_hidden_or_unloaded_lifecycle_event(
            &mut state,
            PageLifecycleEvent::OnUnload,
            &mut events,
        );

        assert_eq!(events, vec![(PageLifecycleEvent::OnUnload, None)]);
        assert_eq!(state.visibility, Visibility::Hidden);
        assert_ne!(state.entry, EntryPhase::LoadOwed);
        assert!(state.bridge_ready);
    }

    #[test]
    fn only_on_unload_cancels_page_bridge_work() {
        assert!(!PageInstance::lifecycle_cancels_bridge_work(
            PageLifecycleEvent::OnHide
        ));
        assert!(PageInstance::lifecycle_cancels_bridge_work(
            PageLifecycleEvent::OnUnload
        ));
    }

    #[test]
    fn attach_reset_boundary_only_resets_real_replacement() {
        assert!(!PageInstance::should_reset_lifecycle_on_attach(None));
        assert!(!PageInstance::should_reset_lifecycle_on_attach(Some(true)));
        assert!(PageInstance::should_reset_lifecycle_on_attach(Some(false)));
    }

    #[test]
    fn first_attach_must_not_drop_bridge_ready_load_request() {
        let mut state = test_page_state();
        let mut events = Vec::new();

        state.bridge_ready = true;
        PageInstance::request_on_load(&mut state);
        state.render_status = PageRenderStatus::Started;

        // First None -> Some attach must not reset; otherwise this state would
        // lose bridge_ready/load_requested before render-start can complete onLoad.
        PageInstance::collect_ready_lifecycle_events(&mut state, &mut events);

        assert_eq!(
            events,
            vec![(PageLifecycleEvent::OnLoad, Some("{}".to_string()))]
        );
        assert_eq!(state.entry, EntryPhase::Loaded);
    }

    #[test]
    fn first_attach_must_not_drop_pending_show_request() {
        let mut state = test_page_state();
        let mut events = Vec::new();

        state.visibility = Visibility::ShowOwed;
        state.bridge_ready = true;
        PageInstance::request_on_load(&mut state);
        state.render_status = PageRenderStatus::Started;

        // OnShow can arrive before onLoad; first attach must preserve that
        // intent so onShow is emitted immediately after onLoad.
        PageInstance::collect_ready_lifecycle_events(&mut state, &mut events);

        assert_eq!(
            events,
            vec![
                (PageLifecycleEvent::OnLoad, Some("{}".to_string())),
                (PageLifecycleEvent::OnShow, None),
            ]
        );
        assert_eq!(state.entry, EntryPhase::Loaded);
        assert_eq!(state.visibility, Visibility::Shown);
    }
}
