use super::context::VideoContextSharedState;
use crate::i18n::{js_error_from_platform_error, js_internal_error, js_resource_not_found_error};
use lingxia_media::{FrameSink, get_stream_provider};
use lingxia_platform::traits::stream_decoder::{
    VideoStreamDecoderHandle, VideoStreamDecoderManager,
};
use lingxia_platform::traits::video_player::{VideoPlayerCommand, VideoPlayerManager};
use log::{info, warn};
use rong::RongJSError;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;

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

fn ensure_stream_decoder_shared(
    shared: &Arc<VideoContextSharedState>,
    component_id: &str,
) -> Result<Arc<dyn VideoStreamDecoderHandle>, RongJSError> {
    let mut guard = shared
        .stream_decoder
        .lock()
        .map_err(|_| js_internal_error("Stream decoder lock poisoned"))?;
    if let Some(decoder) = guard.as_ref() {
        return Ok(decoder.clone());
    }

    let decoder = shared
        .runtime
        .create_stream_decoder(component_id)
        .map_err(|e| js_error_from_platform_error(&e))?;
    let decoder: Arc<dyn VideoStreamDecoderHandle> = decoder.into();
    *guard = Some(decoder.clone());
    Ok(decoder)
}

pub(super) fn reset_decoder_soft_shared(
    shared: &Arc<VideoContextSharedState>,
    component_id: &str,
) -> Result<(), RongJSError> {
    let decoder = ensure_stream_decoder_shared(shared, component_id)?;
    decoder
        .reset_stream(false)
        .map_err(|e| js_error_from_platform_error(&e))
}

pub(super) fn reset_decoder_soft_if_present_shared(
    shared: &Arc<VideoContextSharedState>,
    _component_id: &str,
) -> Result<(), RongJSError> {
    let decoder = shared
        .stream_decoder
        .lock()
        .map_err(|_| js_internal_error("Stream decoder lock poisoned"))?
        .clone();
    let Some(decoder) = decoder else {
        return Ok(());
    };
    decoder
        .reset_stream(false)
        .map_err(|e| js_error_from_platform_error(&e))
}

pub(super) fn stop_stream_session_async_shared(shared: &Arc<VideoContextSharedState>) {
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

pub(super) fn resume_stream_session_shared(
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
        let guard = shared
            .last_stream_source
            .lock()
            .map_err(|_| js_internal_error("Stream source lock poisoned"))?;
        guard.clone()
    };

    let Some(source) = source else {
        return Ok(());
    };

    let provider = get_stream_provider(&source.provider).ok_or_else(|| {
        js_resource_not_found_error(format!("Stream provider not found: {}", source.provider))
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

    let session = provider
        .start(source.params, sink)
        .map_err(|err| js_internal_error(format!("Stream provider start failed: {}", err)))?;

    let mut guard = shared
        .stream_session
        .lock()
        .map_err(|_| js_internal_error("Stream session lock poisoned"))?;
    *guard = Some(session);

    ensure_platform_stream_play(shared, component_id);

    Ok(())
}

pub(super) fn seek_stream_session_async_shared(
    shared: &Arc<VideoContextSharedState>,
    component_id: &str,
    position: f64,
) {
    let _ = seek_stream_session_sync_shared(shared, component_id, position);
}

/// Synchronous seek that returns true if seek was successful, false otherwise.
/// This is used by the FFI seek callback to report actual success/failure.
pub(super) fn seek_stream_session_sync_shared(
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

pub(super) fn pause_or_resume_stream_session_shared(
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

pub(super) fn resume_or_seek_stream_session_shared(
    shared: &Arc<VideoContextSharedState>,
    component_id: &str,
) {
    let position_ms = shared.last_stream_position_ms.swap(0, Ordering::Relaxed);
    if position_ms > 0 {
        let position = (position_ms as f64) / 1000.0;
        seek_stream_session_async_shared(shared, component_id, position);
        return;
    }
    pause_or_resume_stream_session_shared(shared, true, component_id);
}
