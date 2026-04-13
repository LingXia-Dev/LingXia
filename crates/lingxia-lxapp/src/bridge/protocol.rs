//! Bridge wire protocol types — incoming and outgoing message definitions.

use crate::LxAppError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::value::RawValue;
use std::collections::HashMap;

use super::BRIDGE_INTERNAL_ERROR;

// ── Incoming (View → Logic) ─────────────────────────────────────────────

#[derive(Deserialize, Debug, Clone)]
pub struct HelloMsg {
    pub v: u8,
    pub nonce: String,
    pub role: String,
    #[serde(default, rename = "protocolsSupported")]
    pub protocols_supported: Vec<u32>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ReqMsg {
    pub v: u8,
    pub id: String,
    pub method: String,
    pub params: Option<Box<RawValue>>,
    #[serde(default)]
    pub cap: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NotifyMsg {
    pub v: u8,
    pub method: String,
    pub params: Option<Box<RawValue>>,
    #[serde(default)]
    pub cap: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CancelMsg {
    pub v: u8,
    pub id: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ChOpenMsg {
    pub v: u8,
    pub id: String,
    pub topic: String,
    pub params: Option<Box<RawValue>>,
    #[serde(default)]
    pub cap: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ChDataMsg {
    pub v: u8,
    pub id: String,
    pub payload: Box<RawValue>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ChCloseMsg {
    pub v: u8,
    pub id: String,
    pub code: Option<String>,
    pub reason: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StateAckMsg {
    pub v: u8,
    pub scope: Option<String>,
    pub rev: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ResMsg {
    pub v: u8,
    pub id: String,
    #[serde(default)]
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<ResError>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ResError {
    pub code: Value,
    pub message: Option<String>,
    pub data: Option<Value>,
}

impl ResError {
    pub(super) fn normalized_code(&self) -> String {
        match &self.code {
            Value::String(code) => code.clone(),
            Value::Number(code) => code.to_string(),
            other => {
                crate::warn!("Unexpected bridge error code type in response: {}", other);
                BRIDGE_INTERNAL_ERROR.to_string()
            }
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct UnknownMsg {
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
    ChOpen(ChOpenMsg),
    ChData(ChDataMsg),
    ChClose(ChCloseMsg),
    StateAck(StateAckMsg),
    Unknown(UnknownMsg),
}

impl IncomingMessage {
    pub fn from_json_str(json_str: &str) -> Result<Self, LxAppError> {
        #[derive(Deserialize)]
        struct KindProbe {
            v: Option<u8>,
            kind: Option<String>,
            id: Option<String>,
        }

        let probe: KindProbe = serde_json::from_str(json_str)
            .map_err(|e| LxAppError::Bridge(format!("Invalid JSON: {}", e)))?;

        let Some(kind_str) = probe.kind.as_deref() else {
            return Ok(Self::Unknown(UnknownMsg {
                v: probe.v,
                kind: None,
                id: probe.id,
                parse_error: Some("Missing 'kind'".to_string()),
            }));
        };

        match kind_str {
            "hello" => serde_json::from_str::<HelloMsg>(json_str).map(Self::Hello),
            "req" => serde_json::from_str::<ReqMsg>(json_str).map(Self::Req),
            "res" => serde_json::from_str::<ResMsg>(json_str).map(Self::Res),
            "notify" => serde_json::from_str::<NotifyMsg>(json_str).map(Self::Notify),
            "cancel" => serde_json::from_str::<CancelMsg>(json_str).map(Self::Cancel),
            "ch.open" => serde_json::from_str::<ChOpenMsg>(json_str).map(Self::ChOpen),
            "ch.data" => serde_json::from_str::<ChDataMsg>(json_str).map(Self::ChData),
            "ch.close" => serde_json::from_str::<ChCloseMsg>(json_str).map(Self::ChClose),
            "state.ack" => serde_json::from_str::<StateAckMsg>(json_str).map(Self::StateAck),
            _ => {
                return Ok(Self::Unknown(UnknownMsg {
                    v: probe.v,
                    kind: probe.kind,
                    id: probe.id,
                    parse_error: None,
                }));
            }
        }
        .or_else(|e| {
            Ok(Self::Unknown(UnknownMsg {
                v: probe.v,
                kind: probe.kind,
                id: probe.id,
                parse_error: Some(e.to_string()),
            }))
        })
    }
}

// ── Outgoing (Logic → View) ─────────────────────────────────────────────

#[derive(Serialize)]
pub(super) struct HelloAck {
    pub v: u8,
    pub kind: &'static str,
    pub nonce: String,
    pub protocol: u8,
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

#[derive(Serialize)]
pub(super) struct ReadyMsg {
    pub v: u8,
    pub kind: &'static str,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "hostMethods", skip_serializing_if = "HashMap::is_empty")]
    pub host_methods: HashMap<String, &'static str>,
}

#[derive(Serialize)]
pub(super) struct BridgeError {
    pub code: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Serialize)]
pub(super) struct Res {
    pub v: u8,
    pub kind: &'static str,
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Box<RawValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<BridgeError>,
}

#[derive(Serialize)]
pub(super) struct StateSnapshotOut {
    pub v: u8,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    pub rev: u64,
    pub state: Box<RawValue>,
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JsonPatchOp {
    pub op: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

#[derive(Serialize)]
pub(super) struct StatePatch {
    pub v: u8,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(rename = "baseRev")]
    pub base_rev: u64,
    pub rev: u64,
    pub ops: Box<RawValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ack: Option<bool>,
}

#[derive(Serialize)]
pub(super) struct ChAck {
    pub v: u8,
    pub kind: &'static str,
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<BridgeError>,
}

#[derive(Serialize)]
pub(super) struct ChCloseOut {
    pub v: u8,
    pub kind: &'static str,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
