use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Focus {
    #[default]
    Exclusive,
    Duck,
    Mix,
}

impl Focus {
    fn rank(self) -> u8 {
        match self {
            Focus::Mix => 0,
            Focus::Duck => 1,
            Focus::Exclusive => 2,
        }
    }

    pub fn stronger(self, other: Focus) -> Focus {
        if other.rank() > self.rank() {
            other
        } else {
            self
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Focus::Exclusive => "exclusive",
            Focus::Duck => "duck",
            Focus::Mix => "mix",
        }
    }
}

/// What a player asks for; the route says where audio actually goes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputRequest {
    #[default]
    Auto,
    Speaker,
    Earpiece,
}

impl OutputRequest {
    pub fn as_str(self) -> &'static str {
        match self {
            OutputRequest::Auto => "auto",
            OutputRequest::Speaker => "speaker",
            OutputRequest::Earpiece => "earpiece",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Repeat {
    #[default]
    Off,
    One,
    All,
}

impl Repeat {
    pub fn as_str(self) -> &'static str {
        match self {
            Repeat::Off => "off",
            Repeat::One => "one",
            Repeat::All => "all",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlayerStatus {
    #[default]
    Idle,
    Loading,
    Paused,
    Playing,
    Stopped,
    Ended,
    Failed,
    Disposed,
}

impl PlayerStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            PlayerStatus::Idle => "idle",
            PlayerStatus::Loading => "loading",
            PlayerStatus::Paused => "paused",
            PlayerStatus::Playing => "playing",
            PlayerStatus::Stopped => "stopped",
            PlayerStatus::Ended => "ended",
            PlayerStatus::Failed => "failed",
            PlayerStatus::Disposed => "disposed",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RouteOutput {
    #[default]
    Speaker,
    Earpiece,
    Headphones,
    Bluetooth,
    Usb,
    Hdmi,
    Airplay,
    Other,
}

impl RouteOutput {
    pub fn as_str(self) -> &'static str {
        match self {
            RouteOutput::Speaker => "speaker",
            RouteOutput::Earpiece => "earpiece",
            RouteOutput::Headphones => "headphones",
            RouteOutput::Bluetooth => "bluetooth",
            RouteOutput::Usb => "usb",
            RouteOutput::Hdmi => "hdmi",
            RouteOutput::Airplay => "airplay",
            RouteOutput::Other => "other",
        }
    }

    /// Speaker and earpiece are the built-in route; requested output only
    /// applies there.
    pub fn is_built_in(self) -> bool {
        matches!(self, RouteOutput::Speaker | RouteOutput::Earpiece)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Route {
    pub output: RouteOutput,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RouteReason {
    DeviceAdded,
    DeviceRemoved,
    Override,
    Proximity,
    #[default]
    Other,
}

impl RouteReason {
    pub fn as_str(self) -> &'static str {
        match self {
            RouteReason::DeviceAdded => "deviceAdded",
            RouteReason::DeviceRemoved => "deviceRemoved",
            RouteReason::Override => "override",
            RouteReason::Proximity => "proximity",
            RouteReason::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteChange {
    pub route: Route,
    pub reason: RouteReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InterruptionPhase {
    Began,
    Ended,
}

impl InterruptionPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            InterruptionPhase::Began => "began",
            InterruptionPhase::Ended => "ended",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InterruptionReason {
    #[default]
    System,
    Call,
    OtherApp,
    FocusTaken,
    AppHidden,
}

impl InterruptionReason {
    pub fn as_str(self) -> &'static str {
        match self {
            InterruptionReason::System => "system",
            InterruptionReason::Call => "call",
            InterruptionReason::OtherApp => "otherApp",
            InterruptionReason::FocusTaken => "focusTaken",
            InterruptionReason::AppHidden => "appHidden",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interruption {
    pub phase: InterruptionPhase,
    pub reason: InterruptionReason,
    pub should_resume: bool,
    pub resumed: bool,
}

/// `data.code` of an audio rejection. The numeric business code is derived,
/// so `parseLxApiError` keeps working.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioErrorCode {
    InvalidSource,
    InvalidOptions,
    SourceNotFound,
    Network,
    NetworkDenied,
    UnsupportedFormat,
    OutputUnsupported,
    DecodeFailed,
    SourceTooLarge,
    Disposed,
    Interrupted,
    BackgroundUnavailable,
}

impl AudioErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            AudioErrorCode::InvalidSource => "invalid_source",
            AudioErrorCode::InvalidOptions => "invalid_options",
            AudioErrorCode::SourceNotFound => "source_not_found",
            AudioErrorCode::Network | AudioErrorCode::NetworkDenied => "network",
            AudioErrorCode::UnsupportedFormat => "unsupported_format",
            AudioErrorCode::OutputUnsupported => "output_unsupported",
            AudioErrorCode::DecodeFailed => "decode_failed",
            AudioErrorCode::SourceTooLarge => "source_too_large",
            AudioErrorCode::Disposed => "disposed",
            AudioErrorCode::Interrupted => "interrupted",
            AudioErrorCode::BackgroundUnavailable => "background_unavailable",
        }
    }

    pub fn biz_code(self) -> u32 {
        match self {
            AudioErrorCode::InvalidSource
            | AudioErrorCode::InvalidOptions
            | AudioErrorCode::SourceTooLarge => 1002,
            AudioErrorCode::SourceNotFound => 1003,
            AudioErrorCode::DecodeFailed => 1005,
            AudioErrorCode::NetworkDenied => 3000,
            AudioErrorCode::Disposed | AudioErrorCode::Interrupted => 4000,
            AudioErrorCode::Network => 5000,
            AudioErrorCode::UnsupportedFormat
            | AudioErrorCode::OutputUnsupported
            | AudioErrorCode::BackgroundUnavailable => 6000,
        }
    }

    /// Codes a native backend reports for a failed load or playback.
    pub fn from_native(code: &str) -> AudioErrorCode {
        match code {
            "network" => AudioErrorCode::Network,
            "source_not_found" => AudioErrorCode::SourceNotFound,
            "unsupported_format" => AudioErrorCode::UnsupportedFormat,
            "invalid_source" => AudioErrorCode::InvalidSource,
            "output_unsupported" => AudioErrorCode::OutputUnsupported,
            _ => AudioErrorCode::DecodeFailed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioError {
    pub code: AudioErrorCode,
    pub message: String,
}

impl AudioError {
    pub fn new(code: AudioErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn disposed() -> Self {
        Self::new(AudioErrorCode::Disposed, "the audio object is disposed")
    }
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for AudioError {}
