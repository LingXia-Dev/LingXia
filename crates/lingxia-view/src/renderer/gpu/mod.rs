pub(crate) mod backdrop_cache;
pub(crate) mod blur;
pub mod color_filter;
pub mod composite;
mod device;
mod encode;
pub(crate) mod images;
pub(crate) mod pass;
pub(crate) mod scene;
pub(crate) mod surface;
pub(crate) mod text;

use device::{Devices, RenderSurface};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::Instant,
};

/// Device domain shared by related surfaces; page caches remain presenter-owned.
#[derive(Clone)]
pub struct Context(
    Arc<Mutex<Devices>>,
    Arc<Mutex<HashMap<usize, crate::renderer::gpu::pass::SceneRenderer>>>,
);

impl Default for Context {
    fn default() -> Self {
        Self::with_backends(wgpu::Backends::from_env().unwrap_or_else(default_backends))
    }
}

/// wgpu has no Vulkan surface for an OHOS window, only EGL; other platforms take every backend.
fn default_backends() -> wgpu::Backends {
    if cfg!(target_env = "ohos") {
        wgpu::Backends::GL
    } else {
        wgpu::Backends::default()
    }
}

impl Context {
    /// A device domain limited to `backends`, e.g. GLES when the Vulkan driver is blocked.
    pub fn with_backends(backends: wgpu::Backends) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            // Debug builds would otherwise emit driver debug groups, which corrupt the
            // stack in some GLES drivers (Maleoon); `WGPU_DEBUG`/`WGPU_VALIDATION` opt in.
            flags: wgpu::InstanceFlags::empty().with_env(),
            backend_options: wgpu::BackendOptions::from_env_or_default(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        Self(
            Arc::new(Mutex::new(Devices::new(instance))),
            Default::default(),
        )
    }

    pub(crate) fn lock(&self) -> Result<MutexGuard<'_, Devices>, String> {
        self.0
            .lock()
            .map_err(|_| "UI_SURFACE: poisoned GPU context".into())
    }

    /// Ok when some adapter offers the limits the renderer's device requests; else
    /// which limits fall short.
    fn meets_renderer_limits(&self) -> Result<(), String> {
        let adapters = {
            let context = self.lock()?;
            pollster::block_on(context.instance.enumerate_adapters(wgpu::Backends::all()))
        };
        let mut short = Vec::new();
        for adapter in &adapters {
            let missing = device::shortfall(&adapter.limits());
            if missing.is_empty() {
                return Ok(());
            }
            short.push(format!(
                "{}: {}",
                adapter.get_info().name,
                missing.join(", ")
            ));
        }
        Err(if short.is_empty() {
            "no adapter".into()
        } else {
            format!("below required limits ({})", short.join("; "))
        })
    }

    /// Backends with an adapter the crash sentinel refuses.
    fn blocked_backends(&self) -> Result<wgpu::Backends, String> {
        let adapters = {
            let context = self.lock()?;
            pollster::block_on(context.instance.enumerate_adapters(wgpu::Backends::all()))
        };
        Ok(adapters
            .iter()
            .map(|adapter| adapter.get_info())
            .filter(|info| crate::renderer::sentinel::blocked(&adapter_record(info)))
            .fold(wgpu::Backends::empty(), |blocked, info| {
                blocked | wgpu::Backends::from(info.backend)
            }))
    }

    /// Only a context without a device may enumerate adapters: on GLES that makes the shared EGL
    /// context current outside wgpu's lock, racing the render threads that use it (`BadAccess`).
    /// A device here already passed the sentinel for its backend.
    fn probes_adapters(&self) -> Result<bool, String> {
        Ok(self.device_count()? == 0)
    }

    pub fn device_count(&self) -> Result<usize, String> {
        Ok(self.lock()?.devices.len())
    }

    pub fn renderer_count(&self) -> Result<usize, String> {
        Ok(self
            .1
            .lock()
            .map_err(|_| "UI_SURFACE: poisoned renderer registry")?
            .len())
    }

    pub(crate) fn renderer(
        &self,
        device_id: usize,
    ) -> Result<crate::renderer::gpu::pass::SceneRenderer, String> {
        let context = self.lock()?;
        if device_id >= context.devices.len() {
            return Err("UI_SURFACE: missing renderer device".into());
        }
        Ok(self
            .1
            .lock()
            .map_err(|_| "UI_SURFACE: poisoned renderer registry")?
            .entry(device_id)
            .or_default()
            .clone())
    }
}

/// Prefix of the error for an adapter whose driver crashed a previous process.
const BLOCKED: &str = "UI_SURFACE: GPU driver failed during a previous launch";

/// One frame being drawn into a presenter's target slot, with the drawing state it borrowed.
pub(crate) struct PrepareJob {
    slot: usize,
    epoch: u64,
    revision: u64,
    composed: bool,
    keep_base: bool,
    width: u32,
    height: u32,
    device: wgpu::Device,
    queue: wgpu::Queue,
    target: wgpu::TextureView,
    depth: Option<wgpu::TextureView>,
    renderer: crate::renderer::gpu::pass::SceneRenderer,
    scenes: crate::renderer::gpu::scene::SceneCache,
    glyph_cache: glifo::GlyphPrepCache,
    images: crate::renderer::gpu::images::ImageTextures,
    compositor: Option<crate::renderer::gpu::composite::Compositor>,
    base: Option<(u64, wgpu::TextureView)>,
    pass_us: Vec<[u64; 2]>,
    output: Option<crate::renderer::gpu::composite::Surface>,
    preparation_us: [u64; 4],
    scene_cache_hit: bool,
}

/// Renders after which kept scenes are encoded again, under the atlas's 64-render entry age.
const ATLAS_REFRESH: u64 = 32;

impl PrepareJob {
    /// Draws `display`; holds no presenter or context lock.
    pub(crate) fn run(&mut self, display: &crate::DisplayList) -> Result<(), String> {
        let (width, height) = (self.width, self.height);
        let target = &self.target;
        if self.composed {
            let started = Instant::now();
            let compositor = self.compositor.get_or_insert_with(|| {
                crate::renderer::gpu::composite::Compositor::with_renderer(self.renderer.clone())
            });
            let output = compositor.render_display(display, &self.device, &self.queue)?;
            self.output = Some(output);
            self.base = None;
            self.pass_us.clear();
            self.scenes.clear();
            self.images.clear();
            self.preparation_us = [0, 0, crate::session::metrics::micros(started), 0];
            self.scene_cache_hit = false;
            return Ok(());
        }

        let size = vello_gpu::RenderSize {
            width: width as u16,
            height: height as u16,
        };
        let started = Instant::now();
        // A kept scene draws glyphs from the atlas, which drops those no render touched for 64
        // renders: well before that, every scene is encoded again.
        let renders = self.renderer.renders();
        if renders.saturating_sub(self.scenes.encoded_at) > ATLAS_REFRESH {
            self.scenes.clear();
            self.scenes.encoded_at = renders;
        }
        let (scenes, prep) = (&mut self.scenes, &mut self.glyph_cache);
        let scene_cache_hit = self.renderer.with_resources(
            &self.device,
            width as u16,
            height as u16,
            |resources| {
                scenes.prepare(
                    display,
                    &mut crate::renderer::gpu::text::Glyphs {
                        prep,
                        atlas: Some(resources),
                    },
                )
            },
        )??;
        self.glyph_cache.maintain();
        let scene_us = crate::session::metrics::micros(started);
        let started = Instant::now();
        let pixels = display
            .bitmaps()
            .map(|bitmap| bitmap.pixels().clone())
            .chain(self.scenes.glyph_images().cloned());
        let bindings = self.images.bindings(&self.device, &self.queue, pixels);
        let images_us = crate::session::metrics::micros(started);
        let [mut encode_us, mut submit_us] = [0, 0];
        self.pass_us.clear();
        let mut scenes = self.scenes.scenes();
        // A kept bottom segment is copied in; only the passes above it draw again.
        let base = self.scenes.base().filter(|_| self.keep_base);
        self.base = self.base.take().filter(|(id, view)| {
            Some(*id) == base && view.texture().size() == target.texture().size()
        });
        if let Some(id) = base {
            let first = scenes.next().ok_or("UI_SURFACE: no base scene")?;
            if self.base.is_none() {
                let view = crate::renderer::gpu::device::target_view(&self.device, width, height);
                let [encode, submit] = self.renderer.draw(
                    &self.device,
                    &self.queue,
                    first,
                    &bindings,
                    crate::renderer::gpu::pass::Target {
                        size: size.clone(),
                        view: &view,
                        depth: self.depth.as_ref(),
                        init: vello_gpu::TargetInit::Clear(vello_gpu::ClearSettings::default()),
                    },
                )?;
                encode_us += encode;
                submit_us += submit;
                self.pass_us.push([encode, submit]);
                self.base = Some((id, view));
            }
            let started = Instant::now();
            let mut encoder = self.device.create_command_encoder(&Default::default());
            let base = &self.base.as_ref().ok_or("UI_SURFACE: no base")?.1;
            encoder.copy_texture_to_texture(
                base.texture().as_image_copy(),
                target.texture().as_image_copy(),
                target.texture().size(),
            );
            self.queue.submit([encoder.finish()]);
            let copy = crate::session::metrics::micros(started);
            submit_us += copy;
            self.pass_us.push([0, copy]);
        }
        for (index, scene) in scenes.enumerate() {
            let first = index == 0 && base.is_none();
            let [encode, submit] = self.renderer.draw(
                &self.device,
                &self.queue,
                scene,
                &bindings,
                crate::renderer::gpu::pass::Target {
                    size: size.clone(),
                    view: target,
                    depth: self.depth.as_ref(),
                    init: if first {
                        vello_gpu::TargetInit::Clear(vello_gpu::ClearSettings::default())
                    } else {
                        vello_gpu::TargetInit::SrcOver
                    },
                },
            )?;
            encode_us += encode;
            submit_us += submit;
            self.pass_us.push([encode, submit]);
        }
        self.preparation_us = [scene_us, images_us, encode_us, submit_us];
        self.scene_cache_hit = scene_cache_hit;
        Ok(())
    }
}

/// Surface ownership and presentation stay on the platform UI thread.
pub struct Presenter {
    surface: Option<RenderSurface<'static>>,
    device_id: usize,
    /// The revision each target holds, until a newer preparation reuses it.
    prepared: [Option<u64>; 2],
    presentation_us: [u64; 4],
    preparation_us: [u64; 4],
    /// Encode and submit time of each scene pass in the last direct preparation.
    pass_us: Vec<[u64; 2]>,
    /// The bottom static segment rendered once while the rest of the page animates.
    base: Option<(u64, wgpu::TextureView)>,
    /// False from a release until the next present: frames prepared meanwhile (a hidden
    /// page's in-flight ones) must not keep a new base.
    keep_base: bool,
    scene_cache_hit: bool,
    /// Bumped when the surface goes away or is bound anew: a frame drawn across it is dropped.
    epoch: u64,
    renderer: crate::renderer::gpu::pass::SceneRenderer,
    scenes: crate::renderer::gpu::scene::SceneCache,
    glyph_cache: glifo::GlyphPrepCache,
    images: crate::renderer::gpu::images::ImageTextures,
    compositor: Option<crate::renderer::gpu::composite::Compositor>,
    composed: [Option<crate::renderer::gpu::composite::Surface>; 2],

    depth: Option<wgpu::TextureView>,
    context: Context,
    /// This presenter's adapter record for the crash sentinel.
    adapter: String,
    backend: &'static str,
    /// Rebuilds the surface target: a GLES window surface keeps its creation size.
    target: Option<Box<dyn Fn() -> wgpu::SurfaceTarget<'static> + Send + Sync>>,
}

impl Presenter {
    pub fn context(&self) -> Context {
        self.context.clone()
    }

    /// The renderer label for frame metrics, from the adapter this presenter actually uses.
    pub fn backend(&self) -> &'static str {
        self.backend
    }

    /// The adapter record: backend, ids, driver and renderer release.
    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        Self::with_context(Context::default(), target, width, height)
    }

    /// On `context`'s adapter, or on GLES when the crash sentinel blocks it.
    pub fn with_fallback<W>(
        context: Context,
        window: Arc<W>,
        width: u32,
        height: u32,
    ) -> Result<Self, String>
    where
        Arc<W>: Into<wgpu::SurfaceTarget<'static>>,
        W: Send + Sync + 'static,
    {
        let rebind = window.clone();
        // A surface binds the window to its backend; a backend that is going to be
        // refused must not touch the window, or the fallback cannot bind it (Adreno:
        // GLES then finds no compatible device).
        if context.probes_adapters()?
            && context.blocked_backends()?.contains(wgpu::Backends::VULKAN)
        {
            return Self::gles_fallback(BLOCKED.to_string(), window, width, height).map(
                |mut presenter| {
                    presenter.target = Some(Box::new(move || rebind.clone().into()));
                    presenter
                },
            );
        }
        let presenter = match Self::with_context(context, window.clone(), width, height) {
            Err(error) if error.starts_with(BLOCKED) => {
                Self::gles_fallback(error, window, width, height)
            }
            result => result,
        };
        presenter.map(|mut presenter| {
            presenter.target = Some(Box::new(move || rebind.clone().into()));
            presenter
        })
    }

    /// GLES once the Vulkan driver is refused. An adapter below the renderer's limits is
    /// not attempted, so the page goes to the CPU renderer without a failed device request.
    fn gles_fallback<W>(
        error: String,
        window: Arc<W>,
        width: u32,
        height: u32,
    ) -> Result<Self, String>
    where
        Arc<W>: Into<wgpu::SurfaceTarget<'static>>,
        W: Send + Sync + 'static,
    {
        let context = Context::with_backends(wgpu::Backends::GL);
        context
            .meets_renderer_limits()
            .map_err(|short| format!("{error}; GLES: {short}"))?;
        Self::with_context(context, window, width, height)
            .map_err(|fallback| format!("{error}; GLES: {fallback}"))
    }

    pub fn with_context(
        context: Context,
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err("UI_SURFACE: empty size".into());
        }
        let mut shared = context.lock()?;
        let surface = pollster::block_on(shared.create_surface(target, width, height))?;
        let info = shared.devices[surface.dev_id].adapter().get_info();
        let adapter = adapter_record(&info);
        let backend = match info.backend {
            wgpu::Backend::Gl => "vello-gles",
            wgpu::Backend::Vulkan => "vello-vulkan",
            wgpu::Backend::Metal => "vello-metal",
            wgpu::Backend::Dx12 => "vello-dx12",
            _ => "vello-gpu",
        };
        if !crate::renderer::sentinel::arm(&adapter) {
            return Err(format!("{BLOCKED}: {adapter}"));
        }
        let device = &shared.devices[surface.dev_id].device;
        let depth = depth_view(&surface, device);
        let device_id = surface.dev_id;
        drop(shared);
        let renderer = context.renderer(device_id).inspect_err(|_| {
            crate::renderer::sentinel::released(&adapter);
        })?;
        Ok(Self {
            surface: Some(surface),
            device_id,
            prepared: [None; 2],
            presentation_us: [0; 4],
            preparation_us: [0; 4],
            pass_us: Vec::new(),
            base: None,
            keep_base: true,
            epoch: 0,
            scene_cache_hit: false,
            renderer,
            scenes: Default::default(),
            images: Default::default(),
            compositor: None,
            composed: [None, None],
            glyph_cache: Default::default(),
            depth: Some(depth),
            context,
            adapter,
            backend,
            target: None,
        })
    }

    pub fn suspend(&mut self) {
        self.epoch += 1;
        self.surface = None;
        self.depth = None;
        self.images.clear();
        self.composed = [None, None];
        self.base = None;
        self.prepared = [None; 2];
        if let Some(compositor) = &mut self.compositor {
            compositor.clear();
        }
        self.scenes.clear();
        self.scene_cache_hit = false;
    }

    /// Bind to `window` again, e.g. after its `Surface` was recreated.
    pub fn resume<W>(&mut self, window: Arc<W>, width: u32, height: u32) -> Result<(), String>
    where
        Arc<W>: Into<wgpu::SurfaceTarget<'static>>,
        W: Send + Sync + 'static,
    {
        self.target = Some(Box::new(move || window.clone().into()));
        self.bind(width, height)
    }

    fn bind(&mut self, width: u32, height: u32) -> Result<(), String> {
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err("UI_SURFACE: empty size".into());
        }
        let target = self.target.as_ref().ok_or("UI_SURFACE: no target")?();
        // A window carries one EGL surface at a time; release the old one, and everything drawn
        // for it, before binding anew.
        self.suspend();
        let mut context = self.context.lock()?;
        let surface = pollster::block_on(context.create_surface(target, width, height))?;
        if surface.dev_id != self.device_id {
            return Err("UI_SURFACE: device changed".into());
        }
        self.depth = Some(depth_view(
            &surface,
            &context.devices[self.device_id].device,
        ));
        self.surface = Some(surface);
        Ok(())
    }

    pub fn prepare(
        &mut self,
        display: &crate::DisplayList,
        width: u32,
        height: u32,
        revision: u64,
    ) -> Result<bool, String> {
        self.prepare_inner(
            display,
            width,
            height,
            revision,
            display.requires_composition(),
        )
    }

    /// Execute shared group composition and retain its output through presentation.
    pub fn prepare_composited(
        &mut self,
        display: &crate::DisplayList,
        width: u32,
        height: u32,
        revision: u64,
    ) -> Result<bool, String> {
        self.prepare_inner(display, width, height, revision, true)
    }

    fn prepare_inner(
        &mut self,
        display: &crate::DisplayList,
        width: u32,
        height: u32,
        revision: u64,
        composed: bool,
    ) -> Result<bool, String> {
        let Some(mut job) = self.begin_prepare(display, width, height, revision, composed)? else {
            return Ok(false);
        };
        let result = job.run(display);
        self.finish_prepare(job, result)
    }

    /// Picks the target slot and lends the drawing state to a [`PrepareJob`], which draws
    /// without this presenter: presenting the other slot does not wait for it.
    pub(crate) fn begin_prepare(
        &mut self,
        display: &crate::DisplayList,
        width: u32,
        height: u32,
        revision: u64,
        composed: bool,
    ) -> Result<Option<PrepareJob>, String> {
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Ok(None);
        }
        if u32::from(display.width) != width || u32::from(display.height) != height {
            return Err("UI_SURFACE: display dimensions mismatch".into());
        }
        let resized = self
            .surface
            .as_ref()
            .is_some_and(|s| s.config.width != width || s.config.height != height);
        // Reconfiguring a GLES surface keeps the EGL window surface and its buffer
        // geometry (wgpu only resizes Wayland windows); bind a fresh one instead.
        if resized && self.backend == "vello-gles" && self.target.is_some() {
            self.bind(width, height)?;
        }
        let Some(surface) = &mut self.surface else {
            return Ok(None);
        };
        let context = self.context.lock()?;
        if surface.config.width != width || surface.config.height != height {
            context.resize_surface(surface, width, height);
            self.depth = Some(depth_view(surface, &context.devices[surface.dev_id].device));
            self.prepared = [None; 2];
        }
        let device = &context.devices[surface.dev_id];
        // Reuse the target of the older (or no) revision; the newer may still wait to present.
        let slot = match self.prepared {
            [None, _] => 0,
            [_, None] => 1,
            [Some(a), Some(b)] => usize::from(b < a),
        };
        self.prepared[slot] = None;
        // The slot's previous output was presented; its texture can serve this frame.
        self.composed[slot] = None;
        Ok(Some(PrepareJob {
            slot,
            epoch: self.epoch,
            revision,
            composed,
            keep_base: self.keep_base,
            width,
            height,
            device: device.device.clone(),
            queue: device.queue.clone(),
            target: surface.target_views[slot].clone(),
            depth: self.depth.clone(),
            renderer: self.renderer.clone(),
            scenes: std::mem::take(&mut self.scenes),
            glyph_cache: std::mem::take(&mut self.glyph_cache),
            images: std::mem::take(&mut self.images),
            compositor: self.compositor.take(),
            base: self.base.take(),
            pass_us: std::mem::take(&mut self.pass_us),
            output: None,
            preparation_us: [0; 4],
            scene_cache_hit: false,
        }))
    }

    /// Takes the drawing state back; the frame is ready to present unless the surface was
    /// suspended or rebound while it drew.
    pub(crate) fn finish_prepare(
        &mut self,
        job: PrepareJob,
        result: Result<(), String>,
    ) -> Result<bool, String> {
        self.scenes = job.scenes;
        self.glyph_cache = job.glyph_cache;
        self.images = job.images;
        self.compositor = job.compositor;
        self.base = job.base;
        self.pass_us = job.pass_us;
        if job.epoch != self.epoch {
            // Drawn for a surface that is gone: drop what it would have kept.
            self.images.clear();
            self.base = None;
            if let Some(compositor) = &mut self.compositor {
                compositor.clear();
            }
            self.scenes.clear();
            return result.map(|()| false);
        }
        result?;
        self.composed[job.slot] = job.output;
        self.preparation_us = job.preparation_us;
        self.scene_cache_hit = job.scene_cache_hit;
        self.prepared[job.slot] = Some(job.revision);
        Ok(true)
    }

    /// The last composited frame's work, `None` when the frame drew without composition.
    pub fn composite_stats(&self) -> Option<crate::renderer::gpu::composite::CompositeStats> {
        self.compositor
            .as_ref()
            .map(|compositor| compositor.stats())
    }

    /// Backdrops captured and filtered so far, and the filtered images held for later frames:
    /// `(filtered, cached, cached_bytes)`.
    pub fn backdrop_cache_stats(&self) -> (u64, usize, u64) {
        self.compositor.as_ref().map_or((0, 0, 0), |compositor| {
            let (cached, bytes) = compositor.cached_backdrops();
            (compositor.backdrop_filters(), cached, bytes)
        })
    }

    /// CPU wall time for acquire, encode, submit, and present; these are not GPU timestamps.
    pub fn presentation_timings_us(&self) -> [u64; 4] {
        self.presentation_us
    }

    /// CPU wall time for scene construction, texture handling, encoding and submission.
    /// Multipass composition reports its combined time in the encoding slot.
    pub fn preparation_timings_us(&self) -> [u64; 4] {
        self.preparation_us
    }

    /// Drop the kept bottom segment; the next animated frame renders it again.
    pub fn release_transient(&mut self) {
        self.base = None;
        self.keep_base = false;
    }

    /// Bytes of composition textures (layers and backdrops), leased and idle; depth excluded.
    pub fn composite_bytes(&self) -> u64 {
        self.compositor
            .as_ref()
            .map_or(0, |compositor| compositor.texture_stats().bytes)
    }

    /// Bytes held only to speed up coming frames.
    pub fn transient_bytes(&self) -> u64 {
        self.base.as_ref().map_or(0, |(_, view)| {
            let size = view.texture().size();
            u64::from(size.width) * u64::from(size.height) * 4
        })
    }

    /// Per scene pass of the last direct preparation: CPU encode and submit time.
    pub fn pass_timings_us(&self) -> &[[u64; 2]] {
        &self.pass_us
    }

    pub fn scene_cache_hit(&self) -> bool {
        self.scene_cache_hit
    }

    pub fn present(&mut self, revision: u64) -> Result<bool, String> {
        self.presentation_us = [0; 4];
        let Some(slot) = self
            .prepared
            .iter()
            .position(|held| *held == Some(revision))
        else {
            return Ok(false);
        };
        let Some(surface) = &mut self.surface else {
            return Ok(false);
        };
        let context = self.context.lock()?;
        let width = surface.config.width;
        let height = surface.config.height;
        let started = Instant::now();
        let texture = match surface.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated => {
                context.resize_surface(surface, width, height);
                self.depth = Some(depth_view(surface, &context.devices[self.device_id].device));
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            other => return Err(format!("UI_SURFACE: {other:?}")),
        };
        self.presentation_us[0] = started.elapsed().as_micros() as u64;
        let started = Instant::now();
        let device = &context.devices[surface.dev_id];
        let mut encoder = device
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("LingXia UI present"),
            });
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        surface.blitter.copy(
            &device.device,
            &mut encoder,
            self.composed[slot]
                .as_ref()
                .map_or(&surface.target_views[slot], |output| output.view()),
            &view,
        );
        let command = encoder.finish();
        self.presentation_us[1] = started.elapsed().as_micros() as u64;
        let started = Instant::now();
        device.queue.submit([command]);
        self.presentation_us[2] = started.elapsed().as_micros() as u64;
        let started = Instant::now();
        device.queue.present(texture);
        self.presentation_us[3] = started.elapsed().as_micros() as u64;
        crate::renderer::sentinel::presented(&self.adapter);
        self.keep_base = true;
        Ok(true)
    }

    /// The newest frame this presenter holds, shown or waiting to be, as straight-alpha RGBA rows.
    pub fn readback(&self) -> Result<Option<(u32, u32, Vec<u8>)>, String> {
        let Some(surface) = &self.surface else {
            return Ok(None);
        };
        let Some(slot) = (0..2)
            .filter(|slot| self.prepared[*slot].is_some())
            .max_by_key(|slot| self.prepared[*slot])
        else {
            return Ok(None);
        };
        let texture = self.composed[slot]
            .as_ref()
            .map_or(surface.target_views[slot].texture(), |output| {
                output.texture()
            });
        let context = self.context.lock()?;
        let device = &context.devices[surface.dev_id];
        let pixels = read_texture(&device.device, &device.queue, texture)?;
        let size = texture.size();
        Ok(Some((size.width, size.height, pixels)))
    }
}

/// Copy an RGBA8 texture to the CPU: rows packed, premultiplied alpha undone.
fn read_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
) -> Result<Vec<u8>, String> {
    let size = texture.size();
    let row = size.width * 4;
    let stride =
        row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("LingXia UI readback"),
        size: u64::from(stride) * u64::from(size.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(size.height),
            },
        },
        size,
    );
    queue.submit([encoder.finish()]);
    let (send, receive) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = send.send(result);
        });
    let timeout = std::time::Duration::from_secs(5);
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(timeout),
        })
        .map_err(|e| format!("UI_SURFACE: readback: {e}"))?;
    receive
        .recv_timeout(timeout)
        .map_err(|e| format!("UI_SURFACE: readback: {e}"))?
        .map_err(|e| format!("UI_SURFACE: readback: {e}"))?;
    let mapped = buffer
        .slice(..)
        .get_mapped_range()
        .map_err(|e| format!("UI_SURFACE: readback: {e}"))?;
    let mut pixels = Vec::with_capacity((row * size.height) as usize);
    for line in mapped.chunks(stride as usize) {
        pixels.extend_from_slice(&line[..row as usize]);
    }
    drop(mapped);
    buffer.unmap();
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = u16::from(pixel[3]);
        if alpha != 0 && alpha != 255 {
            for channel in &mut pixel[..3] {
                *channel = ((u16::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    Ok(pixels)
}

/// Adapter, driver and renderer release: an update to any of them is tried again.
fn adapter_record(info: &wgpu::AdapterInfo) -> String {
    format!(
        "{:?}|{:04x}:{:04x}|{}|{}|{}|lingxia-view {}",
        info.backend,
        info.vendor,
        info.device,
        info.name,
        info.driver,
        info.driver_info,
        env!("CARGO_PKG_VERSION")
    )
}

/// The renderer's depth buffer for `surface`'s size.
fn depth_view(surface: &RenderSurface<'_>, device: &wgpu::Device) -> wgpu::TextureView {
    vello_gpu::Renderer::create_depth_texture_view(
        device,
        &vello_gpu::RenderSize {
            width: surface.config.width as u16,
            height: surface.config.height as u16,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_ohos_narrows_the_default_backends() {
        let expected = if cfg!(target_env = "ohos") {
            wgpu::Backends::GL
        } else {
            wgpu::Backends::default()
        };
        assert_eq!(default_backends(), expected);
    }

    #[test]
    #[ignore = "requires a GPU adapter; run explicitly with --features gpu -- --ignored"]
    fn readback_packs_rows_and_undoes_premultiplied_alpha() {
        let domain = Context::default();
        let mut context = domain.lock().unwrap();
        let id = pollster::block_on(context.device(None)).expect("GPU adapter required");
        let device = &context.devices[id].device;
        let queue = &context.devices[id].queue;
        // 3 px wide: 12-byte rows that the copy pads to 256.
        let texture = crate::renderer::gpu::device::target_view(device, 3, 2)
            .texture()
            .clone();
        let pixels: Vec<u8> = [[255, 0, 0, 255], [64, 32, 0, 128], [0, 0, 0, 0]]
            .repeat(2)
            .concat();
        queue.write_texture(
            texture.as_image_copy(),
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(12),
                rows_per_image: Some(2),
            },
            texture.size(),
        );
        let read = read_texture(device, queue, &texture).unwrap();
        assert_eq!(read.len(), 3 * 2 * 4);
        assert_eq!(&read[..12], &[255, 0, 0, 255, 128, 64, 0, 128, 0, 0, 0, 0]);
        assert_eq!(&read[..12], &read[12..]);
    }

    #[test]
    fn only_a_context_without_a_device_probes_adapters() {
        let context = Context::with_backends(wgpu::Backends::empty());
        assert_eq!(context.probes_adapters(), Ok(true));
    }

    #[test]
    #[ignore = "requires a GPU adapter; run explicitly with --features gpu -- --ignored"]
    fn a_shared_context_with_a_device_never_probes_adapters() {
        let context = Context::default();
        pollster::block_on(context.lock().unwrap().device(None)).expect("GPU adapter required");
        assert_eq!(context.clone().probes_adapters(), Ok(false));
    }

    #[test]
    #[ignore = "requires a GPU adapter; run explicitly with --features gpu -- --ignored"]
    fn shared_context_reuses_device_after_original_owner_is_dropped() {
        let first = Context::default();
        let lifetime = Arc::downgrade(&first.0);
        let id =
            pollster::block_on(first.lock().unwrap().device(None)).expect("GPU adapter required");
        let second = first.clone();
        drop(first);
        assert!(lifetime.upgrade().is_some());
        assert_eq!(second.device_count().unwrap(), 1);
        {
            let mut context = second.lock().unwrap();
            assert_eq!(pollster::block_on(context.device(None)), Some(id));
            assert_eq!(context.devices.len(), 1);
        }
        drop(second);
        assert!(
            lifetime.upgrade().is_none(),
            "no process-lifetime device retention"
        );
    }
}
