use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error::PlatformError;
use crate::{AssetFileEntry, DeviceInfo, ScreenInfo};

use super::media_interaction::{MediaInteraction, MediaKind};
use super::media_runtime::MediaRuntime;
use super::wifi::Wifi;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastIcon {
    Success,
    Error,
    Loading,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastPosition {
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Clone)]
pub struct ToastOptions {
    pub title: String,
    pub icon: ToastIcon,
    pub image: Option<String>,
    pub duration: f64,
    pub mask: bool,
    pub position: ToastPosition,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationType {
    None = 0,
    Forward = 1,
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

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopupPosition {
    Center = 0,
    #[default]
    Bottom = 1,
    Left = 2,
    Right = 3,
}

#[derive(Debug, Clone)]
pub struct PopupRequest {
    pub app_id: String,
    pub path: String,
    pub width_ratio: f64,
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

pub trait PopupPresenter: Send + Sync + 'static {
    fn show_popup(&self, request: PopupRequest) -> Result<(), PlatformError>;
    fn hide_popup(&self, app_id: &str) -> Result<(), PlatformError>;
}

#[derive(Debug, Clone)]
pub struct OpenDocumentRequest {
    pub file_path: String,
    pub mime_type: Option<String>,
    pub show_menu: Option<bool>,
}

pub trait DocumentInteraction: Send + Sync + 'static {
    fn open_document(&self, request: OpenDocumentRequest) -> Result<(), PlatformError>;
}

pub trait Device: Send + Sync + 'static {
    fn device_info(&self) -> DeviceInfo;
    fn screen_info(&self) -> ScreenInfo;
    fn vibrate(&self, long: bool) -> Result<(), PlatformError>;
    fn make_phone_call(&self, phone_number: &str) -> Result<(), PlatformError>;
}

pub trait DeviceSecureStore: Send + Sync + 'static {
    /// Read a persisted value from a secure, app-scoped store that survives reinstall where supported.
    fn secure_store_read(&self, key: &str) -> Result<Option<Vec<u8>>, PlatformError> {
        Err(PlatformError::Platform(format!(
            "secure_store_read not implemented for key {}",
            key
        )))
    }

    /// Persist a value into the secure store.
    fn secure_store_write(&self, key: &str, value: &[u8]) -> Result<(), PlatformError> {
        let _ = (key, value);
        Err(PlatformError::Platform(
            "secure_store_write not implemented".to_string(),
        ))
    }

    /// Delete a value from the secure store.
    fn secure_store_delete(&self, key: &str) -> Result<(), PlatformError> {
        Err(PlatformError::Platform(format!(
            "secure_store_delete not implemented for key {}",
            key
        )))
    }
}

pub trait DeviceHardware: Send + Sync + 'static {
    /// Get total physical memory in bytes.
    fn get_memory_info(&self) -> Result<u64, PlatformError> {
        Err(PlatformError::Platform(
            "get_memory_info not implemented".to_string(),
        ))
    }

    /// Get the number of logical CPU cores available.
    fn get_cpu_count(&self) -> usize {
        std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1)
    }

    /// Get total ROM storage in bytes.
    fn get_storage_total_bytes(&self) -> Result<u64, PlatformError> {
        Err(PlatformError::Platform(
            "get_storage_total_bytes not implemented".to_string(),
        ))
    }
}

#[derive(Debug, Clone, Default)]
pub struct LocationRequestConfig {
    pub is_high_accuracy: bool,
    pub high_accuracy_expire_time: Option<u32>,
    pub include_altitude: bool,
}

pub trait Location: Send + Sync + 'static {
    fn is_location_enabled(&self) -> Result<bool, PlatformError>;
    fn request_location(
        &self,
        callback_id: u64,
        config: LocationRequestConfig,
    ) -> Result<(), PlatformError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionKind {
    Location,
    Camera,
    Microphone,
    PhotoLibraryRead,
    PhotoLibraryWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionStatus {
    Granted,
    Denied,
    Restricted,
    Unknown,
}

pub trait Permissions: Send + Sync + 'static {
    fn check_permission(
        &self,
        permission: PermissionKind,
    ) -> Result<PermissionStatus, PlatformError>;

    fn request_permission(
        &self,
        permission: PermissionKind,
        callback_id: u64,
    ) -> Result<(), PlatformError>;
}

pub trait UIUpdate: Send + Sync + 'static {
    fn update_navbar_ui(&self, appid: String) -> Result<(), PlatformError>;
    fn update_tabbar_ui(&self, appid: String) -> Result<(), PlatformError>;
}

pub trait UserFeedback: Send + Sync + 'static {
    fn show_toast(&self, options: ToastOptions) -> Result<(), PlatformError>;
    fn hide_toast(&self) -> Result<(), PlatformError>;
    fn show_modal(&self, options: ModalOptions, callback_id: u64) -> Result<(), PlatformError>;
    fn show_action_sheet(
        &self,
        options: Vec<String>,
        cancel_text: String,
        item_color: String,
        callback_id: u64,
    ) -> Result<(), PlatformError>;
}

pub trait UpdateService: Send + Sync + 'static {
    /// Show download progress UI
    fn show_download_progress(&self) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Update download progress (0-100)
    fn update_download_progress(&self, _progress: i32) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Dismiss download progress UI
    fn dismiss_download_progress(&self) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Show update confirmation prompt and invoke callback with the result.
    ///
    /// # Arguments
    /// * `callback_id` - Callback ID for result
    /// * `update_info_json` - Optional JSON string with update details: {"version":"1.2.0","size":15728640,"releaseNotes":["..."]}
    ///
    /// # Callback behavior
    /// - Confirm: callback success with payload (e.g. {"confirm":true})
    /// - Cancel: callback error code 2000
    fn show_update_prompt(
        &self,
        _callback_id: u64,
        _update_info_json: Option<&str>,
    ) -> Result<(), PlatformError> {
        Err(PlatformError::Platform(
            "show_update_prompt not implemented for this platform".to_string(),
        ))
    }

    /// Requests installation of an application update from a local package file.
    ///
    /// This launches the system installer and returns once the request is issued.
    /// It does not guarantee the update is installed successfully.
    ///
    /// # Arguments
    /// * `package_path` - Local, readable update package path (e.g. .apk on Android)
    ///
    /// # Platform Support / Notes
    /// - Android: Launches the system installer; requires user confirmation.
    ///   Requires `REQUEST_INSTALL_PACKAGES` and a `FileProvider` for APK sharing.
    /// - macOS: Planned support for .pkg/.dmg installers.
    /// - iOS: Not supported (App Store only).
    /// - HarmonyOS: Not implemented (returns error).
    fn install_update(&self, package_path: &Path) -> Result<(), PlatformError> {
        let _ = package_path;
        Err(PlatformError::Platform(
            "install_update not implemented for this platform".to_string(),
        ))
    }
}

pub trait AppRuntime:
    Send
    + Sync
    + MediaInteraction
    + MediaRuntime
    + PopupPresenter
    + Device
    + DeviceHardware
    + DeviceSecureStore
    + DocumentInteraction
    + Location
    + Permissions
    + UIUpdate
    + UpdateService
    + UserFeedback
    + Wifi
    + 'static
{
    /// Reads an asset file as a streaming reader.
    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, PlatformError>;
    /// Iterates over files in an asset directory.
    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, PlatformError>> + 'a>;
    /// Returns the app's data directory path.
    fn app_data_dir(&self) -> PathBuf;
    /// Returns the app's cache directory path.
    fn app_cache_dir(&self) -> PathBuf;
    /// Obtains the application identifier.
    fn get_app_identifier(&self) -> Result<String, PlatformError>;
    /// Copies media from the system album to a local file.
    fn copy_album_media_to_file(
        &self,
        uri: &str,
        dest_path: &Path,
        kind: MediaKind,
    ) -> Result<(), PlatformError> {
        MediaRuntime::copy_album_media_to_file(self, uri, dest_path, kind)
    }
    /// Returns the current system locale.
    fn get_system_locale(&self) -> &str;
    /// Show the UI container for the given LxApp and route.
    fn show_lxapp(&self, appid: String, path: String) -> Result<(), PlatformError>;
    /// Hide the UI container for the given LxApp (does not destroy its runtime state).
    fn hide_lxapp(&self, appid: String) -> Result<(), PlatformError>;
    /// Navigates within the given LxApp using an animation.
    fn navigate(
        &self,
        appid: String,
        path: String,
        animation_type: AnimationType,
    ) -> Result<(), PlatformError>;
    /// Launches the given URL in the host environment.
    fn launch_with_url(&self, url: String) -> Result<(), PlatformError>;
    /// Gets the capsule button bounding rect in screen coordinates.
    /// Returns result via callback with JSON string format: {"width": 84.5, "height": 32, "top": 50, "right": 375, "bottom": 82, "left": 290.5}
    /// All values are in pixels, relative to screen top-left corner (0, 0).
    /// Note: This only works when the page has showNavigationBar: false (webview is fullscreen).
    ///
    /// # Arguments
    /// * `callback_id` - The callback ID to invoke with the result
    ///
    /// # iOS/macOS
    /// Returns synchronously via callback immediately
    ///
    /// # Android/HarmonyOS
    /// Returns asynchronously via callback after querying native UI layer
    fn get_capsule_rect(&self, callback_id: u64) -> Result<(), PlatformError>;
}
