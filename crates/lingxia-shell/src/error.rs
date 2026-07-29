use thiserror::Error;

pub type ShellResult<T> = Result<T, ShellError>;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ShellError {
    #[error("shell sidebar action id must not be empty")]
    EmptySidebarActionId,
    #[error("shell sidebar action field '{field}' must not be empty")]
    EmptySidebarActionField { field: &'static str },
    #[error("duplicate shell sidebar action id '{id}'")]
    DuplicateSidebarActionId { id: String },
    #[error("shell sidebar action '{id}' was not found")]
    SidebarActionNotFound { id: String },
    #[error("shell sidebar action update for '{id}' is empty")]
    EmptySidebarActionUpdate { id: String },
    #[error("shell sidebar action header accepts at most {max} items")]
    SidebarActionHeaderLimit { max: usize },
    #[error("shell sidebar action icon '{icon}' must be an lxapp-accessible local resource path")]
    InvalidSidebarActionIcon { icon: String },
    #[error("shell runtime is not initialized")]
    NotInitialized,
    #[error("shell host operation failed: {0}")]
    Host(String),
    #[error("shell sidebar action '{id}' is disabled")]
    SidebarActionDisabled { id: String },
    #[error("stale shell sidebar action generation {generation}; current generation is {current}")]
    StaleSidebarActionIntent { generation: u64, current: u64 },
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
