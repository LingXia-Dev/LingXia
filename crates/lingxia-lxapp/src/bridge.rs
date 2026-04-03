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

pub(crate) use protocol::{ChOpenMsg, HelloMsg, IncomingMessage, JsonPatchOp, NotifyMsg, ReqMsg};

use protocol::*;

use crate::LxAppError;
use crate::host::{self, HostOutput, HostStream, HostStreamItem};
use crate::lxapp::LxApp;
use crate::page::Page;
use base64::Engine;
use futures::StreamExt;
use serde::Serialize;
use serde_json::Value;
use serde_json::value::RawValue;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

// AppServiceCommand — the bridge-level message routed to the JS runtime backend
pub(crate) enum AppServiceCommand {
    Ready,
    StateSnapshot {
        id: String,
        scope: Option<String>,
    },
    Req {
        id: String,
        method: String,
        params_json: Option<String>,
        cancel_rx: oneshot::Receiver<()>,
    },
    Notify {
        method: String,
        params_json: Option<String>,
    },
    ChOpen {
        id: String,
        topic: String,
        params_json: Option<String>,
    },
    ChData {
        id: String,
        payload_json: String,
    },
    ChClose {
        id: String,
        code: Option<String>,
        reason: Option<String>,
    },
    StateAck {
        scope: Option<String>,
        rev: u64,
    },
}

// AppServiceBackend — trait to decouple bridge routing from the JS runtime executor
pub(crate) trait AppServiceBackend: Send + Sync {
    fn forward(
        &self,
        lxapp: Arc<LxApp>,
        path: String,
        message: AppServiceCommand,
    ) -> Result<(), LxAppError>;
}

// Error codes (must match lingxia-bridge/src/types.ts)
pub(crate) const BRIDGE_NOT_READY: &str = "BRIDGE_NOT_READY";
pub(crate) const BRIDGE_TIMEOUT: &str = "BRIDGE_TIMEOUT";
pub(crate) const BRIDGE_CANCELED: &str = "BRIDGE_CANCELED";
pub(crate) const BRIDGE_PROTOCOL_MISMATCH: &str = "BRIDGE_PROTOCOL_MISMATCH";
pub(crate) const BRIDGE_MALFORMED_MESSAGE: &str = "BRIDGE_MALFORMED_MESSAGE";
pub(crate) const BRIDGE_METHOD_NOT_FOUND: &str = "BRIDGE_METHOD_NOT_FOUND";
pub(crate) const BRIDGE_TOPIC_NOT_FOUND: &str = "BRIDGE_TOPIC_NOT_FOUND";
pub(crate) const BRIDGE_INTERNAL_ERROR: &str = "BRIDGE_INTERNAL_ERROR";
pub(crate) const BRIDGE_STREAM_CLOSED: &str = "BRIDGE_STREAM_CLOSED";

// ViewTransport — posting messages back to the WebView
pub(crate) trait ViewTransport {
    fn post_message_to_view(&self, message_json: String) -> Result<(), LxAppError>;
}

impl ViewTransport for Page {
    fn post_message_to_view(&self, message_json: String) -> Result<(), LxAppError> {
        if let Some(controller) = self.webview_controller() {
            controller
                .post_message(&message_json)
                .map_err(|e| LxAppError::WebView(e.to_string()))
        } else {
            Err(LxAppError::WebView("WebView not ready".to_string()))
        }
    }
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
#[derive(Debug, Default)]
struct HandshakeState {
    session_id: Option<String>,
    ready: bool,
}

struct PageBridgeState {
    lxapp: Arc<LxApp>,
    js_backend: Arc<dyn AppServiceBackend>,
    msg_counter: AtomicUsize,
    handshake: Mutex<HandshakeState>,
    pending_req_cancel: Mutex<HashMap<String, oneshot::Sender<()>>>,
    active_host_channels: Mutex<HashMap<String, host::ChannelContextSender>>,
}

#[derive(Clone)]
pub(crate) struct PageBridge {
    inner: Arc<PageBridgeState>,
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
                handshake: Mutex::new(HandshakeState::default()),
                pending_req_cancel: Mutex::new(HashMap::new()),
                active_host_channels: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.inner.handshake.lock().unwrap().ready
    }

    fn lxapp(&self) -> Arc<LxApp> {
        self.inner.lxapp.clone()
    }

    pub(crate) fn handle_incoming(
        &self,
        page: &Page,
        message: Arc<IncomingMessage>,
    ) -> Result<(), LxAppError> {
        match &*message {
            IncomingMessage::Hello(msg) => self.handle_hello(page, msg),
            IncomingMessage::Req(msg) => self.handle_req(page, msg),
            IncomingMessage::Res(msg) => {
                if msg.v != 2 {
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
                crate::appservice::view_call::resolve_view_call(
                    &msg.id,
                    Some(&page.path()),
                    result,
                );
                Ok(())
            }
            IncomingMessage::Notify(msg) => self.handle_notify(page, msg),
            IncomingMessage::ChOpen(msg) => self.handle_ch_open(page, msg),
            IncomingMessage::ChData(msg) => {
                if msg.v != 2 {
                    return Ok(());
                }
                if self.send_data_to_host_channel(&msg.id, msg.payload.get().to_owned()) {
                    return Ok(());
                }
                self.forward_js_message(
                    page,
                    AppServiceCommand::ChData {
                        id: msg.id.clone(),
                        payload_json: msg.payload.get().to_owned(),
                    },
                )
            }
            IncomingMessage::ChClose(msg) => {
                if msg.v != 2 {
                    return Ok(());
                }
                if self.close_host_channel_from_view(&msg.id, msg.code.clone(), msg.reason.clone())
                {
                    return Ok(());
                }
                self.forward_js_message(
                    page,
                    AppServiceCommand::ChClose {
                        id: msg.id.clone(),
                        code: msg.code.clone(),
                        reason: msg.reason.clone(),
                    },
                )
            }
            IncomingMessage::Cancel(msg) => {
                if msg.v != 2 {
                    return Ok(());
                }
                if let Some(tx) = self.take_pending_req_cancel(&msg.id) {
                    let _ = tx.send(());
                }
                Ok(())
            }
            IncomingMessage::StateAck(msg) => {
                if msg.v != 2 {
                    return Ok(());
                }
                self.forward_js_message(
                    page,
                    AppServiceCommand::StateAck {
                        scope: msg.scope.clone(),
                        rev: msg.rev,
                    },
                )
            }
            IncomingMessage::Unknown(unknown) => {
                if let Some(id) = &unknown.id {
                    let (code, message) = if unknown.v != Some(2) {
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
                                .map(|kind| format!("Unknown kind: {}", kind))
                                .or_else(|| unknown.parse_error.clone())
                                .or_else(|| Some("Unknown message".to_string())),
                        )
                    };
                    let _ = self.send_res_err(page, id.clone(), code, message, None);
                }
                Ok(())
            }
        }
    }

    fn handle_hello(&self, page: &Page, msg: &HelloMsg) -> Result<(), LxAppError> {
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

        self.reset_session();

        let session_id = self.new_session_id();
        self.send_hello_ack(page, msg.nonce.clone(), session_id.clone())?;
        self.set_ready(session_id.clone());
        self.send_ready(page, session_id.clone())?;
        if let Err(err) = self.forward_js_message(page, AppServiceCommand::Ready) {
            crate::warn!("bridge ready bootstrap failed: {}", err)
                .with_appid(page.appid())
                .with_path(page.path());
        }
        Ok(())
    }

    fn handle_req(&self, page: &Page, msg: &ReqMsg) -> Result<(), LxAppError> {
        if msg.v != 2 {
            let _ = self.send_res_err(
                page,
                msg.id.clone(),
                BRIDGE_PROTOCOL_MISMATCH,
                Some(format!("Unsupported protocol: {}", msg.v)),
                None,
            );
            return Ok(());
        }
        if !self.is_ready() {
            let _ = self.send_res_err(
                page,
                msg.id.clone(),
                BRIDGE_NOT_READY,
                Some("Bridge not ready".to_string()),
                None,
            );
            return Ok(());
        }

        let required_cap = required_cap_for_name(&msg.method);
        if msg.cap.is_empty() {
            let _ = self.send_res_err(
                page,
                msg.id.clone(),
                BRIDGE_MALFORMED_MESSAGE,
                Some("Missing cap".to_string()),
                None,
            );
            return Ok(());
        }
        if msg.cap != required_cap {
            let _ = self.send_res_err(
                page,
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
                msg.id.clone(),
                AppServiceCommand::StateSnapshot {
                    id: msg.id.clone(),
                    scope,
                },
            );
        }

        // host.* → native Rust handler (bypasses JS worker)
        if let Some(host_method) = msg.method.strip_prefix("host.") {
            return self.dispatch_host_req(
                page,
                msg.id.clone(),
                host_method.to_string(),
                params_json,
            );
        }

        // everything else → JS runtime
        let (cancel_tx, cancel_rx) = oneshot::channel();
        self.register_pending_req_cancel(msg.id.clone(), cancel_tx);
        self.forward_js_request(
            page,
            msg.id.clone(),
            AppServiceCommand::Req {
                id: msg.id.clone(),
                method: msg.method.clone(),
                params_json,
                cancel_rx,
            },
        )
    }

    fn handle_notify(&self, page: &Page, msg: &NotifyMsg) -> Result<(), LxAppError> {
        if msg.v != 2 || !self.is_ready() {
            return Ok(());
        }

        let required_cap = required_cap_for_name(&msg.method);
        if msg.cap.is_empty() || msg.cap != required_cap {
            return Ok(());
        }

        let params_json = msg.params.as_ref().map(|v| v.get().to_owned());
        if let Some(host_method) = msg.method.strip_prefix("host.") {
            return self.dispatch_host_notify(page, host_method.to_string(), params_json);
        }

        self.forward_js_message(
            page,
            AppServiceCommand::Notify {
                method: msg.method.clone(),
                params_json,
            },
        )
    }

    fn handle_ch_open(&self, page: &Page, msg: &ChOpenMsg) -> Result<(), LxAppError> {
        if msg.v != 2 {
            let _ = self.send_ch_ack_err(
                page,
                msg.id.clone(),
                BRIDGE_PROTOCOL_MISMATCH,
                Some(format!("Unsupported protocol: {}", msg.v)),
                None,
            );
            return Ok(());
        }
        if !self.is_ready() {
            let _ = self.send_ch_ack_err(
                page,
                msg.id.clone(),
                BRIDGE_NOT_READY,
                Some("Bridge not ready".to_string()),
                None,
            );
            return Ok(());
        }

        let required_cap = required_cap_for_name(&msg.topic);
        if msg.cap.is_empty() || msg.cap != required_cap {
            let _ = self.send_ch_ack_err(
                page,
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
                msg.id.clone(),
                host_topic,
                msg.params.as_ref().map(|v| v.get().to_owned()),
            );
        }

        self.forward_js_channel_open(
            page,
            msg.id.clone(),
            AppServiceCommand::ChOpen {
                id: msg.id.clone(),
                topic: msg.topic.clone(),
                params_json: msg.params.as_ref().map(|v| v.get().to_owned()),
            },
        )
    }

    fn forward_js_message(
        &self,
        page: &Page,
        message: AppServiceCommand,
    ) -> Result<(), LxAppError> {
        self.inner
            .js_backend
            .forward(self.inner.lxapp.clone(), page.path(), message)
    }

    fn forward_js_request(
        &self,
        page: &Page,
        id: String,
        message: AppServiceCommand,
    ) -> Result<(), LxAppError> {
        if let Err(err) = self.forward_js_message(page, message) {
            self.take_pending_req_cancel(&id);
            let _ = self.send_res_err(page, id, BRIDGE_INTERNAL_ERROR, Some(err.to_string()), None);
        }
        Ok(())
    }

    fn forward_js_channel_open(
        &self,
        page: &Page,
        id: String,
        message: AppServiceCommand,
    ) -> Result<(), LxAppError> {
        if let Err(err) = self.forward_js_message(page, message) {
            let _ =
                self.send_ch_ack_err(page, id, BRIDGE_INTERNAL_ERROR, Some(err.to_string()), None);
        }
        Ok(())
    }

    pub(crate) fn send_res_ok<T: ViewTransport>(
        &self,
        transport: &T,
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
        self.send_json(transport, &msg)
    }

    pub(crate) fn send_res_err<T: ViewTransport>(
        &self,
        transport: &T,
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
        self.send_json(transport, &msg)
    }

    pub(crate) fn send_state_snapshot<T: ViewTransport>(
        &self,
        transport: &T,
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
        self.send_json(transport, &msg)
    }

    pub(crate) fn send_state_patch<T: ViewTransport>(
        &self,
        transport: &T,
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
        self.send_json(transport, &msg)
    }

    pub(crate) fn send_event<T: ViewTransport>(
        &self,
        transport: &T,
        id: impl Into<String>,
        seq: u64,
        payload_json: String,
    ) -> Result<(), LxAppError> {
        let payload =
            RawValue::from_string(payload_json).map_err(|e| LxAppError::Bridge(e.to_string()))?;
        let msg = EventMsg {
            v: 2,
            kind: "event",
            id: id.into(),
            seq,
            payload,
        };
        self.send_json(transport, &msg)
    }

    pub(crate) fn send_ch_ack_ok<T: ViewTransport>(
        &self,
        transport: &T,
        id: impl Into<String>,
    ) -> Result<(), LxAppError> {
        let msg = ChAck {
            v: 2,
            kind: "ch.ack",
            id: id.into(),
            ok: true,
            error: None,
        };
        self.send_json(transport, &msg)
    }

    pub(crate) fn send_ch_ack_err<T: ViewTransport>(
        &self,
        transport: &T,
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
        self.send_json(transport, &msg)
    }

    pub(crate) fn send_ch_data<T: ViewTransport>(
        &self,
        transport: &T,
        id: impl Into<String>,
        seq: u64,
        payload_json: String,
    ) -> Result<(), LxAppError> {
        let payload =
            RawValue::from_string(payload_json).map_err(|e| LxAppError::Bridge(e.to_string()))?;
        let msg = ChDataOut {
            v: 2,
            kind: "ch.data",
            id: id.into(),
            seq,
            payload,
        };
        self.send_json(transport, &msg)
    }

    pub(crate) fn send_ch_close<T: ViewTransport>(
        &self,
        transport: &T,
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
        self.send_json(transport, &msg)
    }

    fn send_json<T: ViewTransport, S: Serialize>(
        &self,
        transport: &T,
        msg: &S,
    ) -> Result<(), LxAppError> {
        let serialized = serde_json::to_string(msg)?;
        transport.post_message_to_view(serialized)
    }

    fn send_hello_ack<T: ViewTransport>(
        &self,
        transport: &T,
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
        self.send_json(transport, &msg)
    }

    fn send_ready<T: ViewTransport>(
        &self,
        transport: &T,
        session_id: String,
    ) -> Result<(), LxAppError> {
        let msg = ReadyMsg {
            v: 2,
            kind: "ready",
            session_id,
            host_methods: host::host_method_schema(),
        };
        self.send_json(transport, &msg)
    }

    fn set_ready(&self, session_id: String) {
        let mut hs = self.inner.handshake.lock().unwrap();
        hs.session_id = Some(session_id);
        hs.ready = true;
    }

    fn reset_session(&self) {
        {
            let mut hs = self.inner.handshake.lock().unwrap();
            hs.session_id = None;
            hs.ready = false;
        }

        let pending_req = {
            let mut pending = self.inner.pending_req_cancel.lock().unwrap();
            std::mem::take(&mut *pending)
        };
        for (_, cancel_tx) in pending_req {
            let _ = cancel_tx.send(());
        }

        // Drop all host channel senders; their ChannelContext receivers will get None,
        // signalling the handler that the session ended.
        let active_host_channels = {
            let mut channels = self.inner.active_host_channels.lock().unwrap();
            std::mem::take(&mut *channels)
        };
        for (_, sender) in active_host_channels {
            sender.send_close(
                Some(BRIDGE_CANCELED.to_string()),
                Some("Session reset".to_string()),
            );
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

    pub(crate) fn register_pending_req_cancel(
        &self,
        id: impl Into<String>,
        cancel_tx: oneshot::Sender<()>,
    ) {
        self.inner
            .pending_req_cancel
            .lock()
            .unwrap()
            .insert(id.into(), cancel_tx);
    }

    pub(crate) fn take_pending_req_cancel(&self, id: &str) -> Option<oneshot::Sender<()>> {
        self.inner.pending_req_cancel.lock().unwrap().remove(id)
    }

    fn register_host_channel(&self, id: impl Into<String>, sender: host::ChannelContextSender) {
        self.inner
            .active_host_channels
            .lock()
            .unwrap()
            .insert(id.into(), sender);
    }

    fn take_host_channel(&self, id: &str) -> Option<host::ChannelContextSender> {
        self.inner.active_host_channels.lock().unwrap().remove(id)
    }

    /// Forward inbound `ch.data` payload to the matching host channel sender.
    /// Returns `true` if the channel was found (message consumed), `false` otherwise.
    fn send_data_to_host_channel(&self, id: &str, payload_json: String) -> bool {
        let lock = self.inner.active_host_channels.lock().unwrap();
        if let Some(sender) = lock.get(id) {
            sender.send_data(payload_json);
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
        code: Option<String>,
        reason: Option<String>,
    ) -> bool {
        let sender = self.inner.active_host_channels.lock().unwrap().remove(id);
        if let Some(sender) = sender {
            sender.send_close(code, reason);
            true
        } else {
            false
        }
    }

    fn dispatch_host_ch_open(
        &self,
        page: &Page,
        id: String,
        host_topic: &str,
        params_json: Option<String>,
    ) -> Result<(), LxAppError> {
        let Some(handler) = host::get_channel_handler(host_topic) else {
            let _ = self.send_ch_ack_err(
                page,
                id,
                BRIDGE_TOPIC_NOT_FOUND,
                Some(format!("Channel not found: host.{}", host_topic)),
                None,
            );
            return Ok(());
        };

        let (ctx, sender, mut outbound_rx) = host::new_channel_context(id.clone());
        self.register_host_channel(id.clone(), sender);

        // Acknowledge the channel open before invoking the handler.
        self.send_ch_ack_ok(page, id.clone())?;

        // Spawn an outbound forwarding task that relays ChannelOutbound messages
        // from the handler back to the View as ch.data / ch.close wire messages.
        let bridge = self.clone();
        let task_page = page.clone();
        let task_id = id.clone();
        crate::executor::spawn(async move {
            let mut seq = 0u64;
            while let Some(msg) = outbound_rx.recv().await {
                match msg {
                    host::ChannelOutbound::Data(payload_json) => {
                        if let Err(e) =
                            bridge.send_ch_data(&task_page, task_id.clone(), seq, payload_json)
                        {
                            crate::warn!("host channel '{}' data send failed: {}", task_id, e)
                                .with_appid(task_page.appid())
                                .with_path(task_page.path());
                        }
                        seq += 1;
                    }
                    host::ChannelOutbound::Close { code, reason } => {
                        bridge.take_host_channel(&task_id);
                        let _ = bridge.send_ch_close(&task_page, task_id.clone(), code, reason);
                        break;
                    }
                }
            }
        });

        // Call handler.on_open synchronously; the handler is expected to spawn
        // its own async task if it needs to do long-running work.
        let lxapp = self.lxapp();
        handler.on_open(lxapp, ctx, params_json);

        Ok(())
    }

    fn dispatch_host_req(
        &self,
        page: &Page,
        id: String,
        host_method: String,
        params_json: Option<String>,
    ) -> Result<(), LxAppError> {
        let Some(handler) = host::get_host(&host_method) else {
            let _ = self.send_res_err(
                page,
                id,
                BRIDGE_METHOD_NOT_FOUND,
                Some(format!("Method not found: host.{}", host_method)),
                None,
            );
            return Ok(());
        };

        let lxapp = self.lxapp();
        let page = page.clone();
        let task_page = page.clone();
        let bridge = self.clone();
        let (cancel_tx, mut cancel_rx) = oneshot::channel();
        self.register_pending_req_cancel(id.clone(), cancel_tx);
        let task_id = id.clone();
        let task_host_method = host_method.clone();

        crate::executor::spawn(async move {
            let started_at = std::time::Instant::now();
            let (tx, rx) = oneshot::channel();
            let mut host_cancel_tx = Some(tx);
            let mut host_fut = handler.call(lxapp, params_json, rx);

            let initial_result: Result<HostOutput, RpcError> = tokio::select! {
                biased;
                _ = &mut cancel_rx => {
                    if let Some(tx) = host_cancel_tx.take() {
                        let _ = tx.send(());
                    }
                    Err(RpcError::new(BRIDGE_CANCELED, None))
                }
                res = &mut host_fut => {
                    match res {
                        Ok(output) => Ok(output),
                        Err(err) => Err(rpc_error_from_lxapp_error(&err)),
                    }
                }
            };

            let send_result = match initial_result {
                Ok(HostOutput::Json(json)) => bridge.send_res_ok(&task_page, task_id.clone(), json),
                Ok(HostOutput::Stream(stream)) => {
                    match bridge
                        .consume_host_stream(
                            &task_page,
                            &task_id,
                            stream,
                            &mut cancel_rx,
                            host_cancel_tx,
                        )
                        .await
                    {
                        Ok(json) => bridge.send_res_ok(&task_page, task_id.clone(), json),
                        Err(err) => bridge.send_res_err(
                            &task_page,
                            task_id.clone(),
                            &err.code,
                            err.message,
                            err.data,
                        ),
                    }
                }
                Err(err) => bridge.send_res_err(
                    &task_page,
                    task_id.clone(),
                    &err.code,
                    err.message,
                    err.data,
                ),
            };

            bridge.take_pending_req_cancel(&task_id);

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
        });

        Ok(())
    }

    fn dispatch_host_notify(
        &self,
        page: &Page,
        host_method: String,
        params_json: Option<String>,
    ) -> Result<(), LxAppError> {
        let Some(handler) = host::get_host(&host_method) else {
            return Ok(());
        };

        let lxapp = self.lxapp();
        let appid = page.appid();
        let path = page.path();
        let task_host_method = host_method.clone();
        crate::executor::spawn(async move {
            let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
            let _keep_alive = cancel_tx;
            match handler.call(lxapp, params_json, cancel_rx).await {
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
                    crate::warn!("host notify '{}' failed: {}", task_host_method, err)
                        .with_appid(appid)
                        .with_path(path);
                }
            }
        });
        Ok(())
    }

    async fn consume_host_stream(
        &self,
        page: &Page,
        stream_id: &str,
        mut stream: HostStream,
        cancel_rx: &mut oneshot::Receiver<()>,
        mut host_cancel_tx: Option<oneshot::Sender<()>>,
    ) -> Result<String, RpcError> {
        let mut seq = 0u64;

        loop {
            let next_item = tokio::select! {
                _ = &mut *cancel_rx => {
                    if let Some(tx) = host_cancel_tx.take() {
                        let _ = tx.send(());
                    }
                    return Err(RpcError::new(BRIDGE_CANCELED, None));
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
                    self.send_event(page, stream_id.to_string(), seq, payload_json)
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
        return RpcError::new(BRIDGE_CANCELED, None);
    }
    RpcError::new(BRIDGE_INTERNAL_ERROR, Some(err.to_string()))
}
