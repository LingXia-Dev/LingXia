//! Bridge wire protocol types — incoming and outgoing message definitions.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::value::RawValue;
use std::collections::HashMap;

use lingxia_error::code;

#[derive(Deserialize)]
struct VersionProbe {
    v: Option<u8>,
}

/// Decode a document frame. Any version other than 2 is rejected before the
/// typed parser can create a message for downstream dispatch.
pub(crate) fn decode_v2_inbound(frame: &str) -> Result<IncomingMessage, lingxia_error::Error> {
    let version = serde_json::from_str::<VersionProbe>(frame)
        .map_err(|e| lingxia_error::Error::internal(format!("Invalid JSON: {e}")))?;
    if version.v != Some(2) {
        return Err(lingxia_error::Error::internal(format!(
            "Unsupported protocol: {:?}",
            version.v
        )));
    }
    IncomingMessage::from_json_str(frame)
}

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
    pub id: String,
    pub method: String,
    pub params: Option<Box<RawValue>>,
    #[serde(default)]
    pub cap: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct NotifyMsg {
    pub method: String,
    pub params: Option<Box<RawValue>>,
    #[serde(default)]
    pub cap: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CancelMsg {
    pub id: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ChOpenMsg {
    pub id: String,
    pub topic: String,
    pub params: Option<Box<RawValue>>,
    #[serde(default)]
    pub cap: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ChDataMsg {
    pub id: String,
    pub payload: Box<RawValue>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ChCloseMsg {
    pub id: String,
    pub error: Option<ResError>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StateAckMsg {
    pub scope: Option<String>,
    pub rev: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ResError {
    pub code: Value,
    pub message: Option<String>,
    // Boxed so a failed reply does not size every incoming message.
    pub reason: Option<Box<str>>,
    pub data: Option<Box<Value>>,
}

impl ResError {
    pub(super) fn normalized_code(&self) -> String {
        match &self.code {
            Value::String(code) => code.clone(),
            Value::Number(code) => code.to_string(),
            other => {
                crate::warn!("Unexpected bridge error code type in response: {}", other);
                code::INTERNAL.to_string()
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
    Notify(NotifyMsg),
    Cancel(CancelMsg),
    ChOpen(ChOpenMsg),
    ChData(ChDataMsg),
    ChClose(ChCloseMsg),
    StateAck(StateAckMsg),
    Unknown(UnknownMsg),
}

impl IncomingMessage {
    pub fn from_json_str(json_str: &str) -> Result<Self, lingxia_error::Error> {
        #[derive(Deserialize)]
        struct KindProbe {
            v: Option<u8>,
            kind: Option<String>,
            id: Option<String>,
        }

        let probe: KindProbe = serde_json::from_str(json_str)
            .map_err(|e| lingxia_error::Error::internal(format!("Invalid JSON: {}", e)))?;

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
    #[serde(rename = "hostChannels", skip_serializing_if = "Vec::is_empty")]
    pub host_channels: Vec<String>,
}

#[derive(Serialize)]
pub(super) struct BridgeError {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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
    pub error: Option<BridgeError>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_schema_advertises_channels_without_overloading_method_kinds() {
        let message = ReadyMsg {
            v: 2,
            kind: "ready",
            session_id: "session".to_string(),
            host_methods: HashMap::from([
                ("demo.call".to_string(), "call"),
                ("demo.watch".to_string(), "stream"),
            ]),
            host_channels: vec!["demo.channel".to_string()],
        };
        let encoded = serde_json::to_value(message).expect("serialize ready schema");

        assert_eq!(encoded["hostMethods"]["demo.call"], "call");
        assert_eq!(encoded["hostMethods"]["demo.watch"], "stream");
        assert!(encoded["hostMethods"].get("demo.channel").is_none());
        assert_eq!(encoded["hostChannels"][0], "demo.channel");
    }

    #[test]
    fn decoder_accepts_v2_and_rejects_other_versions_before_typed_parse() {
        let parsed = decode_v2_inbound(
            r#"{"v":2,"kind":"hello","nonce":"n","role":"view","protocolsSupported":[2]}"#,
        )
        .unwrap();
        assert!(matches!(
            parsed,
            IncomingMessage::Hello(HelloMsg { v: 2, protocols_supported, .. }) if protocols_supported == [2]
        ));
        for frame in [
            r#"{"v":3,"kind":"req","id":"r","method":"host.x","cap":"host"}"#,
            r#"{"kind":"req","id":"r","method":"host.x","cap":"host"}"#,
            "not json",
        ] {
            assert!(decode_v2_inbound(frame).is_err(), "{frame}");
        }
    }
}
