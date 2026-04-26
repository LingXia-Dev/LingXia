//! Apple platform implementation for LingXia
//!
//! This module provides Swift FFI interfaces for iOS and macOS platforms.

mod app;
mod device;
mod ffi;
mod file;
mod location;
mod media;
mod network;
mod pull_to_refresh;
mod resources;
mod surface;
mod ui_update;
mod user_feedback;
mod video_player;
mod wifi;

// Re-export Platform
pub use app::Platform;
