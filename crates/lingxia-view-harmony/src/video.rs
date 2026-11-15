//! OH_AVPlayer frames for the GPU renderer. The player decodes into an OH_NativeImage, which the
//! shared GLES frame path latches and draws inside wgpu's own context.
use lingxia_view::renderer::gpu::hls;
use lingxia_view::renderer::gpu::video::gles::{self, Frames};
use lingxia_view::renderer::gpu::video::{Player, install as install_opener};
use lingxia_view::renderer::gpu::wgpu;
use lingxia_view::renderer::render_thread;
use lingxia_view::scene::video::{Settings, Status, VideoError, VideoOp, resume_ms};
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_void};
use std::fs::File;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU32, Ordering};
use std::sync::{LazyLock, Mutex, Once};

pub(crate) fn install() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| install_opener(open));
}

/// Master playlists' renditions by URL: OH_AVPlayer pins by bitrate and lists no heights.
static MANIFESTS: LazyLock<Mutex<HashMap<String, Vec<hls::Rendition>>>> =
    LazyLock::new(Default::default);

/// Records the playlist read for `url`; players of it list and pin its renditions from now on.
pub(crate) fn store_manifest(url: &str, playlist: &str) {
    let renditions = hls::renditions(playlist);
    log::debug!(target: "lingxia_view", "video renditions {url}: {renditions:?}");
    lock(&MANIFESTS).insert(url.to_owned(), renditions);
}

pub(crate) fn has_manifest(url: &str) -> bool {
    lock(&MANIFESTS).contains_key(url)
}

fn renditions(url: Option<&str>) -> Vec<hls::Rendition> {
    url.and_then(|url| lock(&MANIFESTS).get(url).cloned())
        .unwrap_or_default()
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|p| p.into_inner())
}

/// What the player's callback threads report; read without locks from the registry's calls.
struct Shared {
    player: *mut c_void,
    state: AtomicI32,
    /// Frames queued in the native image and not latched yet.
    frames: AtomicU32,
    ended: AtomicBool,
    buffering: AtomicBool,
    seeking: AtomicBool,
    /// Play asked before the player was prepared.
    play: AtomicBool,
    /// The last play or pause asked: the player's own state trails a request by a callback.
    wants: AtomicBool,
    /// A seek in ms asked before the player was prepared; -1 for none.
    seek: AtomicI64,
    /// The target in ms of the seek in flight, -1 for none: the clock trails it until it lands.
    target: AtomicI64,
    /// Surface and prepare wait for the initialized state when the source set them back.
    window: *mut c_void,
    deferred: AtomicBool,
    error: Mutex<Option<String>>,
    /// The network source, whose playlist names the renditions.
    url: Option<String>,
    /// The rendition playing, by label; `None` plays the master playlist (auto).
    rendition: Mutex<Option<String>>,
    /// A rendition URL asked while the player still prepares: a reset then loses the prepare in
    /// flight and the player never plays, so it reopens once prepared.
    pending: Mutex<Option<String>>,
    /// The URL the player was last opened on.
    opened: Mutex<Option<String>>,
}

// The raw handles are thread-safe NDK objects; the rest is atomics.
unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

impl Shared {
    fn prepared(&self) -> bool {
        matches!(
            self.state.load(Ordering::Acquire),
            STATE_PREPARED | STATE_PLAYING | STATE_PAUSED | STATE_COMPLETED
        )
    }

    /// Issues a seek and records its target until it lands.
    unsafe fn seek_to(&self, ms: i64) {
        self.target.store(ms, Ordering::Release);
        self.seeking.store(true, Ordering::Release);
        unsafe { OH_AVPlayer_Seek(self.player, ms as i32, SEEK_CLOSEST) };
    }

    /// Where the player is, without side effects: an in-flight seek counts as landed.
    fn position_ms(&self) -> i64 {
        let prepared = self.prepared();
        let mut current = 0;
        if prepared {
            unsafe { OH_AVPlayer_GetCurrentTime(self.player, &mut current) };
        }
        resume_ms(
            prepared,
            self.seeking.load(Ordering::Acquire),
            self.target.load(Ordering::Acquire),
            self.seek.load(Ordering::Acquire),
            i64::from(current),
        )
    }

    fn fail(&self, message: String) {
        log::warn!(target: "lingxia_view", "video: {message}");
        *self.error.lock().unwrap_or_else(|p| p.into_inner()) = Some(message);
    }
}

fn open(source: &str) -> Result<Box<dyn Player>, String> {
    let (file, url) = match source
        .strip_prefix("file://")
        .or_else(|| source.starts_with('/').then_some(source))
    {
        Some(path) => {
            let file = File::open(path).map_err(|e| format!("VIDEO_SOURCE: {path}: {e}"))?;
            (Some(file), None)
        }
        None => (
            None,
            Some(CString::new(source).map_err(|_| "VIDEO_SOURCE: invalid URL".to_owned())?),
        ),
    };
    unsafe {
        let image = OH_NativeImage_Create(0, GL_TEXTURE_EXTERNAL_OES);
        if image.is_null() {
            return Err("VIDEO_DECODE: OH_NativeImage_Create failed".into());
        }
        let window = OH_NativeImage_AcquireNativeWindow(image);
        let player = OH_AVPlayer_Create();
        if window.is_null() || player.is_null() {
            let mut image = image;
            OH_NativeImage_Destroy(&mut image);
            return Err("VIDEO_DECODE: OH_AVPlayer_Create failed".into());
        }
        let shared = Box::into_raw(Box::new(Shared {
            player,
            state: AtomicI32::new(0),
            frames: AtomicU32::new(0),
            ended: AtomicBool::new(false),
            buffering: AtomicBool::new(false),
            seeking: AtomicBool::new(false),
            play: AtomicBool::new(false),
            wants: AtomicBool::new(false),
            seek: AtomicI64::new(-1),
            target: AtomicI64::new(-1),
            window,
            deferred: AtomicBool::new(false),
            error: Mutex::new(None),
            url: url.as_ref().map(|_| source.to_owned()),
            rendition: Mutex::new(None),
            pending: Mutex::new(None),
            opened: Mutex::new(url.as_ref().map(|_| source.to_owned())),
        }));
        let context = shared.cast::<c_void>();
        OH_NativeImage_SetOnFrameAvailableListener(
            image,
            OH_OnFrameAvailableListener {
                context,
                on_frame_available: frame_available,
            },
        );
        OH_AVPlayer_SetOnInfoCallback(player, on_info, context);
        OH_AVPlayer_SetOnErrorCallback(player, on_error, context);
        let mut video = Video {
            shared,
            image,
            file: None,
            frames: Frames::default(),
            rate: 1.0,
            volume: 1.0,
            muted: false,
        };
        let code = match (&file, &url) {
            (Some(file), _) => {
                let size = file.metadata().map(|m| m.len() as i64).unwrap_or(0);
                OH_AVPlayer_SetFDSource(player, file.as_raw_fd(), 0, size)
            }
            (_, Some(url)) => OH_AVPlayer_SetURLSource(player, url.as_ptr()),
            _ => unreachable!(),
        };
        video.file = file;
        if code != 0 {
            return Err(format!(
                "VIDEO_SOURCE: the player refused the source ({code})"
            ));
        }
        if OH_AVPlayer_SetVideoSurface(player, window) != 0 || OH_AVPlayer_Prepare(player) != 0 {
            // Not initialized yet: the state callback binds the surface and prepares.
            (*shared).deferred.store(true, Ordering::Release);
            if (*shared).state.load(Ordering::Acquire) == STATE_INITIALIZED {
                bind_and_prepare(&*shared);
            }
        }
        Ok(Box::new(video))
    }
}

unsafe fn bind_and_prepare(shared: &Shared) {
    if !shared.deferred.swap(false, Ordering::AcqRel) {
        return;
    }
    unsafe {
        let surface = OH_AVPlayer_SetVideoSurface(shared.player, shared.window);
        let prepare = OH_AVPlayer_Prepare(shared.player);
        if surface != 0 || prepare != 0 {
            shared.fail(format!(
                "VIDEO_DECODE: the player did not prepare (surface {surface}, prepare {prepare})"
            ));
        }
    }
}

extern "C" fn frame_available(context: *mut c_void) {
    let shared = unsafe { &*context.cast::<Shared>() };
    shared.frames.fetch_add(1, Ordering::AcqRel);
}

extern "C" fn on_info(_: *mut c_void, kind: i32, body: *mut c_void, context: *mut c_void) {
    let shared = unsafe { &*context.cast::<Shared>() };
    let int = |key: *const c_char| {
        let mut value = 0;
        let found = !body.is_null()
            && !key.is_null()
            && unsafe { OH_AVFormat_GetIntValue(body, key, &mut value) };
        found.then_some(value)
    };
    match kind {
        INFO_SEEK_DONE => {
            shared.target.store(-1, Ordering::Release);
            shared.seeking.store(false, Ordering::Release);
        }
        INFO_BUFFERING => {
            let kind = int(unsafe { OH_PLAYER_BUFFERING_TYPE });
            let value = int(unsafe { OH_PLAYER_BUFFERING_VALUE });
            let buffering = match (kind, value) {
                (Some(BUFFERING_START), _) => Some(true),
                (Some(BUFFERING_END), _) => Some(false),
                (_, Some(percent)) => Some(percent < 100),
                _ => None,
            };
            if let Some(buffering) = buffering {
                shared.buffering.store(buffering, Ordering::Release);
            }
        }
        INFO_STATE => {
            let Some(state) = int(unsafe { OH_PLAYER_STATE }) else {
                return;
            };
            shared.state.store(state, Ordering::Release);
            match state {
                STATE_INITIALIZED => unsafe { bind_and_prepare(shared) },
                STATE_PREPARED => {
                    let seek = shared.seek.swap(-1, Ordering::AcqRel);
                    if seek >= 0 {
                        unsafe { shared.seek_to(seek) };
                    }
                    if shared.play.swap(false, Ordering::AcqRel) {
                        unsafe { OH_AVPlayer_Play(shared.player) };
                    }
                }
                STATE_COMPLETED => {
                    shared.ended.store(true, Ordering::Release);
                    shared.wants.store(false, Ordering::Release);
                }
                STATE_ERROR => shared.fail("VIDEO_DECODE: the player stopped on an error".into()),
                _ => {}
            }
        }
        _ => {}
    }
}

extern "C" fn on_error(_: *mut c_void, code: i32, message: *const c_char, context: *mut c_void) {
    let shared = unsafe { &*context.cast::<Shared>() };
    let message = if message.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(message) }
            .to_string_lossy()
            .into_owned()
    };
    // 5411001..5411011 are the IO and network failures of AVErrorCode.
    let kind = if (5411001..=5411011).contains(&code) {
        "VIDEO_NETWORK"
    } else {
        "VIDEO_DECODE"
    };
    shared.fail(format!("{kind}: {message} ({code})"));
}

struct Video {
    shared: *mut Shared,
    image: *mut c_void,
    file: Option<File>,
    frames: Frames,
    rate: f32,
    volume: f32,
    muted: bool,
}

// The NDK handles are thread-safe; GL work runs on the render thread under wgpu's context lock.
unsafe impl Send for Video {}

impl Video {
    fn shared(&self) -> &Shared {
        unsafe { &*self.shared }
    }

    fn player(&self) -> *mut c_void {
        self.shared().player
    }

    /// Reopens the player on `target`, resuming where it is, as it was: the prepared callback
    /// seeks and plays.
    fn reopen(&self, target: String) -> Result<(), String> {
        let shared = self.shared();
        let url =
            CString::new(target.as_str()).map_err(|_| "VIDEO_SOURCE: invalid URL".to_owned())?;
        shared.seek.store(shared.position_ms(), Ordering::Release);
        shared.target.store(-1, Ordering::Release);
        shared.seeking.store(false, Ordering::Release);
        *lock(&shared.opened) = Some(target);
        shared
            .play
            .store(shared.wants.load(Ordering::Acquire), Ordering::Release);
        shared.ended.store(false, Ordering::Release);
        shared.deferred.store(true, Ordering::Release);
        shared.state.store(0, Ordering::Release);
        let mut code = unsafe { OH_AVPlayer_Reset(self.player()) };
        if code == 0 {
            code = unsafe { OH_AVPlayer_SetURLSource(self.player(), url.as_ptr()) };
        }
        if code != 0 {
            return Err(format!(
                "VIDEO_DECODE: the player did not reopen the rendition ({code})"
            ));
        }
        if shared.state.load(Ordering::Acquire) == STATE_INITIALIZED {
            unsafe { bind_and_prepare(shared) };
        }
        Ok(())
    }

    fn play(&self) {
        let shared = self.shared();
        shared.ended.store(false, Ordering::Release);
        shared.wants.store(true, Ordering::Release);
        if shared.prepared() {
            unsafe { OH_AVPlayer_Play(self.player()) };
        } else {
            shared.play.store(true, Ordering::Release);
        }
    }

    fn pause(&self) {
        let shared = self.shared();
        shared.play.store(false, Ordering::Release);
        shared.wants.store(false, Ordering::Release);
        if shared.prepared() {
            unsafe { OH_AVPlayer_Pause(self.player()) };
        }
    }

    fn seek(&self, seconds: f64) {
        let shared = self.shared();
        let ms = (seconds.max(0.0) * 1000.0).round() as i64;
        shared.ended.store(false, Ordering::Release);
        if shared.prepared() {
            unsafe { shared.seek_to(ms) };
        } else {
            shared.seek.store(ms, Ordering::Release);
        }
    }

    fn set_volume(&self) {
        let volume = if self.muted { 0.0 } else { self.volume };
        unsafe { OH_AVPlayer_SetVolume(self.player(), volume, volume) };
    }

    fn set_rate(&mut self, rate: f32) {
        let (speed, applied) = SPEEDS
            .iter()
            .copied()
            .min_by(|a, b| (a.1 - rate).abs().total_cmp(&(b.1 - rate).abs()))
            .unwrap_or((1, 1.0));
        self.rate = applied;
        unsafe { OH_AVPlayer_SetPlaybackSpeed(self.player(), speed) };
    }

    fn size(&self) -> [u32; 2] {
        let (mut width, mut height) = (0, 0);
        unsafe {
            OH_AVPlayer_GetVideoWidth(self.player(), &mut width);
            OH_AVPlayer_GetVideoHeight(self.player(), &mut height);
        }
        [width.max(0) as u32, height.max(0) as u32]
    }
}

impl Player for Video {
    fn has_new(&self) -> bool {
        self.shared().frames.load(Ordering::Acquire) > 0
    }

    fn next_frame(
        &mut self,
        device: &wgpu::Device,
    ) -> Result<Option<(wgpu::Texture, [u16; 2])>, String> {
        if self.shared().frames.load(Ordering::Acquire) == 0 {
            return Ok(None);
        }
        let size = self.size();
        // Not through `self`: the frames borrow it mutably.
        let shared = unsafe { &*self.shared };
        let mut image = Image {
            image: self.image,
            shared,
        };
        let drawn = self.frames.next(device, &mut image, size, None)?;
        Ok(drawn.map(|(_, texture, size)| (texture, size)))
    }

    fn apply(&mut self, settings: &Settings, previous: Option<&Settings>) {
        let changed =
            |field: fn(&Settings) -> f64| previous.is_none_or(|p| field(p) != field(settings));
        if changed(|s| s.looped as u8 as f64) {
            unsafe { OH_AVPlayer_SetLooping(self.player(), settings.looped) };
        }
        if changed(|s| s.rate as f64) {
            self.set_rate(settings.rate);
        }
        if changed(|s| s.volume as f64) || changed(|s| s.muted as u8 as f64) {
            self.volume = settings.volume;
            self.muted = settings.muted;
            self.set_volume();
        }
        if changed(|s| s.paused as u8 as f64) {
            if settings.paused {
                self.pause();
            } else {
                self.play();
            }
        }
    }

    fn command(&mut self, op: &VideoOp) -> Result<(), VideoError> {
        match *op {
            VideoOp::Play => self.play(),
            VideoOp::Pause => self.pause(),
            VideoOp::Seek(seconds) => self.seek(seconds),
            VideoOp::SetMuted(muted) => {
                self.muted = muted;
                self.set_volume();
            }
            VideoOp::SetVolume(volume) => {
                self.volume = volume;
                self.set_volume();
            }
            VideoOp::SetRate(rate) => self.set_rate(rate),
            // The registry reopens the source and switches files.
            VideoOp::Retry | VideoOp::SetQuality(_) => {}
        }
        Ok(())
    }

    fn qualities(&self) -> Vec<String> {
        renditions(self.shared().url.as_deref())
            .into_iter()
            .map(|r| r.label)
            .collect()
    }

    // A second player of the same HLS stream never delivers a frame, and SelectBitRate is
    // acknowledged without a switch: a pin reopens this player on the variant playlist.
    fn pins_in_place(&self) -> bool {
        true
    }

    fn set_quality(&mut self, label: Option<&str>) -> Result<(), VideoError> {
        let shared = self.shared();
        let Some(master) = shared.url.clone() else {
            return Err(VideoError::new(
                "VIDEO_UNSUPPORTED",
                "this source has one quality",
            ));
        };
        if lock(&shared.rendition).as_deref() == label {
            return Ok(());
        }
        let target = match label {
            Some(label) => {
                let rendition = renditions(Some(&master))
                    .into_iter()
                    .find(|r| r.label == label)
                    .ok_or_else(|| {
                        VideoError::new("VIDEO_SOURCE", format!("no quality \"{label}\""))
                    })?;
                hls::resolve(&master, &rendition.uri)
            }
            None => master,
        };
        *lock(&shared.rendition) = label.map(str::to_owned);
        if !shared.prepared() {
            // Back to the URL already opening: nothing more to reopen.
            let opening = lock(&shared.opened).as_deref() == Some(target.as_str());
            *lock(&shared.pending) = (!opening).then_some(target);
            return Ok(());
        }
        self.reopen(target)
            .map_err(|message| VideoError::from_message(&message))
    }

    fn status(&self) -> Status {
        let shared = self.shared();
        if shared.prepared() {
            let pending = lock(&shared.pending).take();
            if let Some(target) = pending
                && lock(&shared.opened).as_deref() != Some(target.as_str())
                && let Err(message) = self.reopen(target)
            {
                shared.fail(message);
            }
        }
        let error = shared
            .error
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_deref()
            .map(VideoError::from_message);
        // Reopening on another rendition or seeking: the position it resumes at.
        let time = shared.position_ms();
        let mut duration = 0;
        if shared.prepared() {
            unsafe { OH_AVPlayer_GetDuration(self.player(), &mut duration) };
        }
        let duration = (duration > 0).then(|| f64::from(duration) / 1000.0);
        let playing = shared.wants.load(Ordering::Acquire);
        Status {
            current_time: time as f64 / 1000.0,
            duration,
            paused: !playing,
            ended: shared.ended.load(Ordering::Acquire),
            buffering: shared.buffering.load(Ordering::Acquire),
            seeking: shared.seeking.load(Ordering::Acquire),
            buffered: if self.file.is_some() {
                duration.unwrap_or(0.0)
            } else {
                0.0
            },
            playback_rate: self.rate,
            volume: self.volume,
            muted: self.muted,
            error,
            ..Status::default()
        }
    }
}

/// The player's native image, driven in the current GL context.
struct Image<'a> {
    image: *mut c_void,
    shared: &'a Shared,
}

impl gles::Image for Image<'_> {
    fn attach(&mut self, external: u32) -> Result<(), String> {
        // Created with texture 0, the image counts as attached already: attaching again would
        // keep texture 0, so it detaches first.
        unsafe { OH_NativeImage_DetachContext(self.image) };
        match unsafe { OH_NativeImage_AttachContext(self.image, external) } {
            0 => Ok(()),
            code => Err(format!("VIDEO_DECODE: OH_NativeImage_AttachContext {code}")),
        }
    }

    fn detach(&mut self) {
        unsafe { OH_NativeImage_DetachContext(self.image) };
    }

    fn latch(&mut self) -> Result<Option<[f32; 16]>, String> {
        // Latch every queued frame; the newest stays bound.
        let queued = self.shared.frames.swap(0, Ordering::AcqRel);
        let updated = (0..queued).fold(false, |updated, _| {
            let latched = unsafe { OH_NativeImage_UpdateSurfaceImage(self.image) } == 0;
            latched || updated
        });
        if !updated {
            return Ok(None);
        }
        let mut matrix = [0f32; 16];
        match unsafe { OH_NativeImage_GetTransformMatrixV2(self.image, matrix.as_mut_ptr()) } {
            0 => Ok(Some(matrix)),
            code => Err(format!(
                "VIDEO_DECODE: OH_NativeImage_GetTransformMatrixV2 {code}"
            )),
        }
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        // Synchronous: no new frame or callback comes after this returns.
        unsafe { OH_AVPlayer_ReleaseSync(self.player()) };
        let mut frames = std::mem::take(&mut self.frames);
        let (image, shared) = (self.image as usize, self.shared as usize);
        // Players close on any thread; their GL work runs on the render thread, like all of it.
        render_thread::run(move || unsafe {
            let mut image = image as *mut c_void;
            let shared = Box::from_raw(shared as *mut Shared);
            frames.release(&mut Image {
                image,
                shared: &*shared,
            });
            // Detached above, so destroying it touches no context.
            OH_NativeImage_Destroy(&mut image);
        });
    }
}

/// `AVPlaybackSpeed` values and the rates they play at.
const SPEEDS: [(i32, f32); 10] = [
    (0, 0.75),
    (1, 1.0),
    (2, 1.25),
    (3, 1.75),
    (4, 2.0),
    (5, 0.5),
    (6, 1.5),
    (7, 3.0),
    (8, 0.25),
    (9, 0.125),
];

const STATE_INITIALIZED: i32 = 1;
const STATE_PREPARED: i32 = 2;
const STATE_PLAYING: i32 = 3;
const STATE_PAUSED: i32 = 4;
const STATE_COMPLETED: i32 = 6;
const STATE_ERROR: i32 = 8;
const INFO_SEEK_DONE: i32 = 0;
const INFO_STATE: i32 = 4;
const INFO_BUFFERING: i32 = 9;
const BUFFERING_START: i32 = 1;
const BUFFERING_END: i32 = 2;
const SEEK_CLOSEST: i32 = 2;

const GL_TEXTURE_EXTERNAL_OES: u32 = 0x8D65;

#[repr(C)]
struct OH_OnFrameAvailableListener {
    context: *mut c_void,
    on_frame_available: extern "C" fn(*mut c_void),
}

#[link(name = "native_image")]
unsafe extern "C" {
    fn OH_NativeImage_Create(texture: u32, target: u32) -> *mut c_void;
    fn OH_NativeImage_AcquireNativeWindow(image: *mut c_void) -> *mut c_void;
    fn OH_NativeImage_AttachContext(image: *mut c_void, texture: u32) -> i32;
    fn OH_NativeImage_DetachContext(image: *mut c_void) -> i32;
    fn OH_NativeImage_UpdateSurfaceImage(image: *mut c_void) -> i32;
    fn OH_NativeImage_GetTransformMatrixV2(image: *mut c_void, matrix: *mut f32) -> i32;
    fn OH_NativeImage_SetOnFrameAvailableListener(
        image: *mut c_void,
        listener: OH_OnFrameAvailableListener,
    ) -> i32;
    fn OH_NativeImage_Destroy(image: *mut *mut c_void);
}

#[link(name = "avplayer")]
unsafe extern "C" {
    static OH_PLAYER_STATE: *const c_char;
    static OH_PLAYER_BUFFERING_TYPE: *const c_char;
    static OH_PLAYER_BUFFERING_VALUE: *const c_char;
    fn OH_AVPlayer_Create() -> *mut c_void;
    fn OH_AVPlayer_SetURLSource(player: *mut c_void, url: *const c_char) -> i32;
    fn OH_AVPlayer_SetFDSource(player: *mut c_void, fd: i32, offset: i64, size: i64) -> i32;
    fn OH_AVPlayer_SetVideoSurface(player: *mut c_void, window: *mut c_void) -> i32;
    fn OH_AVPlayer_Prepare(player: *mut c_void) -> i32;
    fn OH_AVPlayer_Play(player: *mut c_void) -> i32;
    fn OH_AVPlayer_Pause(player: *mut c_void) -> i32;
    fn OH_AVPlayer_Seek(player: *mut c_void, ms: i32, mode: i32) -> i32;
    fn OH_AVPlayer_SetVolume(player: *mut c_void, left: f32, right: f32) -> i32;
    fn OH_AVPlayer_SetPlaybackSpeed(player: *mut c_void, speed: i32) -> i32;
    fn OH_AVPlayer_Reset(player: *mut c_void) -> i32;
    fn OH_AVPlayer_SetLooping(player: *mut c_void, looping: bool) -> i32;
    fn OH_AVPlayer_GetCurrentTime(player: *mut c_void, time: *mut i32) -> i32;
    fn OH_AVPlayer_GetDuration(player: *mut c_void, duration: *mut i32) -> i32;
    fn OH_AVPlayer_GetVideoWidth(player: *mut c_void, width: *mut i32) -> i32;
    fn OH_AVPlayer_GetVideoHeight(player: *mut c_void, height: *mut i32) -> i32;
    fn OH_AVPlayer_ReleaseSync(player: *mut c_void) -> i32;
    fn OH_AVPlayer_SetOnInfoCallback(
        player: *mut c_void,
        callback: extern "C" fn(*mut c_void, i32, *mut c_void, *mut c_void),
        data: *mut c_void,
    ) -> i32;
    fn OH_AVPlayer_SetOnErrorCallback(
        player: *mut c_void,
        callback: extern "C" fn(*mut c_void, i32, *const c_char, *mut c_void),
        data: *mut c_void,
    ) -> i32;
}

#[link(name = "native_media_core")]
unsafe extern "C" {
    fn OH_AVFormat_GetIntValue(format: *mut c_void, key: *const c_char, out: *mut i32) -> bool;
}
