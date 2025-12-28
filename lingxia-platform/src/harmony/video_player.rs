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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

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
    let state_mutex = unsafe { &*(user_data as *const Mutex<StreamDecoderState>) };
    if let Ok(state) = state_mutex.lock() {
        log::error!(
            "[Harmony.StreamDecoder] codec error for {}: {}",
            state.component_id,
            error_code
        );
        let message = format!("codec error: {}", error_code);
        notify_arkts(&state.component_id, "error", Some(&message));
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
    let state_mutex = unsafe { &*(user_data as *const Mutex<StreamDecoderState>) };
    if let Ok(state) = state_mutex.lock() {
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
    let state_mutex = unsafe { &*(user_data as *const Mutex<StreamDecoderState>) };
    if let Ok(mut state) = state_mutex.lock() {
        state.on_need_input_buffer(index, buffer);
    }
}

extern "C" fn on_new_output_buffer(
    codec: *mut OH_AVCodec,
    index: u32,
    _buffer: *mut OH_AVBuffer,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    if codec.is_null() {
        return;
    }
    let state_mutex = unsafe { &*(user_data as *const Mutex<StreamDecoderState>) };
    if let Ok(mut state) = state_mutex.lock() {
        state.on_new_output_buffer(codec, index);
    }
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
    let state_mutex = unsafe { &*(user_data as *const Mutex<StreamDecoderState>) };
    if let Ok(mut state) = state_mutex.lock() {
        state.on_audio_need_input_buffer(index, buffer);
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
    let state_mutex = unsafe { &*(user_data as *const Mutex<StreamDecoderState>) };
    if let Ok(mut state) = state_mutex.lock() {
        state.on_audio_new_output_buffer(codec, index, buffer);
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
    let state_mutex = unsafe { &*(user_data as *const Mutex<StreamDecoderState>) };
    let filled = match state_mutex.lock() {
        Ok(mut state) => state.on_audio_render_write(output),
        Err(_) => 0,
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

    // SAFETY: user_data is Box<InfoCallbackData> created in NativeVideoPlayer::new
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
        if !info_body.is_null() {
            let key_ptr = unsafe { OH_PLAYER_BUFFERING_TYPE };
            if !key_ptr.is_null() {
                unsafe { OH_AVFormat_GetIntValue(info_body, key_ptr, &mut buffering_type) };
            }
        }

        // AVPLAYER_BUFFERING_START = 1, AVPLAYER_BUFFERING_END = 2
        let is_buffering = if buffering_type == 1 {
            Some("1")
        } else if buffering_type == 2 {
            Some("0")
        } else {
            None
        };

        if let Some(status) = is_buffering {
            log::info!(
                "[VideoPlayer] on_info_callback: BUFFERING_UPDATE component_id={}, type={}, status={}",
                component_id,
                buffering_type,
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

        let mut should_autoplay = false;
        let mut player_ptr: *mut OH_AVPlayer = ptr::null_mut();

        // Sync native player state to Rust instance
        if let Some(player) = get_player(component_id) {
            if let Ok(mut p) = player.lock() {
                let new_state = match state_value {
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
                };
                p.state = new_state;
                if new_state == AVPlayerState::Prepared && p.pending_play {
                    p.pending_play = false;
                    should_autoplay = true;
                    player_ptr = p.player;
                }
            }
        } else {
            log::warn!(
                "[VideoPlayer] STATE_CHANGE: player not found for {}",
                component_id
            );
        }

        if should_autoplay && !player_ptr.is_null() {
            let _ = unsafe { OH_AVPlayer_Play(player_ptr) };
        }

        match state_value {
            x if x == AVPlayerState::Prepared as i32 => {
                notify_arkts(component_id, "prepared", None)
            }
            x if x == AVPlayerState::Playing as i32 => notify_arkts(component_id, "playing", None),
            x if x == AVPlayerState::Paused as i32 => notify_arkts(component_id, "paused", None),
            x if x == AVPlayerState::Stopped as i32 => notify_arkts(component_id, "stopped", None),
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
    info_callback_data: Option<Box<InfoCallbackData>>,
    pending_play: bool,
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
        });
        let callback_data_ptr = &*callback_data as *const InfoCallbackData as *mut c_void;
        let result = unsafe {
            OH_AVPlayer_SetOnInfoCallback(player, Some(on_info_callback), callback_data_ptr)
        };
        if result != AV_ERR_OK {
            log::warn!(
                "[VideoPlayer] Failed to set info callback for {}: {}",
                component_id,
                result
            );
            return Ok(Self {
                player,
                component_id: component_id.to_string(),
                window: ptr::null_mut(),
                state: AVPlayerState::Idle,
                volume: 1.0,
                is_looping: false,
                info_callback_data: None,
                pending_play: false,
            });
        }

        Ok(Self {
            player,
            component_id: component_id.to_string(),
            window: ptr::null_mut(),
            state: AVPlayerState::Idle,
            volume: 1.0,
            is_looping: false,
            info_callback_data: Some(callback_data),
            pending_play: false,
        })
    }

    pub fn set_source(&mut self, source: &str) -> Result<(), PlatformError> {
        if source.starts_with("http://") || source.starts_with("https://") {
            self.set_url_source(source)
        } else if source.starts_with("file://") {
            self.set_file_source(&source[7..])
        } else if source.starts_with("fd://") {
            self.set_url_source(source)
        } else if source.starts_with("/") {
            self.set_file_source(source)
        } else {
            self.set_url_source(source)
        }
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
            self.state
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
            if should_play && self.state != AVPlayerState::Playing {
                let play_result = unsafe { OH_AVPlayer_Play(self.player) };
                log::info!("[VideoPlayer] rebind_surface: play result={}", play_result);
                if play_result == AV_ERR_OK {
                    self.state = AVPlayerState::Playing;
                }
            } else if !should_play && self.state == AVPlayerState::Playing {
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

        if self.state == AVPlayerState::Playing {
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
        match self.state {
            AVPlayerState::Stopped | AVPlayerState::Idle | AVPlayerState::Initialized => {
                // For these states, we need to prepare first
                // Prepare is async - it will trigger a state change callback when done
                // For now, just initiate prepare and return success
                // The actual play will happen when state becomes Prepared (via callback or next play call)
                log::info!(
                    "[VideoPlayer] Preparing player before play (current state: {:?})",
                    self.state
                );
                self.pending_play = true;
                let result = check_av_result(
                    unsafe { OH_AVPlayer_Prepare(self.player) },
                    "OH_AVPlayer_Prepare",
                );
                if result.is_err() {
                    self.pending_play = false;
                }
                return result;
            }
            AVPlayerState::Prepared | AVPlayerState::Paused | AVPlayerState::Completed => {
                // These states can transition to Playing directly
                self.pending_play = false;
                let result =
                    check_av_result(unsafe { OH_AVPlayer_Play(self.player) }, "OH_AVPlayer_Play");
                // Don't manually set state - let the callback do it
                return result;
            }
            AVPlayerState::Playing => {
                self.pending_play = false;
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
        self.info_callback_data = None;
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

static STREAM_DECODER_REGISTRY: OnceLock<Mutex<HashMap<String, Arc<Mutex<StreamDecoderState>>>>> =
    OnceLock::new();

fn get_stream_decoder_registry() -> &'static Mutex<HashMap<String, Arc<Mutex<StreamDecoderState>>>>
{
    STREAM_DECODER_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_stream_decoder(component_id: &str, state: Arc<Mutex<StreamDecoderState>>) {
    if let Ok(mut guard) = get_stream_decoder_registry().lock() {
        guard.insert(component_id.to_string(), state);
    }
}

fn lookup_stream_decoder(component_id: &str) -> Option<Arc<Mutex<StreamDecoderState>>> {
    let guard = get_stream_decoder_registry().lock().ok()?;
    guard.get(component_id).cloned()
}

fn remove_stream_decoder_if_current(
    component_id: &str,
    state: &Arc<Mutex<StreamDecoderState>>,
) -> bool {
    if let Ok(mut guard) = get_stream_decoder_registry().lock() {
        if let Some(current) = guard.get(component_id) {
            if Arc::ptr_eq(current, state) {
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
    let state = lookup_stream_decoder(component_id).ok_or_else(|| {
        PlatformError::Platform(format!("Stream decoder not found: {}", component_id))
    })?;
    if let Ok(mut guard) = state.lock() {
        guard.set_volume(volume)
    } else {
        Err(PlatformError::Platform(
            "Stream decoder lock poisoned".to_string(),
        ))
    }
}

struct StreamDecoderState {
    component_id: String,
    paused: bool,
    started: bool,
    has_played: bool,
    volume: f32,
    video: Option<VideoDecoderState>,
    audio: Option<AudioDecoderState>,
    last_video_config: Option<VideoStreamConfig>,
    last_audio_config: Option<AudioStreamConfig>,
}

struct VideoDecoderState {
    codec: *mut OH_AVCodec,
    window: *mut OHNativeWindow,
    available_inputs: VecDeque<InputBuffer>,
    pending_frames: VecDeque<QueuedFrame>,
    logged_input: bool,
    logged_output: bool,
}

struct AudioDecoderState {
    codec: *mut OH_AVCodec,
    renderer: *mut OH_AudioRenderer,
    available_inputs: VecDeque<InputBuffer>,
    pending_frames: VecDeque<QueuedFrame>,
    pcm_queue: VecDeque<Vec<u8>>,
    pcm_offset: usize,
    logged_input: bool,
    logged_output: bool,
    logged_render: bool,
    pcm_only: bool,
}

unsafe impl Send for StreamDecoderState {}
unsafe impl Sync for StreamDecoderState {}

struct InputBuffer {
    index: u32,
    buffer: *mut OH_AVBuffer,
}

struct QueuedFrame {
    data: Vec<u8>,
    pts_us: i64,
    flags: u32,
}

impl StreamDecoderState {
    fn new(component_id: String) -> Self {
        Self {
            component_id,
            paused: false,
            started: false,
            has_played: false,
            volume: 1.0,
            video: None,
            audio: None,
            last_video_config: None,
            last_audio_config: None,
        }
    }

    fn reset_soft(&mut self) {
        if let Some(video_state) = self.video.as_mut() {
            video_state.pending_frames.clear();
            video_state.available_inputs.clear();
            video_state.logged_input = false;
            video_state.logged_output = false;
        }
        if let Some(audio_state) = self.audio.as_mut() {
            audio_state.pending_frames.clear();
            audio_state.available_inputs.clear();
            audio_state.pcm_queue.clear();
            audio_state.pcm_offset = 0;
            audio_state.logged_input = false;
            audio_state.logged_output = false;
            audio_state.logged_render = false;
        }
        self.has_played = false;
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
        self.paused = paused;
        if let Some(audio_state) = self.audio.as_mut() {
            audio_state.set_paused(paused);
        }
        if !self.started {
            return;
        }
        if paused {
            notify_arkts(&self.component_id, "paused", None);
        } else if self.started {
            notify_arkts(&self.component_id, "playing", None);
        }
    }

    fn configure_video(
        &mut self,
        config: VideoStreamConfig,
        user_data: *mut c_void,
    ) -> Result<(), PlatformError> {
        let should_notify = !self.started;
        if let Some(prev) = self.last_video_config.as_ref()
            && self.video.is_some()
            && prev == &config
        {
            return Ok(());
        }
        if let Some(mut existing) = self.video.take() {
            log::info!(
                "[Harmony.StreamDecoder] reconfiguring video decoder for {}",
                self.component_id
            );
            existing.stop();
        }
        if !matches!(
            config.format,
            crate::traits::VideoFormat::AnnexB | crate::traits::VideoFormat::Avcc
        ) {
            return Err(PlatformError::Platform(
                "Harmony decoder expects AnnexB/Avcc format".to_string(),
            ));
        }

        let surface_id = lookup_surface_id(&self.component_id).ok_or_else(|| {
            PlatformError::Platform(format!(
                "Surface not set for component: {}",
                self.component_id
            ))
        })?;
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

        let mut codec_config = Vec::new();
        let mut push_codec_config = false;
        if matches!(config.format, crate::traits::VideoFormat::Avcc) {
            let avcc = build_avcc_config(&config);
            if !avcc.is_empty() {
                let config_set = unsafe {
                    OH_AVFormat_SetBuffer(format, OH_MD_KEY_CODEC_CONFIG, avcc.as_ptr(), avcc.len())
                };
                if config_set {
                    codec_config = avcc;
                    push_codec_config = false;
                } else {
                    log::warn!("[Harmony.StreamDecoder] Failed to set AVCC config");
                }
                log::info!(
                    "[Harmony.StreamDecoder] avcc config: len={}, set={}",
                    codec_config.len(),
                    config_set
                );
            }
        } else {
            codec_config = build_codec_config(&config);
            push_codec_config = !codec_config.is_empty();
            log::info!(
                "[Harmony.StreamDecoder] annexb config: len={}, push_as_input={}",
                codec_config.len(),
                push_codec_config
            );
        }

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
        if let Err(err) = check_av_result(
            unsafe { OH_VideoDecoder_Start(codec) },
            "OH_VideoDecoder_Start",
        ) {
            cleanup_decoder(codec, window);
            return Err(err);
        }

        let mut video_state = VideoDecoderState {
            codec,
            window,
            available_inputs: VecDeque::new(),
            pending_frames: VecDeque::new(),
            logged_input: false,
            logged_output: false,
        };

        if push_codec_config {
            log::info!(
                "[Harmony.StreamDecoder] pushing video codec config via input for {} (len={})",
                self.component_id,
                codec_config.len()
            );
            video_state.pending_frames.push_back(QueuedFrame {
                data: codec_config,
                pts_us: 0,
                flags: AVCODEC_BUFFER_FLAGS_CODEC_DATA,
            });
        }

        self.video = Some(video_state);
        self.last_video_config = Some(config);
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
        if let Some(prev) = self.last_audio_config.as_ref()
            && self.audio.is_some()
            && prev == &config
        {
            return Ok(());
        }
        if let Some(mut existing) = self.audio.take() {
            log::info!(
                "[Harmony.StreamDecoder] reconfiguring audio decoder for {}",
                self.component_id
            );
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
                available_inputs: VecDeque::new(),
                pending_frames: VecDeque::new(),
                pcm_queue: VecDeque::new(),
                pcm_offset: 0,
                logged_input: false,
                logged_output: false,
                logged_render: false,
                pcm_only: true,
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
        let mut push_codec_config = !codec_config.is_empty();
        if config.aac_is_adts {
            push_codec_config = false;
        } else if push_codec_config {
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
        if let Err(err) =
            check_av_result(unsafe { OH_AudioCodec_Start(codec) }, "OH_AudioCodec_Start")
        {
            unsafe {
                let _ = OH_AudioCodec_Destroy(codec);
            }
            return Err(err);
        }

        let renderer = match create_audio_renderer(sample_rate, channels, user_data) {
            Ok(renderer) => renderer,
            Err(err) => {
                unsafe {
                    let _ = OH_AudioCodec_Stop(codec);
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

        let mut audio_state = AudioDecoderState {
            codec,
            renderer,
            available_inputs: VecDeque::new(),
            pending_frames: VecDeque::new(),
            pcm_queue: VecDeque::new(),
            pcm_offset: 0,
            logged_input: false,
            logged_output: false,
            logged_render: false,
            pcm_only: false,
        };

        if push_codec_config {
            log::info!(
                "[Harmony.StreamDecoder] pushing audio codec config via input for {}",
                self.component_id
            );
            audio_state.pending_frames.push_back(QueuedFrame {
                data: codec_config,
                pts_us: 0,
                flags: AVCODEC_BUFFER_FLAGS_CODEC_DATA,
            });
        }

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

    fn enqueue_video(&mut self, frame: VideoFrame) -> Result<(), PlatformError> {
        if self.paused {
            return Ok(());
        }
        if frame.data.is_empty() {
            return Ok(());
        }
        let video_state = self
            .video
            .as_mut()
            .ok_or_else(|| PlatformError::Platform("Video decoder not configured".to_string()))?;

        let mut flags = 0u32;
        if frame.keyframe {
            flags |= AVCODEC_BUFFER_FLAGS_SYNC_FRAME;
        }

        video_state.pending_frames.push_back(QueuedFrame {
            data: frame.data,
            pts_us: frame.pts_ms as i64 * 1000,
            flags,
        });
        video_state.flush()?;

        if !self.paused && !self.has_played {
            notify_arkts(&self.component_id, "playing", None);
            self.has_played = true;
        }
        Ok(())
    }

    fn enqueue_audio(&mut self, frame: AudioFrame) -> Result<(), PlatformError> {
        if self.paused {
            return Ok(());
        }
        if frame.data.is_empty() {
            return Ok(());
        }
        let audio_state = self
            .audio
            .as_mut()
            .ok_or_else(|| PlatformError::Platform("Audio decoder not configured".to_string()))?;

        if audio_state.pcm_only {
            audio_state.pcm_queue.push_back(frame.data);
        } else {
            audio_state.pending_frames.push_back(QueuedFrame {
                data: frame.data,
                pts_us: frame.pts_ms as i64 * 1000,
                flags: 0,
            });
            audio_state.flush()?;
        }

        if !self.paused && !self.has_played {
            notify_arkts(&self.component_id, "playing", None);
            self.has_played = true;
        }
        Ok(())
    }

    fn on_need_input_buffer(&mut self, index: u32, buffer: *mut OH_AVBuffer) {
        if let Some(video_state) = self.video.as_mut() {
            video_state
                .available_inputs
                .push_back(InputBuffer { index, buffer });
            if !video_state.logged_input {
                video_state.logged_input = true;
                log::info!(
                    "[Harmony.StreamDecoder] first video input buffer: index={} for {}",
                    index,
                    self.component_id
                );
            }
            let _ = video_state.flush();
        }
    }

    fn on_audio_need_input_buffer(&mut self, index: u32, buffer: *mut OH_AVBuffer) {
        if let Some(audio_state) = self.audio.as_mut() {
            if audio_state.codec.is_null() {
                return;
            }
            audio_state
                .available_inputs
                .push_back(InputBuffer { index, buffer });
            if !audio_state.logged_input {
                audio_state.logged_input = true;
                log::info!(
                    "[Harmony.StreamDecoder] first audio input buffer: index={} for {}",
                    index,
                    self.component_id
                );
            }
            let _ = audio_state.flush();
        }
    }

    fn on_new_output_buffer(&mut self, codec: *mut OH_AVCodec, index: u32) {
        let result = unsafe { OH_VideoDecoder_RenderOutputBuffer(codec, index) };
        if let Some(video_state) = self.video.as_mut() {
            if !video_state.logged_output {
                video_state.logged_output = true;
                log::info!(
                    "[Harmony.StreamDecoder] first video output buffer: index={} for {} (result={})",
                    index,
                    self.component_id,
                    result
                );
            }
        }
        if result != AV_ERR_OK {
            log::warn!(
                "[Harmony.StreamDecoder] render output buffer failed for {}: {}",
                self.component_id,
                result
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
        if let Some(mut video_state) = self.video.take() {
            video_state.stop();
        }
        if let Some(mut audio_state) = self.audio.take() {
            audio_state.stop();
        }
        self.started = false;
        self.paused = false;
        self.has_played = false;
        self.last_video_config = None;
        self.last_audio_config = None;
        if notify {
            notify_arkts(&self.component_id, "stopped", None);
        }
    }

    fn stop_with_notify(&mut self) {
        self.stop_internal(true);
    }

    fn stop_without_notify(&mut self) {
        self.stop_internal(false);
    }
}

impl VideoDecoderState {
    fn flush(&mut self) -> Result<(), PlatformError> {
        while !self.available_inputs.is_empty() && !self.pending_frames.is_empty() {
            let input = self
                .available_inputs
                .pop_front()
                .ok_or_else(|| PlatformError::Platform("Missing video input buffer".to_string()))?;
            let frame = self
                .pending_frames
                .pop_front()
                .ok_or_else(|| PlatformError::Platform("Missing video frame".to_string()))?;
            fill_input_buffer(self.codec, input.index, input.buffer, frame)?;
        }
        Ok(())
    }

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
    fn flush(&mut self) -> Result<(), PlatformError> {
        if self.codec.is_null() {
            return Ok(());
        }
        while !self.available_inputs.is_empty() && !self.pending_frames.is_empty() {
            let input = self
                .available_inputs
                .pop_front()
                .ok_or_else(|| PlatformError::Platform("Missing audio input buffer".to_string()))?;
            let frame = self
                .pending_frames
                .pop_front()
                .ok_or_else(|| PlatformError::Platform("Missing audio frame".to_string()))?;
            fill_audio_input_buffer(self.codec, input.index, input.buffer, frame)?;
        }
        Ok(())
    }

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
        let result = if paused {
            unsafe { OH_AudioRenderer_Pause(self.renderer) }
        } else {
            unsafe { OH_AudioRenderer_Start(self.renderer) }
        };
        if result != AUDIOSTREAM_SUCCESS {
            log::warn!(
                "[Harmony.StreamDecoder] audio renderer state change failed: {}",
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
        self.available_inputs.clear();
        self.pending_frames.clear();
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
    let player = NativeVideoPlayer::new(component_id, callback_id)?;
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
    Ok(window)
}

pub fn set_video_surface_from_id(
    component_id: &str,
    surface_id: &str,
) -> Result<(), PlatformError> {
    store_surface_id(component_id, surface_id);
    let window = create_native_window_from_surface_id(surface_id)?;
    if let Some(player) = get_player(component_id) {
        if let Ok(mut p) = player.lock() {
            return p.set_video_surface(window);
        }
    }

    Err(PlatformError::Platform(format!(
        "Player not found: {}",
        component_id
    )))
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
    let decoder = lookup_stream_decoder(component_id).ok_or_else(|| {
        PlatformError::Platform(format!("Stream decoder not found: {}", component_id))
    })?;
    let mut state = decoder
        .lock()
        .map_err(|_| PlatformError::Platform("Stream decoder lock poisoned".to_string()))?;
    let video_state = state.video.as_mut().ok_or_else(|| {
        PlatformError::Platform("Stream video decoder not configured".to_string())
    })?;
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
    if let Some(decoder) = lookup_stream_decoder(component_id) {
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
                VideoPlayerCommand::Seek { .. } => {
                    log::warn!(
                        "[Harmony.StreamDecoder] seek not supported for {}",
                        component_id
                    );
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
            decoder: Arc<Mutex<StreamDecoderState>>,
            command: VideoPlayerCommand,
        ) {
            match decoder.lock() {
                Ok(mut state) => {
                    let remove =
                        apply_stream_decoder_command_locked(&component_id, &mut state, command);
                    drop(state);
                    if remove {
                        remove_stream_decoder_if_current(&component_id, &decoder);
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

        match decoder.try_lock() {
            Ok(mut state) => {
                let remove = apply_stream_decoder_command_locked(component_id, &mut state, command);
                drop(state);
                if remove {
                    remove_stream_decoder_if_current(component_id, &decoder);
                }
                return Ok(());
            }
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err(PlatformError::Platform(
                    "Failed to acquire stream decoder lock (poisoned)".to_string(),
                ));
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                let decoder_clone = decoder.clone();
                let component_id = component_id.to_string();
                std::thread::spawn(move || {
                    apply_stream_decoder_command_blocking(component_id, decoder_clone, command);
                });
                return Ok(());
            }
        }
    }
    let player = get_player(component_id)
        .ok_or_else(|| PlatformError::Platform(format!("Player not found: {}", component_id)))?;
    let mut p = player
        .lock()
        .map_err(|_| PlatformError::Platform("Failed to acquire player lock".to_string()))?;

    match command {
        VideoPlayerCommand::Play => p.play(),
        VideoPlayerCommand::Pause => p.pause(),
        VideoPlayerCommand::Stop => p.stop(),
        VideoPlayerCommand::Seek { position } => {
            p.seek((position * 1000.0) as i32, AVPlayerSeekMode::PreviousSync)
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
        // Note: Harmony events are handled via call_arkts, not callback_id

        let cid = component_id.to_string();
        let handle =
            VideoPlayerHandleImpl::new(move |command| dispatch_command_harmony(&cid, command));
        Ok(Box::new(handle))
    }
}

struct HarmonyStreamDecoderHandle {
    component_id: String,
    state: Arc<Mutex<StreamDecoderState>>,
    reset_in_flight: Arc<AtomicBool>,
}

impl VideoStreamDecoderHandle for HarmonyStreamDecoderHandle {
    fn supports_soft_reset(&self) -> bool {
        true
    }

    fn reset_stream(&self, hard: bool) -> Result<(), PlatformError> {
        match self.state.try_lock() {
            Ok(mut state) => {
                if hard {
                    state.stop_without_notify();
                    drop(state);
                    remove_stream_decoder_if_current(&self.component_id, &self.state);
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
                    let state = self.state.clone();
                    let component_id = self.component_id.clone();
                    let in_flight = self.reset_in_flight.clone();
                    std::thread::spawn(move || {
                        let result = (|| {
                            let mut guard = state.lock().map_err(|_| {
                                PlatformError::Platform("Stream decoder lock poisoned".to_string())
                            })?;
                            if hard {
                                guard.stop_without_notify();
                                drop(guard);
                                remove_stream_decoder_if_current(&component_id, &state);
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
        let user_data = Arc::as_ptr(&self.state) as *const Mutex<StreamDecoderState> as *mut c_void;
        let mut state = self
            .state
            .lock()
            .map_err(|_| PlatformError::Platform("Stream decoder lock poisoned".to_string()))?;
        state.configure_video(config, user_data)
    }

    fn configure_audio(&self, config: AudioStreamConfig) -> Result<(), PlatformError> {
        let user_data = Arc::as_ptr(&self.state) as *const Mutex<StreamDecoderState> as *mut c_void;
        let mut state = self
            .state
            .lock()
            .map_err(|_| PlatformError::Platform("Stream decoder lock poisoned".to_string()))?;
        state.configure_audio(config, user_data)
    }

    fn push_video(&self, frame: VideoFrame) -> Result<(), PlatformError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PlatformError::Platform("Stream decoder lock poisoned".to_string()))?;
        state.enqueue_video(frame)
    }

    fn push_audio(&self, frame: AudioFrame) -> Result<(), PlatformError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PlatformError::Platform("Stream decoder lock poisoned".to_string()))?;
        state.enqueue_audio(frame)
    }

    fn stop(&self) -> Result<(), PlatformError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PlatformError::Platform("Stream decoder lock poisoned".to_string()))?;
        // Stopping the low-level decoder should not reset the UI state; UI-level "stopped" is
        // emitted via VideoPlayerCommand::Stop (dispatch_command_harmony).
        state.stop_without_notify();
        drop(state);
        remove_stream_decoder_if_current(&self.component_id, &self.state);
        Ok(())
    }
}

impl VideoStreamDecoderManager for Platform {
    fn create_stream_decoder(
        &self,
        component_id: &str,
    ) -> Result<Box<dyn VideoStreamDecoderHandle>, PlatformError> {
        let state = Arc::new(Mutex::new(StreamDecoderState::new(
            component_id.to_string(),
        )));
        register_stream_decoder(component_id, state.clone());
        Ok(Box::new(HarmonyStreamDecoderHandle {
            component_id: component_id.to_string(),
            state,
            reset_in_flight: Arc::new(AtomicBool::new(false)),
        }))
    }
}
