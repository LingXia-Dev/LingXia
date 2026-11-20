//! The renderer's video players. A `Paint::Video` key opens one platform player off the render
//! thread; the render thread imports each new frame as an external texture, with no CPU copy.
//! Players nothing drew for 2 s stop and close on the opener thread.
use crate::scene::image::Fit;
use crate::scene::paint::Paint;
use crate::scene::video::{Settings, Source, Status, VideoError, VideoOp};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard, mpsc};
use std::time::{Duration, Instant};

/// A player not drawn for this long stops decoding and closes.
const IDLE: Duration = Duration::from_secs(2);

/// Frames failing in a row before the player is failed: one bad frame may recover, a frame
/// path that cannot work would otherwise fail and redraw at frame rate.
const FRAME_FAILURES: u8 = 3;

#[cfg(any(target_os = "android", target_env = "ohos"))]
pub mod gles;

/// Bumped when a frame size changes: cached scenes encode the video's source region.
static EPOCH: AtomicU64 = AtomicU64::new(0);

pub(crate) fn epoch() -> u64 {
    EPOCH
        .load(Ordering::Relaxed)
        .wrapping_add(crate::scene::webview::epoch())
}

/// High bit set: image texture ids are pixmap addresses, which never have it.
pub(crate) fn texture_id(player: &str) -> vello_gpu::TextureId {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    player.hash(&mut hasher);
    vello_gpu::TextureId(hasher.finish() | (1 << 63))
}

/// A platform player the renderer draws from: it decodes on its own and hands over its newest
/// frame as a texture of the renderer's device.
pub trait Player: Send {
    /// A frame newer than the last one taken is due at this refresh.
    fn has_new(&self) -> bool;
    /// Drops a first frame decoded ahead of the player, for a player that starts elsewhere.
    fn discard_first(&mut self) {}
    /// The frame due now, when it is new. A player whose frames reach the CPU writes them
    /// through `queue`.
    fn next_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Option<(wgpu::Texture, [u16; 2])>, String>;
    /// The page's props changed: only the fields that differ from `previous` apply, so a
    /// handle's change stands until its prop changes.
    fn apply(&mut self, settings: &Settings, previous: Option<&Settings>) {
        let _ = (settings, previous);
    }
    /// An imperative edge from a handle or Logic.
    fn command(&mut self, op: &VideoOp) -> Result<(), VideoError> {
        let _ = op;
        Err(VideoError::new(
            "VIDEO_NO_PLAYER",
            "this platform's player takes no commands yet",
        ))
    }
    /// Time, duration and transport state; the registry fills readiness and size.
    fn status(&self) -> Status {
        Status::default()
    }
    /// An adaptive stream's renditions by label, once its manifest is read.
    fn qualities(&self) -> Vec<String> {
        Vec::new()
    }
    /// Whether choosing `label` applies to the playing player rather than through a second one
    /// of the stream: auto only lifts limits, a pin would wait out the buffered segments.
    fn quality_in_place(&self, label: Option<&str>) -> bool {
        label.is_none()
    }
    /// Pins the rendition `label`, or gives the choice back to the player with `None`.
    fn set_quality(&mut self, label: Option<&str>) -> Result<(), VideoError> {
        let _ = label;
        Err(VideoError::new(
            "VIDEO_SOURCE",
            "this video has one quality; give `sources` to choose between files",
        ))
    }
    /// The picture's size in device pixels and its fit, at every drawn frame: "auto" stays
    /// within it ([`crate::scene::video::auto_cap`]); a pin ignores it.
    fn set_view(&mut self, px: [f32; 2], fit: Fit) {
        let _ = (px, fit);
    }
}

/// A smaller box than the largest drawn lately waits this long, so a player drawn in two places
/// (inline and full screen) keeps the larger, and leaving full screen lowers it once.
const VIEW_SETTLE: Duration = Duration::from_millis(500);

/// The largest box each player of `display` is drawn in, in device pixels.
fn views(display: &crate::DisplayList) -> HashMap<&str, ([f32; 2], Fit)> {
    let mut out: HashMap<&str, ([f32; 2], Fit)> = HashMap::new();
    // Video bounds are logical and before transform: a scaled box shows more or fewer pixels.
    let mut stretch = [1.0f32, 1.0];
    for paint in &display.commands {
        if let Paint::Transform(matrix) = paint {
            let [a, b, c, d, _, _] = matrix.as_coeffs();
            stretch = [a.hypot(b) as f32, c.hypot(d) as f32];
        } else if let Paint::Video {
            player,
            bounds,
            fit,
            ..
        } = paint
        {
            let px = [
                bounds[2] * stretch[0] * display.scale,
                bounds[3] * stretch[1] * display.scale,
            ];
            let entry = out.entry(player.as_str()).or_insert((px, *fit));
            if px[0] * px[1] > entry.0[0] * entry.0[1] {
                *entry = (px, *fit);
            }
        }
    }
    out
}

/// How far ahead of the playing position a new quality is parked, in ms of media time, for
/// local files and for network sources. It learns: shorter while the parked player is ready in
/// time, longer when it is not, so a switch takes no longer than the source needs.
static SWITCH_LEAD_MS: [std::sync::atomic::AtomicU32; 2] = [
    std::sync::atomic::AtomicU32::new(400),
    std::sync::atomic::AtomicU32::new(1000),
];

fn switch_lead(url: &str) -> &'static std::sync::atomic::AtomicU32 {
    &SWITCH_LEAD_MS[usize::from(!url.starts_with("file:"))]
}

/// Scales `url`'s lead by `factor`, within 0.4–3 s.
fn tune_lead(url: &str, factor: f64) {
    let lead = switch_lead(url);
    let next = (f64::from(lead.load(Ordering::Relaxed)) * factor).clamp(400.0, 3000.0);
    lead.store(next as u32, Ordering::Relaxed);
}
/// The new quality starts this far before the playing one reaches it, covering its start-up.
const SWITCH_START: f64 = 0.12;

/// Opens the player for a source URL.
pub type Opener = fn(&str) -> Result<Box<dyn Player>, String>;

static INSTALLED: std::sync::OnceLock<Opener> = std::sync::OnceLock::new();

/// Installs the platform's players, for platforms whose decoders live outside this crate.
pub fn install(opener: Opener) {
    let _ = INSTALLED.set(opener);
}

/// The Apple player for `url` outside the registry, for the frame-pull memory test in
/// `tests/video_memory.rs`.
#[doc(hidden)]
#[cfg(target_vendor = "apple")]
pub fn measure_player(url: &str) -> Result<Box<dyn Player>, String> {
    platform::open(url)
}

/// The opener tests' players, for their own URLs only, beside whichever opener another test
/// installed.
#[cfg(test)]
static TEST_OPENER: std::sync::OnceLock<Opener> = std::sync::OnceLock::new();

/// Every URL goes to [`TEST_OPENER`], for host tests driving built pages.
#[cfg(test)]
pub(crate) static TEST_OPENS_ALL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Opens asked so far, and the keys with a player, for host tests.
#[cfg(test)]
pub(crate) fn opens_asked() -> (u64, Vec<String>) {
    let mut keys: Vec<String> = players().keys().cloned().collect();
    keys.sort();
    (
        OPENS[0].load(Ordering::Relaxed) + OPENS[1].load(Ordering::Relaxed),
        keys,
    )
}

/// Routes every open to a player that never shows a frame.
#[cfg(test)]
pub(crate) fn open_nothing_real() {
    struct Blank;
    impl Player for Blank {
        fn has_new(&self) -> bool {
            false
        }
        fn next_frame(
            &mut self,
            _: &wgpu::Device,
            _: &wgpu::Queue,
        ) -> Result<Option<(wgpu::Texture, [u16; 2])>, String> {
            Ok(None)
        }
    }
    TEST_OPENS_ALL.store(true, Ordering::SeqCst);
    let _ = TEST_OPENER.set(|_| Ok(Box::new(Blank)));
}

fn open(source: &str) -> Result<Box<dyn Player>, String> {
    #[cfg(test)]
    if (source.starts_with("https://example.invalid/") || TEST_OPENS_ALL.load(Ordering::SeqCst))
        && let Some(opener) = TEST_OPENER.get()
    {
        return opener(source);
    }
    if let Some(opener) = INSTALLED.get() {
        return opener(source);
    }
    platform::open(source)
}

enum State {
    Opening,
    Ready(Box<dyn Player>),
    Failed(VideoError),
}

/// What each page's last scene asked of its players, by key.
static SOURCES: LazyLock<Mutex<HashMap<String, Result<Source, VideoError>>>> =
    LazyLock::new(Default::default);

fn sources() -> MutexGuard<'static, HashMap<String, Result<Source, VideoError>>> {
    SOURCES.lock().unwrap_or_else(|p| p.into_inner())
}

/// A scene asks `key` to play `source`: a new URL reopens the player, which keeps showing its
/// last frame until the new one's first.
pub(crate) fn configure(key: &str, source: Result<Source, VideoError>) {
    let mut sources = sources();
    let previous = sources.get(key).cloned();
    if previous.as_ref() == Some(&source) {
        return;
    }
    let url =
        |source: &Result<Source, VideoError>| source.as_ref().ok().map(|s| s.variants.clone());
    let reopen = previous
        .as_ref()
        .is_some_and(|previous| url(previous) != url(&source));
    sources.insert(key.to_owned(), source);
    drop(sources);
    let mut configured = CONFIGURED.lock().unwrap_or_else(|p| p.into_inner());
    if configured.len() > 64 {
        configured.clear();
    }
    configured
        .entry(key.to_owned())
        .or_insert_with(Instant::now);
    drop(configured);
    // Open as the scene names the source, not at its first paint a frame later.
    {
        let mut open = players();
        if !open.contains_key(key) {
            touch(&mut open, key);
        }
    }
    if reopen {
        let mut players = players();
        if let Some(entry) = players.get_mut(key) {
            entry.applied = None;
            entry.switch = None;
            entry.quality = None;
            entry.auto = None;
            entry.auto_decided = false;
            let _closed = std::mem::replace(&mut entry.state, State::Opening);
            request(key, &mut entry.state, Reason::Reopen);
        }
    }
}

/// When each key's source was first configured, for the first-frame phases.
static CONFIGURED: LazyLock<Mutex<HashMap<String, Instant>>> = LazyLock::new(Default::default);

fn ms(duration: std::time::Duration) -> f64 {
    (duration.as_secs_f64() * 10_000.0).round() / 10.0
}

fn source_of(key: &str) -> Option<Source> {
    sources().get(key)?.as_ref().ok().cloned()
}

/// Why a player open was asked, as the open log line names it.
#[derive(Clone, Copy, Debug)]
enum Reason {
    /// A key drawn or configured for the first time.
    First,
    /// A key whose player closed idle, drawn again.
    Returned,
    /// The scene named another URL.
    Reopen,
    Retry,
    /// A file quality chosen before the player was ready.
    Quality,
    /// Asked again while the previous open was in flight.
    Again,
}

/// Players opening at once, across pages: a platform media service flooded with creates hangs
/// them, and a hung create can take the app down with it.
const OPENS_AT_ONCE: usize = if cfg!(any(target_os = "android", target_env = "ohos")) {
    2
} else {
    4
};

/// Repeated opens of one key without a frame between wait this long, doubling up to
/// [`BACKOFF_MAX`].
const BACKOFF: Duration = Duration::from_millis(250);
const BACKOFF_MAX: Duration = Duration::from_secs(4);
/// An open asked this long after the key's last one starts the backoff over.
const BACKOFF_RESET: Duration = Duration::from_secs(30);

#[derive(Default)]
struct Flights {
    /// Keys whose open is queued or running; true once asked again meanwhile.
    open: HashMap<String, bool>,
    /// Per key: opens asked since its last frame, and when the last was asked.
    tries: HashMap<String, (u32, Instant)>,
}

static FLIGHTS: LazyLock<Mutex<Flights>> = LazyLock::new(Default::default);

fn flights() -> MutexGuard<'static, Flights> {
    FLIGHTS.lock().unwrap_or_else(|p| p.into_inner())
}

/// Opens started, asks folded into one in flight, and opens held back by the backoff.
static OPENS: [AtomicU64; 3] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];

/// How long the next open of a key asked `tries` times since its last frame waits.
fn backoff(tries: u32) -> Duration {
    match tries {
        0 => Duration::ZERO,
        n => BACKOFF.saturating_mul(1 << (n - 1).min(8)).min(BACKOFF_MAX),
    }
}

/// Asks the opener for `key`'s player: one open per key at a time, a repeat folding into the
/// one in flight, and a key reopening without ever showing a frame backing off.
fn request(key: &str, state: &mut State, reason: Reason) {
    let delay = {
        let mut flights = flights();
        if let Some(again) = flights.open.get_mut(key) {
            *again = true;
            OPENS[1].fetch_add(1, Ordering::Relaxed);
            return;
        }
        flights.open.insert(key.to_owned(), false);
        if flights.tries.len() > 256 {
            flights
                .tries
                .retain(|_, (_, at)| at.elapsed() < BACKOFF_RESET);
        }
        let now = Instant::now();
        let tries = flights.tries.entry(key.to_owned()).or_insert((0, now));
        if now.duration_since(tries.1) > BACKOFF_RESET {
            tries.0 = 0;
        }
        let delay = backoff(tries.0);
        *tries = (tries.0 + 1, now);
        delay
    };
    let count = OPENS[0].fetch_add(1, Ordering::Relaxed) + 1;
    if !delay.is_zero() {
        OPENS[2].fetch_add(1, Ordering::Relaxed);
    }
    log_open(key, reason, delay, count);
    let due = Instant::now() + delay;
    let sent = OPENER
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .send((due, Job::Open(key.to_owned())));
    if sent.is_err() {
        flights().open.remove(key);
        *state = State::Failed(VideoError::new(
            "VIDEO_DECODERS",
            "the opener is not running",
        ));
    }
}

/// An open of `key` finished: one asked meanwhile runs now, unless what landed already
/// plays the current source.
fn landed(key: &str) {
    let again = flights().open.remove(key).unwrap_or(false);
    if !again {
        return;
    }
    let current = source_of(key).map(|source| {
        let quality = players().get(key).and_then(wanted);
        source.url(quality.as_deref()).to_owned()
    });
    let mut players = players();
    let Some(entry) = players.get_mut(key) else {
        return;
    };
    let satisfied =
        matches!(entry.state, State::Ready(_)) && current.as_deref() == Some(&entry.url);
    if !satisfied {
        if matches!(entry.state, State::Ready(_)) {
            let closed = std::mem::replace(&mut entry.state, State::Opening);
            std::thread::spawn(move || drop(closed));
        }
        request(key, &mut entry.state, Reason::Again);
    }
}

/// A frame of `key` showed: its next open starts without waiting.
fn reset_backoff(key: &str) {
    flights().tries.remove(key);
}

/// The open log, at info so release logs name what asked: at most 10 lines a second.
fn log_open(key: &str, reason: Reason, delay: Duration, count: u64) {
    static WINDOW: Mutex<Option<(Instant, u32, u32)>> = Mutex::new(None);
    let mut window = WINDOW.lock().unwrap_or_else(|p| p.into_inner());
    let now = Instant::now();
    let (start, logged, dropped) = window.get_or_insert((now, 0, 0));
    if now.duration_since(*start) >= Duration::from_secs(1) {
        if *dropped > 0 {
            log::info!(target: "lingxia_view", "video open: {dropped} more not logged");
        }
        (*start, *logged, *dropped) = (now, 0, 0);
    }
    if *logged >= 10 {
        *dropped += 1;
        return;
    }
    *logged += 1;
    log::info!(
        target: "lingxia_view",
        "video open key={key} reason={reason:?} delayMs={} #{count}",
        delay.as_millis()
    );
}

fn settings_of(key: &str) -> Option<Settings> {
    sources()
        .get(key)?
        .as_ref()
        .ok()
        .map(|s| s.settings.clone())
}

/// A play that shows no picture in this long fails as a network error, so the page can retry or
/// fall back instead of spinning.
const START_DEADLINE: Duration = Duration::from_secs(15);

/// The state of player `key`, while a page draws it.
pub(crate) fn status(key: &str) -> Option<Status> {
    let mut players = players();
    let entry = players.get_mut(key)?;
    if start_overdue(entry) {
        let error = VideoError::new(
            "VIDEO_NETWORK",
            format!("no picture {} s after play", START_DEADLINE.as_secs()),
        );
        log::warn!(target: "lingxia_view", "video {key}: {}", error.message);
        entry.switch = None;
        let closed = std::mem::replace(&mut entry.state, State::Failed(error));
        // Closing a player can wait on its decoder; the render thread does not.
        std::thread::spawn(move || drop(closed));
    }
    let mut status = match &entry.state {
        State::Ready(player) => player.status(),
        State::Opening => Status::default(),
        State::Failed(error) => Status {
            error: Some(error.clone()),
            ..Status::default()
        },
    };
    // A hidden page's hold is not the page's pause: the player still reads as playing.
    if entry.resume {
        status.paused = false;
    }
    if let Some(frame) = &entry.frame {
        status.ready_state = "ready";
        [status.video_width, status.video_height] = frame.size.map(u32::from);
    } else if status.duration.is_some() {
        status.ready_state = "metadata";
    }
    let source = source_of(key);
    let labels = source.as_ref().map(Source::labels).unwrap_or_default();
    status.qualities = match (&entry.state, labels.is_empty()) {
        (State::Ready(player), true) => player.qualities(),
        _ => labels,
    };
    status.quality = entry.quality.clone().unwrap_or_else(|| "auto".into());
    status.shown = source
        .and_then(|source| {
            source
                .variants
                .into_iter()
                .find(|(label, url)| !label.is_empty() && *url == entry.url)
                .map(|(label, _)| label)
        })
        .or_else(|| (status.video_height > 0).then(|| format!("{}p", status.video_height)))
        .unwrap_or_default();
    Some(status)
}

/// Times a play waiting for its first picture; only while pages poll, so a hidden page's wait does
/// not count.
fn start_overdue(entry: &mut Entry) -> bool {
    let waiting = match &entry.state {
        State::Ready(player) => entry.frame.is_none() && !entry.resume && !player.status().paused,
        _ => false,
    };
    let overdue;
    (entry.waiting, overdue) = start_wait(entry.waiting, waiting, Instant::now());
    overdue
}

fn start_wait(
    wait: Option<(Instant, Instant)>,
    waiting: bool,
    now: Instant,
) -> (Option<(Instant, Instant)>, bool) {
    if !waiting {
        return (None, false);
    }
    let since = match wait {
        // Not polled for a while (its page was away): the wait starts again.
        Some((since, seen)) if now.duration_since(seen) < Duration::from_secs(1) => since,
        _ => now,
    };
    (
        Some((since, now)),
        now.duration_since(since) >= START_DEADLINE,
    )
}

/// Applies `op` to player `key`, or queues it while the player opens.
pub(crate) fn command(key: &str, op: VideoOp) -> Result<(), VideoError> {
    let mut players = players();
    let Some(entry) = players.get_mut(key) else {
        return Err(VideoError::new(
            "VIDEO_NO_PLAYER",
            "no <video> on this page shows this player",
        ));
    };
    if let VideoOp::SetQuality(label) = op {
        return set_quality(key, entry, label);
    }
    // A held player takes play and pause as what it does once its page shows again.
    if matches!(entry.state, State::Ready(_))
        && is_hidden(key)
        && matches!(op, VideoOp::Play | VideoOp::Pause)
    {
        entry.resume = op == VideoOp::Play;
        return Ok(());
    }
    match (&mut entry.state, op) {
        (_, VideoOp::Retry) => {
            entry.applied = None;
            entry.switch = None;
            entry.frame_errors = 0;
            entry.waiting = None;
            let _closed = std::mem::replace(&mut entry.state, State::Opening);
            request(key, &mut entry.state, Reason::Retry);
            Ok(())
        }
        (State::Ready(player), op) => player.command(&op),
        (State::Opening, op) => {
            entry.pending.push(op);
            Ok(())
        }
        (State::Failed(error), _) => Err(error.clone()),
    }
}

/// The label a player's file resolves from: its pin, else what auto chose.
fn wanted(entry: &Entry) -> Option<String> {
    entry.quality.clone().or_else(|| entry.auto.clone())
}

/// Auto's file for a box `height` device px tall, chosen when the box is first laid out: a
/// player with no picture yet reopens on it; one already showing play keeps its file rather than
/// rebuffer.
fn choose_auto(key: &str, entry: &mut Entry, height: f32) {
    if entry.auto_decided || entry.quality.is_some() || !(height >= 1.0) {
        return;
    }
    entry.auto_decided = true;
    let Some(source) = source_of(key) else {
        return;
    };
    let auto = source.auto_label(height).map(str::to_owned);
    if auto == entry.auto {
        return;
    }
    if let State::Ready(player) = &entry.state
        && entry.frame.is_some()
    {
        let now = player.status();
        if !(now.paused && now.current_time == 0.0) {
            return;
        }
    }
    let url = source.url(auto.as_deref()).to_owned();
    entry.auto = auto;
    if url == entry.url {
        return;
    }
    if matches!(entry.state, State::Ready(_)) {
        entry.url = url;
        entry.applied = None;
        let _closed = std::mem::replace(&mut entry.state, State::Opening);
    }
    // An open in flight lands on the first file and reopens on this one.
    request(key, &mut entry.state, Reason::Quality);
}

/// Picks quality `label` of player `key`: a file of `sources` opens beside the playing one and
/// takes over at the same position; an adaptive stream switches renditions itself.
fn set_quality(key: &str, entry: &mut Entry, label: Option<String>) -> Result<(), VideoError> {
    let Some(source) = source_of(key) else {
        return Err(VideoError::new(
            "VIDEO_NO_PLAYER",
            "this player has no source yet",
        ));
    };
    let labels = source.labels();
    if labels.is_empty() {
        let State::Ready(player) = &mut entry.state else {
            return Err(VideoError::new(
                "VIDEO_SOURCE",
                "the video is still opening",
            ));
        };
        let choices = player.qualities();
        if let Some(label) = &label
            && !choices.contains(label)
        {
            return Err(unknown_quality(label, &choices));
        }
        if label == entry.quality && entry.switch.is_none() {
            return Ok(());
        }
        let now = player.status();
        if player.quality_in_place(label.as_deref()) || (now.paused && now.current_time == 0.0) {
            player.set_quality(label.as_deref())?;
            // The latest choice wins: a switch still opening for an earlier one is dropped.
            entry.switch = None;
            entry.quality = label;
            return Ok(());
        }
        // A pin would otherwise wait out the buffered segments: a second player of the same
        // stream, pinned before it loads, takes over seamlessly like a file switch.
        let url = entry.url.clone();
        return start_switch(key, entry, label, url);
    }
    if let Some(label) = &label
        && !labels.contains(label)
    {
        return Err(unknown_quality(label, &labels));
    }
    if label.is_none() {
        // An explicit auto chooses again for the box as it is now.
        entry.auto = entry
            .view
            .and_then(|(px, _, _)| source.auto_label(px[1]))
            .map(str::to_owned);
        entry.auto_decided |= entry.view.is_some();
    }
    let url = source
        .url(label.as_deref().or(entry.auto.as_deref()))
        .to_owned();
    if entry
        .switch
        .as_ref()
        .is_some_and(|switch| switch.url == url)
    {
        entry.switch.as_mut().unwrap().label = label;
        return Ok(());
    }
    if url == entry.url || !matches!(entry.state, State::Ready(_)) {
        // Same file, or nothing playing yet: the choice applies as the player opens.
        entry.switch = None;
        if url != entry.url {
            entry.url = url;
            entry.applied = None;
            let _closed = std::mem::replace(&mut entry.state, State::Opening);
            request(key, &mut entry.state, Reason::Quality);
        }
        entry.quality = label;
        return Ok(());
    }
    start_switch(key, entry, label, url)
}

fn start_switch(
    key: &str,
    entry: &mut Entry,
    label: Option<String>,
    url: String,
) -> Result<(), VideoError> {
    entry.switch = Some(Switch {
        label,
        url: url.clone(),
        asked: Instant::now(),
        player: None,
        target: 0.0,
        started: false,
    });
    let sent = OPENER
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .send((Instant::now(), Job::Switch(key.to_owned(), url)));
    if sent.is_err() {
        entry.switch = None;
        return Err(VideoError::new(
            "VIDEO_DECODERS",
            "the opener is not running",
        ));
    }
    Ok(())
}

fn unknown_quality(label: &str, labels: &[String]) -> VideoError {
    VideoError::new(
        "VIDEO_SOURCE",
        format!(
            "no quality \"{label}\"; pick \"auto\" or one of {}",
            labels
                .iter()
                .map(|l| format!("\"{l}\""))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )
}

/// A quality switch in flight: the next file's player, parked at `target` until the playing one
/// reaches it.
struct Switch {
    label: Option<String>,
    url: String,
    asked: Instant,
    player: Option<Box<dyn Player>>,
    /// Media seconds the next player is parked at.
    target: f64,
    /// The next player was told to play.
    started: bool,
}

/// The last switches: label, ms from the ask to the swap, the frame gap across it and the
/// position step at it.
static SWITCHES: Mutex<Vec<serde_json::Value>> = Mutex::new(Vec::new());

enum Job {
    Open(String),
    Switch(String, String),
}

struct Frame {
    view: wgpu::TextureView,
    size: [u16; 2],
}

struct Entry {
    state: State,
    frame: Option<Frame>,
    drawn: Instant,
    requested: Instant,
    /// The first frame's import, until a present shows it.
    unshown: Option<Instant>,
    first_frame_ms: Option<u64>,
    first_present_us: Option<u64>,
    /// First-frame phases, ms: scene configure to the open request, queued for the opener,
    /// platform open, open to metadata, metadata to the first new frame, and that to its import.
    phases: [Option<f64>; 6],
    ready_at: Option<Instant>,
    loaded_at: Option<Instant>,
    /// The first page draw showing this player, and its first frame's import after it.
    /// When a frame drawing the player's box first reached the screen.
    shown_at: Option<Instant>,
    after_shown_ms: Option<f64>,
    new_at: Option<Instant>,
    frames: u64,
    import_us: [u64; 2],
    /// The props last applied; the player keeps a handle's change until they differ.
    applied: Option<Settings>,
    /// Commands sent while the player opened.
    pending: Vec<VideoOp>,
    /// The URL playing; the chosen quality (`None`: auto) and a switch to another.
    url: String,
    quality: Option<String>,
    /// The file auto chose for the box (`None`: the first), and whether the box has chosen.
    auto: Option<String>,
    auto_decided: bool,
    switch: Option<Switch>,
    /// When the last frame was imported.
    imported: Option<Instant>,
    /// Paused because its page hid while it played; plays again when the page shows.
    resume: bool,
    /// Frames that failed in a row.
    frame_errors: u8,
    /// A play waiting for its first picture: since when, and when a page last polled it.
    waiting: Option<(Instant, Instant)>,
    /// The box "auto" stays within, and when it was last drawn that large.
    view: Option<([f32; 2], Fit, Instant)>,
}

static PLAYERS: LazyLock<Mutex<HashMap<String, Entry>>> = LazyLock::new(Default::default);

fn players() -> MutexGuard<'static, HashMap<String, Entry>> {
    PLAYERS.lock().unwrap_or_else(|p| p.into_inner())
}

/// Redraws without the worker: count, skipped for want of a new frame, then total and max µs of
/// render, present, scene, images, encode and submit.
static REBINDS: Mutex<[u64; 14]> = Mutex::new([0; 14]);

/// Opens players and closes idle ones, so neither stalls a render.
static OPENER: LazyLock<Mutex<mpsc::Sender<(Instant, Job)>>> = LazyLock::new(|| {
    let (sender, receiver) = mpsc::channel::<(Instant, Job)>();
    // A few open threads: a stream's manifest fetch does not hold up a file behind it, and the
    // platform's media service never sees more than a few creates at once.
    let (pool, jobs) = mpsc::channel::<Job>();
    let jobs = std::sync::Arc::new(Mutex::new(jobs));
    for _ in 0..OPENS_AT_ONCE {
        let jobs = jobs.clone();
        let spawned = std::thread::Builder::new()
            .name("lingxia-video-open".into())
            .spawn(move || {
                loop {
                    let job = jobs.lock().unwrap_or_else(|p| p.into_inner()).recv();
                    match job {
                        Ok(Job::Open(key)) => {
                            open_player(&key);
                            landed(&key);
                        }
                        Ok(Job::Switch(key, url)) => open_switch(&key, &url),
                        Err(_) => return,
                    }
                }
            });
        if let Err(error) = spawned {
            log::error!(target: "lingxia_view", "video open thread did not start: {error}");
        }
    }
    let spawned = std::thread::Builder::new()
        .name("lingxia-video".into())
        .spawn(move || {
            // Opens held back by the backoff, by when each is due.
            let mut waiting: Vec<(Instant, Job)> = Vec::new();
            loop {
                let now = Instant::now();
                let wait = waiting
                    .iter()
                    .map(|(due, _)| due.saturating_duration_since(now))
                    .min()
                    .unwrap_or(Duration::from_millis(500))
                    .min(Duration::from_millis(500));
                match receiver.recv_timeout(wait) {
                    Ok(job) => waiting.push(job),
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
                let now = Instant::now();
                let (due, later): (Vec<_>, Vec<_>) =
                    waiting.drain(..).partition(|(due, _)| *due <= now);
                waiting = later;
                for (_, job) in due {
                    if pool.send(job).is_err() {
                        log::error!(target: "lingxia_view", "video open threads are gone");
                    }
                }
                // A shown page's player out of view closes after IDLE; a hidden page's, paused,
                // after HIDDEN_IDLE. Each leaves its last frame as a still.
                let idle: Vec<Entry> = {
                    let mut players = players();
                    let keys: Vec<String> = players
                        .iter()
                        .filter(|(key, entry)| {
                            // A frame still drawn keeps its player, hidden or not: closing it
                            // would only reopen it at the next draw.
                            !entry.url.starts_with("lx-webview://")
                                && entry.drawn.elapsed() > IDLE
                                && (!is_hidden(key) || hidden_long(key))
                        })
                        .map(|(key, _)| key.clone())
                        .collect();
                    keys.iter()
                        .filter_map(|key| {
                            let mut entry = players.remove(key)?;
                            keep_still(key, &mut entry);
                            Some(entry)
                        })
                        .collect()
                };
                if !idle.is_empty() {
                    std::thread::spawn(move || drop(idle));
                }
            }
        });
    if let Err(error) = spawned {
        log::error!(target: "lingxia_view", "video opener did not start: {error}");
    }
    Mutex::new(sender)
});

/// Scopes of pages that are hidden: their players hold still and keep their place.
static HIDDEN: LazyLock<Mutex<HashMap<u64, Instant>>> = LazyLock::new(Default::default);

/// How long a hidden page keeps its paused decoders; after it they close, leaving stills.
const HIDDEN_IDLE: Duration = Duration::from_secs(30);

/// A closed player's last frame and place: its box shows the frame until the next player's
/// first one, which starts from the same place, so the box never shows the page behind it.
struct Still {
    frame: Frame,
    time: f64,
    playing: bool,
}

/// Bounded: each holds one frame's texture.
const STILLS_MAX: usize = 16;
static STILLS: LazyLock<Mutex<Vec<(String, Still)>>> = LazyLock::new(Default::default);

fn stills() -> MutexGuard<'static, Vec<(String, Still)>> {
    STILLS.lock().unwrap_or_else(|p| p.into_inner())
}

fn still_frame(key: &str) -> Option<(wgpu::TextureView, [u16; 2])> {
    stills()
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, still)| (still.frame.view.clone(), still.frame.size))
}

/// Keeps a closing player's last frame and place under its key.
fn keep_still(key: &str, entry: &mut Entry) {
    let Some(frame) = entry.frame.take() else {
        return;
    };
    let (time, playing) = match &entry.state {
        State::Ready(player) => {
            let status = player.status();
            (status.current_time, !status.paused || entry.resume)
        }
        _ => return,
    };
    let mut stills = stills();
    stills.retain(|(k, _)| k != key);
    if stills.len() >= STILLS_MAX {
        stills.remove(0);
    }
    stills.push((
        key.to_owned(),
        Still {
            frame,
            time,
            playing,
        },
    ));
}

/// Whether `key`'s page has been hidden longer than its decoders are kept.
fn hidden_long(key: &str) -> bool {
    scope_of(key).is_some_and(|scope| {
        HIDDEN
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&scope)
            .is_some_and(|since| since.elapsed() > HIDDEN_IDLE)
    })
}

fn scope_of(key: &str) -> Option<u64> {
    key.split_once('/')
        .and_then(|(scope, _)| scope.parse().ok())
}

fn is_hidden(key: &str) -> bool {
    scope_of(key).is_some_and(|scope| {
        HIDDEN
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains_key(&scope)
    })
}

/// A page hid or showed: its playing players pause and keep their position while it is hidden,
/// and those play again when it shows; ones paused before stay paused.
pub(crate) fn set_hidden(scope: u64, hidden: bool) {
    {
        let mut scopes = HIDDEN.lock().unwrap_or_else(|p| p.into_inner());
        let changed = if hidden {
            scopes.insert(scope, Instant::now()).is_none()
        } else {
            scopes.remove(&scope).is_some()
        };
        if !changed {
            return;
        }
    }
    let mut players = players();
    let now = Instant::now();
    for (key, entry) in players
        .iter_mut()
        .filter(|(key, _)| scope_of(key) == Some(scope))
    {
        // Shown again: not idle, whatever time passed while hidden.
        entry.drawn = now;
        let State::Ready(player) = &mut entry.state else {
            continue;
        };
        let op = if hidden && !player.status().paused {
            entry.resume = true;
            VideoOp::Pause
        } else if !hidden && std::mem::take(&mut entry.resume) {
            VideoOp::Play
        } else {
            continue;
        };
        if let Err(error) = player.command(&op) {
            log::warn!(target: "lingxia_view", "video {key}: {}", error.message);
        }
    }
}

/// Scopes of closed pages, newest last. Scopes are never reused, so a late draw of a closed
/// page's key must not open a player nobody will close.
const GONE_MAX: usize = 256;
static GONE: Mutex<std::collections::VecDeque<u64>> = Mutex::new(std::collections::VecDeque::new());

fn is_gone(key: &str) -> bool {
    scope_of(key).is_some_and(|scope| {
        GONE.lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(&scope)
    })
}

/// The page closed: its players close now, and its sources and stills go.
pub(crate) fn forget(scope: u64) {
    {
        let mut gone = GONE.lock().unwrap_or_else(|p| p.into_inner());
        if gone.len() >= GONE_MAX {
            gone.pop_front();
        }
        gone.push_back(scope);
    }
    HIDDEN
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&scope);
    let ours = |key: &String| scope_of(key) == Some(scope);
    stills().retain(|(key, _)| !ours(key));
    sources().retain(|key, _| !ours(key));
    CONFIGURED
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .retain(|key, _| !ours(key));
    flights().tries.retain(|key, _| !ours(key));
    let closed: Vec<Entry> = {
        let mut players = players();
        let keys: Vec<String> = players.keys().filter(|key| ours(key)).cloned().collect();
        keys.iter().filter_map(|key| players.remove(key)).collect()
    };
    if !closed.is_empty() {
        // A platform release can block; the page's teardown must not wait on it.
        std::thread::spawn(move || drop(closed));
    }
}

/// Drops a single player `key` (such as an unmounted `<webview>`).
pub(crate) fn forget_key(key: &str) {
    stills().retain(|(k, _)| k != key);
    sources().remove(key);
    CONFIGURED
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(key);
    flights().tries.remove(key);
    if let Some(closed) = players().remove(key) {
        std::thread::spawn(move || drop(closed));
    }
}

fn open_player(key: &str) {
    let picked = Instant::now();
    let source = sources().get(key).cloned();
    let quality = players().get(key).and_then(wanted);
    let mut url = String::new();
    let mut state = match source {
        Some(Ok(source)) => {
            url = source.url(quality.as_deref()).to_owned();
            match open(&url) {
                Ok(mut player) => {
                    player.apply(&source.settings, None);
                    State::Ready(player)
                }
                Err(error) => {
                    log::warn!(target: "lingxia_view", "video {key}: {error}");
                    State::Failed(VideoError::from_message(&error))
                }
            }
        }
        Some(Err(error)) => State::Failed(error),
        None => State::Failed(VideoError::new("VIDEO_SOURCE", "no source")),
    };
    // Opens run in parallel: one overtaken by a newer source for the key closes unused.
    let current = source_of(key).map(|source| {
        let quality = players().get(key).and_then(wanted);
        source.url(quality.as_deref()).to_owned()
    });
    if !url.is_empty() && current.as_deref() != Some(url.as_str()) {
        return;
    }
    let mut wake = false;
    // Evicted while opening: the player closes here, unused.
    let _unused = match players().get_mut(key) {
        Some(entry) => {
            let now = Instant::now();
            entry.phases[1] = Some(ms(picked.saturating_duration_since(entry.requested)));
            entry.phases[2] = Some(ms(now.saturating_duration_since(picked)));
            entry.ready_at = Some(now);
            if let State::Ready(player) = &mut state {
                entry.applied = settings_of(key);
                // A reopened stream keeps its pin; auto never replaces it.
                if entry.quality.is_some()
                    && source_of(key).is_some_and(|source| source.labels().is_empty())
                    && let Err(error) = player.set_quality(entry.quality.as_deref())
                {
                    log::warn!(target: "lingxia_view", "video {key} quality: {}", error.message);
                    entry.quality = None;
                }
                // A reopened player picks up where its still left off.
                let resume = stills()
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, still)| (still.time, still.playing));
                if let Some((time, playing)) = resume {
                    entry.pending.insert(0, VideoOp::Seek(time));
                    entry.pending.insert(
                        1,
                        if playing {
                            VideoOp::Play
                        } else {
                            VideoOp::Pause
                        },
                    );
                }
                if is_hidden(key) && !player.status().paused {
                    entry.resume = true;
                    entry.pending.push(VideoOp::Pause);
                }
                if entry
                    .pending
                    .iter()
                    .any(|op| matches!(op, VideoOp::Seek(_)))
                {
                    player.discard_first();
                }
                for op in entry.pending.drain(..) {
                    if let Err(error) = player.command(&op) {
                        log::warn!(target: "lingxia_view", "video {key}: {}", error.message);
                    }
                }
            }
            entry.url = url;
            wake = entry.frame.is_none() && matches!(state, State::Ready(_));
            std::mem::replace(&mut entry.state, state)
        }
        None => state,
    };
    drop(_unused);
    if wake {
        crate::renderer::surfaces::wake_video();
    }
}

/// Opens the next quality's file paused, matched to the playing one's sound and rate, and parks
/// it a lead ahead of the playing position.
fn open_switch(key: &str, url: &str) {
    let Some(source) = source_of(key) else {
        return;
    };
    let label = players()
        .get(key)
        .and_then(|entry| entry.switch.as_ref().map(|switch| switch.label.clone()))
        .flatten();
    let mut next = match open(url) {
        Ok(mut player) if source.labels().is_empty() => {
            match player.set_quality(label.as_deref()) {
                Ok(()) => player,
                Err(error) => {
                    log::warn!(target: "lingxia_view", "video {key} quality: {}", error.message);
                    if let Some(entry) = players().get_mut(key) {
                        entry.switch = None;
                    }
                    return;
                }
            }
        }
        Ok(player) => player,
        Err(error) => {
            log::warn!(target: "lingxia_view", "video {key} quality: {error}");
            if let Some(entry) = players().get_mut(key) {
                entry.switch = None;
            }
            return;
        }
    };
    // The switch shows the next player from its parked position, never its opening frame.
    next.discard_first();
    let mut players = players();
    let Some(entry) = players.get_mut(key) else {
        return;
    };
    let State::Ready(playing) = &entry.state else {
        return;
    };
    let now = playing.status();
    let settings = Settings {
        paused: true,
        muted: now.muted,
        looped: source.settings.looped,
        rate: now.playback_rate,
        volume: now.volume,
    };
    next.apply(&settings, None);
    let target = if now.paused {
        now.current_time
    } else {
        lead(
            url,
            now.current_time,
            now.playback_rate,
            now.duration,
            source.settings.looped,
        )
    };
    let _ = next.command(&VideoOp::Seek(target));
    match &mut entry.switch {
        Some(switch) if switch.url == url => {
            // A choice made while this opened wins: a stream pins it now, a file's label follows.
            if switch.label != label
                && source.labels().is_empty()
                && let Err(error) = next.set_quality(switch.label.as_deref())
            {
                log::warn!(target: "lingxia_view", "video {key} quality: {}", error.message);
                entry.switch = None;
                return;
            }
            switch.player = Some(next);
            switch.target = target;
        }
        // Superseded or cancelled while opening.
        _ => {}
    }
}

/// The position a lead ahead of `time`: past the end of a looping source it lies in the next
/// loop, otherwise it stops short of the end.
fn lead(url: &str, time: f64, rate: f32, duration: Option<f64>, looped: bool) -> f64 {
    let seconds = f64::from(switch_lead(url).load(Ordering::Relaxed)) / 1000.0;
    park(time + seconds * f64::from(rate.max(0.25)), duration, looped)
}

fn park(ahead: f64, duration: Option<f64>, looped: bool) -> f64 {
    match duration {
        Some(duration) if looped && duration > 0.1 => ahead.rem_euclid(duration),
        Some(duration) => ahead.min((duration - 0.05).max(0.0)),
        None => ahead,
    }
}

/// Media seconds from `now` until the playing one reaches `target`; negative once past it. A
/// looping source reaches a target behind it on its next loop.
fn until(now: f64, target: f64, duration: Option<f64>, looped: bool) -> f64 {
    let gap = target - now;
    match duration {
        Some(duration) if looped && duration > 0.1 => {
            let gap = gap.rem_euclid(duration);
            if gap > duration / 2.0 {
                gap - duration
            } else {
                gap
            }
        }
        _ => gap,
    }
}

/// What a switch does at a refresh while the playing one plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Wait,
    /// It passed the parked position before the next player was ready.
    Repark,
    /// Start the parked player so its frames are ready as the position arrives.
    Start,
    /// The started player takes over.
    Bind,
}

fn step(now: &Status, target: f64, started: bool, looped: bool) -> Step {
    let ahead = until(now.current_time, target, now.duration, looped);
    if started {
        if ahead > 0.0 { Step::Wait } else { Step::Bind }
    } else if ahead < -0.05 {
        Step::Repark
    } else if ahead > SWITCH_START * f64::from(now.playback_rate) {
        Step::Wait
    } else {
        Step::Start
    }
}

/// Moves a switch on at a refresh: starts the next player just before the playing one reaches
/// its position, then binds its frame once the playing one has, so the picture never stalls.
fn advance_switch(key: &str, entry: &mut Entry, device: &wgpu::Device, queue: &wgpu::Queue) {
    let Some(switch) = &mut entry.switch else {
        return;
    };
    let Some(next) = &mut switch.player else {
        return;
    };
    let now = match &entry.state {
        State::Ready(playing) => playing.status(),
        _ => Status::default(),
    };
    let looped = entry
        .applied
        .as_ref()
        .is_some_and(|settings| settings.looped);
    if now.paused {
        if (now.current_time - switch.target).abs() > 0.02 {
            switch.target = now.current_time;
            let _ = next.command(&VideoOp::Seek(switch.target));
            return;
        }
    } else {
        match step(&now, switch.target, switch.started, looped) {
            Step::Wait => return,
            Step::Repark => {
                // The playing one passed the parked position before it was ready: park further on.
                tune_lead(&switch.url, 2.0);
                switch.target = lead(
                    &switch.url,
                    now.current_time,
                    now.playback_rate,
                    now.duration,
                    looped,
                );
                let _ = next.command(&VideoOp::Seek(switch.target));
                return;
            }
            Step::Start => {
                // Ready in time (its parked frame is decoded): the next switch parks closer.
                let parked =
                    (next.status().current_time - switch.target).abs() < 0.05 && next.has_new();
                tune_lead(&switch.url, if parked { 0.8 } else { 1.5 });
                let _ = next.command(&VideoOp::Play);
                switch.started = true;
                return;
            }
            Step::Bind => {}
        }
    }
    let Ok(Some((texture, size))) = next.next_frame(device, queue) else {
        return;
    };
    let after = next.status().current_time;
    let switch = entry.switch.take().unwrap();
    let gap = entry.imported.map(|at| at.elapsed().as_secs_f64() * 1000.0);
    let record = serde_json::json!({
        "player": key,
        "quality": switch.label.clone().unwrap_or_else(|| "auto".into()),
        "switchMs": switch.asked.elapsed().as_millis() as u64,
        "frameGapMs": gap.map(|g| (g * 10.0).round() / 10.0),
        "positionStep": ((after - now.current_time) * 1000.0).round() / 1000.0,
    });
    log::info!(target: "lingxia_view", "video quality switch {record}");
    {
        let mut switches = SWITCHES.lock().unwrap_or_else(|p| p.into_inner());
        if switches.len() >= 8 {
            switches.remove(0);
        }
        switches.push(record);
    }
    if entry.frame.as_ref().is_none_or(|frame| frame.size != size) {
        EPOCH.fetch_add(1, Ordering::Relaxed);
    }
    entry.frame = Some(Frame {
        view: texture.create_view(&Default::default()),
        size,
    });
    entry.imported = Some(Instant::now());
    entry.frames += 1;
    entry.url = switch.url;
    entry.quality = switch.label;
    let previous = std::mem::replace(&mut entry.state, State::Ready(switch.player.unwrap()));
    // Closing a player can wait on its decoder; the render thread does not.
    std::thread::spawn(move || drop(previous));
}

fn keys(display: &crate::DisplayList) -> Vec<&str> {
    let mut keys: Vec<&str> = display
        .commands
        .iter()
        .filter_map(|paint| match paint {
            Paint::Video { player, .. } => Some(player.as_str()),
            _ => None,
        })
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// A player of `display` is open but has no frame bound yet.
pub(crate) fn awaits_first(display: &crate::DisplayList) -> bool {
    let players = players();
    keys(display).iter().any(|key| {
        players
            .get(*key)
            .is_some_and(|entry| entry.frame.is_none() && matches!(entry.state, State::Ready(_)))
            && still_frame(key).is_none()
    })
}

pub(crate) fn draws_video(display: &crate::DisplayList) -> bool {
    display
        .commands
        .iter()
        .any(|paint| matches!(paint, Paint::Video { .. }))
}

/// The size of the newest frame of `player`, once one is bound.
pub(crate) fn frame_size(player: &str) -> Option<[u16; 2]> {
    let shown = players()
        .get(player)
        .and_then(|entry| entry.frame.as_ref().map(|frame| frame.size));
    shown.or_else(|| still_frame(player).map(|(_, size)| size))
}

/// A view of the newest frame of `player`, once one is bound.
pub(crate) fn frame_view(player: &str) -> Option<wgpu::TextureView> {
    let shown = players()
        .get(player)
        .and_then(|entry| entry.frame.as_ref().map(|frame| frame.view.clone()));
    shown.or_else(|| still_frame(player).map(|(view, _)| view))
}

/// Marks the players of `display` drawn, asking for those not open yet; true when one of them
/// has a frame newer than the one bound.
pub(crate) fn has_new(display: &crate::DisplayList) -> bool {
    let mut players = players();
    let mut new = false;
    let views = views(display);
    for key in keys(display) {
        if let Some(entry) = touch(&mut players, key) {
            if let Some(&(px, fit)) = views.get(key) {
                let now = Instant::now();
                let larger = entry.view.is_none_or(|(held, _, at)| {
                    px[0] * px[1] >= held[0] * held[1] || now.duration_since(at) > VIEW_SETTLE
                });
                if larger {
                    entry.view = Some((px, fit, now));
                }
                choose_auto(key, entry, px[1]);
            }
            if let (State::Ready(player), Some((px, fit, _))) = (&mut entry.state, entry.view) {
                player.set_view(px, fit);
            }
            if let State::Ready(player) = &entry.state {
                if entry.frame.is_none()
                    && entry.loaded_at.is_none()
                    && player.status().duration.is_some()
                {
                    entry.loaded_at = Some(Instant::now());
                }
                let fresh = player.has_new();
                if fresh && entry.new_at.is_none() {
                    entry.new_at = Some(Instant::now());
                }
                new |= fresh;
            }
            new |= entry
                .switch
                .as_ref()
                .is_some_and(|switch| switch.player.as_ref().is_some_and(|next| next.has_new()));
        }
    }
    new
}

fn touch<'a>(players: &'a mut HashMap<String, Entry>, key: &str) -> Option<&'a mut Entry> {
    if !players.contains_key(key) {
        let webview_url = source_of(key)
            .map(|s| s.url(None).to_owned())
            .filter(|u| u.starts_with("lx-webview://"));
        // Its decoder closed for a long hide, or its page closed: no new player for it.
        if (hidden_long(key) && webview_url.is_none()) || is_gone(key) {
            return None;
        }
        let now = Instant::now();
        let (state, url, synchronous) = if let Some(url) = webview_url {
            let state = match open(&url) {
                Ok(player) => State::Ready(player),
                Err(error) => State::Failed(VideoError::from_message(&error)),
            };
            (state, url, true)
        } else {
            let mut state = State::Opening;
            let reason = if still_frame(key).is_some() {
                Reason::Returned
            } else {
                Reason::First
            };
            request(key, &mut state, reason);
            (state, String::new(), false)
        };
        players.insert(
            key.to_owned(),
            Entry {
                state,
                applied: None,
                pending: Vec::new(),
                url,
                quality: None,
                auto: None,
                auto_decided: false,
                switch: None,
                imported: None,
                resume: false,
                frame: None,
                drawn: now,
                requested: now,
                unshown: None,
                first_frame_ms: None,
                first_present_us: None,
                phases: [
                    CONFIGURED
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .get(key)
                        .map(|at| ms(now.saturating_duration_since(*at))),
                    None,
                    None,
                    None,
                    None,
                    None,
                ],
                ready_at: synchronous.then_some(now),
                loaded_at: synchronous.then_some(now),
                shown_at: None,
                after_shown_ms: None,
                new_at: None,
                frames: 0,
                import_us: [0; 2],
                frame_errors: 0,
                waiting: None,
                view: None,
            },
        );
        if !synchronous {
            return None;
        }
    }
    let entry = players.get_mut(key)?;
    entry.drawn = Instant::now();
    if let State::Ready(player) = &mut entry.state
        && let Some(settings) = settings_of(key)
        && entry.applied.as_ref() != Some(&settings)
    {
        player.apply(&settings, entry.applied.as_ref());
        entry.applied = Some(settings);
    }
    Some(entry)
}

/// Imports the frame due now of every player `display` draws: whether it draws a video, and
/// whether a player gave a new frame.
pub(crate) fn import_frames(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    display: &crate::DisplayList,
) -> (bool, bool) {
    let mut imported = false;
    let keys = keys(display);
    let mut players = players();
    for key in &keys {
        let Some(entry) = touch(&mut players, key) else {
            continue;
        };
        let frames = entry.frames;
        advance_switch(key, entry, device, queue);
        if entry.frames != frames {
            imported = true;
            continue;
        }
        let State::Ready(player) = &mut entry.state else {
            continue;
        };
        let started = Instant::now();
        match player.next_frame(device, queue) {
            Ok(Some((texture, size))) => {
                if entry.frame.as_ref().is_none_or(|frame| frame.size != size) {
                    EPOCH.fetch_add(1, Ordering::Relaxed);
                }
                if entry.frame.is_none() {
                    entry.first_frame_ms = Some(entry.requested.elapsed().as_millis() as u64);
                    let now = Instant::now();
                    let new_at = entry.new_at.unwrap_or(now);
                    let loaded = entry.loaded_at.unwrap_or(new_at);
                    entry.phases[3] = entry
                        .ready_at
                        .map(|at| ms(loaded.saturating_duration_since(at)));
                    entry.phases[4] = Some(ms(new_at.saturating_duration_since(loaded)));
                    entry.phases[5] = Some(ms(now.saturating_duration_since(new_at)));
                    entry.unshown = Some(Instant::now());
                }
                if entry.frame.is_none() {
                    stills().retain(|(k, _)| k != *key);
                    reset_backoff(key);
                }
                entry.frame = Some(Frame {
                    view: texture.create_view(&Default::default()),
                    size,
                });
                let us = crate::session::metrics::micros(started);
                entry.imported = Some(Instant::now());
                entry.frames += 1;
                entry.frame_errors = 0;
                imported = true;
                entry.import_us = [entry.import_us[0] + us, entry.import_us[1].max(us)];
            }
            Ok(None) => {}
            Err(error) => {
                if let Some(failed) = frame_failed(&mut entry.frame_errors, key, &error) {
                    let closed = std::mem::replace(&mut entry.state, State::Failed(failed));
                    // Closing a player can wait on its decoder; the render thread does not.
                    std::thread::spawn(move || drop(closed));
                }
            }
        }
    }
    (!keys.is_empty(), imported)
}

/// Counts a failed frame of player `key`, logging the first of a run: the player's error once
/// `FRAME_FAILURES` fail in a row.
fn frame_failed(errors: &mut u8, key: &str, error: &str) -> Option<VideoError> {
    *errors = errors.saturating_add(1);
    if *errors == 1 {
        log::warn!(target: "lingxia_view", "video {key}: {error}");
    }
    if *errors < FRAME_FAILURES {
        return None;
    }
    *errors = 0;
    let message = if error.contains("VIDEO_") {
        error.to_owned()
    } else {
        format!("VIDEO_DECODE: {error}")
    };
    log::warn!(target: "lingxia_view", "video {key}: {FRAME_FAILURES} frames failed in a row; the player stops: {message}");
    Some(VideoError::from_message(&message))
}

/// Binds each player's newest frame under its texture id.
pub(crate) fn bind(display: &crate::DisplayList, bindings: &mut vello_gpu::TextureBindings) {
    let players = players();
    for key in keys(display) {
        if let Some(frame) = players.get(key).and_then(|entry| entry.frame.as_ref()) {
            bindings.insert(texture_id(key), frame.view.clone());
        } else if let Some((view, _)) = still_frame(key) {
            bindings.insert(texture_id(key), view);
        }
    }
}

/// `display` reached the screen: first frames imported before it are now shown.
pub(crate) fn presented(display: &crate::DisplayList) {
    let now = Instant::now();
    let mut players = players();
    for key in keys(display) {
        let Some(entry) = players.get_mut(key) else {
            continue;
        };
        let shown = *entry.shown_at.get_or_insert(now);
        if let Some(imported) = entry.unshown.take() {
            entry.first_present_us = Some(crate::session::metrics::micros(imported));
            entry.after_shown_ms = Some(ms(now.saturating_duration_since(shown)));
        }
    }
}

pub(crate) fn note_rebind(drawn: Option<[u64; 6]>) {
    let mut rebinds = REBINDS.lock().unwrap_or_else(|p| p.into_inner());
    match drawn {
        Some(times) => {
            rebinds[0] += 1;
            for (index, us) in times.into_iter().enumerate() {
                rebinds[2 + index * 2] += us;
                rebinds[3 + index * 2] = rebinds[3 + index * 2].max(us);
            }
        }
        None => rebinds[1] += 1,
    }
}

static PLANS: AtomicU64 = AtomicU64::new(0);
static PLAN_US: Mutex<[u64; 2]> = Mutex::new([0; 2]);
static QUADS: AtomicU64 = AtomicU64::new(0);

/// A kept frame's quad plan was built: its under and over layers rendered.
pub(crate) fn note_plan(us: u64) {
    PLANS.fetch_add(1, Ordering::Relaxed);
    let mut plan = PLAN_US.lock().unwrap_or_else(|p| p.into_inner());
    *plan = [plan[0] + us, plan[1].max(us)];
}

/// A redraw drew its videos as quads.
pub(crate) fn note_quads() {
    QUADS.fetch_add(1, Ordering::Relaxed);
}

/// Every eighth redraw is timed on the GPU.
pub(crate) fn sample_due() -> bool {
    static COUNT: AtomicU64 = AtomicU64::new(0);
    COUNT.fetch_add(1, Ordering::Relaxed).is_multiple_of(8)
}

/// Per-player first-frame and import costs, and the cost of redraws for new frames.
pub fn stats() -> serde_json::Value {
    let players: Vec<_> = players()
        .iter()
        .map(|(key, entry)| {
            serde_json::json!({
                "player": key,
                "state": match entry.state {
                    State::Opening => "opening",
                    State::Ready(_) => "ready",
                    State::Failed(_) => "failed",
                },
                "firstFrameMs": entry.first_frame_ms,
                "firstPresentUs": entry.first_present_us,
                "firstFramePhasesMs": entry.phases,
                "firstFrameAfterShownMs": entry.after_shown_ms,
                "frames": entry.frames,
                "importUsAvg": entry.import_us[0].checked_div(entry.frames),
                "importUsMax": entry.import_us[1],
                "size": entry.frame.as_ref().map(|frame| frame.size),
            })
        })
        .collect();
    let r = *REBINDS.lock().unwrap_or_else(|p| p.into_inner());
    let avg = |index: usize| r[2 + index * 2].checked_div(r[0]);
    let max = |index: usize| r[3 + index * 2];
    serde_json::json!({
        "players": players,
        "rebinds": r[0],
        "rebindsSkipped": r[1],
        "quadRedraws": QUADS.load(Ordering::Relaxed),
        "quadPlans": PLANS.load(Ordering::Relaxed),
        "quadPlanUs": {
            "avg": PLAN_US.lock().unwrap_or_else(|p| p.into_inner())[0]
                .checked_div(PLANS.load(Ordering::Relaxed)),
            "max": PLAN_US.lock().unwrap_or_else(|p| p.into_inner())[1],
        },
        "rebindUs": {
            "renderAvg": avg(0), "renderMax": max(0),
            "presentAvg": avg(1), "presentMax": max(1),
            "sceneAvg": avg(2), "imagesAvg": avg(3), "encodeAvg": avg(4), "submitAvg": avg(5),
        },
        "gpuUs": super::timing::stats(),
        "qualitySwitches": SWITCHES.lock().unwrap_or_else(|p| p.into_inner()).clone(),
        "opens": {
            "started": OPENS[0].load(Ordering::Relaxed),
            "folded": OPENS[1].load(Ordering::Relaxed),
            "backedOff": OPENS[2].load(Ordering::Relaxed),
            "inFlight": flights().open.len(),
        },
    })
}

/// An HLS master playlist cut to the renditions `height` pixels tall, every URI made absolute
/// against `base`: a player handed it can only play that rendition. `None` when none is that tall.
#[cfg(any(test, target_vendor = "apple"))]
fn pin_master(master: &str, base: &str, height: u32) -> Option<String> {
    let tall = |line: &str| {
        attribute(line, "RESOLUTION")
            .and_then(|size| size.split_once('x'))
            .and_then(|(_, h)| h.parse::<u32>().ok())
            == Some(height)
    };
    let mut out = String::new();
    let mut kept = 0;
    let mut lines = master.lines();
    while let Some(line) = lines.next() {
        let line = line.trim_end();
        if line.starts_with("#EXT-X-STREAM-INF:") {
            let uri = lines
                .by_ref()
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.starts_with('#'));
            if let Some(uri) = uri
                && tall(line)
            {
                out.push_str(line);
                out.push('\n');
                out.push_str(&absolute(base, uri));
                out.push('\n');
                kept += 1;
            }
        } else if line.starts_with("#EXT-X-I-FRAME-STREAM-INF:") {
            if tall(line) {
                out.push_str(&absolute_uri_attribute(line, base));
                out.push('\n');
            }
        } else if line.starts_with('#') {
            out.push_str(&absolute_uri_attribute(line, base));
            out.push('\n');
        } else if !line.is_empty() {
            out.push_str(&absolute(base, line));
            out.push('\n');
        }
    }
    (kept > 0).then_some(out)
}

/// The value of attribute `name` in a playlist tag line, unquoted.
#[cfg(any(test, target_vendor = "apple"))]
fn attribute<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let (_, list) = line.split_once(':')?;
    let mut rest = list;
    while !rest.is_empty() {
        let (key, after) = rest.split_once('=')?;
        let (value, next) = match after.strip_prefix('"') {
            Some(quoted) => {
                let end = quoted.find('"')?;
                (&quoted[..end], quoted[end + 1..].trim_start_matches(','))
            }
            None => match after.split_once(',') {
                Some((value, next)) => (value, next),
                None => (after, ""),
            },
        };
        if key.trim() == name {
            return Some(value);
        }
        rest = next;
    }
    None
}

#[cfg(any(test, target_vendor = "apple"))]
fn absolute_uri_attribute(line: &str, base: &str) -> String {
    match line.find("URI=\"") {
        Some(at) => {
            let start = at + 5;
            let end = line[start..].find('"').map_or(line.len(), |e| start + e);
            format!(
                "{}{}{}",
                &line[..start],
                absolute(base, &line[start..end]),
                &line[end..]
            )
        }
        None => line.to_owned(),
    }
}

#[cfg(any(test, target_vendor = "apple"))]
fn absolute(base: &str, uri: &str) -> String {
    if uri.contains("://") {
        return uri.to_owned();
    }
    let base = base.split(['?', '#']).next().unwrap_or(base);
    if uri.starts_with('/') {
        let origin = base
            .find("://")
            .and_then(|at| {
                base[at + 3..]
                    .find('/')
                    .map(|slash| &base[..at + 3 + slash])
            })
            .unwrap_or(base);
        return format!("{origin}{uri}");
    }
    let dir = base.rfind('/').map_or(base, |slash| &base[..=slash]);
    format!("{dir}{uri}")
}

#[cfg(test)]
mod pin_master_tests {
    use super::pin_master;

    const MASTER: &str = "#EXTM3U\n\
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"Alt\",URI=\"alt/prog.m3u8\"\n\
#EXT-X-STREAM-INF:BANDWIDTH=263851,CODECS=\"mp4a.40.2, avc1.4d400d\",RESOLUTION=416x234,AUDIO=\"a\"\n\
gear1/prog_index.m3u8\n\
#EXT-X-I-FRAME-STREAM-INF:BANDWIDTH=28451,RESOLUTION=416x234,URI=\"gear1/iframe.m3u8\"\n\
#EXT-X-STREAM-INF:BANDWIDTH=1924009,CODECS=\"mp4a.40.2, avc1.4d401f\",RESOLUTION=1920x1080,AUDIO=\"a\"\n\
gear5/prog_index.m3u8\n\
#EXT-X-STREAM-INF:BANDWIDTH=41457,CODECS=\"mp4a.40.2\",AUDIO=\"a\"\n\
gear0/prog_index.m3u8\n";

    #[test]
    fn keeps_only_the_pinned_height_with_absolute_uris() {
        let cut = pin_master(MASTER, "https://h.test/x/master.m3u8?t=1", 1080).unwrap();
        assert!(cut.contains("RESOLUTION=1920x1080"));
        assert!(cut.contains("\nhttps://h.test/x/gear5/prog_index.m3u8\n"));
        assert!(!cut.contains("gear1") && !cut.contains("gear0"));
        assert!(cut.contains("URI=\"https://h.test/x/alt/prog.m3u8\""));
        assert!(cut.starts_with("#EXTM3U\n"));
    }

    #[test]
    fn a_height_no_rendition_has_gives_none() {
        assert!(pin_master(MASTER, "https://h.test/master.m3u8", 720).is_none());
    }

    #[test]
    fn root_relative_uris_keep_the_origin() {
        let master = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=640x360\n/v/360.m3u8\n";
        let cut = pin_master(master, "https://h.test/a/b/master.m3u8", 360).unwrap();
        assert!(cut.contains("\nhttps://h.test/v/360.m3u8\n"));
    }
}

#[cfg(target_vendor = "apple")]
mod platform {
    //! AVPlayer → AVPlayerItemVideoOutput (BGRA, Metal-compatible) → CVMetalTextureCache.
    use crate::scene::video::{Settings, Status, VideoError, VideoOp};
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2_metal::{MTLTexture, MTLTextureType};
    use std::ffi::{c_char, c_void};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CMTime {
        value: i64,
        timescale: i32,
        flags: u32,
        epoch: i64,
    }

    type Id = *mut c_void;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_msgSend();
        #[cfg(target_arch = "x86_64")]
        fn objc_msgSend_stret();
        fn sel_registerName(name: *const c_char) -> *const c_void;
        fn objc_getClass(name: *const c_char) -> Id;
        fn objc_retain(object: Id) -> Id;
        fn objc_release(object: Id);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFBooleanTrue: *const c_void;
        static kCFTypeDictionaryKeyCallBacks: c_void;
        static kCFTypeDictionaryValueCallBacks: c_void;
        fn CFNumberCreate(alloc: *const c_void, kind: isize, value: *const c_void)
        -> *const c_void;
        fn CFDictionaryCreate(
            alloc: *const c_void,
            keys: *const *const c_void,
            values: *const *const c_void,
            count: isize,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> *const c_void;
        fn CFRelease(object: *const c_void);
        fn CFRetain(object: *const c_void) -> *const c_void;
    }

    #[link(name = "CoreVideo", kind = "framework")]
    unsafe extern "C" {
        static kCVPixelBufferPixelFormatTypeKey: *const c_void;
        static kCVPixelBufferMetalCompatibilityKey: *const c_void;
        fn CVMetalTextureCacheCreate(
            alloc: *const c_void,
            attributes: *const c_void,
            device: Id,
            texture_attributes: *const c_void,
            cache: *mut *mut c_void,
        ) -> i32;
        fn CVMetalTextureCacheCreateTextureFromImage(
            alloc: *const c_void,
            cache: *mut c_void,
            image: *mut c_void,
            attributes: *const c_void,
            format: usize,
            width: usize,
            height: usize,
            plane: usize,
            texture: *mut *mut c_void,
        ) -> i32;
        fn CVMetalTextureGetTexture(texture: *mut c_void) -> Id;
        fn CVPixelBufferGetWidth(buffer: *mut c_void) -> usize;
        fn CVPixelBufferGetHeight(buffer: *mut c_void) -> usize;
        fn CVBufferRelease(buffer: *mut c_void);
        fn CVBufferRetain(buffer: *mut c_void) -> *mut c_void;
    }

    #[link(name = "CoreMedia", kind = "framework")]
    unsafe extern "C" {
        static kCMTimeZero: CMTime;
        fn CMTimeGetSeconds(time: CMTime) -> f64;
        fn CMTimeMakeWithSeconds(seconds: f64, timescale: i32) -> CMTime;
        fn CMSampleBufferGetImageBuffer(sample: *mut c_void) -> *mut c_void;
    }

    #[link(name = "AVFoundation", kind = "framework")]
    unsafe extern "C" {
        static AVMediaTypeVideo: Id;
    }

    #[link(name = "QuartzCore", kind = "framework")]
    unsafe extern "C" {
        fn CACurrentMediaTime() -> f64;
    }

    unsafe extern "C" {
        fn dispatch_get_global_queue(identifier: isize, flags: usize) -> *mut c_void;
    }

    /// The scheme a forced rendition's cut playlist is served under.
    const PINNED_URL: &std::ffi::CStr = c"lingxia-pinned://stream/master.m3u8";

    objc2::define_class!(
        #[unsafe(super(objc2::runtime::NSObject))]
        #[name = "LingXiaViewPinnedPlaylist"]
        #[ivars = Vec<u8>]
        struct PinnedPlaylist;

        impl PinnedPlaylist {
            #[unsafe(method(resourceLoader:shouldWaitForLoadingOfRequestedResource:))]
            fn should_wait(
                &self,
                _loader: *mut objc2::runtime::AnyObject,
                request: *mut objc2::runtime::AnyObject,
            ) -> objc2::runtime::Bool {
                use objc2::DefinedClass;
                unsafe { serve(request.cast(), self.ivars()) };
                objc2::runtime::Bool::YES
            }
        }
    );

    impl PinnedPlaylist {
        fn new(body: Vec<u8>) -> Retained<Self> {
            use objc2::AllocAnyThread;
            let this = Self::alloc().set_ivars(body);
            unsafe { objc2::msg_send![super(this), init] }
        }
    }

    /// Answers a loading request with the whole playlist.
    unsafe fn serve(request: Id, body: &[u8]) {
        unsafe {
            let info = send0(request, c"contentInformationRequest");
            if !info.is_null() {
                send1(info, c"setContentType:", ns_string(c"public.m3u-playlist"));
                let set_long: unsafe extern "C" fn(Id, *const c_void, i64) =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                set_long(info, sel(c"setContentLength:"), body.len() as i64);
                set_bool(info, c"setByteRangeAccessSupported:", false);
            }
            let data_request = send0(request, c"dataRequest");
            if !data_request.is_null() {
                let bytes: unsafe extern "C" fn(Id, *const c_void, *const c_void, usize) -> Id =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                let data = bytes(
                    objc_getClass(c"NSData".as_ptr()),
                    sel(c"dataWithBytes:length:"),
                    body.as_ptr().cast(),
                    body.len(),
                );
                send1(data_request, c"respondWithData:", data);
            }
            send0(request, c"finishLoading");
        }
    }

    unsafe fn ns_string(text: &std::ffi::CStr) -> Id {
        unsafe {
            let f: unsafe extern "C" fn(Id, *const c_void, *const c_char) -> Id =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            f(
                objc_getClass(c"NSString".as_ptr()),
                sel(c"stringWithUTF8String:"),
                text.as_ptr(),
            )
        }
    }

    unsafe fn utf8(object: Id) -> Option<String> {
        if object.is_null() {
            return None;
        }
        unsafe {
            let f: unsafe extern "C" fn(Id, *const c_void) -> *const c_char =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let raw = f(object, sel(c"UTF8String"));
            (!raw.is_null()).then(|| std::ffi::CStr::from_ptr(raw).to_string_lossy().into_owned())
        }
    }

    /// The BGRA, Metal-compatible pixel buffer attributes frames are read in; the caller releases.
    unsafe fn pixel_attributes() -> *const c_void {
        unsafe {
            let format = CFNumberCreate(std::ptr::null(), 3, (&BGRA as *const i32).cast());
            let keys = [
                kCVPixelBufferPixelFormatTypeKey,
                kCVPixelBufferMetalCompatibilityKey,
            ];
            let values = [format, kCFBooleanTrue];
            let attributes = CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                2,
                (&raw const kCFTypeDictionaryKeyCallBacks).cast(),
                (&raw const kCFTypeDictionaryValueCallBacks).cast(),
            );
            CFRelease(format);
            attributes
        }
    }

    const BGRA: i32 = 0x4247_5241;
    const MTL_PIXEL_FORMAT_BGRA8_UNORM: usize = 80;

    fn sel(name: &std::ffi::CStr) -> *const c_void {
        unsafe { sel_registerName(name.as_ptr()) }
    }

    unsafe fn send0(object: Id, name: &std::ffi::CStr) -> Id {
        let f: unsafe extern "C" fn(Id, *const c_void) -> Id =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(object, sel(name)) }
    }

    unsafe fn set_bool(object: Id, name: &std::ffi::CStr, value: bool) {
        let f: unsafe extern "C" fn(Id, *const c_void, bool) =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(object, sel(name), value) }
    }

    unsafe fn set_float(object: Id, name: &std::ffi::CStr, value: f32) {
        let f: unsafe extern "C" fn(Id, *const c_void, f32) =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(object, sel(name), value) }
    }

    unsafe fn get_int(object: Id, name: &std::ffi::CStr) -> isize {
        let f: unsafe extern "C" fn(Id, *const c_void) -> isize =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(object, sel(name)) }
    }

    unsafe fn get_bool(object: Id, name: &std::ffi::CStr) -> bool {
        let f: unsafe extern "C" fn(Id, *const c_void) -> bool =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(object, sel(name)) }
    }

    unsafe fn get_float(object: Id, name: &std::ffi::CStr) -> f32 {
        let f: unsafe extern "C" fn(Id, *const c_void) -> f32 =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(object, sel(name)) }
    }

    unsafe fn send1(object: Id, name: &std::ffi::CStr, arg: Id) -> Id {
        let f: unsafe extern "C" fn(Id, *const c_void, Id) -> Id =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(object, sel(name), arg) }
    }

    /// A message returning a CMTime: x86-64 returns 24-byte structs through `_stret`.
    unsafe fn send_time(object: Id, name: &std::ffi::CStr, host: Option<f64>) -> CMTime {
        #[cfg(target_arch = "x86_64")]
        let send = objc_msgSend_stret as unsafe extern "C" fn();
        #[cfg(not(target_arch = "x86_64"))]
        let send = objc_msgSend as unsafe extern "C" fn();
        unsafe {
            match host {
                Some(host) => {
                    let f: unsafe extern "C" fn(Id, *const c_void, f64) -> CMTime =
                        std::mem::transmute(send);
                    f(object, sel(name), host)
                }
                None => {
                    let f: unsafe extern "C" fn(Id, *const c_void) -> CMTime =
                        std::mem::transmute(send);
                    f(object, sel(name))
                }
            }
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    /// An HLS variant, named by its height.
    struct Rendition {
        label: String,
        size: [f64; 2],
        peak: f64,
    }

    pub(super) struct Player {
        player: Id,
        item: Id,
        output: Id,
        cache: *mut c_void,
        renditions: Vec<Rendition>,
        /// An HLS stream's URL and master playlist, read at open.
        master: Option<(String, String)>,
        /// Serves the cut playlist of a forced rendition; the resource loader holds it weakly.
        loader: Option<Retained<PinnedPlaylist>>,
        /// The page pinned a rendition; otherwise auto stays within `view`.
        pinned: bool,
        view: Option<([f32; 2], super::Fit)>,
        looped: std::cell::Cell<bool>,
        ended: std::cell::Cell<bool>,
        rate: f32,
        /// The file's first frame, decoded while the player readies; shown until the player's own.
        first: std::cell::Cell<*mut c_void>,
    }

    // AVPlayer and the video output are thread-safe for these calls; the registry serializes them.
    unsafe impl Send for Player {}

    struct Held(*mut c_void);
    unsafe impl Send for Held {}
    unsafe impl Sync for Held {}

    pub(super) fn open(url: &str) -> Result<Box<dyn super::Player>, String> {
        Ok(Box::new(pooled(|| Player::open(url))?))
    }

    /// The opener and closer threads have no run loop to drain autoreleased objects: without a
    /// pool every asset, item and player opened there lives, decoder and all, until the process ends.
    fn pooled<T>(job: impl FnOnce() -> T) -> T {
        objc2::rc::autoreleasepool(|_| job())
    }

    impl super::Player for Player {
        fn has_new(&self) -> bool {
            !self.first.get().is_null() || unsafe { self.due().is_some() }
        }

        fn discard_first(&mut self) {
            let first = self.first.replace(std::ptr::null_mut());
            if !first.is_null() {
                unsafe { CVBufferRelease(first) };
            }
        }

        fn next_frame(
            &mut self,
            device: &wgpu::Device,
            _queue: &wgpu::Queue,
        ) -> Result<Option<(wgpu::Texture, [u16; 2])>, String> {
            self.frame(device)
        }

        fn apply(&mut self, settings: &Settings, previous: Option<&Settings>) {
            pooled(|| self.apply_now(settings, previous))
        }

        fn command(&mut self, op: &VideoOp) -> Result<(), VideoError> {
            pooled(|| self.command_now(op))
        }

        fn qualities(&self) -> Vec<String> {
            self.renditions.iter().map(|r| r.label.clone()).collect()
        }

        fn set_quality(&mut self, label: Option<&str>) -> Result<(), VideoError> {
            pooled(|| self.set_quality_now(label))
        }

        fn set_view(&mut self, px: [f32; 2], fit: super::Fit) {
            if self.view == Some((px, fit)) {
                return;
            }
            self.view = Some((px, fit));
            if !self.pinned {
                pooled(|| self.cap());
            }
        }

        fn status(&self) -> Status {
            pooled(|| self.status_now())
        }

        fn quality_in_place(&self, label: Option<&str>) -> bool {
            // Leaving a forced rendition replaces the item, which a second player hides.
            label.is_none() && self.loader.is_none()
        }
    }

    impl Player {
        fn apply_now(&mut self, settings: &Settings, previous: Option<&Settings>) {
            let changed = |field: fn(&Settings) -> f32| {
                previous.is_none_or(|previous| field(previous) != field(settings))
            };
            self.looped.set(settings.looped);
            unsafe {
                if changed(|s| f32::from(u8::from(s.muted))) {
                    set_bool(self.player, c"setMuted:", settings.muted);
                }
                if changed(|s| s.volume) {
                    set_float(self.player, c"setVolume:", settings.volume);
                }
                if changed(|s| s.rate) {
                    self.rate = settings.rate;
                    if !self.paused() {
                        set_float(self.player, c"setRate:", settings.rate);
                    }
                }
                if changed(|s| f32::from(u8::from(s.paused))) {
                    if settings.paused {
                        send0(self.player, c"pause");
                    } else {
                        self.play();
                    }
                }
            }
        }

        fn command_now(&mut self, op: &VideoOp) -> Result<(), VideoError> {
            unsafe {
                match *op {
                    VideoOp::Play => self.play(),
                    VideoOp::Pause => {
                        send0(self.player, c"pause");
                    }
                    VideoOp::Seek(seconds) => {
                        self.ended.set(false);
                        let seek: unsafe extern "C" fn(Id, *const c_void, CMTime, CMTime, CMTime) =
                            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                        seek(
                            self.player,
                            sel(c"seekToTime:toleranceBefore:toleranceAfter:"),
                            CMTimeMakeWithSeconds(seconds, 600),
                            kCMTimeZero,
                            kCMTimeZero,
                        );
                    }
                    VideoOp::SetMuted(muted) => set_bool(self.player, c"setMuted:", muted),
                    VideoOp::SetVolume(volume) => set_float(self.player, c"setVolume:", volume),
                    VideoOp::SetRate(rate) => {
                        self.rate = rate;
                        if !self.paused() {
                            set_float(self.player, c"setRate:", rate);
                        }
                    }
                    VideoOp::Retry | VideoOp::SetQuality(_) => {}
                }
            }
            Ok(())
        }

        fn set_quality_now(&mut self, label: Option<&str>) -> Result<(), VideoError> {
            let Some(label) = label else {
                if self.loader.is_some() {
                    self.replace_item(None);
                }
                self.pinned = false;
                self.cap();
                return Ok(());
            };
            let cut = self.master.as_ref().and_then(|(url, master)| {
                let height = label.strip_suffix('p')?.parse().ok()?;
                super::pin_master(master, url, height)
            });
            if let Some(cut) = cut {
                self.replace_item(Some(cut));
                self.pinned = true;
                return Ok(());
            }
            let rendition = self
                .renditions
                .iter()
                .find(|r| r.label == label)
                .ok_or_else(|| VideoError::new("VIDEO_SOURCE", format!("no rendition {label}")))?;
            let (size, peak) = (rendition.size, rendition.peak);
            self.pinned = true;
            self.prefer(size, peak);
            Ok(())
        }

        fn status_now(&self) -> Status {
            unsafe {
                let seconds = |time: CMTime| CMTimeGetSeconds(time);
                let duration = seconds(send_time(self.item, c"duration", None));
                let control = get_int(self.player, c"timeControlStatus");
                Status {
                    current_time: seconds(send_time(self.item, c"currentTime", None)).max(0.0),
                    duration: (duration.is_finite() && duration > 0.0).then_some(duration),
                    paused: control == 0,
                    buffering: control == 1,
                    ended: self.ended.get(),
                    playback_rate: self.rate,
                    volume: get_float(self.player, c"volume"),
                    muted: get_bool(self.player, c"isMuted"),
                    error: self.error(),
                    ..Status::default()
                }
            }
        }
    }

    /// The video variants of an HLS item's manifest, highest first, one per height. Reading
    /// them loads the manifest, so this runs on the opener thread.
    unsafe fn renditions(item: Id) -> Vec<Rendition> {
        unsafe {
            let asset = send0(item, c"asset");
            let selector = sel(c"variants");
            let responds: unsafe extern "C" fn(Id, *const c_void, *const c_void) -> bool =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            if asset.is_null() || !responds(asset, sel(c"respondsToSelector:"), selector) {
                return Vec::new();
            }
            let variants = send0(asset, c"variants");
            let count = get_int(variants, c"count").max(0) as usize;
            let at: unsafe extern "C" fn(Id, *const c_void, usize) -> Id =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let size_of: unsafe extern "C" fn(Id, *const c_void) -> CGSize =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let double: unsafe extern "C" fn(Id, *const c_void) -> f64 =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let mut out: Vec<Rendition> = Vec::new();
            for index in 0..count {
                let variant = at(variants, sel(c"objectAtIndex:"), index);
                let video = send0(variant, c"videoAttributes");
                if video.is_null() {
                    continue;
                }
                let size = size_of(video, sel(c"presentationSize"));
                let peak = double(variant, sel(c"peakBitRate"));
                if size.height <= 0.0 {
                    continue;
                }
                let label = format!("{}p", size.height.round() as u32);
                match out.iter_mut().find(|r| r.label == label) {
                    Some(existing) => existing.peak = existing.peak.max(peak),
                    None => out.push(Rendition {
                        label,
                        size: [size.width, size.height],
                        peak,
                    }),
                }
            }
            out.sort_by(|a, b| b.size[1].total_cmp(&a.size[1]));
            out
        }
    }

    /// Decodes a file's first video frame with an asset reader, which takes a few ms where an
    /// AVPlayerItem takes hundreds to become ready. Null when the file gives none.
    unsafe fn first_frame(asset: Id, attributes: *const c_void) -> *mut c_void {
        unsafe {
            let tracks = send1(asset, c"tracksWithMediaType:", AVMediaTypeVideo);
            if tracks.is_null() || get_int(tracks, c"count") < 1 {
                log::debug!(target: "lingxia_view", "video first frame: no video track");
                return std::ptr::null_mut();
            }
            let at: unsafe extern "C" fn(Id, *const c_void, usize) -> Id =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let track = at(tracks, sel(c"objectAtIndex:"), 0);
            let two: unsafe extern "C" fn(Id, *const c_void, Id, Id) -> Id =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let reader = two(
                objc_getClass(c"AVAssetReader".as_ptr()),
                sel(c"assetReaderWithAsset:error:"),
                asset,
                std::ptr::null_mut(),
            );
            if reader.is_null() {
                return std::ptr::null_mut();
            }
            let output = two(
                objc_getClass(c"AVAssetReaderTrackOutput".as_ptr()),
                sel(c"assetReaderTrackOutputWithTrack:outputSettings:"),
                track,
                attributes as Id,
            );
            send1(reader, c"addOutput:", output);
            let mut image = std::ptr::null_mut();
            if get_bool(reader, c"startReading") {
                let sample = send0(output, c"copyNextSampleBuffer");
                if !sample.is_null() {
                    let buffer = CMSampleBufferGetImageBuffer(sample);
                    if !buffer.is_null() {
                        image = CVBufferRetain(buffer);
                    }
                    CFRelease(sample);
                }
            }
            send0(reader, c"cancelReading");
            image
        }
    }

    impl Player {
        fn open(url: &str) -> Result<Self, String> {
            let url = std::ffi::CString::new(url).map_err(|e| e.to_string())?;
            unsafe {
                let string = {
                    let f: unsafe extern "C" fn(Id, *const c_void, *const c_char) -> Id =
                        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                    f(
                        objc_getClass(c"NSString".as_ptr()),
                        sel(c"stringWithUTF8String:"),
                        url.as_ptr(),
                    )
                };
                let nsurl = send1(objc_getClass(c"NSURL".as_ptr()), c"URLWithString:", string);
                if nsurl.is_null() {
                    return Err("invalid URL".into());
                }
                let two: unsafe extern "C" fn(Id, *const c_void, Id, Id) -> Id =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                let asset = two(
                    objc_getClass(c"AVURLAsset".as_ptr()),
                    sel(c"URLAssetWithURL:options:"),
                    nsurl,
                    std::ptr::null_mut(),
                );
                if asset.is_null() {
                    return Err("invalid URL".into());
                }
                let item = objc_retain(send1(
                    objc_getClass(c"AVPlayerItem".as_ptr()),
                    c"playerItemWithAsset:",
                    asset,
                ));
                let attributes = pixel_attributes();
                let output = send1(
                    send0(objc_getClass(c"AVPlayerItemVideoOutput".as_ptr()), c"alloc"),
                    c"initWithPixelBufferAttributes:",
                    attributes as Id,
                );
                send1(item, c"addOutput:", output);
                let player = objc_retain(send1(
                    objc_getClass(c"AVPlayer".as_ptr()),
                    c"playerWithPlayerItem:",
                    item,
                ));
                // Decoded while the item readies, so the first frame does not wait for it.
                let first = if url.to_bytes().starts_with(b"file:") {
                    first_frame(asset, attributes)
                } else {
                    std::ptr::null_mut()
                };
                CFRelease(attributes);
                let set_int: unsafe extern "C" fn(Id, *const c_void, isize) =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                set_int(player, sel(c"setActionAtItemEnd:"), 2);
                // A local file never stalls, so playback need not wait to buffer.
                if url.to_bytes().starts_with(b"file:") {
                    let set_bool: unsafe extern "C" fn(Id, *const c_void, bool) =
                        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                    set_bool(
                        player,
                        sel(c"setAutomaticallyWaitsToMinimizeStalling:"),
                        false,
                    );
                }
                // Paused on the first frame until the page's props say play.
                send0(player, c"pause");
                let stream = url
                    .to_bytes()
                    .split(|b| *b == b'?')
                    .next()
                    .is_some_and(|path| path.ends_with(b".m3u8"));
                let renditions = if stream { renditions(item) } else { Vec::new() };
                // A pin cuts this to one rendition: AVPlayer only takes a limit, never a choice.
                let master = if stream {
                    let read: unsafe extern "C" fn(Id, *const c_void, Id, usize, *mut Id) -> Id =
                        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                    let text = read(
                        objc_getClass(c"NSString".as_ptr()),
                        sel(c"stringWithContentsOfURL:encoding:error:"),
                        nsurl,
                        4,
                        std::ptr::null_mut(),
                    );
                    utf8(text).map(|text| (url.to_string_lossy().into_owned(), text))
                } else {
                    None
                };
                Ok(Self {
                    player,
                    item,
                    output,
                    cache: std::ptr::null_mut(),
                    renditions,
                    master,
                    loader: None,
                    pinned: false,
                    view: None,
                    looped: std::cell::Cell::new(false),
                    ended: std::cell::Cell::new(false),
                    rate: 1.0,
                    first: std::cell::Cell::new(first),
                })
            }
        }

        /// Plays the stream's cut playlist `cut`, or with `None` the whole stream again, from
        /// the same position: a new item with its own output replaces the playing one.
        fn replace_item(&mut self, cut: Option<String>) {
            let Some((url, _)) = &self.master else {
                return;
            };
            unsafe {
                let at = send_time(self.item, c"currentTime", None);
                let address = match &cut {
                    Some(_) => ns_string(PINNED_URL),
                    None => match std::ffi::CString::new(url.as_str()) {
                        Ok(url) => ns_string(&url),
                        Err(_) => return,
                    },
                };
                let nsurl = send1(objc_getClass(c"NSURL".as_ptr()), c"URLWithString:", address);
                let two: unsafe extern "C" fn(Id, *const c_void, Id, Id) -> Id =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                let asset = two(
                    objc_getClass(c"AVURLAsset".as_ptr()),
                    sel(c"URLAssetWithURL:options:"),
                    nsurl,
                    std::ptr::null_mut(),
                );
                if asset.is_null() {
                    return;
                }
                let loader = cut.map(|cut| PinnedPlaylist::new(cut.into_bytes()));
                if let Some(loader) = &loader {
                    let set: unsafe extern "C" fn(Id, *const c_void, Id, *mut c_void) =
                        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                    set(
                        send0(asset, c"resourceLoader"),
                        sel(c"setDelegate:queue:"),
                        Retained::as_ptr(loader) as Id,
                        dispatch_get_global_queue(0, 0),
                    );
                }
                let item = objc_retain(send1(
                    objc_getClass(c"AVPlayerItem".as_ptr()),
                    c"playerItemWithAsset:",
                    asset,
                ));
                let attributes = pixel_attributes();
                let output = send1(
                    send0(objc_getClass(c"AVPlayerItemVideoOutput".as_ptr()), c"alloc"),
                    c"initWithPixelBufferAttributes:",
                    attributes as Id,
                );
                CFRelease(attributes);
                send1(item, c"addOutput:", output);
                send1(self.player, c"replaceCurrentItemWithPlayerItem:", item);
                objc_release(self.output);
                objc_release(self.item);
                self.item = item;
                self.output = output;
                self.loader = loader;
                let seek: unsafe extern "C" fn(Id, *const c_void, CMTime, CMTime, CMTime) =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                seek(
                    self.player,
                    sel(c"seekToTime:toleranceBefore:toleranceAfter:"),
                    at,
                    kCMTimeZero,
                    kCMTimeZero,
                );
            }
        }

        /// Auto within the picture's box: the item's largest resolution is the cap rendition's,
        /// zero (none) before layout or past every rendition. The bit rate stays the player's.
        fn cap(&mut self) {
            let sizes: Vec<[f32; 2]> = self
                .renditions
                .iter()
                .map(|r| [r.size[0] as f32, r.size[1] as f32])
                .collect();
            let cap = self
                .view
                .and_then(|(px, fit)| crate::scene::video::auto_cap(px, fit, &sizes));
            self.prefer(cap.map_or([0.0, 0.0], |i| self.renditions[i].size), 0.0);
        }

        /// The item's largest resolution and peak bit rate; zero lifts either limit.
        fn prefer(&mut self, size: [f64; 2], peak: f64) {
            unsafe {
                let set_size: unsafe extern "C" fn(Id, *const c_void, CGSize) =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                set_size(
                    self.item,
                    sel(c"setPreferredMaximumResolution:"),
                    CGSize {
                        width: size[0],
                        height: size[1],
                    },
                );
                let set_double: unsafe extern "C" fn(Id, *const c_void, f64) =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                set_double(self.item, sel(c"setPreferredPeakBitRate:"), peak);
            }
        }

        /// The frame due at this refresh as a wgpu texture, when a new one is ready.
        fn frame(
            &mut self,
            device: &wgpu::Device,
        ) -> Result<Option<(wgpu::Texture, [u16; 2])>, String> {
            unsafe {
                let copy: unsafe extern "C" fn(Id, *const c_void, CMTime, *mut CMTime) -> Id =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                let buffer = match self.due() {
                    Some(time) => copy(
                        self.output,
                        sel(c"copyPixelBufferForItemTime:itemTimeForDisplay:"),
                        time,
                        std::ptr::null_mut(),
                    ),
                    None => std::ptr::null_mut(),
                };
                // The decoded first frame stands in until the player gives a frame of its own.
                let first = self.first.replace(std::ptr::null_mut());
                let buffer = match (buffer.is_null(), first.is_null()) {
                    (true, true) => return Ok(None),
                    (true, false) => first,
                    (false, stale) => {
                        if !stale {
                            CVBufferRelease(first);
                        }
                        buffer
                    }
                };
                let result = self.import(device, buffer);
                CVBufferRelease(buffer);
                result.map(Some)
            }
        }

        /// The item time of the refresh now, when it has a new frame.
        unsafe fn due(&self) -> Option<CMTime> {
            unsafe {
                self.restart_at_end();
                let time = send_time(
                    self.output,
                    c"itemTimeForHostTime:",
                    Some(CACurrentMediaTime()),
                );
                let has_new: unsafe extern "C" fn(Id, *const c_void, CMTime) -> bool =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                has_new(self.output, sel(c"hasNewPixelBufferForItemTime:"), time).then_some(time)
            }
        }

        /// At the end a looping player starts over; any other pauses there, ended.
        unsafe fn restart_at_end(&self) {
            unsafe {
                let duration = CMTimeGetSeconds(send_time(self.item, c"duration", None));
                let now = CMTimeGetSeconds(send_time(self.item, c"currentTime", None));
                if duration.is_finite() && duration > 0.0 && now >= duration - 0.01 {
                    if self.looped.get() {
                        let seek: unsafe extern "C" fn(Id, *const c_void, CMTime) =
                            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                        seek(self.player, sel(c"seekToTime:"), kCMTimeZero);
                    } else if !self.ended.get() {
                        self.ended.set(true);
                        send0(self.player, c"pause");
                    }
                }
            }
        }

        unsafe fn paused(&self) -> bool {
            unsafe { get_int(self.player, c"timeControlStatus") == 0 }
        }

        /// Plays at the page's rate, from the start once ended.
        unsafe fn play(&self) {
            unsafe {
                if self.ended.replace(false) {
                    let seek: unsafe extern "C" fn(Id, *const c_void, CMTime) =
                        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                    seek(self.player, sel(c"seekToTime:"), kCMTimeZero);
                }
                set_float(self.player, c"setRate:", self.rate);
            }
        }

        /// The item's failure, coded by where it came from.
        unsafe fn error(&self) -> Option<VideoError> {
            unsafe {
                if get_int(self.item, c"status") != 2 {
                    return None;
                }
                let error = send0(self.item, c"error");
                let text = |object: Id| {
                    if object.is_null() {
                        return String::new();
                    }
                    let utf8: unsafe extern "C" fn(Id, *const c_void) -> *const c_char =
                        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                    let raw = utf8(object, sel(c"UTF8String"));
                    if raw.is_null() {
                        String::new()
                    } else {
                        std::ffi::CStr::from_ptr(raw).to_string_lossy().into_owned()
                    }
                };
                let message = text(send0(error, c"localizedDescription"));
                let domain = text(send0(error, c"domain"));
                let code = get_int(error, c"code");
                let kind = if domain == "NSURLErrorDomain" {
                    "VIDEO_NETWORK"
                } else if code == -1100 || code == 4 || message.contains("not found") {
                    "VIDEO_NOT_FOUND"
                } else {
                    "VIDEO_FORMAT"
                };
                Some(VideoError::new(kind, message))
            }
        }

        unsafe fn import(
            &mut self,
            device: &wgpu::Device,
            buffer: Id,
        ) -> Result<(wgpu::Texture, [u16; 2]), String> {
            unsafe {
                let hal = device
                    .as_hal::<wgpu::hal::api::Metal>()
                    .ok_or("not a Metal device")?;
                if self.cache.is_null() {
                    let raw: *const ProtocolObject<dyn objc2_metal::MTLDevice> =
                        &**hal.raw_device();
                    let status = CVMetalTextureCacheCreate(
                        std::ptr::null(),
                        std::ptr::null(),
                        raw as Id,
                        std::ptr::null(),
                        &mut self.cache,
                    );
                    if status != 0 {
                        return Err(format!("CVMetalTextureCacheCreate {status}"));
                    }
                }
                let (width, height) = (
                    CVPixelBufferGetWidth(buffer),
                    CVPixelBufferGetHeight(buffer),
                );
                let mut metal = std::ptr::null_mut();
                let status = CVMetalTextureCacheCreateTextureFromImage(
                    std::ptr::null(),
                    self.cache,
                    buffer,
                    std::ptr::null(),
                    MTL_PIXEL_FORMAT_BGRA8_UNORM,
                    width,
                    height,
                    0,
                    &mut metal,
                );
                if status != 0 || metal.is_null() {
                    return Err(format!(
                        "CVMetalTextureCacheCreateTextureFromImage {status}"
                    ));
                }
                let raw = CVMetalTextureGetTexture(metal);
                let texture: Retained<ProtocolObject<dyn MTLTexture>> =
                    Retained::retain(raw.cast()).ok_or("no Metal texture")?;
                // The CVMetalTexture keeps the IOSurface checked out until wgpu is done with it,
                // and its cache must outlive it, even when the player closes first.
                let held = Held(metal);
                let cache = Held(CFRetain(self.cache).cast_mut());
                let hal_texture = wgpu::hal::metal::Device::texture_from_raw(
                    texture,
                    wgpu::TextureFormat::Bgra8Unorm,
                    MTLTextureType::Type2D,
                    1,
                    1,
                    wgpu::hal::CopyExtent {
                        width: width as u32,
                        height: height as u32,
                        depth: 1,
                    },
                    Some(Box::new(move || {
                        let (held, cache) = (held, cache);
                        CFRelease(held.0);
                        CFRelease(cache.0);
                    })),
                );
                drop(hal);
                let texture = device.create_texture_from_hal::<wgpu::hal::api::Metal>(
                    hal_texture,
                    &wgpu::TextureDescriptor {
                        label: Some("LingXia video frame"),
                        size: wgpu::Extent3d {
                            width: width as u32,
                            height: height as u32,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Bgra8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    },
                    wgpu::TextureUses::RESOURCE,
                );
                Ok((
                    texture,
                    [
                        width.min(u16::MAX as usize) as u16,
                        height.min(u16::MAX as usize) as u16,
                    ],
                ))
            }
        }
    }

    impl Drop for Player {
        fn drop(&mut self) {
            pooled(|| unsafe {
                send0(self.player, c"pause");
                let first = self.first.get();
                if !first.is_null() {
                    CVBufferRelease(first);
                }
                objc_release(self.output);
                objc_release(self.player);
                objc_release(self.item);
                if !self.cache.is_null() {
                    CFRelease(self.cache);
                }
            })
        }
    }
}

/// Imports an RGBA8 AHardwareBuffer as a texture of `device`, sharing its memory.
///
/// # Safety
/// `buffer` must be a live `AHardwareBuffer*` allocated with GPU sampling usage; the texture
/// holds its own reference.
#[cfg(target_os = "android")]
pub unsafe fn import_hardware_buffer(
    device: &wgpu::Device,
    buffer: *mut std::ffi::c_void,
    size: [u32; 2],
) -> Result<wgpu::Texture, String> {
    use ash::{android::external_memory_android_hardware_buffer as ahb, vk};
    unsafe {
        let hal = device
            .as_hal::<wgpu::hal::api::Vulkan>()
            .ok_or("not a Vulkan device")?;
        let instance = hal.shared_instance().raw_instance();
        let raw = hal.raw_device();
        let loader = ahb::Device::new(instance, raw);
        let mut properties = vk::AndroidHardwareBufferPropertiesANDROID::default();
        loader
            .get_android_hardware_buffer_properties(buffer.cast(), &mut properties)
            .map_err(|e| format!("AHardwareBuffer properties: {e}"))?;
        let mut external = vk::ExternalMemoryImageCreateInfo::default()
            .handle_types(vk::ExternalMemoryHandleTypeFlags::ANDROID_HARDWARE_BUFFER_ANDROID);
        let info = vk::ImageCreateInfo::default()
            .push_next(&mut external)
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_UNORM)
            .extent(vk::Extent3D {
                width: size[0],
                height: size[1],
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        let image = raw
            .create_image(&info, None)
            .map_err(|e| format!("vkCreateImage: {e}"))?;
        let mut import =
            vk::ImportAndroidHardwareBufferInfoANDROID::default().buffer(buffer.cast());
        let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let allocate = vk::MemoryAllocateInfo::default()
            .push_next(&mut import)
            .push_next(&mut dedicated)
            .allocation_size(properties.allocation_size)
            .memory_type_index(properties.memory_type_bits.trailing_zeros());
        let memory = match raw.allocate_memory(&allocate, None) {
            Ok(memory) => memory,
            Err(e) => {
                raw.destroy_image(image, None);
                return Err(format!("vkAllocateMemory: {e}"));
            }
        };
        if let Err(e) = raw.bind_image_memory(image, memory, 0) {
            raw.free_memory(memory, None);
            raw.destroy_image(image, None);
            return Err(format!("vkBindImageMemory: {e}"));
        }
        let extent = wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        };
        let hal_texture = hal.texture_from_raw(
            image,
            &wgpu::hal::TextureDescriptor {
                label: Some("LingXia video frame"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUses::RESOURCE,
                memory_flags: wgpu::hal::MemoryFlags::empty(),
                view_formats: Vec::new(),
            },
            None,
            wgpu::hal::vulkan::TextureMemory::Dedicated(memory),
        );
        drop(hal);
        Ok(device.create_texture_from_hal::<wgpu::hal::api::Vulkan>(
            hal_texture,
            &wgpu::TextureDescriptor {
                label: Some("LingXia video frame"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::TextureUses::RESOURCE,
        ))
    }
}

#[cfg(not(target_vendor = "apple"))]
mod platform {
    pub(super) fn open(_url: &str) -> Result<Box<dyn super::Player>, String> {
        Err("VIDEO_UNSUPPORTED: no video player on this platform yet".into())
    }
}

#[cfg(test)]
mod start_deadline_tests {
    use super::{START_DEADLINE, start_wait};
    use std::time::{Duration, Instant};

    #[test]
    fn a_play_without_a_picture_fails_after_the_deadline_of_polled_time() {
        let t0 = Instant::now();
        let step = Duration::from_millis(500);
        let mut wait = None;
        let mut at = t0;
        let mut overdue = false;
        while at.duration_since(t0) <= START_DEADLINE {
            assert!(!overdue, "overdue before the deadline");
            (wait, overdue) = start_wait(wait, true, at);
            at += step;
        }
        assert!(overdue, "overdue at the deadline");
        // A picture, or a pause, ends the wait.
        assert_eq!(start_wait(wait, false, at), (None, false));
        // A gap in polling (the page was away) starts it again.
        let (wait, overdue) = start_wait(Some((t0, t0)), true, t0 + START_DEADLINE * 2);
        assert!(!overdue);
        assert_eq!(wait.unwrap().0, t0 + START_DEADLINE * 2);
    }
}

#[cfg(test)]
mod frame_failure_tests {
    use super::{FRAME_FAILURES, frame_failed};

    #[test]
    fn a_frame_path_failing_in_a_row_fails_its_player_once_with_a_code() {
        let mut errors = 0;
        for _ in 1..FRAME_FAILURES {
            assert!(frame_failed(&mut errors, "k", "not a Vulkan device").is_none());
        }
        let failed = frame_failed(&mut errors, "k", "not a Vulkan device").unwrap();
        assert_eq!(failed.code, "VIDEO_DECODE");
        assert!(failed.message.contains("not a Vulkan device"));
        assert_eq!(errors, 0, "a retried player counts afresh");
        // A message that names its code keeps it.
        let mut errors = FRAME_FAILURES - 1;
        let failed = frame_failed(&mut errors, "k", "GpuVideo: VIDEO_NO_PLAYER: gone").unwrap();
        assert_eq!(failed.code, "VIDEO_NO_PLAYER");
    }
}

#[cfg(test)]
mod switch_tests {
    use super::{Status, Step, park, step, until};

    fn playing(at: f64) -> Status {
        Status {
            current_time: at,
            duration: Some(8.0),
            playback_rate: 1.0,
            ..Status::default()
        }
    }

    #[test]
    fn a_looping_player_past_the_seam_reaches_its_parked_switch() {
        // Parked at 7.95 s of an 8 s loop, the playing one looped to 0.3 s: it passed the park.
        assert_eq!(step(&playing(0.3), 7.95, false, true), Step::Repark);
        assert_eq!(step(&playing(0.3), 7.95, true, true), Step::Bind);
        // Not looping, 0.3 s is still before 7.95 s.
        assert_eq!(step(&playing(0.3), 7.95, true, false), Step::Wait);
        assert_eq!(step(&playing(7.9), 7.95, false, true), Step::Start);
    }

    #[test]
    fn a_looping_park_past_the_end_lies_in_the_next_loop() {
        assert!((park(9.5, Some(8.0), true) - 1.5).abs() < 1e-9);
        assert!((park(9.5, Some(8.0), false) - 7.95).abs() < 1e-9);
        assert!((park(9.5, None, true) - 9.5).abs() < 1e-9);
    }

    #[test]
    fn a_looping_target_ahead_across_the_seam_is_still_ahead() {
        // Playing at 7.5 s of an 8 s loop, parked at 1.5 s: 2 s to go, not 6 s past.
        assert!((until(7.5, 1.5, Some(8.0), true) - 2.0).abs() < 1e-9);
        // Just past the target, across the seam.
        assert!((until(0.2, 7.9, Some(8.0), true) + 0.3).abs() < 1e-9);
        // The bug: parked at 7.95 s, the playing one looped to 0.3 s; a non-looping reading
        // waits 7.65 s, a whole loop.
        assert!((until(0.3, 7.95, Some(8.0), false) - 7.65).abs() < 1e-9);
        assert!((until(0.3, 7.95, Some(8.0), true) + 0.35).abs() < 1e-9);
    }
}

#[cfg(test)]
mod opener_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize};

    static COUNTED: AtomicUsize = AtomicUsize::new(0);
    /// Opens per URL; URLs with `slow` wait for [`GATE`] to open.
    static BY_URL: LazyLock<Mutex<HashMap<String, usize>>> = LazyLock::new(Default::default);
    static GATE: AtomicBool = AtomicBool::new(false);
    static RUNNING: AtomicUsize = AtomicUsize::new(0);
    static PEAK: AtomicUsize = AtomicUsize::new(0);
    /// The tests that hold the gate run one at a time.
    static SERIAL: Mutex<()> = Mutex::new(());

    /// Players closed, per URL.
    static CLOSED: LazyLock<Mutex<HashMap<String, usize>>> = LazyLock::new(Default::default);

    struct Idle(String);

    impl Drop for Idle {
        fn drop(&mut self) {
            *CLOSED.lock().unwrap().entry(self.0.clone()).or_default() += 1;
        }
    }

    impl Player for Idle {
        fn has_new(&self) -> bool {
            false
        }

        fn next_frame(
            &mut self,
            _: &wgpu::Device,
            _: &wgpu::Queue,
        ) -> Result<Option<(wgpu::Texture, [u16; 2])>, String> {
            Ok(None)
        }
    }

    fn counted(url: &str) -> Result<Box<dyn Player>, String> {
        let running = RUNNING.fetch_add(1, Ordering::SeqCst) + 1;
        PEAK.fetch_max(running, Ordering::SeqCst);
        *BY_URL.lock().unwrap().entry(url.to_owned()).or_default() += 1;
        if url.contains("slow") {
            let until = Instant::now() + Duration::from_secs(10);
            while !GATE.load(Ordering::SeqCst) && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        COUNTED.fetch_add(1, Ordering::SeqCst);
        RUNNING.fetch_sub(1, Ordering::SeqCst);
        if url.ends_with(".m3u8") {
            return Ok(Box::new(Stream {
                url: url.to_owned(),
                pin: None,
            }));
        }
        Ok(Box::new(Idle(url.to_owned())))
    }

    /// Pins each stream player was given last, per URL.
    static PINS: LazyLock<Mutex<HashMap<String, Vec<Option<String>>>>> =
        LazyLock::new(Default::default);

    /// An adaptive stream mid-play, with two renditions.
    struct Stream {
        url: String,
        pin: Option<String>,
    }

    impl Player for Stream {
        fn has_new(&self) -> bool {
            false
        }

        fn next_frame(
            &mut self,
            _: &wgpu::Device,
            _: &wgpu::Queue,
        ) -> Result<Option<(wgpu::Texture, [u16; 2])>, String> {
            Ok(None)
        }

        fn status(&self) -> Status {
            Status {
                current_time: 3.0,
                ..Status::default()
            }
        }

        fn qualities(&self) -> Vec<String> {
            vec!["1080p".into(), "360p".into()]
        }

        fn set_quality(&mut self, label: Option<&str>) -> Result<(), VideoError> {
            self.pin = label.map(str::to_owned);
            PINS.lock()
                .unwrap()
                .entry(self.url.clone())
                .or_default()
                .push(self.pin.clone());
            Ok(())
        }
    }

    #[test]
    fn auto_chosen_while_a_pin_opens_wins_over_it() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let _ = TEST_OPENER.set(counted);
        GATE.store(true, Ordering::SeqCst);
        let key = crate::scene::video::key(7_700_006, "name:stream-auto");
        let url = "https://example.invalid/slow-auto.m3u8";
        configure(&key, file(url));
        settle();
        GATE.store(false, Ordering::SeqCst);
        command(&key, VideoOp::SetQuality(Some("1080p".into()))).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        command(&key, VideoOp::SetQuality(None)).unwrap();
        GATE.store(true, Ordering::SeqCst);
        settle();
        opens();
        let players = players();
        assert!(
            players[&key].switch.is_none(),
            "the pin's switch was dropped"
        );
        assert_eq!(players[&key].quality, None, "auto is the choice that stays");
    }

    #[test]
    fn a_reopened_stream_keeps_its_pin() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let _ = TEST_OPENER.set(counted);
        GATE.store(true, Ordering::SeqCst);
        let key = crate::scene::video::key(7_700_007, "name:stream-reopen");
        let url = "https://example.invalid/reopen.m3u8";
        configure(&key, file(url));
        settle();
        players().get_mut(&key).unwrap().quality = Some("360p".into());
        command(&key, VideoOp::Retry).unwrap();
        settle();
        opens();
        settle();
        assert!(matches!(players()[&key].state, State::Ready(_)));
        assert_eq!(players()[&key].quality.as_deref(), Some("360p"));
        let pins = PINS.lock().unwrap();
        assert_eq!(
            pins[url].last(),
            Some(&Some("360p".to_owned())),
            "the new player is pinned"
        );
    }

    #[test]
    fn auto_opens_the_file_that_fills_the_box_once_and_again_only_when_asked() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let _ = TEST_OPENER.set(counted);
        GATE.store(true, Ordering::SeqCst);
        let key = crate::scene::video::key(7_700_009, "name:auto-files");
        let (low, high) = (
            "https://example.invalid/auto-360.mp4",
            "https://example.invalid/auto-720.mp4",
        );
        configure(&key, files(&[("360p", low), ("720p", high)]));
        settle();
        opens();
        settle();
        assert_eq!(players()[&key].url, low, "no box yet: the first file");
        {
            let mut players = players();
            let entry = players.get_mut(&key).unwrap();
            entry.view = Some(([993.0, 558.0], Fit::Contain, Instant::now()));
            choose_auto(&key, entry, 558.0);
        }
        settle();
        opens();
        settle();
        assert_eq!(
            players()[&key].url,
            high,
            "a 558 px box takes the 720p file"
        );
        assert_eq!(players()[&key].quality, None, "still auto");
        // A smaller box later changes nothing until auto is asked for again.
        let mut players = players();
        let entry = players.get_mut(&key).unwrap();
        entry.view = Some(([400.0, 225.0], Fit::Contain, Instant::now()));
        choose_auto(&key, entry, 225.0);
        assert!(
            entry.switch.is_none() && entry.url == high,
            "auto chose once"
        );
        set_quality(&key, entry, None).unwrap();
        assert_eq!(
            entry.switch.as_ref().map(|s| s.url.as_str()),
            Some(low),
            "an explicit auto chooses for the box now"
        );
        // The latest choice wins: a pin replaces the switch auto started.
        set_quality(&key, entry, Some("720p".into())).unwrap();
        assert!(entry.switch.is_none(), "the pin is the file playing");
        assert_eq!(entry.quality.as_deref(), Some("720p"));
        entry.state = State::Opening;
        drop(players);
        settle();
    }

    fn opened(url: &str) -> usize {
        BY_URL.lock().unwrap().get(url).copied().unwrap_or(0)
    }

    fn file(url: &str) -> Result<Source, VideoError> {
        files(&[("", url)])
    }

    fn files(variants: &[(&str, &str)]) -> Result<Source, VideoError> {
        Ok(Source {
            variants: variants
                .iter()
                .map(|(label, url)| (label.to_string(), url.to_string()))
                .collect(),
            settings: Settings {
                paused: true,
                muted: false,
                looped: false,
                rate: 1.0,
                volume: 1.0,
            },
        })
    }

    /// Opens so far, once the opener has run what was asked (past one backoff step).
    fn opens() -> usize {
        std::thread::sleep(Duration::from_millis(600));
        COUNTED.load(Ordering::SeqCst)
    }

    /// Waits until nothing opens.
    fn settle() {
        let until = Instant::now() + Duration::from_secs(10);
        while (!flights().open.is_empty() || RUNNING.load(Ordering::SeqCst) > 0)
            && Instant::now() < until
        {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// One redraw of a kept frame: `has_new` and `import_frames` each touch its players.
    fn redraw(key: &str) {
        for _ in 0..2 {
            touch(&mut players(), key);
        }
    }

    #[test]
    fn a_page_hidden_past_its_keep_reopens_nothing_while_its_frame_is_redrawn() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let _ = TEST_OPENER.set(counted);
        let scope = 7_700_001;
        let key = crate::scene::video::key(scope, "name:held");
        let url = "https://example.invalid/held.mp4";
        configure(&key, file(url));
        settle();
        assert_eq!(opened(url), 1, "the page's player opens once");
        // Hidden past HIDDEN_IDLE and not drawn since: the opener closes its player.
        HIDDEN
            .lock()
            .unwrap()
            .insert(scope, Instant::now() - HIDDEN_IDLE - Duration::from_secs(1));
        players().get_mut(&key).unwrap().drawn = Instant::now() - IDLE - Duration::from_secs(1);
        std::thread::sleep(Duration::from_millis(1200));
        assert!(!players().contains_key(&key), "the hidden player closed");
        // Its kept frame is still redrawn at every refresh, as a stuck window leaves it.
        for _ in 0..60 {
            redraw(&key);
            std::thread::sleep(Duration::from_millis(8));
        }
        opens();
        let hidden = opened(url) - 1;
        set_hidden(scope, false);
        redraw(&key);
        opens();
        settle();
        let shown = opened(url) - 1 - hidden;
        assert_eq!(hidden, 0, "a hidden page's redraws reopen nothing");
        assert_eq!(shown, 1, "showing the page opens its player once");
    }

    #[test]
    fn asks_for_one_key_while_it_opens_fold_into_at_most_one_more_open() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let _ = TEST_OPENER.set(counted);
        GATE.store(false, Ordering::SeqCst);
        let key = crate::scene::video::key(7_700_002, "name:flight");
        let url = "https://example.invalid/slow-flight.mp4";
        configure(&key, file(url));
        std::thread::sleep(Duration::from_millis(100));
        for _ in 0..50 {
            command(&key, VideoOp::Retry).unwrap();
            redraw(&key);
        }
        assert_eq!(opened(url), 1, "one open runs while it is in flight");
        GATE.store(true, Ordering::SeqCst);
        settle();
        opens();
        settle();
        assert!(opened(url) <= 2, "{} opens", opened(url));
        assert!(matches!(players()[&key].state, State::Ready(_)));
    }

    #[test]
    fn no_more_players_open_at_once_than_the_cap() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let _ = TEST_OPENER.set(counted);
        GATE.store(false, Ordering::SeqCst);
        PEAK.store(0, Ordering::SeqCst);
        let keys: Vec<String> = (0..12)
            .map(|n| crate::scene::video::key(7_700_100 + n, "name:many"))
            .collect();
        for (n, key) in keys.iter().enumerate() {
            configure(
                key,
                file(&format!("https://example.invalid/slow-many-{n}.mp4")),
            );
        }
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(RUNNING.load(Ordering::SeqCst), OPENS_AT_ONCE);
        GATE.store(true, Ordering::SeqCst);
        settle();
        assert!(PEAK.load(Ordering::SeqCst) <= OPENS_AT_ONCE);
        for (n, key) in keys.iter().enumerate() {
            assert_eq!(
                opened(&format!("https://example.invalid/slow-many-{n}.mp4")),
                1
            );
            assert!(
                matches!(players()[key].state, State::Ready(_)),
                "{key} opened"
            );
        }
    }

    #[test]
    fn a_quality_chosen_while_the_first_open_runs_is_the_one_that_plays() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let _ = TEST_OPENER.set(counted);
        GATE.store(false, Ordering::SeqCst);
        let key = crate::scene::video::key(7_700_003, "name:choice");
        let (high, low) = (
            "https://example.invalid/slow-choice-720.mp4",
            "https://example.invalid/choice-360.mp4",
        );
        configure(&key, files(&[("720p", high), ("360p", low)]));
        std::thread::sleep(Duration::from_millis(100));
        command(&key, VideoOp::SetQuality(Some("360p".into()))).unwrap();
        GATE.store(true, Ordering::SeqCst);
        settle();
        opens();
        settle();
        assert_eq!(
            opened(low),
            1,
            "the chosen quality opens after the one in flight"
        );
        let players = players();
        assert!(matches!(players[&key].state, State::Ready(_)));
        assert_eq!(players[&key].url, low);
    }

    #[test]
    fn a_closed_pages_players_close_at_once_and_a_late_draw_reopens_none() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let _ = TEST_OPENER.set(counted);
        let scope = 7_700_005;
        let key = crate::scene::video::key(scope, "name:closing");
        let url = "https://example.invalid/closing.mp4";
        configure(&key, file(url));
        settle();
        assert!(matches!(players()[&key].state, State::Ready(_)));
        // The page closes while its frame is still drawn every refresh.
        forget(scope);
        assert!(!players().contains_key(&key), "forget removes the player");
        let until = Instant::now() + Duration::from_millis(500);
        while CLOSED.lock().unwrap().get(url).is_none() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            CLOSED.lock().unwrap().get(url),
            Some(&1),
            "closed within one opener tick"
        );
        for _ in 0..10 {
            redraw(&key);
        }
        opens();
        settle();
        assert_eq!(
            opened(url),
            1,
            "a late draw of a closed page's key opens nothing"
        );
        assert!(!players().contains_key(&key));
    }

    #[test]
    fn reopens_without_a_frame_back_off_and_a_frame_resets_them() {
        assert_eq!(backoff(0), Duration::ZERO);
        assert_eq!(backoff(1), BACKOFF);
        assert_eq!(backoff(2), BACKOFF * 2);
        assert_eq!(backoff(9), BACKOFF_MAX);
        let key = "7700004/name:backoff";
        flights().tries.insert(key.into(), (5, Instant::now()));
        reset_backoff(key);
        assert!(!flights().tries.contains_key(key));
    }
}

#[cfg(test)]
mod view_tests {
    use crate::ViewSession;

    #[test]
    fn auto_reads_the_box_in_device_pixels_under_its_transform() {
        let module = crate::session::tests::source(
            "function render(){const a=useVideo('a'),b=useVideo('b');return h('view',{},h('video',{player:a,src:'a.mp4',style:{width:160,height:90}}),h('view',{style:{scale:1.5}},h('video',{player:b,src:'b.mp4',style:{width:160,height:90}})))}",
        );
        let mut page = ViewSession::new(&module).unwrap();
        let display = page.layout(400, 600, 2.0).unwrap();
        let views = super::views(&display);
        let px = |key: &str| {
            views
                .iter()
                .find(|(player, _)| player.ends_with(key))
                .map(|(_, (px, _))| px.map(f32::round))
                .unwrap()
        };
        assert_eq!(px(":a"), [320.0, 180.0]);
        assert_eq!(px(":b"), [480.0, 270.0], "a scaled box shows more pixels");
    }
}
