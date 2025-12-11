//! LingXia Platform
//!
//! This crate provides the platform-specific implementation for LingXia.

use std::io::Read;

/// Asset file entry with reader for streaming content
pub struct AssetFileEntry<'a> {
    pub path: String,
    pub reader: Box<dyn Read + 'a>,
}

/// Device information
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub brand: String,
    pub model: String,
    pub market_name: String,
    pub system: String,
}

/// Screen information reported in logical pixels (dp/pt) and scale factor
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScreenInfo {
    pub width: f64,
    pub height: f64,
    pub scale: f64,
}

mod traits;

#[cfg(target_os = "android")]
mod android;

#[cfg(any(target_os = "ios", target_os = "macos"))]
mod apple;

#[cfg(target_env = "ohos")]
pub mod harmony;

// Export Platform and Device types for each platform
#[cfg(target_os = "android")]
pub use android::{CachedClass, Platform, get_android_id, init_cached_class};

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use apple::Platform;

#[cfg(target_env = "ohos")]
pub use harmony::Platform;

pub mod error;
// Re-export error types
pub use error::*;

// Re-export traits
pub use traits::*;
