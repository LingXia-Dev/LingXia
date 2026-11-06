//! The immutable frame an adapter has presented, and geometric queries on it.
use crate::scene::node::{ClipSnapshot, NodeSnapshot};
use crate::session::lifecycle::CommitTicket;

/// What the user sees: published once per successful presentation and never mutated.
pub struct PresentedFrame {
    pub commit: CommitTicket,
    /// Device pixels per logical pixel.
    pub scale: f32,
    pub nodes: Vec<NodeSnapshot>,
    /// Frames with an equal key share one laid-out View: the same nodes at the same
    /// indices, differing only in presented (animated, scrolled, decorated) geometry.
    pub layout: Option<(u64, u64)>,
    /// A hash of the serialized nodes: frames with equal content compare as one integer.
    pub content: Option<u64>,
}

impl PresentedFrame {
    pub fn new(commit: CommitTicket, scale: f32, nodes: Vec<NodeSnapshot>) -> Self {
        Self {
            commit,
            scale,
            nodes,
            layout: None,
            content: None,
        }
    }

    pub fn with_content(mut self, content: u64) -> Self {
        self.content = Some(content);
        self
    }

    /// The same nodes with the same fields; a frame without a content hash compares in full.
    pub(crate) fn same_content(&self, other: &Self) -> bool {
        match (self.content, other.content) {
            (Some(a), Some(b)) => a == b,
            _ => {
                serde_json::to_string(&self.nodes).ok() == serde_json::to_string(&other.nodes).ok()
            }
        }
    }

    pub fn with_layout(mut self, layout: (u64, u64)) -> Self {
        self.layout = Some(layout);
        self
    }

    pub(crate) fn same_layout(&self, other: &Self) -> bool {
        self.layout.is_some()
            && self.layout == other.layout
            && self.nodes.len() == other.nodes.len()
    }

    pub fn node(&self, index: usize) -> Option<&NodeSnapshot> {
        self.nodes.get(index)
    }

    pub fn index_of(&self, identity: &str) -> Option<usize> {
        self.nodes.iter().position(|node| node.identity == identity)
    }

    /// Nodes from topmost to bottommost paint order.
    pub fn hit_order(&self) -> impl Iterator<Item = (usize, &NodeSnapshot)> {
        let mut order: Vec<_> = self.nodes.iter().enumerate().collect();
        order.sort_by_key(|(index, node)| {
            (std::cmp::Reverse(node.order), std::cmp::Reverse(*index))
        });
        order.into_iter()
    }

    /// The topmost interactive node under a logical point.
    pub fn hit(&self, point: [f32; 2]) -> Option<usize> {
        self.hit_order()
            .find(|(_, node)| interactive(node) && contains(node, point, false))
            .map(|(index, _)| index)
    }

    /// The topmost node with a `hover:` delta under a logical point.
    pub fn hover_hit(&self, point: [f32; 2]) -> Option<usize> {
        self.hit_order()
            .find(|(_, node)| node.hover_style && contains(node, point, false))
            .map(|(index, _)| index)
    }

    /// The topmost selectable static text under a logical point.
    pub fn selectable_hit(&self, point: [f32; 2]) -> Option<usize> {
        self.hit_order()
            .find(|(_, node)| node.selectable.is_some() && contains(node, point, false))
            .map(|(index, _)| index)
    }

    pub fn has_text_selection(&self) -> bool {
        self.nodes.iter().any(|node| node.text_selection.is_some())
    }

    /// The topmost scroll region that can move under a logical point.
    pub fn scroll_hit(&self, point: [f32; 2]) -> Option<String> {
        self.hit_order()
            .find(|(_, node)| {
                node.scroll.as_ref().is_some_and(|s| s.max_offset > 0.0)
                    && contains(node, point, true)
            })
            .and_then(|(_, node)| node.scroll.as_ref().map(|s| s.id.clone()))
    }

    /// Selected plots a tap at `point` dismisses because it lands outside their viewport.
    pub fn outside_plot_selections(&self, point: [f32; 2]) -> Vec<String> {
        if point.iter().any(|value| !value.is_finite()) {
            return Vec::new();
        }
        self.nodes
            .iter()
            .filter(|node| plot_can_clear(node) && visible(node))
            .filter(|node| !contains(node, point, false) || !contains_content(node, point))
            .map(|node| node.identity.clone())
            .collect()
    }

    /// The control with `previous`'s identity, if it can still be activated and is visible.
    pub fn press_target(&self, previous: &NodeSnapshot) -> Option<usize> {
        if editable(previous) || previous.identity.is_empty() {
            return None;
        }
        self.index_of(&previous.identity).filter(|index| {
            let node = &self.nodes[*index];
            activatable(node) && !editable(node) && visible(node)
        })
    }

    /// The same control in this frame, when only its callback handles changed.
    pub fn rebind(&self, previous: &NodeSnapshot) -> Option<usize> {
        if editable(previous) || !activatable(previous) || previous.identity.is_empty() {
            return None;
        }
        let key = stable_content(previous);
        self.nodes.iter().position(|node| {
            activatable(node) && node.identity == previous.identity && stable_content(node) == key
        })
    }
}

pub fn visible(node: &NodeSnapshot) -> bool {
    node.visible_bounds[2] > 0.0 && node.visible_bounds[3] > 0.0
}

pub fn editable(node: &NodeSnapshot) -> bool {
    node.input.as_ref().is_some_and(|input| input.enabled)
}

pub fn slider_enabled(node: &NodeSnapshot) -> bool {
    node.range
        .as_ref()
        .is_some_and(|range| range.enabled && range.on_change.is_some_and(|handle| handle > 0))
}

pub fn plot_enabled(node: &NodeSnapshot) -> bool {
    node.plot
        .as_ref()
        .is_some_and(|plot| plot.on_select.is_some_and(|handle| handle > 0))
}

pub fn plot_can_clear(node: &NodeSnapshot) -> bool {
    plot_enabled(node)
        && node
            .plot
            .as_ref()
            .is_some_and(|plot| plot.selected_index >= 0)
}

fn activatable(node: &NodeSnapshot) -> bool {
    node.on_click.is_some_and(|handle| handle != 0)
        || plot_enabled(node)
        || node
            .select
            .as_ref()
            .is_some_and(|select| select.on_select.is_some())
}

/// A `drag` element with somewhere to go: keys and assistive input step it between points.
pub fn draggable(node: &NodeSnapshot) -> bool {
    node.drag.as_ref().is_some_and(|drag| drag.points.len() > 1)
}

/// Targets of pointer activation and keyboard focus.
pub fn interactive(node: &NodeSnapshot) -> bool {
    activatable(node) || editable(node) || slider_enabled(node)
}

/// Inside the visible box, optionally the node's own rounded shape, and every ancestor clip.
pub fn contains(node: &NodeSnapshot, point: [f32; 2], own_shape: bool) -> bool {
    if point.iter().any(|value| !value.is_finite()) || !inside_rect(node.visible_bounds, point) {
        return false;
    }
    let shape = ClipSnapshot {
        bounds: node.bounds,
        radius: node.radius,
        radii: node.radii,
        transform: None,
    };
    if own_shape && !shape.contains(point) {
        return false;
    }
    if !node.hit_rects.is_empty() && !node.hit_rects.iter().any(|rect| inside_rect(*rect, point)) {
        return false;
    }
    node.clips.iter().all(|clip| clip.contains(point))
}

pub fn contains_content(node: &NodeSnapshot, point: [f32; 2]) -> bool {
    node.content_clip.is_none_or(|clip| clip.contains(point))
}

fn inside_rect([x, y, w, h]: [f32; 4], [px, py]: [f32; 2]) -> bool {
    w > 0.0 && h > 0.0 && px >= x && py >= y && px < x + w && py < y + h
}

/// Geometry, clipping, label and control state, without per-commit callback handles.
fn stable_content(node: &NodeSnapshot) -> String {
    let mut node = node.clone();
    node.on_click = None;
    if let Some(plot) = &mut node.plot {
        plot.on_select = None;
    }
    serde_json::to_string(&node).unwrap_or_default()
}
