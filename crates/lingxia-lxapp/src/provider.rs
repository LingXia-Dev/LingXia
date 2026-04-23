//! Provider traits and registration.
//!
//! Generic provider contracts live in `lingxia-provider`, `lingxia-update`,
//! and `lingxia-log`.
//! This module keeps only lxapp/runtime-specific pieces: provider aggregation,
//! registration, and conversion into `LxAppError`.

use crate::error::LxAppError;
pub use lingxia_log::LogProvider;
use std::sync::OnceLock;

pub use lingxia_provider::{
    BoxFuture, FingerprintError, FingerprintProvider, ProviderError, ProviderErrorCode,
    PushNotificationProvider,
};
pub use lingxia_update::{LxAppUpdateQuery, UpdatePackageInfo, UpdateProvider, UpdateTarget};

pub trait ProviderErrorExt {
    fn to_lxapp_error(&self) -> LxAppError;
}

impl ProviderErrorExt for ProviderError {
    fn to_lxapp_error(&self) -> LxAppError {
        provider_error_to_lxapp_error(self)
    }
}

pub(crate) fn provider_error_to_lxapp_error(error: &ProviderError) -> LxAppError {
    let biz_code = error.biz_code();
    let detail = error.detail().to_string();
    LxAppError::RongJSHost {
        code: biz_code.to_string(),
        message: detail.clone(),
        data: Some(serde_json::json!({
            "bizCode": biz_code,
            "providerCode": error.code().as_str(),
            "detail": detail,
        })),
    }
}

/// Runtime provider aggregation used by lxapp host registration.
pub trait Provider: UpdateProvider + FingerprintProvider + PushNotificationProvider {}

impl<T> Provider for T where T: UpdateProvider + FingerprintProvider + PushNotificationProvider {}

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
impl PushNotificationProvider for NoOpProvider {}
impl LogProvider for NoOpProvider {}

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

pub(crate) fn has_update_provider() -> bool {
    PROVIDER.get().is_some()
}

/// Bind a push token via the registered provider.
pub async fn bind_push_token(token: String) -> Result<(), ProviderError> {
    get_provider().bind_push_token(token).await
}
