#![cfg_attr(not(feature = "js-appservice"), allow(dead_code, unused_imports))]

mod app;
mod applink;
mod appservice;
mod archive;
pub(crate) mod bridge;
mod cache;
mod delegate;
mod error;
mod executor;
pub mod host;
pub mod lifecycle;
pub mod log {
    pub use lingxia_log::{
        CollectedLogArchive, CollectedLogArchiveInfo, LogLevel, LogMessage, LogProvider,
    };
}
#[cfg(feature = "js-appservice")]
pub mod lx;
mod lxapp;
mod native_component;
mod page;
pub(crate) mod plugin;
pub mod provider;
mod route;
pub mod startup;
mod update;
pub(crate) mod view_call;

/// SDK/runtime version of lingxia-lxapp.
/// This is used for update compatibility checks and can be reported to update services.
pub const SDK_RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use app::LxAppRuntimeConfig;
pub use applink::handle_applink;
#[cfg(feature = "js-appservice")]
pub use appservice::PageSvc;
pub use appservice::event_bus::{publish_app_event, publish_page_event};
#[cfg(feature = "js-appservice")]
pub use appservice::event_bus::{
    register_app_handler, register_page_handler, unregister_app_handler, unregister_page_handler,
};
pub use cache::{
    LxAppCache, ResolveResult, cleanup_all_cache_dirs, cleanup_all_cache_dirs_keep,
    cleanup_cache_dir, cleanup_cache_dir_keep, cleanup_cache_for_storage_pressure,
    cleanup_cache_for_storage_pressure_keep,
};
pub use delegate::{LxAppDelegate, LxAppUiEventType};
pub use error::LxAppError;
pub use lifecycle::{
    AppServiceEvent, AppServiceEventArgs, AppServiceEventReason, AppServiceEventSource,
    LxAppLifecycleEvent, PageLifecycleEvent, PageServiceEvent,
};
pub use lingxia_update::{
    ReleaseType, RuntimeCompatibilityError, SemanticVersion, Version, VersionError,
};
pub use lxapp::set_num_workers;
pub use lxapp::{
    LxApp, LxAppRuntimeInfo, LxAppRuntimePageInfo, PopupMode, close_lxapp, config::LxAppInfo,
    ensure_builtin_lxapp, ensure_lxapp, get_current_lxapp, get_locale, get_platform, init,
    installed_lxapp_path, is_pull_down_refresh_enabled, list_lxapps, on_low_memory, open_lxapp,
    page_config::OrientationConfig, page_config::PageOrientation, register_builtin_asset_bundle,
    register_dev_bundle_source, restart_lxapp, tabbar, try_get, uninstall_lxapp,
};
pub use native_component::on_native_component_event;
pub use page::{
    NavigationType, Page, ViewCallOptions, add_global_page_script, register_page_resolver,
    resolve_page_path,
};
pub use plugin::{build_plugin_page_path, parse_plugin_page_path, parse_plugin_url};
pub use provider::{
    BoxFuture, FingerprintProvider, LxAppUpdateQuery, NoOpProvider, Provider, ProviderError,
    ProviderErrorCode, ProviderErrorExt, PushNotificationProvider, UpdatePackageInfo,
    UpdateProvider, UpdateTarget, register_provider,
};
pub use startup::{
    LxAppStartupOptions, Scene, append_page_query, parse_env_release_type,
    parse_optional_env_release_type,
};
pub use update::{
    DownloadedUpdateInfo, OtaUpdateTarget, UpdateManager, ensure_force_update_for_installed,
    ensure_target_version_ready, is_force_update_downloading, prepare_lxapp_open,
    schedule_lxapp_update_check,
};

// Re-export for internal crate usage
pub(crate) use provider::get_provider;

pub fn js_appservice_supported() -> bool {
    cfg!(feature = "js-appservice")
}

pub fn js_lxapp_supported() -> bool {
    js_appservice_supported()
}

#[doc(hidden)]
pub mod __private {
    pub use lingxia_log::{LogBuilder, LogLevel, LogTag};
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::__private::LogBuilder::new(
            $crate::__private::LogTag::Native,
            format!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::__private::LogBuilder::new(
            $crate::__private::LogTag::Native,
            format!($($arg)*),
        )
        .with_level($crate::__private::LogLevel::Warn)
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::__private::LogBuilder::new(
            $crate::__private::LogTag::Native,
            format!($($arg)*),
        )
        .with_level($crate::__private::LogLevel::Error)
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::__private::LogBuilder::new(
            $crate::__private::LogTag::Native,
            format!($($arg)*),
        )
        .with_level($crate::__private::LogLevel::Debug)
    };
}

#[macro_export]
macro_rules! verbose {
    ($($arg:tt)*) => {
        $crate::__private::LogBuilder::new(
            $crate::__private::LogTag::Native,
            format!($($arg)*),
        )
        .with_level($crate::__private::LogLevel::Verbose)
    };
}
