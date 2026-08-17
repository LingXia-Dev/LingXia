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
        self.code().exit_code()
    }
}

impl ErrorCode {
    /// Every code, in exit-status order. Anything that needs the whole
    /// vocabulary — a decoder, a doc table, a consumer that mirrors it —
    /// iterates this rather than writing the list out again.
    pub const ALL: [ErrorCode; 9] = [
        ErrorCode::Usage,
        ErrorCode::NotFound,
        ErrorCode::Ambiguous,
        ErrorCode::Timeout,
        ErrorCode::Permission,
        ErrorCode::Unsupported,
        ErrorCode::Unavailable,
        ErrorCode::Stale,
        ErrorCode::Failed,
    ];

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

    /// Process exit status for this code. The slug and the status are one
    /// contract, so they are decided in one place.
    pub const fn exit_code(self) -> i32 {
        match self {
            ErrorCode::Usage => 2,
            ErrorCode::NotFound => 3,
            ErrorCode::Ambiguous => 4,
            ErrorCode::Timeout => 5,
            ErrorCode::Permission => 6,
            ErrorCode::Unsupported => 7,
            ErrorCode::Unavailable => 8,
            ErrorCode::Stale => 9,
            ErrorCode::Failed => 10,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        ErrorCode::ALL
            .into_iter()
            .find(|code| code.as_str() == value)
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

    /// `ALL` is what every consumer iterates instead of retyping the list, so
    /// a code added to the enum and forgotten here would silently narrow the
    /// vocabulary they see rather than failing to compile.
    #[test]
    fn all_covers_every_code() {
        for code in ErrorCode::ALL {
            assert_eq!(
                ErrorCode::parse(code.as_str()),
                Some(code),
                "{} must round-trip through parse",
                code.as_str()
            );
            assert_eq!(Error::from_code(code, "").code(), code);
        }
        // A code missing from ALL cannot round-trip, and the exit statuses are
        // consecutive, so their sum pins both the membership and the mapping.
        assert_eq!(
            ErrorCode::ALL.map(ErrorCode::exit_code),
            [2, 3, 4, 5, 6, 7, 8, 9, 10]
        );
    }
}
