//! Retained layout and its scroll/animation composition.
use crate::protocol::Record;
use crate::protocol::{field, tag};
use crate::scene::display::*;
use crate::scene::node::*;
use crate::scene::paint::*;
use parley::Layout;
use std::{collections::HashMap, ops::Range, sync::Arc};
use vello_cpu::Glyph;
use vello_cpu::kurbo::Affine;

#[derive(Clone)]
/// A selectable static text's shaped paragraph, kept for hit-to-offset queries.
pub(crate) struct SelectableText {
    pub(crate) layout: Layout<crate::text::RunBrush>,
    /// The paragraph origin and content box in unscrolled layout space.
    pub(crate) origin: [f32; 2],
    pub(crate) content: [f32; 4],
    pub(crate) text: String,
}

/// Selectable texts realized in one frame; more fall back to unselectable.
pub(crate) const MAX_SELECTABLE: usize = 64;

#[derive(Clone)]
pub(crate) struct InputLayout {
    pub(crate) layout: Layout<crate::text::RunBrush>,
    pub(crate) padding: [f32; 2],
    pub(crate) text_offset_y: f32,
    pub(crate) content_size: [f32; 2],
    pub(crate) color: [u8; 4],
}

/// Where a node's transitionable paint sits in `commands`, as the scene resolved it.
#[derive(Clone, Debug, Default)]
pub(crate) struct PaintStyle {
    /// The solid background rect.
    pub(crate) background: Option<usize>,
    pub(crate) gradient: bool,
    pub(crate) border: Option<usize>,
    /// Analytic shadow layers in authored order; `None` once a layer rasterized.
    pub(crate) shadows: Option<Vec<usize>>,
    pub(crate) foreground: [u8; 4],
    pub(crate) own_color: bool,
    /// Text commands and the record whose inherited colour they paint.
    pub(crate) texts: Vec<(usize, usize)>,
    pub(crate) transform: Option<crate::scene::transform::StaticTransform>,
    pub(crate) corners: Option<[[f32; 2]; 4]>,
    pub(crate) radius: f32,
    /// A `bg-clip-text` gradient and the box it was laid out in, for an animated position.
    pub(crate) text_gradient: Option<(Arc<crate::scene::gradient::BackgroundGradient>, [f32; 4])>,
}

/// Unscrolled geometry and shaped glyphs from one committed View render.
#[derive(Clone)]
pub(crate) struct RetainedLayout {
    pub(crate) nodes: Vec<NodeSnapshot>,
    pub(crate) paint_styles: Vec<PaintStyle>,
    pub(crate) commands: Vec<Paint>,
    pub(crate) ranges: Vec<Range<usize>>,
    pub(crate) parents: Vec<Option<usize>>,
    /// Paint order: depth-first, siblings stably ordered by z-index.
    pub(crate) order: Vec<usize>,
    pub(crate) opacities: Vec<f32>,
    /// Static rotation and scale; translation is already in the geometry.
    pub(crate) transforms: Vec<Option<crate::scene::transform::StaticTransform>>,
    pub(crate) stickies: Vec<Option<crate::scene::sticky::Sticky>>,
    /// The node a popover sits against, by identity; composition places it below or above.
    pub(crate) anchors: Vec<Option<(String, crate::scene::anchor::Area)>>,
    pub(crate) backdrops: Vec<Option<(f32, f32)>>,
    pub(crate) masks: Vec<Option<Arc<Vec<crate::scene::mask::MaskLayer>>>>,
    pub(crate) blurs: Vec<f32>,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) scale: f32,
    pub(crate) inputs: Vec<Option<InputLayout>>,
    pub(crate) selectables: Vec<Option<Arc<SelectableText>>>,
    /// Behind selected text, resolved from the palette.
    pub(crate) selection_fill: [u8; 4],
}

impl RetainedLayout {
    pub(crate) fn rebind(
        self: &Arc<Self>,
        before: &[Record],
        after: &[Record],
    ) -> Option<Arc<Self>> {
        if before.len() != after.len() || after.len() != self.nodes.len() {
            return None;
        }
        for (old, new) in before.iter().zip(after) {
            if old.tag != new.tag
                || old.parent != new.parent
                || old.key != new.key
                || old.test_id != new.test_id
                || old.text != new.text
            {
                return None;
            }
            // Anchors affect scroll reconciliation; callback identities do not affect paint.
            let mut a = old
                .fields
                .iter()
                .filter(|(id, _)| *id != field::LIST_ANCHOR);
            let mut b = new
                .fields
                .iter()
                .filter(|(id, _)| *id != field::LIST_ANCHOR);
            loop {
                match (a.next(), b.next()) {
                    (None, None) => break,
                    (Some(a), Some(b))
                        if a.0 == b.0
                            && (matches!(
                                a.0,
                                field::ON_CLICK
                                    | field::ON_REACH_START
                                    | field::ON_REACH_END
                                    | field::ON_INPUT
                                    | field::ON_SUBMIT
                                    | field::ON_CHECKED_CHANGE
                                    | field::ON_SLIDER_CHANGE
                                    | field::ON_PLOT_SELECT
                                    | field::ON_IMAGE_LOAD
                                    | field::ON_IMAGE_ERROR
                            ) || a.1 == b.1) => {}
                    _ => return None,
                }
            }
        }
        let mut next = self.as_ref().clone();
        for (node, record) in next.nodes.iter_mut().zip(after) {
            let handle = |id| {
                record
                    .fields
                    .iter()
                    .find(|(field, _)| *field == id)
                    .and_then(|(_, value)| value.as_u64())
            };
            if node.on_click.is_some() {
                node.on_click = handle(
                    if matches!(record.tag, tag::CHECKBOX | tag::SWITCH | tag::RADIO_OPTION) {
                        40
                    } else {
                        8
                    },
                );
            }
            if let Some(input) = &mut node.input {
                input.on_input = handle(26);
                input.on_submit = handle(27);
            }
            if let Some(scroll) = &mut node.scroll {
                scroll.on_reach_start = handle(17);
                scroll.on_reach_end = handle(18);
            }
            if let Some(range) = &mut node.range
                && range.on_change.is_some()
            {
                range.on_change = handle(46);
            }
            if let Some(plot) = &mut node.plot {
                plot.on_select = handle(56);
            }
        }
        Some(Arc::new(next))
    }
    /// Nodes whose subtree holds an editor, scroll or slider and so cannot take a paint affine.
    /// They still move: composition translates them.
    pub(crate) fn transform_blocked(&self) -> Vec<bool> {
        let mut blocked: Vec<bool> = self
            .nodes
            .iter()
            .map(|node| node.input.is_some() || node.scroll.is_some() || node.range.is_some())
            .collect();
        for index in (0..blocked.len()).rev() {
            if let Some(parent) = self.parents[index].filter(|p| *p < index) {
                blocked[parent] |= blocked[index];
            }
        }
        blocked
    }

    /// This layout with each exit ghost's subtree, copied from the layout it was last committed
    /// in, grafted under its nearest surviving ancestor: it scrolls with that ancestor's
    /// scrollers and is clipped by its clip chain. Ghosts take no input and no semantics.
    pub(crate) fn with_ghosts(
        self: &Arc<Self>,
        ghosts: &[(String, crate::animation::transition::Origin)],
    ) -> Arc<Self> {
        let mut next = (**self).clone();
        for (identity, crate::animation::transition::Origin(old, root)) in ghosts {
            let root = *root;
            if old
                .nodes
                .get(root)
                .is_none_or(|node| &node.identity != identity)
                || next.nodes.iter().any(|node| &node.identity == identity)
            {
                continue;
            }
            // Preorder: a subtree is the run of nodes whose parent chain reaches the root.
            let mut members = vec![root];
            for index in root + 1..old.nodes.len() {
                match old.parents[index] {
                    Some(parent) if members.contains(&parent) => members.push(index),
                    _ => break,
                }
            }
            let anchor =
                std::iter::successors(old.parents[root], |&p| old.parents[p]).find_map(|p| {
                    next.nodes
                        .iter()
                        .position(|node| node.identity == old.nodes[p].identity)
                });
            let base = next.nodes.len();
            let place = |old_index: usize| {
                members
                    .iter()
                    .position(|&m| m == old_index)
                    .map(|k| base + k)
            };
            for &index in &members {
                let mut node = old.nodes[index].clone();
                node.test_id.clear();
                node.data = None;
                node.on_click = None;
                node.accessibility_hidden = Some(true);
                node.selectable = None;
                node.text_selection = None;
                node.input = None;
                node.select = None;
                node.plot = None;
                node.range = None;
                node.radio_group = None;
                node.scroll_parent = None;
                next.nodes.push(node);
                let start = next.commands.len();
                let shift = |command: usize| command + start - old.ranges[index].start;
                next.commands
                    .extend_from_slice(&old.commands[old.ranges[index].clone()]);
                next.ranges.push(start..next.commands.len());
                let mut style = old.paint_styles[index].clone();
                style.background = style.background.map(shift);
                style.border = style.border.map(shift);
                style.shadows = style
                    .shadows
                    .map(|layers| layers.into_iter().map(shift).collect());
                style.texts = style
                    .texts
                    .into_iter()
                    .filter_map(|(command, record)| Some((shift(command), place(record)?)))
                    .collect();
                next.paint_styles.push(style);
                next.parents.push(if index == root {
                    anchor
                } else {
                    old.parents[index].and_then(place)
                });
                next.opacities.push(old.opacities[index]);
                next.transforms.push(old.transforms[index]);
                next.stickies.push(None);
                next.anchors.push(None);
                next.backdrops.push(old.backdrops[index]);
                next.masks.push(old.masks[index].clone());
                next.blurs.push(old.blurs[index]);
                next.inputs.push(None);
                next.selectables.push(None);
            }
            // Painted right after the anchor's subtree, so above its other children.
            let added: Vec<usize> = (base..next.nodes.len()).collect();
            let at = match anchor {
                Some(anchor) => {
                    let mut end = next
                        .order
                        .iter()
                        .position(|&i| i == anchor)
                        .map_or(next.order.len(), |p| p + 1);
                    while end < next.order.len() && next.descends(next.order[end], anchor) {
                        end += 1;
                    }
                    end
                }
                None => next.order.len(),
            };
            next.order.splice(at..at, added);
        }
        Arc::new(next)
    }

    fn descends(&self, mut index: usize, ancestor: usize) -> bool {
        while let Some(parent) = self.parents[index] {
            if parent == ancestor {
                return true;
            }
            index = parent;
        }
        false
    }

    /// The node's transitionable paint values after state variants.
    pub(crate) fn resolved(
        &self,
        index: usize,
        blocked: bool,
    ) -> crate::animation::transition::Resolved {
        let style = &self.paint_styles[index];
        let [x, y, w, h] = self.nodes[index].bounds;
        let moved = style
            .transform
            .map(|t| t.translation([w, h]))
            .unwrap_or_default();
        let painted = |command: usize| match &self.commands[command] {
            Paint::Rect { color, .. } | Paint::Border { color, .. } => *color,
            _ => [0; 4],
        };
        crate::animation::transition::Resolved {
            opacity: f64::from(self.opacities[index]),
            // A live subtree carries only its translation, which composition applies.
            transform: Some(
                style
                    .transform
                    .map(|t| t.decomposed([w, h]))
                    .unwrap_or_default(),
            )
            .map(|t| if blocked { translation_of(t) } else { t }),
            translate_only: blocked,
            background: (!style.gradient).then(|| style.background.map(painted).unwrap_or([0; 4])),
            foreground: style.foreground,
            border: style.border.map(painted),
            shadows: style.shadows.as_ref().map(|layers| {
                layers
                    .iter()
                    .filter_map(|&command| match &self.commands[command] {
                        Paint::Shadow { shadow, color, .. } => {
                            Some(crate::animation::transition::ShadowLayer {
                                offset: [f64::from(shadow.offset_x), f64::from(shadow.offset_y)],
                                blur: f64::from(shadow.blur),
                                spread: f64::from(shadow.spread),
                                color: *color,
                            })
                        }
                        _ => None,
                    })
                    .collect()
            }),
            position: Some([f64::from(x - moved[0]), f64::from(y - moved[1])]),
        }
    }

    pub(crate) fn compose(
        self: &Arc<Self>,
        offsets: &HashMap<String, f32>,
    ) -> Result<DisplayList, String> {
        self.compose_animated(offsets, &Default::default())
    }

    pub(crate) fn compose_animated(
        self: &Arc<Self>,
        offsets: &HashMap<String, f32>,
        visuals: &std::collections::BTreeMap<String, crate::animation::Visual>,
    ) -> Result<DisplayList, String> {
        if offsets.values().any(|offset| !offset.is_finite()) {
            return Err("UI_SCROLL: non-finite offset".into());
        }
        if visuals
            .keys()
            .any(|id| !self.nodes.iter().any(|node| &node.identity == id))
        {
            return Err("UI_ANIMATION: visual identity missing from layout".into());
        }
        let animated = !visuals.is_empty() || self.transforms.iter().any(Option::is_some);
        let count = self.nodes.len();
        // A subtree holding an editor, scroll or slider moves by composition, so its live parts follow.
        let blocked = (!visuals.is_empty()).then(|| self.transform_blocked());
        let mut transforms: Vec<Affine> = vec![Affine::IDENTITY; count];
        let viewport = [
            0.0,
            0.0,
            self.width as f32 / self.scale,
            self.height as f32 / self.scale,
        ];
        let mut nodes = self.nodes.clone();
        let mut translations: Vec<[f32; 2]> = vec![[0.0; 2]; count];
        let mut clips = vec![[0.0f32; 4]; count];
        let mut commands = Vec::new();
        let mut rounded: Vec<Vec<ClipSnapshot>> = vec![Vec::new(); count];
        let mut snapshot_clips: Vec<Vec<ClipSnapshot>> = if animated {
            vec![Vec::new(); count]
        } else {
            Vec::new()
        };
        let mut active_layers: Vec<Paint> = Vec::new();
        let mut blurred = 0;
        let mut child_layers: Vec<Vec<Paint>> = vec![Vec::new(); count];
        let mut painted = Vec::new();
        let mut input_paints = Vec::new();
        let mut selectable_paints = Vec::new();
        // A transitioning text colour reaches descendants that inherit it.
        let mut foregrounds: Vec<Option<[u8; 4]>> = vec![None; count];
        if !visuals.is_empty() {
            for index in 0..count {
                let own = visuals
                    .get(&self.nodes[index].identity)
                    .and_then(|v| v.transition.foreground);
                foregrounds[index] = own.or_else(|| {
                    if self.paint_styles[index].own_color {
                        None
                    } else {
                        self.parents[index]
                            .filter(|p| *p < index)
                            .and_then(|p| foregrounds[p])
                    }
                });
            }
        }
        // Outermost animated subtrees become self-contained command spans: they open from
        // and close back to the parent's layers, so everything outside them is unchanged
        // between animation frames.
        let mut subtree_end = vec![0usize; count];
        let mut spans = Vec::new();
        let mut open: Option<(usize, usize, usize)> = None;
        if !visuals.is_empty() {
            let mut sizes = vec![1usize; count];
            for index in (0..count).rev() {
                if let Some(parent) = self.parents[index].filter(|p| *p < index) {
                    sizes[parent] += sizes[index];
                }
            }
            for (position, &index) in self.order.iter().enumerate() {
                subtree_end[index] = position + sizes[index];
            }
        }
        let close = |commands: &mut Vec<Paint>,
                     active_layers: &mut Vec<Paint>,
                     child_layers: &[Vec<Paint>],
                     spans: &mut Vec<AnimatedSpan>,
                     (root, start, _): (usize, usize, usize)| {
            let parent_layers = self.parents[root]
                .map(|p| child_layers[p].clone())
                .unwrap_or_default();
            sync_layers(commands, active_layers, parent_layers.clone());
            if commands.len() > start {
                spans.push(AnimatedSpan {
                    identity: self.nodes[root].identity.clone(),
                    commands: start..commands.len(),
                });
            }
            commands.push(Paint::Transform(last_transform(&parent_layers)));
        };
        for (position, &index) in self.order.iter().enumerate() {
            if let Some(span) = open.filter(|(_, _, end)| position >= *end) {
                close(
                    &mut commands,
                    &mut active_layers,
                    &child_layers,
                    &mut spans,
                    span,
                );
                open = None;
            }
            let parent = self.parents[index];
            if open.is_none() && visuals.contains_key(&self.nodes[index].identity) {
                let parent_layers = parent.map(|p| child_layers[p].clone()).unwrap_or_default();
                sync_layers(&mut commands, &mut active_layers, parent_layers);
                open = Some((index, commands.len(), subtree_end[index]));
            }
            let mut translation = parent.map(|p| translations[p]).unwrap_or([0.0; 2]);
            if let Some(sticky) = &self.stickies[index] {
                // The scroll is composed already; its content box is the sticky viewport.
                let scroll = &nodes[sticky.scroll];
                let insets = scroll
                    .scroll
                    .as_ref()
                    .map(|s| s.content_insets)
                    .unwrap_or_default();
                let viewport = snap_to_pixels(
                    [
                        scroll.bounds[0] + insets[3],
                        scroll.bounds[1] + insets[0],
                        (scroll.bounds[2] - insets[1] - insets[3]).max(0.0),
                        (scroll.bounds[3] - insets[0] - insets[2]).max(0.0),
                    ],
                    self.scale,
                );
                let mut natural = self.nodes[index].bounds;
                natural[0] -= translation[0];
                natural[1] -= translation[1];
                let container = sticky.container.map(|mut c| {
                    c[0] -= translation[0];
                    c[1] -= translation[1];
                    c
                });
                let [dx, dy] = sticky.shift(natural, viewport, container);
                translation = [translation[0] - dx, translation[1] - dy];
            }
            if let Some((anchor, area)) = &self.anchors[index]
                && let Some(target) = self.nodes.iter().position(|node| &node.identity == anchor)
            {
                // The target is composed already: it precedes the popover in paint order.
                let mut natural = self.nodes[index].bounds;
                natural[0] -= translation[0];
                natural[1] -= translation[1];
                let [dx, dy] =
                    crate::scene::anchor::place(natural, nodes[target].bounds, viewport, *area);
                translation = [translation[0] - dx, translation[1] - dy];
            }
            let composed = blocked.as_ref().is_some_and(|blocked| blocked[index]);
            if composed
                && let Some(moved) = visuals
                    .get(&self.nodes[index].identity)
                    .and_then(|visual| visual.transition.transform)
            {
                let [_, _, w, h] = self.nodes[index].bounds;
                let laid_out = self.paint_styles[index]
                    .transform
                    .map(|t| t.translation([w, h]))
                    .unwrap_or_default();
                translation[0] -= (moved.translate[0] - f64::from(laid_out[0])) as f32;
                translation[1] -= (moved.translate[1] - f64::from(laid_out[1])) as f32;
            }
            let node = &mut nodes[index];
            node.order = position as u32;
            let mut desired = parent.map(|p| child_layers[p].clone()).unwrap_or_default();
            let world_clip = parent.map(|p| clips[p]).unwrap_or(viewport);
            node.bounds[0] -= translation[0];
            node.bounds[1] -= translation[1];
            if let Some(content) = &mut node.content_clip {
                content.bounds[0] -= translation[0];
                content.bounds[1] -= translation[1];
                // A scroll's edge meets the headers pinned to it on one device-pixel row,
                // so no antialiased half row lets content behind them through.
                if node.scroll.is_some() {
                    content.bounds = snap_to_pixels(content.bounds, self.scale);
                }
            }
            if let Some(control) = &mut node.control_bounds {
                control[0] -= translation[0];
                control[1] -= translation[1];
            }
            for rect in &mut node.hit_rects {
                rect[0] -= translation[0];
                rect[1] -= translation[1];
            }
            let inherited = parent.map(|p| transforms[p]).unwrap_or(Affine::IDENTITY);
            let visual = visuals.get(&node.identity);
            let paint_style = &self.paint_styles[index];
            let overrides = visual.map(|v| &v.transition);
            let inherited = match (
                overrides.and_then(|o| o.transform).filter(|_| !composed),
                self.transforms[index],
            ) {
                (Some(transition), _) => {
                    let [_, _, w, h] = node.bounds;
                    let laid_out = paint_style
                        .transform
                        .map(|t| t.translation([w, h]))
                        .unwrap_or_default();
                    checked_paint_transform(
                        inherited
                            * transition
                                .matrix(node.bounds.map(f64::from), laid_out.map(f64::from)),
                    )?
                }
                (None, Some(transform)) => {
                    checked_paint_transform(inherited * transform.matrix(node.bounds))?
                }
                (None, None) => inherited,
            };
            let matrix = if let Some(visual) = visual {
                let [x, y, w, h] = node.bounds.map(f64::from);
                checked_paint_transform(inherited * visual.matrix_in([x, y, w, h])?)?
            } else {
                inherited
            };
            if matrix != Affine::IDENTITY
                && (node.input.is_some() || node.scroll.is_some() || node.range.is_some())
            {
                return Err(
                    "UI_TRANSFORM: editors, scrolls and sliders cannot rotate or scale".into(),
                );
            }
            let alpha = visual
                .and_then(|v| v.opacity.or(v.transition.opacity))
                .unwrap_or(self.opacities[index] as f64);
            if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
                return Err("UI_ANIMATION: invalid visual opacity".into());
            }
            transforms[index] = matrix;
            if animated {
                desired.push(Paint::Transform(matrix));
            }
            let clip = transformed_bounds(world_clip, matrix.inverse())?;
            node.visible_bounds = intersect(node.bounds, clip);
            let world_clips = parent.map(|p| rounded[p].clone()).unwrap_or_default();
            let mut children_clips = world_clips.clone();
            if animated {
                snapshot_clips[index] = world_clips.clone();
            }
            node.clips = if matrix == Affine::IDENTITY {
                world_clips
            } else {
                world_clips
                    .into_iter()
                    .map(|clip| transformed_clip(clip, matrix.inverse()))
                    .collect::<Result<_, _>>()?
            };
            if node
                .clips
                .iter()
                .any(|shape| !shape.intersects(node.visible_bounds))
            {
                node.visible_bounds[2] = 0.0;
                node.visible_bounds[3] = 0.0;
            }
            let clips_children = node.scroll.is_some() || node.clips_children;
            if clips_children {
                if let Some(content) = node.content_clip {
                    children_clips.push(transformed_clip(content, matrix)?);
                } else if node.radius > 0.0 || node.radii.is_some() {
                    children_clips.push(transformed_clip(
                        ClipSnapshot {
                            radii: node.radii,
                            bounds: node.bounds,
                            radius: node.radius,
                            transform: None,
                        },
                        matrix,
                    )?);
                }
            }
            if let Some((blur, saturation)) = self.backdrops[index] {
                desired.push(Paint::Backdrop {
                    node: index,
                    clip: ClipSnapshot {
                        bounds: node.bounds,
                        radius: node.radius,
                        radii: node.radii,
                        transform: None,
                    },
                    blur,
                    saturation,
                    opacity: alpha as f32,
                });
            } else if alpha < 1.0 {
                desired.push(Paint::Opacity {
                    node: index,
                    alpha: alpha as f32,
                });
            }
            if let Some(layers) = &self.masks[index] {
                desired.push(Paint::Mask {
                    node: index,
                    mask: crate::scene::mask::Mask::new(layers, node.bounds, matrix),
                });
            }
            // Past the budget an element draws sharp rather than failing the frame.
            if self.blurs[index] > 0.0 && blurred < crate::protocol::limits::BLUR_LAYERS {
                blurred += 1;
                desired.push(Paint::Blur {
                    node: index,
                    radius: self.blurs[index],
                });
            }
            let mut descendants = desired.clone();
            if clips_children {
                descendants.push(Paint::Clip(node.content_clip.unwrap_or(ClipSnapshot {
                    bounds: node.bounds,
                    radius: node.radius,
                    radii: node.radii,
                    transform: None,
                })));
            }
            child_layers[index] = descendants;
            rounded[index] = children_clips;
            if let Some(scroll) = &mut node.scroll {
                scroll.offset = offsets
                    .get(&scroll.id)
                    .copied()
                    .unwrap_or(0.0)
                    .clamp(0.0, scroll.max_offset);
            }
            let mut child_translation = translation;
            if let Some(scroll) = &node.scroll {
                // RTL content overflows to the left, so its offsets move it right.
                child_translation[scroll.axis.index()] += if scroll.rtl {
                    -scroll.offset
                } else {
                    scroll.offset
                };
            }
            translations[index] = child_translation;
            clips[index] = transformed_bounds(
                if clips_children {
                    node.content_clip
                        .map(|content| intersect(node.visible_bounds, content.bounds))
                        .unwrap_or(node.visible_bounds)
                } else {
                    clip
                },
                matrix,
            )?;
            if clip[2] == 0.0 || clip[3] == 0.0 {
                continue;
            }
            painted.clear();
            let background = overrides.and_then(|o| o.background);
            let border = overrides.and_then(|o| o.border);
            let shadows = overrides.and_then(|o| o.shadows.as_ref());
            let first = self.ranges[index].start;
            for (offset, paint) in self.commands[self.ranges[index].clone()].iter().enumerate() {
                let command = first + offset;
                let tint = |color: &[u8; 4], replaced: Option<[u8; 4]>, slot: Option<usize>| {
                    replaced.filter(|_| slot == Some(command)).unwrap_or(*color)
                };
                if shadows.is_some()
                    && matches!(paint, Paint::Shadow { .. } | Paint::RasterShadow { .. })
                {
                    continue;
                }
                match paint {
                    Paint::Vector {
                        path,
                        color,
                        gradient,
                        bounds,
                    } => {
                        let mut bounds = *bounds;
                        bounds[0] -= translation[0];
                        bounds[1] -= translation[1];
                        let visible = intersect(bounds, clip);
                        if visible[2] > 0.0
                            && visible[3] > 0.0
                            && node.clips.iter().all(|shape| shape.intersects(visible))
                        {
                            let shift = Affine::translate((
                                -f64::from(translation[0]),
                                -f64::from(translation[1]),
                            ));
                            painted.push(Paint::Vector {
                                path: Arc::new(shift * path.as_ref()),
                                color: *color,
                                gradient: gradient
                                    .as_ref()
                                    .map(|brush| Arc::new(brush.then(shift))),
                                bounds,
                            });
                        }
                    }
                    Paint::RasterShadow { raster } => {
                        let mut raster = raster.clone();
                        raster.bounds[0] -= translation[0];
                        raster.bounds[1] -= translation[1];
                        let visible = intersect(raster.bounds, clip);
                        if visible[2] > 0.0
                            && visible[3] > 0.0
                            && node.clips.iter().all(|shape| shape.intersects(visible))
                        {
                            painted.push(Paint::RasterShadow { raster });
                        }
                    }
                    Paint::Shadow {
                        shadow,
                        color,
                        bounds,
                        radius,
                    } => {
                        let mut bounds = *bounds;
                        bounds[0] -= translation[0];
                        bounds[1] -= translation[1];
                        if let Some(shape) = shadow.geometry(bounds, *radius) {
                            let visible = intersect(shape.extent, clip);
                            if visible[2] > 0.0
                                && visible[3] > 0.0
                                && node.clips.iter().all(|clip| clip.intersects(visible))
                            {
                                painted.push(Paint::Shadow {
                                    shadow: shadow.clone(),
                                    color: *color,
                                    bounds,
                                    radius: *radius,
                                });
                            }
                        }
                    }
                    Paint::Area {
                        points,
                        range,
                        color,
                    } => {
                        let points: Vec<_> = points
                            .iter()
                            .map(|[x, y]| [x - translation[0], y - translation[1]])
                            .collect();
                        let left = points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
                        let right = points
                            .iter()
                            .map(|p| p[0])
                            .fold(f32::NEG_INFINITY, f32::max);
                        let top = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
                        let bottom = points
                            .iter()
                            .map(|p| p[1])
                            .fold(f32::NEG_INFINITY, f32::max);
                        let visible = intersect([left, top, right - left, bottom - top], clip);
                        if visible[2] > 0.0
                            && visible[3] > 0.0
                            && node.clips.iter().all(|shape| shape.intersects(visible))
                        {
                            painted.push(Paint::Area {
                                points,
                                range: range.map(|y| y - translation[1]),
                                color: *color,
                            });
                        }
                    }
                    Paint::Stroke {
                        round,
                        color,
                        points,
                        width,
                    } => {
                        let points: Vec<_> = points
                            .iter()
                            .map(|[x, y]| [x - translation[0], y - translation[1]])
                            .collect();
                        let left =
                            points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min) - width;
                        let top = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min) - width;
                        let right = points
                            .iter()
                            .map(|p| p[0])
                            .fold(f32::NEG_INFINITY, f32::max)
                            + width;
                        let bottom = points
                            .iter()
                            .map(|p| p[1])
                            .fold(f32::NEG_INFINITY, f32::max)
                            + width;
                        let visible = intersect([left, top, right - left, bottom - top], clip);
                        if visible[2] > 0.0
                            && visible[3] > 0.0
                            && node.clips.iter().all(|shape| shape.intersects(visible))
                        {
                            painted.push(Paint::Stroke {
                                round: *round,
                                color: *color,
                                points,
                                width: *width,
                            });
                        }
                    }
                    Paint::Border {
                        corners,
                        color,
                        bounds,
                        widths,
                        radius,
                        style,
                    } => {
                        let mut bounds = *bounds;
                        bounds[0] -= translation[0];
                        bounds[1] -= translation[1];
                        let visible = intersect(bounds, clip);
                        if visible[2] > 0.0
                            && visible[3] > 0.0
                            && node.clips.iter().all(|shape| shape.intersects(visible))
                        {
                            painted.push(Paint::Border {
                                corners: *corners,
                                color: tint(color, border, paint_style.border),
                                bounds,
                                widths: *widths,
                                radius: *radius,
                                style: *style,
                            });
                        }
                    }
                    Paint::Gradient {
                        corners,
                        gradient,
                        bounds,
                        radius,
                    } => {
                        let mut bounds = *bounds;
                        bounds[0] -= translation[0];
                        bounds[1] -= translation[1];
                        let visible = intersect(bounds, clip);
                        if visible[2] > 0.0
                            && visible[3] > 0.0
                            && node.clips.iter().all(|shape| shape.intersects(visible))
                        {
                            // A `background-position` keyframe moves the image inside the box.
                            let gradient = match visual.and_then(|v| v.background_position) {
                                Some(position) => Arc::new(gradient.at_position(position)),
                                None => gradient.clone(),
                            };
                            painted.push(Paint::Gradient {
                                corners: *corners,
                                gradient,
                                bounds,
                                radius: *radius,
                            });
                        }
                    }
                    Paint::Rect {
                        corners,
                        color,
                        bounds,
                        radius,
                    } => {
                        let mut bounds = *bounds;
                        bounds[0] -= translation[0];
                        bounds[1] -= translation[1];
                        let visible = intersect(bounds, clip);
                        if visible[2] > 0.0
                            && visible[3] > 0.0
                            && node.clips.iter().all(|shape| shape.intersects(visible))
                        {
                            painted.push(Paint::Rect {
                                corners: *corners,
                                color: tint(color, background, paint_style.background),
                                bounds,
                                radius: *radius,
                            });
                        }
                    }
                    Paint::Image {
                        src,
                        bounds,
                        radius,
                        fit,
                        position,
                    } => {
                        let mut bounds = *bounds;
                        bounds[0] -= translation[0];
                        bounds[1] -= translation[1];
                        let visible = intersect(bounds, clip);
                        if visible[2] > 0.0
                            && visible[3] > 0.0
                            && node.clips.iter().all(|shape| shape.intersects(visible))
                        {
                            painted.push(Paint::Image {
                                src: src.clone(),
                                bounds,
                                radius: *radius,
                                fit: *fit,
                                position: *position,
                            });
                        }
                    }
                    Paint::Text {
                        color,
                        font,
                        size,
                        coordinates,
                        skew,
                        glyphs,
                        ink,
                        gradient,
                    } => {
                        let ink = ink.map(|mut bounds| {
                            bounds[0] -= translation[0];
                            bounds[1] -= translation[1];
                            bounds
                        });
                        // Focused inputs may reveal initially offscreen glyphs below.
                        if node.input.is_none()
                            && ink.is_some_and(|bounds| {
                                let visible = intersect(bounds, clip);
                                visible[2] == 0.0
                                    || visible[3] == 0.0
                                    || node.clips.iter().any(|shape| !shape.intersects(visible))
                            })
                        {
                            continue;
                        }
                        let inherited = paint_style
                            .texts
                            .binary_search_by_key(&command, |(command, _)| *command)
                            .ok()
                            .and_then(|at| foregrounds[paint_style.texts[at].1]);
                        let [sx, sy] = match (
                            visual.and_then(|v| v.background_position),
                            &paint_style.text_gradient,
                        ) {
                            (Some(position), Some((source, bounds))) => {
                                source.position_shift(*bounds, position)
                            }
                            _ => [0.0, 0.0],
                        };
                        painted.push(Paint::Text {
                            gradient: gradient.as_ref().map(|brush| {
                                Arc::new(brush.then(Affine::translate((
                                    sx - f64::from(translation[0]),
                                    sy - f64::from(translation[1]),
                                ))))
                            }),
                            color: inherited.unwrap_or(*color),
                            font: font.clone(),
                            size: *size,
                            coordinates: coordinates.clone(),
                            skew: *skew,
                            ink,
                            glyphs: glyphs
                                .iter()
                                .map(|g| Glyph {
                                    id: g.id,
                                    x: g.x - translation[0],
                                    y: g.y - translation[1],
                                })
                                .collect(),
                        });
                    }
                    _ => unreachable!("retained paint has no clipping commands"),
                }
            }
            if let Some(color) = background.filter(|_| paint_style.background.is_none()) {
                let at = painted
                    .iter()
                    .take_while(|p| matches!(p, Paint::Shadow { .. } | Paint::RasterShadow { .. }))
                    .count();
                painted.insert(
                    at,
                    Paint::Rect {
                        corners: paint_style.corners,
                        color,
                        bounds: node.bounds,
                        radius: paint_style.radius,
                    },
                );
            }
            if let Some(layers) = shadows {
                painted.splice(
                    0..0,
                    layers.iter().rev().map(|layer| Paint::Shadow {
                        shadow: crate::scene::shadow::BoxShadow {
                            offset_x: layer.offset[0] as f32,
                            offset_y: layer.offset[1] as f32,
                            blur: layer.blur as f32,
                            spread: layer.spread as f32,
                            color: "currentColor".into(),
                        },
                        color: layer.color,
                        bounds: node.bounds,
                        radius: paint_style.radius,
                    }),
                );
            }
            if painted.is_empty() && self.backdrops[index].is_none() {
                continue;
            }
            sync_layers(&mut commands, &mut active_layers, desired);
            let shadows = painted
                .iter()
                .take_while(|p| matches!(p, Paint::Shadow { .. } | Paint::RasterShadow { .. }))
                .count();
            commands.extend(painted.drain(..shadows));
            // Editors clip their contents, but outer shadows belong outside that clip.
            if node.input.is_some() {
                commands.push(Paint::Clip(ClipSnapshot {
                    radii: node.radii,
                    bounds: node.bounds,
                    radius: node.radius,
                    transform: None,
                }));
            }
            let inner_clip = node.content_clip.filter(|_| {
                matches!(
                    node.tag,
                    "image"
                        | "vector"
                        | "input"
                        | "textarea"
                        | "search"
                        | "checkbox"
                        | "switch"
                        | "radio-option"
                        | "slider"
                        | "plot"
                )
            });
            if let Some(inner) = inner_clip {
                let background = painted
                    .iter()
                    .position(|paint| matches!(paint, Paint::Border { .. }))
                    .map(|index| index + 1)
                    .unwrap_or(0);
                commands.extend(painted.drain(..background));
                commands.push(Paint::Clip(inner));
            }
            if node.input.is_some() {
                let background = painted
                    .iter()
                    .take_while(|p| !matches!(p, Paint::Text { .. }))
                    .count();
                input_paints.push((
                    index,
                    commands.len() + background..commands.len() + painted.len(),
                ));
            }
            if self.selectables[index].is_some() {
                selectable_paints.push((index, commands.len()));
            }
            commands.append(&mut painted);
            if inner_clip.is_some() {
                commands.push(Paint::PopClip);
            }
            if node.input.is_some() {
                commands.push(Paint::PopClip);
            }
        }
        if let Some(span) = open {
            close(
                &mut commands,
                &mut active_layers,
                &child_layers,
                &mut spans,
                span,
            );
        }
        for layer in active_layers.iter().rev() {
            if matches!(layer, Paint::Transform(_)) {
                continue;
            }
            commands.push(match layer {
                Paint::Opacity { .. } => Paint::PopOpacity,
                Paint::Backdrop { .. } => Paint::PopBackdrop,
                Paint::Mask { mask, .. } => Paint::PopMask(mask.clone()),
                Paint::Blur { .. } => Paint::PopBlur,
                _ => Paint::PopClip,
            });
        }
        for ((node, matrix), clips) in nodes.iter_mut().zip(transforms).zip(snapshot_clips) {
            node.clips = clips;
            if matrix == Affine::IDENTITY {
                continue;
            }
            if node.plot.is_some() {
                node.plot_local = Some((node.control_bounds.unwrap_or(node.bounds), matrix));
            }
            let border = transformed_clip(
                ClipSnapshot {
                    bounds: node.bounds,
                    radius: node.radius,
                    radii: node.radii,
                    transform: None,
                },
                matrix,
            )?;
            node.bounds = border.extent();
            if border.transform.is_some() {
                // Hits follow the rotated shape, not its extent.
                node.clips.push(border);
                node.radius = 0.0;
                node.radii = None;
            } else {
                node.radius = border.radius;
                node.radii = border.radii;
            }
            node.visible_bounds = transformed_bounds(node.visible_bounds, matrix)?;
            node.content_clip = node
                .content_clip
                .map(|clip| transformed_clip(clip, matrix))
                .transpose()?;
            node.control_bounds = node
                .control_bounds
                .map(|bounds| transformed_bounds(bounds, matrix))
                .transpose()?;
            for rect in &mut node.hit_rects {
                *rect = transformed_bounds(*rect, matrix)?;
            }
        }
        Ok(DisplayList {
            animated: spans,
            images: HashMap::new(),
            commands,
            input_paints,
            selectable_paints,
            caret_paint: None,
            nodes,
            width: self.width,
            height: self.height,
            scale: self.scale,
            retained: Arc::clone(self),
        })
    }
}

/// Pop and push from the active layer stack to `desired`.
fn sync_layers(commands: &mut Vec<Paint>, active_layers: &mut Vec<Paint>, desired: Vec<Paint>) {
    let shared = active_layers
        .iter()
        .zip(&desired)
        .take_while(|(a, b)| a == b)
        .count();
    let mut left_transform = false;
    for layer in active_layers[shared..].iter().rev() {
        if matches!(layer, Paint::Transform(_)) {
            left_transform = true;
            continue;
        }
        commands.push(match layer {
            Paint::Opacity { .. } => Paint::PopOpacity,
            Paint::Backdrop { .. } => Paint::PopBackdrop,
            Paint::Mask { mask, .. } => Paint::PopMask(mask.clone()),
            Paint::Blur { .. } => Paint::PopBlur,
            _ => Paint::PopClip,
        });
    }
    // Transforms are set, not pushed: leaving one restores the shared ancestor's,
    // which may compare equal to a sibling's and so is not in `desired[shared..]`.
    if left_transform {
        commands.push(Paint::Transform(last_transform(&desired[..shared])));
    }
    commands.extend_from_slice(&desired[shared..]);
    *active_layers = desired;
}

fn last_transform(layers: &[Paint]) -> Affine {
    layers
        .iter()
        .rev()
        .find_map(|layer| match layer {
            Paint::Transform(matrix) => Some(*matrix),
            _ => None,
        })
        .unwrap_or(Affine::IDENTITY)
}

/// Depth-first paint order with siblings stably sorted by z-index, then CSS
/// `order`; subtrees stay contiguous.
pub(crate) fn paint_order(parents: &[Option<usize>], z: &[(i32, i32, i32)]) -> Vec<usize> {
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); parents.len()];
    let mut roots = Vec::new();
    for (index, parent) in parents.iter().enumerate() {
        match parent {
            Some(parent) => children[*parent].push(index),
            None => roots.push(index),
        }
    }
    let mut order = Vec::with_capacity(parents.len());
    let mut stack: Vec<usize> = roots.into_iter().rev().collect();
    while let Some(index) = stack.pop() {
        order.push(index);
        let mut siblings = std::mem::take(&mut children[index]);
        siblings.sort_by_key(|child| z[*child]);
        stack.extend(siblings.into_iter().rev());
    }
    order
}

/// Logical bounds shrunk inward until every edge lands on a whole device pixel at `scale`.
fn snap_to_pixels([x, y, w, h]: [f32; 4], scale: f32) -> [f32; 4] {
    if !(scale.is_finite() && scale > 0.0) {
        return [x, y, w, h];
    }
    // A hair of tolerance keeps an edge already on a pixel from moving a whole row.
    let up = |v: f32| (v * scale - 1e-3).ceil() / scale;
    let down = |v: f32| (v * scale + 1e-3).floor() / scale;
    let (left, top) = (up(x), up(y));
    [
        left,
        top,
        (down(x + w) - left).max(0.0),
        (down(y + h) - top).max(0.0),
    ]
}

fn translation_of(
    t: crate::animation::transition::Decomposed,
) -> crate::animation::transition::Decomposed {
    crate::animation::transition::Decomposed {
        translate: t.translate,
        ..Default::default()
    }
}
