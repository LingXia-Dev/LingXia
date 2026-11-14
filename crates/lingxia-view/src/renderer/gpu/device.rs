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
    /// `None` while trimmed; drawing creates them again.
    targets: Option<[wgpu::TextureView; 2]>,
    pub blitter: TextureBlitter,
}

impl RenderSurface<'_> {
    pub fn target(&self, slot: usize) -> Option<&wgpu::TextureView> {
        self.targets.as_ref().map(|targets| &targets[slot])
    }

    pub fn ensure_targets(&mut self, device: &wgpu::Device) {
        if self.targets.is_none() {
            self.targets = Some(new_targets(device, self.config.width, self.config.height));
        }
    }

    pub fn drop_targets(&mut self) {
        self.targets = None;
    }
}

impl Devices {
    pub fn new(instance: wgpu::Instance) -> Self {
        Self {
            instance,
            devices: Vec::new(),
        }
    }

    pub async fn create_surface(
        &mut self,
        source: &super::SurfaceSource,
        width: u32,
        height: u32,
    ) -> Result<RenderSurface<'static>, String> {
        let surface = source.create(&self.instance)?;
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
        configure(&surface, &handle.device, &config)?;
        Ok(RenderSurface {
            targets: Some(new_targets(&handle.device, width, height)),
            blitter: TextureBlitter::new(&handle.device, format),
            surface,
            config,
            dev_id,
        })
    }

    pub fn resize_surface(
        &self,
        surface: &mut RenderSurface<'_>,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let device = &self.devices[surface.dev_id].device;
        surface.config.width = width;
        surface.config.height = height;
        surface.targets = Some(new_targets(device, width, height));
        configure(&surface.surface, device, &surface.config)
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
        self.open(adapter).await
    }

    /// Opens a device on `adapter`; None when it falls short of the renderer's limits.
    pub async fn open(&mut self, adapter: wgpu::Adapter) -> Option<usize> {
        let available = adapter.limits();
        if !shortfall(&available).is_empty() {
            return None;
        }
        // Texture size and alignment follow the adapter: the image atlas scales with them.
        let required_limits = renderer_limits()
            .using_resolution(available.clone())
            .using_alignment(available);
        let descriptor = wgpu::DeviceDescriptor {
            label: Some("LingXia UI"),
            // Timestamps measure sampled redraws; nothing depends on them.
            required_features: adapter.features()
                & (wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::PIPELINE_CACHE),
            required_limits,
            ..Default::default()
        };
        #[cfg(target_os = "android")]
        let opened = video_device(&adapter, &descriptor);
        #[cfg(not(target_os = "android"))]
        let opened = None;
        let (device, queue) = match opened {
            Some(opened) => opened,
            None => adapter.request_device(&descriptor).await.ok()?,
        };
        self.devices.push(DeviceHandle {
            adapter,
            device,
            queue,
        });
        Some(self.devices.len() - 1)
    }
}

/// A Vulkan device that can import video frames' AHardwareBuffers, which wgpu leaves off;
/// `None` elsewhere or when the driver lacks the extension.
#[cfg(target_os = "android")]
fn video_device(
    adapter: &wgpu::Adapter,
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Option<(wgpu::Device, wgpu::Queue)> {
    use ash::{android::external_memory_android_hardware_buffer as ahb, ext, khr};
    let wanted = [
        ahb::NAME,
        ext::queue_family_foreign::NAME,
        khr::sampler_ycbcr_conversion::NAME,
        khr::external_memory::NAME,
        khr::dedicated_allocation::NAME,
        khr::get_memory_requirements2::NAME,
        khr::bind_memory2::NAME,
        khr::maintenance1::NAME,
    ];
    unsafe {
        let hal = adapter.as_hal::<wgpu::hal::api::Vulkan>()?;
        let capabilities = hal.physical_device_capabilities();
        if !capabilities.supports_extension(ahb::NAME) {
            return None;
        }
        let extra: Vec<&'static std::ffi::CStr> = wanted
            .into_iter()
            .filter(|name| capabilities.supports_extension(name))
            .collect();
        let opened = hal
            .open_with_callback(
                descriptor.required_features,
                &descriptor.required_limits,
                &descriptor.memory_hints,
                Some(Box::new(
                    move |args: wgpu::hal::vulkan::CreateDeviceCallbackArgs| {
                        for name in extra {
                            if !args.extensions.contains(&name) {
                                args.extensions.push(name);
                            }
                        }
                    },
                )),
            )
            .ok()?;
        drop(hal);
        adapter.create_device_from_hal(opened, descriptor).ok()
    }
}

/// Configures `surface`, reporting a failure instead of wgpu's default panic: a window that
/// still carries another EGL surface (`BadAlloc`) refuses one while it rotates.
fn configure(
    surface: &wgpu::Surface<'_>,
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
) -> Result<(), String> {
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    surface.configure(device, config);
    match pollster::block_on(scope.pop()) {
        None => Ok(()),
        Some(error) => Err(format!("{SURFACE_REFUSED}: {error}")),
    }
}

/// Prefix of the error for a window that refused its surface; binding again later may succeed.
pub const SURFACE_REFUSED: &str = "UI_SURFACE: the window refused a surface";

fn new_targets(device: &wgpu::Device, width: u32, height: u32) -> [wgpu::TextureView; 2] {
    [0; 2].map(|_| target_view(device, width, height))
}

pub(crate) const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The offscreen target: drawn by render passes, sampled by the blit and composition.
pub(crate) fn target_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
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
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
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
