use rong::RongJSError;
use std::io;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum MiniAppError {
    /// Error when performing web operations
    #[error("WebView error: {0}")]
    WebView(String),

    #[error("{0} not found")]
    ResourceNotFound(String),

    #[error("{0} is not valid JSON file")]
    InvalidJsonFile(String),

    /// Error for invalid parameters
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// Error for unsupported operations
    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    /// Error for I/O operations (file access, network, etc.)
    #[error("I/O error: {0}")]
    IoError(String),

    /// Error for runtime operations
    #[error("Runtime error: {0}")]
    Runtime(String),

    /// Error channel operations
    #[error("Channel error: {0}")]
    ChannelError(String),

    /// Error when resource is exhausted
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    /// Error when bridge error
    #[error("Bridge error: {0}")]
    Bridge(String),

    /// Error for Rong runtime
    #[error("Rong Error: {0}")]
    RongJS(String),
}

impl From<io::Error> for MiniAppError {
    fn from(error: io::Error) -> Self {
        MiniAppError::IoError(error.to_string())
    }
}

impl<T> From<std::sync::mpsc::SendError<T>> for MiniAppError {
    fn from(error: std::sync::mpsc::SendError<T>) -> Self {
        MiniAppError::ChannelError(error.to_string())
    }
}

impl From<serde_json::Error> for MiniAppError {
    fn from(error: serde_json::Error) -> Self {
        MiniAppError::Bridge(format!("JSON Processing Error: {}", error))
    }
}

impl From<RongJSError> for MiniAppError {
    fn from(error: RongJSError) -> Self {
        MiniAppError::RongJS(error.to_string())
    }
}
