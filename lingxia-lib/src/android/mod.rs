//! Android platform implementation for LingXia
//! 
//! This module provides JNI FFI interfaces for Android platform.

mod ffi;

// Re-export all FFI functions
pub use ffi::*;
