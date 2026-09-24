//! Wire types for the `session.test.*` development-session methods.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestStartArgs {
    pub source: String,
    pub source_name: Option<String>,
    pub timeout_ms: Option<u64>,
    /// User `--arg`/`--secret-arg` values: the spec's `t.args`.
    #[serde(default)]
    pub args: HashMap<String, String>,
    /// lxdev's run controls (grep, ids, shard, retries, …), kept apart from
    /// `args` so a user arg can never steer the run. A host that predates the
    /// field ignores it.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub control: HashMap<String, String>,
    /// Run the target lxapp on an isolated data profile. A host that
    /// predates the field ignores it, so a client must first confirm
    /// [`TestCapabilities::profile`] rather than run un-isolated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<TestProfileArgs>,
}

/// `TestStartArgs.profile`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct TestProfileArgs {
    /// Start the app on an empty profile (or on `seed_state_id`).
    #[serde(default)]
    pub isolate: bool,
    /// A snapshot staged with `session.profile.upload`, unpacked into the
    /// profile before the app opens on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_state_id: Option<String>,
    /// Keep the profile after the run for `session.profile.export`.
    #[serde(default)]
    pub retain: bool,
    /// The lxapp to isolate; the session's home lxapp when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appid: Option<String>,
}

/// `session.test.capabilities`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TestCapabilities {
    /// Present when the host can run a test on an isolated profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<ProfileCapability>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProfileCapability {
    /// Largest snapshot the host accepts or produces, in bytes.
    pub max_state_bytes: u64,
    /// Largest decoded chunk of one upload or export call, in bytes.
    pub chunk_bytes: u64,
}

/// Largest decoded chunk of one `session.profile.*` transfer.
pub const PROFILE_CHUNK_BYTES: usize = 4 * 1024 * 1024;

/// `session.profile.upload`: append `base64` at byte `offset` of staged
/// upload `upload_id`; `done` finishes it.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProfileUploadArgs {
    pub upload_id: String,
    pub offset: u64,
    pub base64: String,
    #[serde(default)]
    pub done: bool,
    /// SHA-256 (hex) of the whole snapshot, checked on `done`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProfileUploadResponse {
    pub upload_id: String,
    /// Bytes staged so far.
    pub received: u64,
    /// Set once `done`: pass it as `TestProfileArgs.seed_state_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_id: Option<String>,
}

/// `session.profile.export`: the bytes of run `run_id`'s retained profile
/// from `offset`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProfileExportArgs {
    pub run_id: String,
    #[serde(default)]
    pub offset: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProfileExportResponse {
    pub base64: String,
    pub total: u64,
    pub done: bool,
    /// SHA-256 (hex) of the whole snapshot.
    pub sha256: String,
}

/// `session.profile.discard`: exactly one of the two.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ProfileDiscardArgs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProfileDiscardResponse {
    pub discarded: bool,
}

/// Error code of a `session.test.start` refused because another run holds the
/// session; the error's `data` is a [`TestActiveRun`].
pub const RUN_IN_PROGRESS: &str = "automation_run_in_progress";

/// The run currently holding the session's automation slot.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestActiveRun {
    pub run_id: String,
    /// Time since the run started.
    pub age_ms: u64,
    /// Time since a client last polled it; `None` when nobody ever has.
    pub since_last_poll_ms: Option<u64>,
}

/// `session.test.active`: `run` is `None` when the session is free.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TestActiveResponse {
    pub run: Option<TestActiveRun>,
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
    RunStarted {
        total: usize,
        #[serde(default)]
        cases: Vec<serde_json::Value>,
        /// The run's args as reports show them (secrets masked), when the
        /// framework says.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        args: Option<HashMap<String, String>>,
    },
    Diagnostic {
        phase: String,
        message: String,
    },
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
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        file: Option<String>,
        #[serde(default)]
        line: Option<u64>,
        name: String,
        full_name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
        #[serde(default)]
        watchdog_timeout_ms: Option<u64>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        covers: Vec<String>,
    },
    CaseFinished {
        #[serde(default)]
        record: Option<serde_json::Value>,
        name: String,
        full_name: String,
        status: TestCaseStatus,
        duration_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<TestRunError>,
    },
    StepStarted {
        name: String,
        path: String,
    },
    StepFinished {
        name: String,
        path: String,
        status: String,
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
    #[serde(flatten)]
    pub detail: serde_json::Map<String, serde_json::Value>,
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
    Timeout,
    Xfail,
    Xpass,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestCaseResult {
    #[serde(flatten)]
    pub detail: serde_json::Map<String, serde_json::Value>,
    pub name: String,
    pub full_name: String,
    pub status: TestCaseStatus,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TestRunError>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestReport {
    #[serde(default)]
    pub timeout: usize,
    #[serde(default)]
    pub xfail: usize,
    #[serde(default)]
    pub xpass: usize,
    #[serde(flatten)]
    pub detail: serde_json::Map<String, serde_json::Value>,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration_ms: u64,
    pub cases: Vec<TestCaseResult>,
}

impl TestCaseStatus {
    pub fn is_failure(self) -> bool {
        matches!(self, Self::Failed | Self::Timeout | Self::Xpass)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::Timeout => "timeout",
            Self::Xfail => "xfail",
            Self::Xpass => "xpass",
        }
    }
}

#[cfg(test)]
mod profile_wire_tests {
    use super::*;

    #[test]
    fn a_start_without_a_profile_keeps_its_old_shape() {
        let args = TestStartArgs {
            source: "x".into(),
            source_name: None,
            timeout_ms: None,
            args: HashMap::new(),
            control: HashMap::new(),
            profile: None,
        };
        let value = serde_json::to_value(&args).unwrap();
        assert!(value.get("profile").is_none());
        // An older client's start still parses.
        let parsed: TestStartArgs =
            serde_json::from_value(serde_json::json!({ "source": "x" })).unwrap();
        assert!(parsed.profile.is_none());
    }

    #[test]
    fn profile_args_round_trip_and_default() {
        let parsed: TestStartArgs = serde_json::from_value(serde_json::json!({
            "source": "x",
            "profile": { "isolate": true, "seed_state_id": "s1", "retain": true }
        }))
        .unwrap();
        assert_eq!(
            parsed.profile,
            Some(TestProfileArgs {
                isolate: true,
                seed_state_id: Some("s1".into()),
                retain: true,
                appid: None,
            })
        );
        let empty: TestProfileArgs = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(empty, TestProfileArgs::default());
    }

    #[test]
    fn capabilities_without_profile_mean_no_isolation() {
        let caps: TestCapabilities = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(caps.profile.is_none());
    }
}
