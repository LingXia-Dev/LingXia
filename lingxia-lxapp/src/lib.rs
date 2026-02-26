mod app;
mod appservice;
mod archive;
mod cache;
mod delegate;
mod error;
pub mod event;
mod executor;
pub(crate) mod host;
pub mod key_event;
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

pub use app::{app_config, product_name, product_version};
pub use appservice::PageSvc;
pub use appservice::bridge_events::{
    emit_app_event, emit_page_event, register_app_handler, register_page_handler,
    unregister_app_handler, unregister_page_handler,
};
pub use cache::{LxAppCache, ResolveResult};
pub use delegate::{LxAppDelegate, UiEventType};
pub use error::LxAppError;
pub use event::{
    AppServiceEvent, AppServiceEventArgs, AppServiceEventReason, AppServiceEventSource,
    LxAppLifecycleEvent, PageLifecycleEvent, PageServiceEvent,
};
pub use lxapp::set_home_lxapp_dev_path;
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
pub use update::{DownloadedUpdateInfo, UpdateManager};

// Re-export for internal crate usage
pub(crate) use provider::get_provider;
