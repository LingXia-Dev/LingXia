use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Default)]
pub(crate) struct ImageTextures {
    images: HashMap<usize, (Arc<vello_gpu::Pixmap>, wgpu::TextureView)>,
}

impl ImageTextures {
    pub(crate) fn clear(&mut self) {
        self.images.clear();
    }

    /// Supply the full frame's live images, including bitmap glyphs discovered
    /// while encoding its passes. Cache identity owns the Pixmap allocation.
    pub(crate) fn bindings(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pixels: impl IntoIterator<Item = Arc<vello_gpu::Pixmap>>,
    ) -> vello_gpu::TextureBindings {
        let pixels: Vec<_> = pixels.into_iter().collect();
        let wanted: HashSet<_> = pixels
            .iter()
            .map(|pixels| Arc::as_ptr(pixels) as usize)
            .collect();
        self.images.retain(|key, _| wanted.contains(key));
        let mut bindings = vello_gpu::TextureBindings::new();
        for pixels in pixels {
            let key = Arc::as_ptr(&pixels) as usize;
            let entry = self.images.entry(key).or_insert_with(|| {
                let size = wgpu::Extent3d {
                    width: pixels.width().into(),
                    height: pixels.height().into(),
                    depth_or_array_layers: 1,
                };
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("LingXia image"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                queue.write_texture(
                    texture.as_image_copy(),
                    pixels.data_as_u8_slice(),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(size.width * 4),
                        rows_per_image: Some(size.height),
                    },
                    size,
                );
                (pixels, texture.create_view(&Default::default()))
            });
            bindings.insert(vello_gpu::TextureId(key as u64), entry.1.clone());
        }
        bindings
    }
}
