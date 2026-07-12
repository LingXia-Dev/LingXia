use super::context::{JSVideoContext, StreamSourceState};
use super::stream::{
    pause_or_resume_stream_session_shared, reset_decoder_soft_if_present_shared,
    reset_decoder_soft_shared, resume_or_seek_stream_session_shared, resume_stream_session_shared,
    seek_stream_session_async_shared, stop_stream_session_async_shared,
};
use crate::i18n::{
    js_error_from_platform_error, js_internal_error, js_invalid_parameter_error,
    js_resource_not_found_error,
};
use lingxia_media::get_stream_provider;
use lingxia_platform::traits::stream_decoder::{
    VideoStreamDecoderHandle, VideoStreamDecoderManager,
};
use lingxia_platform::traits::video_player::VideoPlayerCommand;
use log::{info, warn};
use rong::{FromJSObject, JSObject, JSResult, JSValue, js_class, js_method};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

#[derive(FromJSObject)]
struct JSStreamSourceOptions {
    #[js_name = "provider"]
    provider: String,
    #[js_name = "isLive"]
    is_live: bool,
    duration: Option<f64>,
    params: Option<JSObject>,
}

fn parse_stream_params(params: Option<JSObject>) -> JSResult<Value> {
    let Some(obj) = params else {
        return Ok(Value::Null);
    };
    let json = obj.to_json_string()?;
    serde_json::from_str(&json)
        .map_err(|e| js_invalid_parameter_error(format!("params must be JSON-compatible: {}", e)))
}

impl JSVideoContext {
    fn dispatch(&self, command: VideoPlayerCommand) -> JSResult<()> {
        self.player_handle
            .dispatch(command)
            .map_err(|e| js_error_from_platform_error(&e))
    }

    fn stop_stream_session_async(&self) -> JSResult<()> {
        let mut guard = self
            .shared
            .stream_session
            .lock()
            .map_err(|_| js_internal_error("Stream session lock poisoned"))?;
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
        let mut guard = self
            .shared
            .stream_decoder
            .lock()
            .map_err(|_| js_internal_error("Stream decoder lock poisoned"))?;
        if let Some(decoder) = guard.as_ref() {
            return Ok(decoder.clone());
        }

        let decoder = self
            .runtime
            .create_stream_decoder(&self.component_id)
            .map_err(|e| js_error_from_platform_error(&e))?;
        let decoder: Arc<dyn VideoStreamDecoderHandle> = decoder.into();
        *guard = Some(decoder.clone());
        Ok(decoder)
    }

    fn has_stream_source(&self) -> JSResult<bool> {
        let guard = self
            .shared
            .last_stream_source
            .lock()
            .map_err(|_| js_internal_error("Stream source lock poisoned"))?;
        Ok(guard.is_some())
    }

    fn has_stream_session(&self) -> JSResult<bool> {
        let guard = self
            .shared
            .stream_session
            .lock()
            .map_err(|_| js_internal_error("Stream session lock poisoned"))?;
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
        Err(js_invalid_parameter_error("Use lx.createVideoContext()"))
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
            return Err(js_invalid_parameter_error("provider is required"));
        }

        let provider = options.provider;
        let provider_impl = get_stream_provider(&provider).ok_or_else(|| {
            js_resource_not_found_error(format!("Stream provider not found: {}", provider))
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
            let guard = self
                .shared
                .last_stream_source
                .lock()
                .map_err(|_| js_internal_error("Stream source lock poisoned"))?;
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
            let guard = self
                .shared
                .stream_decoder
                .lock()
                .map_err(|_| js_internal_error("Stream decoder lock poisoned"))?;
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
                    let mut guard = self
                        .shared
                        .stream_decoder
                        .lock()
                        .map_err(|_| js_internal_error("Stream decoder lock poisoned"))?;
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
        let mut source_guard = self
            .shared
            .last_stream_source
            .lock()
            .map_err(|_| js_internal_error("Stream source lock poisoned"))?;
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
                if should_autostart
                    && let Err(err) = resume_stream_session_shared(&shared, &component_id)
                {
                    warn!(
                        "setStreamSource autostart failed component_id={} err={}",
                        component_id, err
                    );
                }
            });
        } else {
            // If the player is already in a playing state (e.g. switching cameras while playing),
            // start pulling the new stream immediately; otherwise wait for the platform play intent.
            let should_autostart = (self.shared.play_requested.load(Ordering::Relaxed)
                || self.shared.platform_playing.load(Ordering::Relaxed))
                && !self.shared.stream_paused.load(Ordering::Relaxed);
            if should_autostart && let Err(err) = self.resume_stream_session() {
                warn!(
                    "setStreamSource autostart failed component_id={} err={}",
                    self.component_id, err
                );
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
