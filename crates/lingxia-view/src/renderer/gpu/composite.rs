//! GPU implementation of the shared composition order; no CPU pixel readback.
use crate::{
    renderer::gpu::color_filter::SaturationPass,
    renderer::gpu::pass::{SceneRenderer, Target},
    scene::composite::{Backend, Layer},
};
use std::ops::Range;
use vello_gpu::{Scene, TargetInit, TextureBindings, TextureId};

pub use crate::renderer::gpu::surface::{Surface, TextureStats};

/// What one composited frame did, for diagnostics: counts, pixels and CPU microseconds.
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositeStats {
    /// Scene draws, each one renderer pass and one queue submit.
    pub draws: u32,
    /// Target pixels of those draws.
    pub draw_pixels: u64,
    /// Building the display's content scenes.
    pub content_scene_us: u64,
    /// Building the compositor's own scenes: layer composites, backdrop filters and placement.
    pub composite_scene_us: u64,
    /// Renderer encode and submit of the draws.
    pub encode_us: u64,
    pub submit_us: u64,
    /// Queue submits besides the draws': clears, copies, saturation.
    pub submits: u32,
    pub clears: u32,
    pub copies: u32,
    pub copy_pixels: u64,
    pub backdrop_hits: u32,
    pub backdrop_misses: u32,
    /// Pixels the missed backdrops captured, and blurred at the filter's resolution.
    pub capture_pixels: u64,
    pub filter_pixels: u64,
}

/// Device-bound pipelines and scratch reused across frames. A draw callback and
/// its borrowed display list live only for the duration of `render`.
pub struct Compositor {
    renderer: SceneRenderer,
    pool: crate::renderer::gpu::surface::Pool,
    saturation: Option<SaturationPass>,
    depth: Vec<(u16, u16, wgpu::TextureView)>,
    depth_allocations: u64,
    images: crate::renderer::gpu::images::ImageTextures,
    glyph_cache: glifo::GlyphPrepCache,
    budget_warned: bool,
    /// Filtered backdrop images reused while what lies beneath them stays the same.
    backdrops: crate::renderer::gpu::backdrop_cache::BackdropCache<Surface>,
    /// Backdrops captured and filtered, as opposed to taken from the cache.
    backdrop_filters: u64,
    /// The last rendered plan's work.
    stats: CompositeStats,
}

impl Compositor {
    /// Discard idle textures and depth on suspend. Older leases cannot refill the pool.
    pub fn clear(&mut self) {
        self.backdrops.clear();
        self.pool.clear();
        self.depth.clear();
        self.images.clear();
        self.glyph_cache = Default::default();
    }

    pub fn texture_stats(&self) -> TextureStats {
        let mut stats = self.pool.stats();
        stats.depth_textures = self.depth.len();
        stats.depth_pixels = self
            .depth
            .iter()
            .map(|(w, h, _)| u64::from(*w) * u64::from(*h))
            .sum();
        stats.depth_allocations = self.depth_allocations;
        stats
    }

    pub fn new(_device: &wgpu::Device) -> Self {
        Self::with_renderer(Default::default())
    }

    pub(crate) fn with_renderer(renderer: SceneRenderer) -> Self {
        Self {
            renderer,
            pool: Default::default(),
            saturation: None,
            depth: Vec::new(),
            depth_allocations: 0,
            images: Default::default(),
            glyph_cache: Default::default(),
            budget_warned: false,
            backdrops: Default::default(),
            backdrop_filters: 0,
            stats: CompositeStats::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn lease(
        &mut self,
        device: &wgpu::Device,
        width: u16,
        height: u16,
    ) -> Result<Surface, String> {
        self.pool.lease(device, width, height)
    }

    #[cfg(test)]
    pub(crate) fn budget_warned(&self) -> bool {
        self.budget_warned
    }

    /// Fit `plan` beside the textures earlier frames still hold; a frame that does not fit
    /// draws its deepest groups flattened rather than failing.
    pub(crate) fn fit(
        &mut self,
        plan: crate::scene::composite::Plan,
    ) -> crate::scene::composite::Plan {
        let (width, height) = plan.dimensions();
        self.pool.set_frame(width, height);
        let available = self.pool.available();
        let (plan, flattened) = plan.within(available);
        if flattened > 0 && !self.budget_warned {
            self.budget_warned = true;
            log::warn!(
                target: "lingxia_view",
                "UI_BUDGET: this page's layers need more GPU texture memory than `leaseFrames` allows ({available} bytes free at {width}x{height}); {flattened} nested group(s) draw flattened"
            );
        }
        plan
    }

    fn depth_view(
        &mut self,
        device: &wgpu::Device,
        size: &vello_gpu::RenderSize,
    ) -> wgpu::TextureView {
        let entry = if let Some(index) = self
            .depth
            .iter()
            .position(|(w, h, _)| *w == size.width && *h == size.height)
        {
            self.depth.remove(index)
        } else {
            let pixels = u64::from(size.width) * u64::from(size.height);
            while !self.depth.is_empty()
                && (self.depth.len() >= 4
                    || self
                        .depth
                        .iter()
                        .map(|(w, h, _)| u64::from(*w) * u64::from(*h))
                        .sum::<u64>()
                        + pixels
                        > 4 * 1024 * 1024)
            {
                self.depth.remove(0);
            }
            self.depth_allocations += 1;
            (
                size.width,
                size.height,
                vello_gpu::Renderer::create_depth_texture_view(device, size),
            )
        };
        let view = entry.2.clone();
        self.depth.push(entry);
        view
    }

    pub fn render<F>(
        &mut self,
        plan: &crate::scene::composite::Plan,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        draw: F,
    ) -> Result<Surface, String>
    where
        F: FnMut(Range<usize>, &mut Scene, &mut TextureBindings) -> Result<(), String>,
    {
        self.render_keyed(plan, device, queue, draw, Default::default())
    }

    fn render_keyed<F>(
        &mut self,
        plan: &crate::scene::composite::Plan,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        draw: F,
        backdrops: std::collections::VecDeque<crate::renderer::gpu::backdrop_cache::BackdropKey>,
    ) -> Result<Surface, String>
    where
        F: FnMut(Range<usize>, &mut Scene, &mut TextureBindings) -> Result<(), String>,
    {
        self.stats = CompositeStats::default();
        plan.render(&mut Gpu {
            device,
            queue,
            resources: self,
            draw,
            backdrops,
        })
    }

    /// Backdrop images held for later frames: their count and bytes.
    pub fn cached_backdrops(&self) -> (usize, u64) {
        (self.backdrops.len(), self.backdrops.bytes())
    }

    /// Backdrops captured and filtered so far, rather than taken from the cache.
    pub fn backdrop_filters(&self) -> u64 {
        self.backdrop_filters
    }

    /// The last rendered plan's work.
    pub fn stats(&self) -> CompositeStats {
        self.stats
    }

    /// Render a retained display using the same text/image encoders as Presenter.
    pub fn render_display(
        &mut self,
        display: &crate::DisplayList,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Surface, String> {
        let plan = self.fit(display.composition_plan()?);
        let backdrops = crate::renderer::gpu::backdrop_cache::keys(&plan, display);
        // Move caches out for the callback's lifetime; restore them on errors too.
        let mut images = std::mem::take(&mut self.images);
        let mut glyph_cache = std::mem::take(&mut self.glyph_cache);
        let mut glyph_images = std::collections::HashMap::new();
        let image_ids = display
            .bitmaps()
            .map(|bitmap| {
                let pixels = bitmap.pixels();
                (
                    std::sync::Arc::as_ptr(pixels) as usize,
                    crate::renderer::gpu::text::source(pixels),
                )
            })
            .collect();
        let result = self.render_keyed(
            &plan,
            device,
            queue,
            |span, scene, bindings| {
                display.sparse_span(
                    &mut glyph_cache,
                    scene,
                    &image_ids,
                    &mut glyph_images,
                    span,
                )?;
                *bindings = images.bindings(
                    device,
                    queue,
                    display
                        .bitmaps()
                        .map(|bitmap| bitmap.pixels().clone())
                        .chain(glyph_images.values().cloned()),
                );
                Ok(())
            },
            backdrops,
        );
        glyph_cache.maintain();
        self.images = images;
        self.glyph_cache = glyph_cache;
        result
    }
}

struct Gpu<'a, F> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    resources: &'a mut Compositor,
    draw: F,
    /// The key of each backdrop still to filter, in plan order; empty when nothing is cached.
    backdrops: std::collections::VecDeque<crate::renderer::gpu::backdrop_cache::BackdropKey>,
}

impl<F> Gpu<'_, F> {
    /// A pooled surface for a target the next pass overwrites entirely: no clear pass, no submit.
    fn overwritten(&mut self, width: u16, height: u16) -> Result<Surface, String> {
        self.resources.pool.lease(self.device, width, height)
    }

    fn render(
        &mut self,
        scene: &Scene,
        bindings: &TextureBindings,
        target: &Surface,
        init: TargetInit<'_>,
    ) -> Result<(), String> {
        let size = target.size();
        let pixels = u64::from(size.width) * u64::from(size.height);
        let depth = self.resources.depth_view(self.device, &size);
        let [encode, submit] = self.resources.renderer.draw(
            self.device,
            self.queue,
            scene,
            bindings,
            Target {
                size,
                view: target.view(),
                depth: Some(&depth),
                init,
            },
        )?;
        let stats = &mut self.resources.stats;
        stats.draws += 1;
        stats.draw_pixels += pixels;
        stats.encode_us += encode;
        stats.submit_us += submit;
        Ok(())
    }
}

fn image(scene: &mut Scene, source: &Surface) -> TextureBindings {
    let bindings = image_brush(scene, source, Default::default());
    let size = source.size();
    scene.fill_rect(&vello_cpu::kurbo::Rect::new(
        0.0,
        0.0,
        size.width as f64,
        size.height as f64,
    ));
    bindings
}

fn image_brush(
    scene: &mut Scene,
    source: &Surface,
    sampler: vello_cpu::peniko::ImageSampler,
) -> TextureBindings {
    let size = source.size();
    let mut bindings = TextureBindings::new();
    bindings.insert(TextureId(1), source.view().clone());
    scene.set_paint(vello_gpu::Image {
        image: vello_gpu::ImageSource::external_texture(
            TextureId(1),
            vello_gpu::RectU16::new(0, 0, size.width, size.height),
            true,
        ),
        sampler,
    });
    bindings
}

impl<F> Backend for Gpu<'_, F>
where
    F: FnMut(Range<usize>, &mut Scene, &mut TextureBindings) -> Result<(), String>,
{
    type Surface = Surface;

    fn blank(&mut self, width: u16, height: u16) -> Result<Surface, String> {
        let surface = self.resources.pool.lease(self.device, width, height)?;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("LingXia clear composition surface"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface.view(),
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        }));
        self.queue.submit([encoder.finish()]);
        self.resources.stats.clears += 1;
        self.resources.stats.submits += 1;
        Ok(surface)
    }

    fn draw(&mut self, target: &mut Surface, span: Range<usize>) -> Result<(), String> {
        if span.is_empty() {
            return Ok(());
        }
        let size = target.size();
        let started = std::time::Instant::now();
        let mut scene = Scene::new(size.width, size.height);
        let mut bindings = TextureBindings::new();
        (self.draw)(span, &mut scene, &mut bindings)?;
        self.resources.stats.content_scene_us += crate::session::metrics::micros(started);
        self.render(&scene, &bindings, target, TargetInit::SrcOver)
    }

    fn snapshot(&mut self, source: &Surface) -> Result<Surface, String> {
        let size = source.size();
        let target = self.overwritten(size.width, size.height)?;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_texture(
            source.texture().as_image_copy(),
            target.texture().as_image_copy(),
            source.texture().size(),
        );
        self.queue.submit([encoder.finish()]);
        let stats = &mut self.resources.stats;
        stats.copies += 1;
        stats.submits += 1;
        stats.copy_pixels += u64::from(size.width) * u64::from(size.height);
        Ok(target)
    }

    fn composite(
        &mut self,
        target: &mut Surface,
        source: &Surface,
        layer: &Layer,
    ) -> Result<(), String> {
        let started = std::time::Instant::now();
        let size = target.size();
        let mut scene = Scene::new(size.width, size.height);
        match layer {
            Layer::Clip(clip) => scene.push_clip_layer(&clip.path()),
            Layer::Opacity(alpha) | Layer::Backdrop { opacity: alpha, .. } => {
                scene.push_opacity_layer(*alpha)
            }
            Layer::Mask(_) => scene.push_layer(None, None, None, None, None),
            Layer::Blur(radius) => scene.push_layer(
                None,
                None,
                None,
                None,
                Some(crate::renderer::cpu_paint::blur_filter(*radius)),
            ),
        }
        let bindings = image(&mut scene, source);
        if let Layer::Mask(mask) = layer {
            for fade in &mask.layers {
                scene.push_layer(None, Some(crate::scene::mask::blend()), None, None, None);
                scene.set_paint(fade.gradient.clone());
                scene.set_paint_transform(fade.transform);
                scene.fill_path(&mask.path);
                scene.reset_paint_transform();
                scene.pop_layer();
            }
        }
        scene.pop_layer();
        self.resources.stats.composite_scene_us += crate::session::metrics::micros(started);
        self.render(&scene, &bindings, target, TargetInit::SrcOver)
    }

    fn filter(&mut self, source: &Surface, layer: &Layer) -> Result<Surface, String> {
        let Layer::Backdrop {
            clip,
            blur,
            saturation,
            ..
        } = layer
        else {
            return Err("UI_COMPOSITE: expected backdrop".into());
        };
        let key = self.backdrops.pop_front();
        let size = source.size();
        let Some(region) =
            crate::scene::backdrop_region::Region::new(*clip, size.width, size.height, *blur)?
        else {
            return self.blank(size.width, size.height);
        };
        let cached = key
            .as_ref()
            .and_then(|key| self.resources.backdrops.get(key));
        if cached.is_some() {
            self.resources.stats.backdrop_hits += 1;
        }
        let filtered = match cached {
            Some(filtered) => filtered,
            None => {
                self.resources.backdrop_filters += 1;
                self.resources.stats.backdrop_misses += 1;
                let filtered = self.filtered(source, &region, *blur, *saturation)?;
                if let Some(key) = key {
                    let bytes =
                        u64::from(region.filtered_width) * u64::from(region.filtered_height) * 4;
                    // A quarter of the frame's lease budget, itself under `compositeBytes`:
                    // cached images never crowd a frame's working surfaces into flattening.
                    let budget =
                        crate::renderer::gpu::surface::lease_budget(size.width, size.height) / 4;
                    self.resources
                        .backdrops
                        .insert(key, filtered.clone(), bytes, budget);
                }
                filtered
            }
        };
        let output = self.overwritten(size.width, size.height)?;
        let started = std::time::Instant::now();
        let mut scene = Scene::new(size.width, size.height);
        scene.push_clip_layer(&clip.path());
        let bindings = image_brush(&mut scene, &filtered, Default::default());
        scene.set_paint_transform(region.placement());
        scene.fill_rect(&region.bounds());
        scene.pop_layer();
        self.resources.stats.composite_scene_us += crate::session::metrics::micros(started);
        self.render(
            &scene,
            &bindings,
            &output,
            TargetInit::Clear(Default::default()),
        )?;
        Ok(output)
    }
}

impl<F> Gpu<'_, F> {
    /// The backdrop's region of `source`, blurred and saturated at the filter's resolution.
    fn filtered(
        &mut self,
        source: &Surface,
        region: &crate::scene::backdrop_region::Region,
        blur: f32,
        saturation: crate::scene::color_filter::Saturation,
    ) -> Result<Surface, String> {
        let cropped = self.overwritten(region.width, region.height)?;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut input = source.texture().as_image_copy();
        input.origin = wgpu::Origin3d {
            x: region.x as u32,
            y: region.y as u32,
            z: 0,
        };
        encoder.copy_texture_to_texture(
            input,
            cropped.texture().as_image_copy(),
            cropped.texture().size(),
        );
        self.queue.submit([encoder.finish()]);
        let captured = u64::from(region.width) * u64::from(region.height);
        let stats = &mut self.resources.stats;
        stats.copies += 1;
        stats.submits += 1;
        stats.copy_pixels += captured;
        stats.capture_pixels += captured;
        stats.filter_pixels += u64::from(region.filtered_width) * u64::from(region.filtered_height);
        let mut filtered = self.overwritten(region.filtered_width, region.filtered_height)?;
        let started = std::time::Instant::now();
        let mut scene = Scene::new(region.filtered_width, region.filtered_height);
        if blur > 0.0 {
            scene.push_filter_layer(region.filter.clone());
        }
        let bindings = image_brush(
            &mut scene,
            &cropped,
            vello_cpu::peniko::ImageSampler::new().with_extend(vello_cpu::peniko::Extend::Reflect),
        );
        scene.set_paint_transform(region.downscale());
        scene.fill_rect(&region.sampling_bounds);
        if blur > 0.0 {
            scene.pop_layer();
        }
        self.resources.stats.composite_scene_us += crate::session::metrics::micros(started);
        self.render(
            &scene,
            &bindings,
            &filtered,
            TargetInit::Clear(Default::default()),
        )?;
        drop(cropped);
        if saturation.amount() != 1.0 {
            let colored = self.overwritten(region.filtered_width, region.filtered_height)?;
            let pass = self
                .resources
                .saturation
                .get_or_insert_with(|| SaturationPass::new(self.device));
            let mut encoder = self.device.create_command_encoder(&Default::default());
            pass.apply(
                self.device,
                &mut encoder,
                filtered.texture(),
                colored.texture(),
                saturation,
            )?;
            self.queue.submit([encoder.finish()]);
            self.resources.stats.submits += 1;
            filtered = colored;
        }
        Ok(filtered)
    }
}
