//! HarmonyOS native video player (OH_AVPlayer C API)

use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::{
    AudioCodec, AudioFrame, AudioStreamConfig, VideoFrame, VideoPlayerCommand, VideoPlayerHandle,
    VideoPlayerHandleImpl, VideoPlayerManager, VideoStreamConfig, VideoStreamDecoderHandle,
    VideoStreamDecoderManager,
};
use core::ffi::{c_char, c_void};
use std::collections::{HashMap, VecDeque};
use std::ffi::CString;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_QUEUED_VIDEO_FRAMES: usize = 120;
const MAX_QUEUED_AUDIO_FRAMES: usize = 240;

const AV_ERR_OK: i32 = 0;
const AV_ERR_INVALID_STATE: i32 = 8;
const AVCODEC_BUFFER_FLAGS_CODEC_DATA: u32 = 1 << 3;
const AVCODEC_BUFFER_FLAGS_SYNC_FRAME: u32 = 1 << 1;
const AV_PIXEL_FORMAT_SURFACE_FORMAT: i32 = 4;
const AUDIOSTREAM_SUCCESS: i32 = 0;
const AUDIOSTREAM_TYPE_RENDERER: i32 = 1;
const AUDIOSTREAM_SAMPLE_S16LE: i32 = 1;
const AUDIOSTREAM_ENCODING_TYPE_RAW: i32 = 0;
const AUDIO_DATA_CALLBACK_RESULT_VALID: i32 = 0;
const AUDIO_CODEC_SAMPLE_FORMAT_S16LE: i32 = 1;
const AAC_IS_ADTS_FALSE: i32 = 0;
const AAC_IS_ADTS_TRUE: i32 = 1;
const AUDIOSTREAM_USAGE_MOVIE: i32 = 10;

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn looks_like_annexb(data: &[u8]) -> bool {
    data.starts_with(&[0, 0, 0, 1]) || data.starts_with(&[0, 0, 1])
}

pub fn get_stream_decoder_position_ms(component_id: &str) -> Option<i32> {
    let decoder = lookup_stream_decoder(component_id)?;
    let last_pts_us = decoder.last_video_pts.load(Ordering::Acquire);
    let base_pts_us = decoder.first_video_pts.load(Ordering::Acquire);
    let rel_pts_us = if base_pts_us > 0 {
        last_pts_us.saturating_sub(base_pts_us)
    } else {
        last_pts_us
    };
    if rel_pts_us <= 0 {
        let last_output_ms = decoder.last_video_output_ms.load(Ordering::Acquire);
        let first_output_ms = decoder.first_video_output_ms.load(Ordering::Acquire);
        if last_output_ms > 0 && first_output_ms > 0 && last_output_ms >= first_output_ms {
            let elapsed_ms = last_output_ms - first_output_ms;
            return Some((elapsed_ms.clamp(0, i32::MAX as i64)) as i32);
        }
        let started_at_ms = decoder.video_started_at_ms.load(Ordering::Acquire);
        if last_output_ms > 0 && started_at_ms > 0 && last_output_ms >= started_at_ms {
            let elapsed_ms = last_output_ms - started_at_ms;
            return Some((elapsed_ms.clamp(0, i32::MAX as i64)) as i32);
        }
        let last_enqueue_ms = decoder.last_video_enqueue_ms.load(Ordering::Acquire);
        if last_enqueue_ms > 0 && started_at_ms > 0 && last_enqueue_ms >= started_at_ms {
            let elapsed_ms = last_enqueue_ms - started_at_ms;
            return Some((elapsed_ms.clamp(0, i32::MAX as i64)) as i32);
        }
        return None;
    }
    Some(((rel_pts_us / 1000).clamp(0, i32::MAX as i64)) as i32)
}

/// AVPlayer info callback types (avplayer_base.h)
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AVPlayerOnInfoType {
    SeekDone = 0,
    SpeedDone = 1,
    BitrateDone = 2,
    Eos = 3,
    StateChange = 4,
    PositionUpdate = 5,
    Message = 6,
    VolumeChange = 7,
    ResolutionChange = 8,
    BufferingUpdate = 9,
    BitrateCollect = 10,
    InterruptEvent = 11,
    DurationUpdate = 12,
    IsLiveStream = 13,
    TrackChange = 14,
    TrackInfoUpdate = 15,
    SubtitleUpdate = 16,
    AudioOutputDeviceChange = 17,
    PlaybackRateDone = 18,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AVPlayerState {
    Idle = 0,
    Initialized = 1,
    Prepared = 2,
    Playing = 3,
    Paused = 4,
    Stopped = 5,
    Completed = 6,
    Released = 7,
    Error = 8,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum AVPlayerSeekMode {
    NextSync = 0,
    PreviousSync = 1,
    Closest = 2,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum AVPlaybackSpeed {
    Speed0_75X = 0,
    Speed1_00X = 1,
    Speed1_25X = 2,
    Speed1_75X = 3,
    Speed2_00X = 4,
    Speed0_50X = 5,
    Speed1_50X = 6,
}

// Opaque FFI types
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct OH_AVPlayer {
    _private: [u8; 0],
}
#[repr(C)]
pub struct OHNativeWindow {
    _private: [u8; 0],
}
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct OH_AVFormat {
    _private: [u8; 0],
}
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct OH_AVCodec {
    _private: [u8; 0],
}
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct OH_AVBuffer {
    _private: [u8; 0],
}
#[repr(C)]
pub struct OH_AudioStreamBuilder {
    _private: [u8; 0],
}
#[repr(C)]
pub struct OH_AudioRenderer {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OH_AVCodecBufferAttr {
    pts: i64,
    size: i32,
    offset: i32,
    flags: u32,
}

struct InfoCallbackData {
    component_id: String,
    pending_play: AtomicBool,
    state_value: AtomicI32,
    buffering_status: AtomicI32,
}

fn notify_arkts(component_id: &str, event: &str, payload: Option<&str>) {
    let result = match payload {
        Some(payload) => {
            lingxia_webview::tsfn::call_arkts("videoPlayerEvent", &[component_id, event, payload])
        }
        None => lingxia_webview::tsfn::call_arkts("videoPlayerEvent", &[component_id, event]),
    };

    if let Err(e) = result {
        log::error!(
            "[VideoPlayer] Failed to notify ArkTS: component_id={}, event={}, err={:?}",
            component_id,
            event,
            e
        );
    }
}

/// Public event hook for the Harmony UI layer.
///
/// The Harmony video UI (ArkTS) relies on `videoPlayerEvent` signals (e.g. `play`) to update
/// loading indicators. Some stream playback flows resume the stream session from the logic layer
/// before the native decoder has emitted its first output callback. Expose a minimal helper so the
/// logic layer can signal UI state without reaching into private internals.
pub fn notify_video_player_event(component_id: &str, event: &str, payload: Option<&str>) {
    notify_arkts(component_id, event, payload);
}

fn notify_stream_config(component_id: &str, width: i32, height: i32) {
    if width <= 0 || height <= 0 {
        return;
    }
    let payload = format!(r#"{{"width":{},"height":{}}}"#, width, height);
    notify_arkts(component_id, "streamConfig", Some(payload.as_str()));
}

extern "C" fn on_codec_error(_codec: *mut OH_AVCodec, error_code: i32, user_data: *mut c_void) {
    if user_data.is_null() {
        return;
    }
    let wrapper = unsafe { &*(user_data as *const StreamDecoderWrapper) };
    // Always use try_lock to avoid deadlock
    match wrapper.state.try_lock() {
        Ok(state) => {
            log::error!(
                "[Harmony.StreamDecoder] codec error for {}: {}",
                state.component_id,
                error_code
            );
            let message = format!("codec error: {}", error_code);
            notify_arkts(&state.component_id, "error", Some(&message));
        }
        Err(_) => {
            log::error!(
                "[Harmony.StreamDecoder] codec error (lock unavailable): {}",
                error_code
            );
        }
    }
}

extern "C" fn on_stream_changed(
    _codec: *mut OH_AVCodec,
    format: *mut OH_AVFormat,
    user_data: *mut c_void,
) {
    if user_data.is_null() || format.is_null() {
        return;
    }
    let wrapper = unsafe { &*(user_data as *const StreamDecoderWrapper) };
    // Always use try_lock to avoid deadlock
    let Ok(state) = wrapper.state.try_lock() else {
        return;
    };
    let mut width: i32 = 0;
    let mut height: i32 = 0;
    let width_ok = unsafe { OH_AVFormat_GetIntValue(format, OH_MD_KEY_WIDTH, &mut width) };
    let height_ok = unsafe { OH_AVFormat_GetIntValue(format, OH_MD_KEY_HEIGHT, &mut height) };
    log::info!(
        "[Harmony.StreamDecoder] stream changed for {}: width={}({}), height={}({})",
        state.component_id,
        width,
        width_ok,
        height,
        height_ok
    );
    if width_ok && height_ok {
        notify_stream_config(&state.component_id, width, height);
    }
}

extern "C" fn on_need_input_buffer(
    _codec: *mut OH_AVCodec,
    index: u32,
    buffer: *mut OH_AVBuffer,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let wrapper = unsafe { &*(user_data as *const StreamDecoderWrapper) };
    if wrapper.is_destroying() {
        return;
    }
    if wrapper.paused.load(Ordering::Acquire) {
        // IMPORTANT: Return the input buffer to the codec even when paused. Some Harmony codecs
        // can wedge if we don't call `*_PushInputBuffer` for a requested buffer, and then resume
        // never produces output (leading to a stuck loading indicator after pause->play).
        if !_codec.is_null() && !buffer.is_null() {
            let pts = wrapper.last_video_pts.load(Ordering::Acquire);
            let attr = OH_AVCodecBufferAttr {
                pts,
                size: 0,
                offset: 0,
                flags: 0,
            };
            unsafe {
                let _ = OH_AVBuffer_SetBufferAttr(buffer, &attr);
                let result = OH_VideoDecoder_PushInputBuffer(_codec, index);
                if result != AV_ERR_OK {
                    log::debug!(
                        "[Harmony.StreamDecoder] paused push empty video buffer failed for {}: {}",
                        wrapper.component_id,
                        result
                    );
                }
            }
        }
        return;
    }
    if wrapper
        .logged_first_video_input
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        log::info!(
            "[Harmony.StreamDecoder] first video input buffer: index={} for {}",
            index,
            wrapper.component_id
        );
    }
    // NOTE: `buffer` is only valid for the duration of this callback. Never hold a mutex while
    // calling `OH_*` APIs, as some implementations may re-enter callbacks synchronously.
    // Harmony codecs may request input buffers in bursts. If we respond with "no data" immediately,
    // some devices appear to stall or end up showing only a still frame. Wait a short time for the
    // producer to enqueue a frame before declaring underflow.
    let mut frame = wrapper
        .video_queue
        .lock()
        .ok()
        .and_then(|mut q| q.pop_front());
    if frame.is_none() {
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(2));
            frame = wrapper
                .video_queue
                .lock()
                .ok()
                .and_then(|mut q| q.pop_front());
            if frame.is_some() {
                break;
            }
        }
    }
    let Some(frame) = frame else {
        if wrapper
            .logged_video_underflow
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            log::info!(
                "[Harmony.StreamDecoder] video input buffer requested but no pending frame for {} (index={})",
                wrapper.component_id,
                index
            );
        }
        // IMPORTANT: Return the input buffer to the codec even on underflow. Some Harmony codecs
        // stall permanently if we don't call `*_PushInputBuffer` for a requested buffer.
        // A zero-sized push releases the buffer so the codec can keep requesting later.
        if !_codec.is_null() && !buffer.is_null() {
            let pts = wrapper.last_video_pts.load(Ordering::Acquire);
            let attr = OH_AVCodecBufferAttr {
                pts,
                size: 0,
                offset: 0,
                flags: 0,
            };
            unsafe {
                let _ = OH_AVBuffer_SetBufferAttr(buffer, &attr);
                let result = OH_VideoDecoder_PushInputBuffer(_codec, index);
                if result != AV_ERR_OK {
                    log::warn!(
                        "[Harmony.StreamDecoder] underflow push empty video buffer failed for {}: {}",
                        wrapper.component_id,
                        result
                    );
                }
            }
        }
        return;
    };
    if let Err(err) = fill_input_buffer(_codec, index, buffer, frame) {
        log::warn!(
            "[Harmony.StreamDecoder] fill_input_buffer failed: {:?}",
            err
        );
    }
}

extern "C" fn on_new_output_buffer(
    codec: *mut OH_AVCodec,
    index: u32,
    buffer: *mut OH_AVBuffer,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    if codec.is_null() {
        return;
    }
    let mut output_size: i32 = -1;
    let mut output_flags: u32 = 0;
    let mut output_pts: i64 = 0;
    if !buffer.is_null() {
        let mut attr = OH_AVCodecBufferAttr {
            pts: 0,
            size: 0,
            offset: 0,
            flags: 0,
        };
        if unsafe { OH_AVBuffer_GetBufferAttr(buffer, &mut attr) } == AV_ERR_OK {
            output_size = attr.size;
            output_flags = attr.flags;
            output_pts = attr.pts;
        }
    }
    let is_codec_data = (output_flags & AVCODEC_BUFFER_FLAGS_CODEC_DATA) != 0;
    let render_result = unsafe { OH_VideoDecoder_RenderOutputBuffer(codec, index) };
    let wrapper = unsafe { &*(user_data as *const StreamDecoderWrapper) };
    if render_result == AV_ERR_OK && !is_codec_data {
        let now = now_ms();
        wrapper.last_video_output_ms.store(now, Ordering::Release);
        let _ = wrapper.first_video_output_ms.compare_exchange(
            0,
            now,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        if wrapper.video_started_at_ms.load(Ordering::Acquire) == 0 {
            wrapper.video_started_at_ms.store(now, Ordering::Release);
        }
        if output_pts > 0 {
            wrapper.last_video_pts.store(output_pts, Ordering::Release);
            let _ = wrapper.first_video_pts.compare_exchange(
                0,
                output_pts,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }
    if wrapper
        .logged_first_video_output_callback
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        log::info!(
            "[Harmony.StreamDecoder] on_new_output_buffer callback first for {}: index={} render_result={} size={} flags=0x{:x}",
            wrapper.component_id,
            index,
            render_result,
            output_size,
            output_flags
        );

        // Some devices only start showing video after a surface rebind that happens after the
        // decoder has produced output (fullscreen enter/exit does this). Refresh the surface once
        // in the background by recreating the native window and calling SetSurface again.
        if wrapper
            .video_surface_refresh_scheduled
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let component_id = wrapper.component_id.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(60));
                match refresh_stream_decoder_surface(&component_id) {
                    Ok(()) => log::info!(
                        "[Harmony.StreamDecoder] post-output surface refresh ok for {}",
                        component_id
                    ),
                    Err(err) => log::warn!(
                        "[Harmony.StreamDecoder] post-output surface refresh failed for {}: {}",
                        component_id,
                        err
                    ),
                }
            });
        }
    }
    if render_result == AV_ERR_OK && !is_codec_data && !wrapper.paused.load(Ordering::Acquire) {
        let mut should_notify_playing = false;
        if wrapper
            .video_received_frame
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            should_notify_playing = true;
        }
        if wrapper.playing_event_pending.swap(false, Ordering::AcqRel) {
            should_notify_playing = true;
        }
        if should_notify_playing {
            notify_arkts(&wrapper.component_id, "playing", None);
        }
    }
    if render_result != AV_ERR_OK {
        log::warn!(
            "[Harmony.StreamDecoder] render output buffer failed for {}: {}",
            wrapper.component_id,
            render_result
        );
    }
    // Always use try_lock to avoid deadlock. Rendering is done above without holding the lock.
    match wrapper.state.try_lock() {
        Ok(mut state) => state.on_new_output_buffer(index, render_result),
        Err(_) => {
            // State lock unavailable; we've already rendered the buffer.
        }
    };
}

extern "C" fn on_audio_need_input_buffer(
    _codec: *mut OH_AVCodec,
    index: u32,
    buffer: *mut OH_AVBuffer,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let wrapper = unsafe { &*(user_data as *const StreamDecoderWrapper) };
    if wrapper.is_destroying() {
        return;
    }
    if wrapper.paused.load(Ordering::Acquire) {
        return;
    }
    // Audio codec rejects zero-sized pushes on some devices (seen as error code 6). If we have no
    // pending AAC frame, return without pushing; the codec will request another input buffer.
    if wrapper
        .logged_first_audio_input
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        log::info!(
            "[Harmony.StreamDecoder] first audio input buffer: index={} for {}",
            index,
            wrapper.component_id
        );
    }
    // Like video: give the producer a short window to enqueue a frame to reduce startup underflow.
    let mut frame = wrapper
        .audio_queue
        .lock()
        .ok()
        .and_then(|mut q| q.pop_front());
    if frame.is_none() {
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(2));
            frame = wrapper
                .audio_queue
                .lock()
                .ok()
                .and_then(|mut q| q.pop_front());
            if frame.is_some() {
                break;
            }
        }
    }
    let Some(frame) = frame else {
        if wrapper
            .logged_audio_underflow
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            log::info!(
                "[Harmony.StreamDecoder] audio input buffer requested but no pending frame for {} (index={})",
                wrapper.component_id,
                index
            );
        }
        return;
    };
    if frame.data.is_empty() {
        return;
    }
    if let Err(err) = fill_audio_input_buffer(_codec, index, buffer, frame) {
        log::warn!(
            "[Harmony.StreamDecoder] fill_audio_input_buffer failed: {:?}",
            err
        );
    }
}

extern "C" fn on_audio_new_output_buffer(
    codec: *mut OH_AVCodec,
    index: u32,
    buffer: *mut OH_AVBuffer,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    if codec.is_null() {
        return;
    }
    let wrapper = unsafe { &*(user_data as *const StreamDecoderWrapper) };
    // Always use try_lock to avoid deadlock.
    match wrapper.state.try_lock() {
        Ok(mut state) => {
            state.on_audio_new_output_buffer(codec, index, buffer);
        }
        Err(_) => {
            // Always free the output buffer, otherwise the codec may stall after exhausting buffers.
            unsafe {
                let _ = OH_AudioCodec_FreeOutputBuffer(codec, index);
            }
        }
    }
}

extern "C" fn on_audio_render_write(
    _renderer: *mut OH_AudioRenderer,
    user_data: *mut c_void,
    audio_data: *mut c_void,
    audio_data_size: i32,
) -> i32 {
    if user_data.is_null() || audio_data.is_null() || audio_data_size <= 0 {
        return AUDIO_DATA_CALLBACK_RESULT_VALID;
    }

    let output =
        unsafe { std::slice::from_raw_parts_mut(audio_data as *mut u8, audio_data_size as usize) };
    let wrapper = unsafe { &*(user_data as *const StreamDecoderWrapper) };
    // Always use try_lock to avoid deadlock. Fill with silence if lock unavailable.
    let filled = match wrapper.state.try_lock() {
        Ok(mut state) => state.on_audio_render_write(output),
        Err(_) => 0, // Return silence if lock unavailable
    };

    if filled < output.len() {
        output[filled..].fill(0);
    }
    AUDIO_DATA_CALLBACK_RESULT_VALID
}

extern "C" fn on_info_callback(
    _player: *mut OH_AVPlayer,
    info_type: i32,
    info_body: *mut OH_AVFormat,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }

    // SAFETY: user_data is a leaked Box<InfoCallbackData> created in NativeVideoPlayer::new.
    let callback_data = unsafe { &*(user_data as *const InfoCallbackData) };
    let component_id = &callback_data.component_id;

    if info_type == AVPlayerOnInfoType::SeekDone as i32 {
        let mut seek_position: i32 = 0;

        if !info_body.is_null() {
            let key_ptr = unsafe { OH_PLAYER_SEEK_POSITION };
            if !key_ptr.is_null() {
                let got_value =
                    unsafe { OH_AVFormat_GetIntValue(info_body, key_ptr, &mut seek_position) };
                if got_value {
                    log::info!(
                        "[VideoPlayer] on_info_callback: SEEK_DONE component_id={}, position={}",
                        component_id,
                        seek_position
                    );
                } else {
                    log::warn!(
                        "[VideoPlayer] on_info_callback: SEEK_DONE but failed to get position for {}",
                        component_id
                    );
                }
            } else {
                log::warn!(
                    "[VideoPlayer] on_info_callback: SEEK_DONE but OH_PLAYER_SEEK_POSITION is null"
                );
            }
        }

        let position_str = seek_position.to_string();
        notify_arkts(component_id, "seekDone", Some(&position_str));
    } else if info_type == AVPlayerOnInfoType::BufferingUpdate as i32 {
        let mut buffering_type = 0;
        let mut buffering_value = 0;
        let mut has_value = false;
        if !info_body.is_null() {
            let key_ptr = unsafe { OH_PLAYER_BUFFERING_TYPE };
            if !key_ptr.is_null() {
                unsafe { OH_AVFormat_GetIntValue(info_body, key_ptr, &mut buffering_type) };
            }
            let value_ptr = unsafe { OH_PLAYER_BUFFERING_VALUE };
            if !value_ptr.is_null() {
                has_value =
                    unsafe { OH_AVFormat_GetIntValue(info_body, value_ptr, &mut buffering_value) };
            }
        }

        // On some devices, BufferingUpdate reports progress via BUFFERING_VALUE (0..100) instead
        // of emitting a distinct BUFFERING_END type. Support both encodings.
        // - AVPLAYER_BUFFERING_START = 1, AVPLAYER_BUFFERING_END = 2
        // - BUFFERING_VALUE: 0..99 => buffering, 100 => end
        let status_value = if buffering_type == 1 {
            Some(1)
        } else if buffering_type == 2 {
            Some(0)
        } else if has_value {
            Some(if buffering_value >= 100 { 0 } else { 1 })
        } else {
            None
        };

        if let Some(status_value) = status_value {
            let last_status = callback_data.buffering_status.load(Ordering::Acquire);
            if last_status == status_value {
                return;
            }
            callback_data
                .buffering_status
                .store(status_value, Ordering::Release);
            let status = if status_value == 1 { "1" } else { "0" };
            log::info!(
                "[VideoPlayer] on_info_callback: BUFFERING_UPDATE component_id={}, type={}, value={}({}), status={}",
                component_id,
                buffering_type,
                buffering_value,
                has_value,
                status
            );
            notify_arkts(component_id, "buffering", Some(status));
        }
    } else if info_type == AVPlayerOnInfoType::StateChange as i32 {
        let mut state_value: i32 = 0;
        if !info_body.is_null() {
            let key_ptr = unsafe { OH_PLAYER_STATE };
            if !key_ptr.is_null() {
                unsafe { OH_AVFormat_GetIntValue(info_body, key_ptr, &mut state_value) };
            }
        }

        callback_data
            .state_value
            .store(state_value, Ordering::Release);
        // Avoid taking the NativeVideoPlayer mutex here: callbacks may fire synchronously while the
        // caller holds the lock (e.g. Stop/Prepare), which would deadlock.
        if state_value == AVPlayerState::Prepared as i32
            && callback_data
                .pending_play
                .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            let _ = unsafe { OH_AVPlayer_Play(_player) };
        }

        match state_value {
            x if x == AVPlayerState::Prepared as i32 => {
                notify_arkts(component_id, "prepared", None)
            }
            x if x == AVPlayerState::Playing as i32 => notify_arkts(component_id, "playing", None),
            x if x == AVPlayerState::Paused as i32 => notify_arkts(component_id, "pause", None),
            x if x == AVPlayerState::Stopped as i32 => notify_arkts(component_id, "stop", None),
            _ => {}
        }
    }
}

#[allow(dead_code)]
pub struct NativeVideoPlayer {
    player: *mut OH_AVPlayer,
    component_id: String,
    window: *mut OHNativeWindow,
    state: AVPlayerState,
    volume: f32,
    is_looping: bool,
    info_callback_data: *mut InfoCallbackData,
    pending_play: bool,
    source_set: bool,
}

// SAFETY: Player accessed on main thread, protected by mutex
unsafe impl Send for NativeVideoPlayer {}
unsafe impl Sync for NativeVideoPlayer {}

impl NativeVideoPlayer {
    pub fn new(component_id: &str, _callback_id: u64) -> Result<Self, PlatformError> {
        let player = unsafe { OH_AVPlayer_Create() };
        if player.is_null() {
            return Err(PlatformError::Platform(
                "Failed to create OH_AVPlayer".to_string(),
            ));
        }

        let callback_data = Box::new(InfoCallbackData {
            component_id: component_id.to_string(),
            pending_play: AtomicBool::new(false),
            state_value: AtomicI32::new(AVPlayerState::Idle as i32),
            buffering_status: AtomicI32::new(-1),
        });
        let callback_data_ptr = Box::into_raw(callback_data) as *mut c_void;
        let result = unsafe {
            OH_AVPlayer_SetOnInfoCallback(player, Some(on_info_callback), callback_data_ptr)
        };
        if result != AV_ERR_OK {
            log::warn!(
                "[VideoPlayer] Failed to set info callback for {}: {}",
                component_id,
                result
            );
            // Free the leaked Box to prevent memory leak
            unsafe {
                Box::from_raw(callback_data_ptr as *mut InfoCallbackData);
            }
            return Ok(Self {
                player,
                component_id: component_id.to_string(),
                window: ptr::null_mut(),
                state: AVPlayerState::Idle,
                volume: 1.0,
                is_looping: false,
                info_callback_data: ptr::null_mut(),
                pending_play: false,
                source_set: false,
            });
        }

        Ok(Self {
            player,
            component_id: component_id.to_string(),
            window: ptr::null_mut(),
            state: AVPlayerState::Idle,
            volume: 1.0,
            is_looping: false,
            info_callback_data: callback_data_ptr as *mut InfoCallbackData,
            pending_play: false,
            source_set: false,
        })
    }

    fn callback_state(&self) -> Option<AVPlayerState> {
        let ptr = self.info_callback_data;
        if ptr.is_null() {
            return None;
        }
        let state_value = unsafe { (*ptr).state_value.load(Ordering::Acquire) };
        Some(match state_value {
            0 => AVPlayerState::Idle,
            1 => AVPlayerState::Initialized,
            2 => AVPlayerState::Prepared,
            3 => AVPlayerState::Playing,
            4 => AVPlayerState::Paused,
            5 => AVPlayerState::Stopped,
            6 => AVPlayerState::Completed,
            7 => AVPlayerState::Released,
            8 => AVPlayerState::Error,
            _ => AVPlayerState::Idle,
        })
    }

    fn current_state(&self) -> AVPlayerState {
        self.callback_state().unwrap_or(self.state)
    }

    pub fn set_source(&mut self, source: &str) -> Result<(), PlatformError> {
        let result = if source.starts_with("http://") || source.starts_with("https://") {
            self.set_url_source(source)
        } else if source.starts_with("file://") {
            self.set_file_source(&source[7..])
        } else if source.starts_with("fd://") {
            self.set_url_source(source)
        } else if source.starts_with("/") {
            self.set_file_source(source)
        } else {
            self.set_url_source(source)
        };
        if result.is_ok() {
            self.source_set = true;
        }
        result
    }

    fn set_url_source(&mut self, url: &str) -> Result<(), PlatformError> {
        let c_url = CString::new(url)
            .map_err(|_| PlatformError::Platform("URL contains invalid characters".to_string()))?;
        check_av_result(
            unsafe { OH_AVPlayer_SetURLSource(self.player, c_url.as_ptr()) },
            "OH_AVPlayer_SetURLSource",
        )
    }

    fn set_file_source(&mut self, path: &str) -> Result<(), PlatformError> {
        let c_path = CString::new(path)
            .map_err(|_| PlatformError::Platform("Path contains invalid characters".to_string()))?;

        let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY) };
        if fd < 0 {
            return Err(PlatformError::Platform(format!(
                "Failed to open file: {}",
                path
            )));
        }

        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(fd, &mut stat) } < 0 {
            unsafe { libc::close(fd) };
            return Err(PlatformError::Platform(format!(
                "Failed to stat file: {}",
                path
            )));
        }

        check_av_result(
            unsafe { OH_AVPlayer_SetFDSource(self.player, fd, 0, stat.st_size) },
            "OH_AVPlayer_SetFDSource",
        )
    }

    pub fn set_video_surface(&mut self, window: *mut OHNativeWindow) -> Result<(), PlatformError> {
        self.set_video_surface_internal(window)
    }

    /// Rebind surface for fullscreen transitions (direct switch preferred, Stop/Prepare fallback)
    pub fn rebind_surface_and_resume(
        &mut self,
        window: *mut OHNativeWindow,
        position_ms: i32,
        should_play: bool,
    ) -> Result<(), PlatformError> {
        log::info!(
            "[VideoPlayer] rebind_surface: pos={}, should_play={}, state={:?}",
            position_ms,
            should_play,
            self.current_state()
        );

        let direct_result = unsafe { OH_AVPlayer_SetVideoSurface(self.player, window) };

        if direct_result == AV_ERR_OK {
            log::info!("[VideoPlayer] rebind_surface: direct surface switch succeeded");
            // Update window reference
            if !self.window.is_null() && self.window != window {
                unsafe { OH_NativeWindow_DestroyNativeWindow(self.window) };
            }
            self.window = window;

            // Direct switch worked, just ensure correct playback state
            if should_play && self.current_state() != AVPlayerState::Playing {
                let play_result = unsafe { OH_AVPlayer_Play(self.player) };
                log::info!("[VideoPlayer] rebind_surface: play result={}", play_result);
                if play_result == AV_ERR_OK {
                    self.state = AVPlayerState::Playing;
                }
            } else if !should_play && self.current_state() == AVPlayerState::Playing {
                let pause_result = unsafe { OH_AVPlayer_Pause(self.player) };
                log::info!(
                    "[VideoPlayer] rebind_surface: pause result={}",
                    pause_result
                );
                if pause_result == AV_ERR_OK {
                    self.state = AVPlayerState::Paused;
                }
            }
            log::info!("[VideoPlayer] rebind_surface: done (direct)");
            return Ok(());
        }

        log::info!(
            "[VideoPlayer] rebind_surface: direct switch failed (err={}), fallback to Stop/Prepare",
            direct_result
        );

        if self.current_state() == AVPlayerState::Playing {
            unsafe { OH_AVPlayer_Pause(self.player) };
        }

        let stop_result = unsafe { OH_AVPlayer_Stop(self.player) };
        log::info!("[VideoPlayer] rebind_surface: stop={}", stop_result);
        self.state = AVPlayerState::Stopped;

        self.set_video_surface_internal(window)?;
        let prepare_result = unsafe { OH_AVPlayer_Prepare(self.player) };
        log::info!("[VideoPlayer] rebind_surface: prepare={}", prepare_result);

        // Brief yield for prepare to initialize (fallback path rarely executes)
        std::thread::sleep(std::time::Duration::from_millis(30));

        if position_ms > 0 {
            unsafe {
                OH_AVPlayer_Seek(
                    self.player,
                    position_ms,
                    AVPlayerSeekMode::PreviousSync as i32,
                )
            };
        }

        if should_play {
            let play_result = unsafe { OH_AVPlayer_Play(self.player) };
            if play_result == AV_ERR_OK {
                self.state = AVPlayerState::Playing;
            }
        } else {
            self.state = AVPlayerState::Paused;
        }

        log::info!("[VideoPlayer] rebind_surface: done (fallback)");
        Ok(())
    }

    pub fn prepare(&mut self) -> Result<(), PlatformError> {
        check_av_result(
            unsafe { OH_AVPlayer_Prepare(self.player) },
            "OH_AVPlayer_Prepare",
        )
    }

    pub fn play(&mut self) -> Result<(), PlatformError> {
        // Handle states that require prepare before playing
        match self.current_state() {
            AVPlayerState::Stopped | AVPlayerState::Idle | AVPlayerState::Initialized => {
                // For these states, we need to prepare first
                // Prepare is async - it will trigger a state change callback when done
                // For now, just initiate prepare and return success
                // The actual play will happen when state becomes Prepared (via callback or next play call)
                log::info!(
                    "[VideoPlayer] Preparing player before play (current state: {:?})",
                    self.current_state()
                );
                self.pending_play = true;
                if !self.info_callback_data.is_null() {
                    unsafe {
                        (*self.info_callback_data)
                            .pending_play
                            .store(true, Ordering::Release);
                    }
                }
                let result = check_av_result(
                    unsafe { OH_AVPlayer_Prepare(self.player) },
                    "OH_AVPlayer_Prepare",
                );
                if result.is_err() {
                    self.pending_play = false;
                    if !self.info_callback_data.is_null() {
                        unsafe {
                            (*self.info_callback_data)
                                .pending_play
                                .store(false, Ordering::Release);
                        }
                    }
                }
                return result;
            }
            AVPlayerState::Prepared | AVPlayerState::Paused | AVPlayerState::Completed => {
                // These states can transition to Playing directly
                self.pending_play = false;
                if !self.info_callback_data.is_null() {
                    unsafe {
                        (*self.info_callback_data)
                            .pending_play
                            .store(false, Ordering::Release);
                    }
                }
                let result =
                    check_av_result(unsafe { OH_AVPlayer_Play(self.player) }, "OH_AVPlayer_Play");
                // Don't manually set state - let the callback do it
                return result;
            }
            AVPlayerState::Playing => {
                self.pending_play = false;
                if !self.info_callback_data.is_null() {
                    unsafe {
                        (*self.info_callback_data)
                            .pending_play
                            .store(false, Ordering::Release);
                    }
                }
                // Already playing
                return Ok(());
            }
            _ => {
                return Err(PlatformError::Platform(format!(
                    "Cannot play from state: {:?}",
                    self.state
                )));
            }
        }
    }

    pub fn pause(&mut self) -> Result<(), PlatformError> {
        let result = check_av_result(
            unsafe { OH_AVPlayer_Pause(self.player) },
            "OH_AVPlayer_Pause",
        );
        if result.is_ok() {
            self.state = AVPlayerState::Paused;
        }
        result
    }

    pub fn stop(&mut self) -> Result<(), PlatformError> {
        let result = check_av_result(unsafe { OH_AVPlayer_Stop(self.player) }, "OH_AVPlayer_Stop");
        if result.is_ok() {
            self.state = AVPlayerState::Stopped;
        }
        result
    }

    pub fn seek(&mut self, position_ms: i32, mode: AVPlayerSeekMode) -> Result<(), PlatformError> {
        unsafe { OH_AVPlayer_Seek(self.player, position_ms, mode as i32) };
        Ok(())
    }

    pub fn set_volume(&mut self, volume: f32) -> Result<(), PlatformError> {
        self.volume = volume.clamp(0.0, 1.0);
        check_av_result(
            unsafe { OH_AVPlayer_SetVolume(self.player, self.volume, self.volume) },
            "OH_AVPlayer_SetVolume",
        )
    }

    pub fn set_looping(&mut self, looping: bool) -> Result<(), PlatformError> {
        self.is_looping = looping;
        check_av_result(
            unsafe { OH_AVPlayer_SetLooping(self.player, looping) },
            "OH_AVPlayer_SetLooping",
        )
    }

    pub fn set_speed(&mut self, speed: AVPlaybackSpeed) -> Result<(), PlatformError> {
        check_av_result(
            unsafe { OH_AVPlayer_SetPlaybackSpeed(self.player, speed as i32) },
            "OH_AVPlayer_SetPlaybackSpeed",
        )
    }

    pub fn get_current_time(&mut self) -> Result<i32, PlatformError> {
        let mut position = 0i32;
        check_av_result(
            unsafe { OH_AVPlayer_GetCurrentTime(self.player, &mut position) },
            "OH_AVPlayer_GetCurrentTime",
        )?;
        Ok(position)
    }

    pub fn get_duration(&self) -> Result<i32, PlatformError> {
        let mut duration = 0i32;
        check_av_result(
            unsafe { OH_AVPlayer_GetDuration(self.player, &mut duration) },
            "OH_AVPlayer_GetDuration",
        )?;
        Ok(duration)
    }

    pub fn is_playing(&self) -> bool {
        unsafe { OH_AVPlayer_IsPlaying(self.player) }
    }

    pub fn get_video_size(&self) -> Result<(i32, i32), PlatformError> {
        let mut width = 0i32;
        let mut height = 0i32;
        check_av_result(
            unsafe { OH_AVPlayer_GetVideoWidth(self.player, &mut width) },
            "OH_AVPlayer_GetVideoWidth",
        )?;
        check_av_result(
            unsafe { OH_AVPlayer_GetVideoHeight(self.player, &mut height) },
            "OH_AVPlayer_GetVideoHeight",
        )?;
        Ok((width, height))
    }

    pub fn release(&mut self) -> Result<(), PlatformError> {
        if !self.player.is_null() {
            unsafe { OH_AVPlayer_SetOnInfoCallback(self.player, None, ptr::null_mut()) };
            check_av_result(
                unsafe { OH_AVPlayer_Release(self.player) },
                "OH_AVPlayer_Release",
            )?;
            self.player = ptr::null_mut();
        }
        if !self.window.is_null() {
            unsafe { OH_NativeWindow_DestroyNativeWindow(self.window) };
            self.window = ptr::null_mut();
        }
        // InfoCallbackData is intentionally leaked to keep callback pointers valid even if a late
        // callback arrives after release/switch.
        self.info_callback_data = ptr::null_mut();
        Ok(())
    }

    pub fn as_ptr(&self) -> *mut OH_AVPlayer {
        self.player
    }

    fn set_video_surface_internal(
        &mut self,
        window: *mut OHNativeWindow,
    ) -> Result<(), PlatformError> {
        if !self.window.is_null() && self.window != window {
            unsafe { OH_NativeWindow_DestroyNativeWindow(self.window) };
            self.window = ptr::null_mut();
        }
        check_av_result(
            unsafe { OH_AVPlayer_SetVideoSurface(self.player, window) },
            "OH_AVPlayer_SetVideoSurface",
        )?;
        self.window = window;
        Ok(())
    }
}

impl Drop for NativeVideoPlayer {
    fn drop(&mut self) {
        if !self.player.is_null() {
            let _ = self.release();
        }
    }
}

static PLAYER_MANAGER: std::sync::OnceLock<RwLock<HashMap<String, Arc<Mutex<NativeVideoPlayer>>>>> =
    std::sync::OnceLock::new();

fn get_player_manager() -> &'static RwLock<HashMap<String, Arc<Mutex<NativeVideoPlayer>>>> {
    PLAYER_MANAGER.get_or_init(|| RwLock::new(HashMap::new()))
}

static SURFACE_REGISTRY: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn get_surface_registry() -> &'static Mutex<HashMap<String, String>> {
    SURFACE_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn store_surface_id(component_id: &str, surface_id: &str) {
    if let Ok(mut guard) = get_surface_registry().lock() {
        guard.insert(component_id.to_string(), surface_id.to_string());
    }
}

fn lookup_surface_id(component_id: &str) -> Option<String> {
    let guard = get_surface_registry().lock().ok()?;
    guard.get(component_id).cloned()
}

fn remove_surface_id(component_id: &str) {
    if let Ok(mut guard) = get_surface_registry().lock() {
        guard.remove(component_id);
    }
}

pub fn store_surface_id_only(component_id: &str, surface_id: &str) {
    store_surface_id(component_id, surface_id);
}

pub fn clear_surface_id(component_id: &str) {
    remove_surface_id(component_id);
}

static VIDEO_CALLBACK_REGISTRY: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();

fn get_video_callback_registry() -> &'static Mutex<HashMap<String, u64>> {
    VIDEO_CALLBACK_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn store_video_callback_id(component_id: &str, callback_id: u64) {
    if let Ok(mut guard) = get_video_callback_registry().lock() {
        guard.insert(component_id.to_string(), callback_id);
    }
}

fn lookup_video_callback_id(component_id: &str) -> Option<u64> {
    let guard = get_video_callback_registry().lock().ok()?;
    guard.get(component_id).copied()
}

static STREAM_DECODER_REGISTRY: OnceLock<Mutex<HashMap<String, Arc<StreamDecoderWrapper>>>> =
    OnceLock::new();

fn get_stream_decoder_registry() -> &'static Mutex<HashMap<String, Arc<StreamDecoderWrapper>>> {
    STREAM_DECODER_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

static PENDING_STREAM_PAUSED: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();

fn get_pending_stream_paused() -> &'static Mutex<HashMap<String, bool>> {
    PENDING_STREAM_PAUSED.get_or_init(|| Mutex::new(HashMap::new()))
}

fn set_pending_stream_paused(component_id: &str, paused: bool) {
    if let Ok(mut guard) = get_pending_stream_paused().lock() {
        guard.insert(component_id.to_string(), paused);
    }
}

fn take_pending_stream_paused(component_id: &str) -> Option<bool> {
    let mut guard = get_pending_stream_paused().lock().ok()?;
    guard.remove(component_id)
}

fn register_stream_decoder(component_id: &str, wrapper: Arc<StreamDecoderWrapper>) {
    if let Ok(mut guard) = get_stream_decoder_registry().lock() {
        guard.insert(component_id.to_string(), wrapper);
    }
}

fn lookup_stream_decoder(component_id: &str) -> Option<Arc<StreamDecoderWrapper>> {
    let guard = get_stream_decoder_registry().lock().ok()?;
    guard.get(component_id).cloned()
}

fn remove_stream_decoder_if_current(
    component_id: &str,
    wrapper: &Arc<StreamDecoderWrapper>,
) -> bool {
    if let Ok(mut guard) = get_stream_decoder_registry().lock() {
        if let Some(current) = guard.get(component_id) {
            if Arc::ptr_eq(current, wrapper) {
                guard.remove(component_id);
                return true;
            }
        }
    }
    false
}

fn release_player_for_stream(component_id: &str, surface_id: &str) {
    if get_player(component_id).is_none() {
        return;
    }

    log::info!(
        "[Harmony.StreamDecoder] releasing native player before stream decode: {}",
        component_id
    );
    if let Err(err) = destroy_player(component_id) {
        log::warn!(
            "[Harmony.StreamDecoder] failed to destroy native player for {}: {}",
            component_id,
            err
        );
    }
    store_surface_id(component_id, surface_id);
}

pub fn has_stream_decoder(component_id: &str) -> bool {
    lookup_stream_decoder(component_id).is_some()
}

pub fn set_stream_volume(component_id: &str, volume: f32) -> Result<(), PlatformError> {
    let wrapper = lookup_stream_decoder(component_id).ok_or_else(|| {
        PlatformError::Platform(format!("Stream decoder not found: {}", component_id))
    })?;
    if let Ok(mut guard) = wrapper.state.lock() {
        guard.set_volume(volume)
    } else {
        Err(PlatformError::Platform(
            "Stream decoder lock poisoned".to_string(),
        ))
    }
}

struct QueuedFrame {
    data: Vec<u8>,
    pts_us: i64,
    flags: u32,
}

/// Wrapper that holds both the destruction flag and the decoder state.
/// The destroying flag is checked by callbacks BEFORE acquiring the lock to avoid deadlock.
struct StreamDecoderWrapper {
    component_id: String,
    /// Counter > 0 when destruction is in progress. Callbacks check this first.
    destroying: AtomicUsize,
    paused: AtomicBool,
    /// When true, treat Avcc input as AnnexB by converting frames (fallback for devices that
    /// don't decode length-prefixed samples correctly).
    video_force_annexb: AtomicBool,
    /// Set once we see a non-empty video output buffer (used to gate "play" and fallback).
    video_received_frame: AtomicBool,
    /// When true, emit a `playing` event on the next rendered video frame.
    playing_event_pending: AtomicBool,
    /// Wall-clock ms when we started the video decoder (used to detect "no-output" starts).
    video_started_at_ms: AtomicI64,
    /// Ensures we only schedule one background surface refresh per start/reset cycle.
    video_surface_refresh_scheduled: AtomicBool,
    logged_first_video_input: AtomicBool,
    logged_first_audio_input: AtomicBool,
    logged_drop_video_paused: AtomicBool,
    logged_drop_audio_paused: AtomicBool,
    logged_video_underflow: AtomicBool,
    logged_audio_underflow: AtomicBool,
    logged_first_video_output_callback: AtomicBool,
    /// Base video timestamp (microseconds) used to derive a relative playback position.
    /// Stored from the first enqueued video frame after a reset; 0 means "not set".
    first_video_pts: AtomicI64,
    /// Last seen video timestamp (ms). Used when releasing an empty input buffer on underflow.
    last_video_pts: AtomicI64,
    /// Wall-clock ms of the last enqueued video frame (best-effort, used for underflow recovery).
    last_video_enqueue_ms: AtomicI64,
    /// Wall-clock ms of the last enqueued audio frame (best-effort, used for underflow recovery).
    last_audio_enqueue_ms: AtomicI64,
    /// Wall-clock ms of the last rendered video output buffer (best-effort, used for stall recovery).
    last_video_output_ms: AtomicI64,
    /// Wall-clock ms of the first rendered video output buffer after reset.
    first_video_output_ms: AtomicI64,
    /// Ensures we only spawn one watchdog thread per component.
    watchdog_started: AtomicBool,
    /// Prevent repeated underflow recovery from spawning many threads.
    underflow_recovery_in_flight: AtomicBool,
    video_queue: Mutex<VecDeque<QueuedFrame>>,
    audio_queue: Mutex<VecDeque<QueuedFrame>>,
    state: Mutex<StreamDecoderState>,
}

struct DestroyingGuard<'a> {
    counter: &'a AtomicUsize,
}

impl Drop for DestroyingGuard<'_> {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Release);
    }
}

impl StreamDecoderWrapper {
    fn is_destroying(&self) -> bool {
        self.destroying.load(Ordering::Acquire) > 0
    }

    fn reset_flow_state(&self) {
        self.logged_first_video_input
            .store(false, Ordering::Release);
        self.logged_first_audio_input
            .store(false, Ordering::Release);
        self.logged_drop_video_paused
            .store(false, Ordering::Release);
        self.logged_drop_audio_paused
            .store(false, Ordering::Release);
        self.logged_video_underflow.store(false, Ordering::Release);
        self.logged_audio_underflow.store(false, Ordering::Release);
        self.logged_first_video_output_callback
            .store(false, Ordering::Release);
        self.video_received_frame.store(false, Ordering::Release);
        self.playing_event_pending.store(true, Ordering::Release);
        self.video_started_at_ms.store(0, Ordering::Release);
        self.first_video_pts.store(0, Ordering::Release);
        self.last_video_pts.store(0, Ordering::Release);
        self.first_video_output_ms.store(0, Ordering::Release);
        // Treat the decoder as "no upstream yet" until we actually enqueue a new frame. This
        // prevents underflow recovery from firing based on stale timestamps after pause/resume or
        // soft resets (which would otherwise thrash the decoder before the first keyframe arrives).
        self.last_video_enqueue_ms.store(0, Ordering::Release);
        self.last_audio_enqueue_ms.store(0, Ordering::Release);
        self.video_surface_refresh_scheduled
            .store(false, Ordering::Release);
        if let Ok(mut q) = self.video_queue.lock() {
            q.clear();
        }
        if let Ok(mut q) = self.audio_queue.lock() {
            q.clear();
        }
    }

    fn destroying_guard(&self) -> DestroyingGuard<'_> {
        self.destroying.fetch_add(1, Ordering::Release);
        DestroyingGuard {
            counter: &self.destroying,
        }
    }
}

struct StreamDecoderState {
    component_id: String,
    paused: bool,
    started: bool,
    has_played: bool,
    waiting_notified: bool,
    need_video_keyframe: bool,
    video_codec_prefix_sent: bool,
    gate_audio_until_video: bool,
    gate_audio_deadline: Option<Instant>,
    last_surface_id: Option<String>,
    user_data: *mut c_void,
    volume: f32,
    video: Option<VideoDecoderState>,
    audio: Option<AudioDecoderState>,
    last_video_config: Option<VideoStreamConfig>,
    last_audio_config: Option<AudioStreamConfig>,
}

struct VideoDecoderState {
    codec: *mut OH_AVCodec,
    window: *mut OHNativeWindow,
    logged_output: bool,
    started: bool,
}

struct AudioDecoderState {
    codec: *mut OH_AVCodec,
    renderer: *mut OH_AudioRenderer,
    pcm_queue: VecDeque<Vec<u8>>,
    pcm_offset: usize,
    logged_output: bool,
    logged_render: bool,
    pcm_only: bool,
    started: bool,
    warmup_drop_buffers: u8,
}

unsafe impl Send for StreamDecoderState {}
unsafe impl Sync for StreamDecoderState {}

impl StreamDecoderState {
    fn new(component_id: String) -> Self {
        Self {
            component_id,
            paused: false,
            started: false,
            has_played: false,
            waiting_notified: false,
            need_video_keyframe: true,
            video_codec_prefix_sent: false,
            gate_audio_until_video: true,
            gate_audio_deadline: Some(Instant::now() + Duration::from_secs(2)),
            last_surface_id: None,
            user_data: ptr::null_mut(),
            volume: 1.0,
            video: None,
            audio: None,
            last_video_config: None,
            last_audio_config: None,
        }
    }

    fn wrapper(&self) -> Option<&StreamDecoderWrapper> {
        if self.user_data.is_null() {
            return None;
        }
        Some(unsafe { &*(self.user_data as *const StreamDecoderWrapper) })
    }

    fn reset_soft(&mut self) {
        let video_config = self.last_video_config.clone();
        let audio_config = self.last_audio_config.clone();
        let user_data = self.user_data;

        if let Some(mut video_state) = self.video.take() {
            // Reset the codec pipeline (workaround for devices that wedge after underflow).
            video_state.stop();
        }
        if let Some(mut audio_state) = self.audio.take() {
            audio_state.stop();
        }

        if let Some(video_state) = self.video.as_mut() {
            video_state.logged_output = false;
        }
        if let Some(audio_state) = self.audio.as_mut() {
            audio_state.pcm_queue.clear();
            audio_state.pcm_offset = 0;
            audio_state.logged_output = false;
            audio_state.logged_render = false;
        }
        self.has_played = false;
        self.waiting_notified = true;
        self.need_video_keyframe = true;
        self.video_codec_prefix_sent = false;
        self.gate_audio_until_video = true;
        self.gate_audio_deadline = Some(Instant::now() + Duration::from_secs(2));
        if let Some(wrapper) = self.wrapper() {
            wrapper.reset_flow_state();
        }

        // Attempt to reconfigure decoders in-place using the last known configs. If the surface is
        // not ready yet, configure_video() will just store the config and return Ok(()).
        self.started = false;
        if let Some(config) = video_config {
            let _ = self.configure_video(config, user_data);
        }
        if let Some(config) = audio_config {
            let _ = self.configure_audio(config, user_data);
        }
    }

    fn set_volume(&mut self, volume: f32) -> Result<(), PlatformError> {
        let volume = volume.clamp(0.0, 1.0);
        self.volume = volume;
        if let Some(audio_state) = self.audio.as_mut() {
            if !audio_state.renderer.is_null() {
                let result = unsafe { OH_AudioRenderer_SetVolume(audio_state.renderer, volume) };
                if result != AUDIOSTREAM_SUCCESS {
                    return Err(PlatformError::Platform(format!(
                        "OH_AudioRenderer_SetVolume failed: {}",
                        result
                    )));
                }
            }
        }
        Ok(())
    }

    fn set_paused(&mut self, paused: bool) {
        let was_paused = self.paused;
        self.paused = paused;
        if let Some(wrapper) = self.wrapper() {
            wrapper.paused.store(paused, Ordering::Release);
            if paused {
                wrapper
                    .playing_event_pending
                    .store(false, Ordering::Release);
            } else {
                wrapper.playing_event_pending.store(true, Ordering::Release);
            }
            wrapper
                .logged_drop_video_paused
                .store(false, Ordering::Release);
            wrapper
                .logged_drop_audio_paused
                .store(false, Ordering::Release);
            wrapper
                .logged_first_video_input
                .store(false, Ordering::Release);
            wrapper
                .logged_first_audio_input
                .store(false, Ordering::Release);
            wrapper
                .logged_video_underflow
                .store(false, Ordering::Release);
            wrapper
                .logged_audio_underflow
                .store(false, Ordering::Release);
            if paused {
                if let Ok(mut q) = wrapper.video_queue.lock() {
                    q.clear();
                }
                if let Ok(mut q) = wrapper.audio_queue.lock() {
                    q.clear();
                }
                // Clear last-enqueue timestamps so underflow recovery doesn't immediately soft
                // reset the decoder while the stream session is paused or restarting.
                wrapper.last_video_enqueue_ms.store(0, Ordering::Release);
                wrapper.last_audio_enqueue_ms.store(0, Ordering::Release);
            }
        }
        if paused {
            // Keep decoder state intact so playback resume can continue without requiring a
            // fresh keyframe; only drop queued frames (handled by wrapper queues).
        } else {
            // Resume: gate audio briefly to avoid "audio-only" when the surface/decoder lags, and
            // emit a fresh `playing` event once the first frame arrives after resume.
            self.gate_audio_until_video = true;
            self.gate_audio_deadline = Some(Instant::now() + Duration::from_secs(2));
            self.has_played = false;
            self.waiting_notified = true;
            if let Some(wrapper) = self.wrapper() {
                let last_output_ms = wrapper.last_video_output_ms.load(Ordering::Acquire);
                log::info!(
                    "[Harmony.StreamDecoder] set_paused resume for {} (was_paused={} last_output_ms={})",
                    self.component_id,
                    was_paused,
                    last_output_ms
                );
                // Ensure a fresh `playing` event is emitted after resume so the UI can clear any
                // loading/buffering indicator, and avoid triggering the "no-output-after-start"
                // watchdog path (resume is not a decoder start).
                wrapper.video_received_frame.store(false, Ordering::Release);
                wrapper.playing_event_pending.store(true, Ordering::Release);
                wrapper.last_video_output_ms.store(0, Ordering::Release);
                wrapper.first_video_output_ms.store(0, Ordering::Release);
                wrapper.video_started_at_ms.store(0, Ordering::Release);
                wrapper
                    .logged_first_video_output_callback
                    .store(false, Ordering::Release);
                wrapper
                    .video_surface_refresh_scheduled
                    .store(false, Ordering::Release);
            }
        }
        if let Some(audio_state) = self.audio.as_mut() {
            if paused {
                audio_state.pcm_queue.clear();
                audio_state.pcm_offset = 0;
            }
            audio_state.set_paused(paused);
        }
        if !self.started {
            return;
        }
        if paused {
            log::info!(
                "[Harmony.StreamDecoder] notify paused for {}",
                self.component_id
            );
            notify_arkts(&self.component_id, "pause", None);
        }
    }

    fn configure_video(
        &mut self,
        config: VideoStreamConfig,
        user_data: *mut c_void,
    ) -> Result<(), PlatformError> {
        self.user_data = user_data;
        let should_notify = !self.started;
        let config_is_unchanged = self
            .last_video_config
            .as_ref()
            .is_some_and(|prev| self.video.is_some() && prev == &config);
        // Persist the most recent config even if we cannot configure immediately (e.g. surface not
        // ready yet). This allows us to retry configuration when the surface arrives.
        self.last_video_config = Some(config.clone());
        // After any (re)configuration we must wait for a fresh keyframe; surface rebinds and
        // decoder resets can drop frames mid-GOP and otherwise lead to a persistent black screen.
        self.need_video_keyframe = true;
        self.video_codec_prefix_sent = false;
        self.gate_audio_until_video = true;
        self.gate_audio_deadline = Some(Instant::now() + Duration::from_secs(2));
        if !matches!(
            config.format,
            crate::traits::VideoFormat::AnnexB | crate::traits::VideoFormat::Avcc
        ) {
            return Err(PlatformError::Platform(
                "Harmony decoder expects AnnexB/Avcc format".to_string(),
            ));
        }

        let Some(surface_id) = lookup_surface_id(&self.component_id) else {
            // Surface may not exist yet when the stream provider starts (e.g. first render pass).
            // Treat this as "pending" rather than an error, otherwise stream providers may stop
            // the sink and the UI will get stuck in a permanent loading state.
            log::info!(
                "[Harmony.StreamDecoder] surface not ready yet for {}, delaying video configure",
                self.component_id
            );
            return Ok(());
        };
        let surface_changed = match self.last_surface_id.as_deref() {
            Some(prev) => prev != surface_id.as_str(),
            None => true,
        };
        if config_is_unchanged && !surface_changed {
            return Ok(());
        }
        self.last_surface_id = Some(surface_id.clone());
        if let Some(wrapper) = self.wrapper() {
            wrapper.reset_flow_state();
            wrapper.paused.store(self.paused, Ordering::Release);
        }
        if let Some(mut existing) = self.video.take() {
            log::info!(
                "[Harmony.StreamDecoder] reconfiguring video decoder for {}",
                self.component_id
            );
            let _destroying_guard = if self.user_data.is_null() {
                None
            } else {
                Some(
                    unsafe { &*(self.user_data as *const StreamDecoderWrapper) }.destroying_guard(),
                )
            };
            existing.stop();
        }
        release_player_for_stream(&self.component_id, &surface_id);
        let window = create_native_window_from_surface_id(&surface_id)?;

        let codec = unsafe { OH_VideoDecoder_CreateByMime(video_mime(config.codec)) };
        if codec.is_null() {
            unsafe { OH_NativeWindow_DestroyNativeWindow(window) };
            return Err(PlatformError::Platform(
                "Failed to create video decoder".to_string(),
            ));
        }

        let callback = OH_AVCodecCallback {
            onError: Some(on_codec_error),
            onStreamChanged: Some(on_stream_changed),
            onNeedInputBuffer: Some(on_need_input_buffer),
            onNewOutputBuffer: Some(on_new_output_buffer),
        };
        if let Err(err) = check_av_result(
            unsafe { OH_VideoDecoder_RegisterCallback(codec, callback, user_data) },
            "OH_VideoDecoder_RegisterCallback",
        ) {
            cleanup_decoder(codec, window);
            return Err(err);
        }

        let width = config.width.unwrap_or(0) as i32;
        let height = config.height.unwrap_or(0) as i32;
        if width <= 0 || height <= 0 {
            log::warn!(
                "[Harmony.StreamDecoder] width/height missing for {}, decoder may fail",
                self.component_id
            );
        }
        notify_stream_config(&self.component_id, width, height);
        let format =
            unsafe { OH_AVFormat_CreateVideoFormat(video_mime(config.codec), width, height) };
        if format.is_null() {
            unsafe {
                OH_VideoDecoder_Destroy(codec);
                OH_NativeWindow_DestroyNativeWindow(window);
            }
            return Err(PlatformError::Platform(
                "Failed to create video format".to_string(),
            ));
        }

        let pixel_format_set = unsafe {
            OH_AVFormat_SetIntValue(
                format,
                OH_MD_KEY_PIXEL_FORMAT,
                AV_PIXEL_FORMAT_SURFACE_FORMAT,
            )
        };
        if !pixel_format_set {
            log::warn!("[Harmony.StreamDecoder] Failed to set pixel format");
        }

        // Harmony codec-config: for AVCC input, prefer avcC; for AnnexB input, use start-code SPS/PPS(/VPS).
        // Some devices fail to decode AVCC samples in raw decoder mode; for those, we fall back
        // to AnnexB by converting frames and providing AnnexB CSD.
        let force_annexb = self
            .wrapper()
            .is_some_and(|wrapper| wrapper.video_force_annexb.load(Ordering::Acquire));
        let (codec_config_kind, codec_config) = if force_annexb {
            ("annexb_csd(forced)", build_codec_config(&config))
        } else if matches!(config.format, crate::traits::VideoFormat::Avcc)
            && matches!(config.codec, crate::traits::VideoCodec::H264)
        {
            ("avcC", build_avcc_config(&config))
        } else {
            ("annexb_csd", build_codec_config(&config))
        };
        if codec_config.is_empty() {
            log::warn!(
                "[Harmony.StreamDecoder] codec config missing for {} (sps/pps/vps empty?)",
                self.component_id
            );
        }
        let config_set = if !codec_config.is_empty() {
            unsafe {
                OH_AVFormat_SetBuffer(
                    format,
                    OH_MD_KEY_CODEC_CONFIG,
                    codec_config.as_ptr(),
                    codec_config.len(),
                )
            }
        } else {
            false
        };
        log::info!(
            "[Harmony.StreamDecoder] codec config set: kind={} format={:?} codec={:?} len={} set={}",
            codec_config_kind,
            config.format,
            config.codec,
            codec_config.len(),
            config_set
        );

        let configure_result = check_av_result(
            unsafe { OH_VideoDecoder_Configure(codec, format) },
            "OH_VideoDecoder_Configure",
        );
        unsafe { OH_AVFormat_Destroy(format) };
        if let Err(err) = configure_result {
            cleanup_decoder(codec, window);
            return Err(err);
        }

        if let Err(err) = set_decoder_surface_with_retry(codec, window, &self.component_id) {
            cleanup_decoder(codec, window);
            return Err(err);
        }

        if let Err(err) = check_av_result(
            unsafe { OH_VideoDecoder_Prepare(codec) },
            "OH_VideoDecoder_Prepare",
        ) {
            cleanup_decoder(codec, window);
            return Err(err);
        }

        let video_state = VideoDecoderState {
            codec,
            window,
            logged_output: false,
            started: false,
        };

        self.video = Some(video_state);
        self.started = true;
        self.has_played = false;
        if should_notify {
            notify_arkts(&self.component_id, "prepared", None);
        }
        Ok(())
    }

    fn configure_audio(
        &mut self,
        config: AudioStreamConfig,
        user_data: *mut c_void,
    ) -> Result<(), PlatformError> {
        self.user_data = user_data;
        if let Some(prev) = self.last_audio_config.as_ref()
            && self.audio.is_some()
            && prev == &config
        {
            return Ok(());
        }
        if let Some(wrapper) = self.wrapper() {
            wrapper
                .logged_first_audio_input
                .store(false, Ordering::Release);
            wrapper
                .logged_audio_underflow
                .store(false, Ordering::Release);
            if let Ok(mut q) = wrapper.audio_queue.lock() {
                q.clear();
            }
            wrapper.paused.store(self.paused, Ordering::Release);
        }
        if let Some(mut existing) = self.audio.take() {
            log::info!(
                "[Harmony.StreamDecoder] reconfiguring audio decoder for {}",
                self.component_id
            );
            let _destroying_guard = if self.user_data.is_null() {
                None
            } else {
                Some(
                    unsafe { &*(self.user_data as *const StreamDecoderWrapper) }.destroying_guard(),
                )
            };
            existing.stop();
        }

        let sample_rate = match config.sample_rate {
            Some(rate) if rate > 0 => rate as i32,
            _ => {
                log::warn!(
                    "[Harmony.StreamDecoder] sample_rate missing for {}, defaulting to 44100",
                    self.component_id
                );
                44_100
            }
        };
        let channels = match config.channels {
            Some(ch) if ch > 0 => ch as i32,
            _ => {
                log::warn!(
                    "[Harmony.StreamDecoder] channels missing for {}, defaulting to 2",
                    self.component_id
                );
                2
            }
        };

        if matches!(config.codec, AudioCodec::PcmS16le) {
            log::info!(
                "[Harmony.StreamDecoder] configuring PCM audio: sample_rate={}, channels={}",
                sample_rate,
                channels
            );
            let renderer = create_audio_renderer(sample_rate, channels, user_data)?;
            if let Err(err) = check_audio_result(
                unsafe { OH_AudioRenderer_Start(renderer) },
                "OH_AudioRenderer_Start",
            ) {
                unsafe {
                    let _ = OH_AudioRenderer_Release(renderer);
                }
                return Err(err);
            }
            let audio_state = AudioDecoderState {
                codec: ptr::null_mut(),
                renderer,
                pcm_queue: VecDeque::new(),
                pcm_offset: 0,
                logged_output: false,
                logged_render: false,
                pcm_only: true,
                started: true,
                warmup_drop_buffers: 0,
            };
            let should_notify = !self.started;
            self.audio = Some(audio_state);
            self.last_audio_config = Some(config);
            self.started = true;
            self.has_played = false;
            if should_notify {
                notify_arkts(&self.component_id, "prepared", None);
            }
            let _ = self.set_volume(self.volume);
            return Ok(());
        }

        let codec = unsafe { OH_AudioCodec_CreateByMime(OH_AVCODEC_MIMETYPE_AUDIO_AAC, false) };
        if codec.is_null() {
            return Err(PlatformError::Platform(
                "Failed to create audio decoder".to_string(),
            ));
        }

        let callback = OH_AVCodecCallback {
            onError: Some(on_codec_error),
            onStreamChanged: Some(on_stream_changed),
            onNeedInputBuffer: Some(on_audio_need_input_buffer),
            onNewOutputBuffer: Some(on_audio_new_output_buffer),
        };
        if let Err(err) = check_av_result(
            unsafe { OH_AudioCodec_RegisterCallback(codec, callback, user_data) },
            "OH_AudioCodec_RegisterCallback",
        ) {
            unsafe {
                let _ = OH_AudioCodec_Destroy(codec);
            }
            return Err(err);
        }

        let format = unsafe {
            OH_AVFormat_CreateAudioFormat(OH_AVCODEC_MIMETYPE_AUDIO_AAC, sample_rate, channels)
        };
        if format.is_null() {
            unsafe {
                let _ = OH_AudioCodec_Destroy(codec);
            }
            return Err(PlatformError::Platform(
                "Failed to create audio format".to_string(),
            ));
        }

        let adts_flag = if config.aac_is_adts {
            AAC_IS_ADTS_TRUE
        } else {
            AAC_IS_ADTS_FALSE
        };
        let adts_set = unsafe { OH_AVFormat_SetIntValue(format, OH_MD_KEY_AAC_IS_ADTS, adts_flag) };
        if !adts_set {
            log::warn!("[Harmony.StreamDecoder] Failed to set AAC_IS_ADTS flag");
        }

        let sample_format_set = unsafe {
            OH_AVFormat_SetIntValue(
                format,
                OH_MD_KEY_AUDIO_SAMPLE_FORMAT,
                AUDIO_CODEC_SAMPLE_FORMAT_S16LE,
            )
        };
        if !sample_format_set {
            log::warn!("[Harmony.StreamDecoder] Failed to set audio sample format");
        }

        let codec_config = config.audio_specific_config.clone();
        if !config.aac_is_adts && !codec_config.is_empty() {
            let config_set = unsafe {
                OH_AVFormat_SetBuffer(
                    format,
                    OH_MD_KEY_CODEC_CONFIG,
                    codec_config.as_ptr(),
                    codec_config.len(),
                )
            };
            if !config_set {
                log::warn!("[Harmony.StreamDecoder] Failed to set audio codec config");
            }
        }

        let configure_result = check_av_result(
            unsafe { OH_AudioCodec_Configure(codec, format) },
            "OH_AudioCodec_Configure",
        );
        unsafe { OH_AVFormat_Destroy(format) };
        if let Err(err) = configure_result {
            unsafe {
                let _ = OH_AudioCodec_Destroy(codec);
            }
            return Err(err);
        }
        log::info!(
            "[Harmony.StreamDecoder] audio config: sample_rate={}, channels={}, adts={}, asc_len={}",
            sample_rate,
            channels,
            config.aac_is_adts,
            config.audio_specific_config.len()
        );

        if let Err(err) = check_av_result(
            unsafe { OH_AudioCodec_Prepare(codec) },
            "OH_AudioCodec_Prepare",
        ) {
            unsafe {
                let _ = OH_AudioCodec_Destroy(codec);
            }
            return Err(err);
        }

        let renderer = match create_audio_renderer(sample_rate, channels, user_data) {
            Ok(renderer) => renderer,
            Err(err) => {
                unsafe {
                    let _ = OH_AudioCodec_Destroy(codec);
                }
                return Err(err);
            }
        };
        if let Err(err) = check_audio_result(
            unsafe { OH_AudioRenderer_Start(renderer) },
            "OH_AudioRenderer_Start",
        ) {
            unsafe {
                let _ = OH_AudioRenderer_Release(renderer);
                let _ = OH_AudioCodec_Stop(codec);
                let _ = OH_AudioCodec_Destroy(codec);
            }
            return Err(err);
        }

        let audio_state = AudioDecoderState {
            codec,
            renderer,
            pcm_queue: VecDeque::new(),
            pcm_offset: 0,
            logged_output: false,
            logged_render: false,
            pcm_only: false,
            started: false,
            // Some cameras/devices emit a brief noisy burst right after decoder start or stream
            // switch. Drop the first couple of decoded PCM buffers to avoid an audible squeal.
            warmup_drop_buffers: 2,
        };

        let should_notify = !self.started;
        self.audio = Some(audio_state);
        self.last_audio_config = Some(config);
        self.started = true;
        self.has_played = false;
        if should_notify {
            notify_arkts(&self.component_id, "prepared", None);
        }
        let _ = self.set_volume(self.volume);
        Ok(())
    }

    fn enqueue_video(
        &mut self,
        frame: VideoFrame,
    ) -> Result<Option<*mut OH_AVCodec>, PlatformError> {
        if frame.data.is_empty() {
            return Ok(None);
        }
        let mut is_keyframe = frame.keyframe;
        if !is_keyframe {
            if let Some(config) = self.last_video_config.as_ref() {
                is_keyframe = detect_keyframe(config, &frame.data);
            }
        }
        let wrapper_ptr = self.user_data as *const StreamDecoderWrapper;
        if wrapper_ptr.is_null() {
            return Ok(None);
        }
        // SAFETY: user_data is an Arc<StreamDecoderWrapper> pointer stored by the decoder handle.
        let wrapper = unsafe { &*wrapper_ptr };
        if wrapper.paused.load(Ordering::Acquire) {
            if wrapper
                .logged_drop_video_paused
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                log::info!(
                    "[Harmony.StreamDecoder] dropping video frames while paused for {}",
                    self.component_id
                );
            }
            return Ok(None);
        }
        let Some(video_state) = self.video.as_mut() else {
            // Video decoder can legitimately be unconfigured if the surface isn't ready yet.
            // Drop frames until we can configure (a future surface bind will retry).
            self.need_video_keyframe = true;
            self.gate_audio_until_video = true;
            return Ok(None);
        };

        if self.need_video_keyframe {
            if !is_keyframe {
                return Ok(None);
            }
            self.need_video_keyframe = false;
            // Keep audio gated until we actually render a video frame (not just receive a keyframe),
            // otherwise camera/quality switches can briefly become "audio-only".
        }

        let mut flags = 0u32;
        if is_keyframe {
            flags |= AVCODEC_BUFFER_FLAGS_SYNC_FRAME;
        }

        let mut data = frame.data;
        let mut data_is_annexb = looks_like_annexb(&data);
        let force_annexb = wrapper.video_force_annexb.load(Ordering::Acquire);
        if force_annexb {
            if let Some(config) = self.last_video_config.as_ref() {
                if matches!(config.format, crate::traits::VideoFormat::Avcc) && !data_is_annexb {
                    let nal_length_size = config.nal_length_size.unwrap_or(4);
                    if let Some(converted) = avcc_to_annexb(nal_length_size, &data) {
                        data = converted;
                        data_is_annexb = true;
                    }
                }
                if is_keyframe && !self.video_codec_prefix_sent && data_is_annexb {
                    let prefix = build_codec_config(config);
                    if !prefix.is_empty() {
                        let mut combined = Vec::with_capacity(prefix.len() + data.len());
                        combined.extend_from_slice(&prefix);
                        combined.extend_from_slice(&data);
                        data = combined;
                    }
                    self.video_codec_prefix_sent = true;
                }
            }
        }

        let queued_len = match wrapper.video_queue.lock() {
            Ok(mut q) => {
                let now = now_ms();
                wrapper.last_video_enqueue_ms.store(now, Ordering::Release);
                if wrapper.video_started_at_ms.load(Ordering::Acquire) == 0 {
                    wrapper.video_started_at_ms.store(now, Ordering::Release);
                }
                q.push_back(QueuedFrame {
                    data,
                    // Harmony codec buffer timestamps are in microseconds. Use normalized `dts_ms`
                    // from the provider and convert to micros to keep values small and monotonic.
                    pts_us: (frame.dts_ms as i64).saturating_mul(1000),
                    flags,
                });
                let timeline_ms = if frame.pts_ms > 0 {
                    frame.pts_ms
                } else {
                    frame.dts_ms
                };
                let timeline_us = (timeline_ms as i64).saturating_mul(1000);
                wrapper.last_video_pts.store(timeline_us, Ordering::Release);
                let _ = wrapper.first_video_pts.compare_exchange(
                    0,
                    timeline_us,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
                q.len()
            }
            Err(_) => {
                return Err(PlatformError::Platform(
                    "Stream decoder video queue lock poisoned".to_string(),
                ));
            }
        };
        // Clear underflow log flag only after we rebuild a small backlog, to avoid log spam when
        // the codec requests buffers in bursts.
        if queued_len >= 8 {
            wrapper
                .logged_video_underflow
                .store(false, Ordering::Release);
        }
        if queued_len > MAX_QUEUED_VIDEO_FRAMES {
            wrapper.reset_flow_state();
            self.need_video_keyframe = true;
            self.gate_audio_until_video = true;
            self.gate_audio_deadline = Some(Instant::now() + Duration::from_secs(2));
            return Ok(None);
        }

        // Harmony codecs often request multiple input buffers immediately after `Start`. If we
        // start too early, we underflow and some devices never recover to a steady decode/output.
        // Start after a modest prebuffer; additional protections exist for bursty callbacks and
        // underflow recovery (wait-before-underflow, empty buffer release, watchdog reset).
        //
        // NOTE: Keep this relatively small to reduce "first frame" latency compared to other
        // platforms; the underflow recovery logic is designed to handle occasional jitter.
        const VIDEO_START_THRESHOLD: usize = 12;
        if !video_state.started && queued_len >= VIDEO_START_THRESHOLD {
            video_state.started = true;
            return Ok(Some(video_state.codec));
        }
        Ok(None)
    }

    fn enqueue_audio(
        &mut self,
        frame: AudioFrame,
    ) -> Result<Option<*mut OH_AVCodec>, PlatformError> {
        if frame.data.is_empty() {
            return Ok(None);
        }
        let wrapper_ptr = self.user_data as *const StreamDecoderWrapper;
        if wrapper_ptr.is_null() {
            return Ok(None);
        }
        // SAFETY: user_data is an Arc<StreamDecoderWrapper> pointer stored by the decoder handle.
        let wrapper = unsafe { &*wrapper_ptr };
        if wrapper.paused.load(Ordering::Acquire) {
            if wrapper
                .logged_drop_audio_paused
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                log::info!(
                    "[Harmony.StreamDecoder] dropping audio frames while paused for {}",
                    self.component_id
                );
            }
            return Ok(None);
        }
        if self.gate_audio_until_video {
            // Avoid "audio-only" playback when the video surface/decoder isn't ready yet (switching
            // cameras, surface rebind, etc.). Audio resumes once we actually render a video frame.
            if wrapper.video_received_frame.load(Ordering::Acquire) {
                self.gate_audio_until_video = false;
                self.gate_audio_deadline = None;
            } else if let Some(deadline) = self.gate_audio_deadline {
                if Instant::now() < deadline {
                    return Ok(None);
                }
                // If video is taking too long (unsupported codec / no keyframe), prefer audio-only
                // over permanent silence.
                self.gate_audio_until_video = false;
                self.gate_audio_deadline = None;
            } else {
                return Ok(None);
            }
        }
        let Some(audio_state) = self.audio.as_mut() else {
            // Audio decoder may not be configured yet (e.g. during initial probe). Drop until ready.
            return Ok(None);
        };

        if audio_state.pcm_only {
            audio_state.pcm_queue.push_back(frame.data);
            return Ok(None);
        }
        if audio_state.codec.is_null() {
            return Ok(None);
        }
        let queued_len = match wrapper.audio_queue.lock() {
            Ok(mut q) => {
                wrapper
                    .last_audio_enqueue_ms
                    .store(now_ms(), Ordering::Release);
                q.push_back(QueuedFrame {
                    data: frame.data,
                    pts_us: (frame.dts_ms as i64).saturating_mul(1000),
                    flags: 0,
                });
                q.len()
            }
            Err(_) => {
                return Err(PlatformError::Platform(
                    "Stream decoder audio queue lock poisoned".to_string(),
                ));
            }
        };
        if queued_len >= 4 {
            wrapper
                .logged_audio_underflow
                .store(false, Ordering::Release);
        }
        if queued_len > MAX_QUEUED_AUDIO_FRAMES {
            if let Ok(mut q) = wrapper.audio_queue.lock() {
                q.clear();
            }
            return Ok(None);
        }
        // Same rationale as video: prebuffer enough AAC frames to satisfy the initial burst of
        // input-buffer callbacks, otherwise some devices stall.
        const AUDIO_START_THRESHOLD: usize = 8;
        if !audio_state.started && queued_len >= AUDIO_START_THRESHOLD {
            audio_state.started = true;
            return Ok(Some(audio_state.codec));
        }
        Ok(None)
    }

    fn on_new_output_buffer(&mut self, index: u32, render_result: i32) {
        if let Some(video_state) = self.video.as_mut() {
            if !video_state.logged_output {
                video_state.logged_output = true;
                log::info!(
                    "[Harmony.StreamDecoder] first video output buffer: index={} for {} (result={})",
                    index,
                    self.component_id,
                    render_result
                );
            }
        }
        if render_result != AV_ERR_OK {
            log::warn!(
                "[Harmony.StreamDecoder] render output buffer failed for {}: {}",
                self.component_id,
                render_result
            );
        }
    }

    fn on_audio_new_output_buffer(
        &mut self,
        codec: *mut OH_AVCodec,
        index: u32,
        buffer: *mut OH_AVBuffer,
    ) {
        if buffer.is_null() {
            return;
        }
        let audio_state = match self.audio.as_mut() {
            Some(state) => state,
            None => {
                unsafe {
                    let _ = OH_AudioCodec_FreeOutputBuffer(codec, index);
                }
                return;
            }
        };
        if audio_state.codec.is_null() {
            unsafe {
                let _ = OH_AudioCodec_FreeOutputBuffer(codec, index);
            }
            return;
        }

        let mut attr = OH_AVCodecBufferAttr {
            pts: 0,
            size: 0,
            offset: 0,
            flags: 0,
        };
        let attr_result = check_av_result(
            unsafe { OH_AVBuffer_GetBufferAttr(buffer, &mut attr) },
            "OH_AVBuffer_GetBufferAttr",
        );
        if attr_result.is_err() {
            unsafe {
                let _ = OH_AudioCodec_FreeOutputBuffer(codec, index);
            }
            return;
        }
        if attr.size <= 0 {
            unsafe {
                let _ = OH_AudioCodec_FreeOutputBuffer(codec, index);
            }
            return;
        }
        if audio_state.warmup_drop_buffers > 0 {
            audio_state.warmup_drop_buffers = audio_state.warmup_drop_buffers.saturating_sub(1);
            unsafe {
                let _ = OH_AudioCodec_FreeOutputBuffer(codec, index);
            }
            return;
        }

        let addr = unsafe { OH_AVBuffer_GetAddr(buffer) };
        if !addr.is_null() {
            let offset = attr.offset.max(0) as usize;
            let size = attr.size as usize;
            unsafe {
                let slice = std::slice::from_raw_parts(addr.add(offset), size);
                audio_state.pcm_queue.push_back(slice.to_vec());
            }
            if !audio_state.logged_output {
                audio_state.logged_output = true;
                log::info!(
                    "[Harmony.StreamDecoder] first audio output buffer: index={} size={} for {}",
                    index,
                    size,
                    self.component_id
                );
            }
        }

        unsafe {
            let _ = OH_AudioCodec_FreeOutputBuffer(codec, index);
        }
    }

    fn on_audio_render_write(&mut self, output: &mut [u8]) -> usize {
        if let Some(audio_state) = self.audio.as_mut() {
            let written = audio_state.fill_output(output);
            if written > 0 && !audio_state.logged_render {
                audio_state.logged_render = true;
                log::info!(
                    "[Harmony.StreamDecoder] first audio render write: {} bytes for {}",
                    written,
                    self.component_id
                );
            }
            return written;
        }
        0
    }

    fn stop_internal(&mut self, notify: bool) {
        if let Some(wrapper) = self.wrapper() {
            wrapper.reset_flow_state();
            wrapper.paused.store(false, Ordering::Release);
        }
        let _destroying_guard = if self.user_data.is_null() {
            None
        } else {
            Some(unsafe { &*(self.user_data as *const StreamDecoderWrapper) }.destroying_guard())
        };
        if let Some(mut video_state) = self.video.take() {
            video_state.stop();
        }
        if let Some(mut audio_state) = self.audio.take() {
            audio_state.stop();
        }
        self.started = false;
        self.paused = false;
        self.has_played = false;
        self.last_surface_id = None;
        self.last_video_config = None;
        self.last_audio_config = None;
        self.video_codec_prefix_sent = false;
        if notify {
            notify_arkts(&self.component_id, "stop", None);
        }
    }

    fn stop_with_notify(&mut self) {
        self.stop_internal(true);
    }

    fn stop_without_notify(&mut self) {
        self.stop_internal(false);
    }
}

fn detect_keyframe(config: &VideoStreamConfig, data: &[u8]) -> bool {
    match config.format {
        crate::traits::VideoFormat::AnnexB => detect_keyframe_annexb(config.codec, data),
        crate::traits::VideoFormat::Avcc => {
            let nal_length_size = config.nal_length_size.unwrap_or(4);
            detect_keyframe_avcc(config.codec, nal_length_size, data)
        }
    }
}

fn detect_keyframe_annexb(codec: crate::traits::VideoCodec, data: &[u8]) -> bool {
    let mut i = 0usize;
    while i + 3 < data.len() {
        let (start, header_offset) = if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            (i, 3)
        } else if i + 4 < data.len()
            && data[i] == 0
            && data[i + 1] == 0
            && data[i + 2] == 0
            && data[i + 3] == 1
        {
            (i, 4)
        } else {
            i += 1;
            continue;
        };

        let header_index = start + header_offset;
        if header_index >= data.len() {
            break;
        }
        if is_keyframe_nal(codec, data[header_index]) {
            return true;
        }
        i = header_index;
    }
    false
}

fn detect_keyframe_avcc(
    codec: crate::traits::VideoCodec,
    nal_length_size: u8,
    data: &[u8],
) -> bool {
    let len_size = match nal_length_size {
        1 | 2 | 3 | 4 => nal_length_size as usize,
        _ => 4,
    };
    let mut pos = 0usize;
    while pos + len_size <= data.len() {
        let mut nal_len: usize = 0;
        for _ in 0..len_size {
            nal_len = (nal_len << 8) | data[pos] as usize;
            pos += 1;
        }
        if nal_len == 0 {
            continue;
        }
        if pos + nal_len > data.len() {
            break;
        }
        if is_keyframe_nal(codec, data[pos]) {
            return true;
        }
        pos += nal_len;
    }
    false
}

fn avcc_to_annexb(nal_length_size: u8, data: &[u8]) -> Option<Vec<u8>> {
    let len_size = match nal_length_size {
        1 | 2 | 3 | 4 => nal_length_size as usize,
        _ => 4,
    };
    let mut pos = 0usize;
    let mut out = Vec::with_capacity(data.len().saturating_add(64));
    while pos + len_size <= data.len() {
        let mut nal_len: usize = 0;
        for _ in 0..len_size {
            nal_len = (nal_len << 8) | data[pos] as usize;
            pos += 1;
        }
        if nal_len == 0 {
            continue;
        }
        if pos + nal_len > data.len() {
            return None;
        }
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(&data[pos..pos + nal_len]);
        pos += nal_len;
    }
    if out.is_empty() { None } else { Some(out) }
}

fn is_keyframe_nal(codec: crate::traits::VideoCodec, nal_header: u8) -> bool {
    match codec {
        crate::traits::VideoCodec::H264 => (nal_header & 0x1F) == 5,
        crate::traits::VideoCodec::H265 => {
            let nal_type = (nal_header >> 1) & 0x3F;
            matches!(nal_type, 16 | 17 | 18 | 19 | 20 | 21)
        }
    }
}

impl VideoDecoderState {
    fn stop(&mut self) {
        unsafe {
            let _ = OH_VideoDecoder_Stop(self.codec);
            let _ = OH_VideoDecoder_Destroy(self.codec);
            if !self.window.is_null() {
                OH_NativeWindow_DestroyNativeWindow(self.window);
            }
        }
        self.codec = ptr::null_mut();
        self.window = ptr::null_mut();
    }
}

impl AudioDecoderState {
    fn fill_output(&mut self, output: &mut [u8]) -> usize {
        let mut written = 0;
        while written < output.len() {
            let (chunk_len, offset) = match self.pcm_queue.front() {
                Some(chunk) => (chunk.len(), self.pcm_offset),
                None => break,
            };
            if offset >= chunk_len {
                self.pcm_queue.pop_front();
                self.pcm_offset = 0;
                continue;
            }

            let available = chunk_len - offset;
            let to_copy = std::cmp::min(available, output.len() - written);
            if let Some(chunk) = self.pcm_queue.front() {
                output[written..written + to_copy]
                    .copy_from_slice(&chunk[offset..offset + to_copy]);
            }
            written += to_copy;
            self.pcm_offset += to_copy;

            if self.pcm_offset >= chunk_len {
                self.pcm_queue.pop_front();
                self.pcm_offset = 0;
            }
        }
        written
    }

    fn set_paused(&mut self, paused: bool) {
        if self.renderer.is_null() {
            return;
        }
        // Directly call Start/Pause without state checking.
        // HarmonyOS audio renderer tolerates redundant calls.
        let result = if paused {
            unsafe { OH_AudioRenderer_Pause(self.renderer) }
        } else {
            unsafe { OH_AudioRenderer_Start(self.renderer) }
        };
        if result != AUDIOSTREAM_SUCCESS {
            // Ignore state transition errors - they're usually benign (e.g. already paused)
            log::debug!(
                "[Harmony.StreamDecoder] audio renderer state change result: {}",
                result
            );
        }
    }

    fn stop(&mut self) {
        unsafe {
            if !self.renderer.is_null() {
                let _ = OH_AudioRenderer_Stop(self.renderer);
                let _ = OH_AudioRenderer_Release(self.renderer);
            }
            if !self.codec.is_null() {
                let _ = OH_AudioCodec_Stop(self.codec);
                let _ = OH_AudioCodec_Destroy(self.codec);
            }
        }
        self.codec = ptr::null_mut();
        self.renderer = ptr::null_mut();
        self.pcm_queue.clear();
        self.pcm_offset = 0;
    }
}

fn fill_input_buffer(
    codec: *mut OH_AVCodec,
    index: u32,
    buffer: *mut OH_AVBuffer,
    frame: QueuedFrame,
) -> Result<(), PlatformError> {
    if buffer.is_null() {
        return Err(PlatformError::Platform("Input buffer is null".to_string()));
    }
    let capacity = unsafe { OH_AVBuffer_GetCapacity(buffer) };
    if capacity <= 0 || frame.data.len() > capacity as usize {
        return Err(PlatformError::Platform(format!(
            "Input buffer too small: {} < {}",
            capacity,
            frame.data.len()
        )));
    }
    let addr = unsafe { OH_AVBuffer_GetAddr(buffer) };
    if addr.is_null() {
        return Err(PlatformError::Platform(
            "Input buffer addr is null".to_string(),
        ));
    }

    unsafe {
        ptr::copy_nonoverlapping(frame.data.as_ptr(), addr, frame.data.len());
    }

    let attr = OH_AVCodecBufferAttr {
        pts: frame.pts_us,
        size: frame.data.len() as i32,
        offset: 0,
        flags: frame.flags,
    };
    check_av_result(
        unsafe { OH_AVBuffer_SetBufferAttr(buffer, &attr) },
        "OH_AVBuffer_SetBufferAttr",
    )?;
    check_av_result(
        unsafe { OH_VideoDecoder_PushInputBuffer(codec, index) },
        "OH_VideoDecoder_PushInputBuffer",
    )?;
    Ok(())
}

fn fill_audio_input_buffer(
    codec: *mut OH_AVCodec,
    index: u32,
    buffer: *mut OH_AVBuffer,
    frame: QueuedFrame,
) -> Result<(), PlatformError> {
    if buffer.is_null() {
        return Err(PlatformError::Platform(
            "Audio input buffer is null".to_string(),
        ));
    }
    let capacity = unsafe { OH_AVBuffer_GetCapacity(buffer) };
    if capacity <= 0 || frame.data.len() > capacity as usize {
        return Err(PlatformError::Platform(format!(
            "Audio input buffer too small: {} < {}",
            capacity,
            frame.data.len()
        )));
    }
    let addr = unsafe { OH_AVBuffer_GetAddr(buffer) };
    if addr.is_null() {
        return Err(PlatformError::Platform(
            "Audio input buffer addr is null".to_string(),
        ));
    }

    unsafe {
        ptr::copy_nonoverlapping(frame.data.as_ptr(), addr, frame.data.len());
    }

    let attr = OH_AVCodecBufferAttr {
        pts: frame.pts_us,
        size: frame.data.len() as i32,
        offset: 0,
        flags: frame.flags,
    };
    check_av_result(
        unsafe { OH_AVBuffer_SetBufferAttr(buffer, &attr) },
        "OH_AVBuffer_SetBufferAttr(audio)",
    )?;
    check_av_result(
        unsafe { OH_AudioCodec_PushInputBuffer(codec, index) },
        "OH_AudioCodec_PushInputBuffer",
    )?;
    Ok(())
}

fn cleanup_decoder(codec: *mut OH_AVCodec, window: *mut OHNativeWindow) {
    unsafe {
        if !codec.is_null() {
            let _ = OH_VideoDecoder_Destroy(codec);
        }
        if !window.is_null() {
            OH_NativeWindow_DestroyNativeWindow(window);
        }
    }
}

fn build_codec_config(config: &VideoStreamConfig) -> Vec<u8> {
    let mut data = Vec::new();
    let mut push_nal = |nal: &[u8]| {
        if !nal.is_empty() {
            data.extend_from_slice(&[0, 0, 0, 1]);
            data.extend_from_slice(nal);
        }
    };
    push_nal(&config.vps);
    push_nal(&config.sps);
    push_nal(&config.pps);
    data
}

fn build_avcc_config(config: &VideoStreamConfig) -> Vec<u8> {
    if !matches!(config.codec, crate::traits::VideoCodec::H264) {
        return Vec::new();
    }
    if config.sps.len() < 4 || config.pps.is_empty() {
        return Vec::new();
    }
    let nal_length_size = config.nal_length_size.unwrap_or(4).clamp(1, 4);
    let mut data = Vec::new();
    data.push(1); // configurationVersion
    data.push(config.sps[1]); // AVCProfileIndication
    data.push(config.sps[2]); // profile_compatibility
    data.push(config.sps[3]); // AVCLevelIndication
    data.push(0xFC | (nal_length_size - 1)); // lengthSizeMinusOne
    data.push(0xE1); // numOfSequenceParameterSets (1)
    data.extend_from_slice(&(config.sps.len() as u16).to_be_bytes());
    data.extend_from_slice(&config.sps);
    data.push(1); // numOfPictureParameterSets
    data.extend_from_slice(&(config.pps.len() as u16).to_be_bytes());
    data.extend_from_slice(&config.pps);
    data
}

fn video_mime(codec: crate::traits::VideoCodec) -> *const c_char {
    match codec {
        crate::traits::VideoCodec::H264 => unsafe { OH_AVCODEC_MIMETYPE_VIDEO_AVC },
        crate::traits::VideoCodec::H265 => unsafe { OH_AVCODEC_MIMETYPE_VIDEO_HEVC },
    }
}

pub fn create_player(component_id: &str, callback_id: u64) -> Result<i64, PlatformError> {
    // Check if player already exists (created by native component)
    if let Some(existing) = get_player(component_id) {
        if let Ok(p) = existing.lock() {
            return Ok(p.as_ptr() as i64);
        }
    }

    // Create new player if not exists
    let mut player = NativeVideoPlayer::new(component_id, callback_id)?;
    if let Some(surface_id) = lookup_surface_id(component_id) {
        match create_native_window_from_surface_id(&surface_id) {
            Ok(window) => {
                if let Err(err) = player.set_video_surface(window) {
                    unsafe { OH_NativeWindow_DestroyNativeWindow(window) };
                    log::warn!(
                        "[VideoPlayer] create_player: failed to bind stored surface for {}: {}",
                        component_id,
                        err
                    );
                }
            }
            Err(err) => {
                log::warn!(
                    "[VideoPlayer] create_player: failed to create native window from stored surface for {}: {}",
                    component_id,
                    err
                );
            }
        }
    }
    let ptr = player.as_ptr() as i64;
    let manager = get_player_manager();
    let mut players = manager.write().map_err(|_| {
        PlatformError::Platform("Failed to acquire player manager lock".to_string())
    })?;
    players.insert(component_id.to_string(), Arc::new(Mutex::new(player)));
    Ok(ptr)
}

pub fn get_player(component_id: &str) -> Option<Arc<Mutex<NativeVideoPlayer>>> {
    let manager = get_player_manager();
    let players = manager.read().ok()?;
    players.get(component_id).cloned()
}

pub fn destroy_player(component_id: &str) -> Result<(), PlatformError> {
    let manager = get_player_manager();
    let mut players = manager.write().map_err(|_| {
        PlatformError::Platform("Failed to acquire player manager lock".to_string())
    })?;
    if let Some(player) = players.remove(component_id) {
        if let Ok(mut p) = player.lock() {
            p.release()?;
        }
    }
    remove_surface_id(component_id);
    Ok(())
}

fn check_av_result(code: i32, context: &str) -> Result<(), PlatformError> {
    if code == AV_ERR_OK {
        Ok(())
    } else {
        Err(PlatformError::Platform(format!(
            "{} failed: {}",
            context, code
        )))
    }
}

fn set_decoder_surface_with_retry(
    codec: *mut OH_AVCodec,
    window: *mut OHNativeWindow,
    component_id: &str,
) -> Result<(), PlatformError> {
    let mut attempt = 0u32;
    loop {
        let result = unsafe { OH_VideoDecoder_SetSurface(codec, window) };
        if result == AV_ERR_OK {
            if attempt > 0 {
                log::info!(
                    "[Harmony.StreamDecoder] SetSurface succeeded after {} retries for {}",
                    attempt,
                    component_id
                );
            }
            log::info!(
                "[Harmony.StreamDecoder] SetSurface ok for {} codec={:?} window={:?}",
                component_id,
                codec,
                window
            );
            return Ok(());
        }
        if result != AV_ERR_INVALID_STATE || attempt >= 5 {
            return Err(PlatformError::Platform(format!(
                "OH_VideoDecoder_SetSurface failed: {}",
                result
            )));
        }
        attempt += 1;
        log::warn!(
            "[Harmony.StreamDecoder] SetSurface retry {} for {} (err={})",
            attempt,
            component_id,
            result
        );
        std::thread::sleep(std::time::Duration::from_millis(50 * attempt as u64));
    }
}

fn refresh_stream_decoder_surface(component_id: &str) -> Result<(), PlatformError> {
    let Some(wrapper) = lookup_stream_decoder(component_id) else {
        return Ok(());
    };
    let Some(surface_id) = lookup_surface_id(component_id) else {
        return Ok(());
    };

    let (codec, old_window) = {
        let state = wrapper
            .state
            .lock()
            .map_err(|_| PlatformError::Platform("Stream decoder lock poisoned".to_string()))?;
        let Some(video_state) = state.video.as_ref() else {
            return Ok(());
        };
        (video_state.codec, video_state.window)
    };

    if codec.is_null() {
        return Ok(());
    }
    let new_window = create_native_window_from_surface_id(&surface_id)?;
    if let Err(err) = set_decoder_surface_with_retry(codec, new_window, component_id) {
        unsafe { OH_NativeWindow_DestroyNativeWindow(new_window) };
        return Err(err);
    }

    let mut state = wrapper
        .state
        .lock()
        .map_err(|_| PlatformError::Platform("Stream decoder lock poisoned".to_string()))?;
    let Some(video_state) = state.video.as_mut() else {
        unsafe { OH_NativeWindow_DestroyNativeWindow(new_window) };
        return Ok(());
    };
    if video_state.codec != codec {
        unsafe { OH_NativeWindow_DestroyNativeWindow(new_window) };
        return Ok(());
    }
    if !old_window.is_null() && old_window != new_window {
        unsafe { OH_NativeWindow_DestroyNativeWindow(old_window) };
    }
    video_state.window = new_window;
    Ok(())
}

fn check_audio_result(code: i32, context: &str) -> Result<(), PlatformError> {
    if code == AUDIOSTREAM_SUCCESS {
        Ok(())
    } else {
        Err(PlatformError::Platform(format!(
            "{} failed: {}",
            context, code
        )))
    }
}

fn create_audio_renderer(
    sample_rate: i32,
    channels: i32,
    user_data: *mut c_void,
) -> Result<*mut OH_AudioRenderer, PlatformError> {
    let mut builder: *mut OH_AudioStreamBuilder = ptr::null_mut();
    check_audio_result(
        unsafe { OH_AudioStreamBuilder_Create(&mut builder, AUDIOSTREAM_TYPE_RENDERER) },
        "OH_AudioStreamBuilder_Create",
    )?;
    if builder.is_null() {
        return Err(PlatformError::Platform(
            "Audio stream builder is null".to_string(),
        ));
    }

    if let Err(err) = check_audio_result(
        unsafe { OH_AudioStreamBuilder_SetSamplingRate(builder, sample_rate) },
        "OH_AudioStreamBuilder_SetSamplingRate",
    ) {
        unsafe { OH_AudioStreamBuilder_Destroy(builder) };
        return Err(err);
    }
    if let Err(err) = check_audio_result(
        unsafe { OH_AudioStreamBuilder_SetChannelCount(builder, channels) },
        "OH_AudioStreamBuilder_SetChannelCount",
    ) {
        unsafe { OH_AudioStreamBuilder_Destroy(builder) };
        return Err(err);
    }
    if let Err(err) = check_audio_result(
        unsafe { OH_AudioStreamBuilder_SetSampleFormat(builder, AUDIOSTREAM_SAMPLE_S16LE) },
        "OH_AudioStreamBuilder_SetSampleFormat",
    ) {
        unsafe { OH_AudioStreamBuilder_Destroy(builder) };
        return Err(err);
    }
    if let Err(err) = check_audio_result(
        unsafe { OH_AudioStreamBuilder_SetRendererInfo(builder, AUDIOSTREAM_USAGE_MOVIE) },
        "OH_AudioStreamBuilder_SetRendererInfo",
    ) {
        unsafe { OH_AudioStreamBuilder_Destroy(builder) };
        return Err(err);
    }
    if let Err(err) = check_audio_result(
        unsafe { OH_AudioStreamBuilder_SetEncodingType(builder, AUDIOSTREAM_ENCODING_TYPE_RAW) },
        "OH_AudioStreamBuilder_SetEncodingType",
    ) {
        unsafe { OH_AudioStreamBuilder_Destroy(builder) };
        return Err(err);
    }
    if let Err(err) = check_audio_result(
        unsafe {
            OH_AudioStreamBuilder_SetRendererWriteDataCallback(
                builder,
                Some(on_audio_render_write),
                user_data,
            )
        },
        "OH_AudioStreamBuilder_SetRendererWriteDataCallback",
    ) {
        unsafe { OH_AudioStreamBuilder_Destroy(builder) };
        return Err(err);
    }

    let mut renderer: *mut OH_AudioRenderer = ptr::null_mut();
    let result = unsafe { OH_AudioStreamBuilder_GenerateRenderer(builder, &mut renderer) };
    unsafe { OH_AudioStreamBuilder_Destroy(builder) };
    check_audio_result(result, "OH_AudioStreamBuilder_GenerateRenderer")?;
    if renderer.is_null() {
        return Err(PlatformError::Platform(
            "Audio renderer not created".to_string(),
        ));
    }
    let volume_result = unsafe { OH_AudioRenderer_SetVolume(renderer, 1.0) };
    if volume_result != AUDIOSTREAM_SUCCESS {
        log::warn!(
            "[Harmony.StreamDecoder] Failed to set renderer volume: {}",
            volume_result
        );
    }
    Ok(renderer)
}

type OhAvPlayerOnInfoCallback = Option<
    extern "C" fn(
        player: *mut OH_AVPlayer,
        info_type: i32,
        info_body: *mut OH_AVFormat,
        user_data: *mut c_void,
    ),
>;

#[repr(C)]
#[allow(non_snake_case)]
struct OH_AVCodecCallback {
    onError: Option<extern "C" fn(*mut OH_AVCodec, i32, *mut c_void)>,
    onStreamChanged: Option<extern "C" fn(*mut OH_AVCodec, *mut OH_AVFormat, *mut c_void)>,
    onNeedInputBuffer: Option<extern "C" fn(*mut OH_AVCodec, u32, *mut OH_AVBuffer, *mut c_void)>,
    onNewOutputBuffer: Option<extern "C" fn(*mut OH_AVCodec, u32, *mut OH_AVBuffer, *mut c_void)>,
}

#[link(name = "avplayer")]
unsafe extern "C" {
    fn OH_AVPlayer_Create() -> *mut OH_AVPlayer;
    fn OH_AVPlayer_SetURLSource(player: *mut OH_AVPlayer, url: *const c_char) -> i32;
    fn OH_AVPlayer_SetFDSource(player: *mut OH_AVPlayer, fd: i32, offset: i64, size: i64) -> i32;
    fn OH_AVPlayer_SetVideoSurface(player: *mut OH_AVPlayer, window: *mut OHNativeWindow) -> i32;
    fn OH_AVPlayer_Prepare(player: *mut OH_AVPlayer) -> i32;
    fn OH_AVPlayer_Play(player: *mut OH_AVPlayer) -> i32;
    fn OH_AVPlayer_Pause(player: *mut OH_AVPlayer) -> i32;
    fn OH_AVPlayer_Stop(player: *mut OH_AVPlayer) -> i32;
    #[allow(dead_code)]
    fn OH_AVPlayer_Reset(player: *mut OH_AVPlayer) -> i32;
    fn OH_AVPlayer_Seek(player: *mut OH_AVPlayer, ms: i32, mode: i32) -> i32;
    fn OH_AVPlayer_SetVolume(player: *mut OH_AVPlayer, left: f32, right: f32) -> i32;
    #[allow(dead_code)]
    fn OH_AVPlayer_SetPlaybackSpeed(player: *mut OH_AVPlayer, speed: i32) -> i32;
    fn OH_AVPlayer_SetLooping(player: *mut OH_AVPlayer, looping: bool) -> i32;
    fn OH_AVPlayer_GetCurrentTime(player: *mut OH_AVPlayer, time: *mut i32) -> i32;
    fn OH_AVPlayer_GetDuration(player: *mut OH_AVPlayer, duration: *mut i32) -> i32;
    #[allow(dead_code)]
    fn OH_AVPlayer_GetState(player: *mut OH_AVPlayer, state: *mut i32) -> i32;
    fn OH_AVPlayer_IsPlaying(player: *mut OH_AVPlayer) -> bool;
    fn OH_AVPlayer_GetVideoWidth(player: *mut OH_AVPlayer, width: *mut i32) -> i32;
    fn OH_AVPlayer_GetVideoHeight(player: *mut OH_AVPlayer, height: *mut i32) -> i32;
    fn OH_AVPlayer_Release(player: *mut OH_AVPlayer) -> i32;
    fn OH_AVPlayer_SetOnInfoCallback(
        player: *mut OH_AVPlayer,
        callback: OhAvPlayerOnInfoCallback,
        user_data: *mut c_void,
    ) -> i32;
}

#[link(name = "native_media_core")]
unsafe extern "C" {
    fn OH_AVFormat_GetIntValue(format: *mut OH_AVFormat, key: *const c_char, out: *mut i32)
    -> bool;
    fn OH_AVFormat_CreateAudioFormat(
        mime: *const c_char,
        sample_rate: i32,
        channel_count: i32,
    ) -> *mut OH_AVFormat;
    fn OH_AVFormat_CreateVideoFormat(
        mime: *const c_char,
        width: i32,
        height: i32,
    ) -> *mut OH_AVFormat;
    fn OH_AVFormat_Destroy(format: *mut OH_AVFormat);
    fn OH_AVFormat_SetIntValue(format: *mut OH_AVFormat, key: *const c_char, value: i32) -> bool;
    fn OH_AVFormat_SetBuffer(
        format: *mut OH_AVFormat,
        key: *const c_char,
        addr: *const u8,
        size: usize,
    ) -> bool;

    fn OH_AVBuffer_GetAddr(buffer: *mut OH_AVBuffer) -> *mut u8;
    fn OH_AVBuffer_GetCapacity(buffer: *mut OH_AVBuffer) -> i32;
    fn OH_AVBuffer_GetBufferAttr(buffer: *mut OH_AVBuffer, attr: *mut OH_AVCodecBufferAttr) -> i32;
    fn OH_AVBuffer_SetBufferAttr(
        buffer: *mut OH_AVBuffer,
        attr: *const OH_AVCodecBufferAttr,
    ) -> i32;
}

#[link(name = "native_media_vdec")]
unsafe extern "C" {
    fn OH_VideoDecoder_CreateByMime(mime: *const c_char) -> *mut OH_AVCodec;
    fn OH_VideoDecoder_RegisterCallback(
        codec: *mut OH_AVCodec,
        callback: OH_AVCodecCallback,
        user_data: *mut c_void,
    ) -> i32;
    fn OH_VideoDecoder_SetSurface(codec: *mut OH_AVCodec, window: *mut OHNativeWindow) -> i32;
    fn OH_VideoDecoder_Configure(codec: *mut OH_AVCodec, format: *mut OH_AVFormat) -> i32;
    fn OH_VideoDecoder_Prepare(codec: *mut OH_AVCodec) -> i32;
    fn OH_VideoDecoder_Start(codec: *mut OH_AVCodec) -> i32;
    fn OH_VideoDecoder_Stop(codec: *mut OH_AVCodec) -> i32;
    fn OH_VideoDecoder_Destroy(codec: *mut OH_AVCodec) -> i32;
    fn OH_VideoDecoder_PushInputBuffer(codec: *mut OH_AVCodec, index: u32) -> i32;
    fn OH_VideoDecoder_RenderOutputBuffer(codec: *mut OH_AVCodec, index: u32) -> i32;
}

#[link(name = "native_media_acodec")]
unsafe extern "C" {
    fn OH_AudioCodec_CreateByMime(mime: *const c_char, is_encoder: bool) -> *mut OH_AVCodec;
    fn OH_AudioCodec_RegisterCallback(
        codec: *mut OH_AVCodec,
        callback: OH_AVCodecCallback,
        user_data: *mut c_void,
    ) -> i32;
    fn OH_AudioCodec_Configure(codec: *mut OH_AVCodec, format: *mut OH_AVFormat) -> i32;
    fn OH_AudioCodec_Prepare(codec: *mut OH_AVCodec) -> i32;
    fn OH_AudioCodec_Start(codec: *mut OH_AVCodec) -> i32;
    fn OH_AudioCodec_Stop(codec: *mut OH_AVCodec) -> i32;
    fn OH_AudioCodec_Destroy(codec: *mut OH_AVCodec) -> i32;
    fn OH_AudioCodec_PushInputBuffer(codec: *mut OH_AVCodec, index: u32) -> i32;
    fn OH_AudioCodec_FreeOutputBuffer(codec: *mut OH_AVCodec, index: u32) -> i32;
}

#[link(name = "native_media_codecbase")]
unsafe extern "C" {
    static OH_AVCODEC_MIMETYPE_VIDEO_AVC: *const c_char;
    static OH_AVCODEC_MIMETYPE_VIDEO_HEVC: *const c_char;
    static OH_AVCODEC_MIMETYPE_AUDIO_AAC: *const c_char;
    static OH_MD_KEY_PIXEL_FORMAT: *const c_char;
    static OH_MD_KEY_CODEC_CONFIG: *const c_char;
    static OH_MD_KEY_AUDIO_SAMPLE_FORMAT: *const c_char;
    static OH_MD_KEY_AAC_IS_ADTS: *const c_char;
    static OH_MD_KEY_WIDTH: *const c_char;
    static OH_MD_KEY_HEIGHT: *const c_char;
}

#[link(name = "ohaudio")]
unsafe extern "C" {
    fn OH_AudioStreamBuilder_Create(
        builder: *mut *mut OH_AudioStreamBuilder,
        stream_type: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_Destroy(builder: *mut OH_AudioStreamBuilder) -> i32;
    fn OH_AudioStreamBuilder_SetSamplingRate(builder: *mut OH_AudioStreamBuilder, rate: i32)
    -> i32;
    fn OH_AudioStreamBuilder_SetChannelCount(
        builder: *mut OH_AudioStreamBuilder,
        channel_count: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetSampleFormat(
        builder: *mut OH_AudioStreamBuilder,
        sample_format: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetEncodingType(
        builder: *mut OH_AudioStreamBuilder,
        encoding_type: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetRendererInfo(
        builder: *mut OH_AudioStreamBuilder,
        usage: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetRendererWriteDataCallback(
        builder: *mut OH_AudioStreamBuilder,
        callback: Option<
            extern "C" fn(
                renderer: *mut OH_AudioRenderer,
                user_data: *mut c_void,
                audio_data: *mut c_void,
                audio_data_size: i32,
            ) -> i32,
        >,
        user_data: *mut c_void,
    ) -> i32;
    fn OH_AudioStreamBuilder_GenerateRenderer(
        builder: *mut OH_AudioStreamBuilder,
        renderer: *mut *mut OH_AudioRenderer,
    ) -> i32;

    fn OH_AudioRenderer_Start(renderer: *mut OH_AudioRenderer) -> i32;
    fn OH_AudioRenderer_Pause(renderer: *mut OH_AudioRenderer) -> i32;
    fn OH_AudioRenderer_Stop(renderer: *mut OH_AudioRenderer) -> i32;
    fn OH_AudioRenderer_Release(renderer: *mut OH_AudioRenderer) -> i32;
    fn OH_AudioRenderer_SetVolume(renderer: *mut OH_AudioRenderer, volume: f32) -> i32;
}

#[link(name = "avplayer")]
unsafe extern "C" {
    static OH_PLAYER_SEEK_POSITION: *const c_char;
    static OH_PLAYER_BUFFERING_TYPE: *const c_char;
    #[allow(dead_code)]
    static OH_PLAYER_BUFFERING_VALUE: *const c_char;
    static OH_PLAYER_STATE: *const c_char;
}

#[link(name = "native_window")]
unsafe extern "C" {
    fn OH_NativeWindow_CreateNativeWindowFromSurfaceId(
        surface_id: u64,
        window: *mut *mut OHNativeWindow,
    ) -> i32;
    fn OH_NativeWindow_DestroyNativeWindow(window: *mut OHNativeWindow);
}

pub fn create_native_window_from_surface_id(
    surface_id: &str,
) -> Result<*mut OHNativeWindow, PlatformError> {
    let surface_id_u64: u64 = surface_id
        .parse()
        .map_err(|_| PlatformError::Platform(format!("Invalid surface ID: {}", surface_id)))?;
    let mut window: *mut OHNativeWindow = ptr::null_mut();
    let result =
        unsafe { OH_NativeWindow_CreateNativeWindowFromSurfaceId(surface_id_u64, &mut window) };
    if result != 0 || window.is_null() {
        return Err(PlatformError::Platform(format!(
            "Failed to create native window: {}, error: {}",
            surface_id, result
        )));
    }
    log::info!(
        "[Harmony.StreamDecoder] created native window for surface_id={} ptr={:?}",
        surface_id,
        window
    );
    Ok(window)
}

pub fn set_video_surface_from_id(
    component_id: &str,
    surface_id: &str,
) -> Result<(), PlatformError> {
    store_surface_id(component_id, surface_id);

    // If a stream decoder is already created, surface binds may arrive after the stream provider
    // has already delivered the (cached) `VideoStreamConfig`. Trigger a reconfigure so the decoder
    // can bind the new surface and start rendering.
    if let Some(wrapper) = lookup_stream_decoder(component_id) {
        let component_id = component_id.to_string();
        let wrapper_clone = wrapper.clone();
        std::thread::spawn(move || {
            let user_data =
                Arc::as_ptr(&wrapper_clone) as *const StreamDecoderWrapper as *mut c_void;
            match wrapper_clone.state.lock() {
                Ok(mut state) => {
                    if let Some(config) = state.last_video_config.clone() {
                        if let Err(err) = state.configure_video(config, user_data) {
                            log::warn!(
                                "[Harmony.StreamDecoder] delayed surface configure failed for {}: {}",
                                component_id,
                                err
                            );
                        }
                    }
                }
                Err(_) => {
                    log::warn!(
                        "[Harmony.StreamDecoder] delayed surface configure skipped (poisoned lock) for {}",
                        component_id
                    );
                }
            }
        });
    }

    let mut result = Ok(());
    if let Some(player) = get_player(component_id) {
        let window = create_native_window_from_surface_id(surface_id)?;
        match player.lock() {
            Ok(mut p) => {
                result = p.set_video_surface(window);
            }
            Err(_) => {
                unsafe { OH_NativeWindow_DestroyNativeWindow(window) };
                result = Err(PlatformError::Platform(format!(
                    "Failed to acquire player lock: {}",
                    component_id
                )));
            }
        }
    }
    // Player may not exist yet; surface ID is persisted and will be picked up at create time.
    result
}

pub fn rebind_surface_from_id(
    component_id: &str,
    surface_id: &str,
    position_ms: i32,
    should_play: bool,
) -> Result<(), PlatformError> {
    store_surface_id(component_id, surface_id);
    let window = create_native_window_from_surface_id(surface_id)?;
    if let Some(player) = get_player(component_id) {
        if let Ok(mut p) = player.lock() {
            return p.rebind_surface_and_resume(window, position_ms.max(0), should_play);
        }
    }
    Err(PlatformError::Platform(format!(
        "Player not found: {}",
        component_id
    )))
}

pub fn rebind_stream_surface(component_id: &str, surface_id: &str) -> Result<(), PlatformError> {
    store_surface_id(component_id, surface_id);
    let Some(wrapper) = lookup_stream_decoder(component_id) else {
        // Decoder may not exist yet; surface ID is persisted and will be picked up at configure time.
        return Ok(());
    };

    // First pass: check if we need to configure video (surface arrived after stream config)
    {
        let mut state = wrapper
            .state
            .lock()
            .map_err(|_| PlatformError::Platform("Stream decoder lock poisoned".to_string()))?;

        let is_first_surface = state
            .video
            .as_ref()
            .map(|v| v.window.is_null())
            .unwrap_or(true);

        // Fullscreen enter/exit triggers surface swaps; those should be seamless and must not force a
        // keyframe gate (which would look like a "stream restart"). We only enforce keyframe gating
        // when the decoder is binding its first surface (or when video isn't configured yet).
        if is_first_surface {
            state.need_video_keyframe = true;
            state.gate_audio_until_video = true;
            state.gate_audio_deadline = Some(Instant::now() + Duration::from_secs(2));
            state.has_played = false;
        }
        // Avoid forcing a visible loading spinner on surface rebinds (fullscreen transitions in
        // particular). The UI will stay in its current state while we wait for the next frame.
        state.waiting_notified = false;

        // If we received stream config before the surface existed, configure_video() would have stored
        // last_video_config and returned Ok(()). Now that we have a surface, retry configuration.
        if state.video.is_none() {
            if let Some(config) = state.last_video_config.clone() {
                let user_data = Arc::as_ptr(&wrapper) as *const StreamDecoderWrapper as *mut c_void;
                let _ = state.configure_video(config, user_data);
            }
        }
    }

    // Second pass: rebind the surface
    let mut state = wrapper
        .state
        .lock()
        .map_err(|_| PlatformError::Platform("Stream decoder lock poisoned".to_string()))?;

    let Some(video_state) = state.video.as_mut() else {
        return Ok(());
    };
    let window = create_native_window_from_surface_id(surface_id)?;
    if let Err(err) = set_decoder_surface_with_retry(video_state.codec, window, component_id) {
        unsafe { OH_NativeWindow_DestroyNativeWindow(window) };
        return Err(err);
    }
    if !video_state.window.is_null() {
        unsafe { OH_NativeWindow_DestroyNativeWindow(video_state.window) };
    }
    video_state.window = window;

    Ok(())
}

/// Map playback rate (f64) to AVPlaybackSpeed enum
fn rate_to_speed(rate: f64) -> AVPlaybackSpeed {
    // Round to nearest supported speed
    if rate <= 0.625 {
        AVPlaybackSpeed::Speed0_50X
    } else if rate <= 0.875 {
        AVPlaybackSpeed::Speed0_75X
    } else if rate <= 1.125 {
        AVPlaybackSpeed::Speed1_00X
    } else if rate <= 1.375 {
        AVPlaybackSpeed::Speed1_25X
    } else if rate <= 1.625 {
        AVPlaybackSpeed::Speed1_50X
    } else if rate <= 1.875 {
        AVPlaybackSpeed::Speed1_75X
    } else {
        AVPlaybackSpeed::Speed2_00X
    }
}

pub fn set_speed_from_rate(component_id: &str, rate: f64) -> Result<(), PlatformError> {
    let speed = rate_to_speed(rate);
    if let Some(player) = get_player(component_id) {
        if let Ok(mut p) = player.lock() {
            return p.set_speed(speed);
        }
    }
    Err(PlatformError::Platform(format!(
        "Player not found: {}",
        component_id
    )))
}

fn dispatch_command_harmony(
    component_id: &str,
    command: VideoPlayerCommand,
) -> Result<(), PlatformError> {
    match &command {
        VideoPlayerCommand::SetDuration { duration } => {
            let duration_ms = if duration.is_finite() && *duration > 0.0 {
                (*duration * 1000.0).round().clamp(0.0, i32::MAX as f64) as i32
            } else {
                0
            };
            notify_arkts(component_id, "duration", Some(&duration_ms.to_string()));
        }
        VideoPlayerCommand::Stop => {
            notify_arkts(component_id, "duration", Some("0"));
        }
        _ => {}
    }

    if let Some(decoder) = lookup_stream_decoder(component_id) {
        // Apply pause/unpause intent immediately (without waiting for the state mutex). If we wait
        // for the mutex while the stream provider keeps pushing frames, we can drop the next
        // keyframe and end up stuck buffering until the next keyframe arrives.
        match command {
            VideoPlayerCommand::Play => {
                decoder.paused.store(false, Ordering::Release);
                // Clear stale timestamps so underflow recovery doesn't fire immediately after
                // pause->play while the provider is still reconnecting.
                decoder.last_video_enqueue_ms.store(0, Ordering::Release);
                decoder.last_audio_enqueue_ms.store(0, Ordering::Release);
                decoder.last_video_output_ms.store(0, Ordering::Release);
                decoder.video_started_at_ms.store(0, Ordering::Release);
                decoder.video_received_frame.store(false, Ordering::Release);
                decoder.playing_event_pending.store(true, Ordering::Release);
                log::info!(
                    "[Harmony.StreamDecoder] command Play received for {} (fast-unpause)",
                    component_id
                );
            }
            VideoPlayerCommand::Pause => {
                decoder.paused.store(true, Ordering::Release);
                decoder.last_video_enqueue_ms.store(0, Ordering::Release);
                decoder.last_audio_enqueue_ms.store(0, Ordering::Release);
                decoder
                    .playing_event_pending
                    .store(false, Ordering::Release);
                log::info!(
                    "[Harmony.StreamDecoder] command Pause received for {} (fast-pause)",
                    component_id
                );
            }
            _ => {}
        }

        let callback_id = lookup_video_callback_id(component_id);
        let control_event: Option<(&'static str, serde_json::Value)> = match &command {
            VideoPlayerCommand::Play => Some(("playrequest", serde_json::json!({}))),
            VideoPlayerCommand::Pause => Some(("pause", serde_json::json!({"reason":"user"}))),
            VideoPlayerCommand::Stop => Some(("stop", serde_json::json!({"reason":"user"}))),
            VideoPlayerCommand::NotifyEnded => Some(("ended", serde_json::json!({}))),
            VideoPlayerCommand::Seek { position } => {
                Some(("seeked", serde_json::json!({"time": position})))
            }
            _ => None,
        };

        fn invoke_control_event(
            callback_id: u64,
            component_id: String,
            event: &'static str,
            detail: serde_json::Value,
        ) {
            std::thread::spawn(move || {
                let component_id_for_fields = component_id.clone();
                let payload = serde_json::json!({
                    "action": "component.event",
                    "id": component_id_for_fields,
                    "componentId": component_id,
                    "event": event,
                    "detail": detail,
                })
                .to_string();
                let _ = lingxia_messaging::invoke_callback(callback_id, Ok(payload));
            });
        }

        // Do not block the single-threaded JS worker on decoder mutex contention.
        // Decoder callbacks run on codec threads and take the same lock frequently; if we block here,
        // view->native calls can time out (e.g. selectPlaybackSegment) even though the UI is responsive.
        fn apply_stream_decoder_command_locked(
            component_id: &str,
            state: &mut StreamDecoderState,
            command: VideoPlayerCommand,
        ) -> bool {
            match command {
                VideoPlayerCommand::Play => state.set_paused(false),
                VideoPlayerCommand::Pause => state.set_paused(true),
                VideoPlayerCommand::Stop => {
                    state.stop_with_notify();
                    return true;
                }
                VideoPlayerCommand::NotifyEnded => {
                    state.set_paused(true);
                }
                VideoPlayerCommand::Seek { .. } => {
                    log::warn!(
                        "[Harmony.StreamDecoder] seek not supported for {}",
                        component_id
                    );
                }
                VideoPlayerCommand::SetDuration { .. } => {
                    // Duration metadata is consumed by UI overlays on other platforms.
                    // Harmony stream decoder UI sync is handled separately in ArkTS.
                }
                VideoPlayerCommand::EnterFullscreen => {
                    notify_arkts(component_id, "enterFullscreen", None);
                }
                VideoPlayerCommand::ExitFullscreen => {
                    notify_arkts(component_id, "exitFullscreen", None);
                }
            }
            false
        }

        fn apply_stream_decoder_command_blocking(
            component_id: String,
            wrapper: Arc<StreamDecoderWrapper>,
            command: VideoPlayerCommand,
            callback_id: Option<u64>,
            control_event: Option<(&'static str, serde_json::Value)>,
        ) {
            match wrapper.state.lock() {
                Ok(mut state) => {
                    let remove =
                        apply_stream_decoder_command_locked(&component_id, &mut state, command);
                    drop(state);
                    if remove {
                        remove_stream_decoder_if_current(&component_id, &wrapper);
                    }
                    if let (Some(callback_id), Some((event, detail))) = (callback_id, control_event)
                    {
                        invoke_control_event(callback_id, component_id, event, detail);
                    }
                }
                Err(_) => {
                    log::error!(
                        "[Harmony.StreamDecoder] command skipped due to poisoned lock for {}",
                        component_id
                    );
                }
            }
        }

        match decoder.state.try_lock() {
            Ok(mut state) => {
                let remove = apply_stream_decoder_command_locked(component_id, &mut state, command);
                drop(state);
                if remove {
                    remove_stream_decoder_if_current(component_id, &decoder);
                }
                if let (Some(callback_id), Some((event, detail))) = (callback_id, control_event) {
                    invoke_control_event(callback_id, component_id.to_string(), event, detail);
                }
                return Ok(());
            }
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err(PlatformError::Platform(
                    "Failed to acquire stream decoder lock (poisoned)".to_string(),
                ));
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                let wrapper_clone = decoder.clone();
                let component_id = component_id.to_string();
                let callback_id = callback_id;
                let control_event = control_event;
                std::thread::spawn(move || {
                    apply_stream_decoder_command_blocking(
                        component_id,
                        wrapper_clone,
                        command,
                        callback_id,
                        control_event,
                    );
                });
                return Ok(());
            }
        }
    }
    if matches!(
        command,
        VideoPlayerCommand::Stop
            | VideoPlayerCommand::NotifyEnded
            | VideoPlayerCommand::SetDuration { .. }
    ) && get_player(component_id).is_none()
    {
        // Stream-only mode: stopping is idempotent even when no player/decoder is registered.
        return Ok(());
    }
    if matches!(
        command,
        VideoPlayerCommand::Play | VideoPlayerCommand::Pause
    ) && get_player(component_id).is_none()
        && lookup_stream_decoder(component_id).is_none()
    {
        // Stream-only mode: play/pause can be invoked before the decoder is created (e.g. starting
        // a playback stream). Record intent and apply it when the decoder is instantiated so UI
        // state and first-frame events behave deterministically.
        log::info!(
            "[Harmony.StreamDecoder] command {:?} recorded pending (no decoder yet) for {}",
            command,
            component_id
        );
        match command {
            VideoPlayerCommand::Play => set_pending_stream_paused(component_id, false),
            VideoPlayerCommand::Pause => set_pending_stream_paused(component_id, true),
            _ => {}
        }
        return Ok(());
    }
    let player = get_player(component_id)
        .ok_or_else(|| PlatformError::Platform(format!("Player not found: {}", component_id)))?;
    let mut p = player
        .lock()
        .map_err(|_| PlatformError::Platform("Failed to acquire player lock".to_string()))?;

    if matches!(
        command,
        VideoPlayerCommand::Play | VideoPlayerCommand::Pause | VideoPlayerCommand::Seek { .. }
    ) && !p.source_set
    {
        // Stream decode mode can issue play/pause before an AVPlayer source is configured (or even
        // before the stream decoder is created). If we forward to AVPlayer here, it may fail with
        // "prepare without source" and the UI will think play/pause didn't work. Treat this as
        // stream intent and apply it when/if the stream decoder becomes available.
        match command {
            VideoPlayerCommand::Play => set_pending_stream_paused(component_id, false),
            VideoPlayerCommand::Pause => set_pending_stream_paused(component_id, true),
            VideoPlayerCommand::Seek { .. } => {
                // Stream seek is handled by logic layer session directly.
                // Just return Ok to avoid "prepare without source" error.
            }
            _ => {}
        }
        return Ok(());
    }

    match command {
        VideoPlayerCommand::Play => p.play(),
        VideoPlayerCommand::Pause => p.pause(),
        VideoPlayerCommand::Stop => p.stop(),
        VideoPlayerCommand::NotifyEnded => Ok(()),
        VideoPlayerCommand::Seek { position } => {
            p.seek((position * 1000.0) as i32, AVPlayerSeekMode::PreviousSync)
        }
        VideoPlayerCommand::SetDuration { .. } => {
            // Duration metadata is stored separately for stream decoder UI overlays.
            Ok(())
        }
        VideoPlayerCommand::EnterFullscreen => {
            notify_arkts(component_id, "enterFullscreen", None);
            Ok(())
        }
        VideoPlayerCommand::ExitFullscreen => {
            notify_arkts(component_id, "exitFullscreen", None);
            Ok(())
        }
    }
}

pub fn dispatch_command(
    component_id: &str,
    command: VideoPlayerCommand,
) -> Result<(), PlatformError> {
    dispatch_command_harmony(component_id, command)
}

// VideoPlayerManager Implementation
impl VideoPlayerManager for Platform {
    fn bind_player(&self, component_id: &str) -> Result<Box<dyn VideoPlayerHandle>, PlatformError> {
        // List all registered players for debugging
        let manager = get_player_manager();
        if let Ok(players) = manager.read() {
            let keys: Vec<_> = players.keys().collect();
            log::info!(
                "[VideoPlayer] bind_player: looking for '{}', registered players: {:?}",
                component_id,
                keys
            );
        }

        // Allow binding even if player isn't created yet (stream-only mode).
        if get_player(component_id).is_none() {
            log::warn!(
                "[VideoPlayer] bind_player: player not found for {}, proceeding in stream-only mode",
                component_id
            );
        }
        // Note: Harmony events are handled via call_arkts; callback wiring is set via
        // `set_player_callback()`.

        let cid = component_id.to_string();
        let handle =
            VideoPlayerHandleImpl::new(move |command| dispatch_command_harmony(&cid, command));
        Ok(Box::new(handle))
    }

    fn set_player_callback(
        &self,
        component_id: &str,
        callback_id: u64,
    ) -> Result<(), PlatformError> {
        store_video_callback_id(component_id, callback_id);
        let callback_str = callback_id.to_string();
        lingxia_webview::tsfn::call_arkts("setVideoPlayerCallback", &[component_id, &callback_str])
            .map_err(|e| PlatformError::Platform(format!("Failed to set video callback: {}", e)))?;
        Ok(())
    }
}

struct HarmonyStreamDecoderHandle {
    component_id: String,
    wrapper: Arc<StreamDecoderWrapper>,
    reset_in_flight: Arc<AtomicBool>,
}

impl VideoStreamDecoderHandle for HarmonyStreamDecoderHandle {
    fn supports_soft_reset(&self) -> bool {
        true
    }

    fn supports_in_place_hard_reset(&self) -> bool {
        false
    }

    fn flush(&self) -> Result<(), PlatformError> {
        let mut state = self
            .wrapper
            .state
            .lock()
            .map_err(|_| PlatformError::Platform("Stream decoder lock poisoned".to_string()))?;
        state.reset_soft();
        Ok(())
    }

    fn reset_stream(&self, hard: bool) -> Result<(), PlatformError> {
        notify_arkts(&self.component_id, "waiting", None);

        match self.wrapper.state.try_lock() {
            Ok(mut state) => {
                if hard {
                    state.stop_without_notify();
                    drop(state);
                    remove_stream_decoder_if_current(&self.component_id, &self.wrapper);
                    return Ok(());
                }
                state.reset_soft();
                Ok(())
            }
            Err(std::sync::TryLockError::Poisoned(_)) => Err(PlatformError::Platform(
                "Stream decoder lock poisoned".to_string(),
            )),
            Err(std::sync::TryLockError::WouldBlock) => {
                // Don't block the JS worker thread. Schedule the reset in the background so that
                // stream switching (live/playback/camera) can proceed without view call timeouts,
                // while still guaranteeing that pending buffers will be flushed.
                if self
                    .reset_in_flight
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    let wrapper = self.wrapper.clone();
                    let component_id = self.component_id.clone();
                    let in_flight = self.reset_in_flight.clone();
                    std::thread::spawn(move || {
                        let result = (|| {
                            let mut guard = wrapper.state.lock().map_err(|_| {
                                PlatformError::Platform("Stream decoder lock poisoned".to_string())
                            })?;
                            if hard {
                                guard.stop_without_notify();
                                drop(guard);
                                remove_stream_decoder_if_current(&component_id, &wrapper);
                            } else {
                                guard.reset_soft();
                            }
                            Ok::<(), PlatformError>(())
                        })();

                        if let Err(err) = result {
                            log::warn!(
                                "[Harmony.StreamDecoder] reset_stream async failed for {}: {}",
                                component_id,
                                err
                            );
                        }
                        in_flight.store(false, Ordering::SeqCst);
                    });
                } else {
                    log::debug!(
                        "[Harmony.StreamDecoder] reset_stream already in flight for {}",
                        self.component_id
                    );
                }
                Ok(())
            }
        }
    }

    fn configure_video(&self, config: VideoStreamConfig) -> Result<(), PlatformError> {
        let user_data = Arc::as_ptr(&self.wrapper) as *const StreamDecoderWrapper as *mut c_void;
        let mut state = self
            .wrapper
            .state
            .lock()
            .map_err(|_| PlatformError::Platform("Stream decoder lock poisoned".to_string()))?;
        state.configure_video(config, user_data)?;
        Ok(())
    }

    fn configure_audio(&self, config: AudioStreamConfig) -> Result<(), PlatformError> {
        let user_data = Arc::as_ptr(&self.wrapper) as *const StreamDecoderWrapper as *mut c_void;
        let mut state = self
            .wrapper
            .state
            .lock()
            .map_err(|_| PlatformError::Platform("Stream decoder lock poisoned".to_string()))?;
        state.configure_audio(config, user_data)
    }

    fn push_video(&self, frame: VideoFrame) -> Result<(), PlatformError> {
        let codec_to_start = {
            let mut state =
                self.wrapper.state.lock().map_err(|_| {
                    PlatformError::Platform("Stream decoder lock poisoned".to_string())
                })?;
            state.enqueue_video(frame)?
        };
        if let Some(codec) = codec_to_start {
            log::info!(
                "[Harmony.StreamDecoder] starting video decoder on first frame for {} (codec={:?})",
                self.component_id,
                codec
            );
            if let Err(err) = check_av_result(
                unsafe { OH_VideoDecoder_Start(codec) },
                "OH_VideoDecoder_Start",
            ) {
                log::error!(
                    "[Harmony.StreamDecoder] failed to start video decoder for {}: {:?}",
                    self.component_id,
                    err
                );
                return Err(err);
            }
            self.wrapper
                .video_started_at_ms
                .store(now_ms(), Ordering::Release);

            // Spawn a watchdog that can recover from "no output" starts and "first frame then
            // freeze" stalls even if the codec stops invoking callbacks.
            if self
                .wrapper
                .watchdog_started
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                let component_id = self.component_id.clone();
                std::thread::spawn(move || {
                    loop {
                        std::thread::sleep(Duration::from_millis(500));
                        let Some(decoder) = lookup_stream_decoder(&component_id) else {
                            break;
                        };
                        if decoder.is_destroying() {
                            break;
                        }
                        if decoder.paused.load(Ordering::Acquire) {
                            continue;
                        }

                        let now = now_ms();
                        let last_enqueue = decoder.last_video_enqueue_ms.load(Ordering::Acquire);
                        let enqueue_idle_ms = if last_enqueue > 0 {
                            now.saturating_sub(last_enqueue)
                        } else {
                            i64::MAX
                        };
                        let upstream_active = enqueue_idle_ms < 800;

                        let has_frame = decoder.video_received_frame.load(Ordering::Acquire);
                        let started_at = decoder.video_started_at_ms.load(Ordering::Acquire);
                        if !has_frame
                            && started_at > 0
                            && upstream_active
                            && now.saturating_sub(started_at) >= 1500
                            && decoder
                                .video_force_annexb
                                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                                .is_ok()
                        {
                            log::warn!(
                                "[Harmony.StreamDecoder] no video output after start ({}ms), enabling AVCC->AnnexB fallback and soft reset for {}",
                                now.saturating_sub(started_at),
                                component_id
                            );
                            if let Ok(mut state) = decoder.state.lock() {
                                notify_arkts(&component_id, "waiting", None);
                                state.reset_soft();
                            }
                            decoder
                                .underflow_recovery_in_flight
                                .store(false, Ordering::Release);
                            continue;
                        }

                        if !has_frame {
                            continue;
                        }

                        let last_output = decoder.last_video_output_ms.load(Ordering::Acquire);
                        if last_output <= 0 {
                            continue;
                        }

                        let output_idle_ms = now.saturating_sub(last_output);
                        if output_idle_ms < 1500 || !upstream_active {
                            continue;
                        }

                        if decoder
                            .underflow_recovery_in_flight
                            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                            .is_err()
                        {
                            continue;
                        }

                        log::warn!(
                            "[Harmony.StreamDecoder] video output stalled (output_idle={}ms enqueue_idle={}ms), soft reset for {}",
                            output_idle_ms,
                            enqueue_idle_ms,
                            component_id
                        );

                        if let Ok(mut state) = decoder.state.lock() {
                            notify_arkts(&component_id, "waiting", None);
                            state.reset_soft();
                        }
                        decoder
                            .last_video_output_ms
                            .store(now_ms(), Ordering::Release);
                        decoder
                            .underflow_recovery_in_flight
                            .store(false, Ordering::Release);
                    }
                });
            }
        }
        Ok(())
    }

    fn push_audio(&self, frame: AudioFrame) -> Result<(), PlatformError> {
        let codec_to_start = {
            let mut state =
                self.wrapper.state.lock().map_err(|_| {
                    PlatformError::Platform("Stream decoder lock poisoned".to_string())
                })?;
            state.enqueue_audio(frame)?
        };
        if let Some(codec) = codec_to_start {
            log::info!(
                "[Harmony.StreamDecoder] starting audio decoder on first frame for {} (codec={:?})",
                self.component_id,
                codec
            );
            if let Err(err) =
                check_av_result(unsafe { OH_AudioCodec_Start(codec) }, "OH_AudioCodec_Start")
            {
                log::error!(
                    "[Harmony.StreamDecoder] failed to start audio decoder for {}: {:?}",
                    self.component_id,
                    err
                );
                return Err(err);
            }
        }
        Ok(())
    }

    fn stop(&self) -> Result<(), PlatformError> {
        let mut state = self
            .wrapper
            .state
            .lock()
            .map_err(|_| PlatformError::Platform("Stream decoder lock poisoned".to_string()))?;
        // Stopping the low-level decoder should not reset the UI state; UI-level "stop" is
        // emitted via VideoPlayerCommand::Stop (dispatch_command_harmony).
        state.stop_without_notify();
        drop(state);
        remove_stream_decoder_if_current(&self.component_id, &self.wrapper);
        Ok(())
    }
}

impl VideoStreamDecoderManager for Platform {
    fn create_stream_decoder(
        &self,
        component_id: &str,
    ) -> Result<Box<dyn VideoStreamDecoderHandle>, PlatformError> {
        log::info!(
            "[Harmony.StreamDecoder] create_stream_decoder component_id={}",
            component_id
        );
        let wrapper = Arc::new(StreamDecoderWrapper {
            component_id: component_id.to_string(),
            destroying: AtomicUsize::new(0),
            paused: AtomicBool::new(false),
            video_force_annexb: AtomicBool::new(false),
            video_received_frame: AtomicBool::new(false),
            playing_event_pending: AtomicBool::new(false),
            video_started_at_ms: AtomicI64::new(0),
            video_surface_refresh_scheduled: AtomicBool::new(false),
            logged_first_video_input: AtomicBool::new(false),
            logged_first_audio_input: AtomicBool::new(false),
            logged_drop_video_paused: AtomicBool::new(false),
            logged_drop_audio_paused: AtomicBool::new(false),
            logged_video_underflow: AtomicBool::new(false),
            logged_audio_underflow: AtomicBool::new(false),
            logged_first_video_output_callback: AtomicBool::new(false),
            first_video_pts: AtomicI64::new(0),
            last_video_pts: AtomicI64::new(0),
            last_video_enqueue_ms: AtomicI64::new(0),
            last_audio_enqueue_ms: AtomicI64::new(0),
            last_video_output_ms: AtomicI64::new(0),
            first_video_output_ms: AtomicI64::new(0),
            watchdog_started: AtomicBool::new(false),
            underflow_recovery_in_flight: AtomicBool::new(false),
            video_queue: Mutex::new(VecDeque::new()),
            audio_queue: Mutex::new(VecDeque::new()),
            state: Mutex::new(StreamDecoderState::new(component_id.to_string())),
        });
        register_stream_decoder(component_id, wrapper.clone());
        if let Some(paused) = take_pending_stream_paused(component_id) {
            log::info!(
                "[Harmony.StreamDecoder] apply pending pause={} for {}",
                paused,
                component_id
            );
            wrapper.paused.store(paused, Ordering::Release);
            if let Ok(mut state) = wrapper.state.lock() {
                state.paused = paused;
            }
        }
        Ok(Box::new(HarmonyStreamDecoderHandle {
            component_id: component_id.to_string(),
            wrapper,
            reset_in_flight: Arc::new(AtomicBool::new(false)),
        }))
    }
}
