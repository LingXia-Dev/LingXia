#![cfg_attr(not(feature = "js-appservice"), allow(dead_code, unused_imports))]

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

#[cfg(feature = "js-appservice")]
pub use appservice::PageSvc;
pub use appservice::event_bus::{publish_app_event, publish_page_event};
#[cfg(feature = "js-appservice")]
pub use appservice::event_bus::{
    register_app_handler, register_page_handler, unregister_app_handler, unregister_page_handler,
};
pub use cache::touch_access_time;
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
    CloseReason, CreatePageInstanceRequest, CreatedPageInstance, LxApp, LxAppRuntimeInfo,
    LxAppRuntimePageInfo, LxAppSecurityPrivilege, PageDefinition, PageInstanceEvent, PageOwner,
    PageQueryInput, PageSurface, PageSurfaceRequest, PageSurfaceTarget, PageTarget,
    PresentationKind, ResolvedPage, SceneId, SurfaceKind, SurfacePosition, close_lxapp,
    config::LxAppInfo, create_page_instance, dispose_page_instance, dispose_page_instance_by_id,
    ensure_builtin_lxapp, ensure_lxapp, find_page_by_instance_id, get_current_lxapp, get_locale,
    get_platform, init, installed_lxapp_path, is_pull_down_refresh_enabled, list_lxapps,
    notify_page_instance, notify_page_instance_by_id, on_low_memory, open_lxapp,
    register_builtin_asset_bundle, register_dev_bundle_source, register_surface_close_observer,
    register_synthetic_lxapp, restart_lxapp, tabbar, touch_page_instance_by_id, try_get,
    uninstall_lxapp,
};
#[cfg(target_os = "windows")]
pub use lxapp::{
    WindowsTerminalPanelHandler, WindowsTerminalPanelRequest,
    WindowsTerminalPanelVisibilityHandler, set_windows_terminal_panel_handler,
};
pub use native_component::on_native_component_event;
pub use page::config::{OrientationConfig, PageOrientation};
pub use page::{
    NavigationType, PageInstance, PageInstanceId, ViewCallOptions, add_global_page_script,
    register_page_resolver, resolve_page_path,
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
