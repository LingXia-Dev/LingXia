use super::events::handle_player_event;
use super::stream::seek_stream_session_sync_shared;
use crate::i18n::{js_error_from_platform_error, js_internal_error, js_invalid_parameter_error};
use lingxia_media::StreamSession;
use lingxia_messaging::{CallbackResult, register_handler, remove_callback};
use lingxia_platform::Platform;
use lingxia_platform::traits::stream_decoder::VideoStreamDecoderHandle;
use lingxia_platform::traits::video_player::{VideoPlayerHandle, VideoPlayerManager};
use lxapp::{LxApp, lx};
use rong::{JSContext, JSFunc, JSResult, js_export};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

pub fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_class::<JSVideoContext>()?;
    let create_ctx = JSFunc::new(ctx, |ctx: JSContext, component_id: String| {
        JSVideoContext::create(&ctx, component_id)
    })?;
    lx::register_js_api(ctx, "createVideoContext", create_ctx)?;
    Ok(())
}

#[js_export]
pub struct JSVideoContext {
    pub(super) component_id: String,
    pub(super) player_handle: Arc<dyn VideoPlayerHandle>,
    pub(super) runtime: Arc<Platform>,
    pub(super) shared: Arc<VideoContextSharedState>,
}

#[derive(Debug, Clone)]
pub(super) struct StreamSourceState {
    pub(super) provider: String,
    pub(super) params: Value,
}

pub(super) struct VideoContextSharedState {
    pub(super) runtime: Arc<Platform>,
    pub(super) stream_session: Mutex<Option<Box<dyn StreamSession>>>,
    pub(super) stream_decoder: Mutex<Option<Arc<dyn VideoStreamDecoderHandle>>>,
    pub(super) stream_epoch: Arc<AtomicU64>,
    pub(super) last_stream_source: Mutex<Option<StreamSourceState>>,
    pub(super) stream_live: AtomicBool,
    pub(super) stream_paused: AtomicBool,
    pub(super) stream_duration_override_ms: AtomicU64,
    pub(super) last_stream_position_ms: AtomicU64,
    pub(super) play_requested: AtomicBool,
    pub(super) platform_playing: AtomicBool,
    pub(super) decoder_reset_pending: AtomicBool,
    pub(super) callback_id: Mutex<Option<u64>>,
    pub(super) last_stream_recovery_ms: AtomicU64,
    pub(super) stream_starting: AtomicBool,
    pub(super) seek_callback_registered: AtomicBool,
    pub(super) callback_bound: AtomicBool,
}

impl VideoContextSharedState {
    fn new(runtime: Arc<Platform>) -> Self {
        Self {
            runtime,
            stream_session: Mutex::new(None),
            stream_decoder: Mutex::new(None),
            stream_epoch: Arc::new(AtomicU64::new(0)),
            last_stream_source: Mutex::new(None),
            stream_live: AtomicBool::new(false),
            stream_paused: AtomicBool::new(false),
            stream_duration_override_ms: AtomicU64::new(0),
            last_stream_position_ms: AtomicU64::new(0),
            play_requested: AtomicBool::new(false),
            platform_playing: AtomicBool::new(false),
            decoder_reset_pending: AtomicBool::new(false),
            callback_id: Mutex::new(None),
            last_stream_recovery_ms: AtomicU64::new(0),
            stream_starting: AtomicBool::new(false),
            seek_callback_registered: AtomicBool::new(false),
            callback_bound: AtomicBool::new(false),
        }
    }

    fn register_callback(
        shared: &Arc<VideoContextSharedState>,
        component_id: &str,
    ) -> JSResult<u64> {
        {
            let guard = shared
                .callback_id
                .lock()
                .map_err(|_| js_internal_error("Callback lock poisoned"))?;
            if let Some(id) = *guard {
                return Ok(id);
            }
        }

        let shared_for_handler = shared.clone();
        let component_id_for_handler = component_id.to_string();
        let new_callback_id = register_handler(move |result| {
            if let CallbackResult::Success(payload) = result {
                handle_player_event(&shared_for_handler, &component_id_for_handler, &payload);
            }
        });

        let mut guard = shared
            .callback_id
            .lock()
            .map_err(|_| js_internal_error("Callback lock poisoned"))?;
        if let Some(existing) = *guard {
            remove_callback(new_callback_id);
            return Ok(existing);
        }

        *guard = Some(new_callback_id);
        Ok(new_callback_id)
    }
}

type VideoContextRegistryKey = (usize, String);

fn video_context_registry()
-> &'static Mutex<HashMap<VideoContextRegistryKey, Weak<VideoContextSharedState>>> {
    static REGISTRY: OnceLock<
        Mutex<HashMap<VideoContextRegistryKey, Weak<VideoContextSharedState>>>,
    > = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn shared_state_for(runtime: &Arc<Platform>, component_id: &str) -> Arc<VideoContextSharedState> {
    let key: VideoContextRegistryKey = (Arc::as_ptr(runtime) as usize, component_id.to_string());
    let mut guard = video_context_registry()
        .lock()
        .expect("VideoContext registry lock poisoned");

    guard.retain(|_, weak| weak.upgrade().is_some());

    if let Some(existing) = guard.get(&key).and_then(|weak| weak.upgrade()) {
        return existing;
    }

    let state = Arc::new(VideoContextSharedState::new(runtime.clone()));
    guard.insert(key, Arc::downgrade(&state));
    state
}

impl JSVideoContext {
    pub(super) fn create(ctx: &JSContext, component_id: String) -> JSResult<Self> {
        let component_id = component_id.trim().to_string();
        if component_id.is_empty() {
            return Err(js_invalid_parameter_error("componentId required"));
        }

        let lxapp = LxApp::from_ctx(ctx)?;
        let runtime = lxapp.runtime.clone();
        let shared = shared_state_for(&runtime, &component_id);
        let callback_id = VideoContextSharedState::register_callback(&shared, &component_id)?;
        if !shared.callback_bound.swap(true, Ordering::AcqRel)
            && let Err(e) = runtime.set_player_callback(&component_id, callback_id)
        {
            shared.callback_bound.store(false, Ordering::Release);
            return Err(js_error_from_platform_error(&e));
        }
        let handle = runtime
            .bind_player(&component_id)
            .map_err(|e| js_error_from_platform_error(&e))?;

        // Register stream seek callback so FFI layer can trigger seek without depending on logic layer.
        // Only register once per shared state to avoid callback being lost when JSVideoContext is GC'd.
        if !shared.seek_callback_registered.swap(true, Ordering::AcqRel) {
            let shared_for_seek = shared.clone();
            let component_id_for_seek = component_id.clone();
            lingxia_media::register_stream_seek_callback(&component_id, move |position| {
                seek_stream_session_sync_shared(&shared_for_seek, &component_id_for_seek, position)
            });
        }

        Ok(Self {
            component_id,
            player_handle: handle.into(),
            runtime,
            shared,
        })
    }
}
