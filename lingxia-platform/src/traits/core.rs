use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error::PlatformError;
use crate::{AssetFileEntry, DeviceInfo, ScreenInfo};

use super::media_interaction::{MediaInteraction, MediaKind};
use super::media_runtime::MediaRuntime;

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
pub enum PickerType {
    SingleColumn {
        items: Vec<String>,
    },
    DualColumn {
        first_column: Vec<String>,
        second_column: Vec<String>,
    },
    DualColumnCascading {
        first_column: Vec<String>,
        cascading_data: HashMap<String, Vec<String>>,
    },
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
    #[allow(clippy::too_many_arguments)]
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

pub trait AppRuntime:
    Send
    + Sync
    + MediaInteraction
    + MediaRuntime
    + PopupPresenter
    + Device
    + DocumentInteraction
    + Location
    + UIUpdate
    + UserFeedback
    + 'static
{
    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, PlatformError>;
    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, PlatformError>> + 'a>;
    fn app_data_dir(&self) -> PathBuf;
    fn app_cache_dir(&self) -> PathBuf;
    fn copy_album_media_to_file(
        &self,
        uri: &str,
        dest_path: &Path,
        kind: MediaKind,
    ) -> Result<(), PlatformError> {
        MediaRuntime::copy_album_media_to_file(self, uri, dest_path, kind)
    }
    fn exit_app(&self) -> Result<(), PlatformError>;
    fn get_system_locale(&self) -> &str;
    /// Show the UI container for the given LxApp and route.
    fn show_lxapp(&self, appid: String, path: String) -> Result<(), PlatformError>;
    /// Hide the UI container for the given LxApp (does not destroy its runtime state).
    fn hide_lxapp(&self, appid: String) -> Result<(), PlatformError>;
    fn navigate(
        &self,
        appid: String,
        path: String,
        animation_type: AnimationType,
    ) -> Result<(), PlatformError>;
    fn launch_with_url(&self, url: String) -> Result<(), PlatformError>;
}
