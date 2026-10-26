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
pub(crate) struct InputLayout {
    pub(crate) layout: Layout<()>,
    pub(crate) padding: [f32; 2],
    pub(crate) text_offset_y: f32,
    pub(crate) content_size: [f32; 2],
    pub(crate) color: [u8; 4],
}

/// Unscrolled geometry and shaped glyphs from one committed View render.
#[derive(Clone)]
pub(crate) struct RetainedLayout {
    pub(crate) nodes: Vec<NodeSnapshot>,
    pub(crate) commands: Vec<Paint>,
    pub(crate) ranges: Vec<Range<usize>>,
    pub(crate) parents: Vec<Option<usize>>,
    /// Paint order: depth-first, siblings stably ordered by z-index.
    pub(crate) order: Vec<usize>,
    pub(crate) opacities: Vec<f32>,
    pub(crate) backdrops: Vec<Option<(f32, f32)>>,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) scale: f32,
    pub(crate) inputs: Vec<Option<InputLayout>>,
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
        let animated = !visuals.is_empty();
        let count = self.nodes.len();
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
        let mut child_layers: Vec<Vec<Paint>> = vec![Vec::new(); count];
        let mut painted = Vec::new();
        let mut input_paints = Vec::new();
        for (position, &index) in self.order.iter().enumerate() {
            let node = &mut nodes[index];
            node.order = position as u32;
            let parent = self.parents[index];
            let mut desired = parent.map(|p| child_layers[p].clone()).unwrap_or_default();
            let translation = parent.map(|p| translations[p]).unwrap_or([0.0; 2]);
            let world_clip = parent.map(|p| clips[p]).unwrap_or(viewport);
            node.bounds[0] -= translation[0];
            node.bounds[1] -= translation[1];
            if let Some(content) = &mut node.content_clip {
                content.bounds[0] -= translation[0];
                content.bounds[1] -= translation[1];
            }
            if let Some(control) = &mut node.control_bounds {
                control[0] -= translation[0];
                control[1] -= translation[1];
            }
            let inherited = parent.map(|p| transforms[p]).unwrap_or(Affine::IDENTITY);
            let visual = visuals.get(&node.identity);
            let matrix = if let Some(visual) = visual {
                let [x, y, w, h] = node.bounds.map(f64::from);
                checked_paint_transform(inherited * visual.matrix([x + w / 2.0, y + h / 2.0])?)?
            } else {
                inherited
            };
            if matrix != Affine::IDENTITY
                && (node.input.is_some() || node.scroll.is_some() || node.range.is_some())
            {
                return Err(
                    "UI_ANIMATION: transformed editor, scroll or slider unsupported".into(),
                );
            }
            let alpha = visual
                .and_then(|v| v.opacity)
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
            let mut descendants = desired.clone();
            if clips_children {
                descendants.push(Paint::Clip(node.content_clip.unwrap_or(ClipSnapshot {
                    bounds: node.bounds,
                    radius: node.radius,
                    radii: node.radii,
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
                child_translation[scroll.axis.index()] += scroll.offset;
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
            for paint in &self.commands[self.ranges[index].clone()] {
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
                                color: *color,
                                bounds,
                                widths: *widths,
                                radius: *radius,
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
                            painted.push(Paint::Gradient {
                                corners: *corners,
                                gradient: gradient.clone(),
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
                                color: *color,
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
                        painted.push(Paint::Text {
                            color: *color,
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
            if painted.is_empty() && self.backdrops[index].is_none() {
                continue;
            }
            let shared = active_layers
                .iter()
                .zip(&desired)
                .take_while(|(a, b)| a == b)
                .count();
            for layer in active_layers[shared..].iter().rev() {
                if matches!(layer, Paint::Transform(_)) {
                    continue;
                }
                commands.push(match layer {
                    Paint::Opacity { .. } => Paint::PopOpacity,
                    Paint::Backdrop { .. } => Paint::PopBackdrop,
                    _ => Paint::PopClip,
                });
            }
            commands.extend_from_slice(&desired[shared..]);
            active_layers = desired;
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
            commands.append(&mut painted);
            if inner_clip.is_some() {
                commands.push(Paint::PopClip);
            }
            if node.input.is_some() {
                commands.push(Paint::PopClip);
            }
        }
        for layer in active_layers.iter().rev() {
            if matches!(layer, Paint::Transform(_)) {
                continue;
            }
            commands.push(match layer {
                Paint::Opacity { .. } => Paint::PopOpacity,
                Paint::Backdrop { .. } => Paint::PopBackdrop,
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
                },
                matrix,
            )?;
            node.bounds = border.bounds;
            node.radius = border.radius;
            node.radii = border.radii;
            node.visible_bounds = transformed_bounds(node.visible_bounds, matrix)?;
            node.content_clip = node
                .content_clip
                .map(|clip| transformed_clip(clip, matrix))
                .transpose()?;
            node.control_bounds = node
                .control_bounds
                .map(|bounds| transformed_bounds(bounds, matrix))
                .transpose()?;
        }
        Ok(DisplayList {
            images: HashMap::new(),
            commands,
            input_paints,
            caret_paint: None,
            nodes,
            width: self.width,
            height: self.height,
            scale: self.scale,
            retained: Arc::clone(self),
        })
    }
}

/// Depth-first paint order with siblings stably sorted by z-index; subtrees stay contiguous.
pub(crate) fn paint_order(parents: &[Option<usize>], z: &[i32]) -> Vec<usize> {
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
