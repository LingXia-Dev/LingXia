//! Cloud update provider trait and registration.

use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;

/// Boxed future type for dyn compatibility.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Result of an update check.
#[derive(Clone, Debug, Default)]
pub struct UpdateCheckResult {
    pub has_update: bool,
    pub package: Option<UpdatePackageInfo>,
}

/// Update package information from cloud.
#[derive(Clone, Debug, Default)]
pub struct UpdatePackageInfo {
    pub version: String,
    pub url: String,
    pub checksum_sha256: String,
}

/// Error type for cloud provider operations.
#[derive(Debug, Clone)]
pub struct CloudError(pub String);

impl CloudError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }

    pub fn message(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CloudError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for CloudError {}

/// Trait for cloud update checking.
pub trait CloudUpdateProvider: Send + Sync + 'static {
    /// Check if an update is available for the given lxapp.
    fn check_update<'a>(
        &'a self,
        lxappid: &'a str,
        current_version: Option<&'a str>,
    ) -> BoxFuture<'a, Result<UpdateCheckResult, CloudError>>;
}

/// Combined cloud provider trait using trait bounds.
/// Implementations must satisfy all component traits.
pub trait CloudProvider: CloudUpdateProvider {}

// Blanket implementation: any type implementing CloudUpdateProvider is a CloudProvider
impl<T: CloudUpdateProvider> CloudProvider for T {}

/// Default provider that returns no-update for all requests.
pub struct NoOpCloudProvider;

impl CloudUpdateProvider for NoOpCloudProvider {
    fn check_update<'a>(
        &'a self,
        _lxappid: &'a str,
        _current_version: Option<&'a str>,
    ) -> BoxFuture<'a, Result<UpdateCheckResult, CloudError>> {
        Box::pin(async { Ok(UpdateCheckResult::default()) })
    }
}

static CLOUD_PROVIDER: OnceLock<Box<dyn CloudProvider>> = OnceLock::new();

/// Register a cloud provider. Must be called at app startup before SDK initialization.
pub fn register_cloud_provider(provider: Box<dyn CloudProvider>) {
    if CLOUD_PROVIDER.set(provider).is_err() {
        panic!("register_cloud_provider called more than once");
    }
}

/// Get the registered cloud provider, or a default no-op provider.
pub(crate) fn get_cloud_provider() -> &'static dyn CloudProvider {
    CLOUD_PROVIDER
        .get()
        .map(|b| b.as_ref())
        .unwrap_or(&NoOpCloudProvider)
}
