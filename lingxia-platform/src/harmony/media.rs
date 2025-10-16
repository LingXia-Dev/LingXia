use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::{
    ChooseMediaMode, ChooseMediaRequest, MediaInteraction, MediaKind, MediaSource,
    PreviewMediaRequest, SaveMediaRequest, ScanCodeRequest,
};
use serde::Serialize;

const MEDIA_LIBRARY_IMAGE_RESOURCE: i32 = 1;
const MEDIA_LIBRARY_VIDEO_RESOURCE: i32 = 2;

#[derive(Serialize)]
struct PreviewMediaPayload<'a> {
    path: &'a str,
    media_type: i32,
    cover_path: &'a str,
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
                cover_path: item.cover_path.as_deref().unwrap_or_default(),
            })
            .collect();

        let json = serde_json::to_string(&payloads).map_err(|e| {
            PlatformError::Platform(format!("Failed to serialize preview media payload: {}", e))
        })?;

        let safe_json = json.replace('|', "%7C");

        lingxia_webview::tsfn::call_arkts("previewMedia", &[safe_json.as_str()])
            .map_err(|e| PlatformError::Platform(format!("Failed to preview media: {}", e)))
    }

    fn choose_media(&self, request: ChooseMediaRequest) -> Result<(), PlatformError> {
        if request.max_count == 0 {
            return Err(PlatformError::Platform(
                "chooseMedia requires max_count to be greater than 0".to_string(),
            ));
        }

        let mode_str = match request.mode {
            ChooseMediaMode::Images => "images",
            ChooseMediaMode::Videos => "videos",
            ChooseMediaMode::Mix => "mix",
        };

        let allow_album = request
            .source_types
            .iter()
            .any(|source| matches!(source, MediaSource::Album));

        let payload = ChooseMediaPayload {
            callback_id: request.callback_id.to_string(),
            max_count: request.max_count,
            allow_original: request.allow_original,
            allow_compressed: request.allow_compressed,
            mode: mode_str.to_string(),
            allow_album,
            allow_camera: request
                .source_types
                .iter()
                .any(|source| matches!(source, MediaSource::Camera)),
            max_duration_seconds: None,
            camera_facing: None,
        };

        // Attach optional duration and facing
        let payload = ChooseMediaPayload {
            max_duration_seconds: request.max_duration_seconds,
            camera_facing: request.camera_facing.as_ref().map(|f| match f {
                crate::traits::CameraFacing::Front => "front".to_string(),
                crate::traits::CameraFacing::Back => "back".to_string(),
            }),
            ..payload
        };

        let payload_json = serde_json::to_string(&payload).map_err(|e| {
            PlatformError::Platform(format!("Failed to serialize chooseMedia payload: {}", e))
        })?;

        lingxia_webview::tsfn::call_arkts("chooseMedia", &[payload_json.as_str()]).map_err(|e| {
            let message = format!("Failed to start chooseMedia flow: {}", e);
            lingxia_messaging::invoke_callback(request.callback_id, false, message.clone());
            PlatformError::Platform(message)
        })
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

#[derive(Serialize)]
struct ChooseMediaPayload {
    #[serde(rename = "callbackId")]
    callback_id: String,
    #[serde(rename = "maxCount")]
    max_count: u32,
    #[serde(rename = "allowOriginal")]
    allow_original: bool,
    #[serde(rename = "allowCompressed")]
    allow_compressed: bool,
    mode: String,
    #[serde(rename = "allowAlbum")]
    allow_album: bool,
    #[serde(rename = "allowCamera")]
    allow_camera: bool,
    #[serde(rename = "maxDurationSeconds")]
    max_duration_seconds: Option<u32>,
    #[serde(rename = "cameraFacing")]
    camera_facing: Option<String>,
}
