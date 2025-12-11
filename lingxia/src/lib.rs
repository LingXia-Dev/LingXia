//! LingXia framework.

pub use lxapp::lx::{LxLogicExtension, register_logic_extension};
pub use lxapp::{
    BoxFuture, NoOpProvider, Provider, ProviderError, UpdateCheckResult, UpdatePackageInfo,
    UpdateProvider, register_provider,
};

#[cfg(target_os = "android")]
pub mod android;

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub mod apple;

#[cfg(target_env = "ohos")]
pub mod harmony;

/// Common initialization after Platform is created.
/// Registers built-in runtime and initializes the lxapp system.
pub(crate) fn init_with_platform(platform: lingxia_platform::Platform) -> Option<String> {
    lingxia_logic::register_logic_runtime();
    lxapp::init(platform)
}
