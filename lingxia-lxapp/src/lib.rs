mod app;
mod appservice;
mod cache;
pub mod cloud;
mod delegate;
mod error;
pub mod event;
mod executor;
pub mod log;
pub mod lx;
mod lxapp;
mod page;
pub mod startup;
mod update;

pub use appservice::PageSvc;
pub use appservice::bridge_events::{
    emit_app_event, emit_page_event, register_app_handler, register_page_handler,
    unregister_app_handler, unregister_page_handler,
};
pub use cache::{LxAppCache, ResolveResult};
pub use cloud::{
    BoxFuture, CloudError, CloudProvider, CloudUpdateProvider, UpdateCheckResult,
    UpdatePackageInfo, register_cloud_provider,
};
pub use delegate::{LxAppDelegate, UiEventType};
pub use error::LxAppError;
pub use event::{AppServiceEvent, LxAppLifecycleEvent, PageLifecycleEvent, PageServiceEvent};
pub use lxapp::{
    LxApp, ReleaseType, config::LxAppInfo, get_current_lxapp, init, is_pull_down_refresh_enabled,
    on_low_memory, tabbar, try_get,
};
pub use page::NavigationType;
pub use startup::{LxAppStartupOptions, Scene, parse_env_release_type};
pub use update::{DownloadedUpdateInfo, UpdateManager};

// Re-export for internal crate usage
pub(crate) use cloud::get_cloud_provider;
