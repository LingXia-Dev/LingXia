//! The renderer's video players. A `Paint::Video` key opens one platform player off the render
//! thread; the render thread imports each new frame as an external texture, with no CPU copy.
//! Players nothing drew for 2 s stop and close on the opener thread.
use crate::scene::paint::Paint;
use crate::scene::video::{Settings, Source, Status, VideoError, VideoOp};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard, mpsc};
use std::time::{Duration, Instant};

/// A player not drawn for this long stops decoding and closes.
const IDLE: Duration = Duration::from_secs(2);

/// Bumped when a frame size changes: cached scenes encode the video's source region.
static EPOCH: AtomicU64 = AtomicU64::new(0);

pub(crate) fn epoch() -> u64 {
    EPOCH.load(Ordering::Relaxed)
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
    /// The frame due now, when it is new.
    fn next_frame(
        &mut self,
        device: &wgpu::Device,
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
    /// Pins the rendition `label`, or gives the choice back to the player with `None`.
    fn set_quality(&mut self, label: Option<&str>) -> Result<(), VideoError> {
        let _ = label;
        Err(VideoError::new(
            "VIDEO_SOURCE",
            "this video has one quality; give `sources` to choose between files",
        ))
    }
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

fn open(source: &str) -> Result<Box<dyn Player>, String> {
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
            let _closed = std::mem::replace(&mut entry.state, State::Opening);
            request(key, &mut entry.state);
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

fn request(key: &str, state: &mut State) {
    let sent = OPENER
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .send(Job::Open(key.to_owned()));
    if sent.is_err() {
        *state = State::Failed(VideoError::new(
            "VIDEO_DECODERS",
            "the opener is not running",
        ));
    }
}

fn settings_of(key: &str) -> Option<Settings> {
    sources()
        .get(key)?
        .as_ref()
        .ok()
        .map(|s| s.settings.clone())
}

/// The state of player `key`, while a page draws it.
pub(crate) fn status(key: &str) -> Option<Status> {
    let players = players();
    let entry = players.get(key)?;
    let mut status = match &entry.state {
        State::Ready(player) => player.status(),
        State::Opening => Status::default(),
        State::Failed(error) => Status {
            error: Some(error.clone()),
            ..Status::default()
        },
    };
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
    match (&mut entry.state, op) {
        (_, VideoOp::Retry) => {
            entry.applied = None;
            entry.switch = None;
            let _closed = std::mem::replace(&mut entry.state, State::Opening);
            request(key, &mut entry.state);
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
        if label.is_none() || (now.paused && now.current_time == 0.0) {
            player.set_quality(label.as_deref())?;
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
    let url = source.url(label.as_deref()).to_owned();
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
            request(key, &mut entry.state);
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
        .send(Job::Switch(key.to_owned(), url));
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
    switch: Option<Switch>,
    /// When the last frame was imported.
    imported: Option<Instant>,
}

static PLAYERS: LazyLock<Mutex<HashMap<String, Entry>>> = LazyLock::new(Default::default);

fn players() -> MutexGuard<'static, HashMap<String, Entry>> {
    PLAYERS.lock().unwrap_or_else(|p| p.into_inner())
}

/// Redraws without the worker: count, skipped for want of a new frame, then total and max µs of
/// render, present, scene, images, encode and submit.
static REBINDS: Mutex<[u64; 14]> = Mutex::new([0; 14]);

/// Opens players and closes idle ones, so neither stalls a render.
static OPENER: LazyLock<Mutex<mpsc::Sender<Job>>> = LazyLock::new(|| {
    let (sender, receiver) = mpsc::channel::<Job>();
    let spawned = std::thread::Builder::new()
        .name("lingxia-video".into())
        .spawn(move || {
            loop {
                // Each open runs on its own thread: a stream's manifest fetch must not hold up a
                // file behind it.
                let job = match receiver.recv_timeout(Duration::from_millis(500)) {
                    Ok(job) => Some(job),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                };
                if let Some(job) = job {
                    let spawned = std::thread::Builder::new()
                        .name("lingxia-video-open".into())
                        .spawn(move || match job {
                            Job::Open(key) => open_player(&key),
                            Job::Switch(key, url) => open_switch(&key, &url),
                        });
                    if let Err(error) = spawned {
                        log::warn!(target: "lingxia_view", "video open did not start: {error}");
                    }
                }
                let idle: Vec<Entry> = {
                    let mut players = players();
                    let keys: Vec<String> = players
                        .iter()
                        .filter(|(_, entry)| entry.drawn.elapsed() > IDLE)
                        .map(|(key, _)| key.clone())
                        .collect();
                    keys.iter().filter_map(|key| players.remove(key)).collect()
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

fn open_player(key: &str) {
    let picked = Instant::now();
    let source = sources().get(key).cloned();
    let quality = players().get(key).and_then(|entry| entry.quality.clone());
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
        let quality = players().get(key).and_then(|entry| entry.quality.clone());
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
        lead(url, now.current_time, now.playback_rate, now.duration)
    };
    let _ = next.command(&VideoOp::Seek(target));
    match &mut entry.switch {
        Some(switch) if switch.url == url && switch.label == label => {
            switch.player = Some(next);
            switch.target = target;
        }
        // Superseded or cancelled while opening.
        _ => {}
    }
}

/// The position a lead ahead of `time`, kept short of the end.
fn lead(url: &str, time: f64, rate: f32, duration: Option<f64>) -> f64 {
    let seconds = f64::from(switch_lead(url).load(Ordering::Relaxed)) / 1000.0;
    let ahead = time + seconds * f64::from(rate.max(0.25));
    duration.map_or(ahead, |duration| ahead.min((duration - 0.05).max(0.0)))
}

/// Moves a switch on at a refresh: starts the next player just before the playing one reaches
/// its position, then binds its frame once the playing one has, so the picture never stalls.
fn advance_switch(key: &str, entry: &mut Entry, device: &wgpu::Device) {
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
    if now.paused {
        if (now.current_time - switch.target).abs() > 0.02 {
            switch.target = now.current_time;
            let _ = next.command(&VideoOp::Seek(switch.target));
            return;
        }
    } else if !switch.started {
        if now.current_time > switch.target + 0.05 {
            // The playing one passed the parked position before it was ready: park further on.
            tune_lead(&switch.url, 2.0);
            switch.target = lead(
                &switch.url,
                now.current_time,
                now.playback_rate,
                now.duration,
            );
            let _ = next.command(&VideoOp::Seek(switch.target));
            return;
        }
        if now.current_time < switch.target - SWITCH_START * f64::from(now.playback_rate) {
            return;
        }
        // Ready in time (its parked frame is decoded): the next switch parks closer.
        let parked = (next.status().current_time - switch.target).abs() < 0.05 && next.has_new();
        tune_lead(&switch.url, if parked { 0.8 } else { 1.5 });
        let _ = next.command(&VideoOp::Play);
        switch.started = true;
        return;
    } else if now.current_time < switch.target {
        return;
    }
    let Ok(Some((texture, size))) = next.next_frame(device) else {
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
    players()
        .get(player)?
        .frame
        .as_ref()
        .map(|frame| frame.size)
}

/// A view of the newest frame of `player`, once one is bound.
pub(crate) fn frame_view(player: &str) -> Option<wgpu::TextureView> {
    players()
        .get(player)?
        .frame
        .as_ref()
        .map(|frame| frame.view.clone())
}

/// Marks the players of `display` drawn, asking for those not open yet; true when one of them
/// has a frame newer than the one bound.
pub(crate) fn has_new(display: &crate::DisplayList) -> bool {
    let mut players = players();
    let mut new = false;
    for key in keys(display) {
        if let Some(entry) = touch(&mut players, key) {
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
        let now = Instant::now();
        let mut state = State::Opening;
        request(key, &mut state);
        players.insert(
            key.to_owned(),
            Entry {
                state,
                applied: None,
                pending: Vec::new(),
                url: String::new(),
                quality: None,
                switch: None,
                imported: None,
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
                ready_at: None,
                loaded_at: None,
                shown_at: None,
                after_shown_ms: None,
                new_at: None,
                frames: 0,
                import_us: [0; 2],
            },
        );
        return None;
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
pub(crate) fn import_frames(device: &wgpu::Device, display: &crate::DisplayList) -> (bool, bool) {
    let mut imported = false;
    let keys = keys(display);
    let mut players = players();
    for key in &keys {
        let Some(entry) = touch(&mut players, key) else {
            continue;
        };
        let frames = entry.frames;
        advance_switch(key, entry, device);
        if entry.frames != frames {
            imported = true;
            continue;
        }
        let State::Ready(player) = &mut entry.state else {
            continue;
        };
        let started = Instant::now();
        match player.next_frame(device) {
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
                entry.frame = Some(Frame {
                    view: texture.create_view(&Default::default()),
                    size,
                });
                let us = crate::session::metrics::micros(started);
                entry.imported = Some(Instant::now());
                entry.frames += 1;
                imported = true;
                entry.import_us = [entry.import_us[0] + us, entry.import_us[1].max(us)];
            }
            Ok(None) => {}
            Err(error) => log::warn!(target: "lingxia_view", "video {key}: {error}"),
        }
    }
    (!keys.is_empty(), imported)
}

/// Binds each player's newest frame under its texture id.
pub(crate) fn bind(display: &crate::DisplayList, bindings: &mut vello_gpu::TextureBindings) {
    let players = players();
    for key in keys(display) {
        if let Some(frame) = players.get(key).and_then(|entry| entry.frame.as_ref()) {
            bindings.insert(texture_id(key), frame.view.clone());
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
    })
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
        Ok(Box::new(Player::open(url)?))
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
        ) -> Result<Option<(wgpu::Texture, [u16; 2])>, String> {
            self.frame(device)
        }

        fn apply(&mut self, settings: &Settings, previous: Option<&Settings>) {
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

        fn command(&mut self, op: &VideoOp) -> Result<(), VideoError> {
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

        fn qualities(&self) -> Vec<String> {
            self.renditions.iter().map(|r| r.label.clone()).collect()
        }

        fn set_quality(&mut self, label: Option<&str>) -> Result<(), VideoError> {
            let rendition = match label {
                None => None,
                Some(label) => Some(
                    self.renditions
                        .iter()
                        .find(|r| r.label == label)
                        .ok_or_else(|| {
                            VideoError::new("VIDEO_SOURCE", format!("no rendition {label}"))
                        })?,
                ),
            };
            let (size, peak) = rendition.map_or(([0.0, 0.0], 0.0), |r| (r.size, r.peak));
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
            Ok(())
        }

        fn status(&self) -> Status {
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
                let renditions = if url
                    .to_bytes()
                    .split(|b| *b == b'?')
                    .next()
                    .is_some_and(|path| path.ends_with(b".m3u8"))
                {
                    renditions(item)
                } else {
                    Vec::new()
                };
                Ok(Self {
                    player,
                    item,
                    output,
                    cache: std::ptr::null_mut(),
                    renditions,
                    looped: std::cell::Cell::new(false),
                    ended: std::cell::Cell::new(false),
                    rate: 1.0,
                    first: std::cell::Cell::new(first),
                })
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
            unsafe {
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
            }
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
