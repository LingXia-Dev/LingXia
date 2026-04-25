//! Provider traits and registration helpers for LingXia host integrations.

pub use lingxia_provider::{
    BoxFuture, FingerprintProvider, ProviderError, ProviderErrorCode, PushNotificationProvider,
};
pub use lingxia_update::{LxAppUpdateQuery, UpdatePackageInfo, UpdateProvider, UpdateTarget};
pub use lxapp::{Provider, register_provider};
