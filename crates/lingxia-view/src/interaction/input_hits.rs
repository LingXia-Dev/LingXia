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

    /// Every editor and plot sits and shapes as in `other`: a repaint (a press, a hover) left
    /// their geometry alone, so hit tests made against `other`'s frame still hold.
    pub(crate) fn same_geometry(&self, other: &Self) -> bool {
        let shaped = |map: &Self, index: usize| {
            map.retained
                .inputs
                .get(index)
                .and_then(Option::as_ref)
                .map(|shaped| {
                    (
                        shaped.padding,
                        shaped.text_offset_y,
                        shaped.content_size,
                        shaped.layout.width(),
                        shaped.layout.height(),
                    )
                })
        };
        self.nodes.len() == other.nodes.len()
            && self.nodes.iter().zip(&other.nodes).all(|((i, a), (j, b))| {
                i == j
                    && a.identity == b.identity
                    && a.bounds == b.bounds
                    && a.visible_bounds == b.visible_bounds
                    && a.radii == b.radii
                    && a.radius == b.radius
                    && a.clips == b.clips
                    && a.input
                        .as_ref()
                        .map(|input| (&input.value, input.scroll_offset))
                        == b.input
                            .as_ref()
                            .map(|input| (&input.value, input.scroll_offset))
                    && shaped(self, *i) == shaped(other, *j)
            })
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
            let hit = Selection::word_from_point(&shaped.layout, x, y).text_range();
            let at = parley::editing::Cursor::from_point(&shaped.layout, x, y).index();
            // The layout's byte offsets index the painted text, where a single-line editor
            // shows each line break as one space.
            let shown = if matches!(node.tag, "input" | "search") {
                std::borrow::Cow::Owned(crate::text::style::single_line(&input.value))
            } else {
                std::borrow::Cow::Borrowed(input.value.as_str())
            };
            let range = crate::text::words::snap(&shown, hit, at);
            Selection::new(
                parley::editing::Cursor::from_byte_index(
                    &shaped.layout,
                    range.start,
                    parley::Affinity::Downstream,
                ),
                parley::editing::Cursor::from_byte_index(
                    &shaped.layout,
                    range.end,
                    parley::Affinity::Upstream,
                ),
            )
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
