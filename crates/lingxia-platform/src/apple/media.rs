use super::app::Platform;
use super::ffi::cancel_preview_media;
use super::ffi::choose_media;
use super::ffi::preview_media;
use crate::error::PlatformError;
use crate::traits::media_interaction::{CameraFacing, ChooseMediaMode, MediaSource};
use crate::traits::media_interaction::{
    ChooseMediaRequest, MediaInteraction, MediaKind, MediaObjectFit, PreviewMediaRequest,
    SaveMediaRequest, ScanCodeRequest,
};
use crate::traits::media_runtime::{
    CompressImageRequest, CompressVideoRequest, ExtractVideoThumbnailRequest, ImageInfo,
    MediaRuntime, VideoInfo, VideoThumbnail,
};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct PreviewMediaPayload {
    path: String,
    media_type: i32,
    rotate: Option<u16>,
    object_fit: Option<String>,
    #[serde(rename = "durationMs")]
    duration_ms: Option<u64>,
}

#[derive(Serialize)]
struct PreviewMediaRequestPayload {
    sources: Vec<PreviewMediaPayload>,
    #[serde(rename = "startIndex")]
    start_index: i32,
    advance: &'static str,
    #[serde(rename = "showIndexIndicator")]
    show_index_indicator: bool,
}

impl MediaInteraction for Platform {
    fn preview_media(&self, request: PreviewMediaRequest) -> Result<(), PlatformError> {
        if request.items.is_empty() {
            return Err(PlatformError::Platform(
                "previewMedia requires at least one item".to_string(),
            ));
        }

        let payloads: Vec<PreviewMediaPayload> = request
            .items
            .into_iter()
            .map(|item| PreviewMediaPayload {
                path: item.path,
                media_type: match item.media_type {
                    MediaKind::Image => 0,
                    MediaKind::Video => 1,
                    MediaKind::Unknown => -1,
                },
                rotate: item.rotate,
                object_fit: item.object_fit.map(|fit| match fit {
                    MediaObjectFit::Cover => "cover".to_string(),
                    MediaObjectFit::Contain => "contain".to_string(),
                    MediaObjectFit::Fill => "fill".to_string(),
                    MediaObjectFit::Fit => "fit".to_string(),
                }),
                duration_ms: item.duration_ms,
            })
            .collect();

        let request_payload = PreviewMediaRequestPayload {
            sources: payloads,
            start_index: request.start_index,
            advance: request.advance.as_str(),
            show_index_indicator: request.show_index_indicator,
        };

        let items_json = serde_json::to_string(&request_payload).map_err(|e| {
            PlatformError::Platform(format!("Failed to serialize media items: {}", e))
        })?;

        if preview_media(
            &items_json,
            request.callback_id,
            request.presented_callback_id,
            request.change_callback_id,
        ) {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to preview media on Apple platform".to_string(),
            ))
        }
    }

    fn cancel_preview(&self, callback_id: u64) -> Result<(), PlatformError> {
        if cancel_preview_media(callback_id) {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to cancel preview media on Apple platform".to_string(),
            ))
        }
    }

    async fn choose_media(&self, request: ChooseMediaRequest) -> Result<String, PlatformError> {
        crate::rt::native_call(|callback_id| choose_media_impl(request, callback_id)).await
    }

    async fn scan_code(&self, request: ScanCodeRequest) -> Result<String, PlatformError> {
        #[cfg(target_os = "ios")]
        {
            crate::rt::native_call(|callback_id| ios::scan_code_impl(request, callback_id)).await
        }

        #[cfg(target_os = "macos")]
        {
            crate::desktop::scan::scan_code_desktop(request).await
        }
    }

    async fn save_image_to_photos_album(
        &self,
        request: SaveMediaRequest,
    ) -> Result<(), PlatformError> {
        #[cfg(target_os = "ios")]
        {
            crate::rt::blocking(move || ios::save_image_to_album(&request.file_uri)).await
        }

        #[cfg(not(target_os = "ios"))]
        {
            let _ = request;
            Err(PlatformError::Platform(
                "save_image_to_photos_album is only supported on iOS".to_string(),
            ))
        }
    }

    async fn save_video_to_photos_album(
        &self,
        request: SaveMediaRequest,
    ) -> Result<(), PlatformError> {
        #[cfg(target_os = "ios")]
        {
            crate::rt::blocking(move || ios::save_video_to_album(&request.file_uri)).await
        }

        #[cfg(not(target_os = "ios"))]
        {
            let _ = request;
            Err(PlatformError::Platform(
                "save_video_to_photos_album is only supported on iOS".to_string(),
            ))
        }
    }
}

impl MediaRuntime for Platform {
    fn copy_album_media_to_file(
        &self,
        uri: &str,
        dest_path: &Path,
        kind: MediaKind,
    ) -> Result<(), PlatformError> {
        #[cfg(target_os = "ios")]
        {
            let kind_code = match kind {
                MediaKind::Video => 1,
                _ => 0,
            };
            let dest_str = dest_path
                .to_str()
                .ok_or_else(|| {
                    PlatformError::Platform(format!(
                        "Destination path contains invalid UTF-8: {}",
                        dest_path.display()
                    ))
                })?
                .to_string();

            if super::ffi::copy_album_media_to_file(uri, &dest_str, kind_code) {
                Ok(())
            } else {
                Err(PlatformError::Platform(format!(
                    "Failed to copy media to {}",
                    dest_path.display()
                )))
            }
        }

        #[cfg(not(target_os = "ios"))]
        {
            let _ = uri;
            let _ = dest_path;
            let _ = kind;
            Err(PlatformError::Platform(
                "copy_album_media_to_file is only supported on iOS".to_string(),
            ))
        }
    }

    fn get_image_info(&self, uri: &str) -> Result<ImageInfo, PlatformError> {
        #[cfg(target_os = "ios")]
        {
            let info = super::ffi::get_image_info(uri);
            if !info.success {
                return Err(PlatformError::Platform(if info.error.is_empty() {
                    "get_image_info failed".to_string()
                } else {
                    info.error
                }));
            }
            Ok(ImageInfo {
                width: info.width,
                height: info.height,
                mime_type: if info.mime_type.is_empty() {
                    None
                } else {
                    Some(info.mime_type)
                },
            })
        }

        #[cfg(target_os = "macos")]
        {
            crate::desktop::image::get_image_info_desktop(uri)
        }
    }

    fn compress_image(&self, request: &CompressImageRequest) -> Result<PathBuf, PlatformError> {
        #[cfg(target_os = "ios")]
        {
            let output_path = request.output_path.to_string_lossy();
            let result = super::ffi::compress_image(
                &request.source_uri,
                request.quality as i32,
                request.max_width.unwrap_or(0) as i32,
                request.max_height.unwrap_or(0) as i32,
                output_path.as_ref(),
            );
            if !result.success || result.path.is_empty() {
                return Err(PlatformError::Platform(if result.error.is_empty() {
                    "compress_image failed".to_string()
                } else {
                    result.error
                }));
            }
            Ok(PathBuf::from(result.path))
        }

        #[cfg(target_os = "macos")]
        {
            crate::desktop::image::compress_image_desktop(request)
        }
    }

    fn get_video_info(&self, uri: &str) -> Result<VideoInfo, PlatformError> {
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            let info = super::ffi::get_video_info(uri);
            if !info.success {
                return Err(PlatformError::Platform(if info.error.is_empty() {
                    "get_video_info failed".to_string()
                } else {
                    info.error
                }));
            }
            Ok(VideoInfo {
                width: info.width,
                height: info.height,
                duration_ms: info.duration_ms,
                rotation: if info.has_rotation && info.rotation >= 0 {
                    Some(info.rotation as u16)
                } else {
                    None
                },
                bitrate: if info.has_bitrate {
                    Some(info.bitrate)
                } else {
                    None
                },
                fps: if info.has_fps { Some(info.fps) } else { None },
                mime_type: if info.mime_type.is_empty() {
                    None
                } else {
                    Some(info.mime_type)
                },
            })
        }
    }

    fn extract_video_thumbnail(
        &self,
        request: &ExtractVideoThumbnailRequest,
    ) -> Result<VideoThumbnail, PlatformError> {
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            let output_path = request.output_path.to_string_lossy();
            let result = super::ffi::extract_video_thumbnail(
                &request.source_uri,
                request.quality as i32,
                request.max_width.unwrap_or(0) as i32,
                request.max_height.unwrap_or(0) as i32,
                request.time_ms.map(|v| v as i64).unwrap_or(-1),
                output_path.as_ref(),
            );
            if !result.success || result.path.is_empty() {
                return Err(PlatformError::Platform(if result.error.is_empty() {
                    "extract_video_thumbnail failed".to_string()
                } else {
                    result.error
                }));
            }

            Ok(VideoThumbnail {
                path: PathBuf::from(result.path),
                width: result.width,
                height: result.height,
                mime_type: if result.mime_type.is_empty() {
                    None
                } else {
                    Some(result.mime_type)
                },
            })
        }
    }

    fn compress_video(&self, request: &CompressVideoRequest) -> Result<(), PlatformError> {
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            let output_path = request.output_path.to_string_lossy();
            let quality = request.quality.map(|v| match v {
                crate::traits::media_runtime::VideoCompressQuality::Low => "low",
                crate::traits::media_runtime::VideoCompressQuality::Medium => "medium",
                crate::traits::media_runtime::VideoCompressQuality::High => "high",
            });

            if super::ffi::compress_video(
                &request.source_uri,
                quality,
                request.bitrate_kbps.unwrap_or(0),
                request.fps.unwrap_or(0),
                request.resolution_ratio.unwrap_or(0.0f32),
                output_path.as_ref(),
                request.progress_callback_id,
                request.callback_id,
            ) {
                Ok(())
            } else {
                Err(PlatformError::Platform(
                    "compress_video failed to start".to_string(),
                ))
            }
        }
    }

    fn cancel_compress_video(&self, callback_id: u64) -> Result<(), PlatformError> {
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            if super::ffi::cancel_compress_video(callback_id) {
                Ok(())
            } else {
                Err(PlatformError::Platform(
                    "cancel_compress_video: no running job".to_string(),
                ))
            }
        }
    }
}

fn choose_media_impl(request: ChooseMediaRequest, callback_id: u64) -> Result<(), PlatformError> {
    let max_count = request.max_count;
    let mode = match request.mode {
        ChooseMediaMode::Images => "image",
        ChooseMediaMode::Videos => "video",
        ChooseMediaMode::Mix => "mix",
    };
    let source_types: Vec<String> = request
        .source_types
        .iter()
        .map(|s| match s {
            MediaSource::Album => "album".to_string(),
            MediaSource::Camera => "camera".to_string(),
        })
        .collect();
    let camera_facing = request.camera_facing.map(|c| match c {
        CameraFacing::Front => "front",
        CameraFacing::Back => "back",
    });
    let max_duration = request.max_duration_seconds;

    let source_types_json = serde_json::to_string(&source_types)
        .map_err(|e| PlatformError::Platform(format!("Failed to serialize source types: {}", e)))?;

    let camera_facing_str = camera_facing.unwrap_or("back");

    let success = choose_media(
        max_count,
        mode,
        &source_types_json,
        camera_facing_str,
        max_duration,
        callback_id,
    );
    if success {
        Ok(())
    } else {
        Err(PlatformError::Platform(
            "Failed to start media selection on Apple platform".to_string(),
        ))
    }
}

#[cfg(target_os = "ios")]
mod ios {
    use super::*;
    use crate::apple::ffi::scan_code;
    use crate::traits::media_interaction::ScanType;
    use block2::RcBlock;
    use dispatch2::{DispatchSemaphore, DispatchTime, dispatch_block_t, run_on_main};
    use objc2_foundation::{NSError, NSString, NSURL};
    use objc2_photos::{
        PHAccessLevel, PHAssetCreationRequest, PHAuthorizationStatus, PHPhotoLibrary,
    };
    use std::sync::{Arc, Mutex};

    const PHOTOS_PERMISSION_DENIED: &str = "Photos permission denied";

    pub(super) fn save_image_to_album(file_uri: &str) -> Result<(), PlatformError> {
        let path = resolve_file_uri(file_uri)?;
        run_on_main(|_| save_image_on_main(&path))
    }

    pub(super) fn save_video_to_album(file_uri: &str) -> Result<(), PlatformError> {
        let path = resolve_file_uri(file_uri)?;
        run_on_main(|_| save_video_on_main(&path))
    }

    fn save_image_on_main(path: &str) -> Result<(), PlatformError> {
        ensure_photo_authorization()?;

        let ns_path = NSString::from_str(path);
        let file_url = NSURL::fileURLWithPath(&ns_path);

        let error_holder = Arc::new(Mutex::new(None::<String>));
        let error_clone = Arc::clone(&error_holder);
        let change_block = RcBlock::<dyn Fn()>::new({
            let file_url = file_url.clone();
            move || {
                let created = unsafe {
                    PHAssetCreationRequest::creationRequestForAssetFromImageAtFileURL(&file_url)
                };
                if created.is_none()
                    && let Ok(mut guard) = error_clone.lock()
                {
                    *guard = Some("Failed to create photo asset from image file".to_string());
                }
            }
        });

        perform_photo_library_change(&change_block)?;

        if let Some(message) = error_holder.lock().ok().and_then(|guard| guard.clone()) {
            return Err(PlatformError::Platform(message));
        }

        Ok(())
    }

    fn save_video_on_main(path: &str) -> Result<(), PlatformError> {
        ensure_photo_authorization()?;

        let ns_path = NSString::from_str(path);
        let file_url = NSURL::fileURLWithPath(&ns_path);

        let error_holder = Arc::new(Mutex::new(None::<String>));
        let error_clone = Arc::clone(&error_holder);
        let change_block = RcBlock::<dyn Fn()>::new({
            let file_url = file_url.clone();
            move || {
                let created = unsafe {
                    PHAssetCreationRequest::creationRequestForAssetFromVideoAtFileURL(&file_url)
                };
                if created.is_none()
                    && let Ok(mut guard) = error_clone.lock()
                {
                    *guard = Some("Failed to create photo asset from video file".to_string());
                }
            }
        });

        perform_photo_library_change(&change_block)?;

        if let Some(message) = error_holder.lock().ok().and_then(|guard| guard.clone()) {
            return Err(PlatformError::Platform(message));
        }

        Ok(())
    }

    fn ensure_photo_authorization() -> Result<(), PlatformError> {
        let access_level = PHAccessLevel::AddOnly;
        let initial_status =
            unsafe { PHPhotoLibrary::authorizationStatusForAccessLevel(access_level) };
        if is_authorized(initial_status) {
            return Ok(());
        }

        if matches!(
            initial_status,
            PHAuthorizationStatus::Denied | PHAuthorizationStatus::Restricted
        ) {
            return Err(PlatformError::Platform(
                PHOTOS_PERMISSION_DENIED.to_string(),
            ));
        }

        let semaphore = DispatchSemaphore::new(0);
        let status_holder = Arc::new(Mutex::new(None));
        let status_clone = Arc::clone(&status_holder);
        let semaphore_clone = semaphore.clone();

        let block = RcBlock::new(move |status: PHAuthorizationStatus| {
            if let Ok(mut guard) = status_clone.lock() {
                *guard = Some(status);
            }
            let _ = semaphore_clone.signal();
        });

        unsafe {
            PHPhotoLibrary::requestAuthorizationForAccessLevel_handler(access_level, &block);
        }

        semaphore.wait(DispatchTime::FOREVER);

        let status = status_holder
            .lock()
            .ok()
            .and_then(|guard| *guard)
            .unwrap_or(PHAuthorizationStatus::Denied);

        if is_authorized(status) {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                PHOTOS_PERMISSION_DENIED.to_string(),
            ))
        }
    }

    fn is_authorized(status: PHAuthorizationStatus) -> bool {
        matches!(
            status,
            PHAuthorizationStatus::Authorized | PHAuthorizationStatus::Limited
        )
    }

    fn resolve_file_uri(input: &str) -> Result<String, PlatformError> {
        if let Some(stripped) = input.strip_prefix("file://") {
            Ok(stripped.to_string())
        } else {
            Ok(input.to_string())
        }
    }

    fn perform_photo_library_change(block: &RcBlock<dyn Fn()>) -> Result<(), PlatformError> {
        let library = unsafe { PHPhotoLibrary::sharedPhotoLibrary() };
        let block_ptr = RcBlock::as_ptr(block) as dispatch_block_t;
        unsafe {
            library
                .performChangesAndWait_error(block_ptr)
                .map_err(|err| {
                    PlatformError::Platform(format!(
                        "Photo library change failed: {}",
                        ns_error_to_string(&err)
                    ))
                })
        }
    }

    fn ns_error_to_string(error: &NSError) -> String {
        error.localizedDescription().to_string()
    }

    pub(super) fn scan_code_impl(
        request: ScanCodeRequest,
        callback_id: u64,
    ) -> Result<(), PlatformError> {
        let type_codes: Vec<i32> = request
            .scan_types
            .iter()
            .map(|t| match t {
                ScanType::QrCode => 1,
                ScanType::BarCode => 2,
                ScanType::DataMatrix => 3,
                ScanType::Pdf417 => 4,
            })
            .collect();

        let types_json = serde_json::to_string(&type_codes)
            .map_err(|e| PlatformError::Platform(format!("Failed to encode scan types: {}", e)))?;

        let started = scan_code(&types_json, request.only_from_camera, callback_id);
        if started {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to initiate scanCode on Apple platform".to_string(),
            ))
        }
    }
}
