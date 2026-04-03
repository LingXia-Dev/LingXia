use std::future::Future;

use crate::error::PlatformError;

#[derive(Debug, Clone)]
pub struct PreviewMediaItem {
    pub path: String,
    pub media_type: MediaKind,
    pub cover_path: Option<String>,
    pub rotate: Option<u16>,
    pub object_fit: Option<MediaObjectFit>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewMediaAdvance {
    #[default]
    Manual,
    Next,
    Loop,
}

impl PreviewMediaAdvance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Next => "next",
            Self::Loop => "loop",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreviewMediaRequest {
    pub items: Vec<PreviewMediaItem>,
    pub start_index: i32,
    pub advance: PreviewMediaAdvance,
    pub show_index_indicator: bool,
    /// Internal callback_id for abort signal support.
    pub callback_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaObjectFit {
    Cover,
    Contain,
    Fill,
    Fit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChooseMediaMode {
    Images,
    Videos,
    Mix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaSource {
    Album,
    Camera,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraFacing {
    Front,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaQuality {
    Original,
    Compressed,
}

#[derive(Debug, Clone)]
pub struct ChooseMediaRequest {
    pub max_count: u32,
    pub mode: ChooseMediaMode,
    pub source_types: Vec<MediaSource>,
    pub max_duration_seconds: Option<u32>,
    pub camera_facing: Option<CameraFacing>,
}

impl Default for ChooseMediaRequest {
    fn default() -> Self {
        Self {
            max_count: 9,
            mode: ChooseMediaMode::Images,
            source_types: vec![MediaSource::Album, MediaSource::Camera],
            max_duration_seconds: None,
            camera_facing: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanType {
    BarCode,
    QrCode,
    DataMatrix,
    Pdf417,
}

#[derive(Debug, Clone)]
pub struct ScanCodeRequest {
    pub scan_types: Vec<ScanType>,
    pub only_from_camera: bool,
}

impl Default for ScanCodeRequest {
    fn default() -> Self {
        Self {
            scan_types: Vec::new(),
            only_from_camera: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SaveMediaRequest {
    pub file_uri: String,
}

pub trait MediaInteraction: Send + Sync + 'static {
    /// Preview media. Keeps callback_id pattern for AbortSignal support.
    fn preview_media(&self, request: PreviewMediaRequest) -> Result<(), PlatformError>;
    fn cancel_preview(&self, callback_id: u64) -> Result<(), PlatformError>;

    fn choose_media(
        &self,
        request: ChooseMediaRequest,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send;

    fn scan_code(
        &self,
        request: ScanCodeRequest,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send;

    fn save_image_to_photos_album(
        &self,
        request: SaveMediaRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send;

    fn save_video_to_photos_album(
        &self,
        request: SaveMediaRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send;
}
