mod app;
mod appservice;
mod archive;
pub(crate) mod bridge;
mod cache;
mod delegate;
mod error;
mod executor;
pub mod host;
pub mod lifecycle;
pub mod log;
pub mod lx;
mod lxapp;
mod page;
pub(crate) mod plugin;
pub mod provider;
mod route;
pub mod startup;
mod update;
pub(crate) mod workers;

/// SDK/runtime version of lingxia-lxapp.
/// This is used for update compatibility checks and can be reported to update services.
pub const SDK_RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use app::LxAppRuntimeConfig;
pub use appservice::PageSvc;
pub use appservice::event_bus::{
    publish_app_event, publish_page_event, register_app_handler, register_page_handler,
    unregister_app_handler, unregister_page_handler,
};
pub use appservice::native_component::on_native_component_event;
pub use cache::{LxAppCache, ResolveResult};
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
    LxApp, PopupMode, config::LxAppInfo, ensure_builtin_lxapp, ensure_lxapp, get_current_lxapp,
    get_locale, get_platform, init, installed_lxapp_path, is_pull_down_refresh_enabled,
    on_low_memory, open_lxapp, page_config::OrientationConfig, page_config::PageOrientation,
    register_builtin_asset_bundle, register_dev_bundle_source, tabbar, try_get,
};
pub use page::{
    NavigationType, Page, ViewCallOptions, add_global_page_script, register_page_resolver,
    resolve_page_path,
};
pub use plugin::{build_plugin_page_path, parse_plugin_page_path, parse_plugin_url};
pub use provider::{
    BoxFuture, FingerprintProvider, LogProvider, LxAppUpdateQuery, NoOpProvider, Provider,
    ProviderError, ProviderErrorCode, ProviderErrorExt, PushNotificationProvider,
    UpdatePackageInfo, UpdateProvider, UpdateTarget, register_log_provider, register_provider,
};
pub use startup::{LxAppStartupOptions, Scene, parse_env_release_type};
pub use update::{
    DownloadedUpdateInfo, OtaUpdateTarget, UpdateManager, ensure_force_update_for_installed,
    is_force_update_downloading, prepare_lxapp_open, schedule_lxapp_update_check,
};

// Re-export for internal crate usage
pub(crate) use provider::get_provider;
