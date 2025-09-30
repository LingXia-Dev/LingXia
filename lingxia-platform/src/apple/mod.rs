//! Apple platform implementation for LingXia
//!
//! This module provides Swift FFI interfaces for iOS and macOS platforms.

mod app;
mod device;
mod ffi;
mod media;
mod location;
mod popup;
mod resources;
mod ui_update;
mod user_feedback;

// Re-export Platform
pub use app::Platform;
