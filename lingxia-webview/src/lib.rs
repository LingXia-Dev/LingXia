use miniapp::MiniAppError;
use miniapp::log::LogLevel;
use std::path::PathBuf;

#[cfg(target_os = "android")]
mod android;

#[cfg(any(target_os = "ios", target_os = "macos"))]
mod apple;

mod controller;

#[cfg(target_os = "android")]
pub use android::{App, WebView};

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use apple::{App, WebView};

/// Platform-specific operations for mini apps
///
/// This trait defines operations that must be implemented by platform-specific
/// code (iOS, Android etc) to support mini-app functionality. It includes resource
/// access, directory management, and app lifecycle operations.
pub trait MiniAppPlatform {
    /// Read asset file from platform-specific location
    fn read_asset(&self, path: &str) -> Result<Vec<u8>, MiniAppError>;

    /// Get data directory path for app resources
    fn app_data_dir(&self) -> PathBuf;

    /// Get cache directory path for app temporary files
    fn app_cache_dir(&self) -> PathBuf;

    /// Log message to platform-specific logging system
    fn log(&self, app_id: &str, level: LogLevel, message: &str);

    /// Open a mini app in the platform-specific UI
    fn open_miniapp(&self, app_id: &str, path: &str) -> Result<(), MiniAppError>;

    /// Switch to a different page within a mini app
    fn switch_page(&self, app_id: &str, path: &str) -> Result<(), MiniAppError>;
}
