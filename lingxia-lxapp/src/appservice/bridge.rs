use crate::error::LxAppError;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex, atomic::AtomicUsize};
use tokio::sync::oneshot;

// Error codes (must match lingxia-web-runtime/src/types.ts)
pub const BRIDGE_NOT_READY: &str = "BRIDGE_NOT_READY";
#[allow(dead_code)]
pub const BRIDGE_TIMEOUT: &str = "BRIDGE_TIMEOUT";
pub const BRIDGE_CANCELED: &str = "BRIDGE_CANCELED";
#[allow(dead_code)]
pub const BRIDGE_PROTOCOL_MISMATCH: &str = "BRIDGE_PROTOCOL_MISMATCH";
#[allow(dead_code)]
pub const BRIDGE_HANDSHAKE_FAILED: &str = "BRIDGE_HANDSHAKE_FAILED";
#[allow(dead_code)]
pub const BRIDGE_MALFORMED_MESSAGE: &str = "BRIDGE_MALFORMED_MESSAGE";
pub const BRIDGE_METHOD_NOT_FOUND: &str = "BRIDGE_METHOD_NOT_FOUND";
pub const BRIDGE_CAPABILITY_DENIED: &str = "BRIDGE_CAPABILITY_DENIED";
pub const BRIDGE_INTERNAL_ERROR: &str = "BRIDGE_INTERNAL_ERROR";

#[derive(Clone)]
pub(crate) enum ServiceType {
    None,
    JSFunc(rong::JSFunc),
    HostAPI(Arc<dyn crate::host::HostHandler>),
}

pub(crate) trait MessageTransport {
    fn post_message_to_view(&self, message_json: String) -> Result<(), LxAppError>;
}

pub(crate) trait MessageHandler {
    fn get_service_type(&self, service_name: &str) -> ServiceType;
    async fn get_state_snapshot(&self, scope: Option<&str>) -> Result<Value, LxAppError>;
    async fn handle_req(
        &self,
        method: &str,
        params_json: Option<&str>,
        service_type: ServiceType,
        cancel_rx: oneshot::Receiver<()>,
    ) -> Result<Value, RpcError>;
    async fn handle_notify(
        &self,
        method: &str,
        params_json: Option<&str>,
        service_type: ServiceType,
    );
    async fn handle_bridge_ready(&self);
    fn expected_bridge_nonce(&self) -> Option<String> {
        None
    }
    fn bridge_page_path(&self) -> Option<String> {
        None
    }
    fn is_cap_allowed(&self, cap: &str) -> bool;
    async fn handle_state_ack(&self, scope: Option<String>, rev: u64);
}

#[derive(Clone)]
pub(crate) struct Bridge {
    msg_counter: Rc<AtomicUsize>,
    handshake: Rc<Mutex<HandshakeState>>,
    pending_req_cancel: Rc<Mutex<HashMap<String, oneshot::Sender<()>>>>,
}

#[derive(Debug, Default, Clone)]
struct HandshakeState {
    session_id: Option<String>,
    ready: bool,
}

#[derive(Debug, Clone)]
pub struct RpcError {
    pub code: String,
    pub message: Option<String>,
}

// V2 Incoming message types
#[derive(Deserialize, Debug, Clone)]
pub(crate) struct HelloMsg {
    pub v: u8,
    pub nonce: String,
    pub role: String,
    #[serde(default, rename = "protocolsSupported")]
    pub protocols_supported: Vec<u32>,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct ReqMsg {
    pub v: u8,
    pub id: String,
    pub method: String,
    pub params: Option<Value>,
    #[serde(default)]
    pub cap: String,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct NotifyMsg {
    pub v: u8,
    pub method: String,
    pub params: Option<Value>,
    #[serde(default)]
    pub cap: String,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct CancelMsg {
    pub v: u8,
    pub id: String,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct StateAckMsg {
    pub v: u8,
    pub scope: Option<String>,
    pub rev: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct ResMsg {
    #[allow(dead_code)]
    pub v: u8,
    pub id: String,
    #[serde(default)]
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<ResError>,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct ResError {
    pub code: String,
    pub message: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct UnknownMsg {
    pub v: Option<u8>,
    pub kind: Option<String>,
    pub id: Option<String>,
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum IncomingMessage {
    Hello(HelloMsg),
    Req(ReqMsg),
    Res(ResMsg),
    Notify(NotifyMsg),
    Cancel(CancelMsg),
    StateAck(StateAckMsg),
    Unknown(UnknownMsg),
}

impl IncomingMessage {
    pub fn from_json_str(json_str: &str) -> Result<Self, LxAppError> {
        let raw: Value = serde_json::from_str(json_str)?;
        let obj = raw
            .as_object()
            .ok_or_else(|| LxAppError::Bridge("Message must be JSON object".to_string()))?;

        let v = obj
            .get("v")
            .and_then(|v| v.as_u64())
            .and_then(|v| u8::try_from(v).ok());
        let kind = obj
            .get("kind")
            .and_then(|k| k.as_str())
            .map(|s| s.to_string());
        let id = obj
            .get("id")
            .and_then(|i| i.as_str())
            .map(|s| s.to_string());

        let Some(kind_str) = kind.as_deref() else {
            return Ok(IncomingMessage::Unknown(UnknownMsg {
                v,
                kind,
                id,
                parse_error: Some("Missing 'kind'".to_string()),
            }));
        };

        match kind_str {
            "hello" => serde_json::from_value::<HelloMsg>(raw.clone()).map(IncomingMessage::Hello),
            "req" => serde_json::from_value::<ReqMsg>(raw.clone()).map(IncomingMessage::Req),
            "res" => serde_json::from_value::<ResMsg>(raw.clone()).map(IncomingMessage::Res),
            "notify" => {
                serde_json::from_value::<NotifyMsg>(raw.clone()).map(IncomingMessage::Notify)
            }
            "cancel" => {
                serde_json::from_value::<CancelMsg>(raw.clone()).map(IncomingMessage::Cancel)
            }
            "state.ack" => {
                serde_json::from_value::<StateAckMsg>(raw.clone()).map(IncomingMessage::StateAck)
            }
            _ => {
                return Ok(IncomingMessage::Unknown(UnknownMsg {
                    v,
                    kind,
                    id,
                    parse_error: None,
                }));
            }
        }
        .or_else(|e| {
            Ok(IncomingMessage::Unknown(UnknownMsg {
                v,
                kind,
                id,
                parse_error: Some(e.to_string()),
            }))
        })
    }
}

// V2 Outgoing message types
#[derive(Serialize)]
struct HelloAck {
    v: u8,
    kind: &'static str,
    nonce: String,
    protocol: u8,
    #[serde(rename = "sessionId")]
    session_id: String,
}

#[derive(Serialize)]
struct Ready {
    v: u8,
    kind: &'static str,
    #[serde(rename = "sessionId")]
    session_id: String,
}

#[derive(Serialize)]
struct BridgeError {
    code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retryable: Option<bool>,
}

#[derive(Serialize)]
struct Res {
    v: u8,
    kind: &'static str,
    id: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<BridgeError>,
}

#[derive(Serialize)]
pub struct StateSnapshot {
    v: u8,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
    rev: u64,
    state: Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JsonPatchOp {
    pub op: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

#[derive(Serialize)]
pub struct StatePatch {
    v: u8,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
    #[serde(rename = "baseRev")]
    base_rev: u64,
    rev: u64,
    ops: Vec<JsonPatchOp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ack: Option<bool>,
}

impl Bridge {
    pub(crate) fn new() -> Self {
        Self {
            msg_counter: Rc::new(AtomicUsize::new(0)),
            handshake: Rc::new(Mutex::new(HandshakeState::default())),
            pending_req_cancel: Rc::new(Mutex::new(HashMap::new())),
        }
    }

    // NOTE: Caller-provided `cap` MUST NOT be trusted for security. We infer the required
    // capability from `method` and only use `cap` for consistency validation.
    pub(crate) fn required_cap_for_method(method: &str) -> String {
        if method.starts_with("host.") {
            return "host".to_string();
        }
        if let Some((prefix, _)) = method.split_once('.') {
            return prefix.to_string();
        }
        "page".to_string()
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.handshake.lock().unwrap().ready
    }

    fn set_ready(&self, session_id: String) {
        let mut hs = self.handshake.lock().unwrap();
        hs.session_id = Some(session_id);
        hs.ready = true;
    }

    fn new_session_id(&self) -> String {
        let count = self
            .msg_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let data = format!("{}-{}", ts, count);
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data.as_bytes())
    }

    fn send_json<T: MessageTransport, S: Serialize>(
        &self,
        transport: &T,
        msg: &S,
    ) -> Result<(), LxAppError> {
        let serialized = serde_json::to_string(msg)?;
        transport.post_message_to_view(serialized)?;
        Ok(())
    }

    fn send_res_ok<T: MessageTransport>(
        &self,
        transport: &T,
        id: String,
        result: Value,
    ) -> Result<(), LxAppError> {
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

    fn send_res_err<T: MessageTransport>(
        &self,
        transport: &T,
        id: String,
        code: &str,
        message: Option<String>,
    ) -> Result<(), LxAppError> {
        let msg = Res {
            v: 2,
            kind: "res",
            id,
            ok: false,
            result: None,
            error: Some(BridgeError {
                code: code.to_string(),
                message,
                data: None,
                retryable: None,
            }),
        };
        self.send_json(transport, &msg)
    }

    async fn send_hello_ack<T: MessageTransport>(
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

    async fn send_ready<T: MessageTransport>(
        &self,
        transport: &T,
        session_id: String,
    ) -> Result<(), LxAppError> {
        let msg = Ready {
            v: 2,
            kind: "ready",
            session_id,
        };
        self.send_json(transport, &msg)
    }

    pub fn send_state_snapshot<T: MessageTransport>(
        &self,
        transport: &T,
        scope: Option<String>,
        rev: u64,
        state: Value,
    ) -> Result<(), LxAppError> {
        let msg = StateSnapshot {
            v: 2,
            kind: "state.snapshot",
            scope,
            rev,
            state,
        };
        self.send_json(transport, &msg)
    }

    pub fn send_state_patch<T: MessageTransport>(
        &self,
        transport: &T,
        scope: Option<String>,
        base_rev: u64,
        rev: u64,
        ops: Vec<JsonPatchOp>,
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

    pub async fn process_incoming_message<T, H>(
        &self,
        transport: &T,
        handler: &H,
        message: Arc<IncomingMessage>,
    ) -> Result<(), LxAppError>
    where
        T: MessageTransport + Clone + 'static,
        H: MessageHandler + Clone + 'static,
    {
        match &*message {
            IncomingMessage::Hello(msg) => {
                if msg.v != 2 {
                    return Err(LxAppError::Bridge(format!(
                        "Unsupported protocol: {}",
                        msg.v
                    )));
                }
                if !msg.protocols_supported.contains(&2) {
                    return Err(LxAppError::Bridge(format!(
                        "Protocol 2 not in supported list"
                    )));
                }
                if msg.role != "view" {
                    return Err(LxAppError::Bridge(format!("Unexpected role: {}", msg.role)));
                }

                if let Some(expected) = handler.expected_bridge_nonce() {
                    if &expected != &msg.nonce {
                        return Err(LxAppError::Bridge("Nonce mismatch".to_string()));
                    }
                }

                let session_id = self.new_session_id();
                self.send_hello_ack(transport, msg.nonce.clone(), session_id.clone())
                    .await?;

                // Mark logic-side bridge ready immediately and send `ready` as soon as possible.
                //
                // IMPORTANT:
                // Do not block `ready` behind initial state snapshot generation or lifecycle hooks.
                // In practice these can be slow (large init data / busy worker) and cause the View's
                // handshake timer to fire, leading to intermittent "Handshake timeout" on startup.
                self.set_ready(session_id.clone());
                self.send_ready(transport, session_id.clone()).await?;
                crate::info!("Bridge ready");

                // Send initial state + lifecycle events after the handshake completes.
                // This also better matches the bridge spec ("ready" gates application messages).
                let handler = <H as Clone>::clone(handler);
                rong::spawn(async move {
                    handler.handle_bridge_ready().await;
                });
                return Ok(());
            }
            IncomingMessage::Req(msg) => {
                if msg.v != 2 {
                    let _ = self.send_res_err(
                        transport,
                        msg.id.clone(),
                        BRIDGE_PROTOCOL_MISMATCH,
                        Some(format!("Unsupported protocol: {}", msg.v)),
                    );
                    return Ok(());
                }

                let ReqMsg {
                    id,
                    method,
                    params,
                    cap,
                    ..
                } = msg;
                if !self.is_ready() {
                    let _ = self.send_res_err(
                        transport,
                        id.clone(),
                        BRIDGE_NOT_READY,
                        Some("Bridge not ready".to_string()),
                    );
                    return Ok(());
                }
                let required_cap = Self::required_cap_for_method(method);
                if cap.is_empty() {
                    let _ = self.send_res_err(
                        transport,
                        id.clone(),
                        BRIDGE_MALFORMED_MESSAGE,
                        Some("Missing cap".to_string()),
                    );
                    return Ok(());
                }
                if cap != &required_cap {
                    let _ = self.send_res_err(
                        transport,
                        id.clone(),
                        BRIDGE_MALFORMED_MESSAGE,
                        Some(format!("Capability mismatch: expected '{}'", required_cap)),
                    );
                    return Ok(());
                }
                if !handler.is_cap_allowed(&required_cap) {
                    let _ =
                        self.send_res_err(transport, id.clone(), BRIDGE_CAPABILITY_DENIED, None);
                    return Ok(());
                }

                // Handle state.getSnapshot specially
                if method == "state.getSnapshot" {
                    let scope = params
                        .as_ref()
                        .and_then(|v| v.get("scope"))
                        .and_then(|v| v.as_str());
                    // Snapshot building can be expensive (large state + stringify/parse), so keep
                    // it off the bridge message pump.
                    let bridge = <Bridge as Clone>::clone(self);
                    let transport = <T as Clone>::clone(transport);
                    let handler = <H as Clone>::clone(handler);
                    let id = id.clone();
                    let scope = scope.map(|s| s.to_string());
                    rong::spawn(async move {
                        match handler.get_state_snapshot(scope.as_deref()).await {
                            Ok(snapshot) => {
                                let _ = bridge.send_res_ok(&transport, id.clone(), snapshot);
                            }
                            Err(e) => {
                                let _ = bridge.send_res_err(
                                    &transport,
                                    id.clone(),
                                    BRIDGE_INTERNAL_ERROR,
                                    Some(e.to_string()),
                                );
                            }
                        }
                    });
                    return Ok(());
                }

                // Setup cancel channel
                let (cancel_tx, cancel_rx) = oneshot::channel();
                self.pending_req_cancel
                    .lock()
                    .unwrap()
                    .insert(id.clone(), cancel_tx);

                let params_json = params
                    .as_ref()
                    .map(|v| serde_json::to_string(v))
                    .transpose()?;
                let service_type = handler.get_service_type(method);

                if matches!(service_type, ServiceType::None) {
                    self.pending_req_cancel.lock().unwrap().remove(id);
                    let _ = self.send_res_err(
                        transport,
                        id.clone(),
                        BRIDGE_METHOD_NOT_FOUND,
                        Some(format!("Method not found: {}", method)),
                    );
                    return Ok(());
                }

                // IMPORTANT:
                // Do not await handler.handle_req() here, otherwise we can deadlock the single-thread
                // worker message pump when the handler itself triggers nested ServiceMessages.
                let bridge = <Bridge as Clone>::clone(self);
                let transport = <T as Clone>::clone(transport);
                let handler = <H as Clone>::clone(handler);
                let id = id.clone();
                let method = method.to_string();
                rong::spawn(async move {
                    let result = handler
                        .handle_req(&method, params_json.as_deref(), service_type, cancel_rx)
                        .await;

                    bridge.pending_req_cancel.lock().unwrap().remove(&id);

                    match result {
                        Ok(value) => {
                            let _ = bridge.send_res_ok(&transport, id.clone(), value);
                        }
                        Err(e) => {
                            let _ = bridge.send_res_err(&transport, id.clone(), &e.code, e.message);
                        }
                    }
                });
                return Ok(());
            }
            IncomingMessage::Res(msg) => {
                if msg.v != 2 {
                    return Ok(());
                }
                // Response from View to a Logic→View request (view_call)
                let result = if msg.ok {
                    Ok(msg.result.clone().unwrap_or(Value::Null))
                } else {
                    let err = msg.error.as_ref();
                    Err(RpcError {
                        code: err
                            .map(|e| e.code.clone())
                            .unwrap_or_else(|| BRIDGE_INTERNAL_ERROR.to_string()),
                        message: err.and_then(|e| e.message.clone()),
                    })
                };
                let page_path = handler.bridge_page_path();
                super::view_call::resolve_view_call(&msg.id, page_path.as_deref(), result);
                return Ok(());
            }
            IncomingMessage::Notify(msg) => {
                if msg.v != 2 {
                    return Ok(());
                }
                if !self.is_ready() {
                    return Ok(());
                }
                let required_cap = Self::required_cap_for_method(&msg.method);
                if msg.cap.is_empty() || msg.cap != required_cap {
                    return Ok(());
                }
                if !handler.is_cap_allowed(&required_cap) {
                    return Ok(());
                }

                let params_json = msg
                    .params
                    .as_ref()
                    .map(|v| serde_json::to_string(v))
                    .transpose()?;
                let service_type = handler.get_service_type(&msg.method);
                handler
                    .handle_notify(&msg.method, params_json.as_deref(), service_type)
                    .await;
                return Ok(());
            }
            IncomingMessage::Cancel(msg) => {
                if msg.v != 2 {
                    return Ok(());
                }
                if let Some(tx) = self.pending_req_cancel.lock().unwrap().remove(&msg.id) {
                    let _ = tx.send(());
                }
                return Ok(());
            }
            IncomingMessage::StateAck(msg) => {
                if msg.v != 2 {
                    return Ok(());
                }
                handler.handle_state_ack(msg.scope.clone(), msg.rev).await;
                return Ok(());
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
                                .map(|k| format!("Unknown kind: {}", k))
                                .or_else(|| unknown.parse_error.clone())
                                .or_else(|| Some("Unknown message".to_string())),
                        )
                    };
                    let _ = self.send_res_err(transport, id.clone(), code, message);
                }
                return Ok(());
            }
        }
    }
}
