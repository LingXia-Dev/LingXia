use crate::error::PlatformError;

#[derive(Debug, Clone)]
pub struct PreviewMediaItem {
    pub path: String,
    pub media_type: MediaKind,
    pub cover_path: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PreviewMediaRequest {
    pub items: Vec<PreviewMediaItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
    Unknown,
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
    pub callback_id: u64,
}

impl Default for ChooseMediaRequest {
    fn default() -> Self {
        Self {
            max_count: 9,
            mode: ChooseMediaMode::Images,
            source_types: vec![MediaSource::Album, MediaSource::Camera],
            max_duration_seconds: None,
            camera_facing: None,
            callback_id: 0,
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
    pub callback_id: u64,
}

impl Default for ScanCodeRequest {
    fn default() -> Self {
        Self {
            scan_types: Vec::new(),
            only_from_camera: true,
            callback_id: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SaveMediaRequest {
    pub file_uri: String,
}

pub trait MediaInteraction: Send + Sync + 'static {
    fn preview_media(&self, request: PreviewMediaRequest) -> Result<(), PlatformError>;
    fn choose_media(&self, request: ChooseMediaRequest) -> Result<(), PlatformError>;
    fn scan_code(&self, request: ScanCodeRequest) -> Result<(), PlatformError>;
    fn save_image_to_photos_album(&self, request: SaveMediaRequest) -> Result<(), PlatformError>;
    fn save_video_to_photos_album(&self, request: SaveMediaRequest) -> Result<(), PlatformError>;
}
