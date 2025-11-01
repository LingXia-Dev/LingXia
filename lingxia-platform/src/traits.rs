use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error::PlatformError;
use crate::{AssetFileEntry, DeviceInfo, ScreenInfo};

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

/// Popup vertical alignment for display overlays.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupPosition {
    Center = 0,
    Bottom = 1,
}

impl Default for PopupPosition {
    fn default() -> Self {
        PopupPosition::Bottom
    }
}

/// Popup display request structure.
#[derive(Debug, Clone)]
pub struct PopupRequest {
    pub app_id: String,
    pub path: String,
    /// Width expressed as a fraction of the available viewport (0.0–1.0).
    pub width_ratio: f64,
    /// Height expressed as a fraction of the available viewport (0.0–1.0).
    pub height_ratio: f64,
    pub position: PopupPosition,
}

impl PopupRequest {
    pub fn new(app_id: String, path: String) -> Self {
        Self {
            app_id,
            path,
            width_ratio: f64::NAN,
            height_ratio: f64::NAN,
            position: PopupPosition::Bottom,
        }
    }
}

/// Popup presentation functionality trait.
pub trait PopupPresenter: Send + Sync + 'static {
    /// Show a popup with the specified configuration.
    fn show_popup(&self, request: PopupRequest) -> Result<(), PlatformError>;

    /// Hide the popup associated with the provided `app_id`.
    fn hide_popup(&self, app_id: &str) -> Result<(), PlatformError>;
}

/// Media type for preview operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
    Unknown,
}

/// Selection mode for choosing media assets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChooseMediaMode {
    Images,
    Videos,
    Mix,
}

/// Source preference when choosing media.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaSource {
    Album,
    Camera,
}

/// Preferred camera when capturing media.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraFacing {
    Front,
    Back,
}

/// Quality preference for selected media.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaQuality {
    /// Return original quality images
    Original,
    /// Return compressed images
    Compressed,
}

/// Parameters for previewing media (images, video, etc.).
#[derive(Debug, Clone)]
pub struct PreviewMediaItem {
    /// Fully-resolved file path ready for the platform to load.
    pub path: String,
    pub media_type: MediaKind,
    /// Optional cover thumbnail path for video assets.
    pub cover_path: Option<String>,
}

/// Request payload to preview a collection of media items.
#[derive(Debug, Clone)]
pub struct PreviewMediaRequest {
    /// Items that can be swiped/browsed.
    pub items: Vec<PreviewMediaItem>,
}

impl Default for PreviewMediaRequest {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

/// Request options when choosing media from album/camera.
#[derive(Debug, Clone)]
pub struct ChooseMediaRequest {
    /// Maximum number of assets the user can pick.
    pub max_count: u32,
    /// Media type the user may select or capture.
    pub mode: ChooseMediaMode,
    /// Allowed sources (album/camera).
    pub source_types: Vec<MediaSource>,
    /// Optional maximum duration for captured video (seconds).
    pub max_duration_seconds: Option<u32>,
    /// Preferred camera when capturing (optional hint).
    pub camera_facing: Option<CameraFacing>,
    /// Callback identifier for returning results via messaging.
    pub callback_id: u64,
}

impl Default for ChooseMediaRequest {
    fn default() -> Self {
        Self {
            max_count: 9,
            mode: ChooseMediaMode::Images,
            source_types: vec![MediaSource::Album, MediaSource::Camera],
            max_duration_seconds: None,
            camera_facing: None,
            callback_id: 0,
        }
    }
}

/// Supported scan type groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanType {
    /// 1D barcodes (CODE_128/CODE_39/CODE_93/CODABAR/EAN_8/EAN_13/ITF/UPC_A/UPC_E/etc.)
    BarCode,
    /// QR codes
    QrCode,
    /// Data Matrix
    DataMatrix,
    /// PDF417
    Pdf417,
}

/// Request parameters for initiating a scan operation.
#[derive(Debug, Clone)]
pub struct ScanCodeRequest {
    /// Types of code that should be recognised.
    /// Empty vector means: scan all supported formats on the platform.
    pub scan_types: Vec<ScanType>,
    /// When true, restrict scanning to the camera
    pub only_from_camera: bool,
    /// Callback identifier used to deliver scan results via messaging channel.
    pub callback_id: u64,
}

impl Default for ScanCodeRequest {
    fn default() -> Self {
        Self {
            // Empty means: scan all supported types on the platform
            scan_types: Vec::new(),
            only_from_camera: true,
            callback_id: 0,
        }
    }
}

/// Save a media asset into the system gallery/photos album.
#[derive(Debug, Clone)]
pub struct SaveMediaRequest {
    /// File URI pointing to the media resource (e.g., file:///... ).
    pub file_uri: String,
}

/// Media-related platform functionality.
pub trait MediaInteraction: Send + Sync + 'static {
    /// Preview a collection of media assets (images/videos) in full-screen viewer.
    fn preview_media(&self, request: PreviewMediaRequest) -> Result<(), PlatformError>;

    /// Launch a system picker / capture flow to obtain media resources.
    fn choose_media(&self, request: ChooseMediaRequest) -> Result<(), PlatformError>;

    /// Initiate a scan operation (QR/Bar code, etc.) and deliver results via callback.
    fn scan_code(&self, request: ScanCodeRequest) -> Result<(), PlatformError>;

    /// Persist an image asset to the device's gallery/photos album.
    fn save_image_to_photos_album(&self, request: SaveMediaRequest) -> Result<(), PlatformError>;

    /// Persist a video asset to the device's gallery/photos album.
    fn save_video_to_photos_album(&self, request: SaveMediaRequest) -> Result<(), PlatformError>;
}

/// Device capabilities and information
///
/// This trait provides access to device-specific information and capabilities
/// such as device info, screen properties, and brightness control
pub trait Device: Send + Sync + 'static {
    /// Get device information
    ///
    /// # Returns
    /// * `DeviceInfo` - Device information including brand, model, and system version
    fn device_info(&self) -> DeviceInfo;

    /// Get screen information
    ///
    /// # Returns
    /// * `ScreenInfo` - Screen information including logical width/height and scale factor
    fn screen_info(&self) -> ScreenInfo;

    /// Vibrate the device
    ///
    /// # Arguments
    /// * `long` - true for long vibration, false for short vibration
    ///
    /// # Returns
    /// * `Result<(), PlatformError>` - Success or error
    fn vibrate(&self, long: bool) -> Result<(), PlatformError>;

    /// Make a phone call
    ///
    /// # Arguments
    /// * `phone_number` - Phone number to call
    ///
    /// # Returns
    /// * `Result<(), PlatformError>` - Success or error
    fn make_phone_call(&self, phone_number: &str) -> Result<(), PlatformError>;
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

    /// Copy a platform media resource identified by `uri` into the supplied destination path.
    ///
    /// The destination path must be a writable location.
    /// `kind` hints the platform whether the asset is an image or a video.
    fn copy_media_uri_to_path(
        &self,
        uri: &str,
        dest_path: &Path,
        kind: MediaKind,
    ) -> Result<(), PlatformError>;

    /// Exit the application
    ///
    /// # Returns
    /// * `Result<(), PlatformError>` - Success or error
    fn exit_app(&self) -> Result<(), PlatformError>;

    /// Gets the system's primary locale identifier for language and region.
    ///
    /// The locale is returned in BCP 47 format, typically consisting of a language
    /// code and a country/region code (e.g., "en-US", "zh-CN", "fr-FR").
    ///
    /// # Returns
    /// * `&str` - Locale string (e.g., "en-US", "zh-CN")
    fn get_system_locale(&self) -> &str;

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
    /// * `item_color` - Color for the option items (hex format, e.g., "#007AFF")
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
        item_color: String,
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

/// Location request configuration
#[derive(Debug, Clone)]
pub struct LocationRequestConfig {
    /// Whether to request high accuracy location
    pub is_high_accuracy: bool,
    /// High accuracy expire time in milliseconds
    pub high_accuracy_expire_time: Option<u32>,
    /// Whether to include altitude information
    pub include_altitude: bool,
}

impl Default for LocationRequestConfig {
    fn default() -> Self {
        Self {
            is_high_accuracy: false,
            high_accuracy_expire_time: None,
            include_altitude: false,
        }
    }
}

/// Location services interface
///
/// Provides access to the device's current location. Results are delivered via
/// the callback registered through `lingxia_messaging` using the provided
/// `callback_id`.
pub trait Location: Send + Sync + 'static {
    /// Returns `true` when the system location switch is enabled.
    fn is_location_enabled(&self) -> Result<bool, PlatformError>;

    /// Request a single location update. The platform should report the
    /// position through the provided callback ID.
    fn request_location(
        &self,
        callback_id: u64,
        config: LocationRequestConfig,
    ) -> Result<(), PlatformError>;
}
