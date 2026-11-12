//! OH_AVPlayer frames for the GPU renderer. The player decodes into an OH_NativeImage; on the
//! render thread, inside wgpu's own GLES context, each new frame is latched into an external
//! texture and drawn into an RGBA texture wgpu samples. Commands in one context run in order,
//! so no fence sits between the draw and wgpu's use of the texture.
use lingxia_view::renderer::gpu::hls;
use lingxia_view::renderer::gpu::video::{Player, install as install_opener};
use lingxia_view::renderer::gpu::wgpu;
use lingxia_view::renderer::render_thread;
use lingxia_view::scene::video::{Settings, Status, VideoError, VideoOp};
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_void};
use std::fs::File;
use std::num::NonZeroU32;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU32, Ordering};
use std::sync::{LazyLock, Mutex, Once};

/// Frames in flight: the one wgpu samples and the one being drawn.
const SLOTS: usize = 2;

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
            window,
            deferred: AtomicBool::new(false),
            error: Mutex::new(None),
            url: url.as_ref().map(|_| source.to_owned()),
            rendition: Mutex::new(None),
            pending: Mutex::new(None),
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
            gl: None,
            device: None,
            slots: Vec::new(),
            next: 0,
            size: [0, 0],
            rate: 1.0,
            volume: 1.0,
            muted: false,
            probes: 0,
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
        INFO_SEEK_DONE => shared.seeking.store(false, Ordering::Release),
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
                        shared.seeking.store(true, Ordering::Release);
                        unsafe { OH_AVPlayer_Seek(shared.player, seek as i32, SEEK_CLOSEST) };
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
    gl: Option<Blit>,
    /// The device whose GL context owns `gl`; names are per context, so teardown happens there.
    device: Option<wgpu::Device>,
    slots: Vec<wgpu::Texture>,
    next: usize,
    size: [u32; 2],
    rate: f32,
    volume: f32,
    muted: bool,
    /// Frames drawn so far, up to the first few that log the frame path.
    probes: u8,
}

/// Frames whose path is logged per player.
const PROBES: u8 = 2;

// The NDK handles are thread-safe; GL work runs on the render thread under wgpu's context lock.
unsafe impl Send for Video {}

struct Blit {
    external: u32,
    program: u32,
    transform: i32,
    framebuffer: u32,
}

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
        let url = CString::new(target).map_err(|_| "VIDEO_SOURCE: invalid URL".to_owned())?;
        let at = self.status().current_time;
        shared
            .seek
            .store((at * 1000.0).round() as i64, Ordering::Release);
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
            shared.seeking.store(true, Ordering::Release);
            unsafe { OH_AVPlayer_Seek(self.player(), ms as i32, SEEK_CLOSEST) };
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
        let frames = self.shared().frames.load(Ordering::Acquire);
        let size = self.size();
        if frames == 0 || size[0] == 0 || size[1] == 0 {
            return Ok(None);
        }
        if self.device.as_ref().is_some_and(|owner| owner != device) {
            self.release();
        }
        let resize = size != self.size;
        if resize {
            // wgpu frees the old frames on its own schedule, outside the context lock below.
            self.slots.clear();
            self.next = 0;
        }
        let hal = unsafe { device.as_hal::<wgpu::hal::api::Gles>() }
            .ok_or("VIDEO_DECODE: not a GLES device")?;
        let mut created = Vec::new();
        let drawn = {
            let _context = hal.context().lock();
            if self.gl.is_none() {
                let blit = unsafe { Blit::new()? };
                // Created with texture 0, the image counts as attached already: attaching again
                // would keep texture 0, so it detaches first.
                let detached = unsafe { OH_NativeImage_DetachContext(self.image) };
                let code = unsafe { OH_NativeImage_AttachContext(self.image, blit.external) };
                log::debug!(
                    target: "lingxia_view",
                    "video frame path: detach {detached}, attach {code} to external texture {} (target {GL_TEXTURE_EXTERNAL_OES:#x}) in context {:?}, essl3 external {}",
                    blit.external,
                    unsafe { eglGetCurrentContext() },
                    unsafe { has_extension(c"GL_OES_EGL_image_external_essl3") },
                );
                if code != 0 {
                    unsafe { blit.delete() };
                    return Err(format!("VIDEO_DECODE: OH_NativeImage_AttachContext {code}"));
                }
                self.gl = Some(blit);
                self.device = Some(device.clone());
            }
            if resize {
                for _ in 0..SLOTS {
                    created.push(unsafe { color_texture(size) });
                }
            }
            // Latch every queued frame; the newest stays bound.
            let latched = self.shared().frames.swap(0, Ordering::AcqRel);
            let mut updated = false;
            let mut last = 0;
            for _ in 0..latched {
                last = unsafe { OH_NativeImage_UpdateSurfaceImage(self.image) };
                updated |= last == 0;
            }
            let mut matrix = [0f32; 16];
            let mut code = 0;
            if updated {
                code =
                    unsafe { OH_NativeImage_GetTransformMatrixV2(self.image, matrix.as_mut_ptr()) };
            }
            if self.probes < PROBES {
                log::debug!(
                    target: "lingxia_view",
                    "video frame path: latched {latched}, update {last}, matrix {code} {matrix:?}, size {size:?}"
                );
            }
            updated.then_some(matrix)
        };
        if !created.is_empty() {
            for name in created {
                let name = NonZeroU32::new(name).ok_or("VIDEO_DECODE: no GL texture")?;
                self.slots.push(unsafe { wrap(&hal, device, name, size) });
            }
            self.size = size;
        }
        let Some(matrix) = drawn else {
            return Ok(None);
        };
        let index = self.next;
        self.next = (self.next + 1) % self.slots.len();
        let slot = &self.slots[index];
        let name = unsafe { slot.as_hal::<wgpu::hal::api::Gles>() }
            .and_then(|texture| match &texture.inner {
                wgpu::hal::gles::TextureInner::Texture { raw, .. } => Some(raw.0.get()),
                _ => None,
            })
            .ok_or("VIDEO_DECODE: no GL name for the frame")?;
        let probe = self.probes < PROBES;
        self.probes = self.probes.saturating_add(1);
        let drawn = {
            let _context = hal.context().lock();
            let gl = self.gl.as_ref().ok_or("VIDEO_DECODE: no GL")?;
            unsafe { gl.draw(name, size, &matrix, probe) }
        };
        if let Err(error) = drawn {
            // A target that lost its storage is made again at the next frame.
            self.slots.clear();
            self.size = [0, 0];
            self.next = 0;
            return Err(error);
        }
        Ok(Some((
            slot.clone(),
            [size[0].min(65535) as u16, size[1].min(65535) as u16],
        )))
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
            *lock(&shared.pending) = Some(target);
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
        let (mut time, mut duration) = (0, 0);
        if shared.prepared() {
            unsafe {
                OH_AVPlayer_GetCurrentTime(self.player(), &mut time);
                OH_AVPlayer_GetDuration(self.player(), &mut duration);
            }
        } else {
            // Reopening on another rendition: the position it resumes at.
            time = shared.seek.load(Ordering::Acquire).max(0) as i32;
        }
        let duration = (duration > 0).then(|| f64::from(duration) / 1000.0);
        let playing = shared.wants.load(Ordering::Acquire);
        Status {
            current_time: f64::from(time.max(0)) / 1000.0,
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

impl Video {
    /// Detaches the image from the context that made its GL names and frees them there, to
    /// attach it to another; on the render thread, where `next_frame` runs.
    fn release(&mut self) {
        // wgpu frees these frames itself, under its own lock.
        self.slots.clear();
        self.size = [0, 0];
        self.next = 0;
        if let (Some(gl), Some(device)) = (self.gl.take(), self.device.take()) {
            unsafe { teardown(&device, gl, self.image, false) };
        }
    }
}

/// Frees `gl`'s names under `device`'s context lock, destroying the image or detaching it.
/// Only on the render thread: the lock makes the context current on the calling thread.
unsafe fn teardown(device: &wgpu::Device, gl: Blit, mut image: *mut c_void, destroy: bool) {
    let Some(hal) = (unsafe { device.as_hal::<wgpu::hal::api::Gles>() }) else {
        return;
    };
    let _context = hal.context().lock();
    unsafe {
        if destroy {
            OH_NativeImage_Destroy(&mut image);
        } else {
            OH_NativeImage_DetachContext(image);
        }
        // Destroy leaves the attached texture to this context; checked under the same lock,
        // so the name cannot have gone to another player in between.
        let alive = glIsTexture(gl.external) != 0;
        log::debug!(
            target: "lingxia_view",
            "video teardown: external texture {} alive after destroy: {alive}, context {:?}",
            gl.external,
            eglGetCurrentContext(),
        );
        if alive {
            glDeleteTextures(1, &gl.external);
        }
        gl.delete_pipeline();
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        // Synchronous: no new frame or callback comes after this returns.
        unsafe { OH_AVPlayer_ReleaseSync(self.player()) };
        self.slots.clear();
        let (gl, device) = (self.gl.take(), self.device.take());
        let (image, shared) = (self.image as usize, self.shared as usize);
        // Players close on any thread; their GL work runs on the render thread, like all of it.
        render_thread::run(move || unsafe {
            let mut image = image as *mut c_void;
            match (gl, device) {
                (Some(gl), Some(device)) => teardown(&device, gl, image, true),
                _ => OH_NativeImage_Destroy(&mut image),
            }
            drop(Box::from_raw(shared as *mut Shared));
        });
    }
}

/// An RGBA texture in the current context, for wgpu to own once wrapped.
unsafe fn color_texture(size: [u32; 2]) -> u32 {
    unsafe {
        let mut name = 0;
        glGenTextures(1, &mut name);
        glBindTexture(GL_TEXTURE_2D, name);
        glTexStorage2D(GL_TEXTURE_2D, 1, GL_RGBA8, size[0] as i32, size[1] as i32);
        glBindTexture(GL_TEXTURE_2D, 0);
        name
    }
}

/// Hands texture `name` to wgpu, which deletes it with the texture.
unsafe fn wrap(
    hal: &wgpu::hal::gles::Device,
    device: &wgpu::Device,
    name: NonZeroU32,
    size: [u32; 2],
) -> wgpu::Texture {
    let extent = wgpu::Extent3d {
        width: size[0],
        height: size[1],
        depth_or_array_layers: 1,
    };
    unsafe {
        let texture = hal.texture_from_raw(
            name,
            &wgpu::hal::TextureDescriptor {
                label: Some("LingXia video frame"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUses::RESOURCE,
                memory_flags: wgpu::hal::MemoryFlags::empty(),
                view_formats: Vec::new(),
            },
            None,
        );
        device.create_texture_from_hal::<wgpu::hal::api::Gles>(
            texture,
            &wgpu::TextureDescriptor {
                label: Some("LingXia video frame"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::TextureUses::RESOURCE,
        )
    }
}

const VERTEX: &CStr = c"#version 300 es
uniform mat4 transform;
out vec2 uv;
void main() {
    vec2 p = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1));
    uv = (transform * vec4(p.x, 1.0 - p.y, 0.0, 1.0)).xy;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}";

const FRAGMENT: &CStr = c"#version 300 es
#extension GL_OES_EGL_image_external_essl3 : require
precision mediump float;
uniform samplerExternalOES frame;
in vec2 uv;
out vec4 color;
void main() { color = texture(frame, uv); }";

impl Blit {
    /// Deletes the program and framebuffer; the external texture goes with its image.
    unsafe fn delete_pipeline(&self) {
        unsafe {
            glDeleteProgram(self.program);
            glDeleteFramebuffers(1, &self.framebuffer);
        }
    }

    /// Deletes everything, for a blit no image was attached to.
    unsafe fn delete(&self) {
        unsafe {
            glDeleteTextures(1, &self.external);
            self.delete_pipeline();
        }
    }

    unsafe fn new() -> Result<Self, String> {
        unsafe {
            let vertex = shader(GL_VERTEX_SHADER, VERTEX)?;
            let fragment = shader(GL_FRAGMENT_SHADER, FRAGMENT)?;
            let program = glCreateProgram();
            glAttachShader(program, vertex);
            glAttachShader(program, fragment);
            glLinkProgram(program);
            glDeleteShader(vertex);
            glDeleteShader(fragment);
            let mut linked = 0;
            glGetProgramiv(program, GL_LINK_STATUS, &mut linked);
            if linked == 0 {
                glDeleteProgram(program);
                return Err("VIDEO_DECODE: blit program did not link".into());
            }
            let transform = glGetUniformLocation(program, c"transform".as_ptr());
            let mut external = 0;
            glGenTextures(1, &mut external);
            glBindTexture(GL_TEXTURE_EXTERNAL_OES, external);
            glTexParameteri(GL_TEXTURE_EXTERNAL_OES, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
            glTexParameteri(GL_TEXTURE_EXTERNAL_OES, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
            glTexParameteri(GL_TEXTURE_EXTERNAL_OES, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            glTexParameteri(GL_TEXTURE_EXTERNAL_OES, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            glBindTexture(GL_TEXTURE_EXTERNAL_OES, 0);
            let mut framebuffer = 0;
            glGenFramebuffers(1, &mut framebuffer);
            Ok(Self {
                external,
                program,
                transform,
                framebuffer,
            })
        }
    }

    /// Draws the latched frame into `color`, leaving the state wgpu resets per submission.
    unsafe fn draw(
        &self,
        color: u32,
        size: [u32; 2],
        matrix: &[f32; 16],
        probe: bool,
    ) -> Result<(), String> {
        unsafe {
            let before = glGetError();
            glBindFramebuffer(GL_FRAMEBUFFER, self.framebuffer);
            glFramebufferTexture2D(
                GL_FRAMEBUFFER,
                GL_COLOR_ATTACHMENT0,
                GL_TEXTURE_2D,
                color,
                0,
            );
            let status = glCheckFramebufferStatus(GL_FRAMEBUFFER);
            let complete = status == GL_FRAMEBUFFER_COMPLETE;
            if !complete || probe {
                log::debug!(
                    target: "lingxia_view",
                    "video frame path: target {color} is texture {}, framebuffer {status:#x}, prior GL error {before:#x}",
                    glIsTexture(color) != 0,
                );
            }
            if complete {
                // wgpu leaves its sampler objects bound; one on unit 0 would override the
                // external texture's own filtering.
                glBindSampler(0, 0);
                glViewport(0, 0, size[0] as i32, size[1] as i32);
                glDisable(GL_BLEND);
                glDisable(GL_SCISSOR_TEST);
                glDisable(GL_DEPTH_TEST);
                glDisable(GL_STENCIL_TEST);
                glDisable(GL_CULL_FACE);
                glColorMask(1, 1, 1, 1);
                glUseProgram(self.program);
                glUniformMatrix4fv(self.transform, 1, 0, matrix.as_ptr());
                glActiveTexture(GL_TEXTURE0);
                glBindTexture(GL_TEXTURE_EXTERNAL_OES, self.external);
                glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);
                glBindTexture(GL_TEXTURE_EXTERNAL_OES, 0);
                glUseProgram(0);
                if probe {
                    let mut pixel = [0u8; 4];
                    glReadPixels(
                        size[0] as i32 / 2,
                        size[1] as i32 / 2,
                        1,
                        1,
                        GL_RGBA,
                        GL_UNSIGNED_BYTE,
                        pixel.as_mut_ptr().cast(),
                    );
                    log::debug!(
                        target: "lingxia_view",
                        "video frame path: drew {size:?}, centre {pixel:?}, GL error {:#x}",
                        glGetError(),
                    );
                }
            }
            glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, 0, 0);
            glBindFramebuffer(GL_FRAMEBUFFER, 0);
            glFlush();
            if complete {
                Ok(())
            } else {
                Err("VIDEO_DECODE: the frame texture is not renderable".into())
            }
        }
    }
}

/// `name` is in the current context's extension list.
unsafe fn has_extension(name: &CStr) -> bool {
    unsafe {
        let mut count = 0;
        glGetIntegerv(GL_NUM_EXTENSIONS, &mut count);
        (0..count.max(0) as u32).any(|index| {
            let extension = glGetStringi(GL_EXTENSIONS, index);
            !extension.is_null() && CStr::from_ptr(extension.cast()) == name
        })
    }
}

unsafe fn shader(kind: u32, source: &CStr) -> Result<u32, String> {
    unsafe {
        let shader = glCreateShader(kind);
        let text = source.as_ptr();
        glShaderSource(shader, 1, &text, std::ptr::null());
        glCompileShader(shader);
        let mut compiled = 0;
        glGetShaderiv(shader, GL_COMPILE_STATUS, &mut compiled);
        if compiled == 0 {
            let mut log = [0u8; 512];
            let mut length = 0;
            glGetShaderInfoLog(
                shader,
                log.len() as i32,
                &mut length,
                log.as_mut_ptr().cast(),
            );
            glDeleteShader(shader);
            let length = (length.max(0) as usize).min(log.len());
            return Err(format!(
                "VIDEO_DECODE: shader: {}",
                String::from_utf8_lossy(&log[..length])
            ));
        }
        Ok(shader)
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
const GL_TEXTURE_2D: u32 = 0x0DE1;
const GL_RGBA8: u32 = 0x8058;
const GL_TEXTURE_MIN_FILTER: u32 = 0x2801;
const GL_TEXTURE_MAG_FILTER: u32 = 0x2800;
const GL_TEXTURE_WRAP_S: u32 = 0x2802;
const GL_TEXTURE_WRAP_T: u32 = 0x2803;
const GL_LINEAR: i32 = 0x2601;
const GL_CLAMP_TO_EDGE: i32 = 0x812F;
const GL_FRAMEBUFFER: u32 = 0x8D40;
const GL_COLOR_ATTACHMENT0: u32 = 0x8CE0;
const GL_FRAMEBUFFER_COMPLETE: u32 = 0x8CD5;
const GL_VERTEX_SHADER: u32 = 0x8B31;
const GL_FRAGMENT_SHADER: u32 = 0x8B30;
const GL_COMPILE_STATUS: u32 = 0x8B81;
const GL_LINK_STATUS: u32 = 0x8B82;
const GL_TRIANGLE_STRIP: u32 = 0x0005;
const GL_TEXTURE0: u32 = 0x84C0;
const GL_BLEND: u32 = 0x0BE2;
const GL_SCISSOR_TEST: u32 = 0x0C11;
const GL_DEPTH_TEST: u32 = 0x0B71;
const GL_STENCIL_TEST: u32 = 0x0B90;
const GL_CULL_FACE: u32 = 0x0B44;
const GL_RGBA: u32 = 0x1908;
const GL_UNSIGNED_BYTE: u32 = 0x1401;
const GL_EXTENSIONS: u32 = 0x1F03;
const GL_NUM_EXTENSIONS: u32 = 0x821D;

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

#[link(name = "EGL")]
unsafe extern "C" {
    fn eglGetCurrentContext() -> *mut c_void;
}

#[link(name = "GLESv3")]
unsafe extern "C" {
    fn glGenTextures(count: i32, names: *mut u32);
    fn glDeleteTextures(count: i32, names: *const u32);
    fn glBindTexture(target: u32, name: u32);
    fn glTexParameteri(target: u32, name: u32, value: i32);
    fn glTexStorage2D(target: u32, levels: i32, format: u32, width: i32, height: i32);
    fn glGenFramebuffers(count: i32, names: *mut u32);
    fn glDeleteFramebuffers(count: i32, names: *const u32);
    fn glBindFramebuffer(target: u32, name: u32);
    fn glFramebufferTexture2D(target: u32, attachment: u32, kind: u32, texture: u32, level: i32);
    fn glCheckFramebufferStatus(target: u32) -> u32;
    fn glCreateShader(kind: u32) -> u32;
    fn glShaderSource(shader: u32, count: i32, sources: *const *const c_char, lengths: *const i32);
    fn glCompileShader(shader: u32);
    fn glGetShaderiv(shader: u32, name: u32, value: *mut i32);
    fn glGetShaderInfoLog(shader: u32, size: i32, length: *mut i32, log: *mut c_char);
    fn glDeleteShader(shader: u32);
    fn glCreateProgram() -> u32;
    fn glAttachShader(program: u32, shader: u32);
    fn glLinkProgram(program: u32);
    fn glGetProgramiv(program: u32, name: u32, value: *mut i32);
    fn glDeleteProgram(program: u32);
    fn glGetUniformLocation(program: u32, name: *const c_char) -> i32;
    fn glUseProgram(program: u32);
    fn glUniformMatrix4fv(location: i32, count: i32, transpose: u8, value: *const f32);
    fn glActiveTexture(unit: u32);
    fn glViewport(x: i32, y: i32, width: i32, height: i32);
    fn glDisable(capability: u32);
    fn glColorMask(red: u8, green: u8, blue: u8, alpha: u8);
    fn glDrawArrays(mode: u32, first: i32, count: i32);
    fn glFlush();
    fn glGetError() -> u32;
    fn glIsTexture(name: u32) -> u8;
    fn glBindSampler(unit: u32, sampler: u32);
    fn glReadPixels(
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        format: u32,
        kind: u32,
        data: *mut c_void,
    );
    fn glGetIntegerv(name: u32, value: *mut i32);
    fn glGetStringi(name: u32, index: u32) -> *const u8;
}
