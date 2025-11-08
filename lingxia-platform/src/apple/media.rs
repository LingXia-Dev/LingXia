use super::app::Platform;
use super::ffi::preview_media;
use crate::error::PlatformError;
use crate::traits::{
    ChooseMediaRequest, MediaInteraction, MediaKind, PreviewMediaRequest, SaveMediaRequest,
    ScanCodeRequest,
};
use serde::Serialize;

#[derive(Serialize)]
struct PreviewMediaPayload {
    path: String,
    media_type: i32,
    cover_path: String,
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
                cover_path: item.cover_path.unwrap_or_default(),
            })
            .collect();

        let items_json = serde_json::to_string(&payloads).map_err(|e| {
            PlatformError::Platform(format!("Failed to serialize media items: {}", e))
        })?;

        if preview_media(&items_json) {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to preview media on Apple platform".to_string(),
            ))
        }
    }

    fn choose_media(&self, request: ChooseMediaRequest) -> Result<(), PlatformError> {
        #[cfg(target_os = "ios")]
        {
            ios::choose_media_impl(request)
        }

        #[cfg(not(target_os = "ios"))]
        {
            let _ = request;
            Err(PlatformError::Platform(
                "choose_media is only supported on iOS".to_string(),
            ))
        }
    }

    fn scan_code(&self, request: ScanCodeRequest) -> Result<(), PlatformError> {
        #[cfg(target_os = "ios")]
        {
            ios::scan_code_impl(request)
        }

        #[cfg(not(target_os = "ios"))]
        {
            let _ = request;
            Err(PlatformError::Platform(
                "scan_code is not implemented on this Apple target".to_string(),
            ))
        }
    }

    fn save_image_to_photos_album(&self, request: SaveMediaRequest) -> Result<(), PlatformError> {
        #[cfg(target_os = "ios")]
        {
            ios::save_image_to_album(&request.file_uri)
        }

        #[cfg(not(target_os = "ios"))]
        {
            let _ = request;
            Err(PlatformError::Platform(
                "save_image_to_photos_album is only supported on iOS".to_string(),
            ))
        }
    }

    fn save_video_to_photos_album(&self, request: SaveMediaRequest) -> Result<(), PlatformError> {
        #[cfg(target_os = "ios")]
        {
            ios::save_video_to_album(&request.file_uri)
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

#[cfg(target_os = "ios")]
#[cfg(target_os = "ios")]
mod ios {
    use super::*;
    use crate::apple::ffi::{choose_media, scan_code};
    use crate::traits::ScanType;
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
                if created.is_none() {
                    if let Ok(mut guard) = error_clone.lock() {
                        *guard = Some("Failed to create photo asset from image file".to_string());
                    }
                }
            }
        });

        perform_photo_library_change(&change_block)?;

        if let Some(message) = error_holder.lock().ok().and_then(|guard| guard.clone()) {
            return Err(PlatformError::Platform(message));
        }

        // log::info!("Saved image to Photos album: {}", path);
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
                if created.is_none() {
                    if let Ok(mut guard) = error_clone.lock() {
                        *guard = Some("Failed to create photo asset from video file".to_string());
                    }
                }
            }
        });

        perform_photo_library_change(&change_block)?;

        if let Some(message) = error_holder.lock().ok().and_then(|guard| guard.clone()) {
            return Err(PlatformError::Platform(message));
        }

        // log::info!("Saved video to Photos album: {}", path);
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

    /// Initiate media selection process on iOS
    pub(super) fn choose_media_impl(request: ChooseMediaRequest) -> Result<(), PlatformError> {
        let max_count = request.max_count;
        let mode = match request.mode {
            crate::traits::ChooseMediaMode::Images => "image",
            crate::traits::ChooseMediaMode::Videos => "video",
            crate::traits::ChooseMediaMode::Mix => "mix",
        };
        let source_types: Vec<String> = request
            .source_types
            .iter()
            .map(|s| match s {
                crate::traits::MediaSource::Album => "album".to_string(),
                crate::traits::MediaSource::Camera => "camera".to_string(),
            })
            .collect();
        let camera_facing = request.camera_facing.map(|c| match c {
            crate::traits::CameraFacing::Front => "front",
            crate::traits::CameraFacing::Back => "back",
        });
        let max_duration = request.max_duration_seconds;
        let callback_id = request.callback_id;

        let source_types_json = serde_json::to_string(&source_types).map_err(|e| {
            PlatformError::Platform(format!("Failed to serialize source types: {}", e))
        })?;

        let camera_facing_str = camera_facing.unwrap_or_else(|| "back");
        let max_duration_str = max_duration
            .map(|d| d.to_string())
            .unwrap_or_else(|| "0".to_string());
        run_on_main(|_| {
            start_choose_media(
                max_count,
                mode,
                &source_types_json,
                &camera_facing_str,
                &max_duration_str,
                callback_id,
            )
        })
    }

    fn start_choose_media(
        max_count: u32,
        mode: &str,
        source_types_json: &str,
        camera_facing: &str,
        max_duration: &str,
        callback_id: u64,
    ) -> Result<(), PlatformError> {
        let success = choose_media(
            max_count,
            mode,
            source_types_json,
            camera_facing,
            max_duration,
            callback_id,
        );

        if success {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to start media selection on iOS".to_string(),
            ))
        }
    }

    pub(super) fn scan_code_impl(request: ScanCodeRequest) -> Result<(), PlatformError> {
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

        let started = scan_code(&types_json, request.only_from_camera, request.callback_id);
        if started {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to initiate scanCode on Apple platform".to_string(),
            ))
        }
    }
}
