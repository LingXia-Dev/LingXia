//! Media Foundation's media engine for the GPU renderer's `<video>`: it decodes on its own
//! threads, on a D3D11 device of its own, into a texture each new frame is copied from.
use lingxia_view::renderer::gpu::hls;
use lingxia_view::renderer::gpu::video::{Player, install as install_opener};
use lingxia_view::renderer::gpu::wgpu;
use lingxia_view::scene::video::{Settings, Status, VideoError, VideoOp};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Arc, LazyLock, Mutex, Once};
use windows::Win32::Foundation::{HMODULE, RECT, S_OK};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING, D3D11CreateDevice,
    ID3D11Device, ID3D11DeviceContext, ID3D11Multithread, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Media::MediaFoundation::{
    CLSID_MFMediaEngineClassFactory, IMFAttributes, IMFDXGIDeviceManager, IMFMediaEngine,
    IMFMediaEngineClassFactory, IMFMediaEngineNotify, IMFMediaEngineNotify_Impl,
    MF_MEDIA_ENGINE_CALLBACK, MF_MEDIA_ENGINE_DXGI_MANAGER, MF_MEDIA_ENGINE_EVENT_CANPLAY,
    MF_MEDIA_ENGINE_EVENT_ENDED, MF_MEDIA_ENGINE_EVENT_ERROR,
    MF_MEDIA_ENGINE_EVENT_FIRSTFRAMEREADY, MF_MEDIA_ENGINE_EVENT_PLAYING,
    MF_MEDIA_ENGINE_EVENT_SEEKED, MF_MEDIA_ENGINE_EVENT_SEEKING, MF_MEDIA_ENGINE_EVENT_WAITING,
    MF_MEDIA_ENGINE_PRELOAD_AUTOMATIC, MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT, MF_VERSION,
    MFCreateAttributes, MFCreateDXGIDeviceManager, MFSTARTUP_FULL, MFStartup,
};
use windows::Win32::System::Com::CoInitializeEx;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance};
use windows::core::{BSTR, Interface, implement};

pub(crate) fn install() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| install_opener(open));
}

/// Master playlists' renditions by URL: the engine lists none and pins none itself.
static MANIFESTS: LazyLock<Mutex<HashMap<String, Vec<hls::Rendition>>>> =
    LazyLock::new(Default::default);

/// Records the playlist read for `url`; players of it list and pin its renditions from now on.
pub(crate) fn store_manifest(url: &str, playlist: &str) {
    let renditions = hls::renditions(playlist);
    manifests().insert(url.to_owned(), renditions);
}

pub(crate) fn has_manifest(url: &str) -> bool {
    manifests().contains_key(url)
}

fn manifests() -> std::sync::MutexGuard<'static, HashMap<String, Vec<hls::Rendition>>> {
    MANIFESTS.lock().unwrap_or_else(|p| p.into_inner())
}

fn open(url: &str) -> Result<Box<dyn Player>, String> {
    Video::open(url)
        .map(|video| Box::new(video) as Box<dyn Player>)
        .map_err(|error| format!("VIDEO_NO_PLAYER: {error}"))
}

/// The engine is free-threaded; each thread that calls it joins the MTA once.
fn join_mta() {
    thread_local! {
        static JOINED: () = {
            // SAFETY: no arguments; a thread already in an apartment keeps it.
            let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        };
    }
    JOINED.with(|_| ());
}

fn startup() -> windows::core::Result<()> {
    static STARTED: std::sync::OnceLock<windows::core::Result<()>> = std::sync::OnceLock::new();
    STARTED
        // SAFETY: once per process; never shut down, as players open until exit.
        .get_or_init(|| unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) })
        .clone()
}

/// What the engine reports through its callback, on its own threads.
#[derive(Default)]
struct Events {
    /// An `MF_MEDIA_ENGINE_ERR` code, 0 for none, and its HRESULT.
    error: AtomicU32,
    hresult: AtomicI32,
    ended: AtomicBool,
    waiting: AtomicBool,
    /// A frame to show though no stream tick reports it: the first one, or a paused seek's.
    still: AtomicBool,
}

#[implement(IMFMediaEngineNotify)]
struct Notify(Arc<Events>);

impl IMFMediaEngineNotify_Impl for Notify_Impl {
    fn EventNotify(&self, event: u32, param1: usize, param2: u32) -> windows::core::Result<()> {
        let events = &self.0;
        match event as i32 {
            e if e == MF_MEDIA_ENGINE_EVENT_ERROR.0 => {
                events.hresult.store(param2 as i32, Ordering::Release);
                events.error.store(param1 as u32, Ordering::Release);
            }
            e if e == MF_MEDIA_ENGINE_EVENT_ENDED.0 => events.ended.store(true, Ordering::Release),
            e if e == MF_MEDIA_ENGINE_EVENT_SEEKING.0 => {
                events.ended.store(false, Ordering::Release)
            }
            e if e == MF_MEDIA_ENGINE_EVENT_FIRSTFRAMEREADY.0
                || e == MF_MEDIA_ENGINE_EVENT_SEEKED.0 =>
            {
                events.still.store(true, Ordering::Release)
            }
            e if e == MF_MEDIA_ENGINE_EVENT_WAITING.0 => {
                events.waiting.store(true, Ordering::Release)
            }
            e if e == MF_MEDIA_ENGINE_EVENT_PLAYING.0 || e == MF_MEDIA_ENGINE_EVENT_CANPLAY.0 => {
                events.ended.store(false, Ordering::Release);
                events.waiting.store(false, Ordering::Release);
            }
            _ => {}
        }
        Ok(())
    }
}

/// The texture the engine draws a frame into, and the one the CPU reads it back from.
struct Target {
    size: [u32; 2],
    drawn: ID3D11Texture2D,
    read: ID3D11Texture2D,
}

struct Video {
    engine: IMFMediaEngine,
    events: Arc<Events>,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    _manager: IMFDXGIDeviceManager,
    target: Option<Target>,
    /// Rewritten each frame: wgpu orders the write after draws that read the last one.
    texture: Option<(wgpu::Texture, [u32; 2])>,
    /// The engine said a new frame is due and it is not taken yet.
    due: Cell<bool>,
    rate: f32,
    /// Where a pause stopped the clock: the engine pauses asynchronously and its time runs on
    /// for a few frames after `IsPaused` turns true.
    paused_at: Cell<Option<f64>>,
    /// The source as opened, and the URL now playing: a pinned rendition's playlist.
    master: String,
    playing: String,
}

// SAFETY: the media engine is free-threaded and the D3D11 device multithread-protected; the
// registry calls one player from one thread at a time.
unsafe impl Send for Video {}

impl Video {
    fn open(url: &str) -> windows::core::Result<Self> {
        join_mta();
        startup()?;
        // SAFETY: plain COM and D3D11 creation; every out-pointer is checked.
        unsafe {
            let (mut device, mut context) = (None, None);
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
            let device: ID3D11Device = device.ok_or_else(windows::core::Error::empty)?;
            let context: ID3D11DeviceContext = context.ok_or_else(windows::core::Error::empty)?;
            // The engine's decoder and this player's copies share the immediate context.
            let _ = device
                .cast::<ID3D11Multithread>()?
                .SetMultithreadProtected(true);
            let mut token = 0;
            let mut manager = None;
            MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
            let manager: IMFDXGIDeviceManager = manager.ok_or_else(windows::core::Error::empty)?;
            manager.ResetDevice(&device, token)?;
            let events = Arc::new(Events::default());
            let notify: IMFMediaEngineNotify = Notify(events.clone()).into();
            let mut attributes: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attributes, 3)?;
            let attributes = attributes.ok_or_else(windows::core::Error::empty)?;
            attributes.SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, &notify)?;
            attributes.SetUnknown(&MF_MEDIA_ENGINE_DXGI_MANAGER, &manager)?;
            attributes.SetUINT32(
                &MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT,
                DXGI_FORMAT_B8G8R8A8_UNORM.0 as u32,
            )?;
            let factory: IMFMediaEngineClassFactory =
                CoCreateInstance(&CLSID_MFMediaEngineClassFactory, None, CLSCTX_INPROC_SERVER)?;
            let engine = factory.CreateInstance(0, &attributes)?;
            engine.SetPreload(MF_MEDIA_ENGINE_PRELOAD_AUTOMATIC)?;
            // Paused on the first frame until the page's props say play.
            engine.SetAutoPlay(false)?;
            engine.SetSource(&BSTR::from(url))?;
            Ok(Self {
                engine,
                events,
                device,
                context,
                _manager: manager,
                target: None,
                texture: None,
                due: Cell::new(false),
                rate: 1.0,
                paused_at: Cell::new(None),
                master: url.to_owned(),
                playing: url.to_owned(),
            })
        }
    }

    /// Whether a frame newer than the last one shown is due now.
    fn tick(&self) -> bool {
        if self.due.get() {
            return true;
        }
        let mut pts = 0i64;
        // SAFETY: the engine's own vtable; S_FALSE (no new frame) is a success the projected
        // method would not tell apart.
        let result = unsafe {
            (Interface::vtable(&self.engine).OnVideoStreamTick)(
                Interface::as_raw(&self.engine),
                &mut pts,
            )
        };
        // A paused engine reports no tick for the frame it shows on load or after a seek.
        let due = result == S_OK || self.events.still.swap(false, Ordering::AcqRel);
        self.due.set(due);
        due
    }

    fn target(&mut self, size: [u32; 2]) -> windows::core::Result<&Target> {
        if self.target.as_ref().is_none_or(|t| t.size != size) {
            let mut desc = D3D11_TEXTURE2D_DESC {
                Width: size[0],
                Height: size[1],
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let (mut drawn, mut read) = (None, None);
            // SAFETY: descriptions of plain 2D textures on this player's device.
            unsafe {
                self.device.CreateTexture2D(&desc, None, Some(&mut drawn))?;
                desc.Usage = D3D11_USAGE_STAGING;
                desc.BindFlags = 0;
                desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                self.device.CreateTexture2D(&desc, None, Some(&mut read))?;
            }
            self.target = Some(Target {
                size,
                drawn: drawn.ok_or_else(windows::core::Error::empty)?,
                read: read.ok_or_else(windows::core::Error::empty)?,
            });
        }
        Ok(self.target.as_ref().expect("made above"))
    }

    fn frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> windows::core::Result<Option<(wgpu::Texture, [u16; 2])>> {
        if !self.tick() {
            return Ok(None);
        }
        self.due.set(false);
        let (mut width, mut height) = (0u32, 0u32);
        // SAFETY: out-pointers to locals.
        unsafe {
            self.engine
                .GetNativeVideoSize(Some(&mut width), Some(&mut height))?
        };
        if width == 0 || height == 0 || width > u32::from(u16::MAX) || height > u32::from(u16::MAX)
        {
            return Ok(None);
        }
        let size = [width, height];
        let engine = self.engine.clone();
        let context = self.context.clone();
        let target = self.target(size)?;
        let (drawn, read) = (target.drawn.clone(), target.read.clone());
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: textures of this player's device; the mapping is read before it is released.
        unsafe {
            let rect = RECT {
                left: 0,
                top: 0,
                right: width as i32,
                bottom: height as i32,
            };
            engine.TransferVideoFrame(&drawn, None, &rect, None)?;
            context.CopyResource(&read, &drawn);
            context.Map(&read, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        }
        let texture = match &self.texture {
            Some((texture, at)) if *at == size => texture.clone(),
            _ => {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("LingXia video frame"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Bgra8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                self.texture = Some((texture.clone(), size));
                texture
            }
        };
        // SAFETY: the mapping holds `RowPitch` bytes for each of `height` rows until unmapped.
        let bytes = unsafe {
            std::slice::from_raw_parts(
                mapped.pData.cast::<u8>(),
                mapped.RowPitch as usize * height as usize,
            )
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(mapped.RowPitch),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        // SAFETY: the mapping made above.
        unsafe { context.Unmap(&read, 0) };
        Ok(Some((texture, [width as u16, height as u16])))
    }

    fn play(&self) {
        // SAFETY: plain engine calls.
        unsafe {
            if self.engine.IsEnded().as_bool() && !self.engine.GetLoop().as_bool() {
                let _ = self.engine.SetCurrentTime(0.0);
            }
            self.paused_at.set(None);
            let _ = self.engine.Play();
        }
    }

    fn pause(&self) {
        // SAFETY: plain engine calls.
        unsafe {
            if !self.engine.IsPaused().as_bool() {
                self.paused_at
                    .set(Some(self.engine.GetCurrentTime().max(0.0)));
            }
            let _ = self.engine.Pause();
        }
    }

    fn renditions(&self) -> Vec<hls::Rendition> {
        manifests().get(&self.master).cloned().unwrap_or_default()
    }

    /// Plays `url` from where the current one is, as it was playing or paused.
    fn reopen(&mut self, url: String) -> windows::core::Result<()> {
        if url == self.playing {
            return Ok(());
        }
        // SAFETY: plain engine calls.
        unsafe {
            let paused = self.engine.IsPaused().as_bool();
            let at = match self.paused_at.get() {
                Some(at) if paused => at,
                _ => self.engine.GetCurrentTime(),
            };
            self.engine.SetSource(&BSTR::from(url.as_str()))?;
            if at > 0.0 {
                self.engine.SetCurrentTime(at)?;
            }
            if !paused {
                self.engine.Play()?;
            }
        }
        self.playing = url;
        Ok(())
    }

    fn set_rate(&mut self, rate: f32) {
        self.rate = rate;
        // SAFETY: a plain engine call.
        let _ = unsafe { self.engine.SetPlaybackRate(f64::from(rate)) };
    }

    fn error(&self) -> Option<VideoError> {
        let code = self.events.error.load(Ordering::Acquire);
        if code == 0 {
            return None;
        }
        let hresult = windows::core::HRESULT(self.events.hresult.load(Ordering::Acquire));
        let message = windows::core::Error::from_hresult(hresult).message();
        const NOT_FOUND: [u32; 3] = [0x8007_0002, 0x8007_0003, 0x8019_0194];
        let kind = match code {
            2 => "VIDEO_NETWORK",
            _ if NOT_FOUND.contains(&(hresult.0 as u32)) => "VIDEO_NOT_FOUND",
            _ => "VIDEO_FORMAT",
        };
        Some(VideoError::new(
            kind,
            format!("{message} ({:#010x})", hresult.0 as u32),
        ))
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        // SAFETY: the engine is not used after this.
        let _ = unsafe { self.engine.Shutdown() };
    }
}

impl Player for Video {
    fn has_new(&self) -> bool {
        self.tick()
    }

    fn next_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Option<(wgpu::Texture, [u16; 2])>, String> {
        join_mta();
        self.frame(device, queue).map_err(|error| error.to_string())
    }

    fn apply(&mut self, settings: &Settings, previous: Option<&Settings>) {
        join_mta();
        let changed =
            |field: fn(&Settings) -> f32| previous.is_none_or(|p| field(p) != field(settings));
        // SAFETY: plain engine calls.
        unsafe {
            if changed(|s| f32::from(u8::from(s.looped))) {
                let _ = self.engine.SetLoop(settings.looped);
            }
            if changed(|s| f32::from(u8::from(s.muted))) {
                let _ = self.engine.SetMuted(settings.muted);
            }
            if changed(|s| s.volume) {
                let _ = self.engine.SetVolume(f64::from(settings.volume));
            }
        }
        if changed(|s| s.rate) {
            self.set_rate(settings.rate);
        }
        if changed(|s| f32::from(u8::from(s.paused))) {
            if settings.paused {
                self.pause();
            } else {
                self.play();
            }
        }
    }

    fn command(&mut self, op: &VideoOp) -> Result<(), VideoError> {
        join_mta();
        // SAFETY: plain engine calls.
        unsafe {
            match *op {
                VideoOp::Play => self.play(),
                VideoOp::Pause => self.pause(),
                VideoOp::Seek(seconds) => {
                    self.events.ended.store(false, Ordering::Release);
                    self.paused_at.set(None);
                    let _ = self.engine.SetCurrentTime(seconds);
                }
                VideoOp::SetMuted(muted) => {
                    let _ = self.engine.SetMuted(muted);
                }
                VideoOp::SetVolume(volume) => {
                    let _ = self.engine.SetVolume(f64::from(volume));
                }
                VideoOp::SetRate(rate) => self.set_rate(rate),
                // The registry reopens the source and switches files.
                VideoOp::Retry | VideoOp::SetQuality(_) => {}
            }
        }
        Ok(())
    }

    fn qualities(&self) -> Vec<String> {
        self.renditions().into_iter().map(|r| r.label).collect()
    }

    fn set_quality(&mut self, label: Option<&str>) -> Result<(), VideoError> {
        join_mta();
        let url = match label {
            None => self.master.clone(),
            Some(label) => {
                let rendition = self
                    .renditions()
                    .into_iter()
                    .find(|r| r.label == label)
                    .ok_or_else(|| {
                        VideoError::new("VIDEO_SOURCE", format!("no rendition {label}"))
                    })?;
                hls::resolve(&self.master, &rendition.uri)
            }
        };
        self.reopen(url)
            .map_err(|error| VideoError::new("VIDEO_NO_PLAYER", error.message()))
    }

    fn status(&self) -> Status {
        join_mta();
        // SAFETY: plain engine reads.
        unsafe {
            let duration = self.engine.GetDuration();
            let looped = self.engine.GetLoop().as_bool();
            let paused = self.engine.IsPaused().as_bool();
            let current_time = match self.paused_at.get() {
                Some(at) if paused => at,
                _ => self.engine.GetCurrentTime().max(0.0),
            };
            Status {
                current_time,
                duration: (duration.is_finite() && duration > 0.0).then_some(duration),
                paused,
                ended: !looped
                    && (self.events.ended.load(Ordering::Acquire)
                        || self.engine.IsEnded().as_bool()),
                buffering: self.events.waiting.load(Ordering::Acquire),
                seeking: self.engine.IsSeeking().as_bool(),
                playback_rate: self.rate,
                volume: self.engine.GetVolume() as f32,
                muted: self.engine.GetMuted().as_bool(),
                error: self.error(),
                ..Status::default()
            }
        }
    }
}
