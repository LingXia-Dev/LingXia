mod app;
mod appservice;
mod delegate;
mod error;
mod executor;
pub mod log;
pub mod lx;
mod lxapp;
mod module;
mod page;

pub use delegate::LxAppDelegate;
pub use error::LxAppError;
pub use lxapp::{LxApp, config::LxAppInfo, get, init, tabbar};
pub use module::{LxAppModule, register_module};
