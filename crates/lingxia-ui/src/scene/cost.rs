//! Deterministic per-frame cost counters: what the corpus gate compares, never wall time.

use crate::scene::paint::Paint;

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameCost {
    pub nodes: usize,
    pub paints: usize,
    pub glyphs: usize,
    /// Glyphs the platform outlines (Apple hvgl faces), each filled as a path.
    pub outline_glyphs: usize,
    /// Path elements of vectors, plots and strokes, outlined.
    pub vector_elements: usize,
    /// Opacity groups, clips and backdrops, each an offscreen or clip layer.
    pub layers: usize,
    pub backdrops: usize,
    pub shadows: usize,
    pub images: usize,
    /// Device pixels covered by image boxes.
    pub image_pixels: u64,
}

impl crate::DisplayList {
    pub fn cost(&self) -> FrameCost {
        let mut cost = FrameCost {
            nodes: self.nodes.len(),
            paints: self.commands.len(),
            ..Default::default()
        };
        let scale = f64::from(self.scale);
        for paint in &self.commands {
            match paint {
                Paint::Text {
                    glyphs,
                    font,
                    gradient,
                    ..
                } => {
                    cost.glyphs += glyphs.len();
                    // `bg-clip-text` draws through a mask layer and a blend layer.
                    if gradient.is_some() {
                        cost.layers += 2;
                    }
                    if crate::text::outlines::external(font) {
                        cost.outline_glyphs += glyphs.len();
                    }
                }
                Paint::Vector { path, .. } => cost.vector_elements += path.elements().len(),
                Paint::Area { points, .. } | Paint::Stroke { points, .. } => {
                    cost.vector_elements += points.len()
                }
                Paint::Opacity { .. } | Paint::Clip(_) => cost.layers += 1,
                Paint::Backdrop { .. } => {
                    cost.layers += 1;
                    cost.backdrops += 1;
                }
                Paint::Shadow { .. } | Paint::RasterShadow { .. } => cost.shadows += 1,
                Paint::Image { bounds, .. } => {
                    cost.images += 1;
                    let area = f64::from(bounds[2].max(0.0)) * f64::from(bounds[3].max(0.0));
                    cost.image_pixels += (area * scale * scale).round() as u64;
                }
                _ => {}
            }
        }
        cost
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn counts_are_stable_across_renders_and_follow_the_tree() {
        let source = crate::session::tests::source(
            "function render(){return h('column',{style:{opacity:0.5,padding:4}},h('text',{},'abc'),h('vector',{paths:[{d:'M0 0H10V10Z'}],style:{width:10,height:10}}),h('image',{src:'a.png',alt:'',style:{width:10,height:20}}))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let first = page.layout(100, 100, 2.0).unwrap().cost();
        assert_eq!(first, page.layout(100, 100, 2.0).unwrap().cost());
        assert_eq!((first.nodes, first.glyphs, first.images), (4, 3, 1));
        assert_eq!(first.image_pixels, 10 * 20 * 4);
        assert!(first.vector_elements >= 4 && first.layers >= 1 && first.paints > 4);
    }
}
