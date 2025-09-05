use std::io::Read;
use std::path::PathBuf;

use crate::error::PlatformError;
use crate::{AssetFileEntry, DeviceInfo};

/// Base platform runtime capabilities
///
/// This trait defines the core capabilities required for the lxapp platform,
/// including resource access, directory management
pub trait AppRuntime: Send + Sync + 'static {
    /// Read asset file from platform-specific location as a streaming reader
    ///
    /// # Arguments
    /// * `path` - Path to the asset file to read
    ///
    /// # Returns
    /// * `Result<Box<dyn Read + '_>, PlatformError>` - A reader for streaming the asset content, or an error
    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, PlatformError>;

    /// Iterate over files in an asset directory.
    ///
    /// Returns an iterator of AssetFileEntry, each containing the file path and a reader implementing `Read`.
    ///
    /// # Arguments
    /// * `asset_dir` - Directory path in assets to iterate
    ///
    /// # Returns
    /// * `Box<dyn Iterator<Item = Result<AssetFileEntry, PlatformError>>>` - Iterator over files in the directory
    ///   (If directory cannot be opened, the iterator's first element will be an error)
    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, PlatformError>> + 'a>;

    /// Get data directory path
    ///
    /// # Returns
    /// * `PathBuf` - Path to the application's data directory
    fn app_data_dir(&self) -> PathBuf;

    /// Get cache directory path
    ///
    /// # Returns
    /// * `PathBuf` - Path to the application's cache directory
    fn app_cache_dir(&self) -> PathBuf;

    /// Get device information
    ///
    /// # Returns
    /// * `DeviceInfo` - Device information including brand, model, and screen dimensions
    fn device_info(&self) -> DeviceInfo;

    /// Open a lxapp
    ///
    /// # Arguments
    /// * `appid` - The ID of the lxapp to open
    /// * `path` - The initial path to navigate to within the lxapp
    ///
    /// # Returns
    /// * `Result<(), PlatformError>` - Success or error
    fn open_lxapp(&self, appid: String, path: String) -> Result<(), PlatformError>;

    /// Close a lxapp
    ///
    /// # Arguments
    /// * `appid` - The ID of the lxapp to close
    ///
    /// # Returns
    /// * `Result<(), PlatformError>` - Success or error
    fn close_lxapp(&self, appid: String) -> Result<(), PlatformError>;

    /// Switch to a different page within the same lxapp
    ///
    /// # Arguments
    /// * `appid` - The ID of the lxapp to switch pages in
    /// * `path` - The path of the page to switch to
    ///
    /// # Returns
    /// * `Result<(), PlatformError>` - Success or error
    fn switch_page(&self, appid: String, path: String) -> Result<(), PlatformError>;

    /// Launch external application with URL
    ///
    /// # Arguments
    /// * `url` - Complete URL to open the target app
    ///
    /// # Returns
    /// * `Result<(), PlatformError>` - Success or error
    fn launch_with_url(&self, url: String) -> Result<(), PlatformError>;
}
