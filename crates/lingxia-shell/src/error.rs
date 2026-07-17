use thiserror::Error;

pub type ShellResult<T> = Result<T, ShellError>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ShellError {
    #[error("shell activator id must not be empty")]
    EmptyActivatorId,
    #[error("shell activator target must not be empty")]
    EmptyActivatorTarget,
    #[error("shell activator field '{field}' must not be empty")]
    EmptyActivatorField { field: &'static str },
    #[error("action activator '{id}' requires both label and icon")]
    IncompleteAction { id: String },
    #[error("duplicate shell activator id '{id}'")]
    DuplicateActivatorId { id: String },
    #[error("shell activator '{id}' was not found")]
    ActivatorNotFound { id: String },
    #[error("shell activator update for '{id}' is empty")]
    EmptyActivatorUpdate { id: String },
    #[error("shell native capability '{capability}' is not available")]
    UnsupportedCapability { capability: String },
    #[error("shell runtime is not initialized")]
    NotInitialized,
    #[error("shell host operation failed: {0}")]
    Host(String),
    #[error("shell activation '{id}' is disabled")]
    ActivatorDisabled { id: String },
    #[error("shell state changed concurrently (expected generation {expected}, found {actual})")]
    ConcurrentMutation { expected: u64, actual: u64 },
    #[error("shell Pins changed concurrently")]
    ConcurrentPinMutation,
    #[error("shell Pin limit reached ({max})")]
    LimitReached { max: usize },
    #[error("unsupported shell state version {version}")]
    UnsupportedVersion { version: u32 },
    #[error("invalid shell state: {0}")]
    InvalidState(String),
    #[error("shell state I/O failed: {0}")]
    Io(String),
}

impl From<std::io::Error> for ShellError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<serde_json::Error> for ShellError {
    fn from(value: serde_json::Error) -> Self {
        Self::InvalidState(value.to_string())
    }
}
