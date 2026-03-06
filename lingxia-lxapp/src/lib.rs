mod app;
mod appservice;
mod archive;
pub mod browser;
mod cache;
mod delegate;
mod error;
mod executor;
pub(crate) mod host;
pub mod key_event;
pub mod lifecycle;
pub mod log;
pub mod lx;
mod lxapp;
mod page;
pub(crate) mod plugin;
pub mod provider;
mod route;
pub mod startup;
pub mod stream_source;
mod update;

/// SDK/runtime version of lingxia-lxapp.
/// This is used for update compatibility checks and can be reported to update services.
pub const SDK_RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use app::{app_config, product_name, product_version};
pub use appservice::PageSvc;
pub use appservice::event_bus::{
    publish_app_event, publish_page_event, register_app_handler, register_page_handler,
    unregister_app_handler, unregister_page_handler,
};
pub use browser::{
    BUILTIN_BROWSER_APPID, browser_owner_appid_for_tab_id, browser_owner_session_id_for_tab_id,
    browser_tab_exists, browser_tab_path_for_id, close_browser_tab, close_internal_browser_tab,
    find_browser_webview, generate_browser_startup_html, open_internal_browser_tab,
    resolve_owner_lxapp,
};
pub use cache::{LxAppCache, ResolveResult};
pub use delegate::{LxAppDelegate, UiEventType};
pub use error::LxAppError;
pub use lifecycle::{
    AppServiceEvent, AppServiceEventArgs, AppServiceEventReason, AppServiceEventSource,
    LxAppLifecycleEvent, PageLifecycleEvent, PageServiceEvent,
};
pub use lxapp::set_home_lxapp_dev_path;
pub use lxapp::set_num_workers;
pub use lxapp::{
    LxApp, ReleaseType, config::LxAppInfo, get_current_lxapp, get_locale, get_platform, init,
    is_pull_down_refresh_enabled, on_low_memory, page_config::OrientationConfig,
    page_config::PageOrientation, tabbar, try_get,
};
pub use page::NavigationType;
pub use provider::{
    BoxFuture, FingerprintProvider, NoOpProvider, Provider, ProviderError, ProviderErrorCode,
    UpdatePackageInfo, UpdateProvider, UpdateTarget, register_provider,
};
pub use startup::{LxAppStartupOptions, Scene, parse_env_release_type};
pub use stream_source::{
    FrameSink, StreamError, StreamProvider, StreamSession, register_stream_provider,
};
pub use update::{
    DownloadedUpdateInfo, UpdateManager, ensure_force_update_for_installed,
    is_force_update_downloading,
};

// Re-export for internal crate usage
pub(crate) use provider::get_provider;
