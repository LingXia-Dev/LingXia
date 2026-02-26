use thiserror::Error;

/// Platform-specific error types
#[derive(Error, Debug)]
pub enum PlatformError {
    #[error("Platform error: {0}")]
    Platform(String),

    #[error("Not supported: {0}")]
    NotSupported(String),

    #[error("Asset not found: {0}")]
    AssetNotFound(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}

/// Result type for platform operations
pub type PlatformResult<T> = Result<T, PlatformError>;

#[cfg(target_os = "android")]
impl From<jni::errors::Error> for PlatformError {
    fn from(value: jni::errors::Error) -> Self {
        PlatformError::Platform(format!("JNI error: {}", value))
    }
}
