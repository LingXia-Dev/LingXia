//! Wire types for the `session.test.*` devtool handlers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestStartArgs {
    pub source: String,
    pub source_name: Option<String>,
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub args: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestStartResponse {
    pub run_id: String,
    pub state: TestRunState,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestPollArgs {
    pub run_id: String,
    #[serde(default)]
    pub after_seq: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestPollResponse {
    pub run_id: String,
    pub state: TestRunState,
    pub next_seq: u64,
    pub events: Vec<TestEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<TestRunResult>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestCancelArgs {
    pub run_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestCancelResponse {
    pub run_id: String,
    pub state: TestRunState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TestRunState {
    Running,
    Passed,
    Failed,
    TimedOut,
    Cancelled,
    InternalError,
}

impl TestRunState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
            Self::Cancelled => "cancelled",
            Self::InternalError => "internal_error",
        }
    }

    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Running)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestEvent {
    pub seq: u64,
    #[serde(flatten)]
    pub payload: TestEventPayload,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TestEventPayload {
    Console {
        level: String,
        message: String,
    },
    Artifact {
        name: String,
        mime_type: String,
        base64: String,
    },
    CaseStarted {
        name: String,
        full_name: String,
    },
    CaseFinished {
        name: String,
        full_name: String,
        status: TestCaseStatus,
        duration_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<TestRunError>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestRunResult {
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TestRunError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<TestReport>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestRunError {
    pub name: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub causes: Vec<TestRunError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TestCaseStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestCaseResult {
    pub name: String,
    pub full_name: String,
    pub status: TestCaseStatus,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TestRunError>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestReport {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration_ms: u64,
    pub cases: Vec<TestCaseResult>,
}
