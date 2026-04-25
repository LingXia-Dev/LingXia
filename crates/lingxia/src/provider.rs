//! Provider traits and registration helpers for LingXia host integrations.

pub use lingxia_provider::{
    BoxFuture, FingerprintProvider, ProviderError, ProviderErrorCode, PushNotificationProvider,
};
pub use lxapp::{NoOpProvider, Provider, ProviderErrorExt, register_provider};
