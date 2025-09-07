mod app;
mod appservice;
mod delegate;
mod error;
mod executor;
pub mod log;
pub mod lx;
mod lxapp;
mod page;

pub use delegate::LxAppDelegate;
pub use error::LxAppError;
pub use lxapp::{LxApp, config::LxAppInfo, get, init, on_low_memory, tabbar};
