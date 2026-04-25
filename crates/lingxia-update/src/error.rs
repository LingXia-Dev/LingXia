use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum UpdateError {
    #[error("invalid parameter: {0}")]
    InvalidParameter(String),
    #[error("unsupported operation: {0}")]
    UnsupportedOperation(String),
    #[error("resource not found: {0}")]
    ResourceNotFound(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("runtime error: {0}")]
    Runtime(String),
}

impl UpdateError {
    pub fn invalid_parameter(detail: impl Into<String>) -> Self {
        Self::InvalidParameter(detail.into())
    }

    pub fn unsupported(detail: impl Into<String>) -> Self {
        Self::UnsupportedOperation(detail.into())
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::ResourceNotFound(detail.into())
    }

    pub fn io(detail: impl Into<String>) -> Self {
        Self::Io(detail.into())
    }

    pub fn runtime(detail: impl Into<String>) -> Self {
        Self::Runtime(detail.into())
    }
}

impl From<std::io::Error> for UpdateError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<lingxia_provider::ProviderError> for UpdateError {
    fn from(error: lingxia_provider::ProviderError) -> Self {
        match error.code() {
            lingxia_provider::ProviderErrorCode::InvalidRequest => {
                Self::InvalidParameter(error.detail().to_string())
            }
            lingxia_provider::ProviderErrorCode::NotFound => {
                Self::ResourceNotFound(error.detail().to_string())
            }
            lingxia_provider::ProviderErrorCode::PermissionDenied => {
                Self::UnsupportedOperation(error.detail().to_string())
            }
            lingxia_provider::ProviderErrorCode::Network
            | lingxia_provider::ProviderErrorCode::Timeout
            | lingxia_provider::ProviderErrorCode::Server
            | lingxia_provider::ProviderErrorCode::Internal => Self::Runtime(error.to_string()),
        }
    }
}
