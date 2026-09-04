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

mod protocol;

#[allow(unused_imports)] // Re-exported for the next document-session binding step.
pub(crate) use protocol::{
    BoundV3Protocol, ChOpenMsg, HelloMsg, IncomingMessage, JsonPatchOp, NotifyMsg, ReqMsg,
    V3InboundBinding,
};

use protocol::*;

use crate::LxAppError;
use crate::host::{self, HostOutput, HostStream, HostStreamItem};
use crate::lxapp::LxApp;
use crate::page::PageInstance;
use base64::Engine;
use futures::StreamExt;
use lingxia_webview::{
    DocumentGeneration, DocumentOutboundGate, IncomingWebMessage, WebMessageContext,
};
use serde::Serialize;
use serde_json::Value;
use serde_json::value::RawValue;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// Browser host-TCB lease used only to install a required V3 bridge binding.
/// It may be current while bootstrap is pending, but its supplied outbound
/// gate remains Active-only and is therefore safe to retain for later sends.
#[doc(hidden)]
pub trait RequiredV3DocumentGate: Send + Sync {
    fn with_bootstrap_pending_current(
        &self,
        context: &WebMessageContext,
        action: &mut dyn FnMut(crate::ControlDocumentAuthority, Arc<dyn DocumentOutboundGate>),
    ) -> bool;
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
        params_json: Option<String>,
        cancel_rx: oneshot::Receiver<()>,
        pending_request: PendingRequestGuard,
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
        params_json: Option<String>,
    },
    ChData {
        work_id: Option<SessionWorkId>,
        id: String,
        payload_json: String,
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
    /// Internal lifecycle transitions, never decoded from a document frame.
    BeginSessionWork {
        work_id: SessionWorkId,
    },
    CancelSessionWork {
        work_id: SessionWorkId,
    },
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
        if let Some(controller) = self.webview_controller() {
            controller
                .post_message(&message_json)
                .map_err(LxAppError::from)
        } else {
            Err(LxAppError::WebView("WebView not ready".to_string()))
        }
    }

    fn post_message_to_document(
        &self,
        expected_generation: DocumentGeneration,
        gate: Arc<dyn DocumentOutboundGate>,
        message_json: String,
    ) -> Result<(), LxAppError> {
        if let Some(controller) = self.webview_controller() {
            controller
                .post_message_to_document(expected_generation, gate, &message_json)
                .map_err(LxAppError::from)
        } else {
            Err(LxAppError::WebView("WebView not ready".to_string()))
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
    protocol: BridgeProtocol,
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
pub(crate) struct OutboundContext {
    expected_generation: DocumentGeneration,
    gate: Arc<dyn DocumentOutboundGate>,
    binding: V3OutboundBinding,
}

/// One bridge lifetime, legacy or V3. Work must retain this exact connection
/// rather than consulting the mutable handshake again after an async boundary.
struct BridgeConnection {
    work_id: SessionWorkId,
    outbound: Option<OutboundContext>,
    caller: host::AuthenticatedCaller,
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
    execution_permit: Option<crate::RequiredV3ExecutionPermit>,
}

tokio::task_local! {
    /// Native handlers inherit the document work that admitted them. Any Page
    /// API they invoke after an await must not silently capture a successor.
    static HOST_EFFECT_WORK: CapturedSessionWork;
}

struct DecodedIncoming {
    message: IncomingMessage,
    work: CapturedSessionWork,
    bound_v3: bool,
    ready: bool,
    session_id: Option<String>,
}

/// Opaque, single-use browser ingress prepared while BrowserDocumentSessions
/// holds its exact Active lease. Its fields never leave this crate: execution
/// cannot be re-authorized with a caller proof after the registry lock drops.
#[doc(hidden)]
pub struct PreparedRequiredV3Incoming {
    incoming: IncomingWebMessage,
    decoded: DecodedIncoming,
    execution_gate: crate::RequiredV3ExecutionGate,
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
    handshake: Mutex<HandshakeState>,
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

/// Cancellation detached from a required-V3 connection replacement. Browser
/// lifecycle must finish it only after releasing its document-session mutex:
/// cancellation can synchronously close channels and reach an outbound gate.
#[doc(hidden)]
pub struct DeferredRequiredV3Cancellation {
    bridge: PageBridge,
    page: PageInstance,
    previous: Option<Arc<BridgeConnection>>,
}

async fn wait_for_execution_permit_cancellation(permit: Option<crate::RequiredV3ExecutionPermit>) {
    let Some(mut cancellation) = permit.map(|permit| permit.cancellation_receiver()) else {
        std::future::pending::<()>().await;
        return;
    };
    if *cancellation.borrow() {
        return;
    }
    while cancellation.changed().await.is_ok() {
        if *cancellation.borrow() {
            return;
        }
    }
    std::future::pending::<()>().await;
}

impl DeferredRequiredV3Cancellation {
    #[doc(hidden)]
    pub fn finish(self) {
        if let Some(previous) = self.previous {
            self.bridge
                .cancel_work(&self.page, previous, "Session replaced");
        }
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
                handshake: Mutex::new(HandshakeState::default()),
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

    /// Optional exact-lease installer retained for host integrations that
    /// already hold an active document gate.
    #[allow(dead_code)]
    pub(crate) fn bind_v3_protocol(
        &self,
        page: &PageInstance,
        protocol: BoundV3Protocol,
        expected_generation: DocumentGeneration,
        gate: Arc<dyn DocumentOutboundGate>,
    ) -> Result<(), LxAppError> {
        let mut outcome = Ok(());
        let mut replaced = None;
        let mut protocol = Some(protocol);
        let mut install = || {
            let mut handshake = self.inner.handshake.lock().unwrap();
            if handshake.ready {
                outcome = Err(LxAppError::Bridge(
                    "cannot bind V3 protocol after bridge readiness".to_string(),
                ));
                return;
            }
            let protocol = protocol
                .take()
                .expect("active document gate invoked bind more than once");
            let binding = protocol.outbound_binding();
            let connection = Arc::new(BridgeConnection {
                work_id: self.next_session_work_id(),
                outbound: Some(OutboundContext {
                    expected_generation,
                    gate: Arc::clone(&gate),
                    binding,
                }),
                caller: host::AuthenticatedCaller::for_lxapp(&self.inner.lxapp),
            });
            // The lease gate is held outside the handshake lock.  Thus a
            // revoked browser document cannot install an old binding over a
            // successor, while Begin remains linearized with installation.
            if let Err(err) = self.begin_work_locked(page, connection.work_id) {
                outcome = Err(err);
                return;
            }
            replaced = handshake.connection.replace(connection);
            handshake.protocol = BridgeProtocol::BoundV3(protocol);
            handshake.session_id = None;
            handshake.ready = false;
        };
        if !gate.with_active(&mut install) {
            return Err(LxAppError::Bridge(
                "cannot bind a revoked document session".to_string(),
            ));
        }
        outcome?;
        if let Some(previous) = replaced {
            self.cancel_work(page, previous, "Session replaced");
        }
        Ok(())
    }

    #[doc(hidden)]
    pub fn bind_required_v3_document(
        &self,
        page: &PageInstance,
        context: &WebMessageContext,
        pending: &dyn RequiredV3DocumentGate,
    ) -> Result<(), LxAppError> {
        let mut outcome = Ok(());
        let mut deferred = None;
        let mut install = |authority: crate::ControlDocumentAuthority,
                           outbound_gate: Arc<dyn DocumentOutboundGate>| {
            match self.bind_required_v3_authority(page, context, authority, outbound_gate) {
                Ok(cancellation) => deferred = Some(cancellation),
                Err(error) => outcome = Err(error),
            }
        };
        if !pending.with_bootstrap_pending_current(context, &mut install) {
            return Err(LxAppError::Bridge(
                "required V3 document is no longer bootstrap-current".to_string(),
            ));
        }
        outcome?;
        if let Some(deferred) = deferred {
            deferred.finish();
        }
        Ok(())
    }

    /// Install a required-V3 bridge while the browser session registry already
    /// holds the matching BootstrapPending entry.  Browser host TCB must call
    /// this only from that registry-held closure; this method intentionally
    /// does not re-enter the registry through a lease.
    #[doc(hidden)]
    pub fn bind_required_v3_authority(
        &self,
        page: &PageInstance,
        context: &WebMessageContext,
        authority: crate::ControlDocumentAuthority,
        outbound_gate: Arc<dyn DocumentOutboundGate>,
    ) -> Result<DeferredRequiredV3Cancellation, LxAppError> {
        let expected_generation = match context.document() {
            lingxia_webview::DocumentBinding::Bound(generation) => generation,
            lingxia_webview::DocumentBinding::Unbound => {
                return Err(LxAppError::Bridge(
                    "required V3 document is unbound".to_string(),
                ));
            }
        };
        let protocol = BoundV3Protocol::new(authority.v3_inbound_binding())
            .expect("native-generated control document binding must be valid");
        let connection = Arc::new(BridgeConnection {
            work_id: self.next_session_work_id(),
            outbound: Some(OutboundContext {
                expected_generation,
                gate: outbound_gate,
                binding: protocol.outbound_binding(),
            }),
            // Browser audience is a registry-held ingress scope, never a
            // durable bridge property.
            caller: host::AuthenticatedCaller::for_lxapp(&self.inner.lxapp),
        });
        let replaced = {
            let mut handshake = self.inner.handshake.lock().unwrap();
            // A trusted successor navigation is allowed to replace a ready
            // predecessor. Begin and replacement share this lock; the old
            // work is cancelled only after it has been detached.
            self.begin_work_locked(page, connection.work_id)?;
            let replaced = handshake.connection.replace(connection);
            handshake.protocol = BridgeProtocol::BoundV3(protocol);
            handshake.session_id = None;
            handshake.ready = false;
            replaced
        };
        Ok(DeferredRequiredV3Cancellation {
            bridge: self.clone(),
            page: page.clone(),
            previous: replaced,
        })
    }

    /// Remove exactly the required-V3 connection authenticated by `authority`.
    /// Browser lifecycle calls this only after it has revoked the matching
    /// registry entry and released that registry lock.
    #[doc(hidden)]
    pub fn revoke_required_v3_document(
        &self,
        page: &PageInstance,
        authority: crate::ControlDocumentAuthority,
    ) -> bool {
        let binding = authority.v3_inbound_binding();
        let previous = {
            let mut handshake = self.inner.handshake.lock().unwrap();
            let BridgeProtocol::BoundV3(protocol) = &handshake.protocol else {
                return false;
            };
            if !protocol.authenticates(&binding) {
                return false;
            }
            let previous = handshake.connection.take();
            handshake.protocol = BridgeProtocol::default();
            handshake.session_id = None;
            handshake.ready = false;
            previous
        };
        if let Some(previous) = previous {
            self.cancel_work(page, previous, "Browser document revoked");
        }
        true
    }

    /// Validate an exact active browser document before its registry-held
    /// ingress closure prepares its frame.
    #[doc(hidden)]
    pub fn promote_active_browser_document(
        &self,
        authority: crate::ControlDocumentAuthority,
    ) -> bool {
        let binding = authority.v3_inbound_binding();
        let handshake = self.inner.handshake.lock().unwrap();
        let BridgeProtocol::BoundV3(protocol) = &handshake.protocol else {
            return false;
        };
        if !protocol.authenticates(&binding) {
            return false;
        }
        handshake.connection.is_some()
    }

    fn next_session_work_id(&self) -> SessionWorkId {
        let next = self
            .inner
            .next_session_work_id
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("SessionWorkId space exhausted");
        SessionWorkId(next)
    }

    /// Replace exactly the connection that admitted a legacy hello.  The
    /// comparison, Begin enqueue, and replacement share the handshake lock:
    /// a decoded stale V2 hello must never reset a V3 successor.
    fn replace_with_legacy_session_work(
        &self,
        page: &PageInstance,
        expected_work: Option<SessionWorkId>,
    ) -> Result<Option<SessionWorkId>, LxAppError> {
        let connection = Arc::new(BridgeConnection {
            work_id: self.next_session_work_id(),
            outbound: None,
            caller: host::AuthenticatedCaller::for_lxapp(&self.inner.lxapp),
        });
        let replaced = {
            let mut handshake = self.inner.handshake.lock().unwrap();
            if !connection_matches_work(handshake.connection.as_ref(), expected_work) {
                return Ok(None);
            }
            self.begin_work_locked(page, connection.work_id)?;
            let replaced = handshake.connection.replace(Arc::clone(&connection));
            handshake.protocol = BridgeProtocol::LegacyV2;
            handshake.session_id = None;
            handshake.ready = false;
            replaced
        };
        if let Some(previous) = replaced {
            self.cancel_work(page, previous, "Session replaced");
        }
        Ok(Some(connection.work_id))
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

    fn work_effect_is_active(work: &CapturedSessionWork) -> bool {
        work.execution_permit
            .as_ref()
            .is_none_or(crate::RequiredV3ExecutionPermit::is_active)
    }

    fn work_try_commit_effect(work: &CapturedSessionWork) -> bool {
        work.execution_permit
            .as_ref()
            .is_none_or(crate::RequiredV3ExecutionPermit::try_commit_effect)
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

        self.execute_decoded_incoming(page, context, decoded)
    }

    fn execute_decoded_incoming(
        &self,
        page: &PageInstance,
        context: &WebMessageContext,
        decoded: DecodedIncoming,
    ) -> Result<(), LxAppError> {
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

    /// Prepare an authenticated browser frame while BrowserDocumentSessions
    /// holds its exact Active entry. This method performs no outbound send,
    /// typed parameter decoding, handler clone, task creation, or handler
    /// invocation. The returned opaque value must be executed after releasing
    /// the registry lock.
    #[doc(hidden)]
    pub fn prepare_required_v3_incoming(
        &self,
        page: &PageInstance,
        incoming: IncomingWebMessage,
        authority: crate::ControlDocumentAuthority,
        execution_gate: crate::RequiredV3ExecutionGate,
    ) -> Result<PreparedRequiredV3Incoming, LxAppError> {
        self.admit_incoming(page, incoming.context())?;
        let binding = authority.v3_inbound_binding();
        {
            let handshake = self.inner.handshake.lock().unwrap();
            let BridgeProtocol::BoundV3(protocol) = &handshake.protocol else {
                return Err(LxAppError::Bridge(
                    "required V3 protocol is not bound".to_string(),
                ));
            };
            if !protocol.authenticates(&binding) || handshake.connection.is_none() {
                return Err(LxAppError::Bridge(
                    "browser document binding is not current".to_string(),
                ));
            }
        }
        let mut decoded = self.predecode_inbound(incoming.body())?;
        if !decoded.bound_v3 || !self.is_current_work(decoded.work.work_id) {
            return Err(LxAppError::Bridge(
                "browser document work was revoked".to_string(),
            ));
        }
        let caller = host::AuthenticatedCaller::active_browser_document(authority);
        self.pre_authorize_browser_route(&decoded.message, &caller)?;
        decoded.work.caller = Some(caller);
        Ok(PreparedRequiredV3Incoming {
            incoming,
            decoded,
            execution_gate,
        })
    }

    /// Execute a browser frame prepared under the lifecycle registry lock.
    /// The first action rechecks exact session work before any side effect.
    #[doc(hidden)]
    pub fn execute_prepared_required_v3_incoming(
        &self,
        page: &PageInstance,
        mut prepared: PreparedRequiredV3Incoming,
    ) -> Result<(), LxAppError> {
        // The browser registry flips this exact gate while it still owns its
        // revoke transition. A prepared frame cannot start typed dispatch
        // after that linearization point, even before Page cleanup runs.
        let Some(execution_permit) = prepared.execution_gate.try_begin() else {
            return Ok(());
        };
        prepared.decoded.work.execution_permit = Some(execution_permit);
        if !self.is_current_work(prepared.decoded.work.work_id) {
            return Ok(());
        }
        if !Self::work_try_commit_effect(&prepared.decoded.work) {
            return Ok(());
        }
        self.execute_decoded_incoming(page, prepared.incoming.context(), prepared.decoded)
    }

    fn predecode_inbound(&self, frame: &str) -> Result<DecodedIncoming, LxAppError> {
        let handshake = self.inner.handshake.lock().unwrap();
        let bound_v3 = matches!(handshake.protocol, BridgeProtocol::BoundV3(_));
        let message = handshake
            .protocol
            .predecode_inbound(frame)
            .map_err(|_| LxAppError::Bridge("invalid bridge protocol envelope".to_string()))?;
        let version = message
            .version()
            .ok_or_else(|| LxAppError::Bridge("invalid bridge protocol version".to_string()))?;
        if !handshake.protocol.accepts_version(version) {
            return Err(LxAppError::Bridge(format!(
                "Unsupported protocol: {version}"
            )));
        }
        let work = handshake
            .connection
            .as_ref()
            .map(|connection| CapturedSessionWork {
                work_id: Some(connection.work_id),
                outbound: connection.outbound.clone(),
                caller: Some(connection.caller.clone()),
                execution_permit: None,
            })
            .unwrap_or(CapturedSessionWork {
                work_id: None,
                outbound: None,
                caller: None,
                execution_permit: None,
            });
        Ok(DecodedIncoming {
            message,
            work,
            bound_v3,
            ready: handshake.ready,
            session_id: handshake.protocol.session_id().map(str::to_owned),
        })
    }

    /// Consult immutable route policy while the browser lifecycle entry is
    /// still held. Unknown routes remain eligible for the established
    /// post-lock not-found response.
    fn pre_authorize_browser_route(
        &self,
        message: &IncomingMessage,
        caller: &host::AuthenticatedCaller,
    ) -> Result<(), LxAppError> {
        let authorized = match message {
            IncomingMessage::Req(msg) if msg.method.starts_with("host.") => {
                host::host_route_is_authorized(&msg.method["host.".len()..], caller)
            }
            IncomingMessage::Notify(msg) if msg.method.starts_with("host.") => {
                host::host_route_is_authorized(&msg.method["host.".len()..], caller)
            }
            IncomingMessage::ChOpen(msg) if msg.topic.starts_with("host.") => {
                host::channel_route_is_authorized(&msg.topic["host.".len()..], caller)
            }
            _ => true,
        };
        if authorized {
            Ok(())
        } else {
            Err(LxAppError::Bridge(
                "route audience rejected browser caller".to_string(),
            ))
        }
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
        _context: &WebMessageContext,
        _route: &str,
    ) -> Result<(), LxAppError> {
        Ok(())
    }

    fn handle_res(
        &self,
        page: &PageInstance,
        _context: &WebMessageContext,
        msg: &ResMsg,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work.work_id) || !Self::work_effect_is_active(work) {
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
        _context: &WebMessageContext,
        msg: &ChDataMsg,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work.work_id) || !Self::work_effect_is_active(work) {
            return Ok(());
        }
        let Some(work_id) = work.work_id else {
            return Ok(());
        };
        if self.send_data_to_host_channel(&msg.id, work_id, msg.payload.get().to_owned()) {
            return Ok(());
        }
        self.forward_js_message(
            page,
            AppServiceCommand::ChData {
                work_id: work.work_id,
                id: msg.id.clone(),
                payload_json: msg.payload.get().to_owned(),
            },
        )
    }

    fn handle_ch_close(
        &self,
        page: &PageInstance,
        _context: &WebMessageContext,
        msg: &ChCloseMsg,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work.work_id) || !Self::work_effect_is_active(work) {
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
        _context: &WebMessageContext,
        msg: &CancelMsg,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        if let Some(work_id) = work.work_id
            && self.is_current_work(Some(work_id))
            && Self::work_effect_is_active(work)
        {
            self.inner.pending_requests.cancel(work_id, &msg.id);
        }
        Ok(())
    }

    fn handle_state_ack(
        &self,
        page: &PageInstance,
        _context: &WebMessageContext,
        msg: &StateAckMsg,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work.work_id) || !Self::work_effect_is_active(work) {
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
        _context: &WebMessageContext,
        unknown: &UnknownMsg,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work.work_id) || !Self::work_effect_is_active(work) {
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
        _context: &WebMessageContext,
        msg: &HelloMsg,
        decoded: &DecodedIncoming,
    ) -> Result<(), LxAppError> {
        if decoded.bound_v3 {
            return self.handle_bound_v3_hello(page, msg, decoded);
        }
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
        let Some(work_id) = self.replace_with_legacy_session_work(page, decoded.work.work_id)?
        else {
            return Ok(());
        };
        let work = CapturedSessionWork {
            work_id: Some(work_id),
            outbound: None,
            caller: Some(host::AuthenticatedCaller::for_lxapp(&self.inner.lxapp)),
            execution_permit: None,
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
                .expect("legacy session work always has a caller"),
        )?;
        Ok(())
    }

    fn handle_bound_v3_hello(
        &self,
        page: &PageInstance,
        msg: &HelloMsg,
        decoded: &DecodedIncoming,
    ) -> Result<(), LxAppError> {
        let work = &decoded.work;
        if !self.is_current_work(work.work_id) || !Self::work_effect_is_active(work) {
            return Ok(());
        }
        if msg.v != V3_PROTOCOL || !msg.protocols_supported.contains(&(V3_PROTOCOL as u32)) {
            return Err(LxAppError::Bridge(
                "V3 hello does not negotiate V3".to_string(),
            ));
        }
        if msg.role != "view" {
            return Err(LxAppError::Bridge("Unexpected V3 hello role".to_string()));
        }
        if let Some(expected) = page.bridge_nonce()
            && expected != msg.nonce
        {
            return Err(LxAppError::Bridge("Nonce mismatch".to_string()));
        }
        let session_id = decoded
            .session_id
            .clone()
            .ok_or_else(|| LxAppError::Bridge("missing V3 bridge binding".to_string()))?;
        let first_hello = !decoded.ready;
        if first_hello {
            let Some(work_id) = work.work_id else {
                return Ok(());
            };
            if !self.set_ready_if_current(work_id, session_id.clone()) {
                return Ok(());
            }
            // Do this once only: a retransmitted authenticated hello must not
            // cancel in-flight work or initialize the backend a second time.
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
        }
        self.send_hello_ack(
            page,
            work.work_id,
            work.outbound.as_ref(),
            msg.nonce.clone(),
            session_id.clone(),
        )?;
        self.send_ready(
            page,
            work.work_id,
            work.outbound.as_ref(),
            session_id,
            work.caller
                .as_ref()
                .expect("bound session work always has a caller"),
        )?;
        Ok(())
    }

    fn handle_req(
        &self,
        page: &PageInstance,
        context: &WebMessageContext,
        msg: &ReqMsg,
        work: &CapturedSessionWork,
        ready: bool,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work.work_id) || !Self::work_effect_is_active(work) {
            return Ok(());
        }
        if !ready {
            let _ = self.send_res_err_for_context(
                page,
                work.work_id,
                work.outbound.as_ref(),
                msg.id.clone(),
                BRIDGE_NOT_READY,
                Some("Bridge not ready".to_string()),
                None,
            );
            return Ok(());
        }

        let required_cap = required_cap_for_name(&msg.method);
        if msg.cap.is_empty() {
            let _ = self.send_res_err_for_context(
                page,
                work.work_id,
                work.outbound.as_ref(),
                msg.id.clone(),
                BRIDGE_MALFORMED_MESSAGE,
                Some("Missing cap".to_string()),
                None,
            );
            return Ok(());
        }
        if msg.cap != required_cap {
            let _ = self.send_res_err_for_context(
                page,
                work.work_id,
                work.outbound.as_ref(),
                msg.id.clone(),
                BRIDGE_MALFORMED_MESSAGE,
                Some(format!("Capability mismatch: expected '{}'", required_cap)),
                None,
            );
            return Ok(());
        }

        let params_json = msg.params.as_ref().map(|v| v.get().to_owned());
        if msg.method == "state.getSnapshot" {
            #[derive(serde::Deserialize)]
            struct SnapshotParams {
                scope: Option<String>,
            }

            let scope = params_json
                .as_deref()
                .and_then(|json| serde_json::from_str::<SnapshotParams>(json).ok())
                .and_then(|params| params.scope);
            return self.forward_js_request(
                page,
                work.work_id,
                work.outbound.as_ref(),
                msg.id.clone(),
                AppServiceCommand::StateSnapshot {
                    work_id: work.work_id,
                    outbound: work.outbound.clone(),
                    id: msg.id.clone(),
                    scope,
                },
            );
        }

        // host.* → native Rust handler (bypasses JS worker)
        if let Some(host_method) = msg.method.strip_prefix("host.") {
            return self.dispatch_host_req(
                page,
                context,
                msg.id.clone(),
                host_method.to_string(),
                params_json,
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
            .register(msg.id.clone(), work_id);
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
            msg.id.clone(),
            AppServiceCommand::Req {
                work_id: Some(work_id),
                outbound: work.outbound.clone(),
                id: msg.id.clone(),
                method: msg.method.clone(),
                params_json,
                cancel_rx,
                pending_request,
            },
        )
    }

    fn handle_notify(
        &self,
        page: &PageInstance,
        context: &WebMessageContext,
        msg: &NotifyMsg,
        work: &CapturedSessionWork,
        ready: bool,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work.work_id) || !Self::work_effect_is_active(work) || !ready {
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
        context: &WebMessageContext,
        msg: &ChOpenMsg,
        work: &CapturedSessionWork,
        ready: bool,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work.work_id) || !Self::work_effect_is_active(work) {
            return Ok(());
        }
        if !ready {
            let _ = self.send_ch_ack_err_for_context(
                page,
                work.work_id,
                work.outbound.as_ref(),
                msg.id.clone(),
                BRIDGE_NOT_READY,
                Some("Bridge not ready".to_string()),
                None,
            );
            return Ok(());
        }

        let required_cap = required_cap_for_name(&msg.topic);
        if msg.cap.is_empty() || msg.cap != required_cap {
            let _ = self.send_ch_ack_err_for_context(
                page,
                work.work_id,
                work.outbound.as_ref(),
                msg.id.clone(),
                BRIDGE_MALFORMED_MESSAGE,
                Some(format!("Capability mismatch: expected '{}'", required_cap)),
                None,
            );
            return Ok(());
        }
        if msg.topic.starts_with("host.") {
            let host_topic = &msg.topic["host.".len()..];
            return self.dispatch_host_ch_open(
                page,
                context,
                msg.id.clone(),
                host_topic,
                msg.params.as_ref().map(|v| v.get().to_owned()),
                work,
            );
        }

        self.forward_js_channel_open(
            page,
            work.work_id,
            work.outbound.as_ref(),
            msg.id.clone(),
            AppServiceCommand::ChOpen {
                work_id: work.work_id,
                outbound: work.outbound.clone(),
                id: msg.id.clone(),
                topic: msg.topic.clone(),
                params_json: msg.params.as_ref().map(|v| v.get().to_owned()),
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
        result_json: String,
    ) -> Result<(), LxAppError> {
        let result =
            RawValue::from_string(result_json).map_err(|e| LxAppError::Bridge(e.to_string()))?;
        let msg = Res {
            v: 2,
            kind: "res",
            id,
            ok: true,
            result: Some(result),
            error: None,
        };
        self.send_json_for_context(transport, work_id, outbound, V3OutboundKind::Res, &msg)
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
        let msg = ViewReqOut {
            v: 2,
            kind: "req",
            id,
            method,
            params,
            cap,
        };
        self.send_json_for_context(transport, work_id, outbound, V3OutboundKind::Req, &msg)
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
        self.send_json_for_context(transport, work_id, outbound, V3OutboundKind::Res, &msg)
    }

    pub(crate) fn send_state_snapshot_for_context<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        scope: Option<String>,
        rev: u64,
        state_json: String,
    ) -> Result<(), LxAppError> {
        let state =
            RawValue::from_string(state_json).map_err(|e| LxAppError::Bridge(e.to_string()))?;
        let msg = StateSnapshotOut {
            v: 2,
            kind: "state.snapshot",
            scope,
            rev,
            state,
        };
        self.send_json_for_context(
            transport,
            work_id,
            outbound,
            V3OutboundKind::StateSnapshot,
            &msg,
        )
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
        self.send_json_for_context(
            transport,
            work_id,
            outbound,
            V3OutboundKind::StatePatch,
            &msg,
        )
    }

    pub(crate) fn send_event_for_context<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        id: impl Into<String>,
        seq: u64,
        payload_json: String,
    ) -> Result<(), LxAppError> {
        self.send_seq_frame_with_payload_for_context(
            transport,
            work_id,
            outbound,
            V3OutboundKind::Event,
            "event",
            id.into(),
            seq,
            &payload_json,
        )
    }

    pub(crate) fn send_ch_ack_ok_for_context<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        id: impl Into<String>,
    ) -> Result<(), LxAppError> {
        let msg = ChAck {
            v: 2,
            kind: "ch.ack",
            id: id.into(),
            ok: true,
            error: None,
        };
        self.send_json_for_context(transport, work_id, outbound, V3OutboundKind::ChAck, &msg)
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
        self.send_json_for_context(transport, work_id, outbound, V3OutboundKind::ChAck, &msg)
    }

    pub(crate) fn send_ch_data_for_context<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        id: impl Into<String>,
        seq: u64,
        payload_json: String,
    ) -> Result<(), LxAppError> {
        self.send_seq_frame_with_payload_for_context(
            transport,
            work_id,
            outbound,
            V3OutboundKind::ChData,
            "ch.data",
            id.into(),
            seq,
            &payload_json,
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
        let msg = ChCloseOut {
            v: 2,
            kind: "ch.close",
            id: id.into(),
            code,
            reason,
        };
        self.send_json_for_context(transport, work_id, outbound, V3OutboundKind::ChClose, &msg)
    }

    fn send_json_for_context<T: ViewTransport, S: Serialize>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        kind: V3OutboundKind,
        msg: &S,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work_id) {
            return Ok(());
        }
        let serialized = if let Some(outbound) = outbound {
            let mut payload = serde_json::to_value(msg)?;
            let object = payload
                .as_object_mut()
                .ok_or_else(|| LxAppError::Bridge("invalid outbound bridge payload".to_string()))?;
            // V2 model structs retain their exact serialization for the
            // Legacy branch. Bound V3 owns protocol identity centrally.
            object.remove("v");
            object.remove("kind");
            object.remove("sessionId");
            serde_json::to_string(
                &encode_v3_outbound_frame(&outbound.binding, kind, payload)
                    .map_err(|_| LxAppError::Bridge("invalid V3 outbound payload".to_string()))?,
            )?
        } else {
            serde_json::to_string(msg)?
        };
        if let Some(outbound) = outbound {
            transport.post_message_to_document(
                outbound.expected_generation,
                Arc::clone(&outbound.gate),
                serialized,
            )
        } else {
            transport.post_message_to_view(serialized)
        }
    }

    fn send_seq_frame_with_payload_for_context<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        v3_kind: V3OutboundKind,
        kind: &'static str,
        id: String,
        seq: u64,
        payload_json: &str,
    ) -> Result<(), LxAppError> {
        if !self.is_current_work(work_id) {
            return Ok(());
        }
        if let Some(outbound) = outbound {
            let payload: Value = serde_json::from_str(payload_json)
                .map_err(|_| LxAppError::Bridge("invalid V3 outbound payload".to_string()))?;
            let frame = serde_json::json!({ "id": id, "seq": seq, "payload": payload });
            let frame = encode_v3_outbound_frame(&outbound.binding, v3_kind, frame)
                .map_err(|_| LxAppError::Bridge("invalid V3 outbound payload".to_string()))?;
            return transport.post_message_to_document(
                outbound.expected_generation,
                Arc::clone(&outbound.gate),
                serde_json::to_string(&frame)?,
            );
        }
        transport.post_message_to_view(serialize_seq_frame_with_payload(
            kind,
            id,
            seq,
            payload_json,
        )?)
    }

    fn send_hello_ack<T: ViewTransport>(
        &self,
        transport: &T,
        work_id: Option<SessionWorkId>,
        outbound: Option<&OutboundContext>,
        nonce: String,
        session_id: String,
    ) -> Result<(), LxAppError> {
        // The captured outbound binding, rather than mutable handshake state,
        // identifies the protocol of the document receiving this frame.
        let protocol = if outbound.is_some() { V3_PROTOCOL } else { 2 };
        let msg = HelloAck {
            v: 2,
            kind: "helloAck",
            nonce,
            protocol,
            session_id,
        };
        self.send_json_for_context(transport, work_id, outbound, V3OutboundKind::HelloAck, &msg)
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
        self.send_json_for_context(transport, work_id, outbound, V3OutboundKind::Ready, &msg)
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
            // The V3 binding belongs to the departing document. A successor
            // must be explicitly bound again; otherwise its hello could be
            // checked against revoked credentials.
            handshake.protocol = BridgeProtocol::LegacyV2;
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
    fn send_data_to_host_channel(
        &self,
        id: &str,
        work_id: SessionWorkId,
        payload_json: String,
    ) -> bool {
        let lock = self.inner.active_host_channels.lock().unwrap();
        if let Some(channel) = lock.get(&(work_id, id.to_owned())) {
            channel.sender.send_data(payload_json);
            true
        } else {
            false
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
        context: &WebMessageContext,
        id: String,
        host_topic: &str,
        params_json: Option<String>,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        admit_before_host_dispatch(
            context,
            |context| self.admit_host_route(page, context, host_topic),
            || {
                if !self.is_current_work(work.work_id) || !Self::work_effect_is_active(work) {
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
                if !self.is_current_work(Some(work_id)) || !Self::work_effect_is_active(work) {
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
                                    payload_json,
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
                if !self.is_current_work(Some(work_id)) || !Self::work_effect_is_active(work) {
                    if let Some(channel) = self.take_host_channel(&id, work_id, channel_token) {
                        channel.sender.send_close(
                            Some(BRIDGE_CANCELED.to_string()),
                            Some(PAGE_UNLOADED.to_string()),
                        );
                    }
                    return Ok(());
                }
                let invocation = host::HostInvocationContext::for_dispatch(self.lxapp(), caller)
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
        context: &WebMessageContext,
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
                if !self.is_current_work(work.work_id) || !Self::work_effect_is_active(work) {
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

                let invocation = host::HostInvocationContext::for_dispatch(self.lxapp(), caller)
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
                if !self.is_current_work(Some(work_id)) || !Self::work_effect_is_active(work) {
                    drop(pending_request);
                    return Ok(());
                }
                let task_id = id.clone();
                let task_host_method = host_method.clone();
                let task_outbound = work.outbound.clone();
                let task_work = work.clone();
                let task_permit = work.execution_permit.clone();

                crate::executor::spawn(async move {
                    HOST_EFFECT_WORK
                        .scope(task_work, async move {
                    let started_at = std::time::Instant::now();
                    if !task_permit
                        .as_ref()
                        .is_none_or(crate::RequiredV3ExecutionPermit::is_active)
                    {
                        drop(pending_request);
                        return;
                    }
                    let (tx, rx) = oneshot::channel();
                    let mut host_cancel_tx = Some(tx);
                    let mut host_fut = handler.call(invocation, params_json, rx);
                    let permit_cancel =
                        wait_for_execution_permit_cancellation(task_permit.clone());

                    let initial_result: Result<HostOutput, RpcError> = tokio::select! {
                        biased;
                        _ = permit_cancel => {
                            if let Some(tx) = host_cancel_tx.take() {
                                let _ = tx.send(());
                            }
                            Err(RpcError::new(BRIDGE_CANCELED, Some(PAGE_UNLOADED.to_string())))
                        }
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
                            json,
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
                                    task_permit,
                                )
                                .await
                            {
                                Ok(json) => bridge.send_res_ok_for_context(
                                    &task_page,
                                    Some(work_id),
                                    task_outbound.as_ref(),
                                    task_id.clone(),
                                    json,
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
        context: &WebMessageContext,
        host_method: String,
        params_json: Option<String>,
        work: &CapturedSessionWork,
    ) -> Result<(), LxAppError> {
        let route = host_method.clone();
        admit_before_host_dispatch(
            context,
            |context| self.admit_host_route(page, context, &route),
            || {
                if !self.is_current_work(work.work_id) || !Self::work_effect_is_active(work) {
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

                let invocation = host::HostInvocationContext::for_dispatch(self.lxapp(), caller)
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
                if !self.is_current_work(Some(work_id)) || !Self::work_effect_is_active(work) {
                    drop(notify_guard);
                    return Ok(());
                }
                let task_work = work.clone();
                let task_permit = work.execution_permit.clone();
                crate::executor::spawn(async move {
                    HOST_EFFECT_WORK
                        .scope(task_work, async move {
                            let _notify_guard = notify_guard;
                            if !task_permit
                                .as_ref()
                                .is_none_or(crate::RequiredV3ExecutionPermit::is_active)
                            {
                                return;
                            }
                            let mut host_fut = handler.call(invocation, params_json, cancel_rx);
                            let permit_cancel = wait_for_execution_permit_cancellation(task_permit);
                            match tokio::select! {
                                biased;
                                _ = permit_cancel => Err(LxAppError::Bridge(PAGE_UNLOADED.to_string())),
                                output = &mut host_fut => output,
                            } {
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
        execution_permit: Option<crate::RequiredV3ExecutionPermit>,
    ) -> Result<String, RpcError> {
        let mut seq = 0u64;

        loop {
            let next_item = tokio::select! {
                biased;
                _ = wait_for_execution_permit_cancellation(execution_permit.clone()) => {
                    if let Some(tx) = host_cancel_tx.take() {
                        let _ = tx.send(());
                    }
                    return Err(RpcError::new(BRIDGE_CANCELED, Some(PAGE_UNLOADED.to_string())));
                }
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
                        payload_json,
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

    #[derive(Debug, PartialEq, Eq)]
    struct TestContext {
        native_view: u64,
        frame: &'static str,
    }

    #[test]
    fn host_admission_seam_requires_web_message_context() {
        let _seam: fn(
            &PageBridge,
            &PageInstance,
            &WebMessageContext,
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
    fn stale_legacy_hello_work_does_not_match_v3_successor() {
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
    fn legacy_v2_view_request_wire_remains_byte_stable() {
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
    fn legacy_v2_hello_ack_wire_remains_byte_stable() {
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
