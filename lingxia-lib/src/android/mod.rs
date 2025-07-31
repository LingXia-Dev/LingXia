//! Android platform implementation for LingXia
//!
//! This module provides JNI FFI interfaces for Android platform.

mod app;
mod ffi;

// Re-export App
pub use app::App;
