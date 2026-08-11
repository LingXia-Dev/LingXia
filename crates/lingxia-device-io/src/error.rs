//! The single error taxonomy shared by every `desktop` command surface. Each
//! variant maps to the stable command exit-code contract, so CLI, transport,
//! and in-process JS callers branch on the same codes.

use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// CLI usage / invalid argument (exit 2).
    #[error("{0}")]
    Usage(String),
    /// Target not found / no match (exit 3).
    #[error("{0}")]
    NotFound(String),
    /// Ambiguous match (exit 4).
    #[error("{0}")]
    Ambiguous(String),
    /// Timed out (exit 5).
    #[error("{0}")]
    Timeout(String),
    /// Permission or privilege denied (exit 6).
    #[error("{0}")]
    Permission(String),
    /// Unsupported capability or backend (exit 7).
    #[error("{0}")]
    Unsupported(String),
    /// Required backend/display/app unavailable (exit 8).
    #[error("{0}")]
    Unavailable(String),
    /// Stale target handle, e.g. an expired window id (exit 9).
    #[error("{0}")]
    Stale(String),
    /// Operation failed after the target was resolved (exit 10).
    #[error("{0}")]
    Failed(String),
}

/// Stable, machine-readable slug for the `--json` error envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Usage,
    NotFound,
    Ambiguous,
    Timeout,
    Permission,
    Unsupported,
    Unavailable,
    Stale,
    Failed,
}

impl Error {
    pub fn code(&self) -> ErrorCode {
        match self {
            Error::Usage(_) => ErrorCode::Usage,
            Error::NotFound(_) => ErrorCode::NotFound,
            Error::Ambiguous(_) => ErrorCode::Ambiguous,
            Error::Timeout(_) => ErrorCode::Timeout,
            Error::Permission(_) => ErrorCode::Permission,
            Error::Unsupported(_) => ErrorCode::Unsupported,
            Error::Unavailable(_) => ErrorCode::Unavailable,
            Error::Stale(_) => ErrorCode::Stale,
            Error::Failed(_) => ErrorCode::Failed,
        }
    }

    /// Rebuild an error carried across a transport. The code is what callers
    /// branch on and what becomes the process exit status, so it has to
    /// survive the trip — a message alone would collapse every failure into
    /// one.
    pub fn from_code(code: ErrorCode, message: impl Into<String>) -> Self {
        let message = message.into();
        match code {
            ErrorCode::Usage => Error::Usage(message),
            ErrorCode::NotFound => Error::NotFound(message),
            ErrorCode::Ambiguous => Error::Ambiguous(message),
            ErrorCode::Timeout => Error::Timeout(message),
            ErrorCode::Permission => Error::Permission(message),
            ErrorCode::Unsupported => Error::Unsupported(message),
            ErrorCode::Unavailable => Error::Unavailable(message),
            ErrorCode::Stale => Error::Stale(message),
            ErrorCode::Failed => Error::Failed(message),
        }
    }

    /// Process exit code per the command contract.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Usage(_) => 2,
            Error::NotFound(_) => 3,
            Error::Ambiguous(_) => 4,
            Error::Timeout(_) => 5,
            Error::Permission(_) => 6,
            Error::Unsupported(_) => 7,
            Error::Unavailable(_) => 8,
            Error::Stale(_) => 9,
            Error::Failed(_) => 10,
        }
    }
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::Usage => "usage",
            ErrorCode::NotFound => "not_found",
            ErrorCode::Ambiguous => "ambiguous",
            ErrorCode::Timeout => "timeout",
            ErrorCode::Permission => "permission",
            ErrorCode::Unsupported => "unsupported",
            ErrorCode::Unavailable => "unavailable",
            ErrorCode::Stale => "stale",
            ErrorCode::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "usage" => ErrorCode::Usage,
            "not_found" => ErrorCode::NotFound,
            "ambiguous" => ErrorCode::Ambiguous,
            "timeout" => ErrorCode::Timeout,
            "permission" => ErrorCode::Permission,
            "unsupported" => ErrorCode::Unsupported,
            "unavailable" => ErrorCode::Unavailable,
            "stale" => ErrorCode::Stale,
            "failed" => ErrorCode::Failed,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_match_contract() {
        assert_eq!(Error::Usage("".into()).exit_code(), 2);
        assert_eq!(Error::NotFound("".into()).exit_code(), 3);
        assert_eq!(Error::Ambiguous("".into()).exit_code(), 4);
        assert_eq!(Error::Timeout("".into()).exit_code(), 5);
        assert_eq!(Error::Permission("".into()).exit_code(), 6);
        assert_eq!(Error::Unsupported("".into()).exit_code(), 7);
        assert_eq!(Error::Unavailable("".into()).exit_code(), 8);
        assert_eq!(Error::Stale("".into()).exit_code(), 9);
        assert_eq!(Error::Failed("".into()).exit_code(), 10);
    }
}
