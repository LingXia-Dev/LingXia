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

/// Server-owned lifecycle state of an lxapp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LxAppStatus {
    /// The registry reported nothing for this app — an older server, an app it
    /// does not know, or a check that never reached it.
    #[default]
    Unknown,
    Published,
    /// No longer offered. An already-installed copy keeps working.
    Delisted,
    /// Blocked by the operator. Must not open, installed or not.
    Suspended,
}

impl LxAppStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Published => "published",
            Self::Delisted => "delisted",
            Self::Suspended => "suspended",
        }
    }

    /// Unrecognized values read as `Unknown` so a newer server cannot brick an
    /// older client by inventing a state it never blocks on.
    ///
    /// Case- and whitespace-insensitive: `Unknown` does not block, so a server
    /// sending `"Suspended"` against a case-sensitive match would degrade in
    /// the unsafe direction on the one field that gates opening.
    pub fn from_str_lossy(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "published" => Self::Published,
            "delisted" => Self::Delisted,
            "suspended" => Self::Suspended,
            _ => Self::Unknown,
        }
    }

    pub const fn blocks_open(self) -> bool {
        matches!(self, Self::Suspended)
    }
}

impl std::fmt::Display for LxAppStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The registry's record for one lxapp: the facts the server owns.
///
/// Deliberately carries nothing about a *package* — version, url, checksum,
/// `minRuntimeVersion` all belong to `UpdatePackageInfo` and travel the update
/// path. Server-owned facts in, package facts out; the two must never become
/// two answers to the same question.
#[derive(Debug, Clone, Default)]
pub struct LxAppRegistryInfo {
    pub appid: String,
    /// Already resolved for the requested locale.
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    /// Content hash of the icon `icon_url` points at. The client caches by this
    /// value, so an unchanged icon costs no request at all.
    pub icon_hash: Option<String>,
    pub status: LxAppStatus,
}

/// Lookup of registry records, separate from `UpdateProvider` on purpose: an
/// app's name, icon, and status change without any package changing, and the
/// update path is gated (OTA-managed only, deduped, force-update aware) in ways
/// that would silently strand them.
pub trait LxAppRegistryProvider: Send + Sync + 'static {
    /// Resolve `appids` for `locale`, in one batch.
    ///
    /// The locale fallback chain (`zh-Hant` → `zh-Hans` → `en`) is the server's
    /// to run: `name` and `description` come back resolved to one language, not
    /// as a map, so the chain has one implementation rather than one per client.
    ///
    /// Apps the registry does not know are omitted from the reply rather than
    /// returned as an error.
    fn fetch_registry_info<'a>(
        &'a self,
        _appids: &'a [String],
        _locale: &'a str,
    ) -> BoxFuture<'a, Result<Vec<LxAppRegistryInfo>, ProviderError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

#[cfg(test)]
mod registry_tests {
    use super::LxAppStatus;

    #[test]
    fn status_parsing_is_case_insensitive_because_unknown_never_blocks() {
        assert_eq!(
            LxAppStatus::from_str_lossy("suspended"),
            LxAppStatus::Suspended
        );
        assert_eq!(
            LxAppStatus::from_str_lossy("Suspended"),
            LxAppStatus::Suspended
        );
        assert_eq!(
            LxAppStatus::from_str_lossy(" SUSPENDED "),
            LxAppStatus::Suspended
        );
        assert!(LxAppStatus::from_str_lossy("SUSPENDED").blocks_open());

        assert_eq!(
            LxAppStatus::from_str_lossy("Delisted"),
            LxAppStatus::Delisted
        );
        assert_eq!(LxAppStatus::from_str_lossy(""), LxAppStatus::Unknown);
        assert_eq!(LxAppStatus::from_str_lossy("retired"), LxAppStatus::Unknown);
    }
}
