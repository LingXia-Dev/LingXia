//! Windows backend of the host audio coordinator.
//!
//! One worker thread owns every `MediaPlayer` and the endpoint enumerator;
//! commands are queued to it and WinRT events report back through
//! `emit_audio_event` from the thread pool. Desktop has no audio focus,
//! earpiece, or silent switch, so `session` commands are accepted and ignored.

use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::audio::{AudioBackend, emit_audio_event};
use crate::traits::video_player::VideoPlayerCommand;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::Foundation::{TimeSpan, TypedEventHandler, Uri};
use windows::Media::Core::MediaSource;
use windows::Media::Playback::{
    MediaPlaybackSession, MediaPlaybackState, MediaPlayer, MediaPlayerError,
    MediaPlayerFailedEventArgs,
};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::{
    DEVICE_STATE, DEVICE_STATE_ACTIVE, DigitalAudioDisplayDevice, EDataFlow, ERole, Handset,
    Headphones, Headset, IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl,
    MMDeviceEnumerator, PKEY_AudioEndpoint_FormFactor, SPDIF, eConsole, eRender,
};
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, STGM_READ,
};
use windows::Win32::System::Variant::{VT_LPWSTR, VT_UI4};
use windows::core::{HSTRING, IInspectable, PCWSTR, Ref, implement};

/// Idle wake-up for progress ticks; players tick at their own `tickMs`.
const IDLE_WAIT: Duration = Duration::from_millis(50);

enum Msg {
    Command(String),
    VoiceDone { id: u64, voice: u64 },
    RouteChanged,
}

static WORKER: OnceLock<Sender<Msg>> = OnceLock::new();

fn worker() -> &'static Sender<Msg> {
    WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let worker_tx = tx.clone();
        let spawned = std::thread::Builder::new()
            .name("lx-audio-windows".into())
            .spawn(move || run(rx, worker_tx));
        if let Err(error) = spawned {
            log::error!("audio worker failed to start: {error}");
        }
        tx
    })
}

impl AudioBackend for Platform {
    fn audio_attach(&self, _event_callback_id: u64) -> Result<(), PlatformError> {
        worker();
        Ok(())
    }

    fn audio_send(&self, command_json: &str) -> Result<(), PlatformError> {
        worker()
            .send(Msg::Command(command_json.to_string()))
            .map_err(|_| PlatformError::Platform("audio worker is gone".into()))
    }
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all_fields = "camelCase")]
enum Command {
    #[serde(rename = "player.create")]
    PlayerCreate { id: u64, tick_ms: u32 },
    #[serde(rename = "player.load")]
    PlayerLoad {
        id: u64,
        req: u64,
        uri: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        start_ms: u64,
    },
    #[serde(rename = "player.play")]
    PlayerPlay { id: u64 },
    #[serde(rename = "player.pause")]
    PlayerPause { id: u64 },
    #[serde(rename = "player.seek")]
    PlayerSeek { id: u64, req: u64, position_ms: u64 },
    #[serde(rename = "player.unload")]
    PlayerUnload { id: u64 },
    #[serde(rename = "player.volume")]
    PlayerVolume { id: u64, volume: f64 },
    #[serde(rename = "player.rate")]
    PlayerRate { id: u64, rate: f64 },
    #[serde(rename = "player.dispose")]
    PlayerDispose { id: u64 },
    #[serde(rename = "sound.load")]
    SoundLoad { id: u64, req: u64, path: String },
    #[serde(rename = "sound.play")]
    SoundPlay {
        id: u64,
        voice: u64,
        volume: f64,
        rate: f64,
        pan: f64,
        #[serde(rename = "loop")]
        looping: bool,
    },
    #[serde(rename = "sound.stop")]
    SoundStop { id: u64, voice: u64 },
    #[serde(rename = "sound.dispose")]
    SoundDispose { id: u64 },
    #[serde(rename = "session")]
    Session {},
    #[serde(rename = "video.pause")]
    VideoPause { component_id: String },
}

fn emit(event: Value) {
    let _ = emit_audio_event(event.to_string());
}

fn ms(span: TimeSpan) -> u64 {
    (span.Duration.max(0) / 10_000) as u64
}

fn span(ms: u64) -> TimeSpan {
    TimeSpan {
        Duration: (ms as i64).saturating_mul(10_000),
    }
}

/// `MediaSource` takes URIs; local files become `file:///` URIs. Resolved
/// paths arrive canonicalized as `\\?\C:\...`, which no URI parser accepts.
fn source_uri(uri: &str) -> String {
    let lower = uri.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return uri.to_string();
    }
    let path = uri.strip_prefix(r"\\?\").unwrap_or(uri);
    let mut out = String::from("file:///");
    for ch in path.trim_start_matches(['\\', '/']).chars() {
        match ch {
            '\\' => out.push('/'),
            ' ' => out.push_str("%20"),
            '%' => out.push_str("%25"),
            '#' => out.push_str("%23"),
            _ => out.push(ch),
        }
    }
    out
}

fn open_source(uri: &str) -> windows::core::Result<MediaSource> {
    let uri = Uri::CreateUri(&HSTRING::from(source_uri(uri)))?;
    MediaSource::CreateFromUri(&uri)
}

fn new_player() -> windows::core::Result<MediaPlayer> {
    let player = MediaPlayer::new()?;
    // Phase 1 owns no Now Playing slot: keep players off the SMTC flyout.
    player.CommandManager()?.SetIsEnabled(false)?;
    player.SetAutoPlay(false)?;
    Ok(player)
}

fn failure_code(args: &MediaPlayerFailedEventArgs) -> (&'static str, String) {
    let error = args.Error().unwrap_or(MediaPlayerError::Unknown);
    let message = args
        .ErrorMessage()
        .map(|m| m.to_string())
        .unwrap_or_default();
    let code = match error {
        MediaPlayerError::NetworkError => "network",
        MediaPlayerError::SourceNotSupported => "unsupported_format",
        _ => "decode_failed",
    };
    (code, message)
}

struct PlayerShared {
    id: u64,
    load_req: AtomicU64,
    start_ms: AtomicU64,
    seek_req: AtomicU64,
    opened: AtomicBool,
    playing: AtomicBool,
    disposed: AtomicBool,
}

struct Player {
    media: MediaPlayer,
    shared: Arc<PlayerShared>,
    tick: Duration,
    last_tick: Instant,
}

fn session_position(session: &MediaPlaybackSession) -> u64 {
    session.Position().map(ms).unwrap_or(0)
}

fn create_player(id: u64, tick_ms: u32) -> windows::core::Result<Player> {
    let media = new_player()?;
    let shared = Arc::new(PlayerShared {
        id,
        load_req: AtomicU64::new(0),
        start_ms: AtomicU64::new(0),
        seek_req: AtomicU64::new(0),
        opened: AtomicBool::new(false),
        playing: AtomicBool::new(false),
        disposed: AtomicBool::new(false),
    });

    let s = shared.clone();
    media.MediaOpened(&TypedEventHandler::new(
        move |sender: Ref<MediaPlayer>, _: Ref<IInspectable>| {
            if s.disposed.load(Ordering::Acquire) {
                return Ok(());
            }
            let Some(player) = sender.as_ref() else {
                return Ok(());
            };
            let session = player.PlaybackSession()?;
            let start = s.start_ms.load(Ordering::Acquire);
            if start > 0 {
                let _ = session.SetPosition(span(start));
            }
            s.opened.store(true, Ordering::Release);
            let duration = session.NaturalDuration().map(ms).unwrap_or(0);
            emit(json!({
                "event": "player.loaded",
                "id": s.id,
                "req": s.load_req.load(Ordering::Acquire),
                "durationMs": (duration > 0).then_some(duration),
            }));
            Ok(())
        },
    ))?;

    let s = shared.clone();
    media.MediaFailed(&TypedEventHandler::new(
        move |_: Ref<MediaPlayer>, args: Ref<MediaPlayerFailedEventArgs>| {
            if s.disposed.load(Ordering::Acquire) {
                return Ok(());
            }
            let Some(args) = args.as_ref() else {
                return Ok(());
            };
            let (code, message) = failure_code(args);
            s.playing.store(false, Ordering::Release);
            // Before open the failure answers the load; afterwards it is a playback error.
            let req =
                (!s.opened.load(Ordering::Acquire)).then(|| s.load_req.load(Ordering::Acquire));
            emit(json!({
                "event": "player.error",
                "id": s.id,
                "req": req,
                "code": code,
                "message": message,
            }));
            Ok(())
        },
    ))?;

    let s = shared.clone();
    media.MediaEnded(&TypedEventHandler::new(
        move |_: Ref<MediaPlayer>, _: Ref<IInspectable>| {
            if !s.disposed.load(Ordering::Acquire) {
                s.playing.store(false, Ordering::Release);
                emit(json!({ "event": "player.ended", "id": s.id }));
            }
            Ok(())
        },
    ))?;

    let session = media.PlaybackSession()?;
    let s = shared.clone();
    session.PlaybackStateChanged(&TypedEventHandler::new(
        move |sender: Ref<MediaPlaybackSession>, _: Ref<IInspectable>| {
            if s.disposed.load(Ordering::Acquire) {
                return Ok(());
            }
            let Some(session) = sender.as_ref() else {
                return Ok(());
            };
            let playing = session.PlaybackState()? == MediaPlaybackState::Playing;
            let was = s.playing.swap(playing, Ordering::AcqRel);
            if playing && !was {
                emit(json!({
                    "event": "player.playing",
                    "id": s.id,
                    "positionMs": session_position(session),
                }));
            }
            Ok(())
        },
    ))?;

    let s = shared.clone();
    session.SeekCompleted(&TypedEventHandler::new(
        move |sender: Ref<MediaPlaybackSession>, _: Ref<IInspectable>| {
            let req = s.seek_req.swap(0, Ordering::AcqRel);
            if req != 0
                && !s.disposed.load(Ordering::Acquire)
                && let Some(session) = sender.as_ref()
            {
                emit(json!({
                    "event": "player.seeked",
                    "id": s.id,
                    "req": req,
                    "positionMs": session_position(session),
                }));
            }
            Ok(())
        },
    ))?;

    for buffering in [true, false] {
        let s = shared.clone();
        let handler =
            TypedEventHandler::new(move |_: Ref<MediaPlaybackSession>, _: Ref<IInspectable>| {
                if !s.disposed.load(Ordering::Acquire) {
                    emit(json!({
                        "event": "player.buffering",
                        "id": s.id,
                        "buffering": buffering,
                    }));
                }
                Ok(())
            });
        if buffering {
            session.BufferingStarted(&handler)?;
        } else {
            session.BufferingEnded(&handler)?;
        }
    }

    Ok(Player {
        media,
        shared,
        tick: Duration::from_millis(tick_ms.max(50) as u64),
        last_tick: Instant::now(),
    })
}

struct Sound {
    uri: String,
    probe: MediaPlayer,
    voices: HashMap<u64, MediaPlayer>,
}

fn load_sound(id: u64, req: u64, path: &str) -> windows::core::Result<Sound> {
    let probe = new_player()?;
    probe.MediaOpened(&TypedEventHandler::new(
        move |sender: Ref<MediaPlayer>, _: Ref<IInspectable>| {
            let duration = sender
                .as_ref()
                .and_then(|p| p.PlaybackSession().ok())
                .and_then(|s| s.NaturalDuration().ok())
                .map(ms)
                .unwrap_or(0);
            emit(json!({ "event": "sound.loaded", "id": id, "req": req, "durationMs": duration }));
            Ok(())
        },
    ))?;
    probe.MediaFailed(&TypedEventHandler::new(
        move |_: Ref<MediaPlayer>, args: Ref<MediaPlayerFailedEventArgs>| {
            let (code, message) = args
                .as_ref()
                .map(failure_code)
                .unwrap_or(("decode_failed", String::new()));
            emit(json!({
                "event": "sound.error",
                "id": id,
                "req": req,
                "code": code,
                "message": message,
            }));
            Ok(())
        },
    ))?;
    probe.SetSource(&open_source(path)?)?;
    Ok(Sound {
        uri: path.to_string(),
        probe,
        voices: HashMap::new(),
    })
}

fn play_voice(
    id: u64,
    voice: u64,
    sound: &Sound,
    params: (f64, f64, f64, bool),
    worker: Sender<Msg>,
) -> windows::core::Result<MediaPlayer> {
    let (volume, rate, pan, looping) = params;
    let player = new_player()?;
    player.SetVolume(volume)?;
    player.SetAudioBalance(pan)?;
    player.SetIsLoopingEnabled(looping)?;
    player.MediaOpened(&TypedEventHandler::new(
        move |sender: Ref<MediaPlayer>, _: Ref<IInspectable>| {
            // Rate is per opened item; set it before starting.
            if let Some(player) = sender.as_ref() {
                let _ = player.PlaybackSession()?.SetPlaybackRate(rate);
                player.Play()?;
            }
            Ok(())
        },
    ))?;
    let ended = move |worker: &Sender<Msg>| {
        emit(json!({ "event": "sound.ended", "id": id, "voice": voice }));
        let _ = worker.send(Msg::VoiceDone { id, voice });
    };
    let on_end = worker.clone();
    player.MediaEnded(&TypedEventHandler::new(
        move |_: Ref<MediaPlayer>, _: Ref<IInspectable>| {
            ended(&on_end);
            Ok(())
        },
    ))?;
    player.MediaFailed(&TypedEventHandler::new(
        move |_: Ref<MediaPlayer>, _: Ref<MediaPlayerFailedEventArgs>| {
            ended(&worker);
            Ok(())
        },
    ))?;
    player.SetSource(&open_source(&sound.uri)?)?;
    Ok(player)
}

#[derive(Clone, PartialEq, Eq)]
struct RouteInfo {
    device_id: String,
    output: &'static str,
    name: Option<String>,
}

#[implement(IMMNotificationClient)]
struct EndpointWatcher {
    worker: Mutex<Sender<Msg>>,
}

impl IMMNotificationClient_Impl for EndpointWatcher_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnDeviceAdded(&self, _: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnDeviceRemoved(&self, _: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        role: ERole,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        if flow == eRender
            && role == eConsole
            && let Ok(worker) = self.worker.lock()
        {
            let _ = worker.send(Msg::RouteChanged);
        }
        Ok(())
    }
    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> windows::core::Result<()> {
        Ok(())
    }
}

unsafe fn read_string(value: &PROPVARIANT) -> Option<String> {
    let inner = unsafe { &*value.Anonymous.Anonymous };
    if inner.vt != VT_LPWSTR {
        return None;
    }
    let text = unsafe { inner.Anonymous.pwszVal.to_string() }.ok()?;
    (!text.is_empty()).then_some(text)
}

unsafe fn read_u32(value: &PROPVARIANT) -> Option<u32> {
    let inner = unsafe { &*value.Anonymous.Anonymous };
    if inner.vt == VT_UI4 {
        Some(unsafe { inner.Anonymous.ulVal })
    } else {
        None
    }
}

fn default_route(enumerator: &IMMDeviceEnumerator) -> Option<RouteInfo> {
    unsafe {
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        let device_id = device.GetId().ok()?.to_string().ok()?;
        let store = device.OpenPropertyStore(STGM_READ).ok()?;
        let mut name = store.GetValue(&PKEY_Device_FriendlyName).ok();
        let mut form = store.GetValue(&PKEY_AudioEndpoint_FormFactor).ok();
        let friendly = name.as_ref().and_then(|v| read_string(v));
        let factor = form.as_ref().and_then(|v| read_u32(v)).unwrap_or(10) as i32;
        if let Some(value) = name.as_mut() {
            let _ = PropVariantClear(value);
        }
        if let Some(value) = form.as_mut() {
            let _ = PropVariantClear(value);
        }
        let lower = friendly.clone().unwrap_or_default().to_ascii_lowercase();
        let output = if lower.contains("bluetooth")
            || lower.contains("airpods")
            || lower.contains("hands-free")
        {
            "bluetooth"
        } else if factor == Headphones.0 || factor == Headset.0 || factor == Handset.0 {
            "headphones"
        } else if factor == DigitalAudioDisplayDevice.0 || factor == SPDIF.0 {
            "hdmi"
        } else if lower.contains("usb") {
            "usb"
        } else if factor == 1 || factor == 2 || factor == 10 {
            // Speakers, line level, or unknown: the device's own output.
            "speaker"
        } else {
            "other"
        };
        Some(RouteInfo {
            device_id,
            output,
            name: friendly,
        })
    }
}

fn device_still_active(enumerator: &IMMDeviceEnumerator, device_id: &str) -> bool {
    let wide = HSTRING::from(device_id);
    unsafe {
        enumerator
            .GetDevice(PCWSTR(wide.as_ptr()))
            .and_then(|device| device.GetState())
            .is_ok_and(|state| state == DEVICE_STATE_ACTIVE)
    }
}

fn emit_route(route: &RouteInfo, reason: &str, lost: bool) {
    emit(json!({
        "event": "route",
        "output": route.output,
        "name": route.name,
        "reason": reason,
        "lost": lost,
    }));
}

struct Worker {
    tx: Sender<Msg>,
    players: HashMap<u64, Player>,
    sounds: HashMap<u64, Sound>,
    enumerator: Option<IMMDeviceEnumerator>,
    route: Option<RouteInfo>,
}

fn run(rx: Receiver<Msg>, tx: Sender<Msg>) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let enumerator: Option<IMMDeviceEnumerator> =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .inspect_err(|error| log::warn!("audio endpoints unavailable: {error}"))
            .ok();
    let mut worker = Worker {
        tx: tx.clone(),
        players: HashMap::new(),
        sounds: HashMap::new(),
        route: enumerator.as_ref().and_then(default_route),
        enumerator,
    };
    let fallback = RouteInfo {
        device_id: String::new(),
        output: "speaker",
        name: None,
    };
    emit_route(worker.route.as_ref().unwrap_or(&fallback), "other", false);
    // Kept alive for the process: the enumerator holds it only weakly.
    let _watcher: Option<IMMNotificationClient> = worker.enumerator.as_ref().and_then(|e| {
        let client: IMMNotificationClient = EndpointWatcher {
            worker: Mutex::new(tx.clone()),
        }
        .into();
        unsafe { e.RegisterEndpointNotificationCallback(&client) }
            .inspect_err(|error| log::warn!("audio route watch unavailable: {error}"))
            .ok()
            .map(|_| client)
    });

    loop {
        match rx.recv_timeout(IDLE_WAIT) {
            Ok(Msg::Command(json)) => match serde_json::from_str::<Command>(&json) {
                Ok(command) => worker.command(command),
                Err(error) => log::warn!("unsupported audio command {json}: {error}"),
            },
            Ok(Msg::VoiceDone { id, voice }) => {
                if let Some(player) = worker
                    .sounds
                    .get_mut(&id)
                    .and_then(|sound| sound.voices.remove(&voice))
                {
                    let _ = player.Close();
                }
            }
            Ok(Msg::RouteChanged) => worker.route_changed(),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        worker.tick();
    }
}

impl Worker {
    fn command(&mut self, command: Command) {
        if let Err(error) = self.apply(command) {
            log::warn!("audio command failed: {error}");
        }
    }

    fn apply(&mut self, command: Command) -> windows::core::Result<()> {
        match command {
            Command::PlayerCreate { id, tick_ms } => {
                let player = create_player(id, tick_ms)?;
                self.players.insert(id, player);
            }
            Command::PlayerLoad {
                id,
                req,
                uri,
                headers,
                start_ms,
            } => {
                let Some(player) = self.players.get(&id) else {
                    return Ok(());
                };
                // `MediaSource::CreateFromUri` cannot send headers; fetching the
                // bare URL would fail later as an unexplained `network` error.
                if !headers.is_empty() {
                    emit(json!({
                        "event": "player.error",
                        "id": id,
                        "req": req,
                        "code": "invalid_source",
                        "message": "request headers (source.headers, referrerPolicy 'origin') are not supported on Windows",
                    }));
                    return Ok(());
                }
                let shared = &player.shared;
                shared.load_req.store(req, Ordering::Release);
                shared.start_ms.store(start_ms, Ordering::Release);
                shared.opened.store(false, Ordering::Release);
                shared.playing.store(false, Ordering::Release);
                let source = match open_source(&uri) {
                    Ok(source) => source,
                    Err(error) => {
                        emit(json!({
                            "event": "player.error",
                            "id": id,
                            "req": req,
                            "code": "invalid_source",
                            "message": error.message(),
                        }));
                        return Ok(());
                    }
                };
                player.media.Pause()?;
                player.media.SetSource(&source)?;
            }
            Command::PlayerPlay { id } => {
                if let Some(player) = self.players.get(&id) {
                    player.media.Play()?;
                }
            }
            Command::PlayerPause { id } => {
                if let Some(player) = self.players.get(&id) {
                    player.media.Pause()?;
                    player.shared.playing.store(false, Ordering::Release);
                    let position = session_position(&player.media.PlaybackSession()?);
                    emit(json!({ "event": "player.paused", "id": id, "positionMs": position }));
                }
            }
            Command::PlayerSeek {
                id,
                req,
                position_ms,
            } => {
                if let Some(player) = self.players.get(&id) {
                    let session = player.media.PlaybackSession()?;
                    if session.CanSeek().unwrap_or(false) {
                        player.shared.seek_req.store(req, Ordering::Release);
                        session.SetPosition(span(position_ms))?;
                    } else {
                        // No SeekCompleted will follow: answer so the caller settles.
                        emit(json!({
                            "event": "player.seeked",
                            "id": id,
                            "req": req,
                            "positionMs": session_position(&session),
                        }));
                    }
                }
            }
            Command::PlayerUnload { id } => {
                if let Some(player) = self.players.get(&id) {
                    player.shared.playing.store(false, Ordering::Release);
                    player.shared.load_req.store(0, Ordering::Release);
                    player.media.Pause()?;
                    player
                        .media
                        .SetSource(None::<&windows::Media::Playback::IMediaPlaybackSource>)?;
                }
            }
            Command::PlayerVolume { id, volume } => {
                if let Some(player) = self.players.get(&id) {
                    player.media.SetVolume(volume.clamp(0.0, 1.0))?;
                }
            }
            Command::PlayerRate { id, rate } => {
                if let Some(player) = self.players.get(&id) {
                    player.media.PlaybackSession()?.SetPlaybackRate(rate)?;
                }
            }
            Command::PlayerDispose { id } => {
                if let Some(player) = self.players.remove(&id) {
                    player.shared.disposed.store(true, Ordering::Release);
                    let _ = player.media.Close();
                }
            }
            Command::SoundLoad { id, req, path } => match load_sound(id, req, &path) {
                Ok(sound) => {
                    self.sounds.insert(id, sound);
                }
                Err(error) => emit(json!({
                    "event": "sound.error",
                    "id": id,
                    "req": req,
                    "code": "decode_failed",
                    "message": error.message(),
                })),
            },
            Command::SoundPlay {
                id,
                voice,
                volume,
                rate,
                pan,
                looping,
            } => {
                let Some(sound) = self.sounds.get(&id) else {
                    return Ok(());
                };
                match play_voice(
                    id,
                    voice,
                    sound,
                    (volume, rate, pan, looping),
                    self.tx.clone(),
                ) {
                    Ok(player) => {
                        if let Some(sound) = self.sounds.get_mut(&id) {
                            sound.voices.insert(voice, player);
                        }
                    }
                    Err(error) => {
                        log::warn!("sound voice failed: {error}");
                        emit(json!({ "event": "sound.ended", "id": id, "voice": voice }));
                    }
                }
            }
            Command::SoundStop { id, voice } => {
                if let Some(player) = self
                    .sounds
                    .get_mut(&id)
                    .and_then(|sound| sound.voices.remove(&voice))
                {
                    let _ = player.Close();
                }
            }
            Command::SoundDispose { id } => {
                if let Some(sound) = self.sounds.remove(&id) {
                    for player in sound.voices.into_values() {
                        let _ = player.Close();
                    }
                    let _ = sound.probe.Close();
                }
            }
            Command::Session {} => {}
            Command::VideoPause { component_id } => {
                if let Err(error) =
                    super::video_player::dispatch(&component_id, &VideoPlayerCommand::Pause)
                {
                    log::warn!("video {component_id} did not pause for audio focus: {error}");
                }
            }
        }
        Ok(())
    }

    fn tick(&mut self) {
        let now = Instant::now();
        for player in self.players.values_mut() {
            if !player.shared.playing.load(Ordering::Acquire)
                || now.duration_since(player.last_tick) < player.tick
            {
                continue;
            }
            player.last_tick = now;
            let Ok(session) = player.media.PlaybackSession() else {
                continue;
            };
            let duration = session.NaturalDuration().map(ms).unwrap_or(0);
            let buffered = session
                .DownloadProgress()
                .map(|progress| (progress.clamp(0.0, 1.0) * duration as f64) as u64)
                .unwrap_or(0);
            emit(json!({
                "event": "player.progress",
                "id": player.shared.id,
                "positionMs": session_position(&session),
                "bufferedMs": buffered,
                "durationMs": (duration > 0).then_some(duration),
            }));
        }
    }

    fn route_changed(&mut self) {
        let Some(enumerator) = self.enumerator.as_ref() else {
            return;
        };
        let Some(next) = default_route(enumerator) else {
            return;
        };
        let previous = self.route.replace(next.clone());
        if previous.as_ref() == Some(&next) {
            return;
        }
        let lost = previous.as_ref().is_some_and(|previous| {
            previous.output != "speaker" && !device_still_active(enumerator, &previous.device_id)
        });
        let reason = if lost {
            "deviceRemoved"
        } else if next.output != "speaker" {
            "deviceAdded"
        } else {
            "other"
        };
        emit_route(&next, reason, lost);
    }
}

#[cfg(test)]
mod tests {
    use super::source_uri;

    #[test]
    fn local_paths_become_file_uris() {
        assert_eq!(
            source_uri(r"C:\Users\me\My Music\a#1.mp3"),
            "file:///C:/Users/me/My%20Music/a%231.mp3"
        );
        assert_eq!(
            source_uri(r"\\?\D:\cache\tone.wav"),
            "file:///D:/cache/tone.wav"
        );
        assert_eq!(source_uri("https://x.y/a.mp3"), "https://x.y/a.mp3");
    }
}
