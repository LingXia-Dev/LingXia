//! Host API runtime and extension surface.
//!
//! Built-in host capabilities and third-party host extensions share the same
//! registry. External crates can define handlers and register them here.

use crate::ControlDocumentAuthority;
use crate::error::LxAppError;
use crate::lxapp::{AppSessionClass, LxApp, LxAppSessionStatus};

use futures::Stream;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use tokio::sync::{mpsc, oneshot};

#[macro_use]
mod macros;

mod device;
mod navigation;
mod navigator;

pub type HostResult<T> = Result<T, LxAppError>;
pub type JsonValue = serde_json::Value;

pub type HostCancel = oneshot::Receiver<()>;
pub type HostStream =
    Pin<Box<dyn Stream<Item = Result<HostStreamItem, LxAppError>> + Send + 'static>>;
pub type HostFuture<'a> = Pin<Box<dyn Future<Output = Result<HostOutput, LxAppError>> + Send + 'a>>;

pub enum HostStreamItem {
    Event(String),
    Return(String),
}

pub enum HostOutput {
    Json(String),
    Stream(HostStream),
}

#[doc(hidden)]
pub mod __native {
    use super::{Future, HostResult, LxAppError};

    pub fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        crate::executor::spawn(future)
    }

    pub async fn spawn_blocking<F, R>(f: F) -> HostResult<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        rong_rt::RongExecutor::global()
            .spawn_blocking(f)
            .await
            .map_err(|err| LxAppError::Runtime(err.to_string()))
    }
}

/// Wire-level method kind, serialized into the handshake schema so the JS
/// bridge can automatically choose `invoke` vs `stream`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostMethodKind {
    Call,
    Stream,
}

/// The admission constraint attached to a host route.
///
/// This is deliberately a closed SDK enum. Dispatch policy determines
/// the caller set for each constraint; callers cannot select one from a bridge
/// payload or an app manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteAudience {
    AppSessionOnly,
    AuthenticatedReadOnly,
    ControlAppOnly,
    BrowserControlOnly,
    ControlOnly,
}

/// A privileged native resource that may be assigned to one lxapp session.
///
/// Manifest entries are requests, not grants. The native host seals the
/// granted subset when it creates the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AppResourceGrant {
    Process,
    Downloads,
    Automation,
    AutomationHost,
}

impl AppResourceGrant {
    pub const fn manifest_privilege(self) -> &'static str {
        match self {
            Self::Process => "process",
            Self::Downloads => "downloads",
            Self::Automation => "automation",
            Self::AutomationHost => "host",
        }
    }
}

type AppResourceGrantResolver =
    dyn Fn(&Arc<LxApp>) -> HashSet<AppResourceGrant> + Send + Sync + 'static;

static APP_RESOURCE_GRANT_RESOLVER: OnceLock<Arc<AppResourceGrantResolver>> = OnceLock::new();

/// Install the native host's per-session grant resolver before lxapp runtime
/// initialization. The resolver and each resulting grant set are sealed once.
#[doc(hidden)]
pub fn __install_app_resource_grant_resolver(resolver: Arc<AppResourceGrantResolver>) -> bool {
    APP_RESOURCE_GRANT_RESOLVER.set(resolver).is_ok()
}

pub(crate) fn seal_app_resource_grants(app: &Arc<LxApp>) {
    let grants = APP_RESOURCE_GRANT_RESOLVER
        .get()
        .map(|resolver| resolver(app))
        .unwrap_or_default();
    app.seal_resource_grants(grants);
}

/// Native identity of one authenticated lxapp session.
///
/// The constructor is crate-private: an app id in a bridge payload or manifest
/// cannot create or replace this identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppIdentity {
    app_id: Arc<str>,
    session_id: u64,
}

impl AppIdentity {
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    pub const fn session_id(&self) -> u64 {
        self.session_id
    }
}

/// Filesystem namespace assigned to an lxapp by the native runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppStorageNamespace {
    storage_file: PathBuf,
    user_data: PathBuf,
    user_cache: PathBuf,
    temporary: PathBuf,
}

impl AppStorageNamespace {
    pub fn storage_file(&self) -> &Path {
        &self.storage_file
    }

    pub fn user_data(&self) -> &Path {
        &self.user_data
    }

    pub fn user_cache(&self) -> &Path {
        &self.user_cache
    }

    pub fn temporary(&self) -> &Path {
        &self.temporary
    }
}

/// Native-issued resource grants attached to one live lxapp session.
///
/// Transient paths and references are issued by native pickers and keyed by
/// the immutable app/session identity. A retained scope fails closed once the
/// native app session is gone.
#[derive(Clone)]
pub struct AppResourceGrants {
    app_id: Arc<str>,
    session_id: u64,
    owner: Weak<LxApp>,
}

impl fmt::Debug for AppResourceGrants {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppResourceGrants")
            .field("app_id", &self.app_id)
            .field("session_id", &self.session_id)
            .finish_non_exhaustive()
    }
}

impl AppResourceGrants {
    fn live_owner(&self) -> Result<Arc<LxApp>, LxAppError> {
        let app = self.owner.upgrade().ok_or_else(|| {
            LxAppError::ResourceNotFound("lxapp resource scope is no longer live".to_string())
        })?;
        if app.appid != self.app_id.as_ref()
            || app.session_id() != self.session_id
            || matches!(
                app.status(),
                LxAppSessionStatus::Closed
                    | LxAppSessionStatus::Closing
                    | LxAppSessionStatus::Restarting
            )
        {
            return Err(LxAppError::ResourceNotFound(
                "lxapp resource scope no longer matches its native session".to_string(),
            ));
        }
        Ok(app)
    }

    /// Resolve a native-issued transient `lx://temp/...` grant.
    pub fn resolve_transient_file(&self, resource: &str) -> Result<PathBuf, LxAppError> {
        if !resource.trim().starts_with("lx://temp/") {
            return Err(LxAppError::InvalidParameter(
                "expected a native-issued lx://temp resource grant".to_string(),
            ));
        }
        self.live_owner()?.resolve_accessible_path(resource)
    }

    /// Test a native-issued opaque file reference for this exact app session.
    pub fn contains_file_reference(&self, reference: &str) -> bool {
        self.live_owner()
            .is_ok_and(|app| app.has_transient_file_reference(reference))
    }

    /// Whether the native host assigned this privileged resource to this live
    /// session. A manifest declaration alone never makes this return true.
    pub fn contains(&self, grant: AppResourceGrant) -> bool {
        self.live_owner()
            .is_ok_and(|app| app.has_resource_grant(grant))
    }
}

/// Native-derived resource scope of an authenticated lxapp session.
///
/// This value is constructed from the owning [`LxApp`], never from bridge
/// payload fields. Route audience admission and resource authorization remain
/// distinct: handlers use this scope after admission to resolve app-owned
/// storage or native-issued resource grants.
#[derive(Debug, Clone)]
pub struct AppScope {
    identity: AppIdentity,
    storage: AppStorageNamespace,
    resource_grants: AppResourceGrants,
}

impl AppScope {
    fn from_lxapp(app: &Arc<LxApp>) -> Self {
        let app_id: Arc<str> = Arc::from(app.appid.as_str());
        let session_id = app.session_id();
        Self {
            identity: AppIdentity {
                app_id: Arc::clone(&app_id),
                session_id,
            },
            storage: AppStorageNamespace {
                storage_file: app.storage_file_path.clone(),
                user_data: app.user_data_dir.clone(),
                user_cache: app.user_cache_dir.clone(),
                temporary: app.temp_dir.clone(),
            },
            resource_grants: AppResourceGrants {
                app_id,
                session_id,
                owner: Arc::downgrade(app),
            },
        }
    }

    pub fn identity(&self) -> &AppIdentity {
        &self.identity
    }

    pub fn storage(&self) -> &AppStorageNamespace {
        &self.storage
    }

    pub fn resource_grants(&self) -> &AppResourceGrants {
        &self.resource_grants
    }

    /// Resolve a path only within this app's native storage namespace or its
    /// native-issued transient grants.
    pub fn resolve_accessible_path(&self, resource: &str) -> Result<PathBuf, LxAppError> {
        self.resource_grants
            .live_owner()?
            .resolve_accessible_path(resource)
    }

    fn belongs_to(&self, app: &Arc<LxApp>) -> bool {
        self.identity.app_id() == app.appid
            && self.identity.session_id() == app.session_id()
            && self.resource_grants.owner.ptr_eq(&Arc::downgrade(app))
    }

    #[cfg(test)]
    fn for_test(app_id: &str, session_id: u64) -> Self {
        let app_id: Arc<str> = Arc::from(app_id);
        Self {
            identity: AppIdentity {
                app_id: Arc::clone(&app_id),
                session_id,
            },
            storage: AppStorageNamespace {
                storage_file: PathBuf::new(),
                user_data: PathBuf::new(),
                user_cache: PathBuf::new(),
                temporary: PathBuf::new(),
            },
            resource_grants: AppResourceGrants {
                app_id,
                session_id,
                owner: Weak::new(),
            },
        }
    }
}

/// Authenticated source for a native route invocation.
///
/// Browser construction is reserved for the browser document lifecycle TCB:
/// ordinary bridge frames derive only the `LxAppSession` variant from their
/// owning native `LxApp` session.
#[derive(Clone)]
pub enum AuthenticatedCaller {
    LxAppSession {
        class: AppSessionClass,
        scope: AppScope,
    },
    BrowserDocument {
        authority: ControlDocumentAuthority,
    },
}

impl AuthenticatedCaller {
    pub(crate) fn for_lxapp(app: &Arc<LxApp>) -> Self {
        Self::LxAppSession {
            class: app.app_session_class(),
            scope: AppScope::from_lxapp(app),
        }
    }

    /// Host-TCB constructor used only after the browser registry has promoted
    /// the exact document binding to Active.
    #[doc(hidden)]
    pub fn active_browser_document(authority: ControlDocumentAuthority) -> Self {
        Self::BrowserDocument { authority }
    }

    pub fn app_scope(&self) -> Option<&AppScope> {
        match self {
            Self::LxAppSession { scope, .. } => Some(scope),
            Self::BrowserDocument { .. } => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn standard_for_test(session_id: u64) -> Self {
        Self::LxAppSession {
            class: AppSessionClass::StandardApp,
            scope: AppScope::for_test("test.standard", session_id),
        }
    }

    #[cfg(test)]
    pub(crate) fn control_for_test(session_id: u64) -> Self {
        Self::LxAppSession {
            class: AppSessionClass::ControlApp,
            scope: AppScope::for_test("test.control", session_id),
        }
    }
}

/// Authenticated, native-created context passed to every host handler.
///
/// Its constructor is private to bridge dispatch. Third-party handlers may
/// inspect or clone it, but cannot mint a different caller or app scope.
#[derive(Clone)]
pub struct HostInvocationContext {
    caller: AuthenticatedCaller,
    lxapp: Arc<LxApp>,
}

impl HostInvocationContext {
    pub(crate) fn for_dispatch(lxapp: Arc<LxApp>, caller: &AuthenticatedCaller) -> Option<Self> {
        if let AuthenticatedCaller::LxAppSession { scope, .. } = caller
            && !scope.belongs_to(&lxapp)
        {
            return None;
        }
        Some(Self {
            caller: caller.clone(),
            lxapp,
        })
    }

    pub fn caller(&self) -> &AuthenticatedCaller {
        &self.caller
    }

    pub fn app_scope(&self) -> Option<&AppScope> {
        self.caller.app_scope()
    }

    pub fn lxapp(&self) -> Arc<LxApp> {
        Arc::clone(&self.lxapp)
    }
}

/// The sole route-audience decision point.
pub fn authorize(caller: &AuthenticatedCaller, audience: RouteAudience) -> bool {
    matches!(
        (caller, audience),
        (
            AuthenticatedCaller::LxAppSession { .. },
            RouteAudience::AppSessionOnly
        ) | (
            AuthenticatedCaller::LxAppSession { .. },
            RouteAudience::AuthenticatedReadOnly
        ) | (
            AuthenticatedCaller::LxAppSession {
                class: AppSessionClass::ControlApp,
                ..
            },
            RouteAudience::ControlAppOnly | RouteAudience::ControlOnly
        ) | (
            AuthenticatedCaller::BrowserDocument { .. },
            RouteAudience::AuthenticatedReadOnly
                | RouteAudience::BrowserControlOnly
                | RouteAudience::ControlOnly
        )
    )
}

/// The admission policy resolved when a route is registered.
///
/// Policy evaluation is intentionally separate from registration so every
/// route family can carry the same immutable metadata before dispatch starts
/// using it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffectiveRoutePolicy {
    audience: RouteAudience,
}

impl EffectiveRoutePolicy {
    pub const fn new(audience: RouteAudience) -> Self {
        Self { audience }
    }

    pub const fn audience(self) -> RouteAudience {
        self.audience
    }
}

pub struct HostRegistration {
    namespace: &'static str,
    method: &'static str,
    handler: Arc<dyn HostHandler>,
    kind: HostMethodKind,
    policy: EffectiveRoutePolicy,
}

impl HostRegistration {
    pub fn new(
        namespace: &'static str,
        method: &'static str,
        audience: RouteAudience,
        handler: Arc<dyn HostHandler>,
    ) -> Self {
        Self {
            namespace,
            method,
            handler,
            kind: HostMethodKind::Call,
            policy: EffectiveRoutePolicy::new(audience),
        }
    }

    pub fn stream(
        namespace: &'static str,
        method: &'static str,
        audience: RouteAudience,
        handler: Arc<dyn HostHandler>,
    ) -> Self {
        Self {
            namespace,
            method,
            handler,
            kind: HostMethodKind::Stream,
            policy: EffectiveRoutePolicy::new(audience),
        }
    }

    pub const fn policy(&self) -> EffectiveRoutePolicy {
        self.policy
    }

    pub const fn audience(&self) -> RouteAudience {
        self.policy.audience()
    }
}

/// Host API handler trait - for view layer to call host app capabilities.
///
/// Design constraints:
/// - `input` is owned to avoid capturing borrows in `'static` futures.
/// - `cancel` is reachable so handlers can stop work early.
/// - `call` only constructs a lazy future; it must not perform a side effect
///   before that future is first polled. Bridge admission commits that poll
///   against document revocation and may cancel it before polling begins.
pub trait HostHandler: Send + Sync + 'static {
    fn call<'a>(
        &'a self,
        invocation: HostInvocationContext,
        input: Option<String>,
        cancel: HostCancel,
    ) -> HostFuture<'a>;
}

/// One fully resolved host route in the registry.
///
/// Keep the handler, wire kind, and policy together: a duplicate registration
/// must replace all three values atomically from the registry's perspective.
struct HostRouteRecord {
    handler: Arc<dyn HostHandler>,
    kind: HostMethodKind,
    // Keep policy with the route so registration cannot produce policy-less
    // entries.
    policy: EffectiveRoutePolicy,
}

/// Host API registry.
struct HostRegistry {
    routes: HashMap<String, HostRouteRecord>,
}

impl HostRegistry {
    fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    fn register(&mut self, key: String, route: HostRouteRecord) {
        self.routes.insert(key, route);
    }

    fn route_for_caller(
        &self,
        name: &str,
        caller: &AuthenticatedCaller,
    ) -> Option<&HostRouteRecord> {
        self.routes
            .get(name)
            .filter(|route| authorize(caller, route.policy.audience()))
    }

    fn schema_for_caller(&self, caller: &AuthenticatedCaller) -> HashMap<String, &'static str> {
        self.routes
            .iter()
            .filter(|(_, route)| authorize(caller, route.policy.audience()))
            .map(|(key, route)| {
                let kind = match route.kind {
                    HostMethodKind::Call => "call",
                    HostMethodKind::Stream => "stream",
                };
                (key.clone(), kind)
            })
            .collect()
    }
}

/// Global host API registry instance.
static GLOBAL_HOST_REGISTRY: OnceLock<Mutex<HostRegistry>> = OnceLock::new();

fn get_host_registry() -> &'static Mutex<HostRegistry> {
    GLOBAL_HOST_REGISTRY.get_or_init(|| Mutex::new(HostRegistry::new()))
}

fn validate_host_namespace(namespace: &str) {
    assert_ne!(
        namespace, "channel",
        "host namespace 'channel' is reserved by the JS API; choose a different namespace"
    );
}

pub fn register_host_route(
    namespace: &str,
    method: &str,
    audience: RouteAudience,
    handler: Arc<dyn HostHandler>,
) {
    validate_host_namespace(namespace);
    let key = format!("{namespace}.{method}");
    let registry = get_host_registry();
    let mut reg = registry.lock().unwrap();
    reg.register(
        key,
        HostRouteRecord {
            handler,
            kind: HostMethodKind::Call,
            policy: EffectiveRoutePolicy::new(audience),
        },
    );
}

pub fn register_host(registration: HostRegistration) {
    validate_host_namespace(registration.namespace);
    let key = format!("{}.{}", registration.namespace, registration.method);
    let registry = get_host_registry();
    let mut reg = registry.lock().unwrap();
    reg.register(
        key,
        HostRouteRecord {
            handler: registration.handler,
            kind: registration.kind,
            policy: registration.policy,
        },
    );
}

/// Unified registration entry returned by the `#[native]` macro for all modes
/// (unary, stream, channel). Runtime assembly code dispatches each entry to the
/// correct registry.
pub enum HostRegistrationEntry {
    Handler(HostRegistration),
    Channel(ChannelRegistration),
}

impl HostRegistrationEntry {
    pub const fn policy(&self) -> EffectiveRoutePolicy {
        match self {
            Self::Handler(registration) => registration.policy(),
            Self::Channel(registration) => registration.policy(),
        }
    }

    pub const fn audience(&self) -> RouteAudience {
        self.policy().audience()
    }
}

pub fn register_host_entry(entry: HostRegistrationEntry) {
    match entry {
        HostRegistrationEntry::Handler(reg) => register_host(reg),
        HostRegistrationEntry::Channel(reg) => register_channel_handler(reg),
    }
}

pub(crate) fn get_host_for_caller(
    name: &str,
    caller: &AuthenticatedCaller,
) -> Option<Arc<dyn HostHandler>> {
    let registry = get_host_registry();
    registry
        .lock()
        .unwrap()
        .route_for_caller(name, caller)
        .map(|route| Arc::clone(&route.handler))
}

/// Inspect only immutable route policy. Browser ingress uses this while its
/// lifecycle registry lock is held; cloning a handler is deliberately deferred
/// until after that lock is released.
pub(crate) fn host_route_is_authorized(name: &str, caller: &AuthenticatedCaller) -> bool {
    get_host_registry()
        .lock()
        .unwrap()
        .routes
        .get(name)
        .is_none_or(|route| authorize(caller, route.policy.audience()))
}

/// Returns a map of `"namespace.method"` → `"call"` | `"stream"` for all
/// registered host methods. Included in the handshake `Ready` message so
/// the JS bridge can automatically choose the right wire protocol.
pub fn host_method_schema(caller: &AuthenticatedCaller) -> HashMap<String, &'static str> {
    let registry = get_host_registry();
    let reg = registry.lock().unwrap();
    reg.schema_for_caller(caller)
}

pub fn parse_input<T: DeserializeOwned>(input: Option<&str>) -> HostResult<T> {
    match input {
        Some(json) => serde_json::from_str(json)
            .map_err(|e| LxAppError::InvalidParameter(format!("Invalid input JSON: {}", e))),
        None => Err(LxAppError::InvalidParameter("Missing input".to_string())),
    }
}

pub fn serialize_result<T: Serialize>(result: HostResult<T>) -> HostResult<HostOutput> {
    let value = result?;
    serde_json::to_string(&value)
        .map(HostOutput::Json)
        .map_err(|e| LxAppError::Bridge(e.to_string()))
}

/// Imperative stream context passed to `#[native(..., stream)]` handlers.
///
/// Handlers emit zero or more events with [`send`](Self::send), then finish
/// with [`end`](Self::end) or [`error`](Self::error).
pub struct StreamContext<TEvent, TResult = ()> {
    tx: mpsc::UnboundedSender<HostResult<HostStreamItem>>,
    cancel: HostCancel,
    canceled: bool,
    _marker: PhantomData<fn(TEvent) -> TResult>,
}

impl<TEvent, TResult> StreamContext<TEvent, TResult> {
    /// Resolves when the view cancels the stream.
    pub async fn canceled(&mut self) -> bool {
        if self.canceled {
            return true;
        }
        let _ = (&mut self.cancel).await;
        self.canceled = true;
        true
    }

    #[doc(hidden)]
    pub fn error_sender(&self) -> mpsc::UnboundedSender<HostResult<HostStreamItem>> {
        self.tx.clone()
    }
}

impl<TEvent, TResult> StreamContext<TEvent, TResult>
where
    TEvent: Serialize,
    TResult: Serialize,
{
    /// Emit one event chunk to the view.
    pub fn send(&mut self, event: TEvent) -> HostResult<()> {
        let payload =
            serde_json::to_string(&event).map_err(|e| LxAppError::Bridge(e.to_string()))?;
        self.tx
            .send(Ok(HostStreamItem::Event(payload)))
            .map_err(|_| LxAppError::Bridge("Stream closed".to_string()))
    }

    /// Finish the stream with a final result.
    pub fn end(self, result: TResult) -> HostResult<()> {
        let payload =
            serde_json::to_string(&result).map_err(|e| LxAppError::Bridge(e.to_string()))?;
        self.tx
            .send(Ok(HostStreamItem::Return(payload)))
            .map_err(|_| LxAppError::Bridge("Stream closed".to_string()))
    }

    /// Finish the stream with a structured bridge error.
    pub fn error(self, code: impl Into<String>, message: impl Into<String>) -> HostResult<()> {
        self.tx
            .send(Err(LxAppError::RongJSHost {
                code: code.into(),
                message: message.into(),
                data: None,
            }))
            .map_err(|_| LxAppError::Bridge("Stream closed".to_string()))
    }
}

#[doc(hidden)]
pub fn new_stream_context<TEvent, TResult>(
    cancel: HostCancel,
) -> (
    StreamContext<TEvent, TResult>,
    mpsc::UnboundedReceiver<HostResult<HostStreamItem>>,
) {
    let (tx, rx) = mpsc::unbounded_channel();
    (
        StreamContext {
            tx,
            cancel,
            canceled: false,
            _marker: PhantomData,
        },
        rx,
    )
}

#[doc(hidden)]
pub fn stream_output_from_rx(
    rx: mpsc::UnboundedReceiver<HostResult<HostStreamItem>>,
) -> HostOutput {
    HostOutput::Stream(Box::pin(futures::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    })))
}

pub async fn await_or_cancel<T>(
    cancel: &mut HostCancel,
    fut: impl Future<Output = HostResult<T>>,
) -> HostResult<T> {
    tokio::select! {
        _ = cancel => Err(LxAppError::Bridge("Canceled".to_string()))?,
        res = fut => res,
    }
}

/// Inbound message from the View layer delivered to the channel handler.
pub(crate) enum RawChannelInbound {
    Data(String),
    Close {
        code: Option<String>,
        reason: Option<String>,
    },
}

/// Typed inbound message received from [`ChannelContext::recv_json`].
pub enum ChannelMessage<T> {
    Data(T),
    Close {
        code: Option<String>,
        reason: Option<String>,
    },
}

/// Outbound message from the channel handler to the View layer.
pub(crate) enum ChannelOutbound {
    Data(String),
    Close {
        code: Option<String>,
        reason: Option<String>,
    },
}

/// Context passed to a channel handler when a channel is opened.
///
/// Handlers receive messages via [`recv`](Self::recv) and push messages back
/// via [`send`](Self::send). Dropping or calling [`close`](Self::close) ends
/// the channel from the Logic side.
pub struct ChannelContext<TIn = JsonValue, TOut = TIn> {
    id: String,
    inbound_rx: mpsc::UnboundedReceiver<RawChannelInbound>,
    outbound_tx: mpsc::UnboundedSender<ChannelOutbound>,
    close_on_drop: bool,
    _marker: PhantomData<fn(TIn) -> TOut>,
}

impl<TIn, TOut> ChannelContext<TIn, TOut> {
    /// The channel identifier (matches the `id` field in the wire protocol).
    pub fn id(&self) -> &str {
        &self.id
    }

    #[doc(hidden)]
    pub fn with_types<TNextIn, TNextOut>(mut self) -> ChannelContext<TNextIn, TNextOut> {
        let (dummy_inbound_tx, dummy_inbound_rx) = mpsc::unbounded_channel();
        let (dummy_outbound_tx, _dummy_outbound_rx) = mpsc::unbounded_channel();
        let id = std::mem::take(&mut self.id);
        let inbound_rx = std::mem::replace(&mut self.inbound_rx, dummy_inbound_rx);
        let outbound_tx = std::mem::replace(&mut self.outbound_tx, dummy_outbound_tx);
        let close_on_drop = self.close_on_drop;
        self.close_on_drop = false;
        drop(dummy_inbound_tx);

        ChannelContext {
            id,
            inbound_rx,
            outbound_tx,
            close_on_drop,
            _marker: PhantomData,
        }
    }

    pub(crate) async fn recv_raw(&mut self) -> Option<RawChannelInbound> {
        self.inbound_rx.recv().await
    }

    pub(crate) fn send_raw_json(&self, payload_json: String) -> HostResult<()> {
        self.outbound_tx
            .send(ChannelOutbound::Data(payload_json))
            .map_err(|_| LxAppError::Bridge("Channel closed".to_string()))
    }

    #[doc(hidden)]
    pub fn close_handle(&self) -> ChannelCloseHandle {
        ChannelCloseHandle {
            outbound_tx: self.outbound_tx.clone(),
        }
    }

    #[doc(hidden)]
    pub fn disable_close_on_drop(&mut self) {
        self.close_on_drop = false;
    }
}

impl<TIn, TOut> ChannelContext<TIn, TOut>
where
    TIn: DeserializeOwned,
    TOut: Serialize,
{
    /// Receive the next inbound message from the view.
    ///
    /// Returns `None` when the channel has been closed from the View side or
    /// the session was reset.
    pub async fn recv(&mut self) -> HostResult<Option<ChannelMessage<TIn>>> {
        match self.recv_raw().await {
            Some(RawChannelInbound::Data(payload_json)) => {
                let payload = serde_json::from_str(&payload_json).map_err(|e| {
                    LxAppError::InvalidParameter(format!("Invalid channel payload JSON: {}", e))
                })?;
                Ok(Some(ChannelMessage::Data(payload)))
            }
            Some(RawChannelInbound::Close { code, reason }) => {
                Ok(Some(ChannelMessage::Close { code, reason }))
            }
            None => Ok(None),
        }
    }

    /// Send a JSON-serialisable payload to the view.
    pub fn send(&self, payload: TOut) -> HostResult<()> {
        let payload_json =
            serde_json::to_string(&payload).map_err(|e| LxAppError::Bridge(e.to_string()))?;
        self.send_raw_json(payload_json)
    }
}

impl<TIn, TOut> ChannelContext<TIn, TOut> {
    /// Close the channel cleanly from the Logic side.
    pub fn close(mut self) {
        self.close_on_drop = false;
        let _ = self.outbound_tx.send(ChannelOutbound::Close {
            code: None,
            reason: None,
        });
    }

    /// Close the channel with an error code and human-readable reason.
    pub fn close_with(mut self, code: impl Into<String>, reason: impl Into<String>) {
        self.close_on_drop = false;
        let _ = self.outbound_tx.send(ChannelOutbound::Close {
            code: Some(code.into()),
            reason: Some(reason.into()),
        });
    }
}

impl<TIn, TOut> Drop for ChannelContext<TIn, TOut> {
    fn drop(&mut self) {
        if !self.close_on_drop {
            return;
        }
        let _ = self.outbound_tx.send(ChannelOutbound::Close {
            code: None,
            reason: None,
        });
    }
}

#[doc(hidden)]
pub struct ChannelCloseHandle {
    outbound_tx: mpsc::UnboundedSender<ChannelOutbound>,
}

impl ChannelCloseHandle {
    pub fn close(&self) {
        let _ = self.outbound_tx.send(ChannelOutbound::Close {
            code: None,
            reason: None,
        });
    }

    pub fn close_with(&self, code: impl Into<String>, reason: impl Into<String>) {
        let _ = self.outbound_tx.send(ChannelOutbound::Close {
            code: Some(code.into()),
            reason: Some(reason.into()),
        });
    }
}

/// Bridge-internal sender half for a host channel. Held in `PageBridgeState`
/// so inbound wire messages can be forwarded to the handler's `ChannelContext`.
pub(crate) struct ChannelContextSender {
    inbound_tx: mpsc::UnboundedSender<RawChannelInbound>,
}

impl ChannelContextSender {
    pub(crate) fn send_data(&self, payload_json: String) {
        let _ = self.inbound_tx.send(RawChannelInbound::Data(payload_json));
    }

    pub(crate) fn send_close(&self, code: Option<String>, reason: Option<String>) {
        let _ = self
            .inbound_tx
            .send(RawChannelInbound::Close { code, reason });
    }
}

/// Channel handler trait — invoked when a View opens a host channel.
pub trait ChannelHandler: Send + Sync + 'static {
    /// Called once when the channel is opened. The implementation must spawn
    /// its own async task if it needs to do async work (e.g. via
    /// `tokio::task::spawn`). The method is synchronous so the bridge is not
    /// blocked waiting for the handler.
    fn on_open(
        &self,
        invocation: HostInvocationContext,
        ctx: ChannelContext,
        params: Option<String>,
    );
}

/// A channel handler ready to be inserted into the global channel registry.
pub struct ChannelRegistration {
    namespace: &'static str,
    method: &'static str,
    handler: Arc<dyn ChannelHandler>,
    policy: EffectiveRoutePolicy,
}

impl ChannelRegistration {
    pub fn new(
        namespace: &'static str,
        method: &'static str,
        audience: RouteAudience,
        handler: Arc<dyn ChannelHandler>,
    ) -> Self {
        Self {
            namespace,
            method,
            handler,
            policy: EffectiveRoutePolicy::new(audience),
        }
    }

    pub const fn policy(&self) -> EffectiveRoutePolicy {
        self.policy
    }

    pub const fn audience(&self) -> RouteAudience {
        self.policy.audience()
    }
}

struct ChannelRouteRecord {
    handler: Arc<dyn ChannelHandler>,
    // Channel admission is wired with request/notification authorization.
    policy: EffectiveRoutePolicy,
}

struct ChannelRegistry {
    routes: HashMap<String, ChannelRouteRecord>,
}

impl ChannelRegistry {
    fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    fn register(&mut self, key: String, route: ChannelRouteRecord) {
        self.routes.insert(key, route);
    }
}

static GLOBAL_CHANNEL_REGISTRY: OnceLock<Mutex<ChannelRegistry>> = OnceLock::new();

fn get_channel_registry() -> &'static Mutex<ChannelRegistry> {
    GLOBAL_CHANNEL_REGISTRY.get_or_init(|| Mutex::new(ChannelRegistry::new()))
}

pub fn register_channel_handler(registration: ChannelRegistration) {
    validate_host_namespace(registration.namespace);
    let key = format!("{}.{}", registration.namespace, registration.method);
    get_channel_registry().lock().unwrap().register(
        key,
        ChannelRouteRecord {
            handler: registration.handler,
            policy: registration.policy,
        },
    );
}

pub(crate) fn get_channel_handler_for_caller(
    name: &str,
    caller: &AuthenticatedCaller,
) -> Option<Arc<dyn ChannelHandler>> {
    get_channel_registry()
        .lock()
        .unwrap()
        .routes
        .get(name)
        .filter(|route| authorize(caller, route.policy.audience()))
        .map(|route| Arc::clone(&route.handler))
}

/// See [`host_route_is_authorized`]. Unknown channels remain eligible for the
/// normal post-lock "not found" response.
pub(crate) fn channel_route_is_authorized(name: &str, caller: &AuthenticatedCaller) -> bool {
    get_channel_registry()
        .lock()
        .unwrap()
        .routes
        .get(name)
        .is_none_or(|route| authorize(caller, route.policy.audience()))
}

/// Create a linked `(ChannelContext, ChannelContextSender, outbound_rx)` triple.
///
/// - `ChannelContext` goes to the handler.
/// - `ChannelContextSender` is stored in `PageBridgeState`.
/// - `outbound_rx` is consumed by the bridge's outbound forwarding task.
pub(crate) fn new_channel_context(
    id: String,
) -> (
    ChannelContext,
    ChannelContextSender,
    mpsc::UnboundedReceiver<ChannelOutbound>,
) {
    let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
    let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();
    let ctx = ChannelContext {
        id,
        inbound_rx,
        outbound_tx,
        close_on_drop: true,
        _marker: PhantomData,
    };
    let sender = ChannelContextSender { inbound_tx };
    (ctx, sender, outbound_rx)
}

/// Register built-in Host API set.
///
/// This is invoked once from lxapp initialization so Host API definitions are owned
/// by `lingxia-lxapp` (not `lingxia-logic` or the host app).
pub(crate) fn register_all() {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        device::register_all();
        navigation::register_all();
        navigator::register_all();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appservice::LxAppWorkers;
    use crate::register_synthetic_lxapp;
    use lingxia_platform::Platform;
    use uuid::Uuid;

    fn same_app_id_with_different_classes() -> (tempfile::TempDir, Arc<LxApp>, Arc<LxApp>) {
        let root = tempfile::tempdir().expect("test app root");
        let runtime = Arc::new(
            Platform::new(
                root.path().join("data").display().to_string(),
                root.path().join("cache").display().to_string(),
                "en-US".to_string(),
            )
            .expect("test platform"),
        );
        let workers = LxAppWorkers::init(1);
        let app_id = format!("app.lingxia.scope-test.{}", Uuid::new_v4());
        register_synthetic_lxapp(app_id.clone());
        let mut standard = LxApp::new_with_session_class_for_test(
            app_id.clone(),
            Arc::clone(&runtime),
            Arc::clone(&workers),
            AppSessionClass::StandardApp,
        )
        .expect("standard app");
        standard.config.security.privileges = vec![
            "process".to_string(),
            "downloads".to_string(),
            "automation".to_string(),
            "host".to_string(),
        ];
        let standard = Arc::new(standard);
        standard.bind_arc();
        standard.set_status(LxAppSessionStatus::Opened);
        let mut control = LxApp::new_with_session_class_for_test(
            app_id,
            runtime,
            workers,
            AppSessionClass::ControlApp,
        )
        .expect("control app");
        control.config.security.privileges = vec![
            "process".to_string(),
            "downloads".to_string(),
            "automation".to_string(),
            "host".to_string(),
        ];
        let control = Arc::new(control);
        control.bind_arc();
        control.set_status(LxAppSessionStatus::Opened);
        (root, standard, control)
    }

    struct TestHostHandler;

    impl HostHandler for TestHostHandler {
        fn call<'a>(
            &'a self,
            _invocation: HostInvocationContext,
            _input: Option<String>,
            _cancel: HostCancel,
        ) -> HostFuture<'a> {
            Box::pin(async { Ok(HostOutput::Json("null".to_string())) })
        }
    }

    struct TestChannelHandler;

    impl ChannelHandler for TestChannelHandler {
        fn on_open(
            &self,
            _invocation: HostInvocationContext,
            _ctx: ChannelContext,
            _params: Option<String>,
        ) {
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    struct ObservedScope {
        class: AppSessionClass,
        app_id: String,
        session_id: u64,
    }

    fn observe_scope(invocation: &HostInvocationContext) -> ObservedScope {
        let AuthenticatedCaller::LxAppSession { class, scope } = invocation.caller() else {
            panic!("expected lxapp caller");
        };
        ObservedScope {
            class: *class,
            app_id: scope.identity().app_id().to_string(),
            session_id: scope.identity().session_id(),
        }
    }

    struct ScopeRecordingHostHandler {
        observed: Arc<Mutex<Vec<ObservedScope>>>,
    }

    impl HostHandler for ScopeRecordingHostHandler {
        fn call<'a>(
            &'a self,
            invocation: HostInvocationContext,
            _input: Option<String>,
            _cancel: HostCancel,
        ) -> HostFuture<'a> {
            Box::pin(async move {
                self.observed
                    .lock()
                    .unwrap()
                    .push(observe_scope(&invocation));
                Ok(HostOutput::Json("null".to_string()))
            })
        }
    }

    struct ScopeRecordingChannelHandler {
        observed: Arc<Mutex<Vec<ObservedScope>>>,
    }

    impl ChannelHandler for ScopeRecordingChannelHandler {
        fn on_open(
            &self,
            invocation: HostInvocationContext,
            _ctx: ChannelContext,
            _params: Option<String>,
        ) {
            self.observed
                .lock()
                .unwrap()
                .push(observe_scope(&invocation));
        }
    }

    #[tokio::test]
    async fn invocation_context_carries_native_scope_to_unary_stream_and_channel_handlers() {
        let (_root, standard, control) = same_app_id_with_different_classes();
        let standard_caller = AuthenticatedCaller::for_lxapp(&standard);
        let control_caller = AuthenticatedCaller::for_lxapp(&control);

        assert!(
            HostInvocationContext::for_dispatch(Arc::clone(&control), &standard_caller).is_none()
        );

        let host_observed = Arc::new(Mutex::new(Vec::new()));
        let host_handler = Arc::new(ScopeRecordingHostHandler {
            observed: Arc::clone(&host_observed),
        });
        let stream_registration = HostRegistration::stream(
            "scope",
            "stream",
            RouteAudience::AppSessionOnly,
            host_handler.clone(),
        );
        assert_eq!(stream_registration.kind, HostMethodKind::Stream);

        for (app, caller) in [
            (Arc::clone(&standard), &standard_caller),
            (Arc::clone(&control), &control_caller),
        ] {
            let invocation =
                HostInvocationContext::for_dispatch(app, caller).expect("matching scope");
            let (_cancel_tx, cancel) = oneshot::channel();
            host_handler
                .call(invocation, None, cancel)
                .await
                .expect("handler result");
        }

        let channel_observed = Arc::new(Mutex::new(Vec::new()));
        let channel_handler = ScopeRecordingChannelHandler {
            observed: Arc::clone(&channel_observed),
        };
        for (index, (app, caller)) in [
            (Arc::clone(&standard), &standard_caller),
            (Arc::clone(&control), &control_caller),
        ]
        .into_iter()
        .enumerate()
        {
            let invocation =
                HostInvocationContext::for_dispatch(app, caller).expect("matching scope");
            let (channel, _sender, _outbound) = new_channel_context(format!("scope-{index}"));
            channel_handler.on_open(invocation, channel, None);
        }

        let host_observed = host_observed.lock().unwrap();
        let channel_observed = channel_observed.lock().unwrap();
        assert_eq!(host_observed.as_slice(), channel_observed.as_slice());
        assert_eq!(host_observed.len(), 2);
        assert_eq!(host_observed[0].app_id, host_observed[1].app_id);
        assert_ne!(host_observed[0].session_id, host_observed[1].session_id);
        assert_eq!(host_observed[0].class, AppSessionClass::StandardApp);
        assert_eq!(host_observed[1].class, AppSessionClass::ControlApp);
    }

    #[test]
    fn same_app_id_does_not_share_storage_or_native_resource_grants_between_sessions() {
        let (_root, standard, control) = same_app_id_with_different_classes();
        let standard_caller = AuthenticatedCaller::for_lxapp(&standard);
        let control_caller = AuthenticatedCaller::for_lxapp(&control);
        let standard_scope = standard_caller.app_scope().expect("standard scope");
        let control_scope = control_caller.app_scope().expect("control scope");

        assert_eq!(
            standard_scope.identity().app_id(),
            control_scope.identity().app_id()
        );
        assert_ne!(
            standard_scope.identity().session_id(),
            control_scope.identity().session_id()
        );
        assert_eq!(standard_scope.storage().user_data(), standard.user_data_dir);
        assert_eq!(control_scope.storage().temporary(), control.temp_dir);

        let file = standard.temp_dir.join("native-grant.txt");
        std::fs::write(&file, b"scope-owned").expect("write grant fixture");
        let granted = standard
            .grant_transient_file_access(&file)
            .expect("native grant")
            .to_string();
        assert_eq!(
            standard_scope
                .resource_grants()
                .resolve_transient_file(&granted)
                .expect("owner resolves grant"),
            file
        );
        assert!(
            control_scope
                .resource_grants()
                .resolve_transient_file(&granted)
                .is_err(),
            "same app id must not confer another session's native grant"
        );
    }

    #[test]
    fn privileged_resource_grants_are_native_session_bound_and_expire_on_teardown() {
        let (_root, standard, takeover) = same_app_id_with_different_classes();
        let declared_downloads = crate::LxAppSecurityPrivilege::new("downloads").unwrap();
        let declared_host = crate::LxAppSecurityPrivilege::new("host").unwrap();

        assert!(standard.has_security_privilege(&declared_downloads));
        assert!(standard.has_security_privilege(&declared_host));
        assert!(!standard.has_resource_grant(AppResourceGrant::Downloads));
        assert!(!standard.has_resource_grant(AppResourceGrant::AutomationHost));

        standard.seal_resource_grants(HashSet::from([
            AppResourceGrant::Downloads,
            AppResourceGrant::AutomationHost,
        ]));
        assert!(standard.has_resource_grant(AppResourceGrant::Downloads));
        assert!(standard.has_resource_grant(AppResourceGrant::AutomationHost));

        let scope = AuthenticatedCaller::for_lxapp(&standard)
            .app_scope()
            .expect("app scope")
            .clone();
        assert!(
            scope
                .resource_grants()
                .contains(AppResourceGrant::Downloads)
        );
        assert!(
            !takeover.has_resource_grant(AppResourceGrant::Downloads),
            "same app id on a different native session must not inherit grants"
        );

        standard.set_status(LxAppSessionStatus::Closing);
        assert!(!standard.has_resource_grant(AppResourceGrant::Downloads));
        assert!(
            !scope
                .resource_grants()
                .contains(AppResourceGrant::AutomationHost),
            "retained resource handles must fail as teardown begins"
        );
    }

    #[test]
    fn host_route_inventory_keeps_kind_and_policy_together_on_overwrite() {
        let mut registry = HostRegistry::new();
        registry.register(
            "test.route".to_string(),
            HostRouteRecord {
                handler: Arc::new(TestHostHandler),
                kind: HostMethodKind::Call,
                policy: EffectiveRoutePolicy::new(RouteAudience::AppSessionOnly),
            },
        );
        registry.register(
            "test.route".to_string(),
            HostRouteRecord {
                handler: Arc::new(TestHostHandler),
                kind: HostMethodKind::Stream,
                policy: EffectiveRoutePolicy::new(RouteAudience::ControlAppOnly),
            },
        );

        assert_eq!(registry.routes.len(), 1);
        let route = registry.routes.get("test.route").expect("registered route");
        assert_eq!(route.kind, HostMethodKind::Stream);
        assert_eq!(route.policy.audience(), RouteAudience::ControlAppOnly);
    }

    #[test]
    fn channel_route_inventory_keeps_policy_on_overwrite() {
        let mut registry = ChannelRegistry::new();
        registry.register(
            "test.channel".to_string(),
            ChannelRouteRecord {
                handler: Arc::new(TestChannelHandler),
                policy: EffectiveRoutePolicy::new(RouteAudience::AppSessionOnly),
            },
        );
        registry.register(
            "test.channel".to_string(),
            ChannelRouteRecord {
                handler: Arc::new(TestChannelHandler),
                policy: EffectiveRoutePolicy::new(RouteAudience::BrowserControlOnly),
            },
        );

        assert_eq!(registry.routes.len(), 1);
        let route = registry
            .routes
            .get("test.channel")
            .expect("registered route");
        assert_eq!(route.policy.audience(), RouteAudience::BrowserControlOnly);
    }

    #[test]
    fn registration_entry_exposes_its_effective_policy() {
        let handler = HostRegistrationEntry::Handler(HostRegistration::new(
            "test",
            "call",
            RouteAudience::ControlAppOnly,
            Arc::new(TestHostHandler),
        ));
        let channel = HostRegistrationEntry::Channel(ChannelRegistration::new(
            "test",
            "channel",
            RouteAudience::ControlOnly,
            Arc::new(TestChannelHandler),
        ));

        assert_eq!(handler.audience(), RouteAudience::ControlAppOnly);
        assert_eq!(handler.policy().audience(), RouteAudience::ControlAppOnly);
        assert_eq!(channel.audience(), RouteAudience::ControlOnly);
        assert_eq!(channel.policy().audience(), RouteAudience::ControlOnly);
    }

    #[test]
    fn audience_matrix_rejects_same_scope_standard_caller_for_control_routes() {
        let standard = AuthenticatedCaller::standard_for_test(1);
        let control = AuthenticatedCaller::control_for_test(1);
        let (_, authority) =
            crate::issue_control_document_bootstrap(&ring::rand::SystemRandom::new())
                .expect("native entropy");
        let browser = AuthenticatedCaller::active_browser_document(authority);

        let audiences = [
            RouteAudience::AppSessionOnly,
            RouteAudience::AuthenticatedReadOnly,
            RouteAudience::ControlAppOnly,
            RouteAudience::BrowserControlOnly,
            RouteAudience::ControlOnly,
        ];
        assert_eq!(
            audiences.map(|audience| authorize(&standard, audience)),
            [true, true, false, false, false]
        );
        assert_eq!(
            audiences.map(|audience| authorize(&control, audience)),
            [true, true, true, false, true]
        );
        assert_eq!(
            audiences.map(|audience| authorize(&browser, audience)),
            [false, true, false, true, true]
        );
    }

    #[test]
    fn schema_visibility_matches_dispatch_for_every_audience_and_caller_class() {
        let (_, authority) =
            crate::issue_control_document_bootstrap(&ring::rand::SystemRandom::new())
                .expect("native entropy");
        let callers = [
            AuthenticatedCaller::standard_for_test(42),
            AuthenticatedCaller::control_for_test(42),
            AuthenticatedCaller::active_browser_document(authority),
        ];
        let audiences = [
            RouteAudience::AppSessionOnly,
            RouteAudience::AuthenticatedReadOnly,
            RouteAudience::ControlAppOnly,
            RouteAudience::BrowserControlOnly,
            RouteAudience::ControlOnly,
        ];
        let mut registry = HostRegistry::new();
        for (index, audience) in audiences.into_iter().enumerate() {
            registry.register(
                format!("test.route{index}"),
                HostRouteRecord {
                    handler: Arc::new(TestHostHandler),
                    kind: HostMethodKind::Call,
                    policy: EffectiveRoutePolicy::new(audience),
                },
            );
        }

        for caller in &callers {
            let schema = registry.schema_for_caller(caller);
            for index in 0..audiences.len() {
                let name = format!("test.route{index}");
                assert_eq!(
                    schema.contains_key(&name),
                    registry.route_for_caller(&name, caller).is_some(),
                    "schema and dispatch diverged for {name}",
                );
            }
        }
    }
}
