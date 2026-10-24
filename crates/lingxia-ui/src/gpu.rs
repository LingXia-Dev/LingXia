use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::Instant,
};
use vello::util::{RenderContext, RenderSurface};

/// Device domain shared by related surfaces; page caches remain presenter-owned.
#[derive(Clone)]
pub struct Context(
    Arc<Mutex<RenderContext>>,
    Arc<Mutex<HashMap<usize, crate::gpu_pass::SceneRenderer>>>,
);

impl Default for Context {
    fn default() -> Self {
        Self(
            Arc::new(Mutex::new(RenderContext::new())),
            Default::default(),
        )
    }
}

impl Context {
    pub(crate) fn lock(&self) -> Result<MutexGuard<'_, RenderContext>, String> {
        self.0
            .lock()
            .map_err(|_| "UI_SURFACE: poisoned GPU context".into())
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
    ) -> Result<crate::gpu_pass::SceneRenderer, String> {
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

/// Surface ownership and presentation stay on the platform UI thread.
pub struct Presenter {
    surface: Option<RenderSurface<'static>>,
    device_id: usize,
    revision: u64,
    presentation_us: [u64; 4],
    preparation_us: [u64; 4],
    scene_cache_hit: bool,
    renderer: crate::gpu_pass::SceneRenderer,
    scenes: crate::gpu_scene::SceneCache,
    glyph_cache: glifo::GlyphPrepCache,
    images: crate::gpu_images::ImageTextures,
    compositor: Option<crate::composite_gpu::Compositor>,
    composed: Option<crate::composite_gpu::Surface>,
    depth: Option<wgpu::TextureView>,
    context: Context,
}

impl Presenter {
    pub fn context(&self) -> Context {
        self.context.clone()
    }

    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        Self::with_context(Context::default(), target, width, height)
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
        let mut surface = pollster::block_on(shared.create_surface(
            target,
            width,
            height,
            wgpu::PresentMode::Fifo,
        ))
        .map_err(|e| e.to_string())?;
        let device = &shared.devices[surface.dev_id].device;
        let depth = raster_target(&mut surface, device);
        let device_id = surface.dev_id;
        drop(shared);
        let renderer = context.renderer(device_id)?;
        Ok(Self {
            surface: Some(surface),
            device_id,
            revision: 0,
            presentation_us: [0; 4],
            preparation_us: [0; 4],
            scene_cache_hit: false,
            renderer,
            scenes: Default::default(),
            images: Default::default(),
            compositor: None,
            composed: None,
            glyph_cache: Default::default(),
            depth: Some(depth),
            context,
        })
    }

    pub fn suspend(&mut self) {
        self.surface = None;
        self.depth = None;
        self.images.clear();
        self.composed = None;
        if let Some(compositor) = &mut self.compositor {
            compositor.clear();
        }
        self.scenes.clear();
        self.scene_cache_hit = false;
    }

    pub fn resume(
        &mut self,
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err("UI_SURFACE: empty size".into());
        }
        let mut context = self.context.lock()?;
        let mut surface = pollster::block_on(context.create_surface(
            target,
            width,
            height,
            wgpu::PresentMode::Fifo,
        ))
        .map_err(|e| e.to_string())?;
        if surface.dev_id != self.device_id {
            return Err("UI_SURFACE: device changed".into());
        }
        self.depth = Some(raster_target(
            &mut surface,
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
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Ok(false);
        }
        if u32::from(display.width) != width || u32::from(display.height) != height {
            return Err("UI_SURFACE: display dimensions mismatch".into());
        }
        let Some(surface) = &mut self.surface else {
            return Ok(false);
        };
        let context = self.context.lock()?;
        if surface.config.width != width || surface.config.height != height {
            context.resize_surface(surface, width, height);
            self.depth = Some(raster_target(
                surface,
                &context.devices[surface.dev_id].device,
            ));
        }
        let device = &context.devices[surface.dev_id];

        if composed {
            let started = Instant::now();
            let compositor = self.compositor.get_or_insert_with(|| {
                crate::composite_gpu::Compositor::with_renderer(self.renderer.clone())
            });
            let output = compositor.render_display(display, &device.device, &device.queue)?;
            self.composed = Some(output);
            self.scenes.clear();
            self.images.clear();
            self.preparation_us = [0, 0, crate::metrics::micros(started), 0];
            self.scene_cache_hit = false;
            self.revision = revision;
            return Ok(true);
        }

        let size = vello_gpu::RenderSize {
            width: width as u16,
            height: height as u16,
        };
        let started = Instant::now();
        let scene_cache_hit = self.scenes.prepare(display, &mut self.glyph_cache)?;
        self.glyph_cache.maintain();
        let scene_us = crate::metrics::micros(started);
        let started = Instant::now();
        let pixels = display
            .bitmaps()
            .map(|bitmap| bitmap.pixels().clone())
            .chain(self.scenes.glyph_images().cloned());
        let bindings = self.images.bindings(&device.device, &device.queue, pixels);
        let images_us = crate::metrics::micros(started);
        let [encode_us, submit_us] = self.renderer.draw(
            &device.device,
            &device.queue,
            self.scenes.scene(),
            &bindings,
            crate::gpu_pass::Target {
                size,
                view: &surface.target_view,
                depth: self.depth.as_ref(),
                init: vello_gpu::TargetInit::Clear(vello_gpu::ClearSettings::default()),
            },
        )?;
        self.preparation_us = [scene_us, images_us, encode_us, submit_us];
        self.composed = None;
        self.scene_cache_hit = scene_cache_hit;
        self.revision = revision;
        Ok(true)
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

    pub fn scene_cache_hit(&self) -> bool {
        self.scene_cache_hit
    }

    pub fn present(&mut self, revision: u64) -> Result<bool, String> {
        self.presentation_us = [0; 4];
        if self.revision != revision {
            return Ok(false);
        }
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
                self.depth = Some(raster_target(
                    surface,
                    &context.devices[self.device_id].device,
                ));
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
            self.composed
                .as_ref()
                .map_or(&surface.target_view, |output| output.view()),
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
        Ok(true)
    }
}

fn raster_target(surface: &mut RenderSurface<'_>, device: &wgpu::Device) -> wgpu::TextureView {
    surface.target_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("LingXia sparse target"),
        size: wgpu::Extent3d {
            width: surface.config.width,
            height: surface.config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    surface.target_view = surface
        .target_texture
        .create_view(&wgpu::TextureViewDescriptor::default());
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
