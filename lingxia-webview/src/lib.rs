use miniapp::MiniAppError;
use miniapp::log::LogLevel;
use std::io::Read;
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

/// Asset file entry for iterator-based asset access
pub struct AssetFileEntry<'a> {
    pub path: String,
    pub reader: Box<dyn Read + 'a>,
}

/// Platform-specific operations for mini apps
///
/// This trait defines operations that must be implemented by platform-specific
/// code (iOS, Android etc) to support mini-app functionality. It includes resource
/// access, directory management, and app lifecycle operations.
pub trait MiniAppPlatform {
    /// Read asset file from platform-specific location as a streaming reader
    ///
    /// # Arguments
    /// * `path` - Path to the asset file to read
    ///
    /// # Returns
    /// * `Result<Box<dyn Read + '_>, MiniAppError>` - A reader for streaming the asset content, or an error
    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, MiniAppError>;

    /// Iterate over files in an asset directory.
    ///
    /// Returns an iterator of AssetFileEntry, each containing the file path and a reader implementing `Read`.
    ///
    /// # Arguments
    /// * `asset_dir` - Directory path in assets to iterate
    ///
    /// # Returns
    /// * `Box<dyn Iterator<Item = Result<AssetFileEntry, MiniAppError>>>` - Iterator over files in the directory
    ///   (If directory cannot be opened, the iterator's first element will be an error)
    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, MiniAppError>> + 'a>;

    /// Get data directory path for app resources
    ///
    /// # Returns
    /// * `PathBuf` - Path to the application's data directory
    fn app_data_dir(&self) -> PathBuf;

    /// Get cache directory path for app temporary files
    ///
    /// # Returns
    /// * `PathBuf` - Path to the application's cache directory
    fn app_cache_dir(&self) -> PathBuf;

    /// Log message to platform-specific logging system
    ///
    /// # Arguments
    /// * `level` - Log severity level
    /// * `message` - Log message content
    fn log(&self, level: LogLevel, message: &str);

    /// Open a mini app in the platform-specific UI
    ///
    /// # Arguments
    /// * `appid` - Identifier of the mini application to open
    /// * `path` - Initial path to navigate to within the app
    ///
    /// # Returns
    /// * `Result<(), MiniAppError>` - Success or error response
    fn open_miniapp(&self, appid: &str, path: &str) -> Result<(), MiniAppError>;

    /// Switch to a different page within a mini app
    ///
    /// # Arguments
    /// * `appid` - Identifier of the mini application
    /// * `path` - Path to navigate to within the app
    ///
    /// # Returns
    /// * `Result<(), MiniAppError>` - Success or error response
    fn switch_page(&self, appid: &str, path: &str) -> Result<(), MiniAppError>;
}
