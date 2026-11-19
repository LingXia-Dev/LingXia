//! Redraws a kept frame for new video frames without vello. Per video, the frame keeps two layers
//! of its rectangle, rendered once: what lies under the video and what lies above it.
//! A new frame then restores the rectangle from the first, draws the frame as one textured quad
//! in its clips, and blends the second over it.
use crate::scene::{node::ClipSnapshot, paint::Paint};
use vello_cpu::kurbo::Affine;

/// Clips a quad honours, the video's own rounded bounds included; more fall back to vello.
const CLIPS: usize = 6;
/// Videos one frame redraws this way; more fall back to vello.
const VIDEOS: usize = 4;

/// Where a video lands on the target, in device pixels.
struct Placement {
    index: usize,
    player: String,
    /// `[x0, y0, x1, y1]` of the image after fit and position.
    content: [f32; 4],
    /// `[x0, y0, x1, y1]` and clockwise radii from the top-left, innermost last.
    clips: Vec<([f32; 4], [f32; 4])>,
    /// Integer `[x, y, w, h]` that covers every pixel the video can touch.
    area: [u16; 4],
    /// Clips open at the video and their paint transforms, and its paint transform.
    open: Vec<(ClipSnapshot, Affine)>,
    transform: Affine,
}

/// A device transform that only scales and translates, as `[sx, sy, tx, ty]`.
fn axis_aligned(transform: Affine) -> Option<[f64; 4]> {
    let [a, b, c, d, e, f] = transform.as_coeffs();
    (b.abs() < 1e-6 && c.abs() < 1e-6 && a > 0.0 && d > 0.0).then_some([a, d, e, f])
}

fn map(scale: [f64; 4], rect: [f32; 4]) -> [f32; 4] {
    let [sx, sy, tx, ty] = scale;
    let [x, y, w, h] = rect.map(f64::from);
    [
        x * sx + tx,
        y * sy + ty,
        (x + w) * sx + tx,
        (y + h) * sy + ty,
    ]
    .map(|v| v as f32)
}

/// Clip in device pixels; `None` for shapes a rounded rectangle cannot state.
fn device_clip(scale: [f64; 4], clip: &ClipSnapshot) -> Option<([f32; 4], [f32; 4])> {
    if clip.transform.is_some() {
        return None;
    }
    let rounded = clip.rounded_box();
    let mut radii = [0f32; 4];
    for (out, [rx, ry]) in radii.iter_mut().zip(rounded.radii) {
        let (x, y) = (rx * scale[0], ry * scale[1]);
        if (x - y).abs() > 0.5 {
            return None;
        }
        *out = x.min(y) as f32;
    }
    Some((map(scale, rounded.bounds.map(|v| v as f32)), radii))
}

/// Every video of `display` with its geometry, or `None` when one of them cannot be drawn as
/// a quad: inside a group, under a rotation, or overlapping another video.
fn placements(display: &crate::DisplayList) -> Option<Vec<Placement>> {
    let device = Affine::scale(f64::from(display.scale));
    let (mut transform, mut clips, mut groups) = (Affine::IDENTITY, Vec::new(), 0usize);
    let mut found = Vec::new();
    for (index, paint) in display.commands.iter().enumerate() {
        match paint {
            Paint::Transform(matrix) => transform = *matrix,
            Paint::Clip(clip) => clips.push((*clip, transform)),
            Paint::PopClip => {
                clips.pop()?;
            }
            Paint::Opacity { .. }
            | Paint::Backdrop { .. }
            | Paint::Mask { .. }
            | Paint::Blur { .. } => groups += 1,
            Paint::PopOpacity | Paint::PopBackdrop | Paint::PopMask(_) | Paint::PopBlur => {
                groups = groups.checked_sub(1)?
            }
            Paint::WebHole { .. } => return None,
            Paint::Video {
                player,
                bounds,
                radius,
                fit,
                position,
            } => {
                if groups > 0 || found.len() == VIDEOS {
                    return None;
                }
                let scale = axis_aligned(device * transform)?;
                let size = super::video::frame_size(player)?;
                let image = crate::scene::image::placement_at(
                    size,
                    *bounds,
                    *fit,
                    *position,
                    display.scale,
                )
                .ok()?;
                let content = map(scale, image);
                let mut device_clips = Vec::new();
                for (clip, opened) in &clips {
                    device_clips.push(device_clip(axis_aligned(device * *opened)?, clip)?);
                }
                let own = ClipSnapshot {
                    radii: None,
                    bounds: *bounds,
                    radius: *radius,
                    transform: None,
                };
                device_clips.push(device_clip(scale, &own)?);
                if device_clips.len() > CLIPS {
                    return None;
                }
                let mut visible = content;
                for (rect, _) in &device_clips {
                    visible = [
                        visible[0].max(rect[0]),
                        visible[1].max(rect[1]),
                        visible[2].min(rect[2]),
                        visible[3].min(rect[3]),
                    ];
                }
                let x0 = visible[0].floor().max(0.0);
                let y0 = visible[1].floor().max(0.0);
                let x1 = visible[2].ceil().min(f32::from(display.width));
                let y1 = visible[3].ceil().min(f32::from(display.height));
                let area = if x1 > x0 && y1 > y0 {
                    [x0 as u16, y0 as u16, (x1 - x0) as u16, (y1 - y0) as u16]
                } else {
                    [0; 4]
                };
                found.push(Placement {
                    index,
                    player: player.clone(),
                    content,
                    clips: device_clips,
                    area,
                    open: clips.clone(),
                    transform,
                });
            }
            _ => {}
        }
    }
    let overlap = |a: [u16; 4], b: [u16; 4]| {
        a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
    };
    for (i, a) in found.iter().enumerate() {
        if found[i + 1..].iter().any(|b| overlap(a.area, b.area)) {
            return None;
        }
    }
    Some(found)
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Uniforms {
    target: [f32; 2],
    count: u32,
    /// The first row of the over layer in the layers texture.
    over: f32,
    content: [f32; 4],
    area: [f32; 4],
    rects: [[f32; 4]; CLIPS],
    radii: [[f32; 4]; CLIPS],
}

impl Uniforms {
    fn bytes(&self) -> &[u8] {
        // SAFETY: plain `repr(C)` floats and integers without padding bytes.
        unsafe {
            std::slice::from_raw_parts((self as *const Self).cast(), std::mem::size_of::<Self>())
        }
    }
}

struct Quad {
    player: String,
    area: [u16; 4],
    /// The first row of the under layer in `layers`.
    under_row: u16,
    uniforms: wgpu::BindGroup,
    layers: wgpu::Texture,
    over: wgpu::BindGroup,
}

/// The quads of one drawn revision, valid while its target slot holds it.
pub(crate) struct Plan {
    revision: u64,
    slot: usize,
    epoch: u64,
    quads: Vec<Quad>,
}

impl Plan {
    pub(crate) fn serves(&self, revision: u64, slot: usize) -> bool {
        self.revision == revision && self.slot == slot && self.epoch == super::video::epoch()
    }
}

pub(crate) struct Pipelines {
    format: wgpu::TextureFormat,
    video: wgpu::RenderPipeline,
    over: wgpu::RenderPipeline,
    uniforms: wgpu::BindGroupLayout,
    texture: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

const SHADER: &str = r#"
struct Quad {
    frame: vec2<f32>,
    count: u32,
    over: f32,
    content: vec4<f32>,
    area: vec4<f32>,
    rects: array<vec4<f32>, 6>,
    radii: array<vec4<f32>, 6>,
};
@group(0) @binding(0) var<uniform> quad: Quad;
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) pixel: vec2<f32>,
};

@vertex fn vertex(@builtin(vertex_index) index: u32) -> Out {
    let corner = vec2<f32>(f32(index & 1u), f32(index >> 1u));
    let origin = quad.area.xy;
    let pixel = origin + corner * quad.area.zw;
    var out: Out;
    out.position = vec4<f32>(
        pixel.x / quad.frame.x * 2.0 - 1.0,
        1.0 - pixel.y / quad.frame.y * 2.0,
        0.0,
        1.0,
    );
    out.pixel = pixel;
    return out;
}

fn rounded(p: vec2<f32>, rect: vec4<f32>, radii: vec4<f32>) -> f32 {
    let center = (rect.xy + rect.zw) * 0.5;
    let half = (rect.zw - rect.xy) * 0.5;
    let offset = p - center;
    let top = select(radii.x, radii.y, offset.x > 0.0);
    let bottom = select(radii.w, radii.z, offset.x > 0.0);
    let r = min(select(bottom, top, offset.y < 0.0), min(half.x, half.y));
    let q = abs(offset) - half + vec2<f32>(r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - r;
}

@fragment fn video(in: Out) -> @location(0) vec4<f32> {
    var coverage = clamp(0.5 - rounded(in.pixel, quad.content, vec4<f32>(0.0)), 0.0, 1.0);
    for (var i = 0u; i < quad.count; i++) {
        coverage = min(coverage, clamp(0.5 - rounded(in.pixel, quad.rects[i], quad.radii[i]), 0.0, 1.0));
    }
    let uv = (in.pixel - quad.content.xy) / (quad.content.zw - quad.content.xy);
    let color = textureSampleLevel(image, image_sampler, uv, 0.0);
    return vec4<f32>(color.rgb * color.a, color.a) * coverage;
}

@fragment fn over(in: Out) -> @location(0) vec4<f32> {
    return textureLoad(image, vec2<i32>(in.pixel - quad.area.xy + vec2<f32>(0.0, quad.over)), 0);
}
"#;

impl Pipelines {
    pub(crate) fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("LingXia video quad"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let uniforms = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("LingXia video quad uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let texture = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("LingXia video quad texture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("LingXia video quad"),
            bind_group_layouts: &[Some(&uniforms), Some(&texture)],
            immediate_size: 0,
        });
        let pipeline = |entry: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("LingXia video quad"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vertex"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            format,
            video: pipeline("video"),
            over: pipeline("over"),
            uniforms,
            texture,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("LingXia video quad"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
        }
    }

    fn texture(&self, device: &wgpu::Device, view: &wgpu::TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("LingXia video quad texture"),
            layout: &self.texture,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

/// What drawing a plan needs from the presenter.
pub(crate) struct Draw<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub renderer: &'a mut super::pass::SceneRenderer,
    pub glyph_cache: &'a mut glifo::GlyphPrepCache,
    pub images: &'a mut super::images::ImageTextures,
    /// Glyph bitmaps the kept page scenes draw, so their textures stay cached.
    pub page_glyphs: Vec<std::sync::Arc<vello_gpu::Pixmap>>,
}

/// Builds the plan for `display` as drawn into `slot`; `None` when a video cannot be a quad.
/// Every video's under and over layers are rows of one texture, drawn by one vello render.
pub(crate) fn plan(
    draw: &mut Draw<'_>,
    pipelines: &Pipelines,
    display: &crate::DisplayList,
    revision: u64,
    slot: usize,
) -> Result<Option<Plan>, String> {
    let Some(placements) = placements(display) else {
        return Ok(None);
    };
    let placements: Vec<_> = placements
        .into_iter()
        .filter(|p| p.area[2] > 0 && p.area[3] > 0)
        .collect();
    let width = placements.iter().map(|p| p.area[2]).max().unwrap_or(0);
    let height: u32 = placements.iter().map(|p| 2 * u32::from(p.area[3])).sum();
    let limit = draw
        .device
        .limits()
        .max_texture_dimension_2d
        .min(u32::from(u16::MAX));
    if height > limit {
        return Ok(None);
    }
    let layers = (width > 0)
        .then(|| render(draw, display, &placements, [width, height as u16]))
        .transpose()?;
    let target = [f32::from(display.width), f32::from(display.height)];
    let mut quads = Vec::new();
    let mut row = 0u16;
    for placement in &placements {
        let layers = layers.as_ref().ok_or("video quad: no layers")?;
        let mut uniforms = Uniforms {
            target,
            count: placement.clips.len() as u32,
            over: f32::from(row + placement.area[3]),
            content: placement.content,
            area: placement.area.map(f32::from),
            rects: [[0.0; 4]; CLIPS],
            radii: [[0.0; 4]; CLIPS],
        };
        for (index, (rect, radii)) in placement.clips.iter().enumerate() {
            uniforms.rects[index] = *rect;
            uniforms.radii[index] = *radii;
        }
        let buffer = draw.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("LingXia video quad"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        draw.queue.write_buffer(&buffer, 0, uniforms.bytes());
        let uniforms = draw.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("LingXia video quad uniforms"),
            layout: &pipelines.uniforms,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        quads.push(Quad {
            player: placement.player.clone(),
            area: placement.area,
            under_row: row,
            uniforms,
            over: pipelines.texture(draw.device, &layers.create_view(&Default::default())),
            layers: layers.clone(),
        });
        row += 2 * placement.area[3];
    }
    Ok(Some(Plan {
        revision,
        slot,
        epoch: super::video::epoch(),
        quads,
    }))
}

/// Each video's under layer (the commands before it) and over layer (those after it, inside
/// the clips open at it), stacked as rows of one texture `size` wide and high.
fn render(
    draw: &mut Draw<'_>,
    display: &crate::DisplayList,
    placements: &[Placement],
    size: [u16; 2],
) -> Result<wgpu::Texture, String> {
    let images = display
        .bitmaps()
        .map(|bitmap| {
            let pixels = bitmap.pixels();
            (
                std::sync::Arc::as_ptr(pixels) as usize,
                super::text::source(pixels),
            )
        })
        .collect();
    let mut scene = vello_gpu::Scene::new(size[0], size[1]);
    let mut glyph_images = std::collections::HashMap::new();
    let glyph_cache = &mut *draw.glyph_cache;
    let count = display.commands.len();
    draw.renderer
        .with_resources(draw.device, size[0], size[1], |resources| {
            let mut glyphs = super::text::Glyphs {
                prep: glyph_cache,
                atlas: Some(resources),
            };
            let mut row = 0;
            for placement in placements {
                let under = super::scene::Segment {
                    animated: false,
                    range: 0..placement.index,
                    background: true,
                    clips: Vec::new(),
                    transform: Affine::IDENTITY,
                    open_at_end: placement.open.len(),
                };
                let over = super::scene::Segment {
                    animated: false,
                    range: placement.index + 1..count,
                    background: false,
                    clips: placement.open.clone(),
                    transform: placement.transform,
                    open_at_end: 0,
                };
                for segment in [&under, &over] {
                    display.sparse_area(
                        &mut glyphs,
                        &mut scene,
                        &images,
                        &mut glyph_images,
                        segment,
                        placement.area,
                        row,
                    )?;
                    row += placement.area[3];
                }
            }
            Ok::<_, String>(())
        })??;
    let pixels = display
        .bitmaps()
        .map(|bitmap| bitmap.pixels().clone())
        .chain(draw.page_glyphs.iter().cloned())
        .chain(glyph_images.values().cloned());
    let mut bindings = draw.images.bindings(draw.device, draw.queue, pixels);
    super::video::bind(display, &mut bindings);
    let texture = draw.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("LingXia video layers"),
        size: wgpu::Extent3d {
            width: size[0].into(),
            height: size[1].into(),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let size = vello_gpu::RenderSize {
        width: size[0],
        height: size[1],
    };
    let depth = vello_gpu::Renderer::create_depth_texture_view(draw.device, &size);
    draw.renderer.draw(
        draw.device,
        draw.queue,
        &scene,
        &bindings,
        super::pass::Target {
            size,
            view: &texture.create_view(&Default::default()),
            depth: Some(&depth),
            init: vello_gpu::TargetInit::Clear(vello_gpu::ClearSettings::default()),
        },
    )?;
    Ok(texture)
}

/// Draws each video's newest frame into the target; false when a player has no frame bound.
pub(crate) fn redraw(
    plan: &Plan,
    pipelines: &Pipelines,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &wgpu::TextureView,
) -> bool {
    let mut groups = Vec::with_capacity(plan.quads.len());
    for quad in &plan.quads {
        let Some(view) = super::video::frame_view(&quad.player) else {
            return false;
        };
        groups.push(pipelines.texture(device, &view));
    }
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("LingXia video quads"),
    });
    let span = super::timing::begin(&mut encoder);
    for quad in &plan.quads {
        let [x, y, w, h] = quad.area.map(u32::from);
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &quad.layers,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: u32::from(quad.under_row),
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: target.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("LingXia video quads"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        for (quad, frame) in plan.quads.iter().zip(&groups) {
            let [x, y, w, h] = quad.area.map(u32::from);
            pass.set_scissor_rect(x, y, w, h);
            pass.set_bind_group(0, &quad.uniforms, &[]);
            pass.set_pipeline(&pipelines.video);
            pass.set_bind_group(1, frame, &[]);
            pass.draw(0..4, 0..1);
            pass.set_pipeline(&pipelines.over);
            pass.set_bind_group(1, &quad.over, &[]);
            pass.draw(0..4, 0..1);
        }
    }
    super::timing::end(&mut encoder, span);
    queue.submit([encoder.finish()]);
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_quad_pipelines_build_on_any_adapter() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        else {
            return;
        };
        let (device, _queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        super::Pipelines::new(&device, wgpu::TextureFormat::Rgba8Unorm);
        let error = pollster::block_on(scope.pop());
        assert!(error.is_none(), "{error:?}");
    }
}
