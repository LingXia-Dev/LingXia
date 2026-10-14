//! Host-wide audio coordinator: focus, session, routes, interruptions, and
//! every player's state machine. Runs on one worker thread; every input is a
//! JS command or a native event, every output is a backend command or an
//! event to an owner's Logic context.

use super::protocol::{Command, NativeEvent};
use super::session::{self, Requirement, SessionSpec};
use super::types::{
    AudioError, AudioErrorCode, Focus, Interruption, InterruptionPhase, InterruptionReason,
    OutputRequest, PlayerStatus, Repeat, Route, RouteChange, RouteReason,
};
use lingxia_platform::traits::audio::AudioDeviceCapabilities;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;
use tokio::sync::{mpsc::UnboundedSender, oneshot};

pub type OwnerId = u64;

/// `previous()` restarts the current item past this point.
const RESTART_THRESHOLD_MS: u64 = 3_000;

#[derive(Debug, Clone)]
pub struct PlayerOptions {
    pub focus: Focus,
    pub obey_silent_switch: bool,
    pub output: OutputRequest,
    pub proximity: bool,
    pub resume_after_interruption: bool,
    pub volume: f64,
    pub rate: f64,
    pub repeat: Repeat,
    pub time_update_interval_ms: u32,
}

impl Default for PlayerOptions {
    fn default() -> Self {
        Self {
            focus: Focus::Exclusive,
            obey_silent_switch: false,
            output: OutputRequest::Auto,
            proximity: false,
            resume_after_interruption: false,
            volume: 1.0,
            rate: 1.0,
            repeat: Repeat::Off,
            time_update_interval_ms: 250,
        }
    }
}

/// A queue item after source resolution: an absolute path or http(s) URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueEntry {
    pub uri: String,
    pub headers: BTreeMap<String, String>,
    pub mime_type: Option<String>,
    pub start_ms: u64,
}

#[derive(Debug, Clone)]
pub enum PlayerCommand {
    SetQueue {
        items: Vec<QueueEntry>,
        start_index: usize,
        autoplay: bool,
    },
    Play,
    Pause,
    Stop,
    Seek(u64),
    Next,
    Previous,
    SkipTo(usize),
    /// Volume, mute, rate, and repeat are written to the shared snapshot by
    /// the caller first, so the synchronous setter is visible at once; this
    /// applies them natively and emits `state`.
    ApplySettings,
    SetOutput(OutputRequest),
    Dispose,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    Done,
    Loaded {
        duration_ms: Option<u64>,
        index: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemReason {
    Load,
    Auto,
    Next,
    Previous,
    Skip,
}

impl ItemReason {
    pub fn as_str(self) -> &'static str {
        match self {
            ItemReason::Load => "load",
            ItemReason::Auto => "auto",
            ItemReason::Next => "next",
            ItemReason::Previous => "previous",
            ItemReason::Skip => "skip",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlayerSnapshot {
    pub status: PlayerStatus,
    pub buffering: bool,
    pub seeking: bool,
    pub position_ms: u64,
    pub position_at: Instant,
    pub duration_ms: Option<u64>,
    pub buffered_ms: u64,
    pub index: Option<usize>,
    pub volume: f64,
    pub muted: bool,
    pub rate: f64,
    pub repeat: Repeat,
    pub output: OutputRequest,
    pub route: Route,
    pub error: Option<AudioError>,
}

impl PlayerSnapshot {
    pub fn new(options: &PlayerOptions, route: Route) -> Self {
        Self {
            status: PlayerStatus::Idle,
            buffering: false,
            seeking: false,
            position_ms: 0,
            position_at: Instant::now(),
            duration_ms: None,
            buffered_ms: 0,
            index: None,
            volume: options.volume,
            muted: false,
            rate: options.rate,
            repeat: options.repeat,
            output: options.output,
            route,
            error: None,
        }
    }

    /// Position extrapolated from the last native report while audio renders.
    pub fn position_now(&self) -> u64 {
        if self.status != PlayerStatus::Playing || self.buffering || self.seeking {
            return self.position_ms;
        }
        let elapsed = self.position_at.elapsed().as_millis() as f64 * self.rate;
        let position = self.position_ms + elapsed as u64;
        match self.duration_ms {
            Some(duration) => position.min(duration),
            None => position,
        }
    }

    fn effective_volume(&self) -> f64 {
        if self.muted { 0.0 } else { self.volume }
    }
}

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    State(PlayerSnapshot),
    Time {
        position_ms: u64,
        duration_ms: Option<u64>,
    },
    Item {
        index: usize,
        reason: ItemReason,
    },
    /// An item played to its end; fires before an automatic advance.
    Ended {
        index: usize,
    },
    Error {
        error: AudioError,
        index: Option<usize>,
    },
    Interruption(Interruption),
    Route(RouteChange),
    /// Completes the JS command `req`, after the events it caused.
    Reply {
        req: u64,
        result: Result<Reply, AudioError>,
    },
}

#[derive(Debug, Clone)]
pub enum HostEvent {
    Route(RouteChange),
    Interruption(Interruption),
}

#[derive(Debug, Clone, Copy)]
pub struct SoundOptions {
    pub focus: Focus,
    pub obey_silent_switch: bool,
    pub max_voices: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceParams {
    pub volume: f64,
    pub rate: f64,
    pub pan: f64,
    pub looping: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundEvent {
    VoiceEnded(u64),
}

pub type SoundLoadReply = oneshot::Sender<Result<u64, AudioError>>;

pub enum Input {
    RegisterOwner {
        owner: OwnerId,
        tx: UnboundedSender<HostEvent>,
    },
    OwnerVisible {
        owner: OwnerId,
        visible: bool,
    },
    OwnerShutdown {
        owner: OwnerId,
    },
    CreatePlayer {
        id: u64,
        owner: OwnerId,
        options: PlayerOptions,
        shared: Arc<Mutex<PlayerSnapshot>>,
        tx: UnboundedSender<PlayerEvent>,
    },
    Player {
        id: u64,
        req: u64,
        command: PlayerCommand,
    },
    LoadSound {
        id: u64,
        owner: OwnerId,
        options: SoundOptions,
        path: String,
        tx: UnboundedSender<SoundEvent>,
        reply: SoundLoadReply,
    },
    PlayVoice {
        id: u64,
        voice: u64,
        params: VoiceParams,
    },
    StopVoice {
        id: u64,
        voice: u64,
    },
    StopAllVoices {
        id: u64,
    },
    DisposeSound {
        id: u64,
    },
    Native(NativeEvent),
}

pub type CommandSink = Box<dyn FnMut(Command) + Send>;

struct Player {
    owner: OwnerId,
    options: PlayerOptions,
    shared: Arc<Mutex<PlayerSnapshot>>,
    tx: UnboundedSender<PlayerEvent>,
    queue: Vec<QueueEntry>,
    /// Native request of the load in flight.
    load_req: Option<u64>,
    /// JS command waiting for that load.
    load_reply: Option<u64>,
    autoplay: bool,
    play_replies: Vec<u64>,
    seek: Option<(u64, u64)>,
    /// Holds (or is about to hold) focus: playing, or play issued natively.
    live: bool,
    started: u64,
    paused_by: Option<InterruptionReason>,
}

impl Player {
    fn snap(&self) -> MutexGuard<'_, PlayerSnapshot> {
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn emit(&self, event: PlayerEvent) {
        let _ = self.tx.send(event);
    }

    fn emit_state(&self) {
        let snapshot = self.snap().clone();
        self.emit(PlayerEvent::State(snapshot));
    }

    fn reply(&self, req: u64, result: Result<Reply, AudioError>) {
        self.emit(PlayerEvent::Reply { req, result });
    }

    fn requirement(&self) -> Requirement {
        Requirement {
            focus: self.options.focus,
            obey_silent_switch: self.options.obey_silent_switch,
            output: self.options.output,
            proximity: self.options.proximity,
            started: self.started,
        }
    }

    /// Fail every waiting command; used when a later command or failure
    /// supersedes them.
    fn reject_pending(&mut self, error: &AudioError) {
        if let Some(req) = self.load_reply.take() {
            self.reply(req, Err(error.clone()));
        }
        for req in std::mem::take(&mut self.play_replies) {
            self.reply(req, Err(error.clone()));
        }
        if let Some((_, req)) = self.seek.take() {
            self.reply(req, Err(error.clone()));
        }
    }
}

struct Sound {
    owner: OwnerId,
    options: SoundOptions,
    tx: UnboundedSender<SoundEvent>,
    load: Option<(u64, SoundLoadReply)>,
    loaded: bool,
    voices: VecDeque<u64>,
    started: u64,
}

struct Video {
    live: bool,
    started: u64,
}

pub struct Coordinator {
    sink: CommandSink,
    capabilities: AudioDeviceCapabilities,
    route: Route,
    route_shared: Arc<Mutex<Route>>,
    proximity_near: bool,
    session: SessionSpec,
    owners: HashMap<OwnerId, UnboundedSender<HostEvent>>,
    players: HashMap<u64, Player>,
    sounds: HashMap<u64, Sound>,
    videos: HashMap<String, Video>,
    next_req: u64,
    next_start: u64,
}

impl Coordinator {
    pub fn new(
        sink: CommandSink,
        capabilities: AudioDeviceCapabilities,
        route_shared: Arc<Mutex<Route>>,
    ) -> Self {
        let route = route_shared.lock().map(|r| r.clone()).unwrap_or_default();
        Self {
            sink,
            capabilities,
            route,
            route_shared,
            proximity_near: false,
            session: SessionSpec::INACTIVE,
            owners: HashMap::new(),
            players: HashMap::new(),
            sounds: HashMap::new(),
            videos: HashMap::new(),
            next_req: 1,
            next_start: 1,
        }
    }

    fn send(&mut self, command: Command) {
        (self.sink)(command);
    }

    fn req(&mut self) -> u64 {
        let req = self.next_req;
        self.next_req += 1;
        req
    }

    fn start_seq(&mut self) -> u64 {
        let seq = self.next_start;
        self.next_start += 1;
        seq
    }

    pub fn handle(&mut self, input: Input) {
        match input {
            Input::RegisterOwner { owner, tx } => {
                self.owners.insert(owner, tx);
            }
            Input::OwnerVisible { owner, visible } => self.owner_visible(owner, visible),
            Input::OwnerShutdown { owner } => self.owner_shutdown(owner),
            Input::CreatePlayer {
                id,
                owner,
                options,
                shared,
                tx,
            } => self.create_player(id, owner, options, shared, tx),
            Input::Player { id, req, command } => self.player_command(id, req, command),
            Input::LoadSound {
                id,
                owner,
                options,
                path,
                tx,
                reply,
            } => self.load_sound(id, owner, options, path, tx, reply),
            Input::PlayVoice { id, voice, params } => self.play_voice(id, voice, params),
            Input::StopVoice { id, voice } => self.stop_voice(id, voice, true),
            Input::StopAllVoices { id } => self.stop_all_voices(id),
            Input::DisposeSound { id } => self.dispose_sound(id),
            Input::Native(event) => self.native(event),
        }
    }

    // ----- session -----

    fn apply_session(&mut self) {
        let mut live: Vec<Requirement> = self
            .players
            .values()
            .filter(|p| p.live)
            .map(Player::requirement)
            .collect();
        live.extend(
            self.sounds
                .values()
                .filter(|s| !s.voices.is_empty())
                .map(|s| Requirement {
                    focus: s.options.focus,
                    obey_silent_switch: s.options.obey_silent_switch,
                    output: OutputRequest::Auto,
                    proximity: false,
                    started: s.started,
                }),
        );
        live.extend(
            self.videos
                .values()
                .filter(|v| v.live)
                .map(|v| Requirement {
                    focus: Focus::Exclusive,
                    obey_silent_switch: false,
                    output: OutputRequest::Auto,
                    proximity: false,
                    started: v.started,
                }),
        );
        let spec = session::compute(&live, self.route.output, self.proximity_near);
        if spec != self.session {
            self.session = spec;
            self.send(spec.command());
        }
    }

    /// Rule 2: one `exclusive` participant plays at a time across the host.
    fn take_exclusive_focus(&mut self, except_player: Option<u64>, except_video: Option<&str>) {
        let losers: Vec<u64> = self
            .players
            .iter()
            .filter(|(id, p)| {
                Some(**id) != except_player && p.live && p.options.focus == Focus::Exclusive
            })
            .map(|(id, _)| *id)
            .collect();
        for id in losers {
            self.interrupt(id, InterruptionReason::FocusTaken);
        }
        let videos: Vec<String> = self
            .videos
            .iter()
            .filter(|(cid, v)| v.live && Some(cid.as_str()) != except_video)
            .map(|(cid, _)| cid.clone())
            .collect();
        for component_id in videos {
            if let Some(video) = self.videos.get_mut(&component_id) {
                video.live = false;
            }
            self.send(Command::VideoPause { component_id });
        }
    }

    // ----- owners -----

    fn owner_visible(&mut self, owner: OwnerId, visible: bool) {
        let ids: Vec<u64> = self
            .players
            .iter()
            .filter(|(_, p)| p.owner == owner)
            .map(|(id, _)| *id)
            .collect();
        if visible {
            for id in ids {
                let Some(player) = self.players.get_mut(&id) else {
                    continue;
                };
                if player.paused_by == Some(InterruptionReason::AppHidden) {
                    player.paused_by = None;
                    player.emit(PlayerEvent::Interruption(Interruption {
                        phase: InterruptionPhase::Ended,
                        reason: InterruptionReason::AppHidden,
                        should_resume: true,
                        resumed: false,
                    }));
                }
            }
            return;
        }
        for id in ids {
            self.interrupt(id, InterruptionReason::AppHidden);
        }
        let sounds: Vec<u64> = self
            .sounds
            .iter()
            .filter(|(_, s)| s.owner == owner)
            .map(|(id, _)| *id)
            .collect();
        for id in sounds {
            self.stop_all_voices(id);
        }
        self.apply_session();
    }

    fn owner_shutdown(&mut self, owner: OwnerId) {
        self.owners.remove(&owner);
        let players: Vec<u64> = self
            .players
            .iter()
            .filter(|(_, p)| p.owner == owner)
            .map(|(id, _)| *id)
            .collect();
        for id in players {
            self.dispose_player(id, None);
        }
        let sounds: Vec<u64> = self
            .sounds
            .iter()
            .filter(|(_, s)| s.owner == owner)
            .map(|(id, _)| *id)
            .collect();
        for id in sounds {
            self.dispose_sound(id);
        }
    }

    fn broadcast(&self, event: HostEvent) {
        for tx in self.owners.values() {
            let _ = tx.send(event.clone());
        }
    }

    // ----- players -----

    fn create_player(
        &mut self,
        id: u64,
        owner: OwnerId,
        options: PlayerOptions,
        shared: Arc<Mutex<PlayerSnapshot>>,
        tx: UnboundedSender<PlayerEvent>,
    ) {
        let tick_ms = match options.time_update_interval_ms {
            0 => 1_000,
            interval => interval.max(50),
        };
        {
            let mut snap = shared.lock().unwrap_or_else(|e| e.into_inner());
            snap.route = self.route.clone();
        }
        let player = Player {
            owner,
            options,
            shared,
            tx,
            queue: Vec::new(),
            load_req: None,
            load_reply: None,
            autoplay: false,
            play_replies: Vec::new(),
            seek: None,
            live: false,
            started: 0,
            paused_by: None,
        };
        let (volume, rate) = {
            let snap = player.snap();
            (snap.effective_volume(), snap.rate)
        };
        self.players.insert(id, player);
        self.send(Command::PlayerCreate { id, tick_ms });
        self.send(Command::PlayerVolume { id, volume });
        self.send(Command::PlayerRate { id, rate });
    }

    fn player_command(&mut self, id: u64, req: u64, command: PlayerCommand) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if player.snap().status == PlayerStatus::Disposed {
            player.reply(req, Err(AudioError::disposed()));
            return;
        }
        match command {
            PlayerCommand::SetQueue {
                items,
                start_index,
                autoplay,
            } => self.set_queue(id, req, items, start_index, autoplay),
            PlayerCommand::Play => self.play(id, req),
            PlayerCommand::Pause => self.pause(id, req),
            PlayerCommand::Stop => self.stop(id, req),
            PlayerCommand::Seek(position) => self.seek(id, Some(req), position),
            PlayerCommand::Next => self.step(id, req, ItemReason::Next),
            PlayerCommand::Previous => self.step(id, req, ItemReason::Previous),
            PlayerCommand::SkipTo(index) => self.skip_to(id, req, index),
            PlayerCommand::ApplySettings => self.apply_settings(id, req),
            PlayerCommand::SetOutput(output) => self.set_output(id, req, output),
            PlayerCommand::Dispose => self.dispose_player(id, Some(req)),
        }
    }

    fn set_queue(
        &mut self,
        id: u64,
        req: u64,
        items: Vec<QueueEntry>,
        start_index: usize,
        autoplay: bool,
    ) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        if items.is_empty() || start_index >= items.len() {
            player.reply(
                req,
                Err(AudioError::new(
                    AudioErrorCode::InvalidOptions,
                    "startIndex is outside the queue",
                )),
            );
            return;
        }
        player.reject_pending(&AudioError::new(
            AudioErrorCode::Interrupted,
            "superseded by a new load",
        ));
        player.queue = items;
        let was_live = player.live;
        if was_live {
            player.live = false;
            self.send(Command::PlayerPause { id });
        }
        self.begin_load(
            id,
            start_index,
            autoplay,
            Some(req),
            Some(ItemReason::Load),
            None,
        );
        if was_live {
            self.apply_session();
        }
    }

    /// Replace the native item. `start_override` restarts from 0 instead of
    /// the item's `startMs`.
    fn begin_load(
        &mut self,
        id: u64,
        index: usize,
        autoplay: bool,
        reply: Option<u64>,
        reason: Option<ItemReason>,
        start_override: Option<u64>,
    ) {
        let req = self.req();
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let Some(entry) = player.queue.get(index).cloned() else {
            return;
        };
        let start_ms = start_override.unwrap_or(entry.start_ms);
        if let Some(previous) = player.load_reply.take() {
            player.reply(
                previous,
                Err(AudioError::new(
                    AudioErrorCode::Interrupted,
                    "superseded by a new load",
                )),
            );
        }
        if let Some((_, seek_req)) = player.seek.take() {
            player.reply(
                seek_req,
                Err(AudioError::new(
                    AudioErrorCode::Interrupted,
                    "superseded by a new load",
                )),
            );
        }
        player.load_req = Some(req);
        player.load_reply = reply;
        player.autoplay = autoplay;
        player.paused_by = None;
        {
            let mut snap = player.snap();
            snap.status = PlayerStatus::Loading;
            snap.index = Some(index);
            snap.error = None;
            snap.position_ms = start_ms;
            snap.position_at = Instant::now();
            snap.duration_ms = None;
            snap.buffered_ms = 0;
            snap.buffering = false;
            snap.seeking = false;
        }
        let command = Command::PlayerLoad {
            id,
            req,
            uri: entry.uri,
            headers: entry.headers,
            mime_type: entry.mime_type,
            start_ms,
        };
        player.emit_state();
        if let Some(reason) = reason {
            player.emit(PlayerEvent::Item { index, reason });
        }
        self.send(command);
    }

    fn play(&mut self, id: u64, req: u64) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let status = player.snap().status;
        match status {
            PlayerStatus::Idle => player.reply(
                req,
                Err(AudioError::new(
                    AudioErrorCode::InvalidSource,
                    "no source loaded; call load() or setQueue() first",
                )),
            ),
            PlayerStatus::Disposed => player.reply(req, Err(AudioError::disposed())),
            PlayerStatus::Playing => player.reply(req, Ok(Reply::Done)),
            PlayerStatus::Loading => {
                player.autoplay = true;
                player.play_replies.push(req);
            }
            PlayerStatus::Paused => {
                player.play_replies.push(req);
                if !player.live {
                    self.start_play(id);
                }
            }
            PlayerStatus::Failed | PlayerStatus::Stopped => {
                let index = player.snap().index.unwrap_or(0);
                player.play_replies.push(req);
                let restart = (status == PlayerStatus::Stopped).then_some(0);
                self.begin_load(id, index, true, None, None, restart);
            }
            PlayerStatus::Ended => {
                let previous = player.snap().index.unwrap_or(0);
                player.play_replies.push(req);
                let reason = (previous != 0).then_some(ItemReason::Load);
                self.begin_load(id, 0, true, None, reason, Some(0));
            }
        }
    }

    fn start_play(&mut self, id: u64) {
        let earpiece_unsupported = !self.capabilities.earpiece;
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        if player.options.output == OutputRequest::Earpiece && earpiece_unsupported {
            let error = AudioError::new(
                AudioErrorCode::OutputUnsupported,
                "this device has no earpiece",
            );
            let replies = std::mem::take(&mut player.play_replies);
            if replies.is_empty() {
                let index = player.snap().index;
                player.emit(PlayerEvent::Error { error, index });
            } else {
                for req in replies {
                    player.reply(req, Err(error.clone()));
                }
            }
            return;
        }
        let exclusive = player.options.focus == Focus::Exclusive;
        if exclusive {
            self.take_exclusive_focus(Some(id), None);
        }
        let started = self.start_seq();
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        player.live = true;
        player.started = started;
        player.paused_by = None;
        self.apply_session();
        self.send(Command::PlayerPlay { id });
    }

    fn pause(&mut self, id: u64, req: u64) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let interrupted = AudioError::new(
            AudioErrorCode::Interrupted,
            "paused before playback started",
        );
        for pending in std::mem::take(&mut player.play_replies) {
            player.reply(pending, Err(interrupted.clone()));
        }
        player.autoplay = false;
        let was_live = player.live;
        let changed = {
            let mut snap = player.snap();
            if snap.status == PlayerStatus::Playing {
                snap.position_ms = snap.position_now();
                snap.position_at = Instant::now();
                snap.status = PlayerStatus::Paused;
                snap.buffering = false;
                true
            } else {
                false
            }
        };
        player.live = false;
        if was_live {
            self.send(Command::PlayerPause { id });
        }
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if changed {
            player.emit_state();
        }
        player.reply(req, Ok(Reply::Done));
        if was_live {
            self.apply_session();
        }
    }

    fn stop(&mut self, id: u64, req: u64) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        if player.snap().status == PlayerStatus::Idle {
            player.reply(req, Ok(Reply::Done));
            return;
        }
        player.reject_pending(&AudioError::new(AudioErrorCode::Interrupted, "stopped"));
        player.load_req = None;
        player.autoplay = false;
        player.paused_by = None;
        let was_live = std::mem::replace(&mut player.live, false);
        {
            let mut snap = player.snap();
            snap.status = PlayerStatus::Stopped;
            snap.position_ms = 0;
            snap.position_at = Instant::now();
            snap.buffering = false;
            snap.seeking = false;
            snap.buffered_ms = 0;
        }
        player.emit_state();
        player.reply(req, Ok(Reply::Done));
        self.send(Command::PlayerUnload { id });
        if was_live {
            self.apply_session();
        }
    }

    fn seek(&mut self, id: u64, reply: Option<u64>, position_ms: u64) {
        let native_req = self.req();
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let status = player.snap().status;
        if !matches!(
            status,
            PlayerStatus::Paused | PlayerStatus::Playing | PlayerStatus::Ended
        ) {
            if let Some(req) = reply {
                player.reply(
                    req,
                    Err(AudioError::new(
                        AudioErrorCode::InvalidOptions,
                        format!("cannot seek while {}", status.as_str()),
                    )),
                );
            }
            return;
        }
        if let Some((_, previous)) = player.seek.take() {
            player.reply(
                previous,
                Err(AudioError::new(
                    AudioErrorCode::Interrupted,
                    "superseded by a later seek",
                )),
            );
        }
        let position_ms = {
            let mut snap = player.snap();
            let position = snap.duration_ms.map_or(position_ms, |d| position_ms.min(d));
            snap.seeking = true;
            snap.position_ms = position;
            snap.position_at = Instant::now();
            if snap.status == PlayerStatus::Ended {
                snap.status = PlayerStatus::Paused;
            }
            position
        };
        match reply {
            Some(req) => player.seek = Some((native_req, req)),
            None => player.seek = None,
        }
        player.emit_state();
        self.send(Command::PlayerSeek {
            id,
            req: native_req,
            position_ms,
        });
    }

    fn step(&mut self, id: u64, req: u64, reason: ItemReason) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let len = player.queue.len();
        let (index, position, repeat) = {
            let snap = player.snap();
            (snap.index, snap.position_now(), snap.repeat)
        };
        let Some(index) = index.filter(|_| len > 0) else {
            player.reply(
                req,
                Err(AudioError::new(
                    AudioErrorCode::InvalidSource,
                    "the queue is empty",
                )),
            );
            return;
        };
        let target = match reason {
            ItemReason::Next if index + 1 < len => Some(index + 1),
            ItemReason::Next if repeat == Repeat::All => Some(0),
            ItemReason::Next => {
                player.reply(
                    req,
                    Err(AudioError::new(
                        AudioErrorCode::InvalidOptions,
                        "there is no next item",
                    )),
                );
                return;
            }
            _ if position > RESTART_THRESHOLD_MS => None,
            _ if index > 0 => Some(index - 1),
            _ if repeat == Repeat::All => Some(len - 1),
            _ => None,
        };
        match target {
            Some(target) => self.change_item(id, req, target, reason),
            None => self.seek(id, Some(req), 0),
        }
    }

    fn skip_to(&mut self, id: u64, req: u64, index: usize) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if index >= player.queue.len() {
            player.reply(
                req,
                Err(AudioError::new(
                    AudioErrorCode::InvalidOptions,
                    format!("index {index} is outside the queue"),
                )),
            );
            return;
        }
        self.change_item(id, req, index, ItemReason::Skip);
    }

    /// Switch items keeping focus: a playing player keeps playing.
    fn change_item(&mut self, id: u64, req: u64, index: usize, reason: ItemReason) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let playing = player.live || !player.play_replies.is_empty();
        self.begin_load(id, index, playing, Some(req), Some(reason), Some(0));
    }

    fn apply_settings(&mut self, id: u64, req: u64) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let (volume, rate) = {
            let snap = player.snap();
            (snap.effective_volume(), snap.rate)
        };
        player.emit_state();
        player.reply(req, Ok(Reply::Done));
        self.send(Command::PlayerVolume { id, volume });
        self.send(Command::PlayerRate { id, rate });
    }

    fn set_output(&mut self, id: u64, req: u64, output: OutputRequest) {
        let earpiece = self.capabilities.earpiece;
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        if output == OutputRequest::Earpiece && !earpiece {
            player.reply(
                req,
                Err(AudioError::new(
                    AudioErrorCode::OutputUnsupported,
                    "this device has no earpiece",
                )),
            );
            return;
        }
        if output == OutputRequest::Earpiece && player.options.focus == Focus::Mix {
            player.reply(
                req,
                Err(AudioError::new(
                    AudioErrorCode::InvalidOptions,
                    "a mix player cannot use the earpiece",
                )),
            );
            return;
        }
        player.options.output = output;
        player.snap().output = output;
        let live = player.live;
        player.emit_state();
        player.reply(req, Ok(Reply::Done));
        if live {
            self.apply_session();
        }
    }

    fn dispose_player(&mut self, id: u64, reply: Option<u64>) {
        let Some(mut player) = self.players.remove(&id) else {
            return;
        };
        player.reject_pending(&AudioError::disposed());
        player.live = false;
        player.snap().status = PlayerStatus::Disposed;
        player.emit_state();
        if let Some(req) = reply {
            player.reply(req, Ok(Reply::Done));
        }
        self.send(Command::PlayerDispose { id });
        self.apply_session();
    }

    /// Pause a live player on behalf of the coordinator and tell it why.
    fn interrupt(&mut self, id: u64, reason: InterruptionReason) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let loading_to_play = player.snap().status == PlayerStatus::Loading && player.autoplay;
        if !player.live && !loading_to_play {
            return;
        }
        let error = AudioError::new(
            AudioErrorCode::Interrupted,
            format!("interrupted: {}", reason.as_str()),
        );
        for req in std::mem::take(&mut player.play_replies) {
            player.reply(req, Err(error.clone()));
        }
        player.autoplay = false;
        let was_live = std::mem::replace(&mut player.live, false);
        {
            let mut snap = player.snap();
            if snap.status == PlayerStatus::Playing {
                snap.position_ms = snap.position_now();
                snap.position_at = Instant::now();
                snap.status = PlayerStatus::Paused;
                snap.buffering = false;
            }
        }
        player.paused_by = Some(reason);
        player.emit_state();
        player.emit(PlayerEvent::Interruption(Interruption {
            phase: InterruptionPhase::Began,
            reason,
            should_resume: false,
            resumed: false,
        }));
        if was_live {
            self.send(Command::PlayerPause { id });
        }
    }

    // ----- sounds -----

    fn load_sound(
        &mut self,
        id: u64,
        owner: OwnerId,
        options: SoundOptions,
        path: String,
        tx: UnboundedSender<SoundEvent>,
        reply: SoundLoadReply,
    ) {
        let req = self.req();
        self.sounds.insert(
            id,
            Sound {
                owner,
                options,
                tx,
                load: Some((req, reply)),
                loaded: false,
                voices: VecDeque::new(),
                started: 0,
            },
        );
        self.send(Command::SoundLoad { id, req, path });
    }

    fn play_voice(&mut self, id: u64, voice: u64, params: VoiceParams) {
        let started = self.start_seq();
        let Some(sound) = self.sounds.get_mut(&id) else {
            return;
        };
        if !sound.loaded {
            let _ = sound.tx.send(SoundEvent::VoiceEnded(voice));
            return;
        }
        let mut stolen = None;
        if sound.voices.len() >= sound.options.max_voices {
            stolen = sound.voices.pop_front();
        }
        let first = sound.voices.is_empty() && stolen.is_none();
        sound.voices.push_back(voice);
        if first {
            sound.started = started;
        }
        if let Some(old) = stolen {
            let _ = sound.tx.send(SoundEvent::VoiceEnded(old));
            self.send(Command::SoundStop { id, voice: old });
        }
        if first {
            self.apply_session();
        }
        self.send(Command::SoundPlay {
            id,
            voice,
            volume: params.volume,
            rate: params.rate,
            pan: params.pan,
            looping: params.looping,
        });
    }

    fn stop_voice(&mut self, id: u64, voice: u64, native: bool) {
        let Some(sound) = self.sounds.get_mut(&id) else {
            return;
        };
        let Some(position) = sound.voices.iter().position(|v| *v == voice) else {
            return;
        };
        sound.voices.remove(position);
        let _ = sound.tx.send(SoundEvent::VoiceEnded(voice));
        let idle = sound.voices.is_empty();
        if native {
            self.send(Command::SoundStop { id, voice });
        }
        if idle {
            self.apply_session();
        }
    }

    fn stop_all_voices(&mut self, id: u64) {
        let Some(sound) = self.sounds.get(&id) else {
            return;
        };
        let voices: Vec<u64> = sound.voices.iter().copied().collect();
        for voice in voices {
            self.stop_voice(id, voice, true);
        }
    }

    fn dispose_sound(&mut self, id: u64) {
        self.stop_all_voices(id);
        if let Some(mut sound) = self.sounds.remove(&id)
            && let Some((_, reply)) = sound.load.take()
        {
            let _ = reply.send(Err(AudioError::disposed()));
        }
        self.send(Command::SoundDispose { id });
    }

    // ----- native events -----

    fn native(&mut self, event: NativeEvent) {
        match event {
            NativeEvent::PlayerLoaded {
                id,
                req,
                duration_ms,
            } => self.on_loaded(id, req, duration_ms),
            NativeEvent::PlayerPlaying { id, position_ms } => self.on_playing(id, position_ms),
            NativeEvent::PlayerPaused { id, position_ms } => self.on_paused(id, position_ms),
            NativeEvent::PlayerSeeked {
                id,
                req,
                position_ms,
            } => self.on_seeked(id, req, position_ms),
            NativeEvent::PlayerProgress {
                id,
                position_ms,
                buffered_ms,
                duration_ms,
            } => self.on_progress(id, position_ms, buffered_ms, duration_ms),
            NativeEvent::PlayerBuffering { id, buffering } => {
                if let Some(player) = self.players.get(&id) {
                    let changed = {
                        let mut snap = player.snap();
                        let changed = snap.buffering != buffering;
                        if changed && buffering {
                            snap.position_ms = snap.position_now();
                            snap.position_at = Instant::now();
                        } else if changed {
                            snap.position_at = Instant::now();
                        }
                        snap.buffering = buffering;
                        changed
                    };
                    if changed {
                        player.emit_state();
                    }
                }
            }
            NativeEvent::PlayerEnded { id } => self.on_ended(id),
            NativeEvent::PlayerError {
                id,
                req,
                code,
                message,
            } => self.on_player_error(id, req, &code, message),
            NativeEvent::SoundLoaded {
                id,
                req,
                duration_ms,
            } => {
                if let Some(sound) = self.sounds.get_mut(&id)
                    && sound.load.as_ref().is_some_and(|(r, _)| *r == req)
                    && let Some((_, reply)) = sound.load.take()
                {
                    sound.loaded = true;
                    let _ = reply.send(Ok(duration_ms));
                }
            }
            NativeEvent::SoundError {
                id,
                req,
                code,
                message,
            } => {
                let failed = self
                    .sounds
                    .get(&id)
                    .is_some_and(|s| s.load.as_ref().is_some_and(|(r, _)| *r == req));
                if failed && let Some(mut sound) = self.sounds.remove(&id) {
                    if let Some((_, reply)) = sound.load.take() {
                        let _ = reply.send(Err(AudioError::new(
                            AudioErrorCode::from_native(&code),
                            message,
                        )));
                    }
                    self.send(Command::SoundDispose { id });
                }
            }
            NativeEvent::SoundEnded { id, voice } => {
                let looping_or_known = self
                    .sounds
                    .get(&id)
                    .is_some_and(|s| s.voices.contains(&voice));
                if looping_or_known {
                    self.stop_voice(id, voice, false);
                }
            }
            NativeEvent::Route {
                output,
                name,
                reason,
                lost,
            } => self.on_route(Route { output, name }, reason, lost),
            NativeEvent::Interruption {
                phase,
                reason,
                should_resume,
            } => self.on_interruption(phase, reason, should_resume),
            NativeEvent::Proximity { near } => self.on_proximity(near),
            NativeEvent::Video {
                component_id,
                playing,
            } => self.on_video(component_id, playing),
        }
    }

    fn on_loaded(&mut self, id: u64, req: u64, duration_ms: Option<u64>) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        if player.load_req != Some(req) {
            return;
        }
        player.load_req = None;
        let index = {
            let mut snap = player.snap();
            snap.status = PlayerStatus::Paused;
            snap.duration_ms = duration_ms.filter(|d| *d > 0);
            snap.position_at = Instant::now();
            snap.index.unwrap_or(0)
        };
        player.emit_state();
        if let Some(reply) = player.load_reply.take() {
            player.reply(reply, Ok(Reply::Loaded { duration_ms, index }));
        }
        let autoplay = std::mem::take(&mut player.autoplay);
        let resume_live = player.live;
        if autoplay || resume_live {
            if resume_live {
                // Item change while playing: focus is already held.
                self.send(Command::PlayerPlay { id });
            } else {
                self.start_play(id);
            }
        }
    }

    fn on_playing(&mut self, id: u64, position_ms: Option<u64>) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        if !player.live {
            return;
        }
        let changed = {
            let mut snap = player.snap();
            let changed = snap.status != PlayerStatus::Playing || snap.buffering;
            snap.status = PlayerStatus::Playing;
            snap.buffering = false;
            if let Some(position) = position_ms {
                snap.position_ms = position;
            }
            snap.position_at = Instant::now();
            changed
        };
        if changed {
            player.emit_state();
        }
        for req in std::mem::take(&mut player.play_replies) {
            player.reply(req, Ok(Reply::Done));
        }
    }

    fn on_paused(&mut self, id: u64, position_ms: Option<u64>) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let system_pause = player.live && player.snap().status == PlayerStatus::Playing;
        {
            let mut snap = player.snap();
            if let Some(position) = position_ms {
                snap.position_ms = position;
                snap.position_at = Instant::now();
            }
            if system_pause {
                snap.status = PlayerStatus::Paused;
                snap.buffering = false;
            }
        }
        if system_pause {
            player.live = false;
            player.emit_state();
            self.apply_session();
        }
    }

    fn on_seeked(&mut self, id: u64, req: u64, position_ms: u64) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let waiting = player.seek.filter(|(native, _)| *native == req);
        let changed = {
            let mut snap = player.snap();
            let changed = snap.seeking;
            snap.seeking = false;
            snap.position_ms = position_ms;
            snap.position_at = Instant::now();
            changed
        };
        if changed {
            player.emit_state();
        }
        if let Some((_, reply)) = waiting {
            player.seek = None;
            player.reply(reply, Ok(Reply::Done));
        }
    }

    fn on_progress(
        &mut self,
        id: u64,
        position_ms: u64,
        buffered_ms: Option<u64>,
        duration_ms: Option<u64>,
    ) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let (emit, duration) = {
            let mut snap = player.snap();
            if snap.seeking || snap.status == PlayerStatus::Loading {
                return;
            }
            snap.position_ms = position_ms;
            snap.position_at = Instant::now();
            if let Some(buffered) = buffered_ms {
                snap.buffered_ms = buffered;
            }
            if let Some(duration) = duration_ms.filter(|d| *d > 0) {
                snap.duration_ms = Some(duration);
            }
            (
                snap.status == PlayerStatus::Playing && player.options.time_update_interval_ms > 0,
                snap.duration_ms,
            )
        };
        if emit {
            player.emit(PlayerEvent::Time {
                position_ms,
                duration_ms: duration,
            });
        }
    }

    fn on_ended(&mut self, id: u64) {
        let req = self.req();
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        if !player.live {
            return;
        }
        let (index, repeat, duration) = {
            let snap = player.snap();
            (snap.index.unwrap_or(0), snap.repeat, snap.duration_ms)
        };
        let len = player.queue.len();
        player.emit(PlayerEvent::Ended { index });
        match repeat {
            Repeat::One => {
                player.snap().position_ms = 0;
                self.send(Command::PlayerSeek {
                    id,
                    req,
                    position_ms: 0,
                });
                self.send(Command::PlayerPlay { id });
            }
            _ if index + 1 < len => {
                self.begin_load(id, index + 1, true, None, Some(ItemReason::Auto), Some(0));
            }
            Repeat::All => self.begin_load(id, 0, true, None, Some(ItemReason::Auto), Some(0)),
            Repeat::Off => {
                player.live = false;
                {
                    let mut snap = player.snap();
                    snap.status = PlayerStatus::Ended;
                    snap.buffering = false;
                    if let Some(duration) = duration {
                        snap.position_ms = duration;
                    }
                    snap.position_at = Instant::now();
                }
                player.emit_state();
                self.apply_session();
            }
        }
    }

    fn on_player_error(&mut self, id: u64, req: Option<u64>, code: &str, message: String) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        if let Some(req) = req
            && player.load_req != Some(req)
        {
            return;
        }
        let error = AudioError::new(AudioErrorCode::from_native(code), message);
        player.reject_pending(&error);
        player.load_req = None;
        player.autoplay = false;
        let was_live = std::mem::replace(&mut player.live, false);
        let index = {
            let mut snap = player.snap();
            snap.status = PlayerStatus::Failed;
            snap.buffering = false;
            snap.seeking = false;
            snap.error = Some(error.clone());
            snap.index
        };
        player.emit_state();
        player.emit(PlayerEvent::Error { error, index });
        if was_live {
            self.apply_session();
        }
    }

    fn on_route(&mut self, route: Route, reason: RouteReason, lost: bool) {
        if lost {
            // Rule 4: a vanished device pauses focus holders, never `mix`.
            let ids: Vec<u64> = self
                .players
                .iter()
                .filter(|(_, p)| p.live && p.options.focus != Focus::Mix)
                .map(|(id, _)| *id)
                .collect();
            for id in ids {
                self.pause_silently(id);
            }
            let videos: Vec<String> = self
                .videos
                .iter()
                .filter(|(_, v)| v.live)
                .map(|(cid, _)| cid.clone())
                .collect();
            for component_id in videos {
                if let Some(video) = self.videos.get_mut(&component_id) {
                    video.live = false;
                }
                self.send(Command::VideoPause { component_id });
            }
        }
        let changed = route != self.route;
        if changed || lost {
            let reason = if lost && reason == RouteReason::Other {
                RouteReason::DeviceRemoved
            } else {
                reason
            };
            self.set_route(route, reason);
        }
        self.apply_session();
    }

    fn pause_silently(&mut self, id: u64) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        player.live = false;
        {
            let mut snap = player.snap();
            if snap.status == PlayerStatus::Playing {
                snap.position_ms = snap.position_now();
                snap.position_at = Instant::now();
                snap.status = PlayerStatus::Paused;
                snap.buffering = false;
            }
        }
        player.emit_state();
        self.send(Command::PlayerPause { id });
    }

    fn set_route(&mut self, route: Route, reason: RouteReason) {
        self.route = route.clone();
        if let Ok(mut shared) = self.route_shared.lock() {
            *shared = route.clone();
        }
        let change = RouteChange { route, reason };
        for player in self.players.values() {
            player.snap().route = change.route.clone();
            player.emit(PlayerEvent::Route(change.clone()));
        }
        self.broadcast(HostEvent::Route(change));
    }

    fn on_interruption(
        &mut self,
        phase: InterruptionPhase,
        reason: InterruptionReason,
        should_resume: bool,
    ) {
        let mut resumed_any = false;
        match phase {
            InterruptionPhase::Began => {
                let ids: Vec<u64> = self
                    .players
                    .iter()
                    .filter(|(_, p)| p.live)
                    .map(|(id, _)| *id)
                    .collect();
                for id in ids {
                    self.interrupt(id, reason);
                }
                let sounds: Vec<u64> = self.sounds.keys().copied().collect();
                for id in sounds {
                    self.stop_all_voices(id);
                }
                let videos: Vec<String> = self
                    .videos
                    .iter()
                    .filter(|(_, v)| v.live)
                    .map(|(cid, _)| cid.clone())
                    .collect();
                for component_id in videos {
                    if let Some(video) = self.videos.get_mut(&component_id) {
                        video.live = false;
                    }
                    self.send(Command::VideoPause { component_id });
                }
                self.apply_session();
            }
            InterruptionPhase::Ended => {
                let ids: Vec<u64> = self
                    .players
                    .iter()
                    .filter(|(_, p)| {
                        p.paused_by.is_some_and(|r| {
                            !matches!(
                                r,
                                InterruptionReason::FocusTaken | InterruptionReason::AppHidden
                            )
                        })
                    })
                    .map(|(id, _)| *id)
                    .collect();
                for id in ids {
                    let Some(player) = self.players.get_mut(&id) else {
                        continue;
                    };
                    let paused_reason = player.paused_by.take().unwrap_or(reason);
                    let resume = should_resume
                        && player.options.resume_after_interruption
                        && player.snap().status == PlayerStatus::Paused;
                    player.emit(PlayerEvent::Interruption(Interruption {
                        phase,
                        reason: paused_reason,
                        should_resume,
                        resumed: resume,
                    }));
                    if resume {
                        resumed_any = true;
                        self.start_play(id);
                    }
                }
            }
        }
        self.broadcast(HostEvent::Interruption(Interruption {
            phase,
            reason,
            should_resume,
            resumed: resumed_any,
        }));
    }

    fn on_proximity(&mut self, near: bool) {
        if self.proximity_near == near {
            return;
        }
        self.proximity_near = near;
        let before = self.session.output;
        self.apply_session();
        if self.session.output != before && self.route.output.is_built_in() {
            let output = match self.session.output {
                super::protocol::SessionOutput::Earpiece => super::types::RouteOutput::Earpiece,
                super::protocol::SessionOutput::Speaker => super::types::RouteOutput::Speaker,
            };
            let route = Route {
                output,
                name: self.route.name.clone(),
            };
            self.set_route(route, RouteReason::Proximity);
        }
    }

    fn on_video(&mut self, component_id: String, playing: bool) {
        if playing {
            let already = self.videos.get(&component_id).is_some_and(|v| v.live);
            if !already {
                self.take_exclusive_focus(None, Some(&component_id));
            }
            let started = if already {
                self.videos.get(&component_id).map_or(0, |v| v.started)
            } else {
                self.start_seq()
            };
            self.videos.insert(
                component_id,
                Video {
                    live: true,
                    started,
                },
            );
        } else {
            self.videos.remove(&component_id);
        }
        self.apply_session();
    }
}

#[cfg(test)]
mod tests;
