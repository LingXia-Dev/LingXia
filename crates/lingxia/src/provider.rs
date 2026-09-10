//! Provider traits and registration helpers for LingXia host integrations.

pub use lingxia_provider::{
    BoxFuture, FingerprintProvider, ProviderError, ProviderErrorCode, PushNotificationProvider,
};
pub use lingxia_update::{
    LxAppUpdateQuery, UpdateAuthentication, UpdatePackageInfo, UpdateProvider, UpdateTarget,
};
pub use lxapp::{
    LxAppChannel, LxAppNetworkPermission, LxAppPermissions, LxAppRegistryInfo,
    LxAppRegistryProvider, LxAppRegistryRequest, LxAppStatus, Provider,
    register_lxapp_registry_provider, register_provider,
};
