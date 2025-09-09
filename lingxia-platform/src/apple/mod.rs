//! Apple platform implementation for LingXia
//!
//! This module provides Swift FFI interfaces for iOS and macOS platforms.

mod app;
mod ffi;
mod resources;
mod toast;

// Re-export App
pub use app::Platform;
