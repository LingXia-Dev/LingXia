use super::app::Platform;
use super::ffi::preview_media;
use crate::error::PlatformError;
use crate::traits::{ChooseMediaRequest, MediaInteraction, MediaKind, PreviewMediaRequest, SaveMediaRequest, ScanCodeRequest};
use serde::Serialize;

#[derive(Serialize)]
struct PreviewMediaPayload {
    path: String,
    media_type: i32,
    cover_url: String,
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
                cover_url: item.cover_url.unwrap_or_default(),
            })
            .collect();

        let items_json = serde_json::to_string(&payloads)
            .map_err(|e| PlatformError::Platform(format!("Failed to serialize media items: {}", e)))?;

        if preview_media(&items_json) {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to preview media on Apple platform".to_string(),
            ))
        }
    }

    fn choose_media(&self, _request: ChooseMediaRequest) -> Result<(), PlatformError> {
        Err(PlatformError::Platform(
            "choose_media is not implemented on Apple platform".to_string(),
        ))
    }

    fn scan_code(&self, _request: ScanCodeRequest) -> Result<(), PlatformError> {
        Err(PlatformError::Platform(
            "scan_code is not implemented on Apple platform".to_string(),
        ))
    }

    fn save_image_to_photos_album(&self, _request: SaveMediaRequest) -> Result<(), PlatformError> {
        Err(PlatformError::Platform(
            "save_image_to_photos_album is not implemented on Apple platform".to_string(),
        ))
    }

    fn save_video_to_photos_album(&self, _request: SaveMediaRequest) -> Result<(), PlatformError> {
        Err(PlatformError::Platform(
            "save_video_to_photos_album is not implemented on Apple platform".to_string(),
        ))
    }
}
