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
}
