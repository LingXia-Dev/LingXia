//! The D3D11 side: one instanced quad pass, one glyph atlas.
//!
//! Every cell background, glyph, rule and cursor is the same primitive — a
//! rectangle with a color and an atlas region — so the whole grid is one
//! vertex-less draw over an instance buffer. Solid rectangles point at the
//! atlas's reserved opaque texel, which keeps the shader branchless.
//!
//! Colors are linear in the buffer and the render target is `_SRGB`, so the
//! GPU does the encode and blending happens in the space it is correct in.

use std::collections::HashMap;
use std::ops::Range;

use windows::Win32::Graphics::Direct3D::Fxc::{D3DCOMPILE_OPTIMIZATION_LEVEL3, D3DCompile};
use windows::Win32::Graphics::Direct3D::{D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP, ID3DBlob};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_APPEND_ALIGNED_ELEMENT, D3D11_BIND_CONSTANT_BUFFER, D3D11_BIND_SHADER_RESOURCE,
    D3D11_BIND_VERTEX_BUFFER, D3D11_BLEND_DESC, D3D11_BLEND_INV_SRC_ALPHA, D3D11_BLEND_ONE,
    D3D11_BLEND_OP_ADD, D3D11_BUFFER_DESC, D3D11_COLOR_WRITE_ENABLE_ALL, D3D11_CPU_ACCESS_WRITE,
    D3D11_CULL_NONE, D3D11_FILL_SOLID, D3D11_FILTER_MIN_MAG_LINEAR_MIP_POINT,
    D3D11_FILTER_MIN_MAG_MIP_POINT, D3D11_INPUT_ELEMENT_DESC, D3D11_INPUT_PER_INSTANCE_DATA,
    D3D11_MAP_WRITE_DISCARD, D3D11_RASTERIZER_DESC, D3D11_RENDER_TARGET_BLEND_DESC,
    D3D11_SAMPLER_DESC, D3D11_SUBRESOURCE_DATA, D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, D3D11_USAGE_DYNAMIC, D3D11_VIEWPORT, ID3D11BlendState, ID3D11Buffer,
    ID3D11Device, ID3D11DeviceContext, ID3D11InputLayout, ID3D11PixelShader, ID3D11RasterizerState,
    ID3D11RenderTargetView, ID3D11SamplerState, ID3D11ShaderResourceView, ID3D11Texture2D,
    ID3D11VertexShader,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R32G32B32A32_FLOAT, DXGI_SAMPLE_DESC,
};
use windows::core::{Result, s};

/// One rectangle: position and size in pixels, linear premultiplied color,
/// the atlas region to modulate it by, and how to combine the two.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Quad {
    pub(super) rect: [f32; 4],
    pub(super) color: [f32; 4],
    pub(super) uv: [f32; 4],
    /// `x`: 1 for a sprite that carries its own color — an emoji — which is
    /// drawn as it is rather than tinted. The rest is padding the instance
    /// stride wants anyway.
    pub(super) params: [f32; 4],
}

pub(super) struct DrawBatch {
    pub(super) range: Range<usize>,
    pub(super) texture: Option<ID3D11ShaderResourceView>,
    pub(super) linear: bool,
}

const SHADER: &str = r#"
cbuffer Frame : register(b0) { float2 viewport; float2 _pad; };
Texture2D<float4> atlas : register(t0);
SamplerState atlas_sampler : register(s0);

struct Instance {
    float4 rect   : RECT;
    float4 color  : COLOR;
    float4 uv     : UV;
    float4 params : PARAMS;
};
struct Fragment {
    float4 position : SV_POSITION;
    float4 color    : COLOR;
    float2 uv       : TEXCOORD;
    float  colored  : COLORED;
};

Fragment vs_main(Instance instance, uint vertex : SV_VertexID) {
    float2 corner = float2(vertex & 1, (vertex >> 1) & 1);
    float2 vertex_position = instance.rect.xy + corner * instance.rect.zw;
    Fragment fragment;
    fragment.position = float4(vertex_position.x / viewport.x * 2.0 - 1.0,
                               1.0 - vertex_position.y / viewport.y * 2.0, 0.0, 1.0);
    fragment.color = instance.color;
    fragment.uv = lerp(instance.uv.xy, instance.uv.zw, corner);
    fragment.colored = instance.params.x;
    return fragment;
}

float4 ps_main(Fragment fragment) : SV_TARGET {
    float4 texel = atlas.Sample(atlas_sampler, fragment.uv);
    // A colored sprite is already premultiplied and keeps its own color; a
    // coverage sprite carries only alpha and takes the run's.
    return lerp(fragment.color * texel.a, texel, fragment.colored);
}
"#;

/// Atlas side, in texels. One page is plenty for a terminal's glyph set; a
/// full atlas is rebuilt rather than paged, which costs one frame and never
/// happens twice for the same content.
const ATLAS_SIDE: u32 = 1024;

/// Where a rasterized glyph landed, and how to place it against the pen.
#[derive(Clone, Copy)]
pub(super) struct Sprite {
    /// The sprite carries its own color and must not be tinted.
    pub(super) colored: bool,
    pub(super) uv: [f32; 4],
    pub(super) left: f32,
    pub(super) top: f32,
    pub(super) width: f32,
    pub(super) height: f32,
}

pub(super) struct Pipeline {
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    layout: ID3D11InputLayout,
    blend: ID3D11BlendState,
    /// Culling off: a screen-space quad must not depend on winding, and the
    /// default state culls back faces.
    rasterizer: ID3D11RasterizerState,
    sampler: ID3D11SamplerState,
    image_sampler: ID3D11SamplerState,
    frame: ID3D11Buffer,
    instances: ID3D11Buffer,
    capacity: usize,
    atlas: ID3D11Texture2D,
    atlas_view: ID3D11ShaderResourceView,
    /// The atlas is kept CPU-side and uploaded whole when it changes: glyphs
    /// stop arriving once the set is warm, and one upload has none of the
    /// row-pitch subtleties a partial one does. BGRA, premultiplied.
    pixels: Vec<u8>,
    dirty: bool,
    /// Shelf packing: the current row's origin and the tallest sprite in it.
    shelf_x: u32,
    shelf_y: u32,
    shelf_height: u32,
    sprites: HashMap<(u16, usize), Option<Sprite>>,
    /// UV of the reserved opaque texel, so solid fills share the glyph shader.
    pub(super) solid_uv: [f32; 4],
}

impl Pipeline {
    pub(super) fn new(device: &ID3D11Device) -> Result<Self> {
        let vertex_code = compile(s!("vs_main"), s!("vs_5_0"))?;
        let pixel_code = compile(s!("ps_main"), s!("ps_5_0"))?;
        unsafe {
            let vertex_bytes = blob_bytes(&vertex_code);
            let mut vertex = None;
            device.CreateVertexShader(vertex_bytes, None, Some(&mut vertex))?;
            let mut pixel = None;
            device.CreatePixelShader(blob_bytes(&pixel_code), None, Some(&mut pixel))?;

            let elements = [
                instance_element(s!("RECT"), 0),
                instance_element(s!("COLOR"), 0),
                instance_element(s!("UV"), 0),
                instance_element(s!("PARAMS"), 0),
            ];
            let mut layout = None;
            device.CreateInputLayout(&elements, vertex_bytes, Some(&mut layout))?;

            let mut blend = None;
            device.CreateBlendState(&premultiplied_blend(), Some(&mut blend))?;
            let mut rasterizer = None;
            device.CreateRasterizerState(
                &D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_SOLID,
                    CullMode: D3D11_CULL_NONE,
                    DepthClipEnable: true.into(),
                    ..Default::default()
                },
                Some(&mut rasterizer),
            )?;
            let mut sampler = None;
            device.CreateSamplerState(&point_sampler(), Some(&mut sampler))?;
            let mut image_sampler = None;
            device.CreateSamplerState(&image_sampler_desc(), Some(&mut image_sampler))?;

            let mut frame = None;
            device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: 16,
                    Usage: D3D11_USAGE_DYNAMIC,
                    BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                    CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut frame),
            )?;

            let (atlas, atlas_view) = create_atlas(device)?;
            let mut this = Self {
                vertex: vertex.ok_or_else(err)?,
                pixel: pixel.ok_or_else(err)?,
                layout: layout.ok_or_else(err)?,
                blend: blend.ok_or_else(err)?,
                rasterizer: rasterizer.ok_or_else(err)?,
                sampler: sampler.ok_or_else(err)?,
                image_sampler: image_sampler.ok_or_else(err)?,
                frame: frame.ok_or_else(err)?,
                instances: create_instances(device, 4096)?,
                capacity: 4096,
                atlas,
                atlas_view,
                pixels: vec![0; (ATLAS_SIDE * ATLAS_SIDE * 4) as usize],
                dirty: false,
                shelf_x: 0,
                shelf_y: 0,
                shelf_height: 0,
                sprites: HashMap::new(),
                solid_uv: [0.0; 4],
            };
            this.reserve_solid();
            Ok(this)
        }
    }

    /// Drop every cached sprite — the font changed, so none of them apply.
    pub(super) fn reset_glyphs(&mut self) {
        self.pixels.fill(0);
        self.sprites.clear();
        self.shelf_x = 0;
        self.shelf_y = 0;
        self.shelf_height = 0;
        self.reserve_solid();
    }

    /// A single opaque texel at the atlas origin, so a solid rectangle is the
    /// same draw as a glyph with its coverage forced to 1.
    fn reserve_solid(&mut self) {
        self.upload(0, 0, 1, 1, &[0xff, 0xff, 0xff, 0xff]);
        // Sample the texel's center; its edges would pick up the neighbours.
        let half = 0.5 / ATLAS_SIDE as f32;
        self.solid_uv = [half, half, half, half];
        self.shelf_x = 2;
        self.shelf_height = 2;
    }

    pub(super) fn sprite(&self, glyph: u16, style: usize) -> Option<Option<Sprite>> {
        self.sprites.get(&(glyph, style)).copied()
    }

    /// Place a rasterized glyph in the atlas. Returns `None` when it does not
    /// fit even after a rebuild, which a terminal's glyph set never reaches.
    pub(super) fn insert_sprite(
        &mut self,
        glyph: u16,
        style: usize,
        raster: Option<&super::text::Rasterized>,
    ) -> Option<Sprite> {
        let Some(raster) = raster else {
            self.sprites.insert((glyph, style), None);
            return None;
        };
        if self.shelf_x + raster.width > ATLAS_SIDE {
            self.shelf_x = 0;
            self.shelf_y += self.shelf_height;
            self.shelf_height = 0;
        }
        if self.shelf_y + raster.height > ATLAS_SIDE {
            self.reset_glyphs();
        }
        let (x, y) = (self.shelf_x, self.shelf_y);
        self.upload(x, y, raster.width, raster.height, &raster.pixels);
        self.shelf_x += raster.width + 1;
        self.shelf_height = self.shelf_height.max(raster.height + 1);

        let side = ATLAS_SIDE as f32;
        let sprite = Sprite {
            colored: raster.colored,
            uv: [
                x as f32 / side,
                y as f32 / side,
                (x + raster.width) as f32 / side,
                (y + raster.height) as f32 / side,
            ],
            left: raster.left as f32,
            top: raster.top as f32,
            width: raster.width as f32,
            height: raster.height as f32,
        };
        self.sprites.insert((glyph, style), Some(sprite));
        Some(sprite)
    }

    /// `pixels` is BGRA, premultiplied, `width * height * 4` bytes.
    fn upload(&mut self, x: u32, y: u32, width: u32, height: u32, pixels: &[u8]) {
        let stride = width as usize * 4;
        for row in 0..height {
            let source = row as usize * stride;
            let target = (((y + row) * ATLAS_SIDE + x) * 4) as usize;
            self.pixels[target..target + stride].copy_from_slice(&pixels[source..source + stride]);
        }
        self.dirty = true;
    }

    fn flush_atlas(&mut self, context: &ID3D11DeviceContext) {
        if !self.dirty {
            return;
        }
        unsafe {
            context.UpdateSubresource(
                &self.atlas,
                0,
                None,
                self.pixels.as_ptr().cast(),
                ATLAS_SIDE * 4,
                0,
            );
        }
        self.dirty = false;
    }

    /// Draw every quad in one pass.
    pub(super) fn draw(
        &mut self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        target: &ID3D11RenderTargetView,
        width: f32,
        height: f32,
        quads: &[Quad],
        batches: &[DrawBatch],
    ) -> Result<()> {
        if quads.is_empty() {
            return Ok(());
        }
        if quads.len() > self.capacity {
            self.capacity = quads.len().next_power_of_two();
            self.instances = create_instances(device, self.capacity)?;
        }
        self.flush_atlas(context);
        unsafe {
            write_dynamic(context, &self.instances, quads)?;
            write_dynamic(context, &self.frame, &[[width, height, 0.0, 0.0]])?;

            context.OMSetRenderTargets(Some(&[Some(target.clone())]), None);
            context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                Width: width,
                Height: height,
                MaxDepth: 1.0,
                ..Default::default()
            }]));
            context.RSSetState(&self.rasterizer);
            context.OMSetBlendState(&self.blend, None, u32::MAX);
            context.IASetInputLayout(&self.layout);
            context.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(self.instances.clone())),
                Some(&(size_of::<Quad>() as u32)),
                Some(&0),
            );
            context.VSSetShader(&self.vertex, None);
            context.VSSetConstantBuffers(0, Some(&[Some(self.frame.clone())]));
            context.PSSetShader(&self.pixel, None);
            for batch in batches {
                if batch.range.is_empty() {
                    continue;
                }
                let texture = batch.texture.as_ref().unwrap_or(&self.atlas_view);
                let sampler = if batch.linear {
                    &self.image_sampler
                } else {
                    &self.sampler
                };
                context.PSSetShaderResources(0, Some(&[Some(texture.clone())]));
                context.PSSetSamplers(0, Some(&[Some(sampler.clone())]));
                context.DrawInstanced(4, batch.range.len() as u32, 0, batch.range.start as u32);
            }
        }
        Ok(())
    }
}

fn err() -> windows::core::Error {
    windows::core::Error::from_thread()
}

fn compile(entry: windows::core::PCSTR, target: windows::core::PCSTR) -> Result<ID3DBlob> {
    let mut code = None;
    let mut errors = None;
    unsafe {
        let result = D3DCompile(
            SHADER.as_ptr().cast(),
            SHADER.len(),
            None,
            None,
            None,
            entry,
            target,
            D3DCOMPILE_OPTIMIZATION_LEVEL3,
            0,
            &mut code,
            Some(&mut errors),
        );
        if let Some(errors) = errors.filter(|_| result.is_err()) {
            let text = std::slice::from_raw_parts(
                errors.GetBufferPointer().cast::<u8>(),
                errors.GetBufferSize(),
            );
            log::error!("terminal shader: {}", String::from_utf8_lossy(text));
        }
        result?;
    }
    code.ok_or_else(err)
}

fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
    unsafe { std::slice::from_raw_parts(blob.GetBufferPointer().cast(), blob.GetBufferSize()) }
}

fn instance_element(name: windows::core::PCSTR, index: u32) -> D3D11_INPUT_ELEMENT_DESC {
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: name,
        SemanticIndex: index,
        Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
        InputSlot: 0,
        AlignedByteOffset: D3D11_APPEND_ALIGNED_ELEMENT,
        InputSlotClass: D3D11_INPUT_PER_INSTANCE_DATA,
        InstanceDataStepRate: 1,
    }
}

/// Source is already premultiplied, so the source factor is one.
fn premultiplied_blend() -> D3D11_BLEND_DESC {
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
        BlendEnable: true.into(),
        SrcBlend: D3D11_BLEND_ONE,
        DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
        BlendOp: D3D11_BLEND_OP_ADD,
        SrcBlendAlpha: D3D11_BLEND_ONE,
        DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
        BlendOpAlpha: D3D11_BLEND_OP_ADD,
        RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
    };
    desc
}

/// Point sampling: sprites are placed at whole pixels, so anything else only
/// resamples a bitmap that is already the right size.
fn point_sampler() -> D3D11_SAMPLER_DESC {
    D3D11_SAMPLER_DESC {
        Filter: D3D11_FILTER_MIN_MAG_MIP_POINT,
        AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
        MaxLOD: f32::MAX,
        ..Default::default()
    }
}

fn image_sampler_desc() -> D3D11_SAMPLER_DESC {
    D3D11_SAMPLER_DESC {
        Filter: D3D11_FILTER_MIN_MAG_LINEAR_MIP_POINT,
        AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
        MaxLOD: f32::MAX,
        ..Default::default()
    }
}

fn create_atlas(device: &ID3D11Device) -> Result<(ID3D11Texture2D, ID3D11ShaderResourceView)> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: ATLAS_SIDE,
        Height: ATLAS_SIDE,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        ..Default::default()
    };
    let blank = vec![0u8; (ATLAS_SIDE * ATLAS_SIDE * 4) as usize];
    let initial = D3D11_SUBRESOURCE_DATA {
        pSysMem: blank.as_ptr().cast(),
        SysMemPitch: ATLAS_SIDE * 4,
        SysMemSlicePitch: 0,
    };
    unsafe {
        let mut texture = None;
        device.CreateTexture2D(&desc, Some(&initial), Some(&mut texture))?;
        let texture = texture.ok_or_else(err)?;
        let mut view = None;
        device.CreateShaderResourceView(&texture, None, Some(&mut view))?;
        Ok((texture, view.ok_or_else(err)?))
    }
}

fn create_instances(device: &ID3D11Device, capacity: usize) -> Result<ID3D11Buffer> {
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: (capacity * size_of::<Quad>()) as u32,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        ..Default::default()
    };
    let mut buffer = None;
    unsafe { device.CreateBuffer(&desc, None, Some(&mut buffer))? };
    buffer.ok_or_else(err)
}

fn write_dynamic<T>(
    context: &ID3D11DeviceContext,
    buffer: &ID3D11Buffer,
    data: &[T],
) -> Result<()> {
    unsafe {
        let mut mapped = Default::default();
        context.Map(buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))?;
        std::ptr::copy_nonoverlapping(data.as_ptr(), mapped.pData.cast::<T>(), data.len());
        context.Unmap(buffer, 0);
    }
    Ok(())
}
