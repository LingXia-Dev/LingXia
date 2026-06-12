use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};

use super::{Platform, file, not_supported};
use crate::error::PlatformError;
use crate::traits::media_interaction::{
    ChooseMediaRequest, MediaInteraction, PreviewMediaRequest, SaveMediaRequest, ScanCodeRequest,
};
use crate::traits::media_runtime::{
    CompressImageRequest, CompressVideoRequest, CompressedVideo, ExtractVideoThumbnailRequest,
    ImageInfo, MediaRuntime, VideoInfo, VideoThumbnail,
};

impl MediaInteraction for Platform {
    fn preview_media(&self, request: PreviewMediaRequest) -> Result<(), PlatformError> {
        super::media_preview::open_preview(request).map_err(PlatformError::Platform)
    }

    fn cancel_preview(&self, callback_id: u64) -> Result<(), PlatformError> {
        super::media_preview::cancel_preview(callback_id).map_err(PlatformError::Platform)
    }

    fn choose_media(
        &self,
        _request: ChooseMediaRequest,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async { not_supported("choose_media") }
    }

    fn scan_code(
        &self,
        request: ScanCodeRequest,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        crate::desktop::scan::scan_code_desktop(request)
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
        uri: &str,
        dest_path: &Path,
        _kind: crate::traits::media_interaction::MediaKind,
    ) -> Result<(), PlatformError> {
        let source = file::normalize_file_uri(uri)?;
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                PlatformError::Platform(format!(
                    "failed to create destination directory {}: {err}",
                    parent.display()
                ))
            })?;
        }
        fs::copy(&source, dest_path).map_err(|err| {
            PlatformError::Platform(format!(
                "failed to copy media {} -> {}: {err}",
                source.display(),
                dest_path.display()
            ))
        })?;
        Ok(())
    }

    fn get_image_info(&self, uri: &str) -> Result<ImageInfo, PlatformError> {
        crate::desktop::image::get_image_info_desktop(uri)
    }

    fn compress_image(&self, request: &CompressImageRequest) -> Result<PathBuf, PlatformError> {
        crate::desktop::image::compress_image_desktop(request)
    }

    fn compress_video(
        &self,
        _request: &CompressVideoRequest,
    ) -> Result<CompressedVideo, PlatformError> {
        not_supported("compress_video")
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
