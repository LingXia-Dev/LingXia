//! Realtime media-capture contract.
//!
//! Types and traits only. A host that did not declare capture must not
//! construct a provider, and this module never supplies an always-unsupported
//! placeholder or a process-global registry.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Boxed provider future. Capture is not on `AppRuntime`, so implementors
/// do not have to pull `async_trait` just to start a session.
pub type CaptureFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// Identifies one capture session across geometry and remote input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CaptureSessionId(pub u64);

/// Identifies one track inside a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TrackId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrackKind {
    Visual,
    SystemAudio,
    Microphone,
}

impl TrackKind {
    pub const ALL: [TrackKind; 3] = [
        TrackKind::Visual,
        TrackKind::SystemAudio,
        TrackKind::Microphone,
    ];
}

/// Combinations a provider can start together on one native session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureCapabilities {
    pub combinations: Vec<TrackSet>,
}

impl CaptureCapabilities {
    pub fn visual_only() -> Self {
        Self {
            combinations: vec![TrackSet {
                tracks: vec![TrackKind::Visual],
                coupled_clock: false,
            }],
        }
    }

    pub fn supports(&self, kind: TrackKind) -> bool {
        self.combinations
            .iter()
            .any(|set| set.tracks.contains(&kind))
    }

    pub fn supports_set(&self, kinds: &[TrackKind]) -> Option<&TrackSet> {
        self.combinations.iter().find(|set| {
            kinds.iter().all(|kind| set.tracks.contains(kind)) && set.tracks.len() == kinds.len()
        })
    }
}

/// One startable combination. `coupled_clock` means the tracks share a native
/// clock (typical for visual + system-audio from one MediaProjection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackSet {
    pub tracks: Vec<TrackKind>,
    pub coupled_clock: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisualTarget {
    Screen,
    Display {
        id: String,
    },
    Window {
        id: String,
    },
    Region {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

/// Integer rectangle. Origins may be negative (multi-monitor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn contains(self, x: i32, y: i32) -> bool {
        let w = self.width.max(0);
        let h = self.height.max(0);
        x >= self.x && y >= self.y && x < self.x.saturating_add(w) && y < self.y.saturating_add(h)
    }
}

/// Canonical realtime visual geometry for one generation.
///
/// Remote input carries `session_id` + `generation` and is rejected when
/// either is stale. Coordinates:
///
/// - `source` — platform global/source, including negative display origins
/// - `target_local` — target-local origin (typically 0,0 × source size)
/// - `output` — encoded output pixel size
/// - `content_in_output` — the content rectangle inside the output, after
///   crop and letterbox/pillarbox
/// - `scale` — physical pixels per logical unit
/// - `rotation_degrees` / `mirrored` — applied from source to output
#[derive(Debug, Clone, PartialEq)]
pub struct VisualGeometry {
    pub session_id: CaptureSessionId,
    pub generation: u64,
    pub source: Rect,
    pub target_local: Rect,
    pub output: Size,
    pub content_in_output: Rect,
    pub scale: f64,
    pub rotation_degrees: i32,
    pub mirrored: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    Hevc,
    Vp8,
    Vp9,
    Av1,
    /// Desktop software path. Still an encoded packet, not a public RGBA type.
    Png,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCodec {
    Opus,
    Aac,
    PcmS16le,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VisualTrackConfig {
    pub target: VisualTarget,
    pub max_size: Option<Size>,
    pub fps: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub codec: Option<VideoCodec>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioTrackConfig {
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub bitrate_kbps: Option<u32>,
    pub codec: Option<AudioCodec>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackRequest {
    pub kind: TrackKind,
    pub required: bool,
    pub visual: Option<VisualTrackConfig>,
    pub audio: Option<AudioTrackConfig>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderCaptureRequest {
    pub tracks: Vec<TrackRequest>,
    /// Fresh per session start. Android MediaProjection result lives here
    /// and is not cached across sessions.
    pub authorization: Option<CaptureAuthorization>,
}

/// Opaque authorization material supplied for one provider session start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureAuthorization {
    pub kind: AuthorizationKind,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationKind {
    AndroidMediaProjection,
    AppleScreenRecording,
    HarmonyScreenCapture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureTimestamp {
    pub monotonic_nanos: u64,
    pub wall_unix_nanos: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedPacket {
    pub track_id: TrackId,
    pub kind: TrackKind,
    pub timestamp: CaptureTimestamp,
    pub sequence: u64,
    pub format_generation: u64,
    pub geometry_generation: Option<u64>,
    pub keyframe: bool,
    pub video_codec: Option<VideoCodec>,
    pub audio_codec: Option<AudioCodec>,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscontinuityReason {
    Overflow,
    Resynchronize,
    Gap,
    Reconfigure,
}

#[derive(Debug, Clone)]
pub enum ProviderEvent {
    TrackConfigured {
        track_id: TrackId,
        kind: TrackKind,
        format_generation: u64,
        video_codec: Option<VideoCodec>,
        audio_codec: Option<AudioCodec>,
        sample_rate: Option<u32>,
        channels: Option<u8>,
        size: Option<Size>,
    },
    VisualGeometry {
        track_id: TrackId,
        geometry: VisualGeometry,
    },
    EncodedPacket(EncodedPacket),
    Discontinuity {
        track_id: TrackId,
        kind: TrackKind,
        reason: DiscontinuityReason,
        format_generation: u64,
        timestamp: CaptureTimestamp,
    },
    AuthorizationRevoked {
        track: TrackKind,
    },
    Failed {
        track: Option<TrackKind>,
        error: CaptureError,
    },
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureError {
    Composition(String),
    AuthorizationRequired {
        track: TrackKind,
    },
    Denied {
        track: TrackKind,
    },
    Unavailable {
        track: Option<TrackKind>,
        reason: String,
    },
    Failed {
        reason: String,
    },
    InvalidRequest(String),
    StaleGeneration,
    UnknownSession,
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Composition(reason) => write!(f, "capture composition: {reason}"),
            Self::AuthorizationRequired { track } => {
                write!(f, "authorization required for {track:?}")
            }
            Self::Denied { track } => write!(f, "capture denied for {track:?}"),
            Self::Unavailable { track, reason } => match track {
                Some(track) => write!(f, "capture unavailable for {track:?}: {reason}"),
                None => write!(f, "capture unavailable: {reason}"),
            },
            Self::Failed { reason } => write!(f, "capture failed: {reason}"),
            Self::InvalidRequest(reason) => write!(f, "invalid capture request: {reason}"),
            Self::StaleGeneration => write!(f, "stale capture geometry generation"),
            Self::UnknownSession => write!(f, "unknown capture session"),
        }
    }
}

impl std::error::Error for CaptureError {}

pub trait MediaCaptureProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn capabilities(&self) -> CaptureCapabilities;
    fn start(
        &self,
        request: ProviderCaptureRequest,
        events: Arc<dyn ProviderEventSink>,
    ) -> CaptureFuture<Result<Box<dyn ProviderCaptureSession>, CaptureError>>;
}

pub trait ProviderCaptureSession: Send + Sync {
    fn reconfigure(
        &self,
        request: ProviderCaptureRequest,
    ) -> CaptureFuture<Result<(), CaptureError>>;
    fn request_keyframe(&self, track: TrackId) -> Result<(), CaptureError>;
    fn suspend(&self) -> Result<(), CaptureError>;
    fn resume(&self) -> Result<(), CaptureError>;
    fn stop(&self);
}

pub trait ProviderEventSink: Send + Sync {
    fn emit(&self, event: ProviderEvent);
}

#[cfg(all(feature = "android-capture-provider", target_os = "android"))]
pub mod android;

#[cfg(all(
    feature = "apple-capture-provider",
    any(target_os = "ios", target_os = "macos")
))]
pub mod apple;

#[cfg(all(feature = "harmony-capture-provider", target_env = "ohos"))]
pub mod harmony;
