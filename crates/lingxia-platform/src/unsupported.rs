#![allow(clippy::manual_async_fn)]

use std::future::Future;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error::PlatformError;
use crate::traits::app_runtime::{AnimationType, AppRuntime, LxAppOpenMode, OpenUrlRequest};
use crate::traits::device::{Device, DeviceHardware};
use crate::traits::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogResult, FileService, OpenFileRequest,
    RevealInFileManagerRequest,
};
use crate::traits::location::{Location, LocationRequestConfig};
use crate::traits::media_interaction::{
    ChooseMediaRequest, MediaInteraction, PreviewMediaRequest, SaveMediaRequest, ScanCodeRequest,
};
use crate::traits::media_runtime::{
    CompressImageRequest, CompressVideoRequest, CompressedVideo, ExtractVideoThumbnailRequest,
    ImageInfo, MediaRuntime, VideoInfo, VideoThumbnail,
};
use crate::traits::mouse::AppMouse;
use crate::traits::network::Network;
use crate::traits::pull_to_refresh::PullToRefresh;
use crate::traits::screenshot::AppScreenshot;
use crate::traits::secure_store::SecureStore;
use crate::traits::share::{ShareRequest, ShareResult, ShareService};
use crate::traits::stream_decoder::{VideoStreamDecoderHandle, VideoStreamDecoderManager};
use crate::traits::ui::{ModalOptions, SurfacePresenter, ToastOptions, UIUpdate, UserFeedback};
use crate::traits::update::UpdateService;
use crate::traits::video_player::{VideoPlayerHandle, VideoPlayerManager};
use crate::traits::wifi::Wifi;
use crate::{AssetFileEntry, DeviceInfo, ScreenInfo};

#[derive(Debug, Clone)]
pub struct Platform {
    data_dir: PathBuf,
    cache_dir: PathBuf,
    locale: String,
}

impl Default for Platform {
    fn default() -> Self {
        let base = std::env::temp_dir().join("lingxia");
        Self {
            data_dir: base.join("data"),
            cache_dir: base.join("cache"),
            locale: "en-US".to_string(),
        }
    }
}

impl Platform {
    pub fn new(
        data_dir: impl Into<PathBuf>,
        cache_dir: impl Into<PathBuf>,
        locale: impl Into<String>,
    ) -> Result<Self, PlatformError> {
        Ok(Self {
            data_dir: data_dir.into(),
            cache_dir: cache_dir.into(),
            locale: locale.into(),
        })
    }
}

fn not_supported<T>(name: &str) -> Result<T, PlatformError> {
    Err(PlatformError::NotSupported(format!(
        "{name} is not supported on this platform"
    )))
}

impl Device for Platform {
    fn device_info(&self) -> DeviceInfo {
        DeviceInfo {
            brand: "unsupported".to_string(),
            model: std::env::consts::OS.to_string(),
            market_name: std::env::consts::OS.to_string(),
            os_name: std::env::consts::OS.to_string(),
            os_version: String::new(),
        }
    }

    fn screen_info(&self) -> ScreenInfo {
        ScreenInfo {
            width: 0.0,
            height: 0.0,
            scale: 1.0,
        }
    }

    fn vibrate(&self, _long: bool) -> Result<(), PlatformError> {
        not_supported("vibrate")
    }

    fn make_phone_call(&self, _phone_number: &str) -> Result<(), PlatformError> {
        not_supported("make_phone_call")
    }
}

impl DeviceHardware for Platform {}
impl SecureStore for Platform {}
impl Network for Platform {}
impl SurfacePresenter for Platform {}
impl UpdateService for Platform {}
impl Wifi for Platform {}
impl AppScreenshot for Platform {}
impl AppMouse for Platform {}

impl FileService for Platform {
    fn review_file(
        &self,
        _request: OpenFileRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async { not_supported("review_file") }
    }

    fn open_external(
        &self,
        _request: OpenFileRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async { not_supported("open_external") }
    }

    fn reveal_in_file_manager(
        &self,
        _request: RevealInFileManagerRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async { not_supported("reveal_in_file_manager") }
    }

    fn choose_file(
        &self,
        _request: ChooseFileRequest,
    ) -> impl Future<Output = Result<FileDialogResult, PlatformError>> + Send {
        async { not_supported("choose_file") }
    }

    fn choose_directory(
        &self,
        _request: ChooseDirectoryRequest,
    ) -> impl Future<Output = Result<FileDialogResult, PlatformError>> + Send {
        async { not_supported("choose_directory") }
    }
}

impl Location for Platform {
    fn is_location_enabled(&self) -> Result<bool, PlatformError> {
        not_supported("is_location_enabled")
    }

    fn request_location(
        &self,
        _config: LocationRequestConfig,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async { not_supported("request_location") }
    }
}

impl MediaInteraction for Platform {
    fn preview_media(&self, _request: PreviewMediaRequest) -> Result<(), PlatformError> {
        not_supported("preview_media")
    }

    fn cancel_preview(&self, _callback_id: u64) -> Result<(), PlatformError> {
        not_supported("cancel_preview")
    }

    fn choose_media(
        &self,
        _request: ChooseMediaRequest,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async { not_supported("choose_media") }
    }

    fn scan_code(
        &self,
        _request: ScanCodeRequest,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async { not_supported("scan_code") }
    }

    fn save_image_to_photos_album(
        &self,
        _request: SaveMediaRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async { not_supported("save_image_to_photos_album") }
    }

    fn save_video_to_photos_album(
        &self,
        _request: SaveMediaRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async { not_supported("save_video_to_photos_album") }
    }
}

impl MediaRuntime for Platform {
    fn copy_album_media_to_file(
        &self,
        _uri: &str,
        _dest_path: &Path,
        _kind: crate::traits::media_interaction::MediaKind,
    ) -> Result<(), PlatformError> {
        not_supported("copy_album_media_to_file")
    }

    fn get_image_info(&self, _uri: &str) -> Result<ImageInfo, PlatformError> {
        not_supported("get_image_info")
    }

    fn compress_image(&self, _request: &CompressImageRequest) -> Result<PathBuf, PlatformError> {
        not_supported("compress_image")
    }

    fn compress_video(&self, _request: &CompressVideoRequest) -> Result<(), PlatformError> {
        not_supported("compress_video")
    }

    fn cancel_compress_video(&self, _callback_id: u64) -> Result<(), PlatformError> {
        not_supported("cancel_compress_video")
    }

    fn get_video_info(&self, _uri: &str) -> Result<VideoInfo, PlatformError> {
        not_supported("get_video_info")
    }

    fn extract_video_thumbnail(
        &self,
        _request: &ExtractVideoThumbnailRequest,
    ) -> Result<VideoThumbnail, PlatformError> {
        not_supported("extract_video_thumbnail")
    }
}

impl ShareService for Platform {
    fn share(
        &self,
        _request: ShareRequest,
    ) -> impl Future<Output = Result<ShareResult, PlatformError>> + Send {
        async { not_supported("share") }
    }
}

impl UIUpdate for Platform {
    fn update_navbar_ui(&self, _appid: String) -> Result<(), PlatformError> {
        not_supported("update_navbar_ui")
    }

    fn update_tabbar_ui(&self, _appid: String) -> Result<(), PlatformError> {
        not_supported("update_tabbar_ui")
    }
}

impl UserFeedback for Platform {
    fn show_toast(&self, _options: ToastOptions) -> Result<(), PlatformError> {
        not_supported("show_toast")
    }

    fn hide_toast(&self) -> Result<(), PlatformError> {
        not_supported("hide_toast")
    }

    fn show_modal(
        &self,
        _options: ModalOptions,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async { not_supported("show_modal") }
    }

    fn show_action_sheet(
        &self,
        _options: Vec<String>,
        _cancel_text: String,
        _item_color: String,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async { not_supported("show_action_sheet") }
    }
}

impl PullToRefresh for Platform {
    fn start_pull_down_refresh(&self, _app_id: &str, _path: &str) -> Result<(), PlatformError> {
        not_supported("start_pull_down_refresh")
    }

    fn stop_pull_down_refresh(&self, _app_id: &str, _path: &str) -> Result<(), PlatformError> {
        not_supported("stop_pull_down_refresh")
    }
}

impl VideoPlayerManager for Platform {
    fn bind_player(
        &self,
        _component_id: &str,
    ) -> Result<Box<dyn VideoPlayerHandle>, PlatformError> {
        not_supported("bind_player")
    }
}

impl VideoStreamDecoderManager for Platform {
    fn create_stream_decoder(
        &self,
        _component_id: &str,
    ) -> Result<Box<dyn VideoStreamDecoderHandle>, PlatformError> {
        not_supported("create_stream_decoder")
    }
}

impl AppRuntime for Platform {
    fn read_asset<'a>(&'a self, _path: &str) -> Result<Box<dyn Read + 'a>, PlatformError> {
        not_supported("read_asset")
    }

    fn asset_dir_iter<'a>(
        &'a self,
        _asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, PlatformError>> + 'a> {
        Box::new(std::iter::once(not_supported("asset_dir_iter")))
    }

    fn app_data_dir(&self) -> PathBuf {
        self.data_dir.clone()
    }

    fn app_cache_dir(&self) -> PathBuf {
        self.cache_dir.clone()
    }

    fn get_app_identifier(&self) -> Result<String, PlatformError> {
        not_supported("get_app_identifier")
    }

    fn get_system_locale(&self) -> &str {
        &self.locale
    }

    fn show_lxapp(
        &self,
        _appid: String,
        _path: String,
        _session_id: u64,
        _open_mode: LxAppOpenMode,
        _panel_id: String,
    ) -> Result<(), PlatformError> {
        not_supported("show_lxapp")
    }

    fn hide_lxapp(&self, _appid: String, _session_id: u64) -> Result<(), PlatformError> {
        not_supported("hide_lxapp")
    }

    fn exit(&self) -> Result<(), PlatformError> {
        not_supported("exit")
    }

    fn navigate(
        &self,
        _appid: String,
        _path: String,
        _animation_type: AnimationType,
    ) -> Result<(), PlatformError> {
        not_supported("navigate")
    }

    fn open_url(&self, _req: OpenUrlRequest) -> Result<(), PlatformError> {
        not_supported("open_url")
    }

    fn get_capsule_rect(&self) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async { not_supported("get_capsule_rect") }
    }
}
