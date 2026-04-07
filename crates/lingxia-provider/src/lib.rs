use std::future::Future;
use std::pin::Pin;

/// Boxed future type for dyn compatibility.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Error type for provider operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderErrorCode {
    InvalidRequest,
    NotFound,
    Network,
    Timeout,
    Server,
    PermissionDenied,
    Internal,
}

impl ProviderErrorCode {
    pub const fn biz_code(self) -> u32 {
        match self {
            Self::InvalidRequest => 1002,
            Self::NotFound => 1003,
            Self::Network => 5001,
            Self::Timeout => 5002,
            Self::Server => 5003,
            Self::PermissionDenied => 3000,
            Self::Internal => 1005,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::NotFound => "not_found",
            Self::Network => "network",
            Self::Timeout => "timeout",
            Self::Server => "server",
            Self::PermissionDenied => "permission_denied",
            Self::Internal => "internal",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderError {
    code: ProviderErrorCode,
    detail: String,
}

impl ProviderError {
    pub fn new(code: ProviderErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    pub fn invalid_request(detail: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::InvalidRequest, detail)
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::NotFound, detail)
    }

    pub fn network(detail: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::Network, detail)
    }

    pub fn timeout(detail: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::Timeout, detail)
    }

    pub fn server(detail: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::Server, detail)
    }

    pub fn permission_denied(detail: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::PermissionDenied, detail)
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(ProviderErrorCode::Internal, detail)
    }

    pub const fn code(&self) -> ProviderErrorCode {
        self.code
    }

    pub const fn biz_code(&self) -> u32 {
        self.code.biz_code()
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code.as_str(), self.detail)
    }
}

impl std::error::Error for ProviderError {}

/// Error type for fingerprint operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FingerprintError {
    /// Device ID cannot be loaded/generated on current runtime.
    DeviceIdUnavailable,
}

impl std::fmt::Display for FingerprintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeviceIdUnavailable => write!(f, "device_id_unavailable"),
        }
    }
}

impl std::error::Error for FingerprintError {}

/// Trait for device fingerprint.
pub trait FingerprintProvider: Send + Sync + 'static {
    /// Get the device fingerprint ID.
    fn get_fingerprint(&self) -> Result<String, FingerprintError> {
        Err(FingerprintError::DeviceIdUnavailable)
    }
}

/// Trait for push token binding.
pub trait PushNotificationProvider: Send + Sync + 'static {
    /// Bind push token to cloud side.
    fn bind_push_token<'a>(&'a self, _token: String) -> BoxFuture<'a, Result<(), ProviderError>> {
        Box::pin(async { Ok(()) })
    }
}
