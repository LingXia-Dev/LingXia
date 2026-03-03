//! Provider traits and registration.

use crate::error::LxAppError;
use crate::lxapp::ReleaseType;
use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;

/// Boxed future type for dyn compatibility.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Update query target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateTarget {
    /// Host application.
    App { current_version: Option<String> },
    /// Miniapp.
    LxApp {
        id: String,
        release_type: ReleaseType,
        current_version: Option<String>,
    },
    /// Plugin extension (specific version).
    Plugin { id: String, version: String },
}

/// Update package metadata.
#[derive(Clone, Debug)]
pub struct UpdatePackageInfo {
    pub version: String,
    pub url: String,
    pub checksum_sha256: String,
    pub size: Option<u64>,
    pub release_notes: Option<Vec<String>>,
    pub is_force_update: bool,
    /// Required SDK/runtime version from update metadata (semantic version string, e.g. "0.3.1").
    /// When present, update must be rejected if current runtime version is lower.
    pub required_runtime_version: Option<String>,
}

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

    pub fn to_lxapp_error(&self) -> LxAppError {
        let biz_code = self.biz_code();
        let detail = self.detail().to_string();
        LxAppError::RongJSHost {
            code: biz_code.to_string(),
            message: detail.clone(),
            data: Some(serde_json::json!({
                "bizCode": biz_code,
                "providerCode": self.code().as_str(),
                "detail": detail,
            })),
        }
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

/// Trait for update checking.
pub trait UpdateProvider: Send + Sync + 'static {
    /// Returns Some(package) when available, None when already up to date.
    /// For App/LxApp, current_version=None requests the latest package.
    /// For Plugin, version is required and targets a specific package.
    fn check_update<'a>(
        &'a self,
        target: UpdateTarget,
    ) -> BoxFuture<'a, Result<Option<UpdatePackageInfo>, ProviderError>>;
}

/// Trait for device fingerprint.
pub trait FingerprintProvider: Send + Sync + 'static {
    /// Get the device fingerprint ID.
    fn get_fingerprint(&self) -> Result<String, FingerprintError> {
        Err(FingerprintError::DeviceIdUnavailable)
    }
}

/// Combined provider trait.
/// Implementations must satisfy all component traits.
pub trait Provider: UpdateProvider + FingerprintProvider {}

// Blanket implementation: any type implementing all sub-traits is a Provider
impl<T: UpdateProvider + FingerprintProvider> Provider for T {}

/// Default provider with no-op implementations.
pub struct NoOpProvider;

impl UpdateProvider for NoOpProvider {
    fn check_update<'a>(
        &'a self,
        _target: UpdateTarget,
    ) -> BoxFuture<'a, Result<Option<UpdatePackageInfo>, ProviderError>> {
        Box::pin(async { Ok(None) })
    }
}

impl FingerprintProvider for NoOpProvider {}

static PROVIDER: OnceLock<Box<dyn Provider>> = OnceLock::new();

/// Register a provider. Must be called at app startup before SDK initialization.
pub fn register_provider(provider: Box<dyn Provider>) {
    if PROVIDER.set(provider).is_err() {
        panic!("register_provider called more than once");
    }
}

/// Get the registered provider, or a default no-op provider.
pub(crate) fn get_provider() -> &'static dyn Provider {
    PROVIDER.get().map(|b| b.as_ref()).unwrap_or(&NoOpProvider)
}
