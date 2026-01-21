use lingxia_messaging::{CallbackResult, register_handler, remove_callback};
use lingxia_platform::Platform;
use lingxia_platform::traits::stream_decoder::{
    VideoStreamDecoderHandle, VideoStreamDecoderManager,
};
use lingxia_platform::traits::video_player::{
    VideoPlayerCommand, VideoPlayerHandle, VideoPlayerManager,
};
use log::{debug, info, warn};
use lxapp::stream_source::{FrameSink, StreamSession, get_stream_provider};
use lxapp::{LxApp, lx};
use rong::{
    FromJSObj, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError, error::HostError,
    js_class, js_export, js_method,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn ensure_platform_stream_play(shared: &Arc<VideoContextSharedState>, component_id: &str) {
    if shared.stream_paused.load(Ordering::Relaxed) {
        return;
    }
    if !shared.play_requested.load(Ordering::Relaxed)
        && !shared.platform_playing.load(Ordering::Relaxed)
    {
        return;
    }
    let Ok(handle) = shared.runtime.bind_player(component_id) else {
        return;
    };
    if let Err(err) = handle.dispatch(VideoPlayerCommand::Play) {
        warn!(
            "stream ensure play failed component_id={} err={}",
            component_id, err
        );
    }
}

pub fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_class::<JSVideoContext>()?;
    let create_ctx = JSFunc::new(ctx, |ctx: JSContext, component_id: String| {
        JSVideoContext::create(&ctx, component_id)
    })?;
    lx::register_js_api(ctx, "createVideoContext", create_ctx)?;
    Ok(())
}

#[derive(FromJSObj)]
struct JSStreamSourceOptions {
    #[rename = "provider"]
    provider: String,
    #[rename = "isLive"]
    is_live: bool,
    duration: Option<f64>,
    params: Option<JSObject>,
}

fn parse_stream_params(params: Option<JSObject>) -> JSResult<Value> {
    let Some(obj) = params else {
        return Ok(Value::Null);
    };
    let json = obj.json_stringify()?;
    serde_json::from_str(&json).map_err(|e| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            format!("params must be JSON-compatible: {}", e),
        ))
    })
}

#[js_export]
pub struct JSVideoContext {
    component_id: String,
    player_handle: Arc<dyn VideoPlayerHandle>,
    runtime: Arc<Platform>,
    shared: Arc<VideoContextSharedState>,
}

#[derive(Debug, Clone)]
struct StreamSourceState {
    provider: String,
    params: Value,
}

pub struct VideoContextSharedState {
    runtime: Arc<Platform>,
    stream_session: Mutex<Option<Box<dyn StreamSession>>>,
    stream_decoder: Mutex<Option<Arc<dyn VideoStreamDecoderHandle>>>,
    stream_epoch: Arc<AtomicU64>,
    last_stream_source: Mutex<Option<StreamSourceState>>,
    stream_live: AtomicBool,
    stream_paused: AtomicBool,
    stream_duration_override_ms: AtomicU64,
    last_stream_position_ms: AtomicU64,
    play_requested: AtomicBool,
    platform_playing: AtomicBool,
    decoder_reset_pending: AtomicBool,
    callback_id: Mutex<Option<u64>>,
    last_stream_recovery_ms: AtomicU64,
    stream_starting: AtomicBool,
    seek_callback_registered: AtomicBool,
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
        }
    }

    fn register_callback(
        shared: &Arc<VideoContextSharedState>,
        component_id: &str,
    ) -> JSResult<u64> {
        {
            let guard = shared.callback_id.lock().map_err(|_| {
                RongJSError::from(HostError::new(
                    rong::error::E_INTERNAL,
                    "Callback lock poisoned",
                ))
            })?;
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

        let mut guard = shared.callback_id.lock().map_err(|_| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                "Callback lock poisoned",
            ))
        })?;
        if let Some(existing) = *guard {
            remove_callback(new_callback_id);
            return Ok(existing);
        }

        info!(
            "VideoContext register callback component_id={} callback_id={}",
            component_id, new_callback_id
        );
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
    pub fn create(ctx: &JSContext, component_id: String) -> JSResult<Self> {
        if component_id.trim().is_empty() {
            return Err(RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                "componentId required",
            )));
        }

        let lxapp = LxApp::from_ctx(ctx)?;
        let runtime = lxapp.runtime.clone();
        let shared = shared_state_for(&runtime, &component_id);
        let callback_id = VideoContextSharedState::register_callback(&shared, &component_id)?;
        runtime
            .set_player_callback(&component_id, callback_id)
            .map_err(|e| {
                RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
            })?;
        let handle = runtime.bind_player(&component_id).map_err(|e| {
            RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
        })?;

        // Register stream seek callback so FFI layer can trigger seek without depending on logic layer.
        // Only register once per shared state to avoid callback being lost when JSVideoContext is GC'd.
        if !shared.seek_callback_registered.swap(true, Ordering::AcqRel) {
            let shared_for_seek = shared.clone();
            let component_id_for_seek = component_id.clone();
            lxapp::stream_source::register_stream_seek_callback(&component_id, move |position| {
                seek_stream_session_sync_shared(&shared_for_seek, &component_id_for_seek, position)
            });
            info!(
                "VideoContext seek callback registered component_id={}",
                component_id
            );
        }

        let instance = Self {
            component_id,
            player_handle: handle.into(),
            runtime,
            shared,
        };
        Ok(instance)
    }

    fn dispatch(&self, command: VideoPlayerCommand) -> JSResult<()> {
        self.player_handle
            .dispatch(command)
            .map_err(|e| RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string())))
    }

    fn stop_stream_session_async(&self) -> JSResult<()> {
        let mut guard = self.shared.stream_session.lock().map_err(|_| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                "Stream session lock poisoned",
            ))
        })?;
        let session = guard.take();
        drop(guard);

        if let Some(session) = session {
            thread::spawn(move || {
                if let Err(err) = session.stop() {
                    warn!("Failed to stop stream session: {}", err);
                }
            });
        }
        Ok(())
    }

    fn ensure_stream_decoder(&self) -> JSResult<Arc<dyn VideoStreamDecoderHandle>> {
        let mut guard = self.shared.stream_decoder.lock().map_err(|_| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                "Stream decoder lock poisoned",
            ))
        })?;
        if let Some(decoder) = guard.as_ref() {
            return Ok(decoder.clone());
        }

        let decoder = self
            .runtime
            .create_stream_decoder(&self.component_id)
            .map_err(|e| {
                RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
            })?;
        let decoder: Arc<dyn VideoStreamDecoderHandle> = decoder.into();
        *guard = Some(decoder.clone());
        Ok(decoder)
    }

    fn has_stream_source(&self) -> JSResult<bool> {
        let guard = self.shared.last_stream_source.lock().map_err(|_| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                "Stream source lock poisoned",
            ))
        })?;
        Ok(guard.is_some())
    }

    fn has_stream_session(&self) -> JSResult<bool> {
        let guard = self.shared.stream_session.lock().map_err(|_| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                "Stream session lock poisoned",
            ))
        })?;
        Ok(guard.is_some())
    }

    fn resume_stream_session(&self) -> JSResult<()> {
        resume_stream_session_shared(&self.shared, &self.component_id)
    }
}

// Note: We intentionally don't unregister the seek callback in Drop.
// The callback is tied to VideoContextSharedState lifetime, not JSVideoContext instance.
// The callback holds a strong reference to shared state, keeping it alive as needed.

#[js_class]
impl JSVideoContext {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "Use lx.createVideoContext()",
        )))
    }

    #[js_method]
    fn play(&self) -> JSResult<()> {
        if self.has_stream_source()? {
            let is_live = self.shared.stream_live.load(Ordering::Relaxed);
            let was_paused = self.shared.stream_paused.swap(false, Ordering::Relaxed);
            self.shared.play_requested.store(true, Ordering::Relaxed);
            if self.shared.decoder_reset_pending.load(Ordering::Relaxed) {
                return self.dispatch(VideoPlayerCommand::Play);
            }
            let has_session = self.has_stream_session()?;

            if !has_session {
                if is_live {
                    stop_stream_session_async_shared(&self.shared);
                    if let Err(err) =
                        reset_decoder_soft_if_present_shared(&self.shared, &self.component_id)
                    {
                        warn!(
                            "live play-intent reset_stream failed component_id={} err={}",
                            self.component_id, err
                        );
                    }
                }
                if let Err(err) = resume_stream_session_shared(&self.shared, &self.component_id) {
                    warn!(
                        "resume stream session failed component_id={} err={}",
                        self.component_id, err
                    );
                }
            } else if !is_live && was_paused {
                resume_or_seek_stream_session_shared(&self.shared, &self.component_id);
            }
        }
        self.dispatch(VideoPlayerCommand::Play)
    }

    #[js_method]
    fn pause(&self) -> JSResult<()> {
        if self.has_stream_source()? {
            let is_live = self.shared.stream_live.load(Ordering::Relaxed);
            self.shared.play_requested.store(false, Ordering::Relaxed);
            if !self.shared.stream_paused.swap(true, Ordering::Relaxed) {
                if is_live {
                    self.shared.stream_epoch.fetch_add(1, Ordering::Relaxed);
                    stop_stream_session_async_shared(&self.shared);
                } else {
                    pause_or_resume_stream_session_shared(&self.shared, false, &self.component_id);
                }
            }
        }
        self.dispatch(VideoPlayerCommand::Pause)
    }

    #[js_method]
    fn stop(&self) -> JSResult<()> {
        if self.has_stream_source()? {
            self.shared.play_requested.store(false, Ordering::Relaxed);
            self.shared.stream_paused.store(false, Ordering::Relaxed);
            if self.shared.stream_live.load(Ordering::Relaxed) {
                self.shared.stream_epoch.fetch_add(1, Ordering::Relaxed);
            }
            stop_stream_session_async_shared(&self.shared);
            if let Err(err) = reset_decoder_soft_shared(&self.shared, &self.component_id) {
                warn!(
                    "stop reset_stream failed component_id={} err={}",
                    self.component_id, err
                );
            }
        }
        self.dispatch(VideoPlayerCommand::Stop)
    }

    #[js_method]
    fn seek(&self, position: f64) -> JSResult<()> {
        if self.has_stream_source()? {
            seek_stream_session_async_shared(&self.shared, &self.component_id, position);
        }
        self.dispatch(VideoPlayerCommand::Seek { position })
    }

    #[js_method(rename = "requestFullScreen")]
    fn request_full_screen(&self) -> JSResult<()> {
        self.dispatch(VideoPlayerCommand::EnterFullscreen)
    }

    #[js_method(rename = "exitFullScreen")]
    fn exit_full_screen(&self) -> JSResult<()> {
        self.dispatch(VideoPlayerCommand::ExitFullscreen)
    }

    #[js_method(rename = "setStreamSource")]
    fn set_stream_source(&self, options: JSStreamSourceOptions) -> JSResult<()> {
        if options.provider.trim().is_empty() {
            return Err(RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                "provider is required",
            )));
        }

        let provider = options.provider;
        let provider_impl = get_stream_provider(&provider).ok_or_else(|| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                format!("Stream provider not found: {}", provider),
            ))
        })?;
        let params = parse_stream_params(options.params)?;
        let is_live = options.is_live;
        let duration_seconds = if is_live {
            None
        } else {
            options.duration.filter(|v| v.is_finite() && *v > 0.0)
        };
        self.shared.stream_live.store(is_live, Ordering::Relaxed);
        info!(
            "setStreamSource component_id={} live={} duration={:?}",
            self.component_id, is_live, duration_seconds
        );

        // Duration is playback-segment metadata (used by native UI to enable progress/seek in
        // stream-decoder mode). Keep it out of provider params.
        let duration_for_player = if is_live {
            0.0
        } else {
            duration_seconds.unwrap_or(0.0)
        };
        self.shared.stream_duration_override_ms.store(
            (duration_for_player * 1000.0).round() as u64,
            Ordering::Relaxed,
        );
        // Clear last position when switching streams - the old position is invalid for the new segment
        // and would cause seek errors (e.g., seeking to 46s in a 30s segment).
        self.shared
            .last_stream_position_ms
            .store(0, Ordering::Relaxed);
        if let Err(err) = self.dispatch(VideoPlayerCommand::SetDuration {
            duration: duration_for_player,
        }) {
            warn!(
                "setStreamSource setDuration ignored component_id={} err={}",
                self.component_id, err
            );
        }

        let force_hard = {
            let guard = self.shared.last_stream_source.lock().map_err(|_| {
                RongJSError::from(HostError::new(
                    rong::error::E_INTERNAL,
                    "Stream source lock poisoned",
                ))
            })?;
            match guard.as_ref() {
                Some(prev) if prev.provider != provider => true,
                Some(prev) => provider_impl.should_force_hard_switch(Some(&prev.params), &params),
                None => false,
            }
        };
        if force_hard {
            warn!(
                "setStreamSource forcing hard switch component_id={} reason=identity_change",
                self.component_id
            );
        }

        // Increment the epoch before switching, so any in-flight work from the previous
        // session cannot configure/push/stop the decoder after this point.
        let epoch = self.shared.stream_epoch.fetch_add(1, Ordering::Relaxed) + 1;

        let existing_decoder = {
            let guard = self.shared.stream_decoder.lock().map_err(|_| {
                RongJSError::from(HostError::new(
                    rong::error::E_INTERNAL,
                    "Stream decoder lock poisoned",
                ))
            })?;
            guard.clone()
        };
        let had_existing_decoder = existing_decoder.is_some();

        let mut decoder = match existing_decoder {
            Some(decoder) => decoder,
            None => self.ensure_stream_decoder()?,
        };

        let supports_soft_reset = decoder.supports_soft_reset();
        let supports_in_place_hard_reset = decoder.supports_in_place_hard_reset();
        // Even when the stream identity changes (camera/device/range/token), we prefer reusing the
        // native decoder session when a soft reset is available. Fully stopping/recreating the
        // stream decoder can tear down the TextureView/surface on Android and lead to stalls/ANRs
        // during rapid stream switching.
        let reuse_decoder = supports_soft_reset;
        let hard_reset_in_place = force_hard && supports_in_place_hard_reset;
        info!(
            "setStreamSource component_id={} epoch={} reuse_decoder={}",
            self.component_id, epoch, reuse_decoder
        );

        if reuse_decoder {
            self.stop_stream_session_async()?;
        } else {
            self.stop_stream_session_async()?;
            if had_existing_decoder {
                let old_decoder = {
                    let mut guard = self.shared.stream_decoder.lock().map_err(|_| {
                        RongJSError::from(HostError::new(
                            rong::error::E_INTERNAL,
                            "Stream decoder lock poisoned",
                        ))
                    })?;
                    guard.take()
                };

                if let Some(decoder) = old_decoder {
                    let component_id = self.component_id.clone();
                    thread::spawn(move || {
                        if let Err(err) = decoder.stop() {
                            warn!(
                                "setStreamSource failed to stop old decoder component_id={} err={}",
                                component_id, err
                            );
                        }
                    });
                }

                decoder = self.ensure_stream_decoder()?;
            }
            if let Err(err) = decoder.reset_stream(false) {
                warn!(
                    "setStreamSource failed to reset decoder component_id={} epoch={} err={}",
                    self.component_id, epoch, err
                );
            }
        }

        // Update stream source after successfully switching the decoder state.
        let mut source_guard = self.shared.last_stream_source.lock().map_err(|_| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                "Stream source lock poisoned",
            ))
        })?;
        *source_guard = Some(StreamSourceState { provider, params });

        if reuse_decoder {
            // Decoder reset can be slow on iOS (main-thread work). Run it off the JS thread to avoid
            // blocking page interactions; start the provider after reset if a play intent exists.
            self.shared
                .decoder_reset_pending
                .store(true, Ordering::Relaxed);
            let shared = self.shared.clone();
            let component_id = self.component_id.clone();
            let decoder_reset = decoder.clone();
            let epoch_token = self.shared.stream_epoch.clone();
            let expected_epoch = epoch;
            thread::spawn(move || {
                let reset_started = Instant::now();
                if let Err(err) = decoder_reset.reset_stream(hard_reset_in_place) {
                    warn!(
                        "setStreamSource reset_stream failed component_id={} hard_reset_in_place={} err={}",
                        component_id, hard_reset_in_place, err
                    );

                    if !hard_reset_in_place {
                        for _ in 0..20 {
                            thread::sleep(Duration::from_millis(10));
                            if epoch_token.load(Ordering::Relaxed) != expected_epoch {
                                break;
                            }
                            if decoder_reset.reset_stream(false).is_ok() {
                                break;
                            }
                        }
                    }
                }
                info!(
                    "setStreamSource reset_stream done component_id={} hard_reset_in_place={} elapsed_ms={}",
                    component_id,
                    hard_reset_in_place,
                    reset_started.elapsed().as_millis()
                );

                if epoch_token.load(Ordering::Relaxed) != expected_epoch {
                    return;
                }
                shared.decoder_reset_pending.store(false, Ordering::Relaxed);

                let should_autostart = (shared.play_requested.load(Ordering::Relaxed)
                    || shared.platform_playing.load(Ordering::Relaxed))
                    && !shared.stream_paused.load(Ordering::Relaxed);
                if should_autostart {
                    if let Err(err) = resume_stream_session_shared(&shared, &component_id) {
                        warn!(
                            "setStreamSource autostart failed component_id={} err={}",
                            component_id, err
                        );
                    }
                }
            });
        } else {
            // If the player is already in a playing state (e.g. switching cameras while playing),
            // start pulling the new stream immediately; otherwise wait for the platform play intent.
            let should_autostart = (self.shared.play_requested.load(Ordering::Relaxed)
                || self.shared.platform_playing.load(Ordering::Relaxed))
                && !self.shared.stream_paused.load(Ordering::Relaxed);
            if should_autostart {
                if let Err(err) = self.resume_stream_session() {
                    warn!(
                        "setStreamSource autostart failed component_id={} err={}",
                        self.component_id, err
                    );
                }
            }
        }

        Ok(())
    }

    #[js_method(gc_mark)]
    fn gc_mark_with<F>(&self, _mark_fn: F)
    where
        F: FnMut(&JSValue),
    {
    }
}

fn handle_player_event(shared: &Arc<VideoContextSharedState>, component_id: &str, payload: &str) {
    let Ok(value) = serde_json::from_str::<Value>(payload) else {
        return;
    };
    let Some(event) = value.get("event").and_then(|v| v.as_str()) else {
        return;
    };
    let waiting_reason = value
        .get("detail")
        .and_then(|detail| detail.get("reason"))
        .and_then(|reason| reason.as_str());
    let pause_reason = value
        .get("detail")
        .and_then(|detail| detail.get("reason"))
        .and_then(|reason| reason.as_str());
    let seek_time = value
        .get("detail")
        .and_then(|detail| detail.get("time").or_else(|| detail.get("currentTime")))
        .and_then(|v| v.as_f64())
        .filter(|v| v.is_finite() && *v >= 0.0);
    debug!(
        "VideoContext callback component_id={} event={} waiting_reason={:?} pause_reason={:?}",
        component_id, event, waiting_reason, pause_reason
    );

    let has_source = shared
        .last_stream_source
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false);

    // Events can arrive before `setStreamSource()` is called (e.g. user taps play on a stream-mode
    // player). Treat `playrequest` as a "play intent" latch so the subsequent `setStreamSource()`
    // call can autostart via `play_requested`.
    //
    // Also make sure `unmount` always releases callback resources, even if no stream source was set.
    if !has_source {
        match event {
            "playrequest" => {
                shared.play_requested.store(true, Ordering::Relaxed);
                shared.stream_paused.store(false, Ordering::Relaxed);
            }
            "unmount" => {
                shared.platform_playing.store(false, Ordering::Relaxed);
                shared.play_requested.store(false, Ordering::Relaxed);
                shared.stream_paused.store(true, Ordering::Relaxed);
                stop_stream_session_async_shared(shared);
                let _ = reset_decoder_soft_if_present_shared(shared, component_id);
                if let Ok(mut guard) = shared.last_stream_source.lock() {
                    *guard = None;
                }
                if let Ok(mut guard) = shared.callback_id.lock() {
                    if let Some(callback_id) = guard.take() {
                        remove_callback(callback_id);
                    }
                }
                // Unregister seek callback to prevent leak and stale closure
                if shared
                    .seek_callback_registered
                    .swap(false, Ordering::AcqRel)
                {
                    lxapp::stream_source::unregister_stream_seek_callback(component_id);
                }
            }
            // If a play intent was latched but the user cancels before configuring a source,
            // clear the pending intent so `setStreamSource()` doesn't autostart unexpectedly.
            "pause" | "stop" | "ended" => {
                if shared.play_requested.load(Ordering::Relaxed) {
                    shared.platform_playing.store(false, Ordering::Relaxed);
                    shared.play_requested.store(false, Ordering::Relaxed);
                    shared.stream_paused.store(true, Ordering::Relaxed);
                }
            }
            _ => {}
        }
        return;
    }

    let is_live = shared.stream_live.load(Ordering::Relaxed);

    let handle_play_intent = || {
        let was_paused = shared.stream_paused.swap(false, Ordering::Relaxed);
        shared.play_requested.store(true, Ordering::Relaxed);
        let has_session = shared
            .stream_session
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false);

        if !has_session {
            if is_live {
                stop_stream_session_async_shared(shared);
                if let Err(err) = reset_decoder_soft_if_present_shared(shared, component_id) {
                    warn!(
                        "live play-intent reset_stream failed component_id={} err={}",
                        component_id, err
                    );
                }
            }
            if let Err(err) = resume_stream_session_shared(shared, component_id) {
                warn!(
                    "resume stream session failed component_id={} err={}",
                    component_id, err
                );
            }
            return;
        }

        if !is_live && was_paused {
            resume_or_seek_stream_session_shared(shared, component_id);
        }
    };

    match event {
        "playrequest" => {
            handle_play_intent();
        }
        "waiting" => {
            // `waiting` can be emitted for multiple reasons (buffering, surface rebind, config
            // changes, decoder failures). Only decoder recovery signals should trigger Rust-side
            // control actions.
            let Some(reason) = waiting_reason else {
                return;
            };

            let is_decoder_recovery = matches!(reason, "decode_failed" | "stuck" | "decode");
            if !is_decoder_recovery {
                // Ignore regular buffering/config signals. Otherwise live streams can restart and
                // appear stuck on a frame.
                return;
            }

            if is_decoder_recovery {
                // Don't attempt decoder recovery while the user has explicitly paused.
                if shared.stream_paused.load(Ordering::Relaxed) {
                    return;
                }
                if is_live {
                    // Decoder recovery for live streams: restart the session. Rate-limit to avoid
                    // storms when the decoder rejects multiple consecutive samples.
                    let now = now_epoch_ms();
                    let last = shared.last_stream_recovery_ms.load(Ordering::Relaxed);
                    if now.saturating_sub(last) < 2_000 {
                        return;
                    }
                    shared.last_stream_recovery_ms.store(now, Ordering::Relaxed);
                    warn!(
                        "live stream decoder recovery component_id={} reason={:?}",
                        component_id, waiting_reason
                    );
                    shared.stream_epoch.fetch_add(1, Ordering::Relaxed);
                    stop_stream_session_async_shared(shared);
                    if let Err(err) = reset_decoder_soft_if_present_shared(shared, component_id) {
                        warn!(
                            "live decoder recovery reset_stream failed component_id={} err={}",
                            component_id, err
                        );
                    }
                    if let Err(err) = resume_stream_session_shared(shared, component_id) {
                        warn!(
                            "live decoder recovery resume failed component_id={} err={}",
                            component_id, err
                        );
                    }
                    return;
                }

                // For VOD segments, avoid restarting the stream session (which can lose position).
                // Soft-reset the decoder to recover from transient decode failures.
                if let Err(err) = reset_decoder_soft_if_present_shared(shared, component_id) {
                    warn!(
                        "vod decoder recovery reset_stream failed component_id={} err={}",
                        component_id, err
                    );
                }
                return;
            }
        }
        "playing" => {
            // State signal (first decoded frame / resume), not a play intent.
            shared.platform_playing.store(true, Ordering::Relaxed);
        }
        "seeked" => {
            let Some(position) = seek_time else {
                return;
            };
            if position.is_finite() && position >= 0.0 {
                shared
                    .last_stream_position_ms
                    .store((position * 1000.0).round() as u64, Ordering::Relaxed);
            }
        }
        "seeking" => {
            let Some(position) = seek_time else {
                return;
            };
            if is_live {
                // Live streams are not seekable.
                return;
            }

            if position.is_finite() && position >= 0.0 {
                shared
                    .last_stream_position_ms
                    .store((position * 1000.0).round() as u64, Ordering::Relaxed);
            }

            let has_session = shared
                .stream_session
                .lock()
                .map(|guard| guard.is_some())
                .unwrap_or(false);
            if has_session {
                seek_stream_session_async_shared(shared, component_id, position);
                // Seek was dispatched; avoid re-seeking again on the subsequent play intent.
                shared.last_stream_position_ms.store(0, Ordering::Relaxed);
            }
        }
        "ended" => {
            // Stream playback segment ended (progress reached duration).
            // Stop the Rust-side stream session to avoid continued network/CPU while UI is at 100%.
            shared.platform_playing.store(false, Ordering::Relaxed);
            shared.play_requested.store(false, Ordering::Relaxed);
            if !is_live {
                shared.stream_paused.store(true, Ordering::Relaxed);
                shared.last_stream_position_ms.store(0, Ordering::Relaxed);
                stop_stream_session_async_shared(shared);
            }
        }
        "pause" => {
            if let Some(position) = value
                .get("detail")
                .and_then(|detail| {
                    detail
                        .get("currentTime")
                        .or_else(|| detail.get("time"))
                        .or_else(|| detail.get("position"))
                })
                .and_then(|v| v.as_f64())
                .filter(|v| v.is_finite() && *v >= 0.0)
            {
                shared
                    .last_stream_position_ms
                    .store((position * 1000.0).round() as u64, Ordering::Relaxed);
            }
            shared.platform_playing.store(false, Ordering::Relaxed);
            if is_live {
                if let Some(reason) = pause_reason {
                    if reason != "user" {
                        return;
                    }
                }
            }
            shared.play_requested.store(false, Ordering::Relaxed);
            if !shared.stream_paused.swap(true, Ordering::Relaxed) {
                if is_live {
                    shared.stream_epoch.fetch_add(1, Ordering::Relaxed);
                    stop_stream_session_async_shared(shared);
                } else {
                    pause_or_resume_stream_session_shared(shared, false, component_id);
                }
            }
        }
        "stop" => {
            shared.platform_playing.store(false, Ordering::Relaxed);
            shared.play_requested.store(false, Ordering::Relaxed);
            shared.stream_paused.store(false, Ordering::Relaxed);
            if is_live {
                shared.stream_epoch.fetch_add(1, Ordering::Relaxed);
            }
            stop_stream_session_async_shared(shared);
            if let Err(err) = reset_decoder_soft_shared(shared, component_id) {
                warn!(
                    "stop reset_stream failed component_id={} err={}",
                    component_id, err
                );
            }
        }
        "unmount" => {
            shared.platform_playing.store(false, Ordering::Relaxed);
            shared.play_requested.store(false, Ordering::Relaxed);
            shared.stream_paused.store(true, Ordering::Relaxed);
            if is_live {
                shared.stream_epoch.fetch_add(1, Ordering::Relaxed);
            }
            stop_stream_session_async_shared(shared);
            if let Err(err) = reset_decoder_soft_shared(shared, component_id) {
                warn!(
                    "unmount reset_stream failed component_id={} err={}",
                    component_id, err
                );
            }
            if let Ok(mut guard) = shared.last_stream_source.lock() {
                *guard = None;
            }
            if let Ok(mut guard) = shared.callback_id.lock() {
                if let Some(callback_id) = guard.take() {
                    remove_callback(callback_id);
                }
            }
            // Unregister seek callback to prevent leak and stale closure.
            if shared
                .seek_callback_registered
                .swap(false, Ordering::AcqRel)
            {
                lxapp::stream_source::unregister_stream_seek_callback(component_id);
            }
        }
        _ => {}
    }
}

fn ensure_stream_decoder_shared(
    shared: &Arc<VideoContextSharedState>,
    component_id: &str,
) -> Result<Arc<dyn VideoStreamDecoderHandle>, RongJSError> {
    let mut guard = shared.stream_decoder.lock().map_err(|_| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "Stream decoder lock poisoned",
        ))
    })?;
    if let Some(decoder) = guard.as_ref() {
        return Ok(decoder.clone());
    }

    let decoder = shared
        .runtime
        .create_stream_decoder(component_id)
        .map_err(|e| RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string())))?;
    let decoder: Arc<dyn VideoStreamDecoderHandle> = decoder.into();
    *guard = Some(decoder.clone());
    Ok(decoder)
}

fn reset_decoder_soft_shared(
    shared: &Arc<VideoContextSharedState>,
    component_id: &str,
) -> Result<(), RongJSError> {
    let decoder = ensure_stream_decoder_shared(shared, component_id)?;
    decoder
        .reset_stream(false)
        .map_err(|e| RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string())))
}

fn reset_decoder_soft_if_present_shared(
    shared: &Arc<VideoContextSharedState>,
    _component_id: &str,
) -> Result<(), RongJSError> {
    let decoder = shared
        .stream_decoder
        .lock()
        .map_err(|_| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                "Stream decoder lock poisoned",
            ))
        })?
        .clone();
    let Some(decoder) = decoder else {
        return Ok(());
    };
    decoder
        .reset_stream(false)
        .map_err(|e| RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string())))
}

pub fn stop_stream_session_async_shared(shared: &Arc<VideoContextSharedState>) {
    let session = {
        let mut guard = match shared.stream_session.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        guard.take()
    };

    if let Some(session) = session {
        thread::spawn(move || {
            if let Err(err) = session.stop() {
                warn!("Failed to stop stream session: {}", err);
            }
        });
    }
}

pub fn resume_stream_session_shared(
    shared: &Arc<VideoContextSharedState>,
    component_id: &str,
) -> Result<(), RongJSError> {
    if shared
        .stream_starting
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        info!(
            "resume_stream_session already in flight component_id={}",
            component_id
        );
        return Ok(());
    }
    struct Reset<'a>(&'a AtomicBool);
    impl Drop for Reset<'_> {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Release);
        }
    }
    let _reset = Reset(&shared.stream_starting);

    let source = {
        let guard = shared.last_stream_source.lock().map_err(|_| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                "Stream source lock poisoned",
            ))
        })?;
        guard.clone()
    };

    let Some(source) = source else {
        return Ok(());
    };

    let provider = get_stream_provider(&source.provider).ok_or_else(|| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            format!("Stream provider not found: {}", source.provider),
        ))
    })?;

    let epoch = shared.stream_epoch.fetch_add(1, Ordering::Relaxed) + 1;
    let decoder = ensure_stream_decoder_shared(shared, component_id)?;
    let shared_for_duration = shared.clone();
    let component_id_for_duration = component_id.to_string();
    let expected_epoch_for_duration = epoch;
    let last_duration_ms = Arc::new(AtomicU64::new(0));
    let last_duration_ms_for_reporter = last_duration_ms.clone();
    let shared_for_ended = shared.clone();
    let component_id_for_ended = component_id.to_string();
    let expected_epoch_for_ended = epoch;
    let ended_reported = Arc::new(AtomicBool::new(false));
    let ended_reported_for_reporter = ended_reported.clone();
    let sink = FrameSink::from_arc_with_epoch(decoder, shared.stream_epoch.clone(), epoch)
        .with_component_id(component_id.to_string())
        .with_duration_reporter(move |duration_ms| {
            if shared_for_duration.stream_live.load(Ordering::Relaxed) {
                return;
            }
            if shared_for_duration
                .stream_duration_override_ms
                .load(Ordering::Relaxed)
                > 0
            {
                return;
            }
            if shared_for_duration.stream_epoch.load(Ordering::Relaxed)
                != expected_epoch_for_duration
            {
                return;
            }
            if last_duration_ms_for_reporter.load(Ordering::Relaxed) == duration_ms {
                return;
            }

            let Ok(handle) = shared_for_duration
                .runtime
                .bind_player(&component_id_for_duration)
            else {
                return;
            };
            if handle
                .dispatch(VideoPlayerCommand::SetDuration {
                    duration: (duration_ms as f64) / 1000.0,
                })
                .is_ok()
            {
                info!(
                    "stream duration reported component_id={} duration_ms={}",
                    component_id_for_duration, duration_ms
                );
                last_duration_ms_for_reporter.store(duration_ms, Ordering::Relaxed);
            }
        })
        .with_ended_reporter(move || {
            if shared_for_ended.stream_live.load(Ordering::Relaxed) {
                return;
            }
            if shared_for_ended.stream_epoch.load(Ordering::Relaxed) != expected_epoch_for_ended {
                return;
            }
            if ended_reported_for_reporter.swap(true, Ordering::Relaxed) {
                return;
            }
            let Ok(handle) = shared_for_ended
                .runtime
                .bind_player(&component_id_for_ended)
            else {
                return;
            };
            if handle.dispatch(VideoPlayerCommand::NotifyEnded).is_ok() {
                info!(
                    "stream ended reported component_id={}",
                    component_id_for_ended
                );
            }
        });

    let session = provider.start(source.params, sink).map_err(|err| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            format!("Stream provider start failed: {}", err),
        ))
    })?;

    let mut guard = shared.stream_session.lock().map_err(|_| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "Stream session lock poisoned",
        ))
    })?;
    *guard = Some(session);

    ensure_platform_stream_play(shared, component_id);

    Ok(())
}

pub fn seek_stream_session_async_shared(
    shared: &Arc<VideoContextSharedState>,
    component_id: &str,
    position: f64,
) {
    let _ = seek_stream_session_sync_shared(shared, component_id, position);
}

/// Synchronous seek that returns true if seek was successful, false otherwise.
/// This is used by the FFI seek callback to report actual success/failure.
pub fn seek_stream_session_sync_shared(
    shared: &Arc<VideoContextSharedState>,
    component_id: &str,
    position: f64,
) -> bool {
    // Validate position early
    if !position.is_finite() || position < 0.0 {
        warn!(
            "stream session seek rejected: invalid position={} component_id={}",
            position, component_id
        );
        return false;
    }

    // Lock session, perform seek, then release lock before decoder operations
    // to avoid lock ordering issues (session -> decoder vs decoder -> session)
    let seek_result = {
        let guard = match shared.stream_session.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        match guard.as_ref() {
            Some(session) => session.seek(position),
            None => return false,
        }
    };
    // Session lock is now released

    if let Err(err) = seek_result {
        warn!(
            "stream session seek failed component_id={} position={} err={}",
            component_id, position, err
        );
        return false;
    }

    // Seek succeeded - update state
    shared
        .last_stream_position_ms
        .store((position * 1000.0).round() as u64, Ordering::Relaxed);

    // Flush decoder (now safe - no session lock held)
    if let Ok(decoder_lock) = shared.stream_decoder.lock() {
        if let Some(decoder) = decoder_lock.as_ref() {
            let _ = decoder.flush();
        }
    }

    // Stream seek successful. On Harmony, the stream decoder doesn't emit seekDone
    // (only AVPlayer does), so we must emit it manually to ensure UI sync.
    #[cfg(target_env = "ohos")]
    {
        let position_ms = (position * 1000.0) as i64;
        lingxia_platform::harmony::video_player::notify_video_player_event(
            component_id,
            "seekDone",
            Some(&position_ms.to_string()),
        );
    }

    true
}

pub fn pause_or_resume_stream_session_shared(
    shared: &Arc<VideoContextSharedState>,
    resume: bool,
    component_id: &str,
) {
    let mut guard = match shared.stream_session.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    if let Some(session) = guard.as_mut() {
        let result = if resume {
            session.resume()
        } else {
            session.pause()
        };
        if let Err(err) = result {
            warn!(
                "{} stream session failed component_id={} err={}",
                if resume { "resume" } else { "pause" },
                component_id,
                err
            );
        } else if resume {
            // Resume ok. UI state is driven by platform playback events (`waiting`/`playing`).
        }
    }
}

fn resume_or_seek_stream_session_shared(shared: &Arc<VideoContextSharedState>, component_id: &str) {
    let position_ms = shared.last_stream_position_ms.swap(0, Ordering::Relaxed);
    if position_ms > 0 {
        let position = (position_ms as f64) / 1000.0;
        seek_stream_session_async_shared(shared, component_id, position);
        return;
    }
    pause_or_resume_stream_session_shared(shared, true, component_id);
}
