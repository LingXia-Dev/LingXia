//! The renderer's video players. A `Paint::Video` key opens one platform player off the render
//! thread; the render thread imports each new frame as an external texture, with no CPU copy.
//! Players nothing drew for 2 s stop and close on the opener thread.
use crate::scene::paint::Paint;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard, mpsc};
use std::time::{Duration, Instant};

/// A player not drawn for this long stops decoding and closes.
const IDLE: Duration = Duration::from_secs(2);

/// Bumped when a frame size changes: cached scenes encode the video's source region.
static EPOCH: AtomicU64 = AtomicU64::new(0);

pub(crate) fn epoch() -> u64 {
    EPOCH.load(Ordering::Relaxed)
}

/// High bit set: image texture ids are pixmap addresses, which never have it.
pub(crate) fn texture_id(player: &str) -> vello_gpu::TextureId {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    player.hash(&mut hasher);
    vello_gpu::TextureId(hasher.finish() | (1 << 63))
}

/// A platform player the renderer draws from: it decodes on its own and hands over its newest
/// frame as a texture of the renderer's device.
pub trait Player: Send {
    /// A frame newer than the last one taken is due at this refresh.
    fn has_new(&self) -> bool;
    /// The frame due now, when it is new.
    fn next_frame(
        &mut self,
        device: &wgpu::Device,
    ) -> Result<Option<(wgpu::Texture, [u16; 2])>, String>;
}

/// Opens the player for a source URL.
pub type Opener = fn(&str) -> Result<Box<dyn Player>, String>;

static INSTALLED: std::sync::OnceLock<Opener> = std::sync::OnceLock::new();

/// Installs the platform's players, for platforms whose decoders live outside this crate.
pub fn install(opener: Opener) {
    let _ = INSTALLED.set(opener);
}

fn open(source: &str) -> Result<Box<dyn Player>, String> {
    if let Some(opener) = INSTALLED.get() {
        return opener(source);
    }
    platform::open(source)
}

enum State {
    Opening,
    Ready(Box<dyn Player>),
    Failed,
}

struct Frame {
    view: wgpu::TextureView,
    size: [u16; 2],
}

struct Entry {
    state: State,
    frame: Option<Frame>,
    drawn: Instant,
    requested: Instant,
    /// The first frame's import, until a present shows it.
    unshown: Option<Instant>,
    first_frame_ms: Option<u64>,
    first_present_us: Option<u64>,
    frames: u64,
    import_us: [u64; 2],
}

static PLAYERS: LazyLock<Mutex<HashMap<String, Entry>>> = LazyLock::new(Default::default);

fn players() -> MutexGuard<'static, HashMap<String, Entry>> {
    PLAYERS.lock().unwrap_or_else(|p| p.into_inner())
}

/// Redraws without the worker: count, skipped for want of a new frame, then total and max µs of
/// render, present, scene, images, encode and submit.
static REBINDS: Mutex<[u64; 14]> = Mutex::new([0; 14]);

/// Opens players and closes idle ones, so neither stalls a render.
static OPENER: LazyLock<Mutex<mpsc::Sender<String>>> = LazyLock::new(|| {
    let (sender, receiver) = mpsc::channel::<String>();
    let spawned = std::thread::Builder::new()
        .name("lingxia-video".into())
        .spawn(move || {
            loop {
                match receiver.recv_timeout(Duration::from_millis(500)) {
                    Ok(key) => {
                        let state = match open(&key) {
                            Ok(player) => State::Ready(player),
                            Err(error) => {
                                log::warn!(target: "lingxia_view", "UI_VIDEO: {key}: {error}");
                                State::Failed
                            }
                        };
                        // Evicted while opening: the player closes here, unused.
                        let _unused = match players().get_mut(&key) {
                            Some(entry) => std::mem::replace(&mut entry.state, state),
                            None => state,
                        };
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
                let idle: Vec<Entry> = {
                    let mut players = players();
                    let keys: Vec<String> = players
                        .iter()
                        .filter(|(_, entry)| entry.drawn.elapsed() > IDLE)
                        .map(|(key, _)| key.clone())
                        .collect();
                    keys.iter().filter_map(|key| players.remove(key)).collect()
                };
                drop(idle);
            }
        });
    if let Err(error) = spawned {
        log::error!(target: "lingxia_view", "video opener did not start: {error}");
    }
    Mutex::new(sender)
});

fn keys(display: &crate::DisplayList) -> Vec<&str> {
    let mut keys: Vec<&str> = display
        .commands
        .iter()
        .filter_map(|paint| match paint {
            Paint::Video { player, .. } => Some(player.as_str()),
            _ => None,
        })
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

pub(crate) fn draws_video(display: &crate::DisplayList) -> bool {
    display
        .commands
        .iter()
        .any(|paint| matches!(paint, Paint::Video { .. }))
}

/// The size of the newest frame of `player`, once one is bound.
pub(crate) fn frame_size(player: &str) -> Option<[u16; 2]> {
    players()
        .get(player)?
        .frame
        .as_ref()
        .map(|frame| frame.size)
}

/// Marks the players of `display` drawn, asking for those not open yet; true when one of them
/// has a frame newer than the one bound.
pub(crate) fn has_new(display: &crate::DisplayList) -> bool {
    let mut players = players();
    let mut new = false;
    for key in keys(display) {
        if let Some(Entry {
            state: State::Ready(player),
            ..
        }) = touch(&mut players, key)
        {
            new |= player.has_new();
        }
    }
    new
}

fn touch<'a>(players: &'a mut HashMap<String, Entry>, key: &str) -> Option<&'a mut Entry> {
    if !players.contains_key(key) {
        let now = Instant::now();
        let sent = OPENER
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .send(key.to_owned());
        players.insert(
            key.to_owned(),
            Entry {
                state: if sent.is_ok() {
                    State::Opening
                } else {
                    State::Failed
                },
                frame: None,
                drawn: now,
                requested: now,
                unshown: None,
                first_frame_ms: None,
                first_present_us: None,
                frames: 0,
                import_us: [0; 2],
            },
        );
        return None;
    }
    let entry = players.get_mut(key)?;
    entry.drawn = Instant::now();
    Some(entry)
}

/// Imports the frame due now of every player `display` draws; true when it draws a video.
pub(crate) fn import(device: &wgpu::Device, display: &crate::DisplayList) -> bool {
    let keys = keys(display);
    let mut players = players();
    for key in &keys {
        let Some(entry) = touch(&mut players, key) else {
            continue;
        };
        let State::Ready(player) = &mut entry.state else {
            continue;
        };
        let started = Instant::now();
        match player.next_frame(device) {
            Ok(Some((texture, size))) => {
                if entry.frame.as_ref().is_none_or(|frame| frame.size != size) {
                    EPOCH.fetch_add(1, Ordering::Relaxed);
                }
                if entry.frame.is_none() {
                    entry.first_frame_ms = Some(entry.requested.elapsed().as_millis() as u64);
                    entry.unshown = Some(Instant::now());
                }
                entry.frame = Some(Frame {
                    view: texture.create_view(&Default::default()),
                    size,
                });
                let us = crate::session::metrics::micros(started);
                entry.frames += 1;
                entry.import_us = [entry.import_us[0] + us, entry.import_us[1].max(us)];
            }
            Ok(None) => {}
            Err(error) => log::warn!(target: "lingxia_view", "UI_VIDEO: {key}: {error}"),
        }
    }
    !keys.is_empty()
}

/// Binds each player's newest frame under its texture id.
pub(crate) fn bind(display: &crate::DisplayList, bindings: &mut vello_gpu::TextureBindings) {
    let players = players();
    for key in keys(display) {
        if let Some(frame) = players.get(key).and_then(|entry| entry.frame.as_ref()) {
            bindings.insert(texture_id(key), frame.view.clone());
        }
    }
}

/// A frame drawing video reached the screen: first frames imported before it are now shown.
pub(crate) fn presented() {
    for entry in players().values_mut() {
        if let Some(imported) = entry.unshown.take() {
            entry.first_present_us = Some(crate::session::metrics::micros(imported));
        }
    }
}

pub(crate) fn note_rebind(drawn: Option<[u64; 6]>) {
    let mut rebinds = REBINDS.lock().unwrap_or_else(|p| p.into_inner());
    match drawn {
        Some(times) => {
            rebinds[0] += 1;
            for (index, us) in times.into_iter().enumerate() {
                rebinds[2 + index * 2] += us;
                rebinds[3 + index * 2] = rebinds[3 + index * 2].max(us);
            }
        }
        None => rebinds[1] += 1,
    }
}

/// Every eighth redraw is timed on the GPU.
pub(crate) fn sample_due() -> bool {
    static COUNT: AtomicU64 = AtomicU64::new(0);
    COUNT.fetch_add(1, Ordering::Relaxed).is_multiple_of(8)
}

/// Per-player first-frame and import costs, and the cost of redraws for new frames.
pub fn stats() -> serde_json::Value {
    let players: Vec<_> = players()
        .iter()
        .map(|(key, entry)| {
            serde_json::json!({
                "player": key,
                "state": match entry.state {
                    State::Opening => "opening",
                    State::Ready(_) => "ready",
                    State::Failed => "failed",
                },
                "firstFrameMs": entry.first_frame_ms,
                "firstPresentUs": entry.first_present_us,
                "frames": entry.frames,
                "importUsAvg": entry.import_us[0].checked_div(entry.frames),
                "importUsMax": entry.import_us[1],
                "size": entry.frame.as_ref().map(|frame| frame.size),
            })
        })
        .collect();
    let r = *REBINDS.lock().unwrap_or_else(|p| p.into_inner());
    let avg = |index: usize| r[2 + index * 2].checked_div(r[0]);
    let max = |index: usize| r[3 + index * 2];
    serde_json::json!({
        "players": players,
        "rebinds": r[0],
        "rebindsSkipped": r[1],
        "rebindUs": {
            "renderAvg": avg(0), "renderMax": max(0),
            "presentAvg": avg(1), "presentMax": max(1),
            "sceneAvg": avg(2), "imagesAvg": avg(3), "encodeAvg": avg(4), "submitAvg": avg(5),
        },
        "gpuUs": super::timing::stats(),
    })
}

#[cfg(target_vendor = "apple")]
mod platform {
    //! AVPlayer → AVPlayerItemVideoOutput (BGRA, Metal-compatible) → CVMetalTextureCache.
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2_metal::{MTLTexture, MTLTextureType};
    use std::ffi::{c_char, c_void};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CMTime {
        value: i64,
        timescale: i32,
        flags: u32,
        epoch: i64,
    }

    type Id = *mut c_void;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_msgSend();
        #[cfg(target_arch = "x86_64")]
        fn objc_msgSend_stret();
        fn sel_registerName(name: *const c_char) -> *const c_void;
        fn objc_getClass(name: *const c_char) -> Id;
        fn objc_retain(object: Id) -> Id;
        fn objc_release(object: Id);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFBooleanTrue: *const c_void;
        static kCFTypeDictionaryKeyCallBacks: c_void;
        static kCFTypeDictionaryValueCallBacks: c_void;
        fn CFNumberCreate(alloc: *const c_void, kind: isize, value: *const c_void)
        -> *const c_void;
        fn CFDictionaryCreate(
            alloc: *const c_void,
            keys: *const *const c_void,
            values: *const *const c_void,
            count: isize,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> *const c_void;
        fn CFRelease(object: *const c_void);
    }

    #[link(name = "CoreVideo", kind = "framework")]
    unsafe extern "C" {
        static kCVPixelBufferPixelFormatTypeKey: *const c_void;
        static kCVPixelBufferMetalCompatibilityKey: *const c_void;
        fn CVMetalTextureCacheCreate(
            alloc: *const c_void,
            attributes: *const c_void,
            device: Id,
            texture_attributes: *const c_void,
            cache: *mut *mut c_void,
        ) -> i32;
        fn CVMetalTextureCacheCreateTextureFromImage(
            alloc: *const c_void,
            cache: *mut c_void,
            image: *mut c_void,
            attributes: *const c_void,
            format: usize,
            width: usize,
            height: usize,
            plane: usize,
            texture: *mut *mut c_void,
        ) -> i32;
        fn CVMetalTextureGetTexture(texture: *mut c_void) -> Id;
        fn CVPixelBufferGetWidth(buffer: *mut c_void) -> usize;
        fn CVPixelBufferGetHeight(buffer: *mut c_void) -> usize;
        fn CVBufferRelease(buffer: *mut c_void);
    }

    #[link(name = "CoreMedia", kind = "framework")]
    unsafe extern "C" {
        static kCMTimeZero: CMTime;
        fn CMTimeGetSeconds(time: CMTime) -> f64;
    }

    #[link(name = "AVFoundation", kind = "framework")]
    unsafe extern "C" {}

    #[link(name = "QuartzCore", kind = "framework")]
    unsafe extern "C" {
        fn CACurrentMediaTime() -> f64;
    }

    const BGRA: i32 = 0x4247_5241;
    const MTL_PIXEL_FORMAT_BGRA8_UNORM: usize = 80;

    fn sel(name: &std::ffi::CStr) -> *const c_void {
        unsafe { sel_registerName(name.as_ptr()) }
    }

    unsafe fn send0(object: Id, name: &std::ffi::CStr) -> Id {
        let f: unsafe extern "C" fn(Id, *const c_void) -> Id =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(object, sel(name)) }
    }

    unsafe fn send1(object: Id, name: &std::ffi::CStr, arg: Id) -> Id {
        let f: unsafe extern "C" fn(Id, *const c_void, Id) -> Id =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        unsafe { f(object, sel(name), arg) }
    }

    /// A message returning a CMTime: x86-64 returns 24-byte structs through `_stret`.
    unsafe fn send_time(object: Id, name: &std::ffi::CStr, host: Option<f64>) -> CMTime {
        #[cfg(target_arch = "x86_64")]
        let send = objc_msgSend_stret as unsafe extern "C" fn();
        #[cfg(not(target_arch = "x86_64"))]
        let send = objc_msgSend as unsafe extern "C" fn();
        unsafe {
            match host {
                Some(host) => {
                    let f: unsafe extern "C" fn(Id, *const c_void, f64) -> CMTime =
                        std::mem::transmute(send);
                    f(object, sel(name), host)
                }
                None => {
                    let f: unsafe extern "C" fn(Id, *const c_void) -> CMTime =
                        std::mem::transmute(send);
                    f(object, sel(name))
                }
            }
        }
    }

    pub(super) struct Player {
        player: Id,
        item: Id,
        output: Id,
        cache: *mut c_void,
    }

    // AVPlayer and the video output are thread-safe for these calls; the registry serializes them.
    unsafe impl Send for Player {}

    struct Held(*mut c_void);
    unsafe impl Send for Held {}
    unsafe impl Sync for Held {}

    pub(super) fn open(url: &str) -> Result<Box<dyn super::Player>, String> {
        Ok(Box::new(Player::open(url)?))
    }

    impl super::Player for Player {
        fn has_new(&self) -> bool {
            unsafe { self.due().is_some() }
        }

        fn next_frame(
            &mut self,
            device: &wgpu::Device,
        ) -> Result<Option<(wgpu::Texture, [u16; 2])>, String> {
            self.frame(device)
        }
    }

    impl Player {
        fn open(url: &str) -> Result<Self, String> {
            let url = std::ffi::CString::new(url).map_err(|e| e.to_string())?;
            unsafe {
                let string = {
                    let f: unsafe extern "C" fn(Id, *const c_void, *const c_char) -> Id =
                        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                    f(
                        objc_getClass(c"NSString".as_ptr()),
                        sel(c"stringWithUTF8String:"),
                        url.as_ptr(),
                    )
                };
                let nsurl = send1(objc_getClass(c"NSURL".as_ptr()), c"URLWithString:", string);
                if nsurl.is_null() {
                    return Err("invalid URL".into());
                }
                let item = objc_retain(send1(
                    objc_getClass(c"AVPlayerItem".as_ptr()),
                    c"playerItemWithURL:",
                    nsurl,
                ));
                let format = CFNumberCreate(std::ptr::null(), 3, (&BGRA as *const i32).cast());
                let keys = [
                    kCVPixelBufferPixelFormatTypeKey,
                    kCVPixelBufferMetalCompatibilityKey,
                ];
                let values = [format, kCFBooleanTrue];
                let attributes = CFDictionaryCreate(
                    std::ptr::null(),
                    keys.as_ptr(),
                    values.as_ptr(),
                    2,
                    (&raw const kCFTypeDictionaryKeyCallBacks).cast(),
                    (&raw const kCFTypeDictionaryValueCallBacks).cast(),
                );
                CFRelease(format);
                let output = send1(
                    send0(objc_getClass(c"AVPlayerItemVideoOutput".as_ptr()), c"alloc"),
                    c"initWithPixelBufferAttributes:",
                    attributes as Id,
                );
                CFRelease(attributes);
                send1(item, c"addOutput:", output);
                let player = objc_retain(send1(
                    objc_getClass(c"AVPlayer".as_ptr()),
                    c"playerWithPlayerItem:",
                    item,
                ));
                let set_bool: unsafe extern "C" fn(Id, *const c_void, bool) =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                // Muted until the element's audio policy lands: it never takes the audio session.
                set_bool(player, sel(c"setMuted:"), true);
                let set_int: unsafe extern "C" fn(Id, *const c_void, isize) =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                set_int(player, sel(c"setActionAtItemEnd:"), 2);
                send0(player, c"play");
                Ok(Self {
                    player,
                    item,
                    output,
                    cache: std::ptr::null_mut(),
                })
            }
        }

        /// The frame due at this refresh as a wgpu texture, when a new one is ready.
        fn frame(
            &mut self,
            device: &wgpu::Device,
        ) -> Result<Option<(wgpu::Texture, [u16; 2])>, String> {
            unsafe {
                let Some(time) = self.due() else {
                    return Ok(None);
                };
                let copy: unsafe extern "C" fn(Id, *const c_void, CMTime, *mut CMTime) -> Id =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                let buffer = copy(
                    self.output,
                    sel(c"copyPixelBufferForItemTime:itemTimeForDisplay:"),
                    time,
                    std::ptr::null_mut(),
                );
                if buffer.is_null() {
                    return Ok(None);
                }
                let result = self.import(device, buffer);
                CVBufferRelease(buffer);
                result.map(Some)
            }
        }

        /// The item time of the refresh now, when it has a new frame.
        unsafe fn due(&self) -> Option<CMTime> {
            unsafe {
                self.restart_at_end();
                let time = send_time(
                    self.output,
                    c"itemTimeForHostTime:",
                    Some(CACurrentMediaTime()),
                );
                let has_new: unsafe extern "C" fn(Id, *const c_void, CMTime) -> bool =
                    std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                has_new(self.output, sel(c"hasNewPixelBufferForItemTime:"), time).then_some(time)
            }
        }

        unsafe fn restart_at_end(&self) {
            unsafe {
                let duration = CMTimeGetSeconds(send_time(self.item, c"duration", None));
                let now = CMTimeGetSeconds(send_time(self.item, c"currentTime", None));
                if duration.is_finite() && duration > 0.0 && now >= duration - 0.01 {
                    let seek: unsafe extern "C" fn(Id, *const c_void, CMTime) =
                        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
                    seek(self.player, sel(c"seekToTime:"), kCMTimeZero);
                }
            }
        }

        unsafe fn import(
            &mut self,
            device: &wgpu::Device,
            buffer: Id,
        ) -> Result<(wgpu::Texture, [u16; 2]), String> {
            unsafe {
                let hal = device
                    .as_hal::<wgpu::hal::api::Metal>()
                    .ok_or("not a Metal device")?;
                if self.cache.is_null() {
                    let raw: *const ProtocolObject<dyn objc2_metal::MTLDevice> =
                        &**hal.raw_device();
                    let status = CVMetalTextureCacheCreate(
                        std::ptr::null(),
                        std::ptr::null(),
                        raw as Id,
                        std::ptr::null(),
                        &mut self.cache,
                    );
                    if status != 0 {
                        return Err(format!("CVMetalTextureCacheCreate {status}"));
                    }
                }
                let (width, height) = (
                    CVPixelBufferGetWidth(buffer),
                    CVPixelBufferGetHeight(buffer),
                );
                let mut metal = std::ptr::null_mut();
                let status = CVMetalTextureCacheCreateTextureFromImage(
                    std::ptr::null(),
                    self.cache,
                    buffer,
                    std::ptr::null(),
                    MTL_PIXEL_FORMAT_BGRA8_UNORM,
                    width,
                    height,
                    0,
                    &mut metal,
                );
                if status != 0 || metal.is_null() {
                    return Err(format!(
                        "CVMetalTextureCacheCreateTextureFromImage {status}"
                    ));
                }
                let raw = CVMetalTextureGetTexture(metal);
                let texture: Retained<ProtocolObject<dyn MTLTexture>> =
                    Retained::retain(raw.cast()).ok_or("no Metal texture")?;
                // The CVMetalTexture keeps the IOSurface checked out until wgpu is done with it.
                let held = Held(metal);
                let hal_texture = wgpu::hal::metal::Device::texture_from_raw(
                    texture,
                    wgpu::TextureFormat::Bgra8Unorm,
                    MTLTextureType::Type2D,
                    1,
                    1,
                    wgpu::hal::CopyExtent {
                        width: width as u32,
                        height: height as u32,
                        depth: 1,
                    },
                    Some(Box::new(move || {
                        let held = held;
                        CFRelease(held.0);
                    })),
                );
                drop(hal);
                let texture = device.create_texture_from_hal::<wgpu::hal::api::Metal>(
                    hal_texture,
                    &wgpu::TextureDescriptor {
                        label: Some("LingXia video frame"),
                        size: wgpu::Extent3d {
                            width: width as u32,
                            height: height as u32,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Bgra8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    },
                    wgpu::TextureUses::RESOURCE,
                );
                Ok((
                    texture,
                    [
                        width.min(u16::MAX as usize) as u16,
                        height.min(u16::MAX as usize) as u16,
                    ],
                ))
            }
        }
    }

    impl Drop for Player {
        fn drop(&mut self) {
            unsafe {
                send0(self.player, c"pause");
                objc_release(self.output);
                objc_release(self.player);
                objc_release(self.item);
                if !self.cache.is_null() {
                    CFRelease(self.cache);
                }
            }
        }
    }
}

/// Imports an RGBA8 AHardwareBuffer as a texture of `device`, sharing its memory.
///
/// # Safety
/// `buffer` must be a live `AHardwareBuffer*` allocated with GPU sampling usage; the texture
/// holds its own reference.
#[cfg(target_os = "android")]
pub unsafe fn import_hardware_buffer(
    device: &wgpu::Device,
    buffer: *mut std::ffi::c_void,
    size: [u32; 2],
) -> Result<wgpu::Texture, String> {
    use ash::{android::external_memory_android_hardware_buffer as ahb, vk};
    unsafe {
        let hal = device
            .as_hal::<wgpu::hal::api::Vulkan>()
            .ok_or("not a Vulkan device")?;
        let instance = hal.shared_instance().raw_instance();
        let raw = hal.raw_device();
        let loader = ahb::Device::new(instance, raw);
        let mut properties = vk::AndroidHardwareBufferPropertiesANDROID::default();
        loader
            .get_android_hardware_buffer_properties(buffer.cast(), &mut properties)
            .map_err(|e| format!("AHardwareBuffer properties: {e}"))?;
        let mut external = vk::ExternalMemoryImageCreateInfo::default()
            .handle_types(vk::ExternalMemoryHandleTypeFlags::ANDROID_HARDWARE_BUFFER_ANDROID);
        let info = vk::ImageCreateInfo::default()
            .push_next(&mut external)
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_UNORM)
            .extent(vk::Extent3D {
                width: size[0],
                height: size[1],
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        let image = raw
            .create_image(&info, None)
            .map_err(|e| format!("vkCreateImage: {e}"))?;
        let mut import =
            vk::ImportAndroidHardwareBufferInfoANDROID::default().buffer(buffer.cast());
        let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let allocate = vk::MemoryAllocateInfo::default()
            .push_next(&mut import)
            .push_next(&mut dedicated)
            .allocation_size(properties.allocation_size)
            .memory_type_index(properties.memory_type_bits.trailing_zeros());
        let memory = match raw.allocate_memory(&allocate, None) {
            Ok(memory) => memory,
            Err(e) => {
                raw.destroy_image(image, None);
                return Err(format!("vkAllocateMemory: {e}"));
            }
        };
        if let Err(e) = raw.bind_image_memory(image, memory, 0) {
            raw.free_memory(memory, None);
            raw.destroy_image(image, None);
            return Err(format!("vkBindImageMemory: {e}"));
        }
        let extent = wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        };
        let hal_texture = hal.texture_from_raw(
            image,
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
            wgpu::hal::vulkan::TextureMemory::Dedicated(memory),
        );
        drop(hal);
        Ok(device.create_texture_from_hal::<wgpu::hal::api::Vulkan>(
            hal_texture,
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
        ))
    }
}

#[cfg(not(target_vendor = "apple"))]
mod platform {
    pub(super) fn open(_url: &str) -> Result<Box<dyn super::Player>, String> {
        Err("no video player is installed on this platform".into())
    }
}
