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
    pub system: String,
}

mod traits;

#[cfg(target_os = "android")]
mod android;

#[cfg(any(target_os = "ios", target_os = "macos"))]
mod apple;

#[cfg(target_env = "ohos")]
mod harmony;

// Export Platform type for each platform
#[cfg(target_os = "android")]
pub use android::{Platform, init_lxapp_class};

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use apple::Platform;

#[cfg(target_env = "ohos")]
pub use harmony::Platform;

pub mod error;
// Re-export error types
pub use error::*;

// Re-export traits
pub use traits::*;
