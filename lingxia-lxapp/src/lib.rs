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
pub use appservice::get_or_create_update_manager;
pub use cache::{LxAppCache, ResolveResult};
pub use cloud::{
    BoxFuture, CloudError, CloudProvider, CloudUpdateProvider, UpdateCheckResult,
    UpdatePackageInfo, register_cloud_provider,
};
pub use delegate::{LxAppDelegate, UiEventType};
pub use error::LxAppError;
pub use event::{
    AppServiceEvent, LxAppEvent, LxAppLifecycleEvent, LxAppUpdateEvent, PageLifecycleEvent,
    PageServiceEvent,
};
pub use lxapp::{
    LxApp, ReleaseType, config::LxAppInfo, get_current_lxapp, init, on_low_memory, tabbar, try_get,
};
pub use page::NavigationType;
pub use startup::{LxAppStartupOptions, Scene, parse_env_release_type};
pub use update::{DownloadedUpdateInfo, UpdateManager};

// Re-export for internal crate usage
pub(crate) use cloud::get_cloud_provider;
