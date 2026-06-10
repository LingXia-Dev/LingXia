pub(crate) mod config;
pub(crate) mod definition;
pub(crate) mod runtime;

pub use definition::{register_page_resolver, resolve_page_path};

use crate::bridge::{IncomingMessage, PageBridge};
use crate::lifecycle::PageLifecycleEvent;
use crate::lxapp::{self, navbar::NavigationBarState};
use crate::page::config::{OrientationOverride, PageConfig};
use crate::plugin;
use crate::startup::parse_query_string;
use crate::{LxApp, LxAppError, error, info};
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
    LoadDataRequest, LogLevel, NavigationPolicy, NewWindowPolicy, WebTag, WebView, WebViewBuilder,
    WebViewController, WebViewDelegate,
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

    // Scripts injected on every page load (global + app-level, snapshotted at creation).
    page_scripts: Vec<Arc<str>>,

    // Async notification: bumped on every handle_loaded().
    loaded_tx: watch::Sender<u64>,
}

#[derive(Clone, Debug)]
pub struct PageState {
    // PageInstance(webview) reander status
    render_status: PageRenderStatus,
    // page lifecycle event
    event: PageLifecycleEvent,
    /// Tracks if the UI has requested to show this page. Handles onShow arriving before onLoad.
    show_requested: bool,
    /// Tracks if the onLoad JavaScript event has been fired to prevent duplicates.
    on_load_fired: bool,
    /// Tracks if the onShow JavaScript event has been fired. Reset on hide to allow re-entry.
    on_show_fired: bool,
    /// Tracks if the onReady JavaScript event has been fired to prevent duplicates.
    on_ready_fired: bool,
    // Navigation bar state
    pub(crate) navbar_state: NavigationBarState,
    // Pull-to-refresh enabled flag
    pub(crate) enable_pull_down_refresh: bool,
    // PageInstance orientation overrides
    pub(crate) orientation_override: OrientationOverride,
    // Query parameters
    pub(crate) query: serde_json::Value,
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
        let page_config = if lxapp.logic_enabled() {
            PageConfig::from_json(lxapp, path)
        } else {
            // When logic is disabled, page.json is intentionally ignored.
            // In this mode pages talk directly to Rust without JS/page config.
            PageConfig::default()
        };
        PageState {
            event: PageLifecycleEvent::Unknown,
            render_status: PageRenderStatus::Unstarted,
            show_requested: false,
            on_load_fired: false,
            on_show_fired: false,
            on_ready_fired: false,
            navbar_state: page_config.create_navbar_state(),
            enable_pull_down_refresh: page_config.is_pull_down_refresh_enabled(),
            orientation_override: page_config.get_orientation_override(),
            query: serde_json::Value::Null,
        }
    }

    /// Create a new page in pending state (WebView creation in progress)
    pub(crate) fn new<F, Fut>(appid: String, path: String, lxapp: &LxApp, setup_callback: F) -> Self
    where
        F: Fn(&PageInstance) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
    {
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
            page_scripts: lxapp.page_scripts_snapshot(),
            loaded_tx,
        });

        // Capture weak ref before moving inner into page
        let page_weak_for_lx = Arc::downgrade(&inner);

        let page = Self { inner };

        // Initiate WebView creation with scheme handlers
        // Register closure-based scheme handlers so lingxia-webview
        // doesn't need to know about lxapp business logic.
        let appid_for_lx = appid.clone();

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
                let appid_for_lx = appid_for_lx.clone();
                async move {
                    let Some(inner) = page_weak_for_lx.upgrade() else {
                        return None.into();
                    };
                    let lxapp = lxapp::get(appid_for_lx);
                    let page = PageInstance::from_inner(inner);
                    lxapp.handle_lingxia_request(&page, req).into()
                }
            })
            .on_navigation(move |url| {
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
                    error!("Failed to create WebView: {}", e)
                        .with_appid(appid_clone)
                        .with_path(path_clone);
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

    pub(crate) fn webtag(&self) -> WebTag {
        self.inner.webtag.clone()
    }

    pub(crate) fn bridge(&self) -> PageBridge {
        self.inner.bridge.clone()
    }

    /// Attach WebView to this page (called when WebView is ready)
    pub fn attach_webview(&self, webview: Arc<WebView>) {
        if let Ok(mut webview_guard) = self.inner.webview.lock() {
            *webview_guard = Some(webview);
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

    /// Set page reander status
    fn set_render_status(&self, status: PageRenderStatus) {
        if let Ok(mut state) = self.inner.state.lock() {
            state.render_status = status;
        }
    }

    pub(crate) fn dispatch_lifecycle_event(&self, event: PageLifecycleEvent) {
        // Central lifecycle state machine for a single WebView-backed PageInstance.
        // Sources of events:
        // - First-time creation: WebView/LXPort ready triggers onLoad (AppService side)
        // - Re-navigation with new query (navigateTo): native manually triggers onLoad
        // - Render completion: WebView delegate triggers onReady
        // - Visibility changes: native triggers onShow/onHide
        // Goals (Weixin semantics adapted to a single WebView instance):
        // - onLoad carries query and may occur multiple times across logical navigations
        //   (first-time + each navigateTo with new params)
        // - onReady fires once for each logical navigation after render has finished
        // - onShow fires each time the page becomes visible (after a hide), without query

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
                if state.event != event {
                    events_to_fire.push((event, None));
                    state.event = event;
                    // Reset on_show_fired when the page is hidden, to allow onShow to fire again on re-entry.
                    state.on_show_fired = false;
                }
            } else {
                // This logic handles the Load -> Show -> Ready cascade.

                // Update raw status based on the incoming event.
                if event == PageLifecycleEvent::OnShow {
                    state.show_requested = true;
                }

                // Guard: only honor a manual OnLoad after render has started.
                // Bridge-ready path typically arrives after on_page_started, so it passes.
                if event == PageLifecycleEvent::OnLoad
                    && matches!(state.render_status, PageRenderStatus::Unstarted)
                {
                    // Ignore early OnLoad until WebView render actually begins.
                    // Caller can invoke again later; bridge-ready path will also dispatch.
                    return;
                }

                // Handle onLoad (can occur multiple times for navigateTo with new params)
                if event == PageLifecycleEvent::OnLoad {
                    let query = serde_json::to_string(&state.query).ok();
                    events_to_fire.push((PageLifecycleEvent::OnLoad, query));
                    state.on_load_fired = true;
                    state.on_show_fired = false;
                    state.on_ready_fired = false;
                }

                // Desired order (Weixin semantics): Load -> Show -> Ready (first load)
                // 1) Ready: after Load and render finished; only once per lifecycle
                if state.on_load_fired
                    && state.render_status == PageRenderStatus::Finished
                    && !state.on_ready_fired
                {
                    events_to_fire.push((PageLifecycleEvent::OnReady, None));
                    state.on_ready_fired = true;
                }

                // 2) Show: each time the page becomes visible after hide
                // Do not require Ready; allow Show before Ready on first load.
                if state.on_load_fired && state.show_requested && !state.on_show_fired {
                    events_to_fire.push((PageLifecycleEvent::OnShow, None));
                    state.on_show_fired = true;
                    state.event = PageLifecycleEvent::OnShow;
                }
            }
        }

        //  Fire the collected events outside of the lock to prevent deadlocks.
        if !events_to_fire.is_empty() {
            let lxapp = lxapp::get(self.inner.appid.clone());
            let appid = self.appid();
            let path = self.path();

            for (event, query) in events_to_fire {
                let page_event = match event {
                    PageLifecycleEvent::OnLoad => crate::lifecycle::PageServiceEvent::OnLoad,
                    PageLifecycleEvent::OnShow => crate::lifecycle::PageServiceEvent::OnShow,
                    PageLifecycleEvent::OnReady => crate::lifecycle::PageServiceEvent::OnReady,
                    PageLifecycleEvent::OnHide => crate::lifecycle::PageServiceEvent::OnHide,
                    PageLifecycleEvent::OnUnload => crate::lifecycle::PageServiceEvent::OnUnload,
                    PageLifecycleEvent::OnPullDownRefresh => {
                        crate::lifecycle::PageServiceEvent::OnPullDownRefresh
                    }
                    PageLifecycleEvent::Unknown => {
                        // Skip unknown
                        continue;
                    }
                };

                if let Err(e) = lxapp.executor.call_page_service_event(
                    lxapp.clone(),
                    path.clone(),
                    Some(self.instance_id_string()),
                    page_event,
                    query,
                ) {
                    error!("Failed to call {}: {}", String::from(event), e)
                        .with_appid(appid.clone())
                        .with_path(path.clone());
                }
            }
        }
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

    pub(crate) fn mark_webview_ready(&self, result: Result<(), String>) {
        // Ignore errors; receiver will handle missing updates.
        let _ = self.inner.webview_ready_tx.send(Some(result));
    }

    /// Notify that the page's WebView started loading (mirrors WebViewDelegate::on_page_started).
    /// Used by external delegates to forward events to a shared page.
    pub fn notify_page_started(&self) {
        self.set_render_status(PageRenderStatus::Started);
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
        self.set_render_status(PageRenderStatus::Finished);
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

        self.dispatch_lifecycle_event(PageLifecycleEvent::OnReady);
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
        if let Ok(mut webview_guard) = self.inner.webview.lock() {
            // Drop the Arc by taking it out
            let _ = webview_guard.take();
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
        let lxapp = lxapp::get(self.appid());
        let path = self.path();
        let html_data = lxapp.generate_page_html(&path, self.bridge_nonce().as_deref());
        let base_url = self.base_url();
        let html_string = String::from_utf8_lossy(&html_data).into_owned();

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
        let lxapp = lxapp::get(self.inner.appid.clone());
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
        let lxapp = lxapp::get(self.appid());

        // Normalize through LxApp to ensure consistent canonical paths (e.g. plugin routes).
        let target_page = lxapp.get_or_create_page(&target_page.path());
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
                    target_page = lxapp.get_or_create_page(&path);
                }
                lxapp.clear_page_stack()?;
            }
            NavigationType::Replace => {
                lxapp.pop_from_page_stack();
            }
            NavigationType::Forward => {
                if lxapp.is_page_stack_full() {
                    info!("PageInstance stack is full, cannot navigate forward.");
                    return Ok(target_page);
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
            }
            NavigationType::Launch => {}
            _ => {
                self.dispatch_lifecycle_event(PageLifecycleEvent::OnHide);
            }
        }

        // Request onLoad for the target page; dispatch_lifecycle_event will gate:
        // - If first-time render hasn't started yet, the early OnLoad is ignored; bridge-ready path will dispatch.
        // - If the WebView has rendered before (re-navigation), OnLoad will be accepted and ordered correctly.
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
        let lxapp = lxapp::get(self.appid());
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
            }
        }

        if let Some(path) = lxapp.peek_current_page() {
            // Update UI for the destination page
            // Check if destination is a tabbar page without holding any locks
            let is_tabbar_page = lxapp
                .get_tabbar()
                .is_some_and(|tabbar| tabbar.is_tabbar_page(&path));
            lxapp.with_tabbar_mut(|t| t.set_visible(is_tabbar_page));

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
                path,
                NavigationType::Backward.to_animation(),
            )?;
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
        let lxapp = lxapp::get(self.appid());
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

impl WebViewDelegate for PageInstance {
    /// Called when the page starts loading
    fn on_page_started(&self) {
        self.set_render_status(PageRenderStatus::Started);
    }

    /// Called when the page finishes loading
    fn on_page_finished(&self) {
        self.handle_loaded();
    }

    /// Handles a postMessage from the WebView
    fn handle_post_message(&self, msg: String) {
        match IncomingMessage::from_json_str(&msg) {
            Ok(incoming) => {
                if let Err(e) = self.bridge().handle_incoming(self, Arc::new(incoming)) {
                    error!("Failed to handle view message: {}", e)
                        .with_appid(self.inner.appid.clone());
                }
            }
            Err(e) => {
                error!("Invalid postMessage JSON: {}", e)
                    .with_appid(self.inner.appid.clone())
                    .with_path(self.inner.path.clone());
            }
        }
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

impl Drop for PageInstanceInner {
    fn drop(&mut self) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, serde::Deserialize)]
    struct ViewReply {
        ok: bool,
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
}
