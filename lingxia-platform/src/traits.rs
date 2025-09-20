use std::collections::HashMap;
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

/// Picker type with associated data
#[derive(Debug, Clone)]
pub enum PickerType {
    /// Single column picker with a list of items
    SingleColumn { items: Vec<String> },
    /// Dual column picker with two lists of items
    DualColumn {
        first_column: Vec<String>,
        second_column: Vec<String>,
    },
    /// Dual column picker with cascading (linked) data
    DualColumnCascading {
        first_column: Vec<String>,
        cascading_data: HashMap<String, Vec<String>>,
    },
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

    /// Show an action sheet with a list of options
    ///
    /// This method displays an action sheet interface similar to WeChat mini-program's showActionSheet.
    /// Users can select an option by tapping it directly, or cancel the operation.
    ///
    /// # Arguments
    /// * `options` - List of option strings to display
    /// * `cancel_text` - Text for the cancel button
    /// * `callback_id` - Unique identifier for the callback
    ///
    /// # Behavior
    /// - Displays a list of options that users can tap to select
    /// - Tapping an option immediately confirms the selection and closes the sheet
    /// - Tapping cancel or the background dismisses the sheet without selection
    /// - No confirm button - selection is immediate upon tapping an option
    ///
    /// # Callback Results
    /// The callback receives JSON in the following format:
    /// ```json
    /// {"tapIndex": 2}   // User selected option at index 2
    /// {"tapIndex": -1}  // User cancelled (tapped cancel or background)
    /// ```
    fn show_action_sheet(
        &self,
        options: Vec<String>,
        cancel_text: String,
        callback_id: u64,
    ) -> Result<(), PlatformError>;

    /// Show a picker with the specified type and options
    ///
    /// This method displays a picker interface that supports both single-column and dual-column scenarios.
    /// Unlike action sheets, pickers require explicit confirmation and support real-time selection updates.
    ///
    /// # Arguments
    /// * `picker_type` - Type of picker (SingleColumn or DualColumn) with associated data
    /// * `cancel_text` - Text for the cancel button
    /// * `cancel_color` - Color for the cancel button (hex format, e.g., "#666666")
    /// * `confirm_text` - Text for the confirm button
    /// * `confirm_color` - Color for the confirm button (hex format, e.g., "#007AFF")
    /// * `callback_id` - Callback ID for async result handling
    ///
    /// # Returns
    /// * `Result<(), PlatformError>` - Success or error (result comes via callback)
    ///
    /// # Behavior
    /// ## Single Column Picker
    /// - User can scroll to select an option from a single list
    /// - Must click confirm or cancel to close
    /// - Sends real-time selection updates during scrolling (only when selection changes)
    ///
    /// ## Dual Column Picker
    /// - User can scroll both columns independently
    /// - Must click confirm or cancel to close
    /// - Sends real-time selection updates for both columns during scrolling (only when selection changes)
    ///
    /// # Callback Results
    /// The callback receives JSON in the following format:
    /// ```json
    /// {"index": 1, "confirm": true}       // Single column: User confirmed selection
    /// {"index": [1, 2], "confirm": true}  // Dual column: User confirmed selection
    /// {"index": 1, "cancel": true}        // Single column: User cancelled (shows current selection)
    /// {"index": [1, 2], "cancel": true}   // Dual column: User cancelled (shows current selection)
    /// {"index": 3}                        // Single column: Real-time update during scrolling
    /// {"index": [3, 4]}                   // Dual column: Real-time update during scrolling
    /// ```
    ///
    fn show_picker(
        &self,
        picker_type: PickerType,
        cancel_text: String,
        cancel_button_color: String,
        cancel_text_color: String,
        confirm_text: String,
        confirm_button_color: String,
        confirm_text_color: String,
        callback_id: u64,
    ) -> Result<(), PlatformError>;
}

/// UI update functionality trait
///
/// This trait defines the UI update capabilities for the platform,
/// including NavigationBar and TabBar updates
pub trait UIUpdate: Send + Sync + 'static {
    /// Update NavigationBar UI to refresh state of current path
    ///
    /// # Arguments
    /// * `appid` - The ID of the lxapp whose NavigationBar needs updating
    ///
    /// # Returns
    /// * `Result<(), PlatformError>` - Success or error
    fn update_navbar_ui(&self, appid: String) -> Result<(), PlatformError>;

    /// Update TabBar UI to refresh state
    ///
    /// # Arguments
    /// * `appid` - The ID of the lxapp whose TabBar needs updating
    ///
    /// # Returns
    /// * `Result<(), PlatformError>` - Success or error
    fn update_tabbar_ui(&self, appid: String) -> Result<(), PlatformError>;
}
