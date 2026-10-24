use glifo::{DrawSink, GlyphRenderer};
use std::{collections::HashMap, sync::Arc};
use vello_cpu::kurbo::{Affine, BezPath, Rect};
use vello_gpu::{Image, ImageSource, Pixmap, Scene};

/// Glifo can emit bitmap glyphs even without an atlas. Hydrate them just like app images.
pub(crate) struct GlyphSink<'a> {
    pub scene: &'a mut Scene,
    pub images: &'a mut HashMap<usize, Arc<Pixmap>>,
}

pub(crate) fn source(pixels: &Arc<Pixmap>) -> ImageSource {
    ImageSource::external_texture(
        vello_gpu::TextureId(Arc::as_ptr(pixels) as usize as u64),
        vello_gpu::RectU16::new(0, 0, pixels.width(), pixels.height()),
        true,
    )
}

impl DrawSink for GlyphSink<'_> {
    fn set_transform(&mut self, t: Affine) {
        self.scene.set_transform(t);
    }
    fn set_paint(&mut self, paint: glifo::AtlasPaint) {
        self.scene.set_paint(paint);
    }
    fn set_paint_transform(&mut self, t: Affine) {
        self.scene.set_paint_transform(t);
    }
    fn fill_path(&mut self, path: &BezPath) {
        self.scene.fill_path(path);
    }
    fn fill_rect(&mut self, rect: &Rect) {
        self.scene.fill_rect(rect);
    }
    fn push_clip_layer(&mut self, clip: &BezPath) {
        self.scene.push_clip_layer(clip);
    }
    fn push_clip_path(&mut self, clip: &BezPath) {
        self.scene.push_clip_path(clip);
    }
    fn push_blend_layer(&mut self, blend: vello_cpu::peniko::BlendMode) {
        self.scene.push_blend_layer(blend);
    }
    fn pop_layer(&mut self) {
        self.scene.pop_layer();
    }
    fn pop_clip_path(&mut self) {
        self.scene.pop_clip();
    }
    fn width(&self) -> u16 {
        self.scene.width()
    }
    fn height(&self) -> u16 {
        self.scene.height()
    }
}

impl GlyphRenderer for GlyphSink<'_> {
    type SavedState = <Scene as GlyphRenderer>::SavedState;
    fn save_state(&mut self) -> Self::SavedState {
        self.scene.save_state()
    }
    fn restore_state(&mut self, state: Self::SavedState) {
        self.scene.restore_state(state);
    }
    fn stroke_path(&mut self, path: &BezPath) {
        self.scene.stroke_path(path);
    }
    fn set_paint_image(&mut self, mut image: Image) {
        if let ImageSource::Pixmap(pixels) = &image.image {
            self.images
                .insert(Arc::as_ptr(pixels) as usize, pixels.clone());
            image.image = source(pixels);
        }
        self.scene.set_paint(image);
    }
    fn set_tint(&mut self, tint: Option<vello_gpu::Tint>) {
        self.scene.set_tint(tint);
    }
    fn get_context_color(&self) -> vello_cpu::color::AlphaColor<vello_cpu::color::Srgb> {
        self.scene.get_context_color()
    }
    fn current_paint(&self) -> &vello_gpu::PaintType {
        self.scene.current_paint()
    }
    fn atlas_image_source(&self, slot: &glifo::AtlasSlot) -> ImageSource {
        self.scene.atlas_image_source(slot)
    }
    fn atlas_paint_transform(&self, slot: &glifo::AtlasSlot) -> Affine {
        self.scene.atlas_paint_transform(slot)
    }
}

pub(crate) struct Backend<'a> {
    pub sink: GlyphSink<'a>,
    pub cache: &'a mut glifo::GlyphPrepCache,
}

impl<'a> glifo::GlyphRunBackend<'a> for Backend<'a> {
    fn atlas_cache(self, _: bool) -> Self {
        self
    }
    fn fill_glyphs<G: Iterator<Item = glifo::Glyph> + Clone>(
        mut self,
        run: glifo::GlyphRun<'a>,
        glyphs: G,
    ) -> Result<(), glifo::GlyphRenderError> {
        run.build(glyphs, self.cache.as_mut(), glifo::AtlasCacher::Disabled)
            .fill_glyphs(&mut self.sink)
    }
    fn stroke_glyphs<G: Iterator<Item = glifo::Glyph> + Clone>(
        mut self,
        run: glifo::GlyphRun<'a>,
        glyphs: G,
    ) -> Result<(), glifo::GlyphRenderError> {
        let mut run = run.build(glyphs, self.cache.as_mut(), glifo::AtlasCacher::Disabled);
        let original = self.sink.scene.stroke().clone();
        let mut stroke = original.clone();
        stroke.width *= run.stroke_adjustment();
        self.sink.scene.set_stroke(stroke);
        let result = run.stroke_glyphs(&mut self.sink);
        self.sink.scene.set_stroke(original);
        result
    }
    fn render_decoration<G: Iterator<Item = glifo::Glyph> + Clone>(
        mut self,
        run: glifo::GlyphRun<'a>,
        glyphs: G,
        range: std::ops::RangeInclusive<f32>,
        baseline: f32,
        offset: f32,
        size: f32,
        buffer: f32,
    ) {
        run.build(glyphs, self.cache.as_mut(), glifo::AtlasCacher::Disabled)
            .render_decoration(range, baseline, offset, size, buffer, &mut self.sink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_pixmaps_become_shared_premultiplied_texture_bindings() {
        let pixels = Arc::new(Pixmap::new(17, 19));
        let mut scene = Scene::new(64, 64);
        let mut images = HashMap::new();
        let mut sink = GlyphSink {
            scene: &mut scene,
            images: &mut images,
        };
        for _ in 0..2 {
            sink.set_paint_image(Image {
                image: ImageSource::Pixmap(pixels.clone()),
                sampler: Default::default(),
            });
            let vello_gpu::PaintType::Image(image) = sink.current_paint() else {
                panic!("not an image")
            };
            let ImageSource::ExternalTexture {
                id, source_region, ..
            } = &image.image
            else {
                panic!("unuploaded glyph pixmap")
            };
            assert_eq!(
                *id,
                vello_gpu::TextureId(Arc::as_ptr(&pixels) as usize as u64)
            );
            assert_eq!(*source_region, vello_gpu::RectU16::new(0, 0, 17, 19));
        }
        assert_eq!(images.len(), 1);
        assert!(Arc::ptr_eq(images.values().next().unwrap(), &pixels));
    }
}
