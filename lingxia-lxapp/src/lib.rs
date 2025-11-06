mod app;
mod appservice;
mod cache;
mod delegate;
mod error;
mod executor;
pub mod log;
pub mod lx;
mod lxapp;
mod update;
pub use cache::{LxAppCache, ResolveResult};
mod page;
pub mod startup;

pub use delegate::{LxAppDelegate, UiEventType};
pub use error::LxAppError;
pub use lxapp::{
    LxApp, ReleaseType, config::LxAppInfo, get, get_current_lxapp, init, on_low_memory, tabbar,
};
pub use page::NavigationType;
pub use startup::{LxAppStartupOptions, Scene, parse_env_release_type};
pub use update::{UpdateCheckResult, UpdateManager, UpdatePackageInfo};
