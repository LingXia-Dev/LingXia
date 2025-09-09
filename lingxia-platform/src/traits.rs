use std::io::Read;
use std::path::PathBuf;

use crate::error::PlatformError;
use crate::{AssetFileEntry, DeviceInfo};

/// Toast icon types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastIcon {
    Success,
    Error,
    Loading,
    None,
}

/// Toast position types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastPosition {
    Top,
    Center,
    Bottom,
}

/// Toast configuration options
#[derive(Debug, Clone)]
pub struct ToastOptions {
    pub title: String,
    pub icon: ToastIcon,
    pub image: Option<String>,
    pub duration: f64,
    pub mask: bool,
    pub position: ToastPosition,
}

/// Modal configuration options
#[derive(Debug, Clone)]
pub struct ModalOptions {
    pub title: String,
    pub content: String,
    pub show_cancel: bool,
    pub cancel_text: String,
    pub cancel_color: Option<String>,
    pub confirm_text: String,
    pub confirm_color: Option<String>,
    pub editable: bool,
    pub placeholder_text: String,
}

/// Modal result
#[derive(Debug, Clone)]
pub struct ModalResult {
    pub confirm: bool,
    pub cancel: bool,
    pub content: String, // User input content if editable
}

/// Navigation type for LxApp navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationType {
    /// Launch navigation - for openLxApp to open entry page
    Launch = 0,
    /// Forward navigation - navigate to a new page with animation
    Forward = 1,
    /// Backward navigation - navigate back with animation
    Backward = 2,
    /// Replace navigation - replace current page without animation
    Replace = 3,
    /// Switch tab navigation - switch between tab pages
    SwitchTab = 4,
}

impl From<i32> for NavigationType {
    fn from(value: i32) -> Self {
        match value {
            0 => NavigationType::Launch,
            1 => NavigationType::Forward,
            2 => NavigationType::Backward,
            3 => NavigationType::Replace,
            4 => NavigationType::SwitchTab,
            _ => NavigationType::Forward, // Default fallback
        }
    }
}

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

    /// Navigate to a different page within the same lxapp with specific navigation type
    ///
    /// # Arguments
    /// * `appid` - The ID of the lxapp to navigate in
    /// * `path` - The path of the page to navigate to
    /// * `navigation_type` - The type of navigation to perform
    ///
    /// # Returns
    /// * `Result<(), PlatformError>` - Success or error
    fn navigate(
        &self,
        appid: String,
        path: String,
        navigation_type: NavigationType,
    ) -> Result<(), PlatformError>;

    /// Launch external application with URL
    ///
    /// # Arguments
    /// * `url` - Complete URL to open the target app
    ///
    /// # Returns
    /// * `Result<(), PlatformError>` - Success or error
    fn launch_with_url(&self, url: String) -> Result<(), PlatformError>;
}

/// Toast functionality trait
///
/// This trait defines the toast display capabilities for the platform
pub trait Toast: Send + Sync + 'static {
    /// Show a toast with the specified options
    ///
    /// # Arguments
    /// * `options` - Toast configuration options
    ///
    /// # Returns
    /// * `Result<(), PlatformError>` - Success or error
    fn show_toast(&self, options: ToastOptions) -> Result<(), PlatformError>;

    /// Hide the currently displayed toast
    ///
    /// # Returns
    /// * `Result<(), PlatformError>` - Success or error
    fn hide_toast(&self) -> Result<(), PlatformError>;
}

/// Modal functionality trait
///
/// This trait defines the modal dialog capabilities for the platform
pub trait Modal: Send + Sync + 'static {
    /// Show a modal dialog with the specified options
    ///
    /// # Arguments
    /// * `options` - Modal configuration options
    ///
    /// # Returns
    /// * `Result<ModalResult, PlatformError>` - Modal result or error
    fn show_modal(&self, options: ModalOptions) -> Result<ModalResult, PlatformError>;
}
