use super::context::VideoContextSharedState;
use super::stream::{
    pause_or_resume_stream_session_shared, reset_decoder_soft_if_present_shared,
    reset_decoder_soft_shared, resume_or_seek_stream_session_shared, resume_stream_session_shared,
    seek_stream_session_async_shared, stop_stream_session_async_shared,
};
use lingxia_messaging::remove_callback;
use log::{debug, warn};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub(super) fn handle_player_event(
    shared: &Arc<VideoContextSharedState>,
    component_id: &str,
    payload: &str,
) {
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
