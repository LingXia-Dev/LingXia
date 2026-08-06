//! Versioned per-session restore state.
//!
//! The crate owns the contract: what a restorable session contains
//! (cwd, title, an opaque host profile reference, and byte-bounded
//! plain-text scrollback) and how it is validated. Crash-safe file
//! persistence and tab/pane layout remain host responsibilities.
//!
//! Restored scrollback is plain text only — no VT modes, colors, or
//! alternate-screen state survive, and a fresh shell always starts
//! from a clean emulator state.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Current restore-state schema version.
pub const TERMINAL_RESTORE_VERSION: u32 = 1;

/// Default total byte budget for exported scrollback text.
pub const DEFAULT_RESTORE_SCROLLBACK_LIMIT: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalRestoreState {
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Opaque host profile reference; the crate only round-trips it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    /// Plain text lines, oldest first, clipped from the oldest side
    /// when the byte budget ran out.
    pub scrollback: Vec<String>,
    /// True when oldest lines were dropped to fit the byte budget.
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalRestoreError {
    /// The state was written by an incompatible schema version.
    UnknownVersion(u32),
    /// The state could not be parsed at all.
    Invalid(String),
}

impl std::fmt::Display for TerminalRestoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownVersion(version) => {
                write!(f, "unsupported restore state version {version}")
            }
            Self::Invalid(reason) => write!(f, "invalid restore state: {reason}"),
        }
    }
}

impl std::error::Error for TerminalRestoreError {}

impl TerminalRestoreState {
    /// Validate the schema version. Unknown versions fail loudly so a
    /// newer file is never silently misread.
    pub fn validate(&self) -> Result<(), TerminalRestoreError> {
        if self.version != TERMINAL_RESTORE_VERSION {
            return Err(TerminalRestoreError::UnknownVersion(self.version));
        }
        Ok(())
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn from_json(json: &str) -> Result<Self, TerminalRestoreError> {
        let state: Self = serde_json::from_str(json)
            .map_err(|err| TerminalRestoreError::Invalid(err.to_string()))?;
        state.validate()?;
        Ok(state)
    }
}

/// Clip logical lines (oldest first) to a byte budget, keeping the
/// newest lines. Returns (lines, truncated).
pub fn clip_scrollback(lines: Vec<String>, max_bytes: usize) -> (Vec<String>, bool) {
    let mut kept: Vec<String> = Vec::new();
    let mut bytes = 0_usize;
    let mut truncated = false;
    for line in lines.into_iter().rev() {
        let cost = line.len().saturating_add(2);
        if !kept.is_empty() && bytes.saturating_add(cost) > max_bytes {
            truncated = true;
            break;
        }
        if cost > max_bytes && kept.is_empty() {
            // A single oversized newest line is clipped hard.
            let clipped: String = line.chars().take(max_bytes).collect();
            kept.push(clipped);
            truncated = true;
            break;
        }
        bytes += cost;
        kept.push(line);
    }
    kept.reverse();
    (kept, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip_and_version_gate() {
        let state = TerminalRestoreState {
            version: TERMINAL_RESTORE_VERSION,
            cwd: Some(PathBuf::from("/tmp/work")),
            title: Some("shell".to_string()),
            profile_id: Some("default".to_string()),
            scrollback: vec!["one".to_string(), "two".to_string()],
            truncated: false,
        };
        let parsed = TerminalRestoreState::from_json(&state.to_json()).expect("round trip");
        assert_eq!(parsed, state);

        let future = state.to_json().replace("\"version\":1", "\"version\":99");
        assert_eq!(
            TerminalRestoreState::from_json(&future),
            Err(TerminalRestoreError::UnknownVersion(99))
        );
        assert!(matches!(
            TerminalRestoreState::from_json("not json"),
            Err(TerminalRestoreError::Invalid(_))
        ));
    }

    #[test]
    fn clip_keeps_newest_lines_within_budget() {
        let lines: Vec<String> = (0..10).map(|i| format!("line{i}")).collect();
        let (kept, truncated) = clip_scrollback(lines, 8 + 8); // ~2 lines of 6+2 bytes
        assert!(truncated);
        assert_eq!(kept, vec!["line8".to_string(), "line9".to_string()]);

        let (all, truncated) = clip_scrollback(vec!["a".to_string(), "b".to_string()], 1024);
        assert!(!truncated);
        assert_eq!(all, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn clip_hard_limits_a_single_oversized_line() {
        let (kept, truncated) = clip_scrollback(vec!["x".repeat(100)], 10);
        assert!(truncated);
        assert_eq!(kept[0].len(), 10);
    }
}
