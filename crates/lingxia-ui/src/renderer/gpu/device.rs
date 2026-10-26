//! wgpu instance, devices and window surfaces, requested at what the renderer binds.

use wgpu::util::TextureBlitter;

/// What the renderer's pipelines bind, at most per stage: five sampled textures, one
/// sampler, one uniform buffer, four bind groups, no storage buffers or compute. Surfaces
/// are capped at 4096 px.
pub(crate) fn renderer_limits() -> wgpu::Limits {
    wgpu::Limits {
        max_texture_dimension_1d: 4096,
        max_texture_dimension_2d: 4096,
        ..wgpu::Limits::downlevel_webgl2_defaults()
    }
}

/// Which renderer limits `adapter` falls short of, as `name has < needed`.
pub(crate) fn shortfall(adapter: &wgpu::Limits) -> Vec<String> {
    let mut missing = Vec::new();
    renderer_limits().check_limits_with_fail_fn(adapter, false, |name, needed, has| {
        missing.push(format!("{name} {has} < {needed}"))
    });
    missing
}

pub(crate) struct Devices {
    pub instance: wgpu::Instance,
    pub devices: Vec<DeviceHandle>,
}

pub(crate) struct DeviceHandle {
    adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl DeviceHandle {
    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }
}

/// A window surface and two offscreen targets: one frame can wait for its vsync while the
/// next is drawn.
pub(crate) struct RenderSurface<'s> {
    pub surface: wgpu::Surface<'s>,
    pub config: wgpu::SurfaceConfiguration,
    pub dev_id: usize,
    pub target_views: [wgpu::TextureView; 2],
    pub blitter: TextureBlitter,
}

impl Devices {
    pub fn new(instance: wgpu::Instance) -> Self {
        Self {
            instance,
            devices: Vec::new(),
        }
    }

    pub async fn create_surface<'w>(
        &mut self,
        target: impl Into<wgpu::SurfaceTarget<'w>>,
        width: u32,
        height: u32,
    ) -> Result<RenderSurface<'w>, String> {
        let surface = self
            .instance
            .create_surface(target.into())
            .map_err(|e| e.to_string())?;
        let dev_id = self
            .device(Some(&surface))
            .await
            .ok_or("UI_SURFACE: no compatible GPU device")?;
        let handle = &self.devices[dev_id];
        let format = surface
            .get_capabilities(&handle.adapter)
            .formats
            .into_iter()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
                )
            })
            .ok_or("UI_SURFACE: unsupported surface format")?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            color_space: wgpu::SurfaceColorSpace::Auto,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        surface.configure(&handle.device, &config);
        Ok(RenderSurface {
            target_views: [0; 2].map(|_| target_view(&handle.device, width, height)),
            blitter: TextureBlitter::new(&handle.device, format),
            surface,
            config,
            dev_id,
        })
    }

    pub fn resize_surface(&self, surface: &mut RenderSurface<'_>, width: u32, height: u32) {
        let device = &self.devices[surface.dev_id].device;
        surface.config.width = width;
        surface.config.height = height;
        surface.target_views = [0; 2].map(|_| target_view(device, width, height));
        surface.surface.configure(device, &surface.config);
    }

    /// A device whose adapter supports `compatible`, created on first need.
    pub async fn device(&mut self, compatible: Option<&wgpu::Surface<'_>>) -> Option<usize> {
        let existing = match compatible {
            Some(surface) => self
                .devices
                .iter()
                .position(|d| d.adapter.is_surface_supported(surface)),
            None => (!self.devices.is_empty()).then_some(0),
        };
        if existing.is_some() {
            return existing;
        }
        let adapter =
            wgpu::util::initialize_adapter_from_env_or_default(&self.instance, compatible)
                .await
                .ok()?;
        let available = adapter.limits();
        if !shortfall(&available).is_empty() {
            return None;
        }
        // Texture size and alignment follow the adapter: the image atlas scales with them.
        let required_limits = renderer_limits()
            .using_resolution(available.clone())
            .using_alignment(available);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("LingXia UI"),
                required_limits,
                ..Default::default()
            })
            .await
            .ok()?;
        self.devices.push(DeviceHandle {
            adapter,
            device,
            queue,
        });
        Some(self.devices.len() - 1)
    }
}

/// The offscreen target: drawn by render passes, sampled by the blit and composition.
fn target_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("LingXia sparse target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gles_adapter_without_storage_buffers_meets_the_renderer_limits() {
        let gles = wgpu::Limits {
            max_storage_buffers_per_shader_stage: 0,
            max_texture_dimension_2d: 4096,
            max_texture_dimension_1d: 4096,
            ..wgpu::Limits::downlevel_webgl2_defaults()
        };
        assert!(shortfall(&gles).is_empty());
        let small = wgpu::Limits {
            max_texture_dimension_2d: 2048,
            ..gles
        };
        assert_eq!(
            shortfall(&small).len(),
            1,
            "a 4096 px surface needs 4096 px textures"
        );
    }
}
