use std::io::Read;
use std::path::{Path, PathBuf};

use crate::AssetFileEntry;
use crate::error::PlatformError;

use super::device::{Device, DeviceHardware, DeviceSecureStore};
use super::document::DocumentInteraction;
use super::location::Location;
use super::media_interaction::{MediaInteraction, MediaKind};
use super::media_runtime::MediaRuntime;
use super::network::Network;
use super::ui::{PopupPresenter, UIUpdate, UserFeedback};
use super::update::UpdateService;
use super::wifi::Wifi;

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

pub trait AppRuntime:
    Send
    + Sync
    + MediaInteraction
    + MediaRuntime
    + Network
    + PopupPresenter
    + Device
    + DeviceHardware
    + DeviceSecureStore
    + DocumentInteraction
    + Location
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
    fn show_lxapp(&self, appid: String, path: String, session_id: u64)
    -> Result<(), PlatformError>;

    /// Hide the UI container for the given LxApp (does not destroy its runtime state).
    fn hide_lxapp(&self, appid: String, session_id: u64) -> Result<(), PlatformError>;

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
