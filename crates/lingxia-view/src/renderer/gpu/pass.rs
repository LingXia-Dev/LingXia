use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

type State = Option<(vello_gpu::Renderer, vello_gpu::Resources)>;

/// A render call and its staged resource uploads must reach the queue together.
#[derive(Clone, Default)]
pub(crate) struct SceneRenderer {
    state: Arc<Mutex<State>>,
    /// Render calls so far: the glyph atlas ages its entries by them.
    renders: Arc<AtomicU64>,
}

/// Glyph atlas pages: 4 MB each at RGBA8, made on demand; a glyph that finds no room is
/// drawn from its outline as without the atlas.
const ATLAS: vello_common::multi_atlas::AtlasConfig = vello_common::multi_atlas::AtlasConfig {
    initial_atlas_count: 0,
    max_atlases: 4,
    atlas_size: (1024, 1024),
    auto_grow: true,
    allocation_strategy: vello_common::multi_atlas::AllocationStrategy::FirstFit,
};

fn created<'a>(
    state: &'a mut State,
    device: &wgpu::Device,
    width: u16,
    height: u16,
) -> &'a mut (vello_gpu::Renderer, vello_gpu::Resources) {
    state.get_or_insert_with(|| {
        vello_gpu::Renderer::new_with(
            device,
            &vello_gpu::RenderTargetConfig {
                format: wgpu::TextureFormat::Rgba8Unorm,
                width,
                height,
            },
            vello_gpu::RenderSettings {
                memory_settings: vello_gpu::MemorySettings {
                    image_atlas_config: ATLAS,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
    })
}

pub(crate) struct Target<'a> {
    pub size: vello_gpu::RenderSize,
    pub view: &'a wgpu::TextureView,
    pub depth: Option<&'a wgpu::TextureView>,
    pub init: vello_gpu::TargetInit<'a>,
}

impl SceneRenderer {
    /// The renderer's resources (its glyph atlas) while a scene records against them.
    pub(crate) fn with_resources<R>(
        &self,
        device: &wgpu::Device,
        width: u16,
        height: u16,
        record: impl FnOnce(&mut vello_gpu::Resources) -> R,
    ) -> Result<R, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "UI_SURFACE: poisoned scene renderer")?;
        Ok(record(&mut created(&mut state, device, width, height).1))
    }

    pub(crate) fn renders(&self) -> u64 {
        self.renders.load(Ordering::Relaxed)
    }

    /// Returns CPU encode/submit times; does not wait for GPU completion.
    pub(crate) fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &vello_gpu::Scene,
        bindings: &vello_gpu::TextureBindings,
        target: Target<'_>,
    ) -> Result<[u64; 2], String> {
        let started = Instant::now();
        // Keep shared scratch uploads and submission atomic across page/pass users.
        let mut state = self
            .state
            .lock()
            .map_err(|_| "UI_SURFACE: poisoned scene renderer")?;
        let (renderer, resources) =
            created(&mut state, device, target.size.width, target.size.height);
        self.renders.fetch_add(1, Ordering::Relaxed);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("LingXia sparse pass"),
        });
        renderer
            .render(
                scene,
                resources,
                device,
                queue,
                &mut encoder,
                &target.size,
                target.view,
                target.depth,
                bindings,
                target.init,
            )
            .map_err(|error| error.to_string())?;
        let commands = encoder.finish();
        let encode_us = crate::session::metrics::micros(started);
        let started = Instant::now();
        // Vello reuses buffers uploaded via queue.write_*. A later render call
        // must not replace those uploads before this scene has been submitted.
        queue.submit([commands]);
        Ok([encode_us, crate::session::metrics::micros(started)])
    }
}
