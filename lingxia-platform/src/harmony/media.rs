use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::{
    ChooseMediaRequest, MediaInteraction, MediaKind, PreviewMediaRequest, SaveMediaRequest,
    ScanCodeRequest,
};
use serde::Serialize;

const MEDIA_LIBRARY_IMAGE_RESOURCE: i32 = 1;
const MEDIA_LIBRARY_VIDEO_RESOURCE: i32 = 2;

#[derive(Serialize)]
struct PreviewMediaPayload<'a> {
    path: &'a str,
    media_type: i32,
    cover_url: &'a str,
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
            .iter()
            .map(|item| PreviewMediaPayload {
                path: item.path.as_str(),
                media_type: match item.media_type {
                    MediaKind::Image => 0,
                    MediaKind::Video => 1,
                    MediaKind::Unknown => -1,
                },
                cover_url: item.cover_url.as_deref().unwrap_or_default(),
            })
            .collect();

        let json = serde_json::to_string(&payloads).map_err(|e| {
            PlatformError::Platform(format!("Failed to serialize preview media payload: {}", e))
        })?;

        let safe_json = json.replace('|', "%7C");

        lingxia_webview::tsfn::call_arkts("previewMedia", &[safe_json.as_str()])
            .map_err(|e| PlatformError::Platform(format!("Failed to preview media: {}", e)))
    }

    fn choose_media(&self, _request: ChooseMediaRequest) -> Result<(), PlatformError> {
        Err(PlatformError::Platform(
            "choose_media is not implemented on Harmony platform".to_string(),
        ))
    }

    fn scan_code(&self, _request: ScanCodeRequest) -> Result<(), PlatformError> {
        Err(PlatformError::Platform(
            "scan_code is not implemented on Harmony platform".to_string(),
        ))
    }

    fn save_image_to_photos_album(&self, request: SaveMediaRequest) -> Result<(), PlatformError> {
        save_media_resource(&request.file_uri, MEDIA_LIBRARY_IMAGE_RESOURCE)
    }

    fn save_video_to_photos_album(&self, request: SaveMediaRequest) -> Result<(), PlatformError> {
        save_media_resource(&request.file_uri, MEDIA_LIBRARY_VIDEO_RESOURCE)
    }
}

fn save_media_resource(file_uri: &str, resource_type: i32) -> Result<(), PlatformError> {
    let media_type_str = resource_type.to_string();
    lingxia_webview::tsfn::call_arkts("saveMedia", &[file_uri, &media_type_str])
        .map_err(|e| PlatformError::Platform(format!("Failed to save media: {}", e)))
}
