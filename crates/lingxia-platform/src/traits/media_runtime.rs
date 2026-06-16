use std::path::{Path, PathBuf};

use crate::error::PlatformError;

use super::media_interaction::MediaKind;

#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CompressImageRequest {
    pub source_uri: String,
    pub quality: u8,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub output_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCompressQuality {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone)]
pub struct CompressVideoRequest {
    pub source_uri: String,
    pub quality: Option<VideoCompressQuality>,
    /// Target average bitrate in kbps.
    pub bitrate_kbps: Option<u32>,
    /// Target frame rate in fps.
    pub fps: Option<u32>,
    /// Scale ratio relative to source resolution in (0, 1].
    pub resolution_ratio: Option<f32>,
    pub output_path: PathBuf,
    /// Stream callback fired with `{"progress": N}` (N = 0..100) while the
    /// transcode runs. Native may fire at any cadence; consecutive
    /// duplicates are fine (the JS side dedupes).
    pub progress_callback_id: u64,
    /// Oneshot callback fired when the transcode finishes:
    /// `{"success":true,"path":...,"width":...,"height":...,"durationMs":...,
    /// "size":...,"mimeType":...}` or `{"success":false,"error":"..."}`.
    pub callback_id: u64,
}

#[derive(Debug, Clone)]
pub struct CompressedVideo {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub size: u64,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub rotation: Option<u16>,
    pub bitrate: Option<u64>,
    pub fps: Option<f32>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExtractVideoThumbnailRequest {
    pub source_uri: String,
    pub output_path: PathBuf,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub time_ms: Option<u64>,
    pub quality: u8,
}

#[derive(Debug, Clone)]
pub struct VideoThumbnail {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub mime_type: Option<String>,
}

pub trait MediaRuntime: Send + Sync + 'static {
    /// Copy a picked/album media asset identified by `uri` into a local file at `dest_path`.
    ///
    /// Notes:
    /// - `uri` is an opaque platform media reference coming from platform pickers and may not be a
    ///   directly readable filesystem path.
    /// - Implementations should support platform-specific schemes as applicable, for example:
    ///   - Android: `content://...`
    ///   - iOS: `ph://...` (or other Photos identifiers)
    ///   - Harmony: picker URIs such as `file://media/...`
    ///   - Some platforms may also provide `file:///absolute/path` (or an absolute path string).
    /// - Implementations should create parent directories for `dest_path` if needed and write the
    ///   file content so that `dest_path` exists on success.
    fn copy_album_media_to_file(
        &self,
        uri: &str,
        dest_path: &Path,
        kind: MediaKind,
    ) -> Result<(), PlatformError>;

    fn get_image_info(&self, uri: &str) -> Result<ImageInfo, PlatformError>;

    fn compress_image(&self, request: &CompressImageRequest) -> Result<PathBuf, PlatformError>;

    /// Start a video transcode. Returns as soon as the job is accepted;
    /// progress and the final result arrive on the request's callbacks.
    fn compress_video(&self, request: &CompressVideoRequest) -> Result<(), PlatformError>;

    /// Cancel a running transcode by its completion `callback_id`. Native
    /// stops the job and SHOULD delete any partial output; it MUST NOT fire
    /// the completion callback afterwards (the caller already removed it).
    fn cancel_compress_video(&self, callback_id: u64) -> Result<(), PlatformError>;

    fn get_video_info(&self, uri: &str) -> Result<VideoInfo, PlatformError>;

    fn extract_video_thumbnail(
        &self,
        request: &ExtractVideoThumbnailRequest,
    ) -> Result<VideoThumbnail, PlatformError>;
}
