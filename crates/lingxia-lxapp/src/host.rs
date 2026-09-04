//! Host API runtime and extension surface.
//!
//! Public host capabilities and runtime-owned built-in WebUI capabilities use
//! separate registry scopes. External crates can define public handlers and
//! register them here.

use crate::error::LxAppError;
use crate::lxapp::LxApp;

use futures::Stream;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};
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

pub struct HostRegistration {
    pub namespace: &'static str,
    pub method: &'static str,
    pub handler: Arc<dyn HostHandler>,
    pub kind: HostMethodKind,
}

impl HostRegistration {
    pub fn new(
        namespace: &'static str,
        method: &'static str,
        handler: Arc<dyn HostHandler>,
    ) -> Self {
        Self {
            namespace,
            method,
            handler,
            kind: HostMethodKind::Call,
        }
    }

    pub fn stream(
        namespace: &'static str,
        method: &'static str,
        handler: Arc<dyn HostHandler>,
    ) -> Self {
        Self {
            namespace,
            method,
            handler,
            kind: HostMethodKind::Stream,
        }
    }
}

/// Host API handler trait - for view layer to call host app capabilities.
///
/// Design constraints:
/// - `input` is owned to avoid capturing borrows in `'static` futures.
/// - `cancel` is reachable so handlers can stop work early.
pub trait HostHandler: Send + Sync + 'static {
    fn call<'a>(
        &'a self,
        lxapp: Arc<LxApp>,
        input: Option<String>,
        cancel: HostCancel,
    ) -> HostFuture<'a>;
}

struct RegisteredHostHandler {
    handler: Arc<dyn HostHandler>,
    kind: HostMethodKind,
}

/// Host API registry. Public handlers are visible to every lxapp; built-in
/// handlers are keyed by the runtime-owned app that is allowed to see them.
struct HostRegistry {
    public: HashMap<String, RegisteredHostHandler>,
    builtins: HashMap<String, HashMap<String, RegisteredHostHandler>>,
}

impl HostRegistry {
    fn new() -> Self {
        Self {
            public: HashMap::new(),
            builtins: HashMap::new(),
        }
    }

    fn insert_public(&mut self, key: String, handler: RegisteredHostHandler) {
        self.public.insert(key, handler);
    }

    fn insert_builtin(&mut self, appid: &str, key: String, handler: RegisteredHostHandler) {
        self.builtins
            .entry(appid.to_string())
            .or_default()
            .insert(key, handler);
    }

    fn handler(&self, name: &str, builtin_appid: Option<&str>) -> Option<Arc<dyn HostHandler>> {
        builtin_appid
            .and_then(|appid| self.builtins.get(appid))
            .and_then(|handlers| handlers.get(name))
            .or_else(|| self.public.get(name))
            .map(|registered| registered.handler.clone())
    }

    fn method_schema(&self, builtin_appid: Option<&str>) -> HashMap<String, &'static str> {
        let mut schema = self
            .public
            .iter()
            .map(|(name, registered)| (name.clone(), method_kind_name(registered.kind)))
            .collect::<HashMap<_, _>>();
        if let Some(handlers) = builtin_appid.and_then(|appid| self.builtins.get(appid)) {
            schema.extend(
                handlers
                    .iter()
                    .map(|(name, registered)| (name.clone(), method_kind_name(registered.kind))),
            );
        }
        schema
    }
}

fn method_kind_name(kind: HostMethodKind) -> &'static str {
    match kind {
        HostMethodKind::Call => "call",
        HostMethodKind::Stream => "stream",
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

pub fn register_host_route(namespace: &str, method: &str, handler: Arc<dyn HostHandler>) {
    validate_host_namespace(namespace);
    let key = format!("{namespace}.{method}");
    let registry = get_host_registry();
    let mut reg = registry.lock().unwrap();
    reg.insert_public(
        key,
        RegisteredHostHandler {
            handler,
            kind: HostMethodKind::Call,
        },
    );
}

pub fn register_host(registration: HostRegistration) {
    validate_host_namespace(registration.namespace);
    let key = format!("{}.{}", registration.namespace, registration.method);
    let registry = get_host_registry();
    let mut reg = registry.lock().unwrap();
    reg.insert_public(
        key,
        RegisteredHostHandler {
            handler: registration.handler,
            kind: registration.kind,
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

pub fn register_host_entry(entry: HostRegistrationEntry) {
    match entry {
        HostRegistrationEntry::Handler(reg) => register_host(reg),
        HostRegistrationEntry::Channel(reg) => register_channel_handler(reg),
    }
}

/// Register a Host API that is visible only to a runtime-owned built-in lxapp.
///
/// The app id selects the private namespace, but does not grant access by
/// itself: dispatch also verifies that the caller was constructed from a
/// built-in asset or synthetic runtime source.
#[doc(hidden)]
pub fn register_builtin_host_entry(appid: &str, entry: HostRegistrationEntry) {
    match entry {
        HostRegistrationEntry::Handler(registration) => {
            validate_host_namespace(registration.namespace);
            let key = format!("{}.{}", registration.namespace, registration.method);
            get_host_registry().lock().unwrap().insert_builtin(
                appid,
                key,
                RegisteredHostHandler {
                    handler: registration.handler,
                    kind: registration.kind,
                },
            );
        }
        HostRegistrationEntry::Channel(registration) => {
            register_builtin_channel_handler(appid, registration)
        }
    }
}

fn builtin_caller_appid(lxapp: &LxApp) -> Option<&str> {
    builtin_appid_for_source(&lxapp.appid, &lxapp.bundle_source)
}

fn builtin_appid_for_source<'a>(
    appid: &'a str,
    source: &crate::lxapp::LxAppBundleSource,
) -> Option<&'a str> {
    matches!(
        source,
        crate::lxapp::LxAppBundleSource::BuiltinAssets | crate::lxapp::LxAppBundleSource::Synthetic
    )
    .then_some(appid)
}

pub(crate) fn get_host(name: &str, lxapp: &LxApp) -> Option<Arc<dyn HostHandler>> {
    let registry = get_host_registry();
    registry
        .lock()
        .unwrap()
        .handler(name, builtin_caller_appid(lxapp))
}

/// Returns a map of public `"namespace.method"` → `"call"` | `"stream"`
/// registrations.
pub fn host_method_schema() -> HashMap<String, &'static str> {
    get_host_registry().lock().unwrap().method_schema(None)
}

/// Returns the public methods plus private built-in methods visible to this
/// caller. Used by the bridge handshake without advertising built-in WebUI
/// capabilities to ordinary lxapps.
pub(crate) fn host_method_schema_for(lxapp: &LxApp) -> HashMap<String, &'static str> {
    let registry = get_host_registry();
    registry
        .lock()
        .unwrap()
        .method_schema(builtin_caller_appid(lxapp))
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
    fn on_open(&self, lxapp: Arc<LxApp>, ctx: ChannelContext, params: Option<String>);
}

/// A channel handler ready to be inserted into the global channel registry.
pub struct ChannelRegistration {
    pub namespace: &'static str,
    pub method: &'static str,
    pub handler: Arc<dyn ChannelHandler>,
}

impl ChannelRegistration {
    pub fn new(
        namespace: &'static str,
        method: &'static str,
        handler: Arc<dyn ChannelHandler>,
    ) -> Self {
        Self {
            namespace,
            method,
            handler,
        }
    }
}

struct ChannelRegistry {
    public: HashMap<String, Arc<dyn ChannelHandler>>,
    builtins: HashMap<String, HashMap<String, Arc<dyn ChannelHandler>>>,
}

impl ChannelRegistry {
    fn new() -> Self {
        Self {
            public: HashMap::new(),
            builtins: HashMap::new(),
        }
    }
}

static GLOBAL_CHANNEL_REGISTRY: OnceLock<Mutex<ChannelRegistry>> = OnceLock::new();

fn get_channel_registry() -> &'static Mutex<ChannelRegistry> {
    GLOBAL_CHANNEL_REGISTRY.get_or_init(|| Mutex::new(ChannelRegistry::new()))
}

pub fn register_channel_handler(registration: ChannelRegistration) {
    validate_host_namespace(registration.namespace);
    let key = format!("{}.{}", registration.namespace, registration.method);
    get_channel_registry()
        .lock()
        .unwrap()
        .public
        .insert(key, registration.handler);
}

fn register_builtin_channel_handler(appid: &str, registration: ChannelRegistration) {
    validate_host_namespace(registration.namespace);
    let key = format!("{}.{}", registration.namespace, registration.method);
    get_channel_registry()
        .lock()
        .unwrap()
        .builtins
        .entry(appid.to_string())
        .or_default()
        .insert(key, registration.handler);
}

pub(crate) fn get_channel_handler(name: &str, lxapp: &LxApp) -> Option<Arc<dyn ChannelHandler>> {
    let registry = get_channel_registry();
    let registry = registry.lock().unwrap();
    builtin_caller_appid(lxapp)
        .and_then(|appid| registry.builtins.get(appid))
        .and_then(|handlers| handlers.get(name))
        .or_else(|| registry.public.get(name))
        .cloned()
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

    struct NoopHandler;

    impl HostHandler for NoopHandler {
        fn call<'a>(
            &'a self,
            _lxapp: Arc<LxApp>,
            _input: Option<String>,
            _cancel: HostCancel,
        ) -> HostFuture<'a> {
            Box::pin(async { Ok(HostOutput::Json("null".to_string())) })
        }
    }

    fn registered(kind: HostMethodKind) -> RegisteredHostHandler {
        RegisteredHostHandler {
            handler: Arc::new(NoopHandler),
            kind,
        }
    }

    #[test]
    fn builtin_routes_are_absent_without_the_matching_scope() {
        let mut registry = HostRegistry::new();
        registry.insert_public("shared.read".to_string(), registered(HostMethodKind::Call));
        registry.insert_builtin(
            "app.lingxia.browser",
            "settings.watch".to_string(),
            registered(HostMethodKind::Stream),
        );

        let ordinary = registry.method_schema(None);
        assert_eq!(ordinary.get("shared.read"), Some(&"call"));
        assert!(!ordinary.contains_key("settings.watch"));
        assert!(registry.handler("settings.watch", None).is_none());

        let wrong_builtin = registry.method_schema(Some("app.lingxia.other"));
        assert!(!wrong_builtin.contains_key("settings.watch"));
        assert!(
            registry
                .handler("settings.watch", Some("app.lingxia.other"))
                .is_none()
        );

        let browser = registry.method_schema(Some("app.lingxia.browser"));
        assert_eq!(browser.get("settings.watch"), Some(&"stream"));
        assert!(
            registry
                .handler("settings.watch", Some("app.lingxia.browser"))
                .is_some()
        );
    }

    #[test]
    fn builtin_route_can_coexist_with_the_same_public_name() {
        let mut registry = HostRegistry::new();
        registry.insert_public(
            "settings.read".to_string(),
            registered(HostMethodKind::Call),
        );
        registry.insert_builtin(
            "app.lingxia.browser",
            "settings.read".to_string(),
            registered(HostMethodKind::Stream),
        );

        assert_eq!(
            registry.method_schema(None).get("settings.read"),
            Some(&"call")
        );
        assert_eq!(
            registry
                .method_schema(Some("app.lingxia.browser"))
                .get("settings.read"),
            Some(&"stream")
        );
    }

    #[test]
    fn an_appid_does_not_make_an_installed_or_dev_bundle_builtin() {
        use crate::lxapp::LxAppBundleSource;
        use std::path::PathBuf;

        let appid = "app.lingxia.browser";
        assert_eq!(
            builtin_appid_for_source(appid, &LxAppBundleSource::BuiltinAssets),
            Some(appid)
        );
        assert_eq!(
            builtin_appid_for_source(appid, &LxAppBundleSource::Synthetic),
            Some(appid)
        );
        assert_eq!(
            builtin_appid_for_source(appid, &LxAppBundleSource::Installed),
            None
        );
        assert_eq!(
            builtin_appid_for_source(
                appid,
                &LxAppBundleSource::DevPath {
                    root: PathBuf::from("browser-webui"),
                },
            ),
            None
        );
    }
}
