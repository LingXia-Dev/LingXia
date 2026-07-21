//! Public run types for the isolated host automation runtime.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Maximum unencoded JavaScript source accepted for one run.
pub const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
/// Maximum JSON-compatible final result retained for one run.
pub const MAX_RESULT_BYTES: usize = 8 * 1024 * 1024;
/// Maximum bytes retained for one console message.
pub const MAX_CONSOLE_EVENT_BYTES: usize = 64 * 1024;
/// Maximum retained non-artifact event bytes before oldest events are dropped.
pub const MAX_RETAINED_EVENT_BYTES: usize = 8 * 1024 * 1024;
/// Maximum decoded bytes for one attachment.
pub const MAX_ATTACHMENT_BYTES: usize = 16 * 1024 * 1024;
/// Maximum decoded attachment bytes retained by one run.
pub const MAX_RUN_ATTACHMENT_BYTES: usize = 32 * 1024 * 1024;
/// Maximum serialized event bytes returned by one poll response.
pub const MAX_POLL_EVENT_BYTES: usize = 24 * 1024 * 1024;
pub const MIN_TIMEOUT_MS: u64 = 1_000;
pub const MAX_TIMEOUT_MS: u64 = 3_600_000;
pub const DEFAULT_TIMEOUT_MS: u64 = 60_000;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AutomationStartArgs {
    /// One classic JavaScript program. Its final value becomes `result.output`.
    pub source: String,
    pub source_name: Option<String>,
    pub timeout_ms: Option<u64>,
    /// String inputs exposed as `__LINGXIA_AUTOMATION_HOST__.args`.
    #[serde(default)]
    pub args: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AutomationStartResponse {
    pub run_id: String,
    pub state: AutomationRunState,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AutomationPollArgs {
    pub run_id: String,
    /// Events with `seq` greater than this are returned; asking for later
    /// events acknowledges and releases everything at or below it.
    #[serde(default)]
    pub after_seq: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AutomationPollResponse {
    pub run_id: String,
    pub state: AutomationRunState,
    pub next_seq: u64,
    pub events: Vec<AutomationEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<AutomationRunResult>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AutomationCancelArgs {
    pub run_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AutomationCancelResponse {
    pub run_id: String,
    pub state: AutomationRunState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationRunState {
    Running,
    Succeeded,
    Failed,
    TimedOut,
    Cancelled,
    InternalError,
}

impl AutomationRunState {
    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Running)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AutomationEvent {
    pub seq: u64,
    #[serde(flatten)]
    pub payload: AutomationEventPayload,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AutomationEventPayload {
    Console {
        level: String,
        message: String,
    },
    Artifact {
        name: String,
        mime_type: String,
        base64: String,
    },
    /// Structured host event emitted by the automation program.
    Event {
        value: Value,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AutomationRunResult {
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AutomationRunError>,
    /// JSON-compatible final value of the automation program.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AutomationRunError {
    pub name: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub causes: Vec<AutomationRunError>,
}
