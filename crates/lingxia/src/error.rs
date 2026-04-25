use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("platform error: {0}")]
    Platform(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl Error {
    pub fn invalid_request(detail: impl Into<String>) -> Self {
        Self::InvalidRequest(detail.into())
    }

    pub fn permission_denied(detail: impl Into<String>) -> Self {
        Self::PermissionDenied(detail.into())
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::NotFound(detail.into())
    }

    pub fn platform(detail: impl Into<String>) -> Self {
        Self::Platform(detail.into())
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::Internal(detail.into())
    }
}

impl From<Error> for lxapp::LxAppError {
    fn from(error: Error) -> Self {
        match error {
            Error::InvalidRequest(detail) => lxapp::LxAppError::InvalidParameter(detail),
            Error::PermissionDenied(detail) => {
                lxapp::LxAppError::UnsupportedOperation(format!("permission denied: {detail}"))
            }
            Error::NotFound(detail) => lxapp::LxAppError::ResourceNotFound(detail),
            Error::Io(error) => lxapp::LxAppError::IoError(error.to_string()),
            Error::Platform(detail) | Error::Internal(detail) => lxapp::LxAppError::Runtime(detail),
        }
    }
}

impl From<lxapp::LxAppError> for Error {
    fn from(error: lxapp::LxAppError) -> Self {
        match error {
            lxapp::LxAppError::ResourceNotFound(detail) => Self::NotFound(detail),
            lxapp::LxAppError::InvalidJsonFile(detail)
            | lxapp::LxAppError::InvalidParameter(detail) => Self::InvalidRequest(detail),
            lxapp::LxAppError::UnsupportedOperation(detail) => Self::InvalidRequest(detail),
            lxapp::LxAppError::IoError(detail) => Self::Internal(detail),
            lxapp::LxAppError::WebView(detail)
            | lxapp::LxAppError::Runtime(detail)
            | lxapp::LxAppError::ChannelError(detail)
            | lxapp::LxAppError::ResourceExhausted(detail)
            | lxapp::LxAppError::Bridge(detail)
            | lxapp::LxAppError::RongJS(detail)
            | lxapp::LxAppError::PluginNotConfigured(detail)
            | lxapp::LxAppError::PluginDownloadFailed(detail) => Self::Internal(detail),
            lxapp::LxAppError::RongJSHost { code, message, .. } => {
                Self::Internal(format!("{code}: {message}"))
            }
        }
    }
}

impl From<lingxia_platform::PlatformError> for Error {
    fn from(error: lingxia_platform::PlatformError) -> Self {
        match error {
            lingxia_platform::PlatformError::AssetNotFound(detail) => Self::NotFound(detail),
            lingxia_platform::PlatformError::InvalidParameter(detail) => {
                Self::InvalidRequest(detail)
            }
            lingxia_platform::PlatformError::NotSupported(detail) => Self::InvalidRequest(detail),
            lingxia_platform::PlatformError::Platform(detail) => Self::Platform(detail),
            lingxia_platform::PlatformError::BusinessError(code) => {
                Self::Platform(format!("business error: code {code}"))
            }
            lingxia_platform::PlatformError::CallbackDropped => {
                Self::Internal("platform callback dropped".to_string())
            }
        }
    }
}

impl From<lingxia_provider::ProviderError> for Error {
    fn from(error: lingxia_provider::ProviderError) -> Self {
        match error.code() {
            lingxia_provider::ProviderErrorCode::InvalidRequest => {
                Self::InvalidRequest(error.detail().to_string())
            }
            lingxia_provider::ProviderErrorCode::NotFound => {
                Self::NotFound(error.detail().to_string())
            }
            lingxia_provider::ProviderErrorCode::PermissionDenied => {
                Self::PermissionDenied(error.detail().to_string())
            }
            lingxia_provider::ProviderErrorCode::Network
            | lingxia_provider::ProviderErrorCode::Timeout
            | lingxia_provider::ProviderErrorCode::Server
            | lingxia_provider::ProviderErrorCode::Internal => Self::Internal(error.to_string()),
        }
    }
}

impl From<lingxia_update::UpdateError> for Error {
    fn from(error: lingxia_update::UpdateError) -> Self {
        match error {
            lingxia_update::UpdateError::InvalidParameter(detail) => Self::InvalidRequest(detail),
            lingxia_update::UpdateError::UnsupportedOperation(detail) => {
                Self::InvalidRequest(detail)
            }
            lingxia_update::UpdateError::ResourceNotFound(detail) => Self::NotFound(detail),
            lingxia_update::UpdateError::Io(detail)
            | lingxia_update::UpdateError::Runtime(detail) => Self::Internal(detail),
        }
    }
}

impl From<lingxia_service::downloads::DownloadsError> for Error {
    fn from(error: lingxia_service::downloads::DownloadsError) -> Self {
        match error {
            lingxia_service::downloads::DownloadsError::InvalidParameter(detail) => {
                Self::InvalidRequest(detail)
            }
            lingxia_service::downloads::DownloadsError::ResourceNotFound(detail) => {
                Self::NotFound(detail)
            }
            lingxia_service::downloads::DownloadsError::UnsupportedOperation(detail)
            | lingxia_service::downloads::DownloadsError::Runtime(detail) => Self::Internal(detail),
            lingxia_service::downloads::DownloadsError::Io(error) => Self::Io(error),
            lingxia_service::downloads::DownloadsError::Json(error) => {
                Self::Internal(error.to_string())
            }
            lingxia_service::downloads::DownloadsError::Settings(error) => error.into(),
        }
    }
}

impl From<lingxia_service::settings::SettingsError> for Error {
    fn from(error: lingxia_service::settings::SettingsError) -> Self {
        match error {
            lingxia_service::settings::SettingsError::Io(error) => Self::Io(error),
            lingxia_service::settings::SettingsError::Json(error) => {
                Self::Internal(error.to_string())
            }
        }
    }
}

impl From<tokio::task::JoinError> for Error {
    fn from(error: tokio::task::JoinError) -> Self {
        Self::Internal(format!("task failed: {error}"))
    }
}
