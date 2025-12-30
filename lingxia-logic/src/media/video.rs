use lingxia_platform::{
    Platform, VideoPlayerCommand, VideoPlayerHandle, VideoPlayerManager, VideoStreamDecoderHandle,
    VideoStreamDecoderManager,
};
use log::{info, warn};
use lxapp::stream_source::{FrameSink, StreamSession, get_stream_provider};
use lxapp::{LxApp, lx};
use rong::{
    FromJSObj, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError, js_class, js_export,
    js_method,
};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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
    #[rename = "type"]
    stream_type: String,
    params: Option<JSObject>,
}

fn parse_stream_params(params: Option<JSObject>) -> JSResult<Value> {
    let Some(obj) = params else {
        return Ok(Value::Null);
    };
    let json = obj.json_stringify()?;
    serde_json::from_str(&json)
        .map_err(|e| RongJSError::Error(format!("params must be JSON-compatible: {}", e)))
}

#[js_export]
pub struct JSVideoContext {
    component_id: String,
    player_handle: Arc<dyn VideoPlayerHandle>,
    runtime: Arc<Platform>,
    stream_session: Arc<Mutex<Option<Box<dyn StreamSession>>>>,
    stream_decoder: Arc<Mutex<Option<Arc<dyn VideoStreamDecoderHandle>>>>,
    stream_epoch: Arc<AtomicU64>,
    last_stream_source: Arc<Mutex<Option<StreamSourceState>>>,
}

#[derive(Debug, Clone)]
struct StreamSourceState {
    provider_type: String,
    params: Value,
}

impl JSVideoContext {
    pub fn create(ctx: &JSContext, component_id: String) -> JSResult<Self> {
        if component_id.trim().is_empty() {
            return Err(RongJSError::Error("componentId required".into()));
        }

        let lxapp = LxApp::from_ctx(ctx)?;
        let runtime = lxapp.runtime.clone();
        let handle = runtime
            .bind_player(&component_id)
            .map_err(|e| RongJSError::Error(e.to_string()))?;

        Ok(Self {
            component_id,
            player_handle: handle.into(),
            runtime,
            stream_session: Arc::new(Mutex::new(None)),
            stream_decoder: Arc::new(Mutex::new(None)),
            stream_epoch: Arc::new(AtomicU64::new(0)),
            last_stream_source: Arc::new(Mutex::new(None)),
        })
    }

    fn dispatch(&self, command: VideoPlayerCommand) -> JSResult<()> {
        self.player_handle
            .dispatch(command)
            .map_err(|e| RongJSError::Error(e.to_string()))
    }

    fn dispatch_async_if_epoch(
        &self,
        command: VideoPlayerCommand,
        expected_epoch: u64,
    ) -> JSResult<()> {
        let handle = self.player_handle.clone();
        let epoch_token = self.stream_epoch.clone();
        if epoch_token.load(Ordering::Relaxed) != expected_epoch {
            warn!(
                "Skipping async video player command due to epoch change (expected={}, current={})",
                expected_epoch,
                epoch_token.load(Ordering::Relaxed)
            );
            return Ok(());
        }
        thread::spawn(move || {
            if epoch_token.load(Ordering::Relaxed) != expected_epoch {
                warn!(
                    "Skipping async video player command due to epoch change (expected={}, current={})",
                    expected_epoch,
                    epoch_token.load(Ordering::Relaxed)
                );
                return;
            }
            if let Err(err) = handle.dispatch(command) {
                warn!("Failed to dispatch video player command: {}", err);
            }
        });
        Ok(())
    }

    fn stop_stream_session_async(&self) -> JSResult<()> {
        let mut guard = self
            .stream_session
            .lock()
            .map_err(|_| RongJSError::Error("Stream session lock poisoned".into()))?;
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

    fn abort_stream_session_async(&self) -> JSResult<()> {
        let mut guard = self
            .stream_session
            .lock()
            .map_err(|_| RongJSError::Error("Stream session lock poisoned".into()))?;
        let session = guard.take();
        drop(guard);

        if let Some(session) = session {
            thread::spawn(move || {
                if let Err(err) = session.abort() {
                    warn!("Failed to abort stream session: {}", err);
                }
            });
        }
        Ok(())
    }

    fn ensure_stream_decoder(&self) -> JSResult<Arc<dyn VideoStreamDecoderHandle>> {
        let mut guard = self
            .stream_decoder
            .lock()
            .map_err(|_| RongJSError::Error("Stream decoder lock poisoned".into()))?;
        if let Some(decoder) = guard.as_ref() {
            return Ok(decoder.clone());
        }

        let decoder = self
            .runtime
            .create_stream_decoder(&self.component_id)
            .map_err(|e| RongJSError::Error(e.to_string()))?;
        let decoder: Arc<dyn VideoStreamDecoderHandle> = decoder.into();
        *guard = Some(decoder.clone());
        Ok(decoder)
    }
}

#[js_class]
impl JSVideoContext {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(RongJSError::TypeError("Use lx.createVideoContext()".into()))
    }

    #[js_method]
    fn play(&self) -> JSResult<()> {
        self.dispatch(VideoPlayerCommand::Play)
    }

    #[js_method]
    fn pause(&self) -> JSResult<()> {
        self.dispatch(VideoPlayerCommand::Pause)
    }

    #[js_method]
    fn stop(&self) -> JSResult<()> {
        let epoch = self.stream_epoch.load(Ordering::Relaxed);
        self.stop_stream_session_async()?;
        self.dispatch_async_if_epoch(VideoPlayerCommand::Stop, epoch)
    }

    #[js_method]
    fn seek(&self, position: f64) -> JSResult<()> {
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
        if options.stream_type.trim().is_empty() {
            return Err(RongJSError::Error("type is required".into()));
        }

        let provider_type = options.stream_type;
        let provider = get_stream_provider(&provider_type).ok_or_else(|| {
            RongJSError::Error(format!("Stream provider not found: {}", provider_type))
        })?;
        let params = parse_stream_params(options.params)?;

        let force_hard = {
            let guard = self
                .last_stream_source
                .lock()
                .map_err(|_| RongJSError::Error("Stream source lock poisoned".into()))?;
            match guard.as_ref() {
                Some(prev) if prev.provider_type != provider_type => true,
                Some(prev) => provider.should_force_hard_switch(Some(&prev.params), &params),
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
        let epoch = self.stream_epoch.fetch_add(1, Ordering::Relaxed) + 1;

        let existing_decoder = {
            let guard = self
                .stream_decoder
                .lock()
                .map_err(|_| RongJSError::Error("Stream decoder lock poisoned".into()))?;
            guard.clone()
        };
        let had_existing_decoder = existing_decoder.is_some();

        let mut decoder = match existing_decoder {
            Some(decoder) => decoder,
            None => self.ensure_stream_decoder()?,
        };

        let supports_soft_reset = decoder.supports_soft_reset();
        let supports_in_place_hard_reset = decoder.supports_in_place_hard_reset();
        let reuse_decoder = supports_soft_reset && (!force_hard || supports_in_place_hard_reset);
        let hard_reset_in_place = force_hard && supports_in_place_hard_reset;
        info!(
            "setStreamSource component_id={} provider_type={} epoch={} reuse_decoder={}",
            self.component_id, &provider_type, epoch, reuse_decoder
        );

        if reuse_decoder {
            self.abort_stream_session_async()?;
            if let Err(err) = decoder.reset_stream(hard_reset_in_place) {
                warn!(
                    "setStreamSource reset_stream failed component_id={} hard_reset_in_place={} err={}",
                    self.component_id, hard_reset_in_place, err
                );

                if !hard_reset_in_place {
                    let decoder_retry = decoder.clone();
                    let epoch_token = self.stream_epoch.clone();
                    let expected_epoch = epoch;
                    thread::spawn(move || {
                        for _ in 0..20 {
                            thread::sleep(Duration::from_millis(10));
                            if epoch_token.load(Ordering::Relaxed) != expected_epoch {
                                break;
                            }
                            if decoder_retry.reset_stream(false).is_ok() {
                                break;
                            }
                        }
                    });
                }
            }
        } else {
            self.stop_stream_session_async()?;
            if had_existing_decoder {
                let old_decoder = {
                    let mut guard = self
                        .stream_decoder
                        .lock()
                        .map_err(|_| RongJSError::Error("Stream decoder lock poisoned".into()))?;
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
                    "setStreamSource failed to reset decoder component_id={} provider_type={} epoch={} err={}",
                    self.component_id, &provider_type, epoch, err
                );
            }
        }

        let sink = FrameSink::from_arc_with_epoch(decoder, self.stream_epoch.clone(), epoch);

        let session = provider.start(params.clone(), sink).map_err(|err| {
            warn!(
                "setStreamSource provider start failed component_id={} provider_type={} epoch={} err={}",
                self.component_id, &provider_type, epoch, err
            );
            RongJSError::Error(err.to_string())
        })?;

        let mut guard = self
            .stream_session
            .lock()
            .map_err(|_| RongJSError::Error("Stream session lock poisoned".into()))?;
        *guard = Some(session);

        // Update stream source after successfully starting the new session.
        let mut source_guard = self
            .last_stream_source
            .lock()
            .map_err(|_| RongJSError::Error("Stream source lock poisoned".into()))?;
        *source_guard = Some(StreamSourceState {
            provider_type,
            params,
        });

        Ok(())
    }

    #[js_method(gc_mark)]
    fn gc_mark_with<F>(&self, _mark_fn: F)
    where
        F: FnMut(&JSValue),
    {
    }
}
