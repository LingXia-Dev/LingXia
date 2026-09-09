//! Bridge wire protocol types — incoming and outgoing message definitions.

use crate::LxAppError;
use ring::hmac;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::value::RawValue;
use std::collections::HashMap;

use super::BRIDGE_INTERNAL_ERROR;

/// Protocol version reserved for document-session binding. Production bridge
/// negotiation remains on V2 until both endpoints opt in together.
pub(crate) const V3_PROTOCOL: u8 = 3;

/// Upper bound for a single unauthenticated V3 frame. The production ingress
/// will apply this before parsing once V3 is enabled.
pub(crate) const DEFAULT_MAX_V3_FRAME_BYTES: usize = 64 * 1024;

/// A document-to-native secret. It deliberately has no `Debug` or serde
/// implementation so diagnostics and outbound codecs cannot expose it.
pub(crate) struct DocumentSecret(String);

impl DocumentSecret {
    fn new(value: String) -> Self {
        Self(value)
    }
}

/// Binding carried by every V3 document-to-native frame.
pub(crate) struct V3InboundBinding {
    session_id: String,
    secret: DocumentSecret,
}

impl V3InboundBinding {
    pub(crate) fn new(session_id: String, secret: String) -> Result<Self, V3CodecError> {
        if session_id.is_empty() || secret.is_empty() {
            return Err(V3CodecError::InvalidInboundBinding);
        }
        Ok(Self {
            session_id,
            secret: DocumentSecret::new(secret),
        })
    }

    pub(crate) fn session_id(&self) -> &str {
        &self.session_id
    }

    fn matches(&self, candidate: &Self) -> bool {
        // HMAC verification is ring's maintained constant-time comparison.
        // Evaluate both public-id and secret comparisons before combining them.
        let key = hmac::Key::new(hmac::HMAC_SHA256, b"lingxia-v3-bridge-binding");
        let expected_session = hmac::sign(&key, self.session_id.as_bytes());
        let expected_secret = hmac::sign(&key, self.secret.0.as_bytes());
        let session_matches = hmac::verify(
            &key,
            candidate.session_id.as_bytes(),
            expected_session.as_ref(),
        )
        .is_ok();
        let secret_matches = hmac::verify(
            &key,
            candidate.secret.0.as_bytes(),
            expected_secret.as_ref(),
        )
        .is_ok();
        session_matches & secret_matches
    }
}

/// Binding carried by every V3 native-to-document frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct V3OutboundBinding {
    session_id: String,
}

impl V3OutboundBinding {
    #[allow(dead_code)] // Constructed when a document session binds V3 in the next step.
    pub(crate) fn new(session_id: String) -> Result<Self, V3CodecError> {
        if session_id.is_empty() {
            return Err(V3CodecError::InvalidOutboundBinding);
        }
        Ok(Self { session_id })
    }
}

/// The connection mode is chosen by native document-session activation. Every
/// existing page remains `LegacyV2` until that later integration opts in.
#[derive(Default)]
pub(crate) enum BridgeProtocol {
    #[default]
    LegacyV2,
    BoundV3(BoundV3Protocol),
}

/// Native-held credentials for one activated V3 document. The secret stays in
/// the inbound binding and cannot flow into the outbound encoder.
pub(crate) struct BoundV3Protocol {
    inbound: V3InboundBinding,
    outbound: V3OutboundBinding,
}

impl BoundV3Protocol {
    #[allow(dead_code)] // Called by the future document-session bridge binding.
    pub(crate) fn new(inbound: V3InboundBinding) -> Result<Self, V3CodecError> {
        let outbound = V3OutboundBinding::new(inbound.session_id.clone())?;
        Ok(Self { inbound, outbound })
    }

    pub(crate) fn session_id(&self) -> &str {
        self.inbound.session_id()
    }

    pub(crate) fn authenticates(&self, binding: &V3InboundBinding) -> bool {
        self.inbound.matches(binding)
    }

    pub(crate) fn outbound_binding(&self) -> V3OutboundBinding {
        self.outbound.clone()
    }
}

impl BridgeProtocol {
    pub(crate) fn predecode_inbound(&self, frame: &str) -> Result<IncomingMessage, V3CodecError> {
        match self {
            Self::LegacyV2 => {
                // Reject a V3 (or unknown) version before the legacy typed
                // parser can create a message for downstream dispatch.
                let version = serde_json::from_str::<VersionProbe>(frame)
                    .map_err(|_| V3CodecError::MalformedEnvelope)?;
                if version.v != Some(2) {
                    return Err(V3CodecError::UnsupportedVersion);
                }
                IncomingMessage::from_json_str(frame).map_err(|_| V3CodecError::MalformedEnvelope)
            }
            Self::BoundV3(bound) => {
                let envelope = parse_v3_inbound_envelope(frame, DEFAULT_MAX_V3_FRAME_BYTES)?;
                if !bound.authenticates(&envelope.binding) {
                    return Err(V3CodecError::BindingMismatch);
                }
                let message = IncomingMessage::from_json_str(frame)
                    .map_err(|_| V3CodecError::MalformedEnvelope)?;
                if message.v3_kind() != Some(envelope.kind) {
                    return Err(V3CodecError::MalformedEnvelope);
                }
                Ok(message)
            }
        }
    }

    pub(crate) const fn accepts_version(&self, version: u8) -> bool {
        matches!(
            (self, version),
            (Self::LegacyV2, 2) | (Self::BoundV3(_), V3_PROTOCOL)
        )
    }

    pub(crate) fn session_id(&self) -> Option<&str> {
        match self {
            Self::LegacyV2 => None,
            Self::BoundV3(bound) => Some(bound.session_id()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum V3InboundKind {
    Hello,
    Req,
    Res,
    Notify,
    Cancel,
    ChOpen,
    ChData,
    ChClose,
    StateAck,
}

impl V3InboundKind {
    fn parse(kind: &str) -> Option<Self> {
        Some(match kind {
            "hello" => Self::Hello,
            "req" => Self::Req,
            "res" => Self::Res,
            "notify" => Self::Notify,
            "cancel" => Self::Cancel,
            "ch.open" => Self::ChOpen,
            "ch.data" => Self::ChData,
            "ch.close" => Self::ChClose,
            "state.ack" => Self::StateAck,
            _ => return None,
        })
    }

    #[cfg(test)]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Hello => "hello",
            Self::Req => "req",
            Self::Res => "res",
            Self::Notify => "notify",
            Self::Cancel => "cancel",
            Self::ChOpen => "ch.open",
            Self::ChData => "ch.data",
            Self::ChClose => "ch.close",
            Self::StateAck => "state.ack",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum V3OutboundKind {
    HelloAck,
    Ready,
    Req,
    Res,
    Event,
    StateSnapshot,
    StatePatch,
    ChAck,
    ChData,
    ChClose,
}

impl V3OutboundKind {
    #[cfg(test)]
    pub(crate) fn parse(kind: &str) -> Option<Self> {
        Some(match kind {
            "helloAck" => Self::HelloAck,
            "ready" => Self::Ready,
            "req" => Self::Req,
            "res" => Self::Res,
            "event" => Self::Event,
            "state.snapshot" => Self::StateSnapshot,
            "state.patch" => Self::StatePatch,
            "ch.ack" => Self::ChAck,
            "ch.data" => Self::ChData,
            "ch.close" => Self::ChClose,
            _ => return None,
        })
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::HelloAck => "helloAck",
            Self::Ready => "ready",
            Self::Req => "req",
            Self::Res => "res",
            Self::Event => "event",
            Self::StateSnapshot => "state.snapshot",
            Self::StatePatch => "state.patch",
            Self::ChAck => "ch.ack",
            Self::ChData => "ch.data",
            Self::ChClose => "ch.close",
        }
    }
}

/// The result of the intentionally small V3 authentication-envelope parse.
/// Route-specific payload decoding remains a separate, later operation.
pub(crate) struct V3InboundEnvelope {
    pub(crate) kind: V3InboundKind,
    pub(crate) binding: V3InboundBinding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum V3CodecError {
    FrameTooLarge,
    MalformedEnvelope,
    UnsupportedVersion,
    UnsupportedInboundKind,
    InvalidOutboundPayload,
    #[allow(dead_code)] // Reported when a future document session creates its outbound binding.
    InvalidOutboundBinding,
    InvalidInboundBinding,
    BindingMismatch,
    SecurityFieldInPayload,
}

#[derive(Deserialize)]
struct V3EnvelopeProbe {
    v: Option<u8>,
    kind: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    secret: Option<String>,
}

#[derive(Deserialize)]
struct VersionProbe {
    v: Option<u8>,
}

/// Parse only the fixed V3 authentication envelope. This intentionally ignores
/// route and parameter fields so a future admission layer can authenticate
/// before route-specific decoding or allocation.
pub(crate) fn parse_v3_inbound_envelope(
    frame: &str,
    max_frame_bytes: usize,
) -> Result<V3InboundEnvelope, V3CodecError> {
    if frame.len() > max_frame_bytes {
        return Err(V3CodecError::FrameTooLarge);
    }

    let probe = serde_json::from_str::<V3EnvelopeProbe>(frame)
        .map_err(|_| V3CodecError::MalformedEnvelope)?;
    if probe.v != Some(V3_PROTOCOL) {
        return Err(V3CodecError::UnsupportedVersion);
    }
    let kind = probe
        .kind
        .as_deref()
        .and_then(V3InboundKind::parse)
        .ok_or(V3CodecError::UnsupportedInboundKind)?;
    let session_id = probe
        .session_id
        .filter(|value| !value.is_empty())
        .ok_or(V3CodecError::MalformedEnvelope)?;
    let secret = probe
        .secret
        .filter(|value| !value.is_empty())
        .ok_or(V3CodecError::MalformedEnvelope)?;

    Ok(V3InboundEnvelope {
        kind,
        binding: V3InboundBinding::new(session_id, secret)?,
    })
}

/// Compose a native-to-document V3 frame. Protocol identity belongs only to
/// the binding, so matching route-payload fields are rejected.
pub(crate) fn encode_v3_outbound_frame(
    binding: &V3OutboundBinding,
    kind: V3OutboundKind,
    payload: Value,
) -> Result<Value, V3CodecError> {
    let mut frame = payload
        .as_object()
        .cloned()
        .ok_or(V3CodecError::InvalidOutboundPayload)?;
    if ["v", "kind", "sessionId", "secret"]
        .iter()
        .any(|field| frame.contains_key(*field))
    {
        return Err(V3CodecError::SecurityFieldInPayload);
    }
    frame.insert("v".to_string(), Value::from(V3_PROTOCOL));
    frame.insert("kind".to_string(), Value::from(kind.as_str()));
    frame.insert(
        "sessionId".to_string(),
        Value::from(binding.session_id.clone()),
    );
    Ok(Value::Object(frame))
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
    pub(crate) fn version(&self) -> Option<u8> {
        match self {
            Self::Hello(message) => Some(message.v),
            Self::Req(message) => Some(message.v),
            Self::Res(message) => Some(message.v),
            Self::Notify(message) => Some(message.v),
            Self::Cancel(message) => Some(message.v),
            Self::ChOpen(message) => Some(message.v),
            Self::ChData(message) => Some(message.v),
            Self::ChClose(message) => Some(message.v),
            Self::StateAck(message) => Some(message.v),
            Self::Unknown(message) => message.v,
        }
    }

    fn v3_kind(&self) -> Option<V3InboundKind> {
        Some(match self {
            Self::Hello(_) => V3InboundKind::Hello,
            Self::Req(_) => V3InboundKind::Req,
            Self::Res(_) => V3InboundKind::Res,
            Self::Notify(_) => V3InboundKind::Notify,
            Self::Cancel(_) => V3InboundKind::Cancel,
            Self::ChOpen(_) => V3InboundKind::ChOpen,
            Self::ChData(_) => V3InboundKind::ChData,
            Self::ChClose(_) => V3InboundKind::ChClose,
            Self::StateAck(_) => V3InboundKind::StateAck,
            Self::Unknown(_) => return None,
        })
    }

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
    #[serde(rename = "hostChannels", skip_serializing_if = "Vec::is_empty")]
    pub host_channels: Vec<String>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct GoldenFrame {
        kind: String,
        #[serde(default)]
        payload: Option<Value>,
        frame: Value,
    }

    #[derive(Deserialize)]
    struct InvalidGoldenFrame {
        frame: String,
        error: String,
    }

    #[derive(Deserialize)]
    struct InvalidFrames {
        #[serde(rename = "documentToNative")]
        document_to_native: Vec<InvalidGoldenFrame>,
        #[serde(rename = "nestedSecurityKeysAreNotTopLevelDuplicates")]
        nested_security_keys: NestedSecurityKeys,
    }

    #[derive(Deserialize)]
    struct NestedSecurityKeys {
        #[serde(rename = "documentToNative")]
        document_to_native: String,
    }

    #[derive(Deserialize)]
    struct GoldenFrames {
        inbound: Vec<GoldenFrame>,
        outbound: Vec<GoldenFrame>,
    }

    fn golden_frames() -> GoldenFrames {
        serde_json::from_str(include_str!("../../../../testdata/bridge-v3/golden.json"))
            .expect("shared V3 golden fixture must be valid JSON")
    }

    fn invalid_frames() -> InvalidFrames {
        serde_json::from_str(include_str!("../../../../testdata/bridge-v3/invalid.json"))
            .expect("shared V3 invalid fixture must be valid JSON")
    }

    #[test]
    fn v3_inbound_envelopes_match_shared_golden_frames() {
        let golden = golden_frames();
        let protocol = BridgeProtocol::BoundV3(
            BoundV3Protocol::new(
                V3InboundBinding::new(
                    "v3-session".to_string(),
                    "bridge-v3-test-secret".to_string(),
                )
                .unwrap(),
            )
            .unwrap(),
        );

        for expected in golden.inbound {
            let raw = serde_json::to_string(&expected.frame).unwrap();
            let message = protocol.predecode_inbound(&raw).unwrap();
            assert_eq!(message.v3_kind().unwrap().as_str(), expected.kind);
        }
    }

    #[test]
    fn v3_outbound_frames_match_shared_golden_frames() {
        let binding = V3OutboundBinding::new("v3-session".to_string()).unwrap();
        let golden = golden_frames();

        for expected in golden.outbound {
            let kind = V3OutboundKind::parse(&expected.kind)
                .expect("shared fixture must use a known outbound kind");
            let encoded = encode_v3_outbound_frame(
                &binding,
                kind,
                expected.payload.expect("outbound fixture needs a payload"),
            )
            .unwrap();
            assert_eq!(encoded, expected.frame, "kind={}", expected.kind);
        }
    }

    #[test]
    fn v3_envelope_rejects_shared_invalid_frames() {
        for expected in invalid_frames().document_to_native {
            let actual = parse_v3_inbound_envelope(&expected.frame, DEFAULT_MAX_V3_FRAME_BYTES);
            let expected_error = match expected.error.as_str() {
                "MALFORMED_ENVELOPE" => V3CodecError::MalformedEnvelope,
                "UNSUPPORTED_VERSION" => V3CodecError::UnsupportedVersion,
                "UNSUPPORTED_INBOUND_KIND" => V3CodecError::UnsupportedInboundKind,
                other => panic!("unknown shared V3 codec error: {other}"),
            };
            assert!(matches!(actual, Err(error) if error == expected_error));
        }
    }

    #[test]
    fn v3_envelope_allows_nested_security_keys() {
        let frame = invalid_frames().nested_security_keys.document_to_native;
        assert!(parse_v3_inbound_envelope(&frame, DEFAULT_MAX_V3_FRAME_BYTES).is_ok());
    }

    #[test]
    fn v3_envelope_rejects_oversized_or_unbound_frames() {
        let oversized = format!(
            r#"{{"v":3,"kind":"req","sessionId":"s","secret":"x","pad":"{}"}}"#,
            "a".repeat(32)
        );
        assert!(matches!(
            parse_v3_inbound_envelope(&oversized, 16),
            Err(V3CodecError::FrameTooLarge)
        ));
        assert!(matches!(
            parse_v3_inbound_envelope(r#"{"v":3,"kind":"req","sessionId":"s"}"#, 1024),
            Err(V3CodecError::MalformedEnvelope)
        ));
        assert!(matches!(
            parse_v3_inbound_envelope(r#"{"v":2,"kind":"req","sessionId":"s","secret":"x"}"#, 1024),
            Err(V3CodecError::UnsupportedVersion)
        ));
    }

    #[test]
    fn bound_v3_rejects_wrong_binding_and_legacy_mixing_before_typed_parse() {
        let protocol = BridgeProtocol::BoundV3(
            BoundV3Protocol::new(
                V3InboundBinding::new("session".to_string(), "secret".to_string()).unwrap(),
            )
            .unwrap(),
        );
        assert!(matches!(
            protocol.predecode_inbound(
                r#"{"v":3,"kind":"req","sessionId":"session","secret":"wrong","id":"r","method":"host.x","cap":"host"}"#
            ),
            Err(V3CodecError::BindingMismatch)
        ));
        assert!(matches!(
            protocol.predecode_inbound(
                r#"{"v":2,"kind":"req","id":"r","method":"host.x","cap":"host"}"#
            ),
            Err(V3CodecError::UnsupportedVersion)
        ));

        let legacy = BridgeProtocol::LegacyV2;
        assert!(matches!(
            legacy.predecode_inbound(
                r#"{"v":3,"kind":"req","id":"r","method":"host.x","cap":"host"}"#
            ),
            Err(V3CodecError::UnsupportedVersion)
        ));
    }

    #[test]
    fn v3_outbound_rejects_security_fields_and_empty_binding() {
        assert!(matches!(
            V3OutboundBinding::new(String::new()),
            Err(V3CodecError::InvalidOutboundBinding)
        ));
        let binding = V3OutboundBinding::new("v3-session".to_string()).unwrap();
        for field in ["v", "kind", "sessionId", "secret"] {
            let mut payload = serde_json::Map::new();
            payload.insert(field.to_string(), Value::String("forged".to_string()));
            assert!(matches!(
                encode_v3_outbound_frame(&binding, V3OutboundKind::Ready, Value::Object(payload)),
                Err(V3CodecError::SecurityFieldInPayload)
            ));
        }
    }

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
    fn v2_parser_behavior_remains_available_while_v3_is_dormant() {
        let legacy = BridgeProtocol::LegacyV2;
        let parsed = legacy
            .predecode_inbound(
                r#"{"v":2,"kind":"hello","nonce":"legacy","role":"view","protocolsSupported":[2]}"#,
            )
            .unwrap();
        assert!(matches!(
            parsed,
            IncomingMessage::Hello(HelloMsg { v: 2, protocols_supported, .. }) if protocols_supported == [2]
        ));
        assert!(matches!(
            legacy.predecode_inbound(
                r#"{"v":3,"kind":"hello","sessionId":"s","secret":"x","nonce":"v3","role":"view","protocolsSupported":[3]}"#
            ),
            Err(V3CodecError::UnsupportedVersion)
        ));
    }
}
