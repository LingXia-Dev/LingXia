use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

/// A render call and its staged resource uploads must reach the queue together.
#[derive(Clone, Default)]
pub(crate) struct SceneRenderer {
    state: Arc<Mutex<Option<(vello_gpu::Renderer, vello_gpu::Resources)>>>,
}

pub(crate) struct Target<'a> {
    pub size: vello_gpu::RenderSize,
    pub view: &'a wgpu::TextureView,
    pub depth: Option<&'a wgpu::TextureView>,
    pub init: vello_gpu::TargetInit<'a>,
}

impl SceneRenderer {
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
        let (renderer, resources) = state.get_or_insert_with(|| {
            vello_gpu::Renderer::new(
                device,
                &vello_gpu::RenderTargetConfig {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    width: target.size.width,
                    height: target.size.height,
                },
            )
        });
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
        let encode_us = crate::metrics::micros(started);
        let started = Instant::now();
        // Vello reuses buffers uploaded via queue.write_*. A later render call
        // must not replace those uploads before this scene has been submitted.
        queue.submit([commands]);
        Ok([encode_us, crate::metrics::micros(started)])
    }
}
