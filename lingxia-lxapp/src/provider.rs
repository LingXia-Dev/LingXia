//! Provider traits and registration.

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
}

/// Error type for provider operations.
#[derive(Debug, Clone)]
pub struct ProviderError(pub String);

impl ProviderError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }

    pub fn message(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ProviderError {}

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
    /// Returns None if fingerprint is not available.
    fn get_fingerprint(&self) -> Option<String> {
        None
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
