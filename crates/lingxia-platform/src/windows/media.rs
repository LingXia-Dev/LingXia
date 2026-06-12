use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};

use super::{Platform, file, not_supported};
use crate::error::PlatformError;
use crate::traits::media_interaction::{
    ChooseMediaMode, ChooseMediaRequest, MediaInteraction, MediaSource, PreviewMediaRequest,
    SaveMediaRequest, ScanCodeRequest,
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
        request: ChooseMediaRequest,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async move {
            if !request.source_types.contains(&MediaSource::Album) {
                return not_supported("choose_media from camera");
            }
            let handle = crate::rt::spawn_blocking(move || pick_media_files(&request));
            match handle {
                Some(task) => task.await.map_err(|err| {
                    PlatformError::Platform(format!("choose_media task panicked: {err}"))
                })?,
                None => Err(PlatformError::Platform(
                    "choose_media: async runtime not initialized".into(),
                )),
            }
        }
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

const IMAGE_EXTENSIONS: [&str; 8] = ["png", "jpg", "jpeg", "gif", "bmp", "webp", "tif", "tiff"];
const VIDEO_EXTENSIONS: [&str; 7] = ["mp4", "mov", "avi", "mkv", "webm", "m4v", "wmv"];

/// Album picking on Windows is a file dialog over the media types; the
/// chosen paths go back as the `[{uri, fileType, isOriginal}]` array the
/// logic layer copies into the app cache. Cancel yields an empty list.
fn pick_media_files(request: &ChooseMediaRequest) -> Result<String, PlatformError> {
    let dialog = rfd::FileDialog::new();
    let dialog = match request.mode {
        ChooseMediaMode::Images => dialog
            .set_title("Choose Images")
            .add_filter("Images", &IMAGE_EXTENSIONS),
        ChooseMediaMode::Videos => dialog
            .set_title("Choose Videos")
            .add_filter("Videos", &VIDEO_EXTENSIONS),
        ChooseMediaMode::Mix => {
            let mut all: Vec<&str> = Vec::new();
            all.extend_from_slice(&IMAGE_EXTENSIONS);
            all.extend_from_slice(&VIDEO_EXTENSIONS);
            dialog
                .set_title("Choose Media")
                .add_filter("Media", &all)
                .add_filter("Images", &IMAGE_EXTENSIONS)
                .add_filter("Videos", &VIDEO_EXTENSIONS)
        }
    };
    let picked = if request.max_count > 1 {
        dialog.pick_files().unwrap_or_default()
    } else {
        dialog.pick_file().map(|path| vec![path]).unwrap_or_default()
    };
    let entries: Vec<serde_json::Value> = picked
        .into_iter()
        .take(request.max_count.max(1) as usize)
        .map(|path| {
            let ext = path
                .extension()
                .map(|ext| ext.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let kind = if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
                "video"
            } else {
                "image"
            };
            serde_json::json!({
                "uri": path.to_string_lossy(),
                "fileType": kind,
                "isOriginal": true,
                "fileExt": ext,
            })
        })
        .collect();
    serde_json::to_string(&entries)
        .map_err(|err| PlatformError::Platform(format!("choose_media: {err}")))
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

    fn get_video_info(&self, uri: &str) -> Result<VideoInfo, PlatformError> {
        let path = file::normalize_file_uri(uri)?;
        super::video_info::read_video_info(&path)
    }

    fn extract_video_thumbnail(
        &self,
        request: &ExtractVideoThumbnailRequest,
    ) -> Result<VideoThumbnail, PlatformError> {
        let source = file::normalize_file_uri(&request.source_uri)?;
        super::video_info::extract_thumbnail(request, &source).inspect_err(|err| {
            log::warn!("extract_video_thumbnail({}): {err}", source.display());
        })
    }
}
