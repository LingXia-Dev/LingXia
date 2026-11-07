//! GPU implementation of the shared composition order; no CPU pixel readback.
use crate::{
    renderer::gpu::blur::{BlurPass, Edge, Source},
    renderer::gpu::color_filter::SaturationPass,
    renderer::gpu::pass::{SceneRenderer, Target},
    scene::composite::{Backend, Extent, Layer},
    scene::node::ClipSnapshot,
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
    /// Nested groups the texture budget drew flattened; nonzero means a visual downgrade.
    pub flattened: u32,
}

/// Device-bound pipelines and scratch reused across frames. A draw callback and
/// its borrowed display list live only for the duration of `render`.
pub struct Compositor {
    renderer: SceneRenderer,
    pool: crate::renderer::gpu::surface::Pool,
    saturation: Option<SaturationPass>,
    blur: Option<BlurPass>,
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
            blur: None,
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
    pub(crate) fn cache_backdrops(&mut self, surfaces: Vec<Surface>) {
        for (index, surface) in surfaces.into_iter().enumerate() {
            let size = surface.size();
            let bytes = u64::from(size.width) * u64::from(size.height) * 4;
            let key = crate::renderer::gpu::backdrop_cache::test_key(index as u16);
            self.backdrops.insert(key, surface, bytes, u64::MAX);
        }
    }

    #[cfg(test)]
    pub(crate) fn budget_warned(&self) -> bool {
        self.budget_warned
    }

    /// Fit `plan` beside the textures earlier frames still hold; a frame that does not fit
    /// draws its deepest groups flattened rather than failing. Also returns how many.
    pub(crate) fn fit(
        &mut self,
        plan: crate::scene::composite::Plan,
    ) -> (crate::scene::composite::Plan, u32) {
        let (width, height) = plan.dimensions();
        self.pool.set_frame(width, height);
        // Cached backdrops are only a speedup; they yield before the frame loses a group.
        while plan.working_bytes() > self.pool.available() && self.backdrops.evict_oldest() {}
        let available = self.pool.available();
        let (plan, flattened) = plan.within(available);
        if flattened > 0 && !self.budget_warned {
            self.budget_warned = true;
            log::warn!(
                target: "lingxia_view",
                "UI_BUDGET: this page's layers need more GPU texture memory than `leaseFrames` allows ({available} bytes free at {width}x{height}); {flattened} nested group(s) draw flattened"
            );
        }
        (plan, u32::try_from(flattened).unwrap_or(u32::MAX))
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
        F: FnMut(Range<usize>, (u16, u16), &mut Scene, &mut TextureBindings) -> Result<(), String>,
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
        F: FnMut(Range<usize>, (u16, u16), &mut Scene, &mut TextureBindings) -> Result<(), String>,
    {
        self.stats = CompositeStats::default();
        let mut gpu = Gpu {
            device,
            queue,
            resources: self,
            draw,
            backdrops,
            uncleared: Vec::new(),
            pending: Vec::new(),
        };
        let surface = plan.render(&mut gpu)?;
        gpu.settle(&surface)?;
        Ok(surface)
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
        let (plan, flattened) = self.fit(display.composition_plan()?);
        let backdrops = crate::renderer::gpu::backdrop_cache::keys(&plan, display);
        // Move caches out for the callback's lifetime; restore them on errors too.
        let mut images = std::mem::take(&mut self.images);
        let mut glyph_cache = std::mem::take(&mut self.glyph_cache);
        let mut glyph_images = std::collections::HashMap::new();
        let renderer = self.renderer.clone();
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
            |span, origin, scene, bindings| {
                renderer.with_resources(device, scene.width(), scene.height(), |resources| {
                    display.sparse_span(
                        &mut crate::renderer::gpu::text::Glyphs {
                            prep: &mut glyph_cache,
                            atlas: Some(resources),
                        },
                        scene,
                        &image_ids,
                        &mut glyph_images,
                        span,
                        origin,
                    )
                })??;
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
        self.stats.flattened = flattened;
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
    /// Blank surfaces not yet cleared: the first draw into one clears it in the same pass.
    uncleared: Vec<Surface>,
    /// Work recorded for a surface and not drawn yet: one pass draws it when the surface is read.
    pending: Vec<Pending>,
}

/// A surface's recorded work. Each pass loads and stores the whole surface, so content spans,
/// composited groups and placed backdrops share one pass until something reads the surface.
struct Pending {
    target: Surface,
    scene: Scene,
    /// Content draws' image bindings; compositor images bind apart, so a draw that replaces
    /// the set keeps them.
    content: Option<TextureBindings>,
    placed: Vec<(TextureId, wgpu::TextureView)>,
    /// Surfaces the scene samples, held so the pool cannot lease them out before the pass.
    sources: Vec<Surface>,
    held_bytes: u64,
}

impl Pending {
    /// Binds `source` under a new id; the scene's image paints use it.
    fn place(&mut self, source: &Surface) -> TextureId {
        let id = TextureId(1 + self.placed.len() as u64);
        let size = source.size();
        self.held_bytes += u64::from(size.width) * u64::from(size.height) * 4;
        self.placed.push((id, source.view().clone()));
        self.sources.push(source.clone());
        id
    }
}

impl<F> Gpu<'_, F> {
    /// A pooled surface for a target the next pass overwrites entirely: no clear pass, no submit.
    fn overwritten(&mut self, width: u16, height: u16) -> Result<Surface, String> {
        self.resources.pool.lease(self.device, width, height)
    }

    /// The recording for `target`, started if there is none.
    fn pending(&mut self, target: &Surface) -> usize {
        if let Some(index) = self.pending.iter().position(|p| p.target.same(target)) {
            return index;
        }
        let size = target.size();
        self.pending.push(Pending {
            target: target.clone(),
            scene: Scene::new(size.width, size.height),
            content: None,
            placed: Vec::new(),
            sources: Vec::new(),
            held_bytes: 0,
        });
        self.pending.len() - 1
    }

    /// The recording for `target`, drawn first if holding `source` too would keep more than a
    /// frame's worth of finished groups out of the pool.
    fn pending_beside(&mut self, target: &Surface, source: &Surface) -> Result<usize, String> {
        let size = source.size();
        let bytes = u64::from(size.width) * u64::from(size.height) * 4;
        let target_size = target.size();
        let frame = u64::from(target_size.width) * u64::from(target_size.height) * 4;
        if self
            .pending
            .iter()
            .any(|p| p.target.same(target) && p.held_bytes > 0 && p.held_bytes + bytes > frame)
        {
            self.settle(target)?;
        }
        Ok(self.pending(target))
    }

    /// Draw `surface`'s recorded work, or clear it if it is still blank, before it is read.
    fn settle(&mut self, surface: &Surface) -> Result<(), String> {
        if let Some(index) = self.pending.iter().position(|p| p.target.same(surface)) {
            let pending = self.pending.swap_remove(index);
            let mut bindings = pending.content.unwrap_or_default();
            for (id, view) in pending.placed {
                bindings.insert(id, view);
            }
            return self.render(&pending.scene, &bindings, surface, TargetInit::SrcOver);
        }
        if !self.take_uncleared(surface) {
            return Ok(());
        }
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
        Ok(())
    }

    fn take_uncleared(&mut self, surface: &Surface) -> bool {
        let Some(index) = self.uncleared.iter().position(|held| held.same(surface)) else {
            return false;
        };
        self.uncleared.swap_remove(index);
        true
    }

    fn render(
        &mut self,
        scene: &Scene,
        bindings: &TextureBindings,
        target: &Surface,
        init: TargetInit<'_>,
    ) -> Result<(), String> {
        let init = match init {
            TargetInit::SrcOver if self.take_uncleared(target) => {
                TargetInit::Clear(Default::default())
            }
            init => init,
        };
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

/// `source`, bound as `id`, drawn 1:1 with its top-left corner at `at`.
fn image(scene: &mut Scene, source: &Surface, id: TextureId, at: (f64, f64)) {
    image_paint(scene, source, Default::default(), id);
    let size = source.size();
    scene.set_paint_transform(vello_cpu::kurbo::Affine::translate(at));
    scene.fill_rect(&vello_cpu::kurbo::Rect::new(
        at.0,
        at.1,
        at.0 + size.width as f64,
        at.1 + size.height as f64,
    ));
    scene.reset_paint_transform();
}

fn size([x0, y0, x1, y1]: Extent) -> (u16, u16) {
    (x1 - x0, y1 - y0)
}

/// Draws in frame coordinates onto a surface covering `extent`.
fn origin(scene: &mut Scene, extent: Extent) {
    scene.set_transform(vello_cpu::kurbo::Affine::translate((
        -f64::from(extent[0]),
        -f64::from(extent[1]),
    )));
}

fn image_brush(
    scene: &mut Scene,
    source: &Surface,
    sampler: vello_cpu::peniko::ImageSampler,
) -> TextureBindings {
    let mut bindings = TextureBindings::new();
    bindings.insert(TextureId(1), source.view().clone());
    image_paint(scene, source, sampler, TextureId(1));
    bindings
}

/// Sets `source`, bound as `id`, as the paint.
fn image_paint(
    scene: &mut Scene,
    source: &Surface,
    sampler: vello_cpu::peniko::ImageSampler,
    id: TextureId,
) {
    let size = source.size();
    scene.set_paint(vello_gpu::Image {
        image: vello_gpu::ImageSource::external_texture(
            id,
            vello_gpu::RectU16::new(0, 0, size.width, size.height),
            true,
        ),
        sampler,
    });
}

impl<F> Backend for Gpu<'_, F>
where
    F: FnMut(Range<usize>, (u16, u16), &mut Scene, &mut TextureBindings) -> Result<(), String>,
{
    type Surface = Surface;

    const BOUNDED: bool = true;

    fn blank(&mut self, extent: Extent) -> Result<Surface, String> {
        let (width, height) = size(extent);
        let surface = self.resources.pool.lease(self.device, width, height)?;
        self.uncleared.push(surface.clone());
        Ok(surface)
    }

    fn draw(
        &mut self,
        target: &mut Surface,
        extent: Extent,
        span: Range<usize>,
        clips: &[ClipSnapshot],
    ) -> Result<(), String> {
        if span.is_empty() {
            return Ok(());
        }
        let started = std::time::Instant::now();
        let index = self.pending(target);
        let pending = &mut self.pending[index];
        origin(&mut pending.scene, extent);
        for clip in clips {
            pending.scene.push_clip_path(&clip.path());
        }
        (self.draw)(
            span,
            (extent[0], extent[1]),
            &mut pending.scene,
            pending.content.get_or_insert_default(),
        )?;
        for _ in clips {
            pending.scene.pop_clip();
        }
        self.resources.stats.content_scene_us += crate::session::metrics::micros(started);
        Ok(())
    }

    fn snapshot(&mut self, source: &Surface) -> Result<Surface, String> {
        self.settle(source)?;
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
        target_extent: Extent,
        source: &Surface,
        source_extent: Extent,
        layer: &Layer,
        clips: &[ClipSnapshot],
    ) -> Result<(), String> {
        self.settle(source)?;
        let blurred = match layer {
            Layer::Blur(radius) if *radius > 0.0 => Some(self.blurred(source, *radius)?),
            _ => None,
        };
        let started = std::time::Instant::now();
        let index = self.pending_beside(target, source)?;
        let pending = &mut self.pending[index];
        let at = (f64::from(source_extent[0]), f64::from(source_extent[1]));
        if let Some((blurred, scale)) = &blurred {
            let id = pending.place(blurred);
            let scene = &mut pending.scene;
            origin(scene, target_extent);
            for clip in clips {
                scene.push_clip_path(&clip.path());
            }
            image_paint(scene, blurred, Default::default(), id);
            scene.set_paint_transform(
                vello_cpu::kurbo::Affine::translate(at) * vello_cpu::kurbo::Affine::scale(*scale),
            );
            let (width, height) = size(source_extent);
            scene.fill_rect(&vello_cpu::kurbo::Rect::new(
                at.0,
                at.1,
                at.0 + f64::from(width),
                at.1 + f64::from(height),
            ));
            scene.reset_paint_transform();
            for _ in clips {
                scene.pop_clip();
            }
            self.resources.stats.composite_scene_us += crate::session::metrics::micros(started);
            return Ok(());
        }
        let id = pending.place(source);
        let scene = &mut pending.scene;
        origin(scene, target_extent);
        for clip in clips {
            scene.push_clip_path(&clip.path());
        }
        match layer {
            Layer::Clip(clip) => scene.push_clip_path(&clip.path()),
            Layer::Opacity(alpha) | Layer::Backdrop { opacity: alpha, .. } => {
                scene.push_opacity_layer(*alpha)
            }
            Layer::Mask(_) | Layer::Blur(_) => scene.push_layer(None, None, None, None, None),
        }
        image(scene, source, id, at);
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
        if matches!(layer, Layer::Clip(_)) {
            scene.pop_clip();
        } else {
            scene.pop_layer();
        }
        for _ in clips {
            scene.pop_clip();
        }
        self.resources.stats.composite_scene_us += crate::session::metrics::micros(started);
        Ok(())
    }

    fn filter(
        &mut self,
        source: &Surface,
        source_extent: Extent,
        layer: &Layer,
        frame: (u16, u16),
        extent: Extent,
    ) -> Result<Surface, String> {
        let Layer::Backdrop { clip, .. } = layer else {
            return Err("UI_COMPOSITE: expected backdrop".into());
        };
        match self.backdrop(source, source_extent, layer, frame)? {
            Some((filtered, region)) => self.place(&filtered, &region, clip, extent),
            None => self.blank(extent),
        }
    }

    const INLINE_BACKDROPS: bool = true;

    fn backdrop_into(
        &mut self,
        target: &mut Surface,
        target_extent: Extent,
        layer: &Layer,
        frame: (u16, u16),
        clips: &[ClipSnapshot],
    ) -> Result<(), String> {
        let Layer::Backdrop { clip, .. } = layer else {
            return Err("UI_COMPOSITE: expected backdrop".into());
        };
        let Some((filtered, region)) = self.backdrop(target, target_extent, layer, frame)? else {
            return Ok(());
        };
        let index = self.pending_beside(target, &filtered)?;
        let pending = &mut self.pending[index];
        let id = pending.place(&filtered);
        let scene = &mut pending.scene;
        origin(scene, target_extent);
        for clip in clips {
            scene.push_clip_path(&clip.path());
        }
        scene.push_clip_path(&clip.path());
        image_paint(scene, &filtered, Default::default(), id);
        scene.set_paint_transform(region.placement());
        scene.fill_rect(&region.bounds());
        scene.reset_paint_transform();
        scene.pop_clip();
        for _ in clips {
            scene.pop_clip();
        }
        Ok(())
    }

    fn reuse(
        &mut self,
        layer: &Layer,
        (width, height): (u16, u16),
        extent: Extent,
    ) -> Result<Option<Surface>, String> {
        let Layer::Backdrop { clip, blur, .. } = layer else {
            return Ok(None);
        };
        let Some(filtered) = self
            .backdrops
            .front()
            .and_then(|key| self.resources.backdrops.get(key))
        else {
            return Ok(None);
        };
        let Some(region) = crate::scene::backdrop_region::Region::new(*clip, width, height, *blur)?
        else {
            return Ok(None);
        };
        self.backdrops.pop_front();
        self.resources.stats.backdrop_hits += 1;
        self.place(&filtered, &region, clip, extent).map(Some)
    }
}

impl<F> Gpu<'_, F> {
    /// The backdrop's filtered image and region, from the cache while what lies beneath holds;
    /// `None` when the backdrop covers nothing. Consumes the backdrop's cache key.
    fn backdrop(
        &mut self,
        source: &Surface,
        source_extent: Extent,
        layer: &Layer,
        frame: (u16, u16),
    ) -> Result<Option<(Surface, crate::scene::backdrop_region::Region)>, String> {
        let Layer::Backdrop {
            clip,
            blur,
            saturation,
            ..
        } = layer
        else {
            return Err("UI_COMPOSITE: expected backdrop".into());
        };
        self.settle(source)?;
        let key = self.backdrops.pop_front();
        let Some(region) =
            crate::scene::backdrop_region::Region::new(*clip, frame.0, frame.1, *blur)?
        else {
            return Ok(None);
        };
        let cached = key
            .as_ref()
            .and_then(|key| self.resources.backdrops.get(key));
        if let Some(filtered) = cached {
            self.resources.stats.backdrop_hits += 1;
            return Ok(Some((filtered, region)));
        }
        self.resources.backdrop_filters += 1;
        self.resources.stats.backdrop_misses += 1;
        let filtered = self.filtered(source, source_extent, &region, *blur, *saturation)?;
        if let Some(key) = key {
            let bytes = u64::from(region.filtered_width) * u64::from(region.filtered_height) * 4;
            // A quarter of the frame's lease budget, itself under `compositeBytes`;
            // `fit` evicts them before a frame would flatten.
            let budget = crate::renderer::gpu::surface::lease_budget(frame.0, frame.1) / 4;
            self.resources
                .backdrops
                .insert(key, filtered.clone(), bytes, budget);
        }
        Ok(Some((filtered, region)))
    }

    /// The owner's surface: `filtered` stretched over the backdrop's region, within its clip.
    fn place(
        &mut self,
        filtered: &Surface,
        region: &crate::scene::backdrop_region::Region,
        clip: &crate::scene::node::ClipSnapshot,
        extent: Extent,
    ) -> Result<Surface, String> {
        let (width, height) = size(extent);
        let output = self.overwritten(width, height)?;
        let started = std::time::Instant::now();
        let mut scene = Scene::new(width, height);
        origin(&mut scene, extent);
        scene.push_clip_path(&clip.path());
        let bindings = image_brush(&mut scene, filtered, Default::default());
        scene.set_paint_transform(region.placement());
        scene.fill_rect(&region.bounds());
        scene.pop_clip();
        self.resources.stats.composite_scene_us += crate::session::metrics::micros(started);
        self.render(
            &scene,
            &bindings,
            &output,
            TargetInit::Clear(Default::default()),
        )?;
        Ok(output)
    }

    /// The backdrop's region of `source`, blurred and saturated at the filter's resolution.
    fn filtered(
        &mut self,
        source: &Surface,
        source_extent: Extent,
        region: &crate::scene::backdrop_region::Region,
        blur: f32,
        saturation: crate::scene::color_filter::Saturation,
    ) -> Result<Surface, String> {
        let (x, y) = (
            region.x.checked_sub(source_extent[0]),
            region.y.checked_sub(source_extent[1]),
        );
        let (Some(x), Some(y)) = (x, y) else {
            return Err("UI_COMPOSITE: backdrop region outside its source".into());
        };
        if region.x + region.width > source_extent[2] || region.y + region.height > source_extent[3]
        {
            return Err("UI_COMPOSITE: backdrop region outside its source".into());
        }
        let (width, height) = (region.filtered_width, region.filtered_height);
        let filtered = self.overwritten(width, height)?;
        let scratch = self.overwritten(width, height)?;
        let colored = if saturation.amount() != 1.0 {
            Some(self.overwritten(width, height)?)
        } else {
            None
        };
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.resources
            .blur
            .get_or_insert_with(|| BlurPass::new(self.device))
            .apply(
                self.device,
                &mut encoder,
                Source {
                    texture: source.texture(),
                    origin: (x, y),
                    size: (region.width, region.height),
                },
                scratch.texture(),
                filtered.texture(),
                region.scale,
                blur / region.scale,
                Edge::Mirror,
            )?;
        if let Some(colored) = &colored {
            self.resources
                .saturation
                .get_or_insert_with(|| SaturationPass::new(self.device))
                .apply(
                    self.device,
                    &mut encoder,
                    filtered.texture(),
                    colored.texture(),
                    saturation,
                )?;
        }
        self.queue.submit([encoder.finish()]);
        let stats = &mut self.resources.stats;
        stats.submits += 1;
        stats.capture_pixels += u64::from(region.width) * u64::from(region.height);
        stats.filter_pixels += u64::from(width) * u64::from(height);
        Ok(colored.unwrap_or(filtered))
    }

    /// A `blur-*` group's surface blurred by `radius` device pixels, at a fraction of its
    /// resolution, and that fraction's scale.
    fn blurred(&mut self, source: &Surface, radius: f32) -> Result<(Surface, f64), String> {
        let scale = crate::scene::backdrop_region::filter_scale(radius);
        let size = source.size();
        let reduce = |n: u16| (f32::from(n) / scale).ceil().max(1.0) as u16;
        let (width, height) = (reduce(size.width), reduce(size.height));
        let blurred = self.overwritten(width, height)?;
        let scratch = self.overwritten(width, height)?;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.resources
            .blur
            .get_or_insert_with(|| BlurPass::new(self.device))
            .apply(
                self.device,
                &mut encoder,
                Source {
                    texture: source.texture(),
                    origin: (0, 0),
                    size: (size.width, size.height),
                },
                scratch.texture(),
                blurred.texture(),
                scale,
                radius / scale,
                Edge::Transparent,
            )?;
        self.queue.submit([encoder.finish()]);
        let stats = &mut self.resources.stats;
        stats.submits += 1;
        stats.filter_pixels += u64::from(width) * u64::from(height);
        Ok((blurred, f64::from(scale)))
    }
}
