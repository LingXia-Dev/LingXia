pub use lingxia_media::{
    FrameSink, StreamError, StreamProvider, StreamSession, get_stream_provider,
    register_stream_provider, register_stream_seek_callback, seek_stream_session,
    unregister_stream_seek_callback,
};
pub use lingxia_service::media::{
    CameraFacing, ChooseMediaMode, ChooseMediaRequest, MediaKind, MediaObjectFit, MediaQuality,
    MediaSource, PreviewMediaAdvance, PreviewMediaItem, PreviewMediaRequest, SaveMediaRequest,
    ScanCodeRequest, ScanType,
};

pub async fn choose_media(
    app: &crate::LxApp,
    request: ChooseMediaRequest,
) -> crate::Result<String> {
    lingxia_service::media::choose_media(&*app.runtime, request)
        .await
        .map_err(Into::into)
}

pub fn preview_media(app: &crate::LxApp, request: PreviewMediaRequest) -> crate::Result<()> {
    lingxia_service::media::preview_media(&*app.runtime, request).map_err(Into::into)
}

pub fn cancel_preview(app: &crate::LxApp, callback_id: u64) -> crate::Result<()> {
    lingxia_service::media::cancel_preview(&*app.runtime, callback_id).map_err(Into::into)
}

pub async fn scan_code(app: &crate::LxApp, request: ScanCodeRequest) -> crate::Result<String> {
    lingxia_service::media::scan_code(&*app.runtime, request)
        .await
        .map_err(Into::into)
}

pub async fn save_image_to_photos_album(
    app: &crate::LxApp,
    request: SaveMediaRequest,
) -> crate::Result<()> {
    lingxia_service::media::save_image_to_photos_album(&*app.runtime, request)
        .await
        .map_err(Into::into)
}

pub async fn save_video_to_photos_album(
    app: &crate::LxApp,
    request: SaveMediaRequest,
) -> crate::Result<()> {
    lingxia_service::media::save_video_to_photos_album(&*app.runtime, request)
        .await
        .map_err(Into::into)
}
