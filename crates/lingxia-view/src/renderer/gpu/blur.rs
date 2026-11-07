//! Separable Gaussian blur at a reduced resolution, for backdrops and `blur-*` groups.
use wgpu::util::DeviceExt;

/// What the blur reads beyond its source rectangle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Edge {
    /// The source mirrored at its edge, as a backdrop sees what lies past its capture.
    Mirror,
    /// Nothing: a group surface already holds its blur's spread.
    Transparent,
}

/// Where a blur reads: a rectangle of a texture.
pub struct Source<'a> {
    pub texture: &'a wgpu::Texture,
    pub origin: (u16, u16),
    pub size: (u16, u16),
}

/// The longest kernel half-width, in reduced pixels.
const MAX_RADIUS: i32 = 256;

pub struct BlurPass {
    layout: wgpu::BindGroupLayout,
    reduce: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
}

impl BlurPass {
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("LingXia blur bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(PARAMETERS as u64),
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("LingXia blur layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("blur.wgsl"));
        let pipeline = |entry: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("LingXia blur"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vertex"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            reduce: pipeline("reduce"),
            blur: pipeline("blur"),
            layout,
        }
    }

    /// Writes `source` reduced by `scale` and blurred by `sigma` reduced pixels into `target`,
    /// which is the source size divided by `scale`, rounded up. `scratch` is target-sized.
    /// Three passes in `encoder`; the target is overwritten.
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: Source<'_>,
        scratch: &wgpu::Texture,
        target: &wgpu::Texture,
        scale: f32,
        sigma: f32,
        edge: Edge,
    ) -> Result<(), String> {
        let size = target.size();
        let reduced = (
            (f32::from(source.size.0) / scale).ceil() as u32,
            (f32::from(source.size.1) / scale).ceil() as u32,
        );
        let fits = |texture: &wgpu::Texture| {
            texture.format() == wgpu::TextureFormat::Rgba8Unorm
                && texture.usage().contains(
                    wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
                )
        };
        if !(scale >= 1.0 && sigma.is_finite() && sigma >= 0.0)
            || (size.width, size.height) != reduced
            || scratch.size() != size
            || u32::from(source.origin.0) + u32::from(source.size.0) > source.texture.width()
            || u32::from(source.origin.1) + u32::from(source.size.1) > source.texture.height()
            || !fits(target)
            || !fits(scratch)
            || source.texture.format() != wgpu::TextureFormat::Rgba8Unorm
        {
            return Err("UI_FILTER: blur textures do not fit".into());
        }
        let edge = match edge {
            Edge::Mirror => 0,
            Edge::Transparent => 1,
        };
        let origin = [i32::from(source.origin.0), i32::from(source.origin.1)];
        let full = [i32::from(source.size.0), i32::from(source.size.1)];
        let small = [size.width as i32, size.height as i32];
        let radius = ((3.0 * sigma).ceil() as i32).min(MAX_RADIUS);
        self.pass(
            device,
            encoder,
            &self.reduce,
            source.texture,
            target,
            Parameters {
                origin,
                source: full,
                edge,
                radius: 0,
                amount: scale,
                step: [0, 0],
            },
        );
        if radius == 0 {
            return Ok(());
        }
        for (from, to, step) in [(target, scratch, [1, 0]), (scratch, target, [0, 1])] {
            self.pass(
                device,
                encoder,
                &self.blur,
                from,
                to,
                Parameters {
                    origin: [0, 0],
                    source: small,
                    edge,
                    radius,
                    amount: sigma,
                    step,
                },
            );
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn pass(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        source: &wgpu::Texture,
        target: &wgpu::Texture,
        parameters: Parameters,
    ) {
        // Each pass owns its parameters: a later pass must not overwrite them before submit.
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("LingXia blur parameters"),
            contents: &parameters.bytes(),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let source_view = source.create_view(&Default::default());
        let target_view = target.create_view(&Default::default());
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("LingXia blur input"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffer.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("LingXia blur pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
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
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bindings, &[]);
        pass.draw(0..3, 0..1);
    }
}

const PARAMETERS: usize = 48;

struct Parameters {
    origin: [i32; 2],
    source: [i32; 2],
    edge: i32,
    radius: i32,
    amount: f32,
    step: [i32; 2],
}

impl Parameters {
    fn bytes(&self) -> [u8; PARAMETERS] {
        let words: [[u8; 4]; 12] = [
            self.origin[0].to_ne_bytes(),
            self.origin[1].to_ne_bytes(),
            self.source[0].to_ne_bytes(),
            self.source[1].to_ne_bytes(),
            self.edge.to_ne_bytes(),
            self.radius.to_ne_bytes(),
            self.amount.to_ne_bytes(),
            0f32.to_ne_bytes(),
            self.step[0].to_ne_bytes(),
            self.step[1].to_ne_bytes(),
            [0; 4],
            [0; 4],
        ];
        let mut bytes = [0; PARAMETERS];
        for (chunk, word) in bytes.chunks_exact_mut(4).zip(words) {
            chunk.copy_from_slice(&word);
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::backdrop_tests::readback;

    fn texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    /// Runs the blur over a `width`×`height` image whose pixel `(x, y)` is `paint(x, y)`
    /// (one channel value for all four); the middle row of the result.
    #[allow(clippy::too_many_arguments)]
    fn run(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u16,
        height: u16,
        paint: impl Fn(u16, u16) -> u8,
        scale: f32,
        sigma: f32,
        edge: Edge,
    ) -> Vec<u8> {
        let source = texture(device, width.into(), height.into());
        let mut pixels = Vec::new();
        for y in 0..height {
            for x in 0..width {
                pixels.extend([paint(x, y); 4]);
            }
        }
        queue.write_texture(
            source.as_image_copy(),
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(u32::from(width) * 4),
                rows_per_image: None,
            },
            source.size(),
        );
        let reduced = |n: u16| (f32::from(n) / scale).ceil() as u32;
        let target = texture(device, reduced(width), reduced(height));
        let scratch = texture(device, reduced(width), reduced(height));
        let mut encoder = device.create_command_encoder(&Default::default());
        BlurPass::new(device)
            .apply(
                device,
                &mut encoder,
                Source {
                    texture: &source,
                    origin: (0, 0),
                    size: (width, height),
                },
                &scratch,
                &target,
                scale,
                sigma,
                edge,
            )
            .unwrap();
        queue.submit([encoder.finish()]);
        let row = reduced(height) as usize / 2 * 512;
        readback(device, queue, &target)[row..row + reduced(width) as usize * 4]
            .chunks_exact(4)
            .map(|pixel| pixel[0])
            .collect()
    }

    #[test]
    #[ignore = "requires a GPU adapter; run explicitly with --features gpu -- --ignored"]
    fn blur_spreads_a_line_as_a_gaussian_and_reduces_by_area() {
        let domain = crate::renderer::gpu::Context::default();
        let mut context = domain.lock().unwrap();
        let id = pollster::block_on(context.device(None)).expect("GPU adapter required");
        let device = &context.devices[id].device;
        let queue = &context.devices[id].queue;

        let line = run(
            device,
            queue,
            96,
            4,
            |x, _| if x == 40 { 255 } else { 0 },
            1.0,
            3.0,
            Edge::Mirror,
        );
        let weight = |i: i32| (-0.5 * (i * i) as f32 / 9.0).exp();
        let total: f32 = (-9..=9).map(weight).sum();
        for (x, value) in line.iter().enumerate() {
            let offset = x as i32 - 40;
            let expected = if offset.abs() <= 9 {
                255.0 * weight(offset) / total
            } else {
                0.0
            };
            assert!(
                (f32::from(*value) - expected).abs() <= 1.0,
                "x={x}: {value} vs {expected}"
            );
        }

        // Reduced by 4 with no blur: each output pixel is the mean of the 4×4 block it covers.
        let halves = run(
            device,
            queue,
            96,
            8,
            |x, _| if x < 46 { 255 } else { 0 },
            4.0,
            0.0,
            Edge::Mirror,
        );
        assert_eq!(halves.len(), 24);
        assert!(halves[..11].iter().all(|v| *v == 255));
        assert!(halves[11].abs_diff(128) <= 1, "{}", halves[11]);
        assert!(halves[12..].iter().all(|v| *v == 0));

        // Mirrored edges keep a flat image flat however wide the blur.
        let flat = run(device, queue, 90, 8, |_, _| 200, 4.0, 6.0, Edge::Mirror);
        assert!(flat.iter().all(|v| v.abs_diff(200) <= 1), "{flat:?}");
        // A transparent edge lets a flat group fade toward its border.
        let fading = run(
            device,
            queue,
            90,
            96,
            |_, _| 200,
            4.0,
            2.0,
            Edge::Transparent,
        );
        assert!(
            fading[0] < 150 && fading[11].abs_diff(200) <= 1,
            "{fading:?}"
        );
    }
}
