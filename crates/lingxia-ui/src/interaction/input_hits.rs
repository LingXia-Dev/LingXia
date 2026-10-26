//! Editor hit testing against presented, shaped geometry.
use crate::scene::display::*;
use crate::scene::node::*;
use crate::scene::retained::*;
use crate::text::editing::*;
use std::sync::Arc;

/// Interactive geometry from a successfully built frame; shares its shaped layout.
pub(crate) struct InputHitMap {
    pub(crate) retained: Arc<RetainedLayout>,
    pub(crate) nodes: Vec<(usize, NodeSnapshot)>,
}

impl InputHitMap {
    pub(crate) fn capture(display: &DisplayList) -> Self {
        Self {
            retained: Arc::clone(&display.retained),
            nodes: display
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, node)| {
                    node.plot.is_some() || node.input.as_ref().is_some_and(|input| input.enabled)
                })
                .map(|(index, node)| (index, node.clone()))
                .collect(),
        }
    }

    pub(crate) fn plot_node(&self, identity: &str) -> Option<&NodeSnapshot> {
        self.nodes
            .iter()
            .map(|(_, node)| node)
            .find(|node| node.identity == identity && node.plot.is_some())
    }

    pub(crate) fn hit(&self, identity: &str, point: [f32; 2], word: bool) -> Option<[u32; 2]> {
        self.hit_with_edge(identity, point, word, [0, 0])
    }

    pub(crate) fn drag_hit(
        &self,
        identity: &str,
        point: [f32; 2],
        edge: [i8; 2],
    ) -> Option<[u32; 2]> {
        self.hit_with_edge(identity, point, false, edge)
    }

    fn hit_with_edge(
        &self,
        identity: &str,
        point: [f32; 2],
        word: bool,
        edge: [i8; 2],
    ) -> Option<[u32; 2]> {
        use parley::editing::Selection;
        if !point.iter().all(|value| value.is_finite())
            || edge.iter().any(|value| !(-1..=1).contains(value))
        {
            return None;
        }
        let (index, node) = self
            .nodes
            .iter()
            .find(|(_, node)| node.identity == identity)?;
        let contains = |clip: ClipSnapshot| clip.contains(point);
        if !contains(ClipSnapshot {
            radii: None,
            bounds: node.visible_bounds,
            radius: 0.0,
            transform: None,
        }) || !contains(ClipSnapshot {
            radii: node.radii,
            bounds: node.bounds,
            radius: node.radius,
            transform: None,
        }) || !node.clips.iter().copied().all(contains)
        {
            return None;
        }
        let input = node.input.as_ref()?;
        if input.value.is_empty() {
            return Some([0, 0]);
        }
        let shaped = self.retained.inputs[*index].as_ref()?;
        let x = point[0] - node.bounds[0] - shaped.padding[0] + input.scroll_offset[0];
        let y = point[1] - node.bounds[1] - shaped.padding[1] - shaped.text_offset_y
            + input.scroll_offset[1];
        // Masked glyphs must not reveal the underlying password's word boundaries.
        if word && input.secure {
            return Some([0, input.value.encode_utf16().count() as u32]);
        }
        let mut selection = if word {
            Selection::word_from_point(&shaped.layout, x, y)
        } else {
            Selection::from_point(&shaped.layout, x, y)
        };
        // A point at the viewport edge can keep snapping to the same large glyph.
        // Advance through shaped cursors only while there is more text to reveal.
        let can_step = |axis: usize, extent: f32| match edge[axis] {
            -1 => input.scroll_offset[axis] > 0.01,
            1 => input.scroll_offset[axis] + shaped.content_size[axis] < extent - 0.01,
            _ => false,
        };
        if can_step(1, shaped.layout.height()) {
            selection = if edge[1] < 0 {
                selection.previous_line(&shaped.layout, false)
            } else {
                selection.next_line(&shaped.layout, false)
            };
        } else if can_step(0, shaped.layout.width()) {
            selection = if edge[0] < 0 {
                selection.previous_visual(&shaped.layout, false)
            } else {
                selection.next_visual(&shaped.layout, false)
            };
        }
        let logical = |byte| {
            display_offset(
                &input.value,
                byte,
                input.secure,
                matches!(node.tag, "input" | "search"),
            )
        };
        Some([
            logical(selection.anchor().index())?,
            logical(selection.focus().index())?,
        ])
    }
}
