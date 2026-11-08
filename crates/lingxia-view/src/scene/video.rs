//! `<video>` players as the View sees them: what a page asks of each player, what each reports,
//! and the host's resolution of an authored `src` to the URL a player opens.
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Resolves an authored source under the page's authority; runs on the View worker.
pub type VideoResolver = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

/// Past this many sources the cache starts over rather than grow with a feed.
const CACHED: usize = 256;

/// What the page's props ask of a player; applied when they change.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub paused: bool,
    pub muted: bool,
    pub looped: bool,
    pub rate: f32,
    pub volume: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            paused: false,
            muted: false,
            looped: false,
            rate: 1.0,
            volume: 1.0,
        }
    }
}

/// One player's source and props, as the last scene built them.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    /// `(label, url)` per quality; a plain `src` is one variant labelled "".
    pub variants: Vec<(String, String)>,
    pub settings: Settings,
}

impl Source {
    /// The URL of `label`; "auto" or an unknown label plays the first variant.
    pub fn url(&self, label: Option<&str>) -> &str {
        label
            .and_then(|label| self.variants.iter().find(|(l, _)| l == label))
            .or(self.variants.first())
            .map_or("", |(_, url)| url)
    }

    /// The labels a page can pick between; empty for a plain `src`.
    pub fn labels(&self) -> Vec<String> {
        self.variants
            .iter()
            .filter(|(label, _)| !label.is_empty())
            .map(|(label, _)| label.clone())
            .collect()
    }
}

/// The `{src, label}` list of a `sources` prop, already checked by the runtime.
pub fn variants(json: &str) -> Option<Vec<(String, String)>> {
    let list: Vec<serde_json::Value> = serde_json::from_str(json).ok()?;
    if list.is_empty() || list.len() > 8 {
        return None;
    }
    let out: Vec<(String, String)> = list
        .iter()
        .map(|entry| {
            let src = entry["src"]
                .as_str()
                .filter(|s| !s.is_empty() && !s.contains('\0'))?;
            let label = entry["label"]
                .as_str()
                .filter(|l| !l.is_empty() && l.chars().count() <= 32 && *l != "auto")?;
            Some((label.to_owned(), src.to_owned()))
        })
        .collect::<Option<_>>()?;
    let unique = out
        .iter()
        .enumerate()
        .all(|(i, (label, _))| out[..i].iter().all(|(l, _)| l != label));
    unique.then_some(out)
}

/// An imperative edge from a handle or Logic: it never changes the props' values.
#[derive(Clone, Debug, PartialEq)]
pub enum VideoOp {
    Play,
    Pause,
    Seek(f64),
    SetMuted(bool),
    SetVolume(f32),
    SetRate(f32),
    /// A label of `sources` or of an adaptive stream's renditions; `None` is "auto".
    SetQuality(Option<String>),
    Retry,
}

impl VideoOp {
    /// The runtime's `(op, value)` form.
    pub fn parse(op: &str, value: f64, text: &str) -> Option<Self> {
        Some(match op {
            "play" => Self::Play,
            "pause" => Self::Pause,
            "seek" if value.is_finite() && value >= 0.0 => Self::Seek(value),
            "muted" => Self::SetMuted(value != 0.0),
            "volume" if (0.0..=1.0).contains(&value) => Self::SetVolume(value as f32),
            "rate" if (0.25..=4.0).contains(&value) => Self::SetRate(value as f32),
            "retry" => Self::Retry,
            "quality" if text == "auto" => Self::SetQuality(None),
            "quality" if !text.is_empty() => Self::SetQuality(Some(text.to_owned())),
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct VideoError {
    pub code: &'static str,
    pub message: String,
    pub recoverable: bool,
}

impl VideoError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: format!("{code}: {}", message.into()),
            recoverable: code == "VIDEO_NETWORK",
        }
    }

    /// A host resolver's message names its code first; others are source errors.
    pub fn from_message(message: &str) -> Self {
        const CODES: [&str; 9] = [
            "VIDEO_SOURCE",
            "VIDEO_NOT_GRANTED",
            "VIDEO_NETWORK",
            "VIDEO_NOT_FOUND",
            "VIDEO_FORMAT",
            "VIDEO_DECODE",
            "VIDEO_DECODERS",
            "VIDEO_NO_PLAYER",
            "VIDEO_UNSUPPORTED",
        ];
        let code = CODES
            .into_iter()
            .find(|code| message.contains(code))
            .unwrap_or("VIDEO_SOURCE");
        Self {
            code,
            message: message
                .find(code)
                .map_or(message, |at| &message[at..])
                .to_owned(),
            recoverable: code == "VIDEO_NETWORK",
        }
    }
}

/// A player's state, as `useVideo` reads it and events derive from it.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub ready_state: &'static str,
    pub current_time: f64,
    /// `None` before metadata.
    pub duration: Option<f64>,
    pub paused: bool,
    pub ended: bool,
    pub buffering: bool,
    pub seeking: bool,
    pub buffered: f64,
    pub video_width: u32,
    pub video_height: u32,
    pub playback_rate: f32,
    pub volume: f32,
    pub muted: bool,
    /// "auto" or the label now on screen; `qualities` are the labels to pick between.
    pub quality: String,
    pub qualities: Vec<String>,
    /// The label on screen, which "auto" leaves to the player.
    #[serde(skip)]
    pub shown: String,
    pub error: Option<VideoError>,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            ready_state: "empty",
            current_time: 0.0,
            duration: None,
            paused: true,
            ended: false,
            buffering: false,
            seeking: false,
            buffered: 0.0,
            video_width: 0,
            video_height: 0,
            playback_rate: 1.0,
            volume: 1.0,
            muted: false,
            quality: "auto".into(),
            qualities: Vec::new(),
            shown: String::new(),
            error: None,
        }
    }
}

static SCOPES: AtomicU64 = AtomicU64::new(1);

/// A page's players: their keys are scoped to it, and their sources resolved under its authority.
pub(crate) struct Sources {
    scope: u64,
    resolver: Option<VideoResolver>,
    resolved: HashMap<String, Result<String, VideoError>>,
}

impl Default for Sources {
    fn default() -> Self {
        Self {
            scope: SCOPES.fetch_add(1, Ordering::Relaxed),
            resolver: None,
            resolved: HashMap::new(),
        }
    }
}

impl Sources {
    pub(crate) fn has_resolver(&self) -> bool {
        self.resolver.is_some()
    }

    pub(crate) fn set_resolver(&mut self, resolver: VideoResolver) {
        self.resolver = Some(resolver);
        self.resolved.clear();
    }

    pub(crate) fn scope(&self) -> u64 {
        self.scope
    }

    /// The renderer's key for the page's player named `player` (`name:…` or `el:…`).
    pub(crate) fn key(&self, player: &str) -> String {
        key(self.scope, player)
    }

    /// The player URL for `src`; without a host resolver the source is the URL.
    pub(crate) fn url(&mut self, src: &str) -> Result<String, VideoError> {
        if let Some(url) = self.resolved.get(src) {
            return url.clone();
        }
        let url = match &self.resolver {
            None => Ok(src.to_owned()),
            Some(resolve) => resolve(src).map_err(|error| {
                log::warn!(target: "lingxia_view", "<video src=\"{src}\">: {error}");
                VideoError::from_message(&error)
            }),
        };
        if self.resolved.len() >= CACHED {
            self.resolved.clear();
        }
        self.resolved.insert(src.to_owned(), url.clone());
        url
    }
}

pub(crate) fn key(scope: u64, player: &str) -> String {
    format!("{scope}/{player}")
}
