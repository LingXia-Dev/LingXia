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

    /// The topmost interactive node under a logical point; an open top-layer panel over the
    /// point keeps it from everything beneath, its own backdrop included.
    pub fn hit(&self, point: [f32; 2]) -> Option<usize> {
        let mut panel: Option<&NodeSnapshot> = None;
        for (index, node) in self.hit_order() {
            if panel.is_some_and(|panel| !descends(node, panel)) {
                continue;
            }
            if hit_target(node)
                && contains(node, point, false)
                && super::slider::takes(node, point[0])
            {
                return Some(index);
            }
            if node.panel && panel.is_none() && contains(node, point, false) {
                panel = Some(node);
            }
        }
        None
    }

    /// What a finger at `point` presses: the exact hit, else a small target whose box grown to
    /// `minimum` (logical px; 0 for a fine pointer) reaches the point. Only targets inside the
    /// exact hit compete, the nearest edge winning and a tie going to the one painted higher.
    pub fn press_hit(&self, point: [f32; 2], minimum: f32) -> Option<usize> {
        let exact = self.hit(point);
        if minimum <= 0.0 || point.iter().any(|value| !value.is_finite()) {
            return exact;
        }
        let scope = exact.map(|index| &self.nodes[index]);
        let order: Vec<_> = self.hit_order().collect();
        let boxes = boxes_at(&self.nodes, &order, point);
        let mut best: Option<(f32, usize)> = None;
        for (rank, &(index, node)) in order.iter().enumerate() {
            if Some(index) == exact
                || node.touch_target_none
                || !hit_target(node)
                || scope.is_some_and(|scope| !descends(node, scope))
            {
                continue;
            }
            let Some(distance) = reach(node, point, minimum) else {
                continue;
            };
            if !covered(&boxes, rank, node) && best.is_none_or(|(nearest, _)| distance < nearest) {
                best = Some((distance, index));
            }
        }
        best.map(|(_, index)| index).or(exact)
    }

    /// The topmost node with a `hover:` delta under a logical point.
    pub fn hover_hit(&self, point: [f32; 2]) -> Option<usize> {
        self.hit_order()
            .find(|(_, node)| node.hover_style && contains(node, point, false))
            .map(|(index, _)| index)
    }

    /// The topmost `interestFor` trigger under a logical point, or the popover with id `hint`.
    pub fn interest_hit(&self, point: [f32; 2], hint: Option<&str>) -> Option<usize> {
        self.hit_order()
            .find(|(_, node)| {
                (node.interest_for.is_some()
                    || (hint.is_some() && node.element_id.as_deref() == hint))
                    && contains(node, point, false)
            })
            .map(|(index, _)| index)
    }

    /// The topmost selectable static text under a logical point.
    pub fn selectable_hit(&self, point: [f32; 2]) -> Option<usize> {
        self.hit_order()
            .find(|(_, node)| node.selectable.is_some() && contains(node, point, false))
            .map(|(index, _)| index)
    }

    /// The innermost element with `onContextMenu` under a logical point.
    pub fn context_menu_hit(&self, point: [f32; 2]) -> Option<usize> {
        self.hit_order()
            .find(|(_, node)| node.on_context_menu.is_some() && contains(node, point, false))
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
            .map(|node| node.identity.to_string())
            .collect()
    }

    /// The control with `previous`'s identity, if it can still be activated and is visible.
    pub fn press_target(&self, previous: &NodeSnapshot) -> Option<usize> {
        if editable(previous) || previous.identity.is_empty() {
            return None;
        }
        self.index_of(&previous.identity).filter(|index| {
            let node = &self.nodes[*index];
            clickable(node) && !editable(node) && visible(node)
        })
    }

    /// The same control in this frame, when only its callback handles changed.
    pub fn rebind(&self, previous: &NodeSnapshot) -> Option<usize> {
        if editable(previous) || !clickable(previous) || previous.identity.is_empty() {
            return None;
        }
        let key = stable_content(previous);
        self.nodes.iter().position(|node| {
            clickable(node) && node.identity == previous.identity && stable_content(node) == key
        })
    }
}

/// An open dialog: every host lifts the page above its bars while one shows.
pub fn has_open_dialog(frame: &PresentedFrame) -> bool {
    frame
        .nodes
        .iter()
        .any(|node| matches!(node.role.as_deref(), Some("dialog" | "alertdialog")) && visible(node))
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

/// What a node can do, decided once here so every input path agrees.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Acts(u8);

impl Acts {
    /// A control kind HTML focuses whatever its handlers: enabled unless authored disabled.
    pub const CONTROL: Self = Self(1);
    /// A click, a choice or a plot selection runs a callback.
    pub const ACTIVATE: Self = Self(1 << 1);
    pub const EDIT: Self = Self(1 << 2);
    /// A slider takes new values.
    pub const ADJUST: Self = Self(1 << 3);
    pub const DRAG: Self = Self(1 << 4);
    /// An `interestFor` trigger shows its hint.
    pub const INTEREST: Self = Self(1 << 5);
    /// A pointer crossing it is reported; it takes no focus and no hit.
    pub const CROSSING: Self = Self(1 << 6);
    /// An `onPointer` element with a value steps it from keys and assistive input.
    pub const STEP: Self = Self(1 << 7);

    const FOCUS: Self = Self(
        Self::CONTROL.0
            | Self::ACTIVATE.0
            | Self::EDIT.0
            | Self::ADJUST.0
            | Self::DRAG.0
            | Self::INTEREST.0
            | Self::STEP.0,
    );
    const HIT: Self = Self(Self::ACTIVATE.0 | Self::EDIT.0 | Self::ADJUST.0 | Self::DRAG.0);

    pub fn any(self, of: Self) -> bool {
        self.0 & of.0 != 0
    }
}

impl std::ops::BitOr for Acts {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// The element kinds that stay focusable and enabled without a handler, as in HTML.
const CONTROLS: &[&str] = &[
    "button",
    "checkbox",
    "switch",
    "radio-option",
    "select",
    "input",
    "search",
    "textarea",
    "slider",
];

pub fn acts(node: &NodeSnapshot) -> Acts {
    let mut acts = Acts::default();
    let mut add = |on: bool, act: Acts| {
        if on {
            acts = acts | act;
        }
    };
    add(
        !node.disabled && CONTROLS.contains(&node.tag),
        Acts::CONTROL,
    );
    add(
        node.on_click.is_some_and(|handle| handle != 0)
            || plot_enabled(node)
            || node
                .select
                .as_ref()
                .is_some_and(|select| select.on_select.is_some()),
        Acts::ACTIVATE,
    );
    add(editable(node), Acts::EDIT);
    add(slider_enabled(node), Acts::ADJUST);
    add(draggable(node), Acts::DRAG);
    add(
        !node.disabled && node.interest_for.is_some(),
        Acts::INTEREST,
    );
    add(node.on_pointer.iter().any(Option::is_some), Acts::CROSSING);
    add(steppable(node), Acts::STEP);
    acts
}

/// A click, keyboard Activate or accessibility Click runs something here.
pub fn clickable(node: &NodeSnapshot) -> bool {
    acts(node).any(Acts::ACTIVATE)
}

/// A drag element with somewhere to go: keys and assistive input step it between points.
pub fn draggable(node: &NodeSnapshot) -> bool {
    node.drag
        .as_ref()
        .is_some_and(|drag| !drag.ghost && drag.points.len() > 1)
}

/// An `onPointer` element that reports a value (`aria-valuenow`): arrows on its axis and assistive
/// increment and decrement reach its handler, as a pointer's position does.
pub fn steppable(node: &NodeSnapshot) -> bool {
    !node.disabled
        && node.on_pointer_event.is_some()
        && node
            .progress
            .as_ref()
            .is_some_and(|value| value.now.is_some())
}

/// Targets of pointer presses and hover.
pub fn hit_target(node: &NodeSnapshot) -> bool {
    acts(node).any(Acts::HIT)
}

/// Keyboard focus may rest here.
pub fn focusable(node: &NodeSnapshot) -> bool {
    acts(node).any(Acts::FOCUS)
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

/// The boxes that keep a touch band off `point`, by rank in `order`: controls and laid-out
/// boxes; text, images and the full-surface box holding a top layer's panel let it through.
fn boxes_at<'a>(
    nodes: &[NodeSnapshot],
    order: &[(usize, &'a NodeSnapshot)],
    point: [f32; 2],
) -> Vec<(usize, &'a NodeSnapshot)> {
    let layers: Vec<&str> = nodes
        .iter()
        .filter(|node| node.panel)
        .filter_map(|node| node.identity.rsplit_once('/').map(|(layer, _)| layer))
        .collect();
    order
        .iter()
        .enumerate()
        .filter(|(_, (_, node))| {
            (hit_target(node)
                || !(matches!(node.tag, "text" | "span" | "image" | "vector")
                    || layers.contains(&&*node.identity)))
                && contains(node, point, false)
        })
        .map(|(rank, &(_, node))| (rank, node))
        .collect()
}

/// A box painted above `node` (at a lower rank) that is neither its ancestor nor its content.
fn covered(boxes: &[(usize, &NodeSnapshot)], rank: usize, node: &NodeSnapshot) -> bool {
    boxes
        .iter()
        .take_while(|(above, _)| *above < rank)
        .any(|(_, above)| !descends(node, above) && !descends(above, node))
}

/// How far `point` lies outside `node`'s box when that box, grown on each axis to `minimum`
/// about its centre, reaches it through every clip; a link's lines grow only vertically.
fn reach(node: &NodeSnapshot, point: [f32; 2], minimum: f32) -> Option<f32> {
    if !visible(node) || !node.clips.iter().all(|clip| clip.contains(point)) {
        return None;
    }
    let grow = |start: f32, size: f32| {
        let size_to = size.max(minimum);
        (start - (size_to - size) / 2.0, size_to)
    };
    let rects: &[[f32; 4]] = if node.hit_rects.is_empty() {
        std::slice::from_ref(&node.bounds)
    } else {
        &node.hit_rects
    };
    rects
        .iter()
        .filter(|rect| rect[2] > 0.0 && rect[3] > 0.0)
        .filter_map(|&[x, y, w, h]| {
            let (gx, gw) = if node.hit_rects.is_empty() {
                grow(x, w)
            } else {
                (x, w)
            };
            let (gy, gh) = grow(y, h);
            inside_rect([gx, gy, gw, gh], point).then(|| {
                let dx = (x - point[0]).max(point[0] - (x + w)).max(0.0);
                let dy = (y - point[1]).max(point[1] - (y + h)).max(0.0);
                dx.hypot(dy)
            })
        })
        .min_by(f32::total_cmp)
}

/// `node` sits inside `ancestor`'s subtree.
pub fn descends(node: &NodeSnapshot, ancestor: &NodeSnapshot) -> bool {
    node.identity.len() > ancestor.identity.len()
        && node.identity.starts_with(&*ancestor.identity)
        && node.identity.as_bytes()[ancestor.identity.len()] == b'/'
}

/// A shown popover's or dialog's panel whose layer takes a close request: assistive tech may
/// dismiss it, whatever role it has.
pub fn dismissable(nodes: &[NodeSnapshot], node: &NodeSnapshot) -> bool {
    node.panel
        && nodes
            .iter()
            .any(|layer| layer.close_request && descends(node, layer))
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
