//! GPU implementation of the shared composition order; no CPU pixel readback.
use crate::{
    composite::{Backend, Layer},
    gpu_color_filter::SaturationPass,
    gpu_pass::{SceneRenderer, Target},
};
use std::ops::Range;
use vello_gpu::{Scene, TargetInit, TextureBindings, TextureId};

pub use crate::gpu_surface::{Surface, TextureStats};

/// Device-bound pipelines and scratch reused across frames. A draw callback and
/// its borrowed display list live only for the duration of `render`.
pub struct Compositor {
    renderer: SceneRenderer,
    pool: crate::gpu_surface::Pool,
    saturation: Option<SaturationPass>,
    depth: Vec<(u16, u16, wgpu::TextureView)>,
    depth_allocations: u64,
    images: crate::gpu_images::ImageTextures,
    glyph_cache: glifo::GlyphPrepCache,
}

impl Compositor {
    /// Discard idle textures and depth on suspend. Older leases cannot refill the pool.
    pub fn clear(&mut self) {
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
        }
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
        plan: &crate::composite::Plan,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        draw: F,
    ) -> Result<Surface, String>
    where
        F: FnMut(Range<usize>, &mut Scene, &mut TextureBindings) -> Result<(), String>,
    {
        plan.render(&mut Gpu {
            device,
            queue,
            resources: self,
            draw,
        })
    }

    /// Render a retained display using the same text/image encoders as Presenter.
    pub fn render_display(
        &mut self,
        display: &crate::DisplayList,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Surface, String> {
        let plan = display.composition_plan()?;
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
                    crate::gpu_text::source(pixels),
                )
            })
            .collect();
        let result = self.render(&plan, device, queue, |span, scene, bindings| {
            display.sparse_span(&mut glyph_cache, scene, &image_ids, &mut glyph_images, span)?;
            *bindings = images.bindings(
                device,
                queue,
                display
                    .bitmaps()
                    .map(|bitmap| bitmap.pixels().clone())
                    .chain(glyph_images.values().cloned()),
            );
            Ok(())
        });
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
}

impl<F> Gpu<'_, F> {
    fn render(
        &mut self,
        scene: &Scene,
        bindings: &TextureBindings,
        target: &Surface,
        init: TargetInit<'_>,
    ) -> Result<(), String> {
        let size = target.size();
        let depth = self.resources.depth_view(self.device, &size);
        self.resources.renderer.draw(
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
        Ok(surface)
    }

    fn draw(&mut self, target: &mut Surface, span: Range<usize>) -> Result<(), String> {
        if span.is_empty() {
            return Ok(());
        }
        let size = target.size();
        let mut scene = Scene::new(size.width, size.height);
        let mut bindings = TextureBindings::new();
        (self.draw)(span, &mut scene, &mut bindings)?;
        self.render(&scene, &bindings, target, TargetInit::SrcOver)
    }

    fn snapshot(&mut self, source: &Surface) -> Result<Surface, String> {
        let size = source.size();
        let target = self.blank(size.width, size.height)?;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_texture(
            source.texture().as_image_copy(),
            target.texture().as_image_copy(),
            source.texture().size(),
        );
        self.queue.submit([encoder.finish()]);
        Ok(target)
    }

    fn composite(
        &mut self,
        target: &mut Surface,
        source: &Surface,
        layer: &Layer,
    ) -> Result<(), String> {
        let size = target.size();
        let mut scene = Scene::new(size.width, size.height);
        match layer {
            Layer::Clip(clip) => scene.push_clip_layer(&clip.path()),
            Layer::Opacity(alpha) | Layer::Backdrop { opacity: alpha, .. } => {
                scene.push_opacity_layer(*alpha)
            }
        }
        let bindings = image(&mut scene, source);
        scene.pop_layer();
        self.render(&scene, &bindings, target, TargetInit::SrcOver)
    }

    fn filter(&mut self, source: Surface, layer: &Layer) -> Result<Surface, String> {
        let Layer::Backdrop {
            clip,
            blur,
            saturation,
            ..
        } = layer
        else {
            return Err("UI_COMPOSITE: expected backdrop".into());
        };
        let size = source.size();
        let Some(region) =
            crate::backdrop_region::Region::new(*clip, size.width, size.height, *blur)?
        else {
            drop(source);
            return self.blank(size.width, size.height);
        };
        let cropped = self.blank(region.width, region.height)?;
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
        drop(source);
        let mut filtered = self.blank(region.width, region.height)?;
        let mut scene = Scene::new(region.width, region.height);
        if *blur > 0.0 {
            scene.push_filter_layer(region.filter.clone());
        }
        let bindings = image_brush(
            &mut scene,
            &cropped,
            vello_cpu::peniko::ImageSampler::new().with_extend(vello_cpu::peniko::Extend::Reflect),
        );
        scene.fill_rect(&region.sampling_bounds);
        if *blur > 0.0 {
            scene.pop_layer();
        }
        self.render(
            &scene,
            &bindings,
            &filtered,
            TargetInit::Clear(Default::default()),
        )?;
        drop(cropped);
        if saturation.amount() != 1.0 {
            let colored = self.blank(region.width, region.height)?;
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
                *saturation,
            )?;
            self.queue.submit([encoder.finish()]);
            filtered = colored;
        }
        let output = self.blank(size.width, size.height)?;
        let mut scene = Scene::new(size.width, size.height);
        scene.push_clip_layer(&clip.path());
        let bindings = image_brush(&mut scene, &filtered, Default::default());
        scene.set_paint_transform(vello_cpu::kurbo::Affine::translate((
            f64::from(region.x),
            f64::from(region.y),
        )));
        scene.fill_rect(&region.bounds());
        scene.pop_layer();
        self.render(
            &scene,
            &bindings,
            &output,
            TargetInit::Clear(Default::default()),
        )?;
        Ok(output)
    }
}
