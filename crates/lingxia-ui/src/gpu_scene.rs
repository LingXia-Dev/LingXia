use crate::{DisplayList, render::SceneKey};
use std::{collections::HashMap, sync::Arc};

struct Entry {
    scene: vello_gpu::Scene,
    key: Option<SceneKey>,
    glyph_images: HashMap<usize, Arc<vello_gpu::Pixmap>>,
}

impl Entry {
    fn new() -> Self {
        Self {
            scene: vello_gpu::Scene::new(1, 1),
            key: None,
            glyph_images: HashMap::new(),
        }
    }

    fn matches(&self, display: &DisplayList) -> bool {
        self.key.as_ref().is_some_and(|key| key.matches(display))
    }

    fn build(
        &mut self,
        display: &DisplayList,
        glyphs: &mut glifo::GlyphPrepCache,
    ) -> Result<(), String> {
        self.key = None;
        self.glyph_images.clear();
        let images = display
            .bitmaps()
            .map(|bitmap| {
                let pixels = bitmap.pixels();
                (
                    Arc::as_ptr(pixels) as usize,
                    crate::gpu_text::source(pixels),
                )
            })
            .collect();
        display.sparse_scene(glyphs, &mut self.scene, &images, &mut self.glyph_images)
    }
}

/// Keeps at most two eligible sparse scenes for recurring caret phases.
pub(crate) struct SceneCache {
    current: Entry,
    previous: Option<Entry>,
}

impl Default for SceneCache {
    fn default() -> Self {
        Self {
            current: Entry::new(),
            previous: None,
        }
    }
}

impl SceneCache {
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn prepare(
        &mut self,
        display: &DisplayList,
        glyphs: &mut glifo::GlyphPrepCache,
    ) -> Result<bool, String> {
        if self.current.matches(display) {
            return Ok(true);
        }
        if self
            .previous
            .as_ref()
            .is_some_and(|entry| entry.matches(display))
        {
            std::mem::swap(&mut self.current, self.previous.as_mut().unwrap());
            return Ok(true);
        }
        let key = SceneKey::capture(display);
        if key.is_none() {
            self.previous = None;
            self.current.build(display, glyphs)?;
            return Ok(false);
        }
        let mut next = self.previous.take().unwrap_or_else(Entry::new);
        next.build(display, glyphs)?;
        let bitmap_bytes: usize = next
            .glyph_images
            .values()
            .map(|pixels| usize::from(pixels.width()) * usize::from(pixels.height()) * 4)
            .sum();
        next.key = (bitmap_bytes <= 8 * 1024 * 1024).then_some(key).flatten();
        let cacheable = next.key.is_some();
        let old = std::mem::replace(&mut self.current, next);
        // Drop an oversized uncached scene instead of retaining its allocations as a spare.
        self.previous = (cacheable && old.key.is_some()).then_some(old);
        Ok(false)
    }

    pub(crate) fn scene(&self) -> &vello_gpu::Scene {
        &self.current.scene
    }

    pub(crate) fn glyph_images(&self) -> impl Iterator<Item = &Arc<vello_gpu::Pixmap>> {
        self.current.glyph_images.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recurring_caret_phases_reuse_two_scenes_and_clear_releases_them() {
        let module = crate::session::tests::source(
            "function render(){return h('input',{value:'Hello',autoFocus:true})}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let visible = page.layout(300, 100, 1.0).unwrap();
        let mut hidden = page.prepare_native(300, 100, 1.0, false, None, Ok).unwrap();
        hidden.hide_caret();
        let mut cache = SceneCache::default();
        let mut glyphs = glifo::GlyphPrepCache::default();
        assert!(!cache.prepare(&visible, &mut glyphs).unwrap());
        assert!(!cache.prepare(&hidden, &mut glyphs).unwrap());
        for _ in 0..4 {
            assert!(cache.prepare(&visible, &mut glyphs).unwrap());
            assert!(cache.prepare(&hidden, &mut glyphs).unwrap());
        }
        let changed = page.layout(320, 100, 1.0).unwrap();
        assert!(!cache.prepare(&changed, &mut glyphs).unwrap());
        assert!(
            !cache.prepare(&visible, &mut glyphs).unwrap(),
            "third scene evicts the oldest"
        );
        cache.clear();
        assert!(cache.previous.is_none());
        assert!(cache.current.key.is_none());
        assert!(!cache.prepare(&visible, &mut glyphs).unwrap());
    }

    #[test]
    fn oversized_viewports_bypass_and_discard_cached_scenes() {
        let module =
            crate::session::tests::source("function render(){return h('text',{},'Hello')}");
        let mut page = crate::ViewSession::new(&module).unwrap();
        let small = page.layout(200, 100, 1.0).unwrap();
        let large = page.layout(4096, 2048, 1.0).unwrap();
        let mut cache = SceneCache::default();
        let mut glyphs = glifo::GlyphPrepCache::default();
        assert!(!cache.prepare(&small, &mut glyphs).unwrap());
        assert!(cache.prepare(&small, &mut glyphs).unwrap());
        for _ in 0..2 {
            assert!(!cache.prepare(&large, &mut glyphs).unwrap());
            assert!(cache.previous.is_none());
            assert!(cache.current.key.is_none());
        }
        assert!(!cache.prepare(&small, &mut glyphs).unwrap());
        assert!(cache.previous.is_none());
    }

    #[test]
    fn selection_changes_invalidate_scene_even_when_text_is_unchanged() {
        let module = crate::session::tests::source(
            "function render(){return h('input',{value:'Hello',autoFocus:true})}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let frame = page.layout(300, 100, 1.0).unwrap();
        let id = frame.nodes[0].identity.clone();
        let token = frame.nodes[0].input.as_ref().unwrap().edit_token.unwrap();
        let mut cache = SceneCache::default();
        let mut glyphs = glifo::GlyphPrepCache::default();
        assert!(!cache.prepare(&frame, &mut glyphs).unwrap());
        for anchor in [0, 1] {
            page.edit_input(
                token,
                &id,
                &[crate::text_input::Edit::Selection { anchor, focus: 3 }],
                false,
            )
            .unwrap();
            let selected = page.prepare_native(300, 100, 1.0, false, None, Ok).unwrap();
            assert!(!cache.prepare(&selected, &mut glyphs).unwrap());
            assert!(cache.prepare(&selected, &mut glyphs).unwrap());
        }
    }
}
