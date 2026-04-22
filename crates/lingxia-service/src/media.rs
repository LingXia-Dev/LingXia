pub use lingxia_platform::PlatformError;
pub use lingxia_platform::traits::media_interaction::{
    CameraFacing, ChooseMediaMode, ChooseMediaRequest, MediaInteraction, MediaKind, MediaObjectFit,
    MediaQuality, MediaSource, PreviewMediaAdvance, PreviewMediaItem, PreviewMediaRequest,
    SaveMediaRequest, ScanCodeRequest, ScanType,
};

pub type Result<T> = std::result::Result<T, PlatformError>;

pub async fn choose_media(
    runtime: &(impl MediaInteraction + ?Sized),
    request: ChooseMediaRequest,
) -> Result<String> {
    runtime.choose_media(request).await
}

pub fn preview_media(
    runtime: &(impl MediaInteraction + ?Sized),
    request: PreviewMediaRequest,
) -> Result<()> {
    runtime.preview_media(request)
}

pub fn cancel_preview(runtime: &(impl MediaInteraction + ?Sized), callback_id: u64) -> Result<()> {
    runtime.cancel_preview(callback_id)
}

pub async fn scan_code(
    runtime: &(impl MediaInteraction + ?Sized),
    request: ScanCodeRequest,
) -> Result<String> {
    runtime.scan_code(request).await
}

pub async fn save_image_to_photos_album(
    runtime: &(impl MediaInteraction + ?Sized),
    request: SaveMediaRequest,
) -> Result<()> {
    runtime.save_image_to_photos_album(request).await
}

pub async fn save_video_to_photos_album(
    runtime: &(impl MediaInteraction + ?Sized),
    request: SaveMediaRequest,
) -> Result<()> {
    runtime.save_video_to_photos_album(request).await
}
