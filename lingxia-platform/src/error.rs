use thiserror::Error;

/// Platform-specific error types
#[derive(Error, Debug)]
pub enum PlatformError {
    #[error("Platform error: {0}")]
    Platform(String),

    #[error("Asset not found: {0}")]
    AssetNotFound(String),
}

/// Result type for platform operations
pub type PlatformResult<T> = Result<T, PlatformError>;
