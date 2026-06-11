use std::future::Future;

use crate::error::PlatformError;

#[derive(Debug, Clone)]
pub struct PreviewMediaItem {
    pub path: String,
    pub media_type: MediaKind,
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
    /// Callback_id for the final preview result. Fires once when the session
    /// ends (manual/auto/interrupted/error). Payload: serialized
    /// `PreviewMediaResultObj` ({reason, lastIndex}).
    pub callback_id: u64,
    /// Callback_id for the "first frame composited" signal. Fires once when
    /// the first pixel of the underlying media has been painted to screen.
    /// Native MAY skip on degenerate paths (abort before any item rendered,
    /// process tear-down, etc.) — the JS-side `presented` Promise is also
    /// woken by a fallback when `completed` settles, and by a total timeout
    /// after that, so it never hangs. The callback payload, when fired, is
    /// an empty JSON object `{}`.
    pub presented_callback_id: u64,
    /// Stream callback_id for current-item changes. Native fires it with
    /// payload `{"index": N}` whenever the displayed item changes — including
    /// the initially displayed item — for swipes, taps, and auto-advance.
    /// Consecutive duplicates are tolerated (the JS side dedupes); missing
    /// the initial fire is also tolerated (the JS side seeds the snapshot
    /// from startIndex).
    pub change_callback_id: u64,
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
