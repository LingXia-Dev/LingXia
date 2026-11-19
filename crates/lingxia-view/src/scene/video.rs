//! `<video>` players as the View sees them: what a page asks of each player, what each reports,
//! and the host's resolution of an authored `src` to the URL a player opens.
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

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

/// Where a player is, in ms, for a player that reopens or seeks asynchronously: before it is
/// prepared, the seek parked for its prepare (`-1` none); while a seek is in flight, that seek's
/// target, since the player's clock still reads the old place; otherwise its clock.
pub fn resume_ms(prepared: bool, seeking: bool, target: i64, parked: i64, current: i64) -> i64 {
    if !prepared {
        parked.max(0)
    } else if seeking && target >= 0 {
        target
    } else {
        current.max(0)
    }
}

/// Upscale nobody sees: a 1080-row rendition fills a 1084-px box rather than fetching 1440p.
const UPSCALE_SLACK: f32 = 1.05;

/// The rendition "auto" may climb to for a picture of `px` device pixels drawn with `fit`: the
/// smallest of `renditions` (width, height) that draws without visible upscaling, so auto never
/// fetches more pixels than the picture shows. `None` leaves auto uncapped: no layout yet, or no
/// rendition is that large.
pub fn auto_cap(
    px: [f32; 2],
    fit: crate::scene::image::Fit,
    renditions: &[[f32; 2]],
) -> Option<usize> {
    use crate::scene::image::Fit;
    let px = px.map(f32::round);
    if !(px[0] >= 1.0 && px[1] >= 1.0) {
        return None;
    }
    let draws_unscaled = |r: &[f32; 2]| {
        let (sx, sy) = (px[0] / r[0], px[1] / r[1]);
        r[0] > 0.0
            && r[1] > 0.0
            && match fit {
                Fit::Contain => sx.min(sy) <= UPSCALE_SLACK,
                Fit::Cover => sx.max(sy) <= UPSCALE_SLACK,
                Fit::Fill | Fit::Natural => sx <= UPSCALE_SLACK && sy <= UPSCALE_SLACK,
            }
    };
    renditions
        .iter()
        .enumerate()
        .filter(|(_, r)| draws_unscaled(r))
        .min_by(|(_, a), (_, b)| (a[0] * a[1]).total_cmp(&(b[0] * b[1])))
        .map(|(index, _)| index)
}

/// The window orientation a full-screen video asks for: the shape of its picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    Landscape,
    Portrait,
}

impl Orientation {
    fn code(orientation: Option<Self>) -> u8 {
        match orientation {
            None => 0,
            Some(Self::Landscape) => 1,
            Some(Self::Portrait) => 2,
        }
    }

    fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Landscape),
            2 => Some(Self::Portrait),
            _ => None,
        }
    }

    pub(crate) fn parse(text: &str) -> Option<Option<Self>> {
        match text {
            "" => Some(None),
            "landscape" => Some(Some(Self::Landscape)),
            "portrait" => Some(Some(Self::Portrait)),
            _ => None,
        }
    }
}

static SCOPES: AtomicU64 = AtomicU64::new(1);

/// A page's players: their keys are scoped to it, and their sources resolved under its authority.
pub(crate) struct Sources {
    scope: u64,
    resolver: Option<VideoResolver>,
    resolved: HashMap<String, Result<String, VideoError>>,
    /// Written by the runtime's full-screen layer, read into each frame's metrics.
    orientation: Arc<AtomicU8>,
}

impl Default for Sources {
    fn default() -> Self {
        Self {
            scope: SCOPES.fetch_add(1, Ordering::Relaxed),
            resolver: None,
            resolved: HashMap::new(),
            orientation: Arc::default(),
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

    pub(crate) fn orientation(&self) -> Option<Orientation> {
        Orientation::from_code(self.orientation.load(Ordering::Relaxed))
    }

    /// Sets what [`Self::orientation`] reads; the runtime's global holds one.
    pub(crate) fn orientation_writer(&self) -> impl Fn(Option<Orientation>) + 'static {
        let cell = self.orientation.clone();
        move |orientation| cell.store(Orientation::code(orientation), Ordering::Relaxed)
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

impl Drop for Sources {
    fn drop(&mut self) {
        crate::renderer::video_forget(self.scope);
    }
}

pub(crate) fn key(scope: u64, player: &str) -> String {
    format!("{scope}/{player}")
}

#[cfg(test)]
mod auto_cap_tests {
    use super::auto_cap;
    use crate::scene::image::Fit;

    /// vodobox's ladder, highest first: 2160p down to 144p.
    const LADDER: [[f32; 2]; 8] = [
        [3840.0, 2160.0],
        [2560.0, 1440.0],
        [1920.0, 1080.0],
        [1280.0, 720.0],
        [854.0, 480.0],
        [640.0, 360.0],
        [426.0, 240.0],
        [256.0, 144.0],
    ];

    fn label(px: [f32; 2], fit: Fit) -> Option<f32> {
        auto_cap(px, fit, &LADDER).map(|i| LADDER[i][1])
    }

    #[test]
    fn a_portrait_phone_inline_player_stops_at_the_first_rendition_its_box_does_not_upscale() {
        // 390 pt wide 16:9 box at 3x: 1170 x 658 px.
        assert_eq!(label([390.0 * 3.0, 219.4 * 3.0], Fit::Contain), Some(720.0));
        // The same box at 2x: 780 x 439 px.
        assert_eq!(label([780.0, 438.8], Fit::Contain), Some(480.0));
    }

    #[test]
    fn full_screen_on_a_landscape_phone_climbs_to_the_screen() {
        // 2400 x 1080 px screen, letterboxed 16:9 picture.
        assert_eq!(label([2400.0, 1080.0], Fit::Contain), Some(1080.0));
        // Cover fills the wider screen: 1920 x 1080 would stretch past its width.
        assert_eq!(label([2400.0, 1080.0], Fit::Cover), Some(1440.0));
    }

    #[test]
    fn a_few_pixels_short_still_counts_as_filled() {
        // A 1084 x 2412 phone in landscape: 1080 rows upscale 0.4%, not worth 1.8x the pixels.
        assert_eq!(label([2412.0, 1084.0], Fit::Contain), Some(1080.0));
        // 1242 rows are 15% short of 1080: the next rung up.
        assert_eq!(label([2208.0, 1242.0], Fit::Contain), Some(1440.0));
        // Exact fits are unchanged.
        assert_eq!(label([1920.0, 1080.0], Fit::Contain), Some(1080.0));
        assert_eq!(label([1280.0, 720.0], Fit::Cover), Some(720.0));
    }

    #[test]
    fn auto_takes_the_smallest_rendition_that_covers_the_box_never_one_below_it() {
        let ladder = [[640.0, 360.0], [1280.0, 720.0], [1920.0, 1080.0]];
        let pick = |px: [f32; 2]| auto_cap(px, Fit::Contain, &ladder).map(|i| ladder[i][1]);
        // The Pixel's inline box: 360p would upscale 1.55x.
        assert_eq!(pick([993.0, 558.0]), Some(720.0));
        assert_eq!(pick([640.0, 360.0]), Some(360.0));
        // 1.6% past 720p is within the slack nobody sees; 6% past it climbs.
        assert_eq!(pick([1300.0, 731.0]), Some(720.0));
        assert_eq!(pick([1360.0, 765.0]), Some(1080.0));
    }

    #[test]
    fn a_box_larger_than_every_rendition_leaves_auto_uncapped() {
        assert_eq!(label([5120.0, 2880.0], Fit::Contain), None);
    }

    #[test]
    fn no_layout_yet_is_no_cap_yet() {
        assert_eq!(label([0.0, 0.0], Fit::Contain), None);
        assert_eq!(label([1170.0, 0.0], Fit::Contain), None);
        assert_eq!(label([f32::NAN, 600.0], Fit::Contain), None);
    }

    #[test]
    fn subpixel_layout_does_not_skip_a_rung() {
        assert_eq!(label([1920.3, 1080.2], Fit::Fill), Some(1080.0));
    }
}

#[cfg(test)]
mod tests {
    use super::resume_ms;

    #[test]
    fn a_pin_parked_behind_a_reopen_resumes_at_the_restore_seek() {
        // 480p pinned at 3.7 s: its reopen parks the position until prepared.
        assert_eq!(resume_ms(false, false, -1, 3711, 0), 3711);
        // 480p prepared: the restore seek is in flight and the clock still reads 0. The 288p pin
        // parked meanwhile reopens now, from the seek's target, not from 0.
        assert_eq!(resume_ms(true, true, 3711, -1, 0), 3711);
        // The seek landed: the clock is right again.
        assert_eq!(resume_ms(true, false, -1, -1, 3790), 3790);
    }

    #[test]
    fn a_fresh_player_starts_at_zero() {
        assert_eq!(resume_ms(false, false, -1, -1, 0), 0);
        assert_eq!(resume_ms(true, true, -1, -1, 1200), 1200);
    }
}
