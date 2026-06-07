//! Media, camera, scanner, and media-preview APIs for native Rust code.
//!
//! In addition to the interaction helpers (choose/preview/scan/save), this
//! module exposes the platform [`MediaRuntime`] processing APIs (image/video
//! info, compression, thumbnails) through typed Rust request and response
//! structs. Processing APIs operate on already-resolved filesystem paths;
//! callers that start from an `lx://` URI must resolve it first.

use std::path::PathBuf;

use lingxia_platform::traits::media_runtime::{
    CompressImageRequest as PlatformCompressImageRequest,
    CompressVideoRequest as PlatformCompressVideoRequest,
    ExtractVideoThumbnailRequest as PlatformExtractVideoThumbnailRequest, MediaRuntime,
    VideoCompressQuality as PlatformVideoCompressQuality,
};
use serde::{Deserialize, Serialize};

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

/// Presents the host media picker and returns the serialized selection payload.
pub async fn choose(request: ChooseMediaRequest) -> crate::Result<String> {
    let runtime = crate::runtime::platform()?;
    lingxia_service::media::choose_media(&*runtime, request)
        .await
        .map_err(Into::into)
}

/// Opens LingXia's native media preview UI.
pub fn preview(request: PreviewMediaRequest) -> crate::Result<()> {
    let runtime = crate::runtime::platform()?;
    lingxia_service::media::preview_media(&*runtime, request).map_err(Into::into)
}

/// Cancels an active media preview callback sequence.
pub fn cancel_preview(callback_id: u64) -> crate::Result<()> {
    let runtime = crate::runtime::platform()?;
    lingxia_service::media::cancel_preview(&*runtime, callback_id).map_err(Into::into)
}

/// Opens a scanner flow and returns the serialized scan result payload.
pub async fn scan(request: ScanCodeRequest) -> crate::Result<String> {
    let runtime = crate::runtime::platform()?;
    lingxia_service::media::scan_code(&*runtime, request)
        .await
        .map_err(Into::into)
}

/// Saves an image into the platform photo library.
pub async fn save_image_to_photo_library(request: SaveMediaRequest) -> crate::Result<()> {
    let runtime = crate::runtime::platform()?;
    lingxia_service::media::save_image_to_photos_album(&*runtime, request)
        .await
        .map_err(Into::into)
}

/// Saves a video into the platform photo library.
pub async fn save_video_to_photo_library(request: SaveMediaRequest) -> crate::Result<()> {
    let runtime = crate::runtime::platform()?;
    lingxia_service::media::save_video_to_photos_album(&*runtime, request)
        .await
        .map_err(Into::into)
}

// MediaRuntime processing APIs. Inputs are already-resolved filesystem paths.

/// Image dimensions and MIME type.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Request for [`compress_image`].
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressImage {
    /// Source image path.
    pub source_path: String,
    /// JPEG-style quality in `0..=100`.
    pub quality: u8,
    #[serde(default)]
    pub max_width: Option<u32>,
    #[serde(default)]
    pub max_height: Option<u32>,
    /// Destination path for the compressed image.
    pub output_path: String,
}

/// Result of [`compress_image`]: the written file path.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompressedImage {
    pub path: PathBuf,
}

/// Video dimensions, duration, and codec metadata.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fps: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Request for [`extract_video_thumbnail`].
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractVideoThumbnail {
    /// Source video path.
    pub source_path: String,
    /// Destination path for the extracted thumbnail.
    pub output_path: String,
    #[serde(default)]
    pub max_width: Option<u32>,
    #[serde(default)]
    pub max_height: Option<u32>,
    /// Frame timestamp in milliseconds; defaults to the platform default.
    #[serde(default)]
    pub time_ms: Option<u64>,
    /// JPEG-style quality in `0..=100`.
    pub quality: u8,
}

/// Result of [`extract_video_thumbnail`].
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VideoThumbnail {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Target quality preset for [`compress_video`].
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VideoQuality {
    Low,
    Medium,
    High,
}

impl From<VideoQuality> for PlatformVideoCompressQuality {
    fn from(value: VideoQuality) -> Self {
        match value {
            VideoQuality::Low => PlatformVideoCompressQuality::Low,
            VideoQuality::Medium => PlatformVideoCompressQuality::Medium,
            VideoQuality::High => PlatformVideoCompressQuality::High,
        }
    }
}

/// Request for [`compress_video`].
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressVideo {
    /// Source video path.
    pub source_path: String,
    #[serde(default)]
    pub quality: Option<VideoQuality>,
    /// Target average bitrate in kbps.
    #[serde(default)]
    pub bitrate_kbps: Option<u32>,
    /// Target frame rate in fps.
    #[serde(default)]
    pub fps: Option<u32>,
    /// Scale ratio relative to source resolution in `(0, 1]`.
    #[serde(default)]
    pub resolution_ratio: Option<f32>,
    /// Destination path for the compressed video.
    pub output_path: String,
}

/// Result of [`compress_video`].
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompressedVideo {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Reads width/height/MIME for an image file.
pub fn image_info(path: &str) -> crate::Result<ImageInfo> {
    let info = crate::runtime::platform()?
        .get_image_info(path)
        .map_err(crate::Error::from)?;
    Ok(ImageInfo {
        width: info.width,
        height: info.height,
        mime_type: info.mime_type,
    })
}

/// Compresses an image to `output_path` and returns the written path.
pub fn compress_image(input: CompressImage) -> crate::Result<CompressedImage> {
    let request = PlatformCompressImageRequest {
        source_uri: input.source_path,
        quality: input.quality,
        max_width: input.max_width,
        max_height: input.max_height,
        output_path: PathBuf::from(input.output_path),
    };
    let path = crate::runtime::platform()?
        .compress_image(&request)
        .map_err(crate::Error::from)?;
    Ok(CompressedImage { path })
}

/// Reads dimensions/duration/codec metadata for a video file.
pub fn video_info(path: &str) -> crate::Result<VideoInfo> {
    let info = crate::runtime::platform()?
        .get_video_info(path)
        .map_err(crate::Error::from)?;
    Ok(VideoInfo {
        width: info.width,
        height: info.height,
        duration_ms: info.duration_ms,
        rotation: info.rotation,
        bitrate: info.bitrate,
        fps: info.fps,
        mime_type: info.mime_type,
    })
}

/// Extracts a still thumbnail from a video into `output_path`.
pub fn extract_video_thumbnail(input: ExtractVideoThumbnail) -> crate::Result<VideoThumbnail> {
    let request = PlatformExtractVideoThumbnailRequest {
        source_uri: input.source_path,
        output_path: PathBuf::from(input.output_path),
        max_width: input.max_width,
        max_height: input.max_height,
        time_ms: input.time_ms,
        quality: input.quality,
    };
    let thumb = crate::runtime::platform()?
        .extract_video_thumbnail(&request)
        .map_err(crate::Error::from)?;
    Ok(VideoThumbnail {
        path: thumb.path,
        width: thumb.width,
        height: thumb.height,
        mime_type: thumb.mime_type,
    })
}

/// Compresses a video to `output_path` and returns its resulting metadata.
pub fn compress_video(input: CompressVideo) -> crate::Result<CompressedVideo> {
    let request = PlatformCompressVideoRequest {
        source_uri: input.source_path,
        quality: input.quality.map(Into::into),
        bitrate_kbps: input.bitrate_kbps,
        fps: input.fps,
        resolution_ratio: input.resolution_ratio,
        output_path: PathBuf::from(input.output_path),
    };
    let result = crate::runtime::platform()?
        .compress_video(&request)
        .map_err(crate::Error::from)?;
    Ok(CompressedVideo {
        path: result.path,
        width: result.width,
        height: result.height,
        duration_ms: result.duration_ms,
        size: result.size,
        mime_type: result.mime_type,
    })
}
