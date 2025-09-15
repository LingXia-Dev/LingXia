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
}

/// Animation type for page transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationType {
    /// No animation (used for Launch/Replace/SwitchTab semantics)
    None = 0,
    /// Forward animation (push-style)
    Forward = 1,
    /// Backward animation (pop-style)
    Backward = 2,
}

impl From<i32> for AnimationType {
    fn from(value: i32) -> Self {
        match value {
            1 => AnimationType::Forward,
            2 => AnimationType::Backward,
            _ => AnimationType::None,
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

    /// Navigate to a different page within the same lxapp with specific animation type
    ///
    /// # Arguments
    /// * `appid` - The ID of the lxapp to navigate in
    /// * `path` - The path of the page to navigate to
    /// * `animation_type` - The type of animation to perform
    ///
    /// # Returns
    /// * `Result<(), PlatformError>` - Success or error
    fn navigate(
        &self,
        appid: String,
        path: String,
        animation_type: AnimationType,
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

/// User feedback functionality trait
///
/// This trait defines the user feedback capabilities for the platform,
/// including toast notifications and modal dialogs
pub trait UserFeedback: Send + Sync + 'static {
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

    /// Show a modal dialog with the specified options
    ///
    /// # Arguments
    /// * `options` - Modal configuration options
    /// * `callback_id` - Callback ID for async result handling
    ///
    /// # Returns
    /// * `Result<(), PlatformError>` - Success or error (result comes via callback)
    fn show_modal(&self, options: ModalOptions, callback_id: u64) -> Result<(), PlatformError>;
}
