//! HarmonyOS native video player (OH_AVPlayer C API)

use core::ffi::{c_char, c_void};
use std::collections::HashMap;
use std::ffi::CString;
use std::ptr;
use std::sync::{Arc, Mutex, RwLock};

const AV_ERR_OK: i32 = 0;

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
pub struct OH_AVPlayer { _private: [u8; 0] }
#[repr(C)]
pub struct OHNativeWindow { _private: [u8; 0] }
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct OH_AVFormat { _private: [u8; 0] }

struct InfoCallbackData {
    component_id: String,
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
                let got_value = unsafe { OH_AVFormat_GetIntValue(info_body, key_ptr, &mut seek_position) };
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
        if let Err(e) = lingxia_webview::tsfn::call_arkts(
            "videoPlayerSeekDone",
            &[component_id.as_str(), &position_str],
        ) {
            log::error!("[VideoPlayer] Failed to notify ArkTS of seek done: {:?}", e);
        }
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
            if let Err(e) = lingxia_webview::tsfn::call_arkts(
                "videoPlayerBuffering",
                &[component_id.as_str(), status],
            ) {
                log::error!("[VideoPlayer] Failed to notify ArkTS of buffering: {:?}", e);
            }
        }
    } else if info_type == AVPlayerOnInfoType::StateChange as i32 {
        let mut state_value: i32 = 0;
        if !info_body.is_null() {
            let key_ptr = unsafe { OH_PLAYER_STATE };
            if !key_ptr.is_null() {
                unsafe { OH_AVFormat_GetIntValue(info_body, key_ptr, &mut state_value) };
            }
        }

        log::info!(
            "[VideoPlayer] on_info_callback: STATE_CHANGE component_id={}, state={}",
            component_id,
            state_value
        );

        // When state becomes Prepared (2), notify ArkTS to emit loadedmetadata
        if state_value == AVPlayerState::Prepared as i32 {
            if let Err(e) = lingxia_webview::tsfn::call_arkts(
                "videoPlayerPrepared",
                &[component_id.as_str()],
            ) {
                log::error!("[VideoPlayer] Failed to notify ArkTS of prepared state: {:?}", e);
            }
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
}

// SAFETY: Player accessed on main thread, protected by mutex
unsafe impl Send for NativeVideoPlayer {}
unsafe impl Sync for NativeVideoPlayer {}

impl NativeVideoPlayer {
    pub fn new(component_id: &str, _callback_id: u64) -> Result<Self, PlatformError> {
        let player = unsafe { OH_AVPlayer_Create() };
        if player.is_null() {
            return Err(PlatformError::Platform("Failed to create OH_AVPlayer".to_string()));
        }

        let callback_data = Box::new(InfoCallbackData {
            component_id: component_id.to_string(),
        });
        let callback_data_ptr = &*callback_data as *const InfoCallbackData as *mut c_void;
        let result = unsafe {
            OH_AVPlayer_SetOnInfoCallback(player, Some(on_info_callback), callback_data_ptr)
        };
        if result != AV_ERR_OK {
            log::warn!("[VideoPlayer] Failed to set info callback for {}: {}", component_id, result);
            return Ok(Self {
                player,
                component_id: component_id.to_string(),
                window: ptr::null_mut(),
                state: AVPlayerState::Idle,
                volume: 1.0,
                is_looping: false,
                info_callback_data: None,
            });
        }

        log::info!("[VideoPlayer] Info callback registered for {}", component_id);

        Ok(Self {
            player,
            component_id: component_id.to_string(),
            window: ptr::null_mut(),
            state: AVPlayerState::Idle,
            volume: 1.0,
            is_looping: false,
            info_callback_data: Some(callback_data),
        })
    }

    pub fn set_source(&mut self, source: &str) -> Result<(), PlatformError> {
        if source.starts_with("http://") || source.starts_with("https://") || source.starts_with("fd://") {
            self.set_url_source(source)
        } else if source.starts_with("file://") {
            self.set_file_source(&source[7..])
        } else if source.starts_with("/") {
            self.set_file_source(source)
        } else {
            self.set_url_source(source)
        }
    }

    fn set_url_source(&mut self, url: &str) -> Result<(), PlatformError> {
        let c_url = CString::new(url).map_err(|_| {
            PlatformError::Platform("URL contains invalid characters".to_string())
        })?;
        check_av_result(
            unsafe { OH_AVPlayer_SetURLSource(self.player, c_url.as_ptr()) },
            "OH_AVPlayer_SetURLSource",
        )
    }

    fn set_file_source(&mut self, path: &str) -> Result<(), PlatformError> {
        let c_path = CString::new(path).map_err(|_| {
            PlatformError::Platform("Path contains invalid characters".to_string())
        })?;

        let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY) };
        if fd < 0 {
            return Err(PlatformError::Platform(format!(
                "Failed to open file: {}", path
            )));
        }

        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(fd, &mut stat) } < 0 {
            unsafe { libc::close(fd) };
            return Err(PlatformError::Platform(format!("Failed to stat file: {}", path)));
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
        log::info!("[VideoPlayer] rebind_surface: pos={}, should_play={}, state={:?}",
                   position_ms, should_play, self.state);

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
                log::info!("[VideoPlayer] rebind_surface: pause result={}", pause_result);
                if pause_result == AV_ERR_OK {
                    self.state = AVPlayerState::Paused;
                }
            }
            log::info!("[VideoPlayer] rebind_surface: done (direct)");
            return Ok(());
        }

        log::info!("[VideoPlayer] rebind_surface: direct switch failed (err={}), fallback to Stop/Prepare", direct_result);

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
            unsafe { OH_AVPlayer_Seek(self.player, position_ms, AVPlayerSeekMode::PreviousSync as i32) };
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
        check_av_result(unsafe { OH_AVPlayer_Prepare(self.player) }, "OH_AVPlayer_Prepare")
    }

    pub fn play(&mut self) -> Result<(), PlatformError> {
        check_av_result(unsafe { OH_AVPlayer_Play(self.player) }, "OH_AVPlayer_Play")
    }

    pub fn pause(&mut self) -> Result<(), PlatformError> {
        check_av_result(unsafe { OH_AVPlayer_Pause(self.player) }, "OH_AVPlayer_Pause")
    }

    pub fn stop(&mut self) -> Result<(), PlatformError> {
        check_av_result(unsafe { OH_AVPlayer_Stop(self.player) }, "OH_AVPlayer_Stop")
    }

    pub fn seek(&mut self, position_ms: i32, mode: AVPlayerSeekMode) -> Result<(), PlatformError> {
        let mut before_pos = 0i32;
        unsafe { OH_AVPlayer_GetCurrentTime(self.player, &mut before_pos) };
        log::info!("[VideoPlayer] seek: target={}, before_pos={}", position_ms, before_pos);
        let seek_result = unsafe { OH_AVPlayer_Seek(self.player, position_ms, mode as i32) };
        log::info!("[VideoPlayer] seek: result={}", seek_result);
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
            check_av_result(unsafe { OH_AVPlayer_Release(self.player) }, "OH_AVPlayer_Release")?;
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

    fn set_video_surface_internal(&mut self, window: *mut OHNativeWindow) -> Result<(), PlatformError> {
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

pub fn create_player(component_id: &str, callback_id: u64) -> Result<i64, PlatformError> {
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
    Ok(())
}

fn check_av_result(code: i32, context: &str) -> Result<(), PlatformError> {
    if code == AV_ERR_OK {
        Ok(())
    } else {
        Err(PlatformError::Platform(format!("{} failed: {}", context, code)))
    }
}

type OhAvPlayerOnInfoCallback = Option<
    extern "C" fn(
        player: *mut OH_AVPlayer,
        info_type: i32,
        info_body: *mut OH_AVFormat,
        user_data: *mut c_void,
    ),
>;

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
    fn OH_AVFormat_GetIntValue(format: *mut OH_AVFormat, key: *const c_char, out: *mut i32) -> bool;
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
    fn OH_NativeWindow_CreateNativeWindowFromSurfaceId(surface_id: u64, window: *mut *mut OHNativeWindow) -> i32;
    fn OH_NativeWindow_DestroyNativeWindow(window: *mut OHNativeWindow);
}

pub fn create_native_window_from_surface_id(surface_id: &str) -> Result<*mut OHNativeWindow, PlatformError> {
    let surface_id_u64: u64 = surface_id.parse().map_err(|_| {
        PlatformError::Platform(format!("Invalid surface ID: {}", surface_id))
    })?;
    let mut window: *mut OHNativeWindow = ptr::null_mut();
    let result = unsafe { OH_NativeWindow_CreateNativeWindowFromSurfaceId(surface_id_u64, &mut window) };
    if result != 0 || window.is_null() {
        return Err(PlatformError::Platform(format!(
            "Failed to create native window: {}, error: {}", surface_id, result
        )));
    }
    Ok(window)
}

pub fn set_video_surface_from_id(component_id: &str, surface_id: &str) -> Result<(), PlatformError> {
    let window = create_native_window_from_surface_id(surface_id)?;
    if let Some(player) = get_player(component_id) {
        if let Ok(mut p) = player.lock() {
            return p.set_video_surface(window);
        }
    }

    Err(PlatformError::Platform(format!("Player not found: {}", component_id)))
}

pub fn rebind_surface_from_id(
    component_id: &str,
    surface_id: &str,
    position_ms: i32,
    should_play: bool,
) -> Result<(), PlatformError> {
    let window = create_native_window_from_surface_id(surface_id)?;
    if let Some(player) = get_player(component_id) {
        if let Ok(mut p) = player.lock() {
            return p.rebind_surface_and_resume(window, position_ms.max(0), should_play);
        }
    }
    Err(PlatformError::Platform(format!("Player not found: {}", component_id)))
}

