//! Bridge — bidirectional message layer between WebView and native backends.
//!
//! ```text
//!                    WebView (MessagePort)
//!                       ↕ JSON-RPC
//!              ┌── bridge ──────────────────────┐
//!              │  bridge.rs          — routing  │
//!              │  bridge/protocol.rs — wire fmt │
//!              └───────────────────────────────┘
//!                ↕ host.*            ↕ others
//!          Rust host registry   AppServiceBackend
//! ```

pub(crate) mod probe;
mod protocol;

#[cfg(feature = "js-appservice")]
pub(crate) use protocol::JsonPatchOp;

use protocol::*;

use crate::LxAppError;
use crate::host::{self, HostOutput, HostStream, HostStreamItem};
use crate::lxapp::LxApp;
use crate::page::PageInstance;
use base64::Engine;
use futures::StreamExt;
use lingxia_webview::{
    DocumentBinding, DocumentGeneration, DocumentOutboundGate, IncomingWebMessage,
    WebMessageContext,
};
use serde::Serialize;
use serde_json::Value;
use serde_json::value::RawValue;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

enum BridgeMessageContext<'a> {
    Web(&'a WebMessageContext),
    Gpu,
}

// AppServiceCommand — the bridge-level message routed to the JS runtime backend
pub(crate) enum AppServiceCommand {
    Ready {
        work_id: Option<SessionWorkId>,
        outbound: Option<OutboundContext>,
    },
    StateSnapshot {
        work_id: Option<SessionWorkId>,
        outbound: Option<OutboundContext>,
        id: String,
        scope: Option<String>,
    },
    Req {
        work_id: Option<SessionWorkId>,
        outbound: Option<OutboundContext>,
        id: String,
        method: String,
        params: Option<Payload>,
        cancel_rx: oneshot::Receiver<()>,
        pending_request: PendingRequestGuard,
        /// When the View queued it (`probe::now_us`), 0 when unknown.
        sent_us: u64,
    },
    Notify {
        work_id: Option<SessionWorkId>,
        outbound: Option<OutboundContext>,
        method: String,
        params_json: Option<String>,
    },
    ChOpen {
        work_id: Option<SessionWorkId>,
        outbound: Option<OutboundContext>,
        id: String,
        topic: String,
        params: Option<Payload>,
    },
    ChData {
        work_id: Option<SessionWorkId>,
        id: String,
        payload: Payload,
    },
    ChClose {
        work_id: Option<SessionWorkId>,
        id: String,
        code: Option<String>,
        reason: Option<String>,
    },
    StateAck {
        work_id: Option<SessionWorkId>,
        scope: Option<String>,
        rev: u64,
    },
    /// A GPU View dropped a patch it could not apply and waits for a snapshot.
    StateResync {
        work_id: Option<SessionWorkId>,
        outbound: Option<OutboundContext>,
    },
    /// Internal lifecycle transitions, never decoded from a document frame.
    BeginSessionWork {
        work_id: SessionWorkId,
    },
    CancelSessionWork {
        work_id: SessionWorkId,
    },
}

/// A View's request, from a WebView frame or a GPU View.
struct Call {
    id: String,
    method: String,
    cap: String,
    params: Option<Payload>,
    /// When the View queued it (`probe::now_us`).
    sent_us: u64,
}

/// Logic data from a View: JSON text from a WebView frame, an owned tree from a GPU View.
#[derive(Debug)]
pub(crate) enum Payload {
    Json(String),
    Tree(Value),
}

impl Payload {
    /// For consumers whose input is JSON text (host routes): a tree is serialized only here.
    pub(crate) fn into_json(self) -> String {
        match self {
            Self::Json(json) => json,
            Self::Tree(value) => value.to_string(),
        }
    }

    pub(crate) fn is_null(&self) -> bool {
        match self {
            Self::Json(json) => json == "null",
            Self::Tree(value) => value.is_null(),
        }
    }

    /// For a GPU View: JSON text (a host route's result) is parsed only here.
    fn into_tree(self) -> Result<Value, LxAppError> {
        match self {
            Self::Json(json) => Ok(serde_json::from_str(&json)?),
            Self::Tree(value) => Ok(value),
        }
    }
}

// AppServiceBackend — trait to decouple bridge routing from the JS runtime executor
pub(crate) trait AppServiceBackend: Send + Sync {
    fn forward(
        &self,
        lxapp: Arc<LxApp>,
        path: String,
        page_instance_id: Option<String>,
        message: AppServiceCommand,
    ) -> Result<(), LxAppError>;
}

// Error codes (must match lingxia-bridge/src/types.ts)
pub(crate) const BRIDGE_NOT_READY: &str = "BRIDGE_NOT_READY";
pub(crate) const BRIDGE_TIMEOUT: &str = "BRIDGE_TIMEOUT";
pub(crate) const BRIDGE_CANCELED: &str = "BRIDGE_CANCELED";
/// Why an in-flight call was cancelled. Without a message the page normalises
/// it to "Unknown error" (`@lingxia/bridge` invocation.ts), which tells a
/// developer reading a log nothing about what happened.
pub(crate) const PAGE_UNLOADED: &str = "Page unloaded";
pub(crate) const BRIDGE_PROTOCOL_MISMATCH: &str = "BRIDGE_PROTOCOL_MISMATCH";
pub(crate) const BRIDGE_MALFORMED_MESSAGE: &str = "BRIDGE_MALFORMED_MESSAGE";
pub(crate) const BRIDGE_METHOD_NOT_FOUND: &str = "BRIDGE_METHOD_NOT_FOUND";
pub(crate) const BRIDGE_TOPIC_NOT_FOUND: &str = "BRIDGE_TOPIC_NOT_FOUND";
pub(crate) const BRIDGE_INTERNAL_ERROR: &str = "BRIDGE_INTERNAL_ERROR";

#[derive(Serialize)]
struct ViewReqOut {
    v: u8,
    kind: &'static str,
    id: String,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
    cap: String,
}

// ViewTransport — posting messages back to the WebView
pub(crate) trait ViewTransport {
    fn post_message_to_view(&self, message_json: String) -> Result<(), LxAppError>;

    fn post_message_to_document(
        &self,
        _expected_generation: DocumentGeneration,
        _gate: Arc<dyn DocumentOutboundGate>,
        _message_json: String,
    ) -> Result<(), LxAppError> {
        Err(LxAppError::Bridge(
            "document-bound message posting is unavailable".to_string(),
        ))
    }
}

/// Invoke route admission before any host lookup or allocation. Keeping the
/// context generic makes the ordering invariant directly unit-testable while
/// production dispatch supplies the non-forgeable `WebMessageContext`.
fn admit_before_host_dispatch<C, T>(
    context: &C,
    admit: impl FnOnce(&C) -> Result<(), LxAppError>,
    dispatch: impl FnOnce() -> Result<T, LxAppError>,
) -> Result<T, LxAppError> {
    admit(context)?;
    dispatch()
}

impl ViewTransport for PageInstance {
    fn post_message_to_view(&self, message_json: String) -> Result<(), LxAppError> {
        if let Some(view) = self.page_view() {
            view.post_message(&message_json)
        } else {
            Err(LxAppError::Runtime("page View not ready".to_string()))
        }
    }

    fn post_message_to_document(
        &self,
        expected_generation: DocumentGeneration,
        gate: Arc<dyn DocumentOutboundGate>,
        message_json: String,
    ) -> Result<(), LxAppError> {
        if let Some(view) = self.page_view() {
            view.post_message_to_document(expected_generation, gate, &message_json)
        } else {
            Err(LxAppError::Runtime("page View not ready".to_string()))
        }
    }
}

fn serialize_seq_frame_with_payload(
    kind: &'static str,
    id: String,
    seq: u64,
    payload_json: &str,
) -> Result<String, LxAppError> {
    let id_json = serde_json::to_string(&id)?;
    let mut message_json = String::with_capacity(id_json.len() + payload_json.len() + 64);
    message_json.push_str("{\"v\":2,\"kind\":\"");
    message_json.push_str(kind);
    message_json.push_str("\",\"id\":");
    message_json.push_str(&id_json);
    message_json.push_str(",\"seq\":");
    message_json.push_str(&seq.to_string());
    message_json.push_str(",\"payload\":");
    message_json.push_str(payload_json);
    message_json.push('}');
    Ok(message_json)
}

// RpcError
#[derive(Debug, Clone)]
pub(crate) struct RpcError {
    pub(crate) code: String,
    pub(crate) message: Option<String>,
    pub(crate) data: Option<Value>,
}

impl RpcError {
    pub(crate) fn new(code: impl Into<String>, message: Option<String>) -> Self {
        Self {
            code: code.into(),
            message,
            data: None,
        }
    }
}

// PageBridge — per-page bridge state and routing
#[derive(Default)]
struct HandshakeState {
    session_id: Option<String>,
    ready: bool,
    connection: Option<Arc<BridgeConnection>>,
}

/// Monotonic identity for native work that belongs to one bound document.
/// It is never reused after the connection has been revoked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SessionWorkId(u64);

impl SessionWorkId {
    pub(crate) const fn is_newer_than(self, other: Self) -> bool {
        self.0 > other.0
    }
}

#[cfg(test)]
impl SessionWorkId {
    pub(crate) const fn for_test(value: u64) -> Self {
        Self(value)
    }
}

/// Immutable delivery credentials captured when work is created. The gate
/// owns the final document/session check at the native JavaScript boundary.
#[derive(Clone)]
pub(crate) enum OutboundContext {
    /// Queued delivery that rechecks the generation at the native boundary;
    /// only Windows constructs it.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    Document {
        expected_generation: DocumentGeneration,
        gate: Arc<dyn DocumentOutboundGate>,
    },
    Gpu {
        endpoint: std::sync::Weak<dyn crate::GpuPageView>,
        generation: crate::GpuViewGeneration,
    },
}

/// The outbound context when it is a GPU View's.
fn gpu(outbound: Option<&OutboundContext>) -> Option<&OutboundContext> {
    outbound.filter(|outbound| outbound.is_gpu())
}

impl OutboundContext {
    pub(crate) fn is_gpu(&self) -> bool {
        matches!(self, Self::Gpu { .. })
    }

    /// Typed state for a GPU View; a document takes state only as frames.
    fn post_state(&self, write: crate::GpuStateWrite) -> Result<(), LxAppError> {
        let Self::Gpu {
            endpoint,
            generation,
        } = self
        else {
            return Err(LxAppError::Bridge(
                "typed state is only for GPU pages".into(),
            ));
        };
        let endpoint = endpoint
            .upgrade()
            .ok_or_else(|| LxAppError::Bridge("GPU endpoint was disposed".into()))?;
        endpoint.write_state(*generation, write)
    }

    /// Everything else a GPU View takes from Logic, typed.
    fn post_gpu(&self, message: crate::GpuViewMessage) -> Result<(), LxAppError> {
        let Self::Gpu {
            endpoint,
            generation,
        } = self
        else {
            return Err(LxAppError::Bridge(
                "typed messages are only for GPU pages".into(),
            ));
        };
        let endpoint = endpoint
            .upgrade()
            .ok_or_else(|| LxAppError::Bridge("GPU endpoint was disposed".into()))?;
        endpoint.deliver(*generation, message)
    }

    fn post<T: ViewTransport>(&self, transport: &T, message: String) -> Result<(), LxAppError> {
        match self {
            Self::Document {
                expected_generation,
                gate,
            } => {
                transport.post_message_to_document(*expected_generation, Arc::clone(gate), message)
            }
            Self::Gpu { .. } => Err(LxAppError::Bridge(
                "a GPU View takes typed messages only".into(),
            )),
        }
    }
}

/// One bridge lifetime. Work must retain this exact connection
/// rather than consulting the mutable handshake again after an async boundary.
struct BridgeConnection {
    work_id: SessionWorkId,
    outbound: Option<OutboundContext>,
    caller: host::AuthenticatedCaller,
}

#[cfg(any(target_os = "windows", test))]
struct SessionOutboundGate {
    handshake: std::sync::Weak<Mutex<HandshakeState>>,
    work_id: SessionWorkId,
}

#[cfg(any(target_os = "windows", test))]
impl DocumentOutboundGate for SessionOutboundGate {
    fn with_active(&self, action: &mut dyn FnMut()) -> bool {
        let Some(handshake) = self.handshake.upgrade() else {
            return false;
        };
        let handshake = handshake.lock().unwrap();
        if handshake
            .connection
            .as_ref()
            .is_none_or(|connection| connection.work_id != self.work_id)
        {
            return false;
        }
        action();
        true
    }
}

fn connection_matches_work(
    connection: Option<&Arc<BridgeConnection>>,
    expected_work: Option<SessionWorkId>,
) -> bool {
    match (expected_work, connection) {
        (Some(expected), Some(current)) => current.work_id == expected,
        (None, None) => true,
        _ => false,
    }
}

/// The one-time snapshot used to construct a document-originated backend
/// command. Keeping the fields together prevents a work id from one session
/// being paired with delivery credentials from another.
#[derive(Clone)]
struct CapturedSessionWork {
    work_id: Option<SessionWorkId>,
    outbound: Option<OutboundContext>,
    caller: Option<host::AuthenticatedCaller>,
}

tokio::task_local! {
    /// Native handlers inherit the document work that admitted them. Any Page
    /// API they invoke after an await must not silently capture a successor.
    static HOST_EFFECT_WORK: CapturedSessionWork;
}

struct DecodedIncoming {
    message: IncomingMessage,
    work: CapturedSessionWork,
    ready: bool,
}

#[derive(Default)]
struct PendingRequestRegistry {
    next_token: AtomicUsize,
    // A document can reuse a JSON-RPC id after navigation.  Keep the
    // document work in the key so a late retired request cannot replace or
    // cancel its successor merely because the caller reused an id.
    requests: Mutex<HashMap<(SessionWorkId, String), PendingRequestEntry>>,
}

struct PendingRequestEntry {
    token: usize,
    work_id: SessionWorkId,
    cancel_tx: oneshot::Sender<()>,
}

pub(crate) struct PendingRequestGuard {
    registry: Arc<PendingRequestRegistry>,
    key: (SessionWorkId, String),
    token: usize,
}

impl PendingRequestRegistry {
    fn register(
        self: &Arc<Self>,
        id: String,
        work_id: SessionWorkId,
    ) -> (oneshot::Receiver<()>, PendingRequestGuard) {
        let token = self
            .next_token
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("pending request token space exhausted");
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let key = (work_id, id);
        let replaced = self.requests.lock().unwrap().insert(
            key.clone(),
            PendingRequestEntry {
                token,
                work_id,
                cancel_tx,
            },
        );
        if let Some(replaced) = replaced {
            let _ = replaced.cancel_tx.send(());
        }
        (
            cancel_rx,
            PendingRequestGuard {
                registry: Arc::clone(self),
                key,
                token,
            },
        )
    }

    fn complete(&self, key: &(SessionWorkId, String), token: usize) {
        let mut requests = self.requests.lock().unwrap();
        if requests.get(key).is_some_and(|entry| entry.token == token) {
            requests.remove(key);
        }
    }

    fn cancel(&self, work_id: SessionWorkId, id: &str) {
        let key = (work_id, id.to_owned());
        if let Some(entry) = self.requests.lock().unwrap().remove(&key) {
            let _ = entry.cancel_tx.send(());
        }
    }

    fn cancel_work(&self, work_id: SessionWorkId) {
        let canceled = {
            let mut requests = self.requests.lock().unwrap();
            let keys = requests
                .iter()
                .filter(|(_, entry)| entry.work_id == work_id)
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>();
            keys.into_iter()
                .filter_map(|key| requests.remove(&key))
                .collect::<Vec<_>>()
        };
        for entry in canceled {
            let _ = entry.cancel_tx.send(());
        }
    }

    #[cfg(test)]
    fn cancel_all(&self) {
        let requests = {
            let mut requests = self.requests.lock().unwrap();
            std::mem::take(&mut *requests)
        };
        for (_, entry) in requests {
            let _ = entry.cancel_tx.send(());
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

#[cfg(test)]
impl PendingRequestGuard {
    pub(crate) fn detached_for_test(
        id: &str,
        work_id: SessionWorkId,
    ) -> (oneshot::Receiver<()>, Self) {
        Arc::new(PendingRequestRegistry::default()).register(id.to_owned(), work_id)
    }
}

impl Drop for PendingRequestGuard {
    fn drop(&mut self) {
        self.registry.complete(&self.key, self.token);
    }
}

struct PageBridgeState {
    lxapp: Arc<LxApp>,
    js_backend: Arc<dyn AppServiceBackend>,
    msg_counter: AtomicUsize,
    next_session_work_id: std::sync::atomic::AtomicU64,
    handshake: Arc<Mutex<HandshakeState>>,
    pending_requests: Arc<PendingRequestRegistry>,
    // The same channel id may occur in successive document sessions.  The
    // work id is part of the registry key; the token additionally protects a
    // same-work replacement of an id.
    active_host_channels: Mutex<HashMap<(SessionWorkId, String), ActiveHostChannel>>,
    next_host_channel_token: AtomicUsize,
    next_host_notify_token: AtomicUsize,
    active_host_notifies: Mutex<HashMap<usize, ActiveHostNotify>>,
}

struct ActiveHostChannel {
    token: usize,
    work_id: SessionWorkId,
    outbound: Option<OutboundContext>,
    sender: host::ChannelContextSender,
}

struct ActiveHostNotify {
    work_id: SessionWorkId,
    _outbound: Option<OutboundContext>,
    cancel_tx: oneshot::Sender<()>,
}

struct PendingHostNotifyGuard {
    state: Arc<PageBridgeState>,
    token: usize,
    _outbound: Option<OutboundContext>,
}

impl Drop for PendingHostNotifyGuard {
    fn drop(&mut self) {
        self.state
            .active_host_notifies
            .lock()
            .unwrap()
            .remove(&self.token);
    }
}

#[derive(Clone)]
pub(crate) struct PageBridge {
    inner: Arc<PageBridgeState>,
}

fn gpu_view_control_route(route: &str) -> bool {
    matches!(
        route,
        "navigation.setLeaveGuard" | "navigation.leave" | "navigation.setTitle"
    )
}

#[test]
fn gpu_leave_controls_are_an_exact_allowlist() {
    assert!(gpu_view_control_route("navigation.setLeaveGuard"));
    assert!(gpu_view_control_route("navigation.leave"));
    assert!(gpu_view_control_route("navigation.setTitle"));
    for route in [
        "navigation.leave.extra",
        "navigation",
        "navigation.navigateBack",
        "navigation.goHome",
        "host.navigation.leave",
        "navigation.leave ",
        "storage.get",
    ] {
        assert!(!gpu_view_control_route(route), "{route}");
    }
}

pub(crate) fn required_cap_for_name(name: &str) -> String {
    if name.starts_with("host.") {
        return "host".to_string();
    }
    if name.starts_with("state.") {
        return "state".to_string();
    }
    if let Some((prefix, _)) = name.split_once('.') {
        return prefix.to_string();
    }
    "page".to_string()
}

impl PageBridge {
    pub(crate) fn new(lxapp: Arc<LxApp>, js_backend: Arc<dyn AppServiceBackend>) -> Self {
        Self {
            inner: Arc::new(PageBridgeState {
                lxapp,
                js_backend,
                msg_counter: AtomicUsize::new(0),
                next_session_work_id: std::sync::atomic::AtomicU64::new(1),
                handshake: Arc::new(Mutex::new(HandshakeState::default())),
                pending_requests: Arc::new(PendingRequestRegistry::default()),
                active_host_channels: Mutex::new(HashMap::new()),
                next_host_channel_token: AtomicUsize::new(1),
                next_host_notify_token: AtomicUsize::new(1),
                active_host_notifies: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.inner.handshake.lock().unwrap().ready
    }

    fn next_session_work_id(&self) -> SessionWorkId {
        let next = self
            .inner
            .next_session_work_id
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("SessionWorkId space exhausted");
        SessionWorkId(next)
    }

    /// Replace exactly the connection that admitted a hello. The comparison,
    /// Begin enqueue, and replacement share the handshake lock: a decoded
    /// stale hello must never reset a successor.
    fn replace_with_session_work(
        &self,
        page: &PageInstance,
        expected_work: Option<SessionWorkId>,
        document: DocumentBinding,
    ) -> Result<Option<Arc<BridgeConnection>>, LxAppError> {
        let work_id = self.next_session_work_id();
        #[cfg(target_os = "windows")]
        let outbound = match document {
            DocumentBinding::Bound(expected_generation) => Some(OutboundContext::Document {
                expected_generation,
                gate: Arc::new(SessionOutboundGate {
                    handshake: Arc::downgrade(&self.inner.handshake),
                    work_id,
                }),
            }),
            // Older platform adapters which cannot attest a generation retain
            // their existing V2 transport semantics. A bound message never
            // downgrades to this path.
            DocumentBinding::Unbound => None,
        };
        // Android's legacy JavascriptInterface can prove a generation without
        // supporting the document-port sender; Apple and Harmony likewise keep
        // their established V2 transport. Only Windows needs queued delivery
        // to avoid blocking its UI callback on the ingress generation lock.
        #[cfg(not(target_os = "windows"))]
        let outbound = {
            let _ = document;
            None
        };
        let connection = Arc::new(BridgeConnection {
            work_id,
            outbound,
            caller: host::AuthenticatedCaller::for_lxapp(&self.inner.lxapp),
        });
        let replaced = {
            let mut handshake = self.inner.handshake.lock().unwrap();
            if !connection_matches_work(handshake.connection.as_ref(), expected_work) {
                return Ok(None);
            }
            self.begin_work_locked(page, connection.work_id)?;
            let replaced = handshake.connection.replace(Arc::clone(&connection));
            handshake.session_id = None;
            handshake.ready = false;
            replaced
        };
        if let Some(previous) = replaced {
            self.cancel_work(page, previous, "Session replaced");
        }
        Ok(Some(connection))
    }

    /// Capture the exact native work identity and document-bound transport at
    /// creation time. Completion paths must use this value, never query a
    /// successor connection.
    pub(crate) fn capture_session_work(&self) -> Option<(SessionWorkId, Option<OutboundContext>)> {
        if let Ok(work) = HOST_EFFECT_WORK.try_with(Clone::clone)
            && let Some(work_id) = work.work_id
        {
            return Some((work_id, work.outbound));
        }
        self.inner
            .handshake
            .lock()
            .unwrap()
            .connection
            .as_ref()
            .map(|connection| (connection.work_id, connection.outbound.clone()))
    }

    pub(crate) fn is_current_work(&self, work_id: Option<SessionWorkId>) -> bool {
        let handshake = self.inner.handshake.lock().unwrap();
        match (work_id, handshake.connection.as_ref()) {
            (Some(work_id), Some(connection)) => connection.work_id == work_id,
            (None, None) => true,
            _ => false,
        }
    }

    fn begin_work_locked(
        &self,
        page: &PageInstance,
        work_id: SessionWorkId,
    ) -> Result<(), LxAppError> {
        // Callers hold `handshake`. This order deliberately makes Begin part
        // of the connection state transition rather than a late side effect.
        self.inner.js_backend.forward(
            Arc::clone(&self.inner.lxapp),
            page.path(),
            Some(page.instance_id_string()),
            AppServiceCommand::BeginSessionWork { work_id },
        )
    }

    fn cancel_work(&self, page: &PageInstance, connection: Arc<BridgeConnection>, reason: &str) {
        // The old connection has already been removed under the handshake
        // lock. Every cancellation below is keyed, so it cannot affect a
        // replacement that wins the race before this code runs.
        self.inner.pending_requests.cancel_work(connection.work_id);
        self.cancel_host_notifies_for_work(connection.work_id);
        crate::view_call::cancel_view_calls_for_work(
            connection.work_id,
            page.instance_id_string().as_str(),
            "Document session revoked",
        );
        self.close_host_channels_for_work(page, connection.work_id, reason);
        let _ = self.forward_js_message(
            page,
            AppServiceCommand::CancelSessionWork {
                work_id: connection.work_id,
            },
        );
    }

    pub(crate) fn lxapp(&self) -> Arc<LxApp> {
        self.inner.lxapp.clone()
    }

    pub(crate) fn bind_gpu(
        &self,
        page: &PageInstance,
        endpoint: &Arc<dyn crate::GpuPageView>,
    ) -> Result<crate::GpuViewGeneration, LxAppError> {
        if !page.is_current_gpu_endpoint(endpoint) {
            return Err(LxAppError::Bridge(
                "GPU endpoint is no longer attached".into(),
            ));
        }
        let work_id = self.next_session_work_id();
        let generation = crate::GpuViewGeneration(work_id.0);
        let connection = Arc::new(BridgeConnection {
            work_id,
            outbound: Some(OutboundContext::Gpu {
                endpoint: Arc::downgrade(endpoint),
                generation,
            }),
            caller: host::AuthenticatedCaller::for_lxapp(&self.inner.lxapp),
        });
        let previous = {
            let mut handshake = self.inner.handshake.lock().unwrap();
            self.begin_work_locked(page, work_id)?;
            handshake.ready = false;
            handshake.session_id = None;
            handshake.connection.replace(connection)
        };
        if let Some(previous) = previous {
            self.cancel_work(page, previous, "GPU connection replaced");
        }
        Ok(generation)
    }

    pub(crate) fn cancel_gpu_generation(
        &self,
        page: &PageInstance,
        generation: crate::GpuViewGeneration,
    ) {
        let connection = {
            let mut handshake = self.inner.handshake.lock().unwrap();
            if !connection_matches_work(
                handshake.connection.as_ref(),
                Some(SessionWorkId(generation.0)),
            ) {
                return;
            }
            handshake.ready = false;
            handshake.session_id = None;
            handshake.connection.take()
        };
        if let Some(connection) = connection {
            self.cancel_work(page, connection, "GPU connection failed");
        }
    }

    pub(crate) fn cancel_gpu_endpoint(
        &self,
        page: &PageInstance,
        endpoint: &Arc<dyn crate::GpuPageView>,
    ) {
        let connection = {
            let mut handshake = self.inner.handshake.lock().unwrap();
            let matches = handshake.connection.as_ref().is_some_and(|connection|
                matches!(connection.outbound.as_ref(), Some(OutboundContext::Gpu { endpoint: current, .. })
                    if current.upgrade().is_some_and(|current| Arc::ptr_eq(&current, endpoint))));
            if !matches {
                return;
            }
            handshake.ready = false;
            handshake.session_id = None;
            handshake.connection.take()
        };
        if let Some(connection) = connection {
            self.cancel_work(page, connection, "GPU setup cancelled");
        }
    }

    /// A GPU View's report on its typed state writes, admitted like its frames.
    pub(crate) fn handle_gpu_state_signal(
        &self,
        page: &PageInstance,
        endpoint: &Arc<dyn crate::GpuPageView>,
        generation: crate::GpuViewGeneration,
        signal: crate::GpuStateSignal,
    ) -> Result<(), LxAppError> {
        let outbound = {
            let handshake = self.inner.handshake.lock().unwrap();
            handshake
                .connection
                .as_ref()
                .and_then(|connection| connection.outbound.clone())
                .filter(|outbound| {
                    matches!(outbound, OutboundContext::Gpu { endpoint: expected, generation: current }
                        if *current == generation && expected.upgrade().is_some_and(|expected| Arc::ptr_eq(&expected, endpoint)))
                })
                .ok_or_else(|| LxAppError::Bridge("stale GPU connection".into()))?
        };
        let work_id = Some(SessionWorkId(generation.0));
        self.forward_js_message(
            page,
            match signal {
                crate::GpuStateSignal::Acked(rev) => AppServiceCommand::StateAck {
                    work_id,
                    scope: None,
                    rev,
                },
                crate::GpuStateSignal::Resync => AppServiceCommand::StateResync {
                    work_id,
                    outbound: Some(outbound),
                },
            },
        )
    }

    /// A typed message from the GPU View, admitted like its frames: endpoint and generation
    /// must be the current connection's before anything is dispatched.
    pub(crate) fn handle_gpu_event(
        &self,
        page: &PageInstance,
        endpoint: &Arc<dyn crate::GpuPageView>,
        generation: crate::GpuViewGeneration,
        event: crate::GpuViewEvent,
    ) -> Result<(), LxAppError> {
        let (work, ready) = {
            let handshake = self.inner.handshake.lock().unwrap();
            let connection = handshake
                .connection
                .as_ref()
                .filter(|connection| {
                    connection.work_id == SessionWorkId(generation.0)
                        && matches!(connection.outbound.as_ref(), Some(OutboundContext::Gpu { endpoint: expected, generation: current })
                            if *current == generation && expected.upgrade().is_some_and(|expected| Arc::ptr_eq(&expected, endpoint)))
                })
                .ok_or_else(|| LxAppError::Bridge("stale GPU connection".into()))?;
            let work = CapturedSessionWork {
                work_id: Some(connection.work_id),
                outbound: connection.outbound.clone(),
                caller: Some(connection.caller.clone()),
            };
            (work, handshake.ready)
        };
        let context = &BridgeMessageContext::Gpu;
        match event {
            crate::GpuViewEvent::Connected => {
                let Some(work_id) = work.work_id.filter(|_| !ready) else {
                    return Err(LxAppError::Bridge("repeated GPU connection".into()));
                };
                if !self.set_ready_if_current(work_id, self.new_session_id()) {
                    return Ok(());
                }
                if let Err(err) = self.forward_js_message(
                    page,
                    AppServiceCommand::Ready {
                        work_id: work.work_id,
                        outbound: work.outbound.clone(),
                    },
                ) {
                    crate::warn!("bridge ready bootstrap failed: {}", err)
                        .with_appid(page.appid())
                        .with_path(page.path());
                }
                Ok(())
            }
            crate::GpuViewEvent::Call {
                id,
                method,
                params,
                sent_us,
            } => {
                if method
                    .strip_prefix("host.")
                    .is_some_and(|route| !gpu_view_control_route(route))
                {
                    return self.send_res_err_for_context(
                        page,
                        work.work_id,
                        work.outbound.as_ref(),
                        id,
                        "E_PERMISSION_DENIED",
                        Some("GPU View host calls require a Logic action".into()),
                        None,
                    );
                }
                let call = Call {
                    cap: required_cap_for_name(&method),
                    id,
                    method,
                    params: params.map(Payload::Tree),
                    sent_us,
                };
                self.handle_call(page, context, call, &work, ready)
            }
            crate::GpuViewEvent::Cancel { id } => {
                self.handle_cancel(page, context, &CancelMsg { id }, &work)
            }
            crate::GpuViewEvent::ChannelOpen { id, topic, params } => {
                if topic.contains('.') {
                    return self.send_ch_ack_err_for_context(
                        page,
                        work.work_id,
                        work.outbound.as_ref(),
                        id,
                        "E_PERMISSION_DENIED",
                        Some("GPU View channels require a page Logic handler".into()),
                        None,
                    );
                }
                let cap = required_cap_for_name(&topic);
                let params = params.map(Payload::Tree);
                self.handle_ch_open_parts(page, context, &id, &topic, &cap, params, &work, ready)
            }
            crate::GpuViewEvent::ChannelData { id, payload } => {
                self.handle_ch_data_parts(page, &id, Payload::Tree(payload), &work)
            }
            crate::GpuViewEvent::ChannelClose { id, code, reason } => {
                self.handle_ch_close(page, context, &ChCloseMsg { id, code, reason }, &work)
            }
            crate::GpuViewEvent::Reply { id, outcome } => {
                if !self.is_current_work(work.work_id) {
                    return Ok(());
                }
                let result = outcome.map_err(|error| RpcError {
                    code: match error.code {
                        Value::String(code) => code,
                        Value::Number(code) => code.to_string(),
                        _ => BRIDGE_INTERNAL_ERROR.to_string(),
                    },
                    message: error.message,
                    data: error.data,
                });
                let page_instance_id = page.instance_id_string();
                crate::view_call::resolve_view_call(
                    &id,
                    Some(&page_instance_id),
                    work.work_id,
                    result,
                );
                Ok(())
            }
        }
    }

    pub(crate) fn handle_incoming(
        &self,
        page: &PageInstance,
        incoming: IncomingWebMessage,
    ) -> Result<(), LxAppError> {
        // This is deliberately before JSON decode and before any request or
        // channel allocation. Every inbound bridge kind retains the same seam
        // and receives platform-attested context.
        let context = incoming.context();
        self.admit_incoming(page, context)?;
        let decoded = self.predecode_inbound(incoming.body())?;
        let context = &BridgeMessageContext::Web(context);
        match &decoded.message {
            IncomingMessage::Hello(msg) => self.handle_hello(page, context, msg, &decoded),
            IncomingMessage::Req(msg) => {
                self.handle_req(page, context, msg, &decoded.work, decoded.ready)
            }
            IncomingMessage::Res(msg) => self.handle_res(page, context, msg, &decoded.work),
            IncomingMessage::Notify(msg) => {
                self.handle_notify(page, context, msg, &decoded.work, decoded.ready)
            }
            IncomingMessage::ChOpen(msg) => {
                self.handle_ch_open(page, context, msg, &decoded.work, decoded.ready)
            }
            IncomingMessage::ChData(msg) => self.handle_ch_data(page, context, msg, &decoded.work),
            IncomingMessage::ChClose(msg) => {
                self.handle_ch_close(page, context, msg, &decoded.work)
            }
            IncomingMessage::Cancel(msg) => self.handle_cancel(page, context, msg, &decoded.work),
            IncomingMessage::StateAck(msg) => {
                self.handle_state_ack(page, context, msg, &decoded.work)
            }
            IncomingMessage::Unknown(unknown) => {
                self.handle_unknown(page, context, unknown, &decoded.work)
            }
        }
    }

    fn predecode_inbound(&self, frame: &str) -> Result<DecodedIncoming, LxAppError> {
        let message = decode_v2_inbound(frame)
            .map_err(|_| LxAppError::Bridge("invalid bridge protocol envelope".to_string()))?;
        let handshake = self.inner.handshake.lock().unwrap();
        let work = handshake
            .connection
            .as_ref()
            .map(|connection| CapturedSessionWork {
                work_id: Some(connection.work_id),
                outbound: connection.outbound.clone(),
                caller: Some(connection.caller.clone()),
            })
            .unwrap_or(CapturedSessionWork {
                work_id: None,
                outbound: None,
                caller: None,
            });
        Ok(DecodedIncoming {
            message,
            work,
            ready: handshake.ready,
        })
    }

    /// The single pre-decode admission seam. It intentionally permits every
    /// context for now; authorization is introduced in a later change. This
    /// check is deliberately idempotent: Page delegates invoke it before
    /// handling non-bridge envelopes, and bridge dispatch invokes it again
    /// before protocol decoding.
    pub(crate) fn admit_incoming(
        &self,
        _page: &PageInstance,
        _context: &WebMessageContext,
    ) -> Result<(), LxAppError> {
        Ok(())
    }

    /// Host routes are admitted before handler lookup, task/channel creation,
    /// or mutation. Keeping this distinct from protocol admission lets route
    /// policy evaluate an audience without parsing or allocation races.
    fn admit_host_route(
        &self,
        _page: &PageInstance,
        context: &BridgeMessageContext,
        route: &str,
    ) -> Result<(), LxAppError> {
        if matches!(context, BridgeMessageContext::Gpu) && !gpu_view_control_route(route) {
            return Err(LxAppError::Bridge(
                "GPU Views must invoke host APIs through Logic actions".into(),
            ));
        }
        Ok(())
    }

    fn handle_res(
        &self,
        page: &PageInstance,
        _context: &BridgeMessageContext,
        msg: &ResMsg,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work.work_id) {
            return Ok(());
        }

        let result = if msg.ok {
            Ok(msg.result.clone().unwrap_or(Value::Null))
        } else {
            let err = msg.error.as_ref();
            Err(RpcError {
                code: err
                    .map(|e| e.normalized_code())
                    .unwrap_or_else(|| BRIDGE_INTERNAL_ERROR.to_string()),
                message: err.and_then(|e| e.message.clone()),
                data: err.and_then(|e| e.data.clone()),
            })
        };
        let page_instance_id = page.instance_id_string();
        crate::view_call::resolve_view_call(&msg.id, Some(&page_instance_id), work.work_id, result);
        Ok(())
    }

    fn handle_ch_data(
        &self,
        page: &PageInstance,
        _context: &BridgeMessageContext,
        msg: &ChDataMsg,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        self.handle_ch_data_parts(
            page,
            &msg.id,
            Payload::Json(msg.payload.get().to_owned()),
            work,
        )
    }

    fn handle_ch_data_parts(
        &self,
        page: &PageInstance,
        id: &str,
        payload: Payload,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work.work_id) {
            return Ok(());
        }
        let Some(work_id) = work.work_id else {
            return Ok(());
        };
        let payload = match self.send_data_to_host_channel(id, work_id, payload) {
            None => return Ok(()),
            Some(payload) => payload,
        };
        self.forward_js_message(
            page,
            AppServiceCommand::ChData {
                work_id: work.work_id,
                id: id.to_owned(),
                payload,
            },
        )
    }

    fn handle_ch_close(
        &self,
        page: &PageInstance,
        _context: &BridgeMessageContext,
        msg: &ChCloseMsg,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work.work_id) {
            return Ok(());
        }
        let Some(work_id) = work.work_id else {
            return Ok(());
        };
        if self.close_host_channel_from_view(&msg.id, work_id, msg.code.clone(), msg.reason.clone())
        {
            return Ok(());
        }
        self.forward_js_message(
            page,
            AppServiceCommand::ChClose {
                work_id: work.work_id,
                id: msg.id.clone(),
                code: msg.code.clone(),
                reason: msg.reason.clone(),
            },
        )
    }

    fn handle_cancel(
        &self,
        _page: &PageInstance,
        _context: &BridgeMessageContext,
        msg: &CancelMsg,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        if let Some(work_id) = work.work_id
            && self.is_current_work(Some(work_id))
        {
            self.inner.pending_requests.cancel(work_id, &msg.id);
        }
        Ok(())
    }

    fn handle_state_ack(
        &self,
        page: &PageInstance,
        _context: &BridgeMessageContext,
        msg: &StateAckMsg,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work.work_id) {
            return Ok(());
        }
        self.forward_js_message(
            page,
            AppServiceCommand::StateAck {
                work_id: work.work_id,
                scope: msg.scope.clone(),
                rev: msg.rev,
            },
        )
    }

    fn handle_unknown(
        &self,
        page: &PageInstance,
        _context: &BridgeMessageContext,
        unknown: &UnknownMsg,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work.work_id) {
            return Ok(());
        }
        if let Some(id) = &unknown.id {
            let (code, message) = if unknown.v.is_none() {
                (
                    BRIDGE_PROTOCOL_MISMATCH,
                    Some(format!(
                        "Unsupported protocol: {}",
                        unknown
                            .v
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "missing".to_string())
                    )),
                )
            } else {
                (
                    BRIDGE_MALFORMED_MESSAGE,
                    unknown
                        .kind
                        .as_deref()
                        .map(|kind| format!("Unknown kind: {kind}"))
                        .or_else(|| unknown.parse_error.clone())
                        .or_else(|| Some("Unknown message".to_string())),
                )
            };
            let _ = self.send_res_err_for_context(
                page,
                work.work_id,
                work.outbound.as_ref(),
                id.clone(),
                code,
                message,
                None,
            );
        }
        Ok(())
    }

    fn handle_hello(
        &self,
        page: &PageInstance,
        _context: &BridgeMessageContext,
        msg: &HelloMsg,
        decoded: &DecodedIncoming,
    ) -> Result<(), LxAppError> {
        if msg.v != 2 {
            return Err(LxAppError::Bridge(format!(
                "Unsupported protocol: {}",
                msg.v
            )));
        }
        if !msg.protocols_supported.contains(&2) {
            return Err(LxAppError::Bridge(
                "Protocol 2 not in supported list".to_string(),
            ));
        }
        if msg.role != "view" {
            return Err(LxAppError::Bridge(format!("Unexpected role: {}", msg.role)));
        }
        if let Some(expected) = page.bridge_nonce()
            && expected != msg.nonce
        {
            return Err(LxAppError::Bridge("Nonce mismatch".to_string()));
        }
        if !self.is_current_work(decoded.work.work_id) {
            return Ok(());
        }

        let session_id = self.new_session_id();
        let connection = match _context {
            BridgeMessageContext::Web(context) => {
                self.replace_with_session_work(page, decoded.work.work_id, context.document())?
            }
            BridgeMessageContext::Gpu => {
                return Err(LxAppError::Bridge(
                    "a GPU View connects without hello".into(),
                ));
            }
        };
        let Some(connection) = connection else {
            return Ok(());
        };
        let work_id = connection.work_id;
        let work = CapturedSessionWork {
            work_id: Some(work_id),
            outbound: connection.outbound.clone(),
            caller: Some(host::AuthenticatedCaller::for_lxapp(&self.inner.lxapp)),
        };
        self.send_hello_ack(
            page,
            work.work_id,
            work.outbound.as_ref(),
            msg.nonce.clone(),
            session_id.clone(),
        )?;
        if !self.set_ready_if_current(work_id, session_id.clone()) {
            return Ok(());
        }
        // Queue AppService initialization before exposing `ready` to the View.
        // Otherwise a fast View can flush a page-action notification while the
        // worker still considers the page uninitialized, losing the action.
        if let Err(err) = self.forward_js_message(
            page,
            AppServiceCommand::Ready {
                work_id: work.work_id,
                outbound: work.outbound.clone(),
            },
        ) {
            crate::warn!("bridge ready bootstrap failed: {}", err)
                .with_appid(page.appid())
                .with_path(page.path());
        }
        self.send_ready(
            page,
            work.work_id,
            work.outbound.as_ref(),
            session_id.clone(),
            work.caller
                .as_ref()
                .expect("session work always has a caller"),
        )?;
        Ok(())
    }

    fn handle_req(
        &self,
        page: &PageInstance,
        context: &BridgeMessageContext,
        msg: &ReqMsg,
        work: &CapturedSessionWork,
        ready: bool,
    ) -> Result<(), LxAppError> {
        let call = Call {
            id: msg.id.clone(),
            method: msg.method.clone(),
            cap: msg.cap.clone(),
            params: msg
                .params
                .as_ref()
                .map(|v| Payload::Json(v.get().to_owned())),
            sent_us: probe::now_us(),
        };
        self.handle_call(page, context, call, work, ready)
    }

    fn handle_call(
        &self,
        page: &PageInstance,
        context: &BridgeMessageContext,
        call: Call,
        work: &CapturedSessionWork,
        ready: bool,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work.work_id) {
            return Ok(());
        }
        if !ready {
            let _ = self.send_res_err_for_context(
                page,
                work.work_id,
                work.outbound.as_ref(),
                call.id.clone(),
                BRIDGE_NOT_READY,
                Some("Bridge not ready".to_string()),
                None,
            );
            return Ok(());
        }

        let required_cap = required_cap_for_name(&call.method);
        if call.cap.is_empty() {
            let _ = self.send_res_err_for_context(
                page,
                work.work_id,
                work.outbound.as_ref(),
                call.id.clone(),
                BRIDGE_MALFORMED_MESSAGE,
                Some("Missing cap".to_string()),
                None,
            );
            return Ok(());
        }
        if call.cap != required_cap {
            let _ = self.send_res_err_for_context(
                page,
                work.work_id,
                work.outbound.as_ref(),
                call.id.clone(),
                BRIDGE_MALFORMED_MESSAGE,
                Some(format!("Capability mismatch: expected '{}'", required_cap)),
                None,
            );
            return Ok(());
        }

        if call.method == "state.getSnapshot" {
            #[derive(serde::Deserialize)]
            struct SnapshotParams {
                scope: Option<String>,
            }

            let scope = call
                .params
                .and_then(|params| match params {
                    Payload::Json(json) => serde_json::from_str::<SnapshotParams>(&json).ok(),
                    Payload::Tree(value) => serde_json::from_value::<SnapshotParams>(value).ok(),
                })
                .and_then(|params| params.scope);
            return self.forward_js_request(
                page,
                work.work_id,
                work.outbound.as_ref(),
                call.id.clone(),
                AppServiceCommand::StateSnapshot {
                    work_id: work.work_id,
                    outbound: work.outbound.clone(),
                    id: call.id.clone(),
                    scope,
                },
            );
        }

        // host.* → native Rust handler (bypasses JS worker)
        if let Some(host_method) = call.method.strip_prefix("host.") {
            return self.dispatch_host_req(
                page,
                context,
                call.id.clone(),
                host_method.to_string(),
                call.params.map(Payload::into_json),
                work,
            );
        }

        // everything else → JS runtime
        let Some(work_id) = work.work_id else {
            return Ok(());
        };
        let (cancel_rx, pending_request) = self
            .inner
            .pending_requests
            .register(call.id.clone(), work_id);
        // Registration and session revocation race across independent locks.
        // If revocation swept this work just before the insertion, compensate
        // before handing the request to the asynchronous JS backend.
        if !self.is_current_work(Some(work_id)) {
            drop(pending_request);
            return Ok(());
        }
        self.forward_js_request(
            page,
            Some(work_id),
            work.outbound.as_ref(),
            call.id.clone(),
            AppServiceCommand::Req {
                work_id: Some(work_id),
                outbound: work.outbound.clone(),
                id: call.id.clone(),
                method: call.method.clone(),
                params: call.params,
                cancel_rx,
                pending_request,
                sent_us: call.sent_us,
            },
        )
    }

    fn handle_notify(
        &self,
        page: &PageInstance,
        context: &BridgeMessageContext,
        msg: &NotifyMsg,
        work: &CapturedSessionWork,
        ready: bool,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work.work_id) || !ready {
            return Ok(());
        }

        let required_cap = required_cap_for_name(&msg.method);
        if msg.cap.is_empty() || msg.cap != required_cap {
            return Ok(());
        }

        let params_json = msg.params.as_ref().map(|v| v.get().to_owned());
        if let Some(host_method) = msg.method.strip_prefix("host.") {
            return self.dispatch_host_notify(
                page,
                context,
                host_method.to_string(),
                params_json,
                work,
            );
        }

        self.forward_js_message(
            page,
            AppServiceCommand::Notify {
                work_id: work.work_id,
                outbound: work.outbound.clone(),
                method: msg.method.clone(),
                params_json,
            },
        )
    }

    fn handle_ch_open(
        &self,
        page: &PageInstance,
        context: &BridgeMessageContext,
        msg: &ChOpenMsg,
        work: &CapturedSessionWork,
        ready: bool,
    ) -> Result<(), LxAppError> {
        let params = msg
            .params
            .as_ref()
            .map(|v| Payload::Json(v.get().to_owned()));
        self.handle_ch_open_parts(
            page, context, &msg.id, &msg.topic, &msg.cap, params, work, ready,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_ch_open_parts(
        &self,
        page: &PageInstance,
        context: &BridgeMessageContext,
        id: &str,
        topic: &str,
        cap: &str,
        params: Option<Payload>,
        work: &CapturedSessionWork,
        ready: bool,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work.work_id) {
            return Ok(());
        }
        if !ready {
            let _ = self.send_ch_ack_err_for_context(
                page,
                work.work_id,
                work.outbound.as_ref(),
                id.to_owned(),
                BRIDGE_NOT_READY,
                Some("Bridge not ready".to_string()),
                None,
            );
            return Ok(());
        }

        let required_cap = required_cap_for_name(topic);
        if cap.is_empty() || cap != required_cap {
            let _ = self.send_ch_ack_err_for_context(
                page,
                work.work_id,
                work.outbound.as_ref(),
                id.to_owned(),
                BRIDGE_MALFORMED_MESSAGE,
                Some(format!("Capability mismatch: expected '{}'", required_cap)),
                None,
            );
            return Ok(());
        }
        if let Some(host_topic) = topic.strip_prefix("host.") {
            return self.dispatch_host_ch_open(
                page,
                context,
                id.to_owned(),
                host_topic,
                params.map(Payload::into_json),
                work,
            );
        }

        self.forward_js_channel_open(
            page,
            work.work_id,
            work.outbound.as_ref(),
            id.to_owned(),
            AppServiceCommand::ChOpen {
                work_id: work.work_id,
                outbound: work.outbound.clone(),
                id: id.to_owned(),
                topic: topic.to_owned(),
                params,
            },
        )
    }

    fn forward_js_message(
        &self,
        page: &PageInstance,
        message: AppServiceCommand,
    ) -> Result<(), LxAppError> {
        self.inner.js_backend.forward(
            self.inner.lxapp.clone(),
            page.path(),
            Some(page.instance_id_string()),
            message,
        )
    }

    fn forward_js_request(
        &self,
        page: &PageInstance,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        id: String,
        message: AppServiceCommand,
    ) -> Result<(), LxAppError> {
        if let Err(err) = self.forward_js_message(page, message) {
            let _ = self.send_res_err_for_context(
                page,
                work_id,
                outbound,
                id,
                BRIDGE_INTERNAL_ERROR,
                Some(err.to_string()),
                None,
            );
        }
        Ok(())
    }

    fn forward_js_channel_open(
        &self,
        page: &PageInstance,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        id: String,
        message: AppServiceCommand,
    ) -> Result<(), LxAppError> {
        if let Err(err) = self.forward_js_message(page, message) {
            let _ = self.send_ch_ack_err_for_context(
                page,
                work_id,
                outbound,
                id,
                BRIDGE_INTERNAL_ERROR,
                Some(err.to_string()),
                None,
            );
        }
        Ok(())
    }

    pub(crate) fn send_res_ok_for_context<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        id: String,
        result: Payload,
    ) -> Result<(), LxAppError> {
        if let Some(outbound) = gpu(outbound) {
            return self.post_gpu(work_id, outbound, || {
                Ok(crate::GpuViewMessage::Reply {
                    id,
                    outcome: Ok(result.into_tree()?),
                })
            });
        }
        let result = RawValue::from_string(result.into_json())
            .map_err(|e| LxAppError::Bridge(e.to_string()))?;
        let msg = Res {
            v: 2,
            kind: "res",
            id,
            ok: true,
            result: Some(result),
            error: None,
        };
        self.send_json_for_context(transport, work_id, outbound, &msg)
    }

    pub(crate) fn send_view_request_for_context<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        id: String,
        method: String,
        params: Option<Value>,
        cap: String,
    ) -> Result<(), LxAppError> {
        if let Some(outbound) = gpu(outbound) {
            let request =
                crate::GpuViewRequest::from_call(&method, params).map_err(|(code, message)| {
                    LxAppError::RongJSHost {
                        code: code.into(),
                        message,
                        data: None,
                    }
                })?;
            return self.post_gpu(work_id, outbound, || {
                Ok(crate::GpuViewMessage::Request { id, request })
            });
        }
        let msg = ViewReqOut {
            v: 2,
            kind: "req",
            id,
            method,
            params,
            cap,
        };
        self.send_json_for_context(transport, work_id, outbound, &msg)
    }

    pub(crate) fn send_res_err_for_context<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        id: String,
        code: &str,
        message: Option<String>,
        data: Option<Value>,
    ) -> Result<(), LxAppError> {
        let wire_code = data
            .as_ref()
            .and_then(|d| d.get("bizCode"))
            .and_then(|v| v.as_u64())
            .map(|n| Value::Number(n.into()))
            .unwrap_or_else(|| Value::String(code.to_string()));
        if let Some(outbound) = gpu(outbound) {
            return self.post_gpu(work_id, outbound, || {
                Ok(crate::GpuViewMessage::Reply {
                    id,
                    outcome: Err(crate::GpuBridgeError {
                        code: wire_code,
                        message,
                        data,
                    }),
                })
            });
        }

        let msg = Res {
            v: 2,
            kind: "res",
            id,
            ok: false,
            result: None,
            error: Some(BridgeError {
                code: wire_code,
                message,
                data,
            }),
        };
        self.send_json_for_context(transport, work_id, outbound, &msg)
    }

    pub(crate) fn send_state_snapshot_for_context<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        scope: Option<String>,
        rev: u64,
        state: Payload,
    ) -> Result<(), LxAppError> {
        if let Some(outbound) = outbound.filter(|outbound| outbound.is_gpu()) {
            if !self.is_current_work(work_id) {
                return Ok(());
            }
            let state = state.into_tree()?;
            return outbound.post_state(crate::GpuStateWrite::Snapshot { rev, state });
        }
        let state = RawValue::from_string(state.into_json())
            .map_err(|e| LxAppError::Bridge(e.to_string()))?;
        let msg = StateSnapshotOut {
            v: 2,
            kind: "state.snapshot",
            scope,
            rev,
            state,
        };
        self.send_json_for_context(transport, work_id, outbound, &msg)
    }

    /// A GPU page's state patch, typed and in process.
    pub(crate) fn send_gpu_state_patch(
        &self,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        rev: u64,
        ops: Vec<crate::GpuPatchOp>,
        ack: bool,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work_id) {
            return Ok(());
        }
        let outbound =
            outbound.ok_or_else(|| LxAppError::Bridge("GPU state has no connection".into()))?;
        outbound.post_state(crate::GpuStateWrite::Patch { rev, ops, ack })
    }

    pub(crate) fn send_state_patch_for_context<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        scope: Option<String>,
        base_rev: u64,
        rev: u64,
        ops: Box<RawValue>,
        ack: Option<bool>,
    ) -> Result<(), LxAppError> {
        let msg = StatePatch {
            v: 2,
            kind: "state.patch",
            scope,
            base_rev,
            rev,
            ops,
            ack,
        };
        self.send_json_for_context(transport, work_id, outbound, &msg)
    }

    pub(crate) fn send_event_for_context<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        id: impl Into<String>,
        seq: u64,
        payload: Payload,
    ) -> Result<(), LxAppError> {
        if let Some(outbound) = gpu(outbound) {
            return self.post_gpu(work_id, outbound, || {
                Ok(crate::GpuViewMessage::StreamEvent {
                    id: id.into(),
                    seq,
                    payload: payload.into_tree()?,
                })
            });
        }
        self.send_seq_frame_with_payload_for_context(
            transport,
            work_id,
            outbound,
            "event",
            id.into(),
            seq,
            &payload.into_json(),
        )
    }

    pub(crate) fn send_ch_ack_ok_for_context<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        id: impl Into<String>,
    ) -> Result<(), LxAppError> {
        if let Some(outbound) = gpu(outbound) {
            return self.post_gpu(work_id, outbound, || {
                Ok(crate::GpuViewMessage::ChannelAck {
                    id: id.into(),
                    outcome: Ok(()),
                })
            });
        }
        let msg = ChAck {
            v: 2,
            kind: "ch.ack",
            id: id.into(),
            ok: true,
            error: None,
        };
        self.send_json_for_context(transport, work_id, outbound, &msg)
    }

    pub(crate) fn send_ch_ack_err_for_context<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        id: impl Into<String>,
        code: &str,
        message: Option<String>,
        data: Option<Value>,
    ) -> Result<(), LxAppError> {
        if let Some(outbound) = gpu(outbound) {
            return self.post_gpu(work_id, outbound, || {
                Ok(crate::GpuViewMessage::ChannelAck {
                    id: id.into(),
                    outcome: Err(crate::GpuBridgeError {
                        code: Value::String(code.to_string()),
                        message,
                        data,
                    }),
                })
            });
        }
        let msg = ChAck {
            v: 2,
            kind: "ch.ack",
            id: id.into(),
            ok: false,
            error: Some(BridgeError {
                code: Value::String(code.to_string()),
                message,
                data,
            }),
        };
        self.send_json_for_context(transport, work_id, outbound, &msg)
    }

    pub(crate) fn send_ch_data_for_context<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        id: impl Into<String>,
        seq: u64,
        payload: Payload,
    ) -> Result<(), LxAppError> {
        if let Some(outbound) = gpu(outbound) {
            return self.post_gpu(work_id, outbound, || {
                Ok(crate::GpuViewMessage::ChannelData {
                    id: id.into(),
                    seq,
                    payload: payload.into_tree()?,
                })
            });
        }
        self.send_seq_frame_with_payload_for_context(
            transport,
            work_id,
            outbound,
            "ch.data",
            id.into(),
            seq,
            &payload.into_json(),
        )
    }

    pub(crate) fn send_ch_close_for_context<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        id: impl Into<String>,
        code: Option<String>,
        reason: Option<String>,
    ) -> Result<(), LxAppError> {
        if let Some(outbound) = gpu(outbound) {
            return self.post_gpu(work_id, outbound, || {
                Ok(crate::GpuViewMessage::ChannelClose {
                    id: id.into(),
                    code,
                    reason,
                })
            });
        }
        let msg = ChCloseOut {
            v: 2,
            kind: "ch.close",
            id: id.into(),
            code,
            reason,
        };
        self.send_json_for_context(transport, work_id, outbound, &msg)
    }

    /// A typed message for the current work's GPU View; `build` runs only for current work.
    fn post_gpu(
        &self,
        work_id: Option<SessionWorkId>,
        outbound: &OutboundContext,
        build: impl FnOnce() -> Result<crate::GpuViewMessage, LxAppError>,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work_id) {
            return Ok(());
        }
        outbound.post_gpu(build()?)
    }

    fn send_json_for_context<T: ViewTransport, S: Serialize>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        msg: &S,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work_id) {
            return Ok(());
        }
        let serialized = serde_json::to_string(msg)?;
        if let Some(outbound) = outbound {
            outbound.post(transport, serialized)
        } else {
            transport.post_message_to_view(serialized)
        }
    }

    fn send_seq_frame_with_payload_for_context<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        kind: &'static str,
        id: String,
        seq: u64,
        payload_json: &str,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work_id) {
            return Ok(());
        }
        let serialized = serialize_seq_frame_with_payload(kind, id, seq, payload_json)?;
        if let Some(outbound) = outbound {
            outbound.post(transport, serialized)
        } else {
            transport.post_message_to_view(serialized)
        }
    }

    fn send_hello_ack<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        nonce: String,
        session_id: String,
    ) -> Result<(), LxAppError> {
        let msg = HelloAck {
            v: 2,
            kind: "helloAck",
            nonce,
            protocol: 2,
            session_id,
        };
        self.send_json_for_context(transport, work_id, outbound, &msg)
    }

    fn send_ready<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        session_id: String,
        caller: &host::AuthenticatedCaller,
    ) -> Result<(), LxAppError> {
        let schema = host::host_route_schema(caller);
        let msg = ReadyMsg {
            v: 2,
            kind: "ready",
            session_id,
            host_methods: schema.methods,
            host_channels: schema.channels,
        };
        self.send_json_for_context(transport, work_id, outbound, &msg)
    }

    fn set_ready_if_current(&self, work_id: SessionWorkId, session_id: String) -> bool {
        let mut hs = self.inner.handshake.lock().unwrap();
        if hs
            .connection
            .as_ref()
            .is_none_or(|connection| connection.work_id != work_id)
            || hs.ready
        {
            return false;
        }
        hs.session_id = Some(session_id);
        hs.ready = true;
        true
    }

    /// Revoke the document session and all of its work when its page departs.
    pub(crate) fn cancel_page_work(&self, page: &PageInstance) {
        let connection = {
            let mut handshake = self.inner.handshake.lock().unwrap();
            handshake.session_id = None;
            handshake.ready = false;
            handshake.connection.take()
        };
        if let Some(connection) = connection {
            self.cancel_work(page, connection, PAGE_UNLOADED);
        }
    }

    fn close_host_channels_for_work(
        &self,
        page: &PageInstance,
        work_id: SessionWorkId,
        reason: &str,
    ) {
        let active_host_channels = {
            let mut channels = self.inner.active_host_channels.lock().unwrap();
            let keys = channels
                .iter()
                .filter(|(_, channel)| channel.work_id == work_id)
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>();
            keys.into_iter()
                .filter_map(|key| channels.remove(&key).map(|channel| (key.1, channel)))
                .collect::<Vec<_>>()
        };
        for (id, channel) in active_host_channels {
            let _ = self.send_ch_close_for_context(
                page,
                Some(channel.work_id),
                channel.outbound.as_ref(),
                id,
                Some(BRIDGE_CANCELED.to_string()),
                Some(reason.to_string()),
            );
            channel
                .sender
                .send_close(Some(BRIDGE_CANCELED.to_string()), Some(reason.to_string()));
        }
    }

    fn new_session_id(&self) -> String {
        let count = self.inner.msg_counter.fetch_add(1, Ordering::Relaxed);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let data = format!("{}-{}", ts, count);
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data.as_bytes())
    }

    fn register_host_channel(
        &self,
        id: impl Into<String>,
        work_id: SessionWorkId,
        outbound: Option<OutboundContext>,
        sender: host::ChannelContextSender,
    ) -> usize {
        let token = self
            .inner
            .next_host_channel_token
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("host channel token space exhausted");
        let id = id.into();
        let replaced = self.inner.active_host_channels.lock().unwrap().insert(
            (work_id, id),
            ActiveHostChannel {
                token,
                work_id,
                outbound,
                sender,
            },
        );
        if let Some(replaced) = replaced {
            replaced.sender.send_close(
                Some(BRIDGE_CANCELED.to_string()),
                Some("Channel replaced".to_string()),
            );
        }
        token
    }

    fn register_host_notify(
        &self,
        work_id: SessionWorkId,
        outbound: Option<OutboundContext>,
    ) -> (oneshot::Receiver<()>, PendingHostNotifyGuard) {
        let token = self
            .inner
            .next_host_notify_token
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("host notify token space exhausted");
        let (cancel_tx, cancel_rx) = oneshot::channel();
        self.inner.active_host_notifies.lock().unwrap().insert(
            token,
            ActiveHostNotify {
                work_id,
                _outbound: outbound.clone(),
                cancel_tx,
            },
        );
        (
            cancel_rx,
            PendingHostNotifyGuard {
                state: Arc::clone(&self.inner),
                token,
                _outbound: outbound,
            },
        )
    }

    fn cancel_host_notifies_for_work(&self, work_id: SessionWorkId) {
        let canceled = {
            let mut notifies = self.inner.active_host_notifies.lock().unwrap();
            let tokens = notifies
                .iter()
                .filter(|(_, notify)| notify.work_id == work_id)
                .map(|(token, _)| *token)
                .collect::<Vec<_>>();
            tokens
                .into_iter()
                .filter_map(|token| notifies.remove(&token))
                .collect::<Vec<_>>()
        };
        for notify in canceled {
            let _ = notify.cancel_tx.send(());
        }
    }

    fn take_host_channel(
        &self,
        id: &str,
        work_id: SessionWorkId,
        token: usize,
    ) -> Option<ActiveHostChannel> {
        let mut channels = self.inner.active_host_channels.lock().unwrap();
        let key = (work_id, id.to_owned());
        if channels
            .get(&key)
            .is_some_and(|channel| channel.work_id == work_id && channel.token == token)
        {
            channels.remove(&key)
        } else {
            None
        }
    }

    fn host_channel_is_current(&self, id: &str, work_id: SessionWorkId, token: usize) -> bool {
        self.inner
            .active_host_channels
            .lock()
            .unwrap()
            .get(&(work_id, id.to_owned()))
            .is_some_and(|channel| channel.work_id == work_id && channel.token == token)
    }

    /// Forward inbound `ch.data` payload to the matching host channel sender.
    /// Returns `true` if the channel was found (message consumed), `false` otherwise.
    /// Hands `payload` to the host channel `id`; returns it when no host channel has that id.
    fn send_data_to_host_channel(
        &self,
        id: &str,
        work_id: SessionWorkId,
        payload: Payload,
    ) -> Option<Payload> {
        let lock = self.inner.active_host_channels.lock().unwrap();
        if let Some(channel) = lock.get(&(work_id, id.to_owned())) {
            channel.sender.send_data(payload.into_json());
            None
        } else {
            Some(payload)
        }
    }

    /// Forward a View-initiated `ch.close` to the matching host channel sender.
    /// Removes the sender from the map and returns `true` if found.
    fn close_host_channel_from_view(
        &self,
        id: &str,
        work_id: SessionWorkId,
        code: Option<String>,
        reason: Option<String>,
    ) -> bool {
        let channel = self
            .inner
            .active_host_channels
            .lock()
            .unwrap()
            .remove(&(work_id, id.to_owned()));
        if let Some(channel) = channel {
            channel.sender.send_close(code, reason);
            true
        } else {
            false
        }
    }

    fn dispatch_host_ch_open(
        &self,
        page: &PageInstance,
        context: &BridgeMessageContext,
        id: String,
        host_topic: &str,
        params_json: Option<String>,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        admit_before_host_dispatch(
            context,
            |context| self.admit_host_route(page, context, host_topic),
            || {
                if !self.is_current_work(work.work_id) {
                    return Ok(());
                }
                let Some(caller) = work.caller.as_ref() else {
                    return Ok(());
                };
                let Some(handler) = host::get_channel_handler_for_caller(host_topic, caller) else {
                    let _ = self.send_ch_ack_err_for_context(
                        page,
                        work.work_id,
                        work.outbound.as_ref(),
                        id,
                        BRIDGE_TOPIC_NOT_FOUND,
                        Some(format!("Channel not found: host.{}", host_topic)),
                        None,
                    );
                    return Ok(());
                };
                let Some(work_id) = work.work_id else {
                    return Ok(());
                };

                let (ctx, sender, mut outbound_rx) = host::new_channel_context(id.clone());
                let channel_token =
                    self.register_host_channel(id.clone(), work_id, work.outbound.clone(), sender);
                // Compensate for a revoke that happened after capture but
                // before this channel entered the work registry.
                if !self.is_current_work(Some(work_id)) {
                    if let Some(channel) = self.take_host_channel(&id, work_id, channel_token) {
                        channel.sender.send_close(
                            Some(BRIDGE_CANCELED.to_string()),
                            Some(PAGE_UNLOADED.to_string()),
                        );
                    }
                    return Ok(());
                }

                // Acknowledge the channel open before invoking the handler.
                self.send_ch_ack_ok_for_context(
                    page,
                    Some(work_id),
                    work.outbound.as_ref(),
                    id.clone(),
                )?;

                // Spawn an outbound forwarding task that relays ChannelOutbound messages
                // from the handler back to the View as ch.data / ch.close wire messages.
                let bridge = self.clone();
                let task_page = page.clone();
                let task_id = id.clone();
                let task_outbound = work.outbound.clone();
                crate::executor::spawn(async move {
                    let mut seq = 0u64;
                    while let Some(msg) = outbound_rx.recv().await {
                        match msg {
                            host::ChannelOutbound::Data(payload_json) => {
                                if !bridge.host_channel_is_current(&task_id, work_id, channel_token)
                                {
                                    break;
                                }
                                if let Err(e) = bridge.send_ch_data_for_context(
                                    &task_page,
                                    Some(work_id),
                                    task_outbound.as_ref(),
                                    task_id.clone(),
                                    seq,
                                    Payload::Json(payload_json),
                                ) {
                                    crate::warn!(
                                        "host channel '{}' data send failed: {}",
                                        task_id,
                                        e
                                    )
                                    .with_appid(task_page.appid())
                                    .with_path(task_page.path());
                                }
                                seq += 1;
                            }
                            host::ChannelOutbound::Close { code, reason } => {
                                if bridge
                                    .take_host_channel(&task_id, work_id, channel_token)
                                    .is_some()
                                {
                                    let _ = bridge.send_ch_close_for_context(
                                        &task_page,
                                        Some(work_id),
                                        task_outbound.as_ref(),
                                        task_id.clone(),
                                        code,
                                        reason,
                                    );
                                }
                                break;
                            }
                        }
                    }
                });

                // Call handler.on_open synchronously; the handler is expected to spawn
                // its own async task if it needs to do long-running work.
                if !self.is_current_work(Some(work_id)) {
                    if let Some(channel) = self.take_host_channel(&id, work_id, channel_token) {
                        channel.sender.send_close(
                            Some(BRIDGE_CANCELED.to_string()),
                            Some(PAGE_UNLOADED.to_string()),
                        );
                    }
                    return Ok(());
                }
                let invocation =
                    host::HostInvocationContext::for_dispatch(self.lxapp(), caller, Some(page))
                        .ok_or_else(|| {
                            LxAppError::Bridge(
                                "authenticated caller does not match the native lxapp session"
                                    .to_string(),
                            )
                        })?;
                HOST_EFFECT_WORK.sync_scope(work.clone(), || {
                    handler.on_open(invocation, ctx, params_json)
                });

                Ok(())
            },
        )
    }

    fn dispatch_host_req(
        &self,
        page: &PageInstance,
        context: &BridgeMessageContext,
        id: String,
        host_method: String,
        params_json: Option<String>,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        let route = host_method.clone();
        admit_before_host_dispatch(
            context,
            |context| self.admit_host_route(page, context, &route),
            || {
                if !self.is_current_work(work.work_id) {
                    return Ok(());
                }
                let Some(caller) = work.caller.as_ref() else {
                    return Ok(());
                };
                let Some(handler) = host::get_host_for_caller(&host_method, caller) else {
                    let _ = self.send_res_err_for_context(
                        page,
                        work.work_id,
                        work.outbound.as_ref(),
                        id,
                        BRIDGE_METHOD_NOT_FOUND,
                        Some(format!("Method not found: host.{}", host_method)),
                        None,
                    );
                    return Ok(());
                };
                let Some(work_id) = work.work_id else {
                    return Ok(());
                };

                let invocation =
                    host::HostInvocationContext::for_dispatch(self.lxapp(), caller, Some(page))
                        .ok_or_else(|| {
                            LxAppError::Bridge(
                                "authenticated caller does not match the native lxapp session"
                                    .to_string(),
                            )
                        })?;
                let page = page.clone();
                let task_page = page.clone();
                let bridge = self.clone();
                let (mut cancel_rx, pending_request) =
                    self.inner.pending_requests.register(id.clone(), work_id);
                // The revoke path may have completed its work-id sweep before
                // this insertion. Do not start a handler in that gap.
                if !self.is_current_work(Some(work_id)) {
                    drop(pending_request);
                    let _ = self.send_res_err_for_context(
                        &page,
                        Some(work_id),
                        work.outbound.as_ref(),
                        id,
                        BRIDGE_CANCELED,
                        Some(PAGE_UNLOADED.to_string()),
                        None,
                    );
                    return Ok(());
                }
                let task_id = id.clone();
                let task_host_method = host_method.clone();
                let task_outbound = work.outbound.clone();
                let task_work = work.clone();

                crate::executor::spawn(async move {
                    HOST_EFFECT_WORK
                        .scope(task_work, async move {
                    let started_at = std::time::Instant::now();
                    let (tx, rx) = oneshot::channel();
                    let mut host_cancel_tx = Some(tx);
                    let mut host_fut = handler.call(invocation, params_json, rx);

                    let initial_result: Result<HostOutput, RpcError> = tokio::select! {
                        biased;
                        _ = &mut cancel_rx => {
                            if let Some(tx) = host_cancel_tx.take() {
                                let _ = tx.send(());
                            }
                            Err(RpcError::new(BRIDGE_CANCELED, Some(PAGE_UNLOADED.to_string())))
                        }
                        res = &mut host_fut => {
                            match res {
                                Ok(output) => Ok(output),
                                Err(err) => Err(rpc_error_from_lxapp_error(&err)),
                            }
                        }
                    };

                    let send_result = match initial_result {
                        Ok(HostOutput::Json(json)) => bridge.send_res_ok_for_context(
                            &task_page,
                            Some(work_id),
                            task_outbound.as_ref(),
                            task_id.clone(),
                            Payload::Json(json),
                        ),
                        Ok(HostOutput::Stream(stream)) => {
                            match bridge
                                .consume_host_stream(
                                    &task_page,
                                    work_id,
                                    task_outbound.as_ref(),
                                    &task_id,
                                    stream,
                                    &mut cancel_rx,
                                    host_cancel_tx,
                                )
                                .await
                            {
                                Ok(json) => bridge.send_res_ok_for_context(
                                    &task_page,
                                    Some(work_id),
                                    task_outbound.as_ref(),
                                    task_id.clone(),
                                    Payload::Json(json),
                                ),
                                Err(err) => bridge.send_res_err_for_context(
                                    &task_page,
                                    Some(work_id),
                                    task_outbound.as_ref(),
                                    task_id.clone(),
                                    &err.code,
                                    err.message,
                                    err.data,
                                ),
                            }
                        }
                        Err(err) => bridge.send_res_err_for_context(
                            &task_page,
                            Some(work_id),
                            task_outbound.as_ref(),
                            task_id.clone(),
                            &err.code,
                            err.message,
                            err.data,
                        ),
                    };

                    drop(pending_request);

                    let elapsed = started_at.elapsed();
                    if elapsed > std::time::Duration::from_secs(3) {
                        crate::warn!(
                            "[{}] host req '{}' slow: {:?}",
                            task_page.path(),
                            task_host_method,
                            elapsed
                        )
                        .with_appid(task_page.appid())
                        .with_path(task_page.path());
                    }

                    if let Err(err) = send_result {
                        crate::warn!("host req '{}' reply failed: {}", task_host_method, err)
                            .with_appid(task_page.appid())
                            .with_path(task_page.path());
                    }
                    })
                    .await;
                });

                Ok(())
            },
        )
    }

    fn dispatch_host_notify(
        &self,
        page: &PageInstance,
        context: &BridgeMessageContext,
        host_method: String,
        params_json: Option<String>,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        let route = host_method.clone();
        admit_before_host_dispatch(
            context,
            |context| self.admit_host_route(page, context, &route),
            || {
                if !self.is_current_work(work.work_id) {
                    return Ok(());
                }
                let Some(caller) = work.caller.as_ref() else {
                    return Ok(());
                };
                let Some(handler) = host::get_host_for_caller(&host_method, caller) else {
                    return Ok(());
                };
                let Some(work_id) = work.work_id else {
                    return Ok(());
                };

                let invocation =
                    host::HostInvocationContext::for_dispatch(self.lxapp(), caller, Some(page))
                        .ok_or_else(|| {
                            LxAppError::Bridge(
                                "authenticated caller does not match the native lxapp session"
                                    .to_string(),
                            )
                        })?;
                let appid = page.appid();
                let path = page.path();
                let task_host_method = host_method.clone();
                let (cancel_rx, notify_guard) =
                    self.register_host_notify(work_id, work.outbound.clone());
                // Same post-registration compensation as requests: a reset
                // that won before insertion must not leave a live notify.
                if !self.is_current_work(Some(work_id)) {
                    drop(notify_guard);
                    return Ok(());
                }
                let task_work = work.clone();
                crate::executor::spawn(async move {
                    HOST_EFFECT_WORK
                        .scope(task_work, async move {
                            let _notify_guard = notify_guard;
                            match handler.call(invocation, params_json, cancel_rx).await {
                                Ok(HostOutput::Json(_)) => {}
                                Ok(HostOutput::Stream(_)) => {
                                    crate::warn!(
                                        "host notify '{}' returned a stream; dropping output",
                                        task_host_method
                                    )
                                    .with_appid(appid.clone())
                                    .with_path(path.clone());
                                }
                                Err(err) => {
                                    crate::warn!(
                                        "host notify '{}' failed: {}",
                                        task_host_method,
                                        err
                                    )
                                    .with_appid(appid)
                                    .with_path(path);
                                }
                            }
                        })
                        .await;
                });
                Ok(())
            },
        )
    }

    async fn consume_host_stream(
        &self,
        page: &PageInstance,
        work_id: SessionWorkId,
        outbound: Option<&OutboundContext>,
        stream_id: &str,
        mut stream: HostStream,
        cancel_rx: &mut oneshot::Receiver<()>,
        mut host_cancel_tx: Option<oneshot::Sender<()>>,
    ) -> Result<String, RpcError> {
        let mut seq = 0u64;

        loop {
            let next_item = tokio::select! {
                biased;
                _ = &mut *cancel_rx => {
                    if let Some(tx) = host_cancel_tx.take() {
                        let _ = tx.send(());
                    }
                    return Err(RpcError::new(BRIDGE_CANCELED, Some(PAGE_UNLOADED.to_string())));
                }
                item = stream.next() => item,
            };

            match next_item {
                Some(Ok(HostStreamItem::Event(payload_json))) => {
                    let payload_json = RawValue::from_string(payload_json)
                        .map(|raw| raw.get().to_owned())
                        .map_err(|e| {
                            RpcError::new(
                                BRIDGE_INTERNAL_ERROR,
                                Some(format!("Host stream emitted invalid JSON: {}", e)),
                            )
                        })?;
                    self.send_event_for_context(
                        page,
                        Some(work_id),
                        outbound,
                        stream_id.to_string(),
                        seq,
                        Payload::Json(payload_json),
                    )
                    .map_err(|e| RpcError::new(BRIDGE_INTERNAL_ERROR, Some(e.to_string())))?;
                    seq += 1;
                }
                Some(Ok(HostStreamItem::Return(result_json))) => {
                    return RawValue::from_string(result_json)
                        .map(|raw| raw.get().to_owned())
                        .map_err(|e| {
                            RpcError::new(
                                BRIDGE_INTERNAL_ERROR,
                                Some(format!("Host stream returned invalid JSON: {}", e)),
                            )
                        });
                }
                Some(Err(err)) => return Err(rpc_error_from_lxapp_error(&err)),
                None => return Ok("null".to_string()),
            }
        }
    }
}

fn rpc_error_from_lxapp_error(err: &LxAppError) -> RpcError {
    if let LxAppError::RongJSHost {
        code,
        message,
        data,
    } = err
    {
        return RpcError {
            code: code.clone(),
            message: Some(message.clone()),
            data: data.clone(),
        };
    }
    if matches!(err, LxAppError::Bridge(msg) if msg == "Canceled") {
        return RpcError::new(BRIDGE_CANCELED, Some(PAGE_UNLOADED.to_string()));
    }
    RpcError::new(BRIDGE_INTERNAL_ERROR, Some(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn gpu_outbound_keeps_its_endpoint_and_generation_without_document_fallback() {
        #[derive(Default)]
        struct Endpoint(Mutex<Vec<(u64, String)>>);
        impl crate::GpuPageView for Endpoint {
            fn write_state(
                &self,
                generation: crate::GpuViewGeneration,
                write: crate::GpuStateWrite,
            ) -> Result<(), LxAppError> {
                self.0
                    .lock()
                    .unwrap()
                    .push((generation.get(), format!("{write:?}")));
                Ok(())
            }
            fn deliver(
                &self,
                generation: crate::GpuViewGeneration,
                message: crate::GpuViewMessage,
            ) -> Result<(), LxAppError> {
                self.0
                    .lock()
                    .unwrap()
                    .push((generation.get(), format!("{message:?}")));
                Ok(())
            }
            fn connect(&self, _: crate::GpuViewGeneration) -> Result<(), LxAppError> {
                Ok(())
            }
            fn request_leave(&self, _: crate::LeaveReason) -> Result<(), LxAppError> {
                Ok(())
            }
            fn update_environment(&self, _: crate::PageViewEnvironment) -> Result<(), LxAppError> {
                Ok(())
            }
            fn reload(&self) -> Result<(), LxAppError> {
                Ok(())
            }
            fn park(&self) -> Result<(), LxAppError> {
                Ok(())
            }
            fn retain_surface(&self, _: bool) -> Result<(), LxAppError> {
                Ok(())
            }
            fn dispose(&self) {}
        }
        struct OtherTransport;
        impl ViewTransport for OtherTransport {
            fn post_message_to_view(&self, _: String) -> Result<(), LxAppError> {
                panic!("must not target current View")
            }
        }
        let target = Arc::new(Endpoint::default());
        let endpoint: Arc<dyn crate::GpuPageView> = target.clone();
        let outbound = OutboundContext::Gpu {
            endpoint: Arc::downgrade(&endpoint),
            generation: crate::GpuViewGeneration(42),
        };
        assert!(outbound.post(&OtherTransport, "frame".into()).is_err());
        outbound
            .post_gpu(crate::GpuViewMessage::ChannelClose {
                id: "c".into(),
                code: None,
                reason: None,
            })
            .unwrap();
        let reply = target.0.lock().unwrap()[0].clone();
        assert!(
            reply.0 == 42 && reply.1.starts_with("ChannelClose"),
            "{reply:?}"
        );
        let ops: Vec<crate::GpuPatchOp> = serde_json::from_str(
            r#"[{"op":"replace","path":"/n","value":1},{"op":"remove","path":"/m"}]"#,
        )
        .unwrap();
        assert_eq!(ops[1], crate::GpuPatchOp::Remove { path: "/m".into() });
        assert!(
            serde_json::from_str::<Vec<crate::GpuPatchOp>>(
                r#"[{"op":"move","from":"/a","path":"/b"}]"#
            )
            .is_err()
        );
        outbound
            .post_state(crate::GpuStateWrite::Patch {
                rev: 3,
                ops,
                ack: true,
            })
            .unwrap();
        let state = target.0.lock().unwrap()[1].clone();
        assert!(
            state.0 == 42 && state.1.starts_with("Patch { rev: 3"),
            "{state:?}"
        );
        drop(endpoint);
        drop(target);
        assert!(
            outbound
                .post_gpu(crate::GpuViewMessage::ChannelClose {
                    id: "late".into(),
                    code: None,
                    reason: None,
                })
                .is_err()
        );
    }

    #[derive(Debug, PartialEq, Eq)]
    struct TestContext {
        native_view: u64,
        frame: &'static str,
    }

    #[test]
    fn host_admission_seam_requires_native_message_context() {
        let _seam: fn(
            &PageBridge,
            &PageInstance,
            &BridgeMessageContext,
            &str,
        ) -> Result<(), LxAppError> = PageBridge::admit_host_route;
    }

    #[test]
    fn host_dispatch_preserves_context_identity_and_runs_after_admission() {
        let context = TestContext {
            native_view: 41,
            frame: "top-level",
        };
        let events = RefCell::new(Vec::new());

        let result = admit_before_host_dispatch(
            &context,
            |received| {
                assert!(std::ptr::eq(received, &context));
                assert_eq!(received.native_view, 41);
                assert_eq!(received.frame, "top-level");
                events.borrow_mut().push("admit");
                Ok(())
            },
            || {
                events.borrow_mut().push("dispatch");
                Ok(())
            },
        );

        assert!(result.is_ok());
        assert_eq!(*events.borrow(), ["admit", "dispatch"]);
    }

    #[test]
    fn rejected_host_admission_prevents_dispatch() {
        let context = TestContext {
            native_view: 9,
            frame: "subframe",
        };
        let dispatched = RefCell::new(false);

        let result = admit_before_host_dispatch(
            &context,
            |received| {
                assert!(std::ptr::eq(received, &context));
                Err(LxAppError::Bridge("denied".to_string()))
            },
            || {
                *dispatched.borrow_mut() = true;
                Ok(())
            },
        );

        assert!(matches!(result, Err(LxAppError::Bridge(message)) if message == "denied"));
        assert!(!*dispatched.borrow());
    }

    #[test]
    fn stale_hello_work_does_not_match_its_successor() {
        let successor = Arc::new(BridgeConnection {
            work_id: SessionWorkId::for_test(12),
            outbound: None,
            caller: host::AuthenticatedCaller::standard_for_test(1),
        });

        assert!(!connection_matches_work(
            Some(&successor),
            Some(SessionWorkId::for_test(11)),
        ));
        assert!(!connection_matches_work(Some(&successor), None));
        assert!(connection_matches_work(
            Some(&successor),
            Some(SessionWorkId::for_test(12))
        ));
    }

    #[test]
    fn document_gate_accepts_current_unready_work_and_rejects_replacement() {
        let handshake = Arc::new(Mutex::new(HandshakeState {
            session_id: None,
            ready: false,
            connection: Some(Arc::new(BridgeConnection {
                work_id: SessionWorkId::for_test(20),
                outbound: None,
                caller: host::AuthenticatedCaller::standard_for_test(1),
            })),
        }));
        let gate = Arc::new(SessionOutboundGate {
            handshake: Arc::downgrade(&handshake),
            work_id: SessionWorkId::for_test(20),
        });

        let mut deliveries = 0;
        assert!(gate.with_active(&mut || deliveries += 1));
        assert_eq!(deliveries, 1, "helloAck is allowed before ready");

        handshake.lock().unwrap().connection = Some(Arc::new(BridgeConnection {
            work_id: SessionWorkId::for_test(21),
            outbound: None,
            caller: host::AuthenticatedCaller::standard_for_test(1),
        }));
        assert!(!gate.with_active(&mut || deliveries += 1));
        assert_eq!(deliveries, 1, "retired work cannot reach the same document");

        drop(handshake);
        assert!(!gate.with_active(&mut || deliveries += 1));
    }

    #[test]
    fn pending_request_registry_cancels_all_and_auto_unregisters() {
        let pending = Arc::new(PendingRequestRegistry::default());
        let (mut first_rx, first_guard) = pending.register("first".to_string(), SessionWorkId(1));
        let (mut second_rx, second_guard) =
            pending.register("second".to_string(), SessionWorkId(1));

        assert_eq!(pending.len(), 2);
        drop(first_guard);
        assert_eq!(pending.len(), 1);
        assert!(first_rx.try_recv().is_err());

        pending.cancel_all();

        assert_eq!(pending.len(), 0);
        drop(second_guard);
        assert_eq!(pending.len(), 0);
        assert_eq!(second_rx.try_recv(), Ok(()));
    }

    #[test]
    fn completed_request_cannot_remove_a_reused_request_id() {
        let pending = Arc::new(PendingRequestRegistry::default());
        let (mut first_rx, first_guard) = pending.register("same".to_string(), SessionWorkId(1));
        let (mut second_rx, second_guard) = pending.register("same".to_string(), SessionWorkId(1));

        assert_eq!(first_rx.try_recv(), Ok(()));
        drop(first_guard);
        assert_eq!(pending.len(), 1);

        pending.cancel_all();

        drop(second_guard);
        assert_eq!(pending.len(), 0);
        assert_eq!(second_rx.try_recv(), Ok(()));
    }

    #[test]
    fn canceling_retired_work_cannot_cancel_successor_request_with_same_id() {
        let pending = Arc::new(PendingRequestRegistry::default());
        let (mut old_rx, old_guard) = pending.register("same".to_string(), SessionWorkId(7));
        let (mut new_rx, new_guard) = pending.register("same".to_string(), SessionWorkId(8));

        pending.cancel_work(SessionWorkId(7));

        assert_eq!(old_rx.try_recv(), Ok(()));
        assert!(new_rx.try_recv().is_err());
        assert_eq!(pending.len(), 1);
        drop(old_guard);
        drop(new_guard);
    }

    #[test]
    fn stale_cancel_cannot_remove_successor_request_with_same_id() {
        let pending = Arc::new(PendingRequestRegistry::default());
        let (mut old_rx, old_guard) = pending.register("same".to_string(), SessionWorkId(7));
        let (mut new_rx, new_guard) = pending.register("same".to_string(), SessionWorkId(8));

        pending.cancel(SessionWorkId(7), "same");

        assert_eq!(old_rx.try_recv(), Ok(()));
        assert!(new_rx.try_recv().is_err());
        drop(old_guard);
        drop(new_guard);
    }

    #[test]
    fn host_channel_registry_keeps_same_id_from_successive_works_distinct() {
        let (_old_context, old_sender, _old_outbound) =
            host::new_channel_context("same".to_string());
        let (_new_context, new_sender, _new_outbound) =
            host::new_channel_context("same".to_string());
        let mut channels = HashMap::new();
        channels.insert(
            (SessionWorkId::for_test(40), "same".to_string()),
            ActiveHostChannel {
                token: 1,
                work_id: SessionWorkId::for_test(40),
                outbound: None,
                sender: old_sender,
            },
        );
        channels.insert(
            (SessionWorkId::for_test(41), "same".to_string()),
            ActiveHostChannel {
                token: 1,
                work_id: SessionWorkId::for_test(41),
                outbound: None,
                sender: new_sender,
            },
        );

        assert_eq!(channels.len(), 2);
        assert!(channels.contains_key(&(SessionWorkId::for_test(40), "same".to_string())));
        assert!(channels.contains_key(&(SessionWorkId::for_test(41), "same".to_string())));
    }

    #[test]
    fn send_event_embeds_payload_without_reencoding() {
        assert_eq!(
            serialize_seq_frame_with_payload("event", "req\"1".to_string(), 7, r#"{"token":"hi"}"#)
                .unwrap(),
            r#"{"v":2,"kind":"event","id":"req\"1","seq":7,"payload":{"token":"hi"}}"#
        );
    }

    #[test]
    fn send_ch_data_embeds_scalar_payload_without_reencoding() {
        assert_eq!(
            serialize_seq_frame_with_payload("ch.data", "ch-1".to_string(), 3, "true").unwrap(),
            r#"{"v":2,"kind":"ch.data","id":"ch-1","seq":3,"payload":true}"#
        );
    }

    #[test]
    fn v2_view_request_wire_is_byte_stable() {
        let frame = serde_json::to_string(&ViewReqOut {
            v: 2,
            kind: "req",
            id: "lv_1".to_string(),
            method: "view.confirm".to_string(),
            params: Some(serde_json::json!({ "title": "Confirm" })),
            cap: "view".to_string(),
        })
        .unwrap();
        assert_eq!(
            frame,
            r#"{"v":2,"kind":"req","id":"lv_1","method":"view.confirm","params":{"title":"Confirm"},"cap":"view"}"#
        );
    }

    #[test]
    fn v2_hello_ack_wire_is_byte_stable() {
        let frame = serde_json::to_string(&HelloAck {
            v: 2,
            kind: "helloAck",
            nonce: "legacy-nonce".to_string(),
            protocol: 2,
            session_id: "legacy-session".to_string(),
        })
        .unwrap();
        assert_eq!(
            frame,
            r#"{"v":2,"kind":"helloAck","nonce":"legacy-nonce","protocol":2,"sessionId":"legacy-session"}"#
        );
    }
}
