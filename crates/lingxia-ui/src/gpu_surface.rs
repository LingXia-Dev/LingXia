use std::sync::{Arc, Mutex, Weak};

const MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone)]
struct Texture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl Texture {
    fn bytes(&self) -> u64 {
        u64::from(self.texture.width()) * u64::from(self.texture.height()) * 4
    }
}

#[derive(Default)]
struct State {
    free: Vec<Texture>,
    bytes: u64,
    generation: u64,
    allocations: u64,
    reuses: u64,
}

/// RGBA bytes plus depth texture/pixel counts; not total driver allocations.
#[derive(Clone, Copy, Debug)]
pub struct TextureStats {
    pub bytes: u64,
    pub idle_bytes: u64,
    pub allocations: u64,
    pub reuses: u64,
    pub depth_textures: usize,
    pub depth_pixels: u64,
    pub depth_allocations: u64,
}

struct Lease {
    resource: Texture,
    pool: Weak<Mutex<State>>,
    generation: u64,
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.upgrade() {
            let mut pool = pool.lock().unwrap();
            if pool.generation == self.generation {
                pool.free.push(self.resource.clone());
            } else {
                pool.bytes -= self.resource.bytes();
            }
        }
    }
}

/// Keep this lease (or its clone) alive until the last use has been submitted.
/// Cloning a raw texture handle does not extend the pool lease.
#[derive(Clone)]
pub struct Surface {
    lease: Arc<Lease>,
}

impl Surface {
    pub fn texture(&self) -> &wgpu::Texture {
        &self.lease.resource.texture
    }
    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.lease.resource.view
    }
    pub(crate) fn size(&self) -> vello_gpu::RenderSize {
        vello_gpu::RenderSize {
            width: self.texture().width() as u16,
            height: self.texture().height() as u16,
        }
    }
}

#[derive(Default)]
pub(crate) struct Pool {
    state: Arc<Mutex<State>>,
}

impl Pool {
    pub(crate) fn clear(&mut self) {
        let mut state = self.state.lock().unwrap();
        let idle_bytes: u64 = state.free.iter().map(Texture::bytes).sum();
        state.free.clear();
        state.bytes -= idle_bytes;
        state.generation += 1;
    }

    pub(crate) fn stats(&self) -> TextureStats {
        let state = self.state.lock().unwrap();
        TextureStats {
            bytes: state.bytes,
            idle_bytes: state.free.iter().map(Texture::bytes).sum(),
            allocations: state.allocations,
            reuses: state.reuses,
            depth_textures: 0,
            depth_pixels: 0,
            depth_allocations: 0,
        }
    }

    pub(crate) fn lease(
        &mut self,
        device: &wgpu::Device,
        width: u16,
        height: u16,
    ) -> Result<Surface, String> {
        let bytes = u64::from(width) * u64::from(height) * 4;
        if width == 0 || height == 0 || width > 4096 || height > 4096 || bytes > 16 * 1024 * 1024 {
            return Err("UI_COMPOSITE: invalid texture dimensions".into());
        }
        let mut state = self.state.lock().unwrap();
        let resource = if let Some(index) = state.free.iter().position(|entry| {
            entry.texture.width() == u32::from(width) && entry.texture.height() == u32::from(height)
        }) {
            state.reuses += 1;
            state.free.swap_remove(index)
        } else {
            while state.bytes + bytes > MAX_BYTES {
                let Some(evicted) = state.free.pop() else {
                    return Err("UI_COMPOSITE: live texture leases exceed budget".into());
                };
                state.bytes -= evicted.bytes();
            }
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("LingXia composition surface"),
                size: wgpu::Extent3d {
                    width: width as u32,
                    height: height as u32,
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
            });
            let view = texture.create_view(&Default::default());
            state.bytes += bytes;
            state.allocations += 1;
            Texture { texture, view }
        };
        Ok(Surface {
            lease: Arc::new(Lease {
                resource,
                pool: Arc::downgrade(&self.state),
                generation: state.generation,
            }),
        })
    }
}
