//! Apple platform implementation for LingXia
//!
//! This module provides Swift FFI interfaces for iOS and macOS platforms.

mod app;
mod device;
mod document;
mod ffi;
mod location;
mod media;
mod popup;
mod resources;
mod ui_update;
mod user_feedback;
mod video_player;

// Re-export Platform
pub use app::Platform;
