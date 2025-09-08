mod app;
mod appservice;
mod delegate;
mod error;
mod executor;
pub mod log;
pub mod lx;
mod lxapp;
mod page;

pub use delegate::{LxAppDelegate, UiEventType};
pub use error::LxAppError;
pub use lxapp::{LxApp, config::LxAppInfo, get, get_current_lxapp, init, on_low_memory, tabbar};
