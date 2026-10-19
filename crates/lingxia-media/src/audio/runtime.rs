//! The coordinator's worker thread and its handles.

use super::coordinator::{
    Coordinator, HostEvent, Input, OwnerId, PlayerCommand, PlayerEvent, PlayerOptions,
    PlayerSnapshot, SoundEvent, SoundOptions, VoiceParams,
};
use super::protocol::NativeEvent;
use super::types::{AudioError, AudioErrorCode, Route};
use lingxia_messaging::{CallbackResult, register_handler};
use lingxia_platform::traits::audio::{
    AudioBackend, AudioDeviceCapabilities, set_audio_event_callback,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio::sync::oneshot;

pub struct AudioRuntime {
    tx: Sender<Input>,
    route: Arc<Mutex<Route>>,
    capabilities: AudioDeviceCapabilities,
    next_id: AtomicU64,
}

pub struct PlayerHandle {
    pub id: u64,
    pub shared: Arc<Mutex<PlayerSnapshot>>,
    pub events: UnboundedReceiver<PlayerEvent>,
}

pub struct SoundHandle {
    pub id: u64,
    pub events: UnboundedReceiver<SoundEvent>,
    pub loaded: oneshot::Receiver<Result<u64, AudioError>>,
}

static RUNTIME: OnceLock<Result<AudioRuntime, AudioError>> = OnceLock::new();

/// Start the coordinator once per process, attached to `backend`.
pub fn start(backend: Arc<dyn AudioBackend>) -> Result<&'static AudioRuntime, AudioError> {
    RUNTIME
        .get_or_init(|| AudioRuntime::spawn(backend))
        .as_ref()
        .map_err(Clone::clone)
}

/// The running coordinator, if Logic has started it.
pub fn get() -> Option<&'static AudioRuntime> {
    RUNTIME.get().and_then(|runtime| runtime.as_ref().ok())
}

impl AudioRuntime {
    fn spawn(backend: Arc<dyn AudioBackend>) -> Result<Self, AudioError> {
        let capabilities = backend.audio_capabilities();
        let route = Arc::new(Mutex::new(Route::default()));
        let (tx, rx) = mpsc::channel::<Input>();
        let sink_backend = backend.clone();
        let sink = Box::new(move |command| {
            let json = match serde_json::to_string(&command) {
                Ok(json) => json,
                Err(error) => {
                    log::error!("audio command does not serialize: {error}");
                    return;
                }
            };
            if let Err(error) = sink_backend.audio_send(&json) {
                log::warn!("audio command rejected by the platform: {error}");
            }
        });
        let mut coordinator = Coordinator::new(sink, capabilities, route.clone());
        std::thread::Builder::new()
            .name("lx-audio".into())
            .spawn(move || {
                while let Ok(input) = rx.recv() {
                    coordinator.handle(input);
                }
            })
            .map_err(|error| {
                AudioError::new(
                    AudioErrorCode::DecodeFailed,
                    format!("audio worker failed to start: {error}"),
                )
            })?;

        let events = tx.clone();
        let callback_id = register_handler(move |result| {
            let CallbackResult::Success(json) = result else {
                return;
            };
            match serde_json::from_str::<NativeEvent>(&json) {
                Ok(event) => {
                    let _ = events.send(Input::Native(event));
                }
                Err(error) => log::warn!("unrecognized audio event {json}: {error}"),
            }
        });
        set_audio_event_callback(callback_id);
        if let Err(error) = backend.audio_attach(callback_id) {
            log::warn!("audio backend did not attach: {error}");
        }
        Ok(Self {
            tx,
            route,
            capabilities,
            next_id: AtomicU64::new(1),
        })
    }

    fn post(&self, input: Input) {
        let _ = self.tx.send(input);
    }

    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn capabilities(&self) -> AudioDeviceCapabilities {
        self.capabilities
    }

    pub fn route(&self) -> Route {
        self.route.lock().map(|r| r.clone()).unwrap_or_default()
    }

    pub fn register_owner(&self) -> (OwnerId, UnboundedReceiver<HostEvent>) {
        let owner = self.next_id();
        let (tx, rx) = unbounded_channel();
        self.post(Input::RegisterOwner { owner, tx });
        (owner, rx)
    }

    pub fn set_owner_visible(&self, owner: OwnerId, visible: bool) {
        self.post(Input::OwnerVisible { owner, visible });
    }

    pub fn shutdown_owner(&self, owner: OwnerId) {
        self.post(Input::OwnerShutdown { owner });
    }

    pub fn create_player(&self, owner: OwnerId, options: PlayerOptions) -> PlayerHandle {
        let id = self.next_id();
        let shared = Arc::new(Mutex::new(PlayerSnapshot::new(&options, self.route())));
        let (tx, events) = unbounded_channel();
        self.post(Input::CreatePlayer {
            id,
            owner,
            options,
            shared: shared.clone(),
            tx,
        });
        PlayerHandle { id, shared, events }
    }

    pub fn player_command(&self, id: u64, req: u64, command: PlayerCommand) {
        self.post(Input::Player { id, req, command });
    }

    pub fn load_sound(&self, owner: OwnerId, options: SoundOptions, path: String) -> SoundHandle {
        let id = self.next_id();
        let (tx, events) = unbounded_channel();
        let (reply, loaded) = oneshot::channel();
        self.post(Input::LoadSound {
            id,
            owner,
            options,
            path,
            tx,
            reply,
        });
        SoundHandle { id, events, loaded }
    }

    pub fn play_voice(&self, sound: u64, params: VoiceParams) -> u64 {
        let voice = self.next_id();
        self.post(Input::PlayVoice {
            id: sound,
            voice,
            params,
        });
        voice
    }

    pub fn stop_voice(&self, sound: u64, voice: u64) {
        self.post(Input::StopVoice { id: sound, voice });
    }

    pub fn stop_all_voices(&self, sound: u64) {
        self.post(Input::StopAllVoices { id: sound });
    }

    pub fn dispose_sound(&self, sound: u64) {
        self.post(Input::DisposeSound { id: sound });
    }
}
