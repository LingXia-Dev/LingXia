//! Immutable display lists shared by every renderer.
use crate::scene::node::*;
use crate::scene::paint::*;
use crate::scene::retained::*;
use crate::text::editing::*;
use std::{collections::HashMap, ops::Range, sync::Arc};
use vello_cpu::kurbo::Affine;

/// Immutable layout output shared by CPU reference rendering and GPU scene encoding.
pub struct DisplayList {
    /// Paint spans whose commands change from one animation frame to the next.
    pub(crate) animated: Vec<AnimatedSpan>,
    pub(crate) images: HashMap<String, crate::scene::image::Bitmap>,
    pub(crate) commands: Vec<Paint>,
    pub(crate) input_paints: Vec<(usize, Range<usize>)>,
    /// Where each selectable text's paints start, for the selection highlight.
    pub(crate) selectable_paints: Vec<(usize, usize)>,
    pub(crate) caret_paint: Option<usize>,
    pub nodes: Vec<NodeSnapshot>,
    pub width: u16,
    pub height: u16,
    pub scale: f32,
    pub(crate) retained: Arc<RetainedLayout>,
}

/// An outermost animated subtree's commands, holding this frame's sampled values. A span
/// opens from its parent's layer stack and closes back to it (pushes and pops balance), and
/// the command after it re-sets the parent's absolute transform. Commands outside spans
/// are identical across frames of one retained layout, offsets, images and caret.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedSpan {
    pub identity: String,
    pub commands: Range<usize>,
}

#[cfg(feature = "gpu")]
pub(crate) struct SceneKey {
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) scale: f32,
    pub(crate) commands: Vec<Paint>,
    pub(crate) images: HashMap<String, crate::scene::image::Bitmap>,
}

#[cfg(feature = "gpu")]
impl SceneKey {
    pub(crate) fn capture(display: &DisplayList) -> Option<Self> {
        Self::eligible(display, 256, 4096).then(|| Self {
            width: display.width,
            height: display.height,
            scale: display.scale,
            commands: display.commands.clone(),
            images: display.images.clone(),
        })
    }

    /// Whether a frame is small enough to keep its commands as a cache key.
    pub(crate) fn eligible(display: &DisplayList, max_commands: usize, max_glyphs: usize) -> bool {
        let glyphs: usize = display
            .commands
            .iter()
            .map(|paint| match paint {
                Paint::Text { glyphs, .. } => glyphs.len(),
                _ => 0,
            })
            .sum();
        let image_bytes: usize = display
            .images
            .values()
            .map(crate::scene::image::Bitmap::byte_len)
            .sum();
        !(display.commands.len() > max_commands
            || glyphs > max_glyphs
            || display.images.len() > 16
            || image_bytes > 8 * 1024 * 1024
            || u32::from(display.width) * u32::from(display.height) > 4 * 1024 * 1024
            || display
                .commands
                .iter()
                .any(|paint| matches!(paint, Paint::Text { size, .. } if *size > 256.0)))
    }

    pub(crate) fn matches(&self, display: &DisplayList) -> bool {
        self.width == display.width
            && self.height == display.height
            && self.scale == display.scale
            && self.commands == display.commands
            && self.images.len() == display.images.len()
            && self.images.iter().all(|(src, image)| {
                display
                    .images
                    .get(src)
                    .is_some_and(|other| Arc::ptr_eq(image.pixels(), other.pixels()))
            })
    }
}

impl DisplayList {
    pub fn animated_spans(&self) -> &[AnimatedSpan] {
        &self.animated
    }

    #[cfg(test)]
    pub(crate) fn animate_for_test(self, index: usize, visual: crate::animation::Visual) -> Self {
        let offsets = self
            .nodes
            .iter()
            .filter_map(|node| node.scroll.as_ref())
            .map(|scroll| (scroll.id.clone(), scroll.offset))
            .collect();
        let visuals =
            std::collections::BTreeMap::from([(self.nodes[index].identity.clone(), visual)]);
        let mut display = self.retained.compose_animated(&offsets, &visuals).unwrap();
        display.bind_images(&self.images).unwrap();
        display
    }
    /// Draw `paints` over everything else as an animated span of their own, so the page's
    /// own commands keep their encoded scenes while the overlay changes every frame.
    pub(crate) fn push_overlay(&mut self, identity: &str, paints: Vec<crate::scene::paint::Paint>) {
        if paints.is_empty() {
            return;
        }
        let start = self.commands.len();
        self.commands.extend(paints);
        self.animated.push(AnimatedSpan {
            identity: identity.to_owned(),
            commands: start..self.commands.len(),
        });
    }

    /// The caret blink's off phase.
    pub(crate) fn hide_caret(&mut self) {
        if let Some(index) = self.caret_paint.take() {
            self.commands.remove(index);
            for span in &mut self.animated {
                let range = &mut span.commands;
                if range.start <= index && index < range.end {
                    range.end -= 1;
                } else if range.start > index {
                    *range = range.start - 1..range.end - 1;
                }
            }
        }
    }

    /// Keep animated spans on their commands when `count` paints are inserted at `at`
    /// for the node whose paints are `owner`.
    fn spans_inserted(spans: &mut [AnimatedSpan], owner: Range<usize>, at: usize, count: usize) {
        for span in spans {
            let range = &mut span.commands;
            if range.start <= owner.start && owner.end <= range.end {
                range.end += count;
            } else if range.start >= at {
                *range = range.start + count..range.end + count;
            }
        }
    }
    pub(crate) fn decorate_inputs(&mut self) {
        use parley::{
            Affinity,
            editing::{Cursor, Selection},
        };
        // Reverse insertion preserves the command offsets of earlier input nodes.
        for (index, range) in self.input_paints.iter().rev() {
            let node = &mut self.nodes[*index];
            let Some(input) = &mut node.input else {
                continue;
            };
            let Some(state) = &input.edit else { continue };
            let Some(shaped) = &self.retained.inputs[*index] else {
                continue;
            };
            if !state.focused || state.text != input.value {
                input.scroll_offset = [0.0; 2];
                continue;
            }
            let layout = &shaped.layout;
            let mut origin = [
                node.bounds[0] + shaped.padding[0],
                node.bounds[1] + shaped.padding[1],
            ];
            let cursor = |offset| {
                display_byte(
                    &input.value,
                    offset,
                    input.secure,
                    matches!(node.tag, "input" | "search"),
                )
                .map(|byte| Cursor::from_byte_index(layout, byte, Affinity::Downstream))
            };
            let (Some(anchor), Some(focus)) =
                (cursor(state.selection[0]), cursor(state.selection[1]))
            else {
                continue;
            };
            let local_caret = focus.geometry(layout, 1.5);
            let content_bounds = [
                origin[0],
                origin[1],
                shaped.content_size[0],
                shaped.content_size[1],
            ];
            origin[1] += shaped.text_offset_y;
            input.scroll_offset = [
                reveal_caret(
                    input.scroll_offset[0],
                    local_caret.x0 as f32,
                    local_caret.x1 as f32,
                    shaped.content_size[0],
                    layout.width(),
                ),
                reveal_caret(
                    input.scroll_offset[1],
                    local_caret.y0 as f32 + shaped.text_offset_y,
                    local_caret.y1 as f32 + shaped.text_offset_y,
                    shaped.content_size[1],
                    layout.height() + shaped.text_offset_y,
                ),
            ];
            for (origin, offset) in origin.iter_mut().zip(input.scroll_offset) {
                *origin -= offset;
            }
            for paint in &mut self.commands[range.clone()] {
                if let Paint::Text { glyphs, ink, .. } = paint {
                    for glyph in glyphs {
                        glyph.x -= input.scroll_offset[0];
                        glyph.y -= input.scroll_offset[1];
                    }
                    if let Some(ink) = ink {
                        ink[0] -= input.scroll_offset[0];
                        ink[1] -= input.scroll_offset[1];
                    }
                }
            }
            let bounds = |rect: parley::BoundingBox| {
                [
                    origin[0] + rect.x0 as f32,
                    origin[1] + rect.y0 as f32,
                    (rect.x1 - rect.x0) as f32,
                    (rect.y1 - rect.y0) as f32,
                ]
            };
            let selection: Vec<_> = Selection::new(anchor, focus)
                .geometry(layout)
                .into_iter()
                .map(|(rect, _)| bounds(rect))
                .collect();
            let composition: Vec<_> = state
                .composition
                .and_then(|[a, b]| Some((cursor(a)?, cursor(b)?)))
                .map(|(a, b)| {
                    Selection::new(a, b)
                        .geometry(layout)
                        .into_iter()
                        .map(|(rect, _)| {
                            let mut rect = bounds(rect);
                            rect[1] += rect[3] - 1.0;
                            rect[3] = 1.0;
                            rect
                        })
                        .collect()
                })
                .unwrap_or_default();
            let caret = bounds(local_caret);
            let rect = |bounds, color| Paint::Rect {
                corners: None,
                bounds,
                color,
                radius: 0.0,
            };
            let mut foreground: Vec<_> =
                composition.iter().map(|b| rect(*b, shaped.color)).collect();
            if state.selection[0] == state.selection[1] {
                // The content clip and selection paints are inserted before this range.
                self.caret_paint = Some(range.end + foreground.len() + 1 + selection.len());
                foreground.push(rect(caret, shaped.color));
            }
            foreground.push(Paint::PopClip);
            let owner = range.clone();
            Self::spans_inserted(
                &mut self.animated,
                owner.clone(),
                range.end,
                foreground.len(),
            );
            self.commands.splice(range.end..range.end, foreground);
            let fill = self.retained.selection_fill;
            Self::spans_inserted(&mut self.animated, owner, range.start, 1 + selection.len());
            self.commands.splice(
                range.start..range.start,
                std::iter::once(Paint::Clip(ClipSnapshot {
                    radii: None,
                    bounds: content_bounds,
                    radius: 0.0,
                    transform: None,
                }))
                .chain(selection.iter().map(|b| rect(*b, fill))),
            );
            input.geometry = Some(InputGeometry {
                anchor: bounds(anchor.geometry(layout, 1.5)),
                caret,
                content_bounds,
                selection,
                composition,
            });
        }
    }
    /// Paint a static-text selection and publish its geometry for chrome; `false`
    /// when the selection no longer fits its node and must be dropped.
    pub(crate) fn decorate_selection(
        &mut self,
        selection: &crate::interaction::input_state::TextSelection,
    ) -> bool {
        use parley::{
            Affinity,
            editing::{Cursor, Selection},
        };
        let Some(index) = self
            .nodes
            .iter()
            .position(|node| node.identity == selection.identity)
        else {
            return false;
        };
        let Some(text) = self.retained.selectables[index].clone() else {
            return false;
        };
        if text.text != selection.text
            || [selection.anchor, selection.focus]
                .iter()
                .any(|i| *i > text.text.len() || !text.text.is_char_boundary(*i))
        {
            return false;
        }
        // Composition moved the node (scrolls, sticky, transforms keep its size).
        let base = &self.retained.nodes[index].bounds;
        let node = &mut self.nodes[index];
        let origin = [
            text.origin[0] + node.bounds[0] - base[0],
            text.origin[1] + node.bounds[1] - base[1],
        ];
        let content = [
            text.content[0] + node.bounds[0] - base[0],
            text.content[1] + node.bounds[1] - base[1],
            text.content[2],
            text.content[3],
        ];
        let cursor = |byte| Cursor::from_byte_index(&text.layout, byte, Affinity::Downstream);
        let (anchor, focus) = (cursor(selection.anchor), cursor(selection.focus));
        let bounds = |rect: parley::BoundingBox| {
            [
                origin[0] + rect.x0 as f32,
                origin[1] + rect.y0 as f32,
                (rect.x1 - rect.x0) as f32,
                (rect.y1 - rect.y0) as f32,
            ]
        };
        let rects: Vec<_> = Selection::new(anchor, focus)
            .geometry(&text.layout)
            .into_iter()
            .map(|(rect, _)| bounds(rect))
            .collect();
        let units = |byte: usize| text.text[..byte].encode_utf16().count() as u32;
        node.selection_range = Some([units(selection.anchor), units(selection.focus)]);
        node.text_selection = Some(InputGeometry {
            anchor: bounds(anchor.geometry(&text.layout, 1.5)),
            caret: bounds(focus.geometry(&text.layout, 1.5)),
            content_bounds: content,
            selection: rects.clone(),
            composition: Vec::new(),
        });
        let Some(&(_, start)) = self
            .selectable_paints
            .iter()
            .find(|(node, _)| *node == index)
        else {
            return true;
        };
        let count = rects.len();
        Self::spans_inserted(&mut self.animated, start..start, start, count);
        self.commands.splice(
            start..start,
            rects.into_iter().map(|bounds| Paint::Rect {
                corners: None,
                bounds,
                color: self.retained.selection_fill,
                radius: 0.0,
            }),
        );
        if let Some(caret) = &mut self.caret_paint
            && *caret >= start
        {
            *caret += count;
        }
        true
    }

    /// Only currently painted sources are requested; list overscan does not load images.
    pub fn image_sources(&self) -> impl Iterator<Item = &str> {
        self.commands.iter().filter_map(|paint| match paint {
            Paint::Image { src, .. } => Some(src.as_str()),
            _ => None,
        })
    }
    /// Each painted source's decode bucket: the union of its boxes, in device pixels.
    pub fn image_buckets(&self) -> HashMap<String, crate::scene::image::Bucket> {
        let mut buckets = HashMap::new();
        for paint in &self.commands {
            if let Paint::Image { src, bounds, .. } = paint {
                let bucket =
                    crate::scene::image::Bucket::of(bounds[2] * self.scale, bounds[3] * self.scale);
                let entry = buckets.entry(src.clone()).or_insert(bucket);
                *entry = entry.union(bucket);
            }
        }
        buckets
    }
    /// Bind authorized, decoded pixels without rerunning View or changing layout.
    pub fn bind_images(
        &mut self,
        images: &HashMap<String, crate::scene::image::Bitmap>,
    ) -> Result<(), String> {
        if self
            .image_sources()
            .collect::<std::collections::HashSet<_>>()
            .len()
            > 128
        {
            return Err("UI_IMAGE: too many visible sources".into());
        }
        // The session binds at most `VISIBLE_IMAGE_BYTES` of painted sources.
        let bound: HashMap<_, _> = self
            .image_sources()
            .filter_map(|src| images.get(src).map(|image| (src.to_owned(), image.clone())))
            .collect();
        self.images = bound;
        Ok(())
    }
    pub(crate) fn paint_count(&self) -> usize {
        self.commands.len()
    }

    pub fn loaded_image_count(&self) -> usize {
        self.images.len()
    }
    #[cfg(any(feature = "gpu", test))]
    pub(crate) fn bitmaps(&self) -> impl Iterator<Item = &crate::scene::image::Bitmap> {
        self.images
            .values()
            .chain(self.commands.iter().filter_map(|paint| match paint {
                Paint::RasterShadow { raster } => Some(&raster.bitmap),
                _ => None,
            }))
    }

    /// Lower retained paint order into shared group scopes. Index zero draws the
    /// surface background; remaining span indices address paints plus one.
    pub fn composition_plan(&self) -> Result<crate::scene::composite::Plan, String> {
        use crate::scene::composite::{Layer, Plan, Step};
        let inline = self.inline_groups();
        let mut steps = Vec::new();
        let mut groups = Vec::new();
        let mut start = 0;
        let mut transform = Affine::IDENTITY;
        for (index, paint) in self.commands.iter().enumerate() {
            if inline[index] {
                // Drawn in its span, as the flat path draws it: no surface of its own.
                continue;
            }
            // `Some(None)` ends the span without a group: a clip that clips nothing.
            let marker = match paint {
                Paint::Transform(matrix) => {
                    transform = checked_paint_transform(*matrix)?;
                    None
                }
                Paint::Clip(clip) => {
                    let clip = transformed_clip(*clip, transform)?.scaled(self.scale);
                    if self.covers_frame(&clip) {
                        groups.push(5);
                        Some(None)
                    } else {
                        groups.push(0);
                        Some(Some(Step::Push(Layer::Clip(clip))))
                    }
                }
                Paint::Opacity { alpha, .. } => {
                    groups.push(1);
                    Some(Some(Step::Push(Layer::Opacity(*alpha))))
                }
                Paint::Backdrop {
                    clip,
                    blur,
                    saturation,
                    opacity,
                    ..
                } => {
                    let [sx, _, _, sy, _, _] = transform.as_coeffs();
                    if sx != sy || !axis_aligned(transform) {
                        return Err(
                            "UI_TRANSFORM: backdrops need a uniform, unrotated transform".into(),
                        );
                    }
                    let clip = transformed_clip(*clip, transform)?.scaled(self.scale);
                    groups.push(2);
                    Some(Some(Step::Push(Layer::Backdrop {
                        clip,
                        blur: blur * self.scale * sx as f32,
                        saturation: crate::scene::color_filter::Saturation::new(*saturation)?,
                        opacity: *opacity,
                    })))
                }
                Paint::Mask { mask, .. } => {
                    groups.push(3);
                    Some(Some(Step::Push(Layer::Mask(std::sync::Arc::new(
                        crate::scene::composite::DeviceMask::new(
                            mask,
                            Affine::scale(f64::from(self.scale)),
                        ),
                    )))))
                }
                Paint::Blur { radius, .. } => {
                    let [sx, _, _, sy, _, _] = transform.as_coeffs();
                    groups.push(4);
                    Some(Some(Step::Push(Layer::Blur(
                        radius * self.scale * ((sx * sy).abs().sqrt() as f32),
                    ))))
                }
                Paint::PopClip
                | Paint::PopOpacity
                | Paint::PopBackdrop
                | Paint::PopMask(_)
                | Paint::PopBlur => {
                    let kind = match paint {
                        Paint::PopClip => 0,
                        Paint::PopOpacity => 1,
                        Paint::PopMask(_) => 3,
                        Paint::PopBlur => 4,
                        _ => 2,
                    };
                    match groups.pop() {
                        Some(5) if kind == 0 => Some(None),
                        Some(open) if open == kind => Some(Some(Step::Pop)),
                        _ => return Err("UI_COMPOSITE: mismatched display-list group".into()),
                    }
                }
                _ => None,
            };
            if let Some(marker) = marker {
                let end = index + 1;
                if start < end {
                    steps.push(Step::Draw(start..end));
                }
                steps.extend(marker);
                start = end + 1;
            }
        }
        if start < self.commands.len() + 1 {
            steps.push(Step::Draw(start..self.commands.len() + 1));
        }
        let extents = self.group_extents(&inline)?;
        Plan::flattened_within(
            self.width,
            self.height,
            self.commands.len() + 1,
            steps,
            extents,
        )
        .map(|(plan, _)| plan)
    }

    /// The device box each group surface needs, in push order: what its commands and nested
    /// groups paint, cut to its own clip, spread by its blur and holding its backdrop.
    fn group_extents(
        &self,
        inline: &[bool],
    ) -> Result<Vec<crate::scene::composite::Extent>, String> {
        // Per open group: its push ordinal (none for a clip that keeps the frame), what it
        // paints, and how its box changes on the way out.
        let mut open: Vec<(Option<usize>, [f32; 4], Exit)> = vec![(None, EMPTY, Exit::Keep)];
        let mut extents = Vec::new();
        let mut transform = Affine::IDENTITY;
        let device = |b: [f32; 4]| -> [f32; 4] {
            let [x, y, w, h] = b.map(|v| v * self.scale);
            [x - 2.0, y - 2.0, x + w + 2.0, y + h + 2.0]
        };
        for (index, paint) in self.commands.iter().enumerate() {
            let exit = match paint {
                Paint::Transform(matrix) => {
                    transform = checked_paint_transform(*matrix)?;
                    continue;
                }
                _ if inline[index] => continue,
                Paint::Clip(clip) => {
                    let clip = transformed_clip(*clip, transform)?.scaled(self.scale);
                    if self.covers_frame(&clip) {
                        open.push((None, EMPTY, Exit::Keep));
                        continue;
                    }
                    let [x, y, w, h] = clip.extent();
                    Exit::Cut([x, y, x + w, y + h])
                }
                Paint::Opacity { .. } | Paint::Mask { .. } => Exit::Keep,
                Paint::Backdrop { clip, .. } => {
                    let [x, y, w, h] = transformed_clip(*clip, transform)?
                        .scaled(self.scale)
                        .extent();
                    Exit::Hold([x, y, x + w, y + h])
                }
                Paint::Blur { radius, .. } => {
                    let [sx, _, _, sy, _, _] = transform.as_coeffs();
                    Exit::Spread(3.0 * radius * self.scale * ((sx * sy).abs().sqrt() as f32) + 2.0)
                }
                Paint::PopClip
                | Paint::PopOpacity
                | Paint::PopBackdrop
                | Paint::PopMask(_)
                | Paint::PopBlur => {
                    let Some((ordinal, mut painted, exit)) = open.pop() else {
                        return Err("UI_COMPOSITE: mismatched display-list group".into());
                    };
                    painted = match exit {
                        Exit::Keep => painted,
                        Exit::Cut(clip) => intersect(painted, clip),
                        Exit::Hold(clip) => union(painted, clip),
                        Exit::Spread(by) if painted[0] <= painted[2] => [
                            painted[0] - by,
                            painted[1] - by,
                            painted[2] + by,
                            painted[3] + by,
                        ],
                        Exit::Spread(_) => painted,
                    };
                    if let Some(ordinal) = ordinal {
                        extents[ordinal] = self.extent(painted);
                    }
                    let parent = open
                        .last_mut()
                        .ok_or("UI_COMPOSITE: mismatched display-list group")?;
                    parent.1 = union(parent.1, painted);
                    continue;
                }
                _ => {
                    let painted = match self.paint_box(paint, transform)? {
                        Some(Some(b)) => device(b),
                        Some(None) => EMPTY,
                        None => [0.0, 0.0, f32::from(self.width), f32::from(self.height)],
                    };
                    let top = open.last_mut().unwrap();
                    top.1 = union(top.1, painted);
                    continue;
                }
            };
            open.push((Some(extents.len()), EMPTY, exit));
            extents.push([0, 0, self.width, self.height]);
        }
        Ok(extents)
    }

    /// The logical box `paint` can touch under `transform`: `Some(None)` when it paints nothing,
    /// `None` when it is not bounded.
    fn paint_box(
        &self,
        paint: &Paint,
        transform: Affine,
    ) -> Result<Option<Option<[f32; 4]>>, String> {
        let local = match paint {
            Paint::Rect { bounds, .. }
            | Paint::Gradient { bounds, .. }
            | Paint::Border { bounds, .. }
            | Paint::Image { bounds, .. }
            | Paint::Vector { bounds, .. } => *bounds,
            Paint::RasterShadow { raster } => raster.bounds,
            Paint::Shadow {
                shadow,
                bounds,
                radius,
                ..
            } => match shadow.geometry(*bounds, *radius) {
                Some(shape) => {
                    let pad = 3.0 * shape.sigma + 1.0;
                    let r = shape.rect;
                    [
                        r.x0 as f32 - pad,
                        r.y0 as f32 - pad,
                        r.width() as f32 + 2.0 * pad,
                        r.height() as f32 + 2.0 * pad,
                    ]
                }
                None => return Ok(Some(None)),
            },
            Paint::Stroke { points, width, .. } if !points.is_empty() => {
                let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                for [x, y] in points {
                    (x0, y0, x1, y1) = (x0.min(*x), y0.min(*y), x1.max(*x), y1.max(*y));
                }
                let pad = width.max(0.0) + 1.0;
                [x0 - pad, y0 - pad, x1 - x0 + 2.0 * pad, y1 - y0 + 2.0 * pad]
            }
            Paint::Text { ink: Some(ink), .. } if *ink == [0.0; 4] => return Ok(Some(None)),
            Paint::Text { ink: Some(ink), .. } => *ink,
            _ => return Ok(None),
        };
        if !local.iter().all(|v| v.is_finite()) {
            return Ok(None);
        }
        Ok(Some(Some(transformed_bounds(local, transform)?)))
    }

    /// `painted` snapped outward to whole device pixels inside the frame.
    fn extent(&self, [x0, y0, x1, y1]: [f32; 4]) -> crate::scene::composite::Extent {
        let (width, height) = (f32::from(self.width), f32::from(self.height));
        if !(x0 < x1 && y0 < y1) {
            return [0, 0, 0, 0];
        }
        let clamp = |v: f32, max: f32| v.clamp(0.0, max) as u16;
        [
            clamp(x0.floor(), width),
            clamp(y0.floor(), height),
            clamp(x1.ceil(), width),
            clamp(y1.ceil(), height),
        ]
    }

    /// Opening and closing commands of groups that need no surface of their own: clips and
    /// opacity groups with no other surface inside, which their span draws as the flat path does.
    fn inline_groups(&self) -> Vec<bool> {
        let mut inline = vec![false; self.commands.len()];
        // Open groups: where each opened, whether it can be inline, and whether all inside is.
        let mut open: Vec<(usize, bool, bool)> = Vec::new();
        for (index, paint) in self.commands.iter().enumerate() {
            match paint {
                Paint::Clip(_) | Paint::Opacity { .. } => open.push((index, true, true)),
                Paint::Backdrop { .. } | Paint::Mask { .. } | Paint::Blur { .. } => {
                    open.push((index, false, true))
                }
                Paint::PopClip
                | Paint::PopOpacity
                | Paint::PopBackdrop
                | Paint::PopMask(_)
                | Paint::PopBlur => {
                    let Some((opened, kind, inside)) = open.pop() else {
                        // The plan reports the mismatch; nothing here is inline.
                        return vec![false; self.commands.len()];
                    };
                    let flat = kind && inside;
                    if flat {
                        inline[opened] = true;
                        inline[index] = true;
                    }
                    if let Some(parent) = open.last_mut() {
                        parent.2 &= flat;
                    }
                }
                _ => {}
            }
        }
        inline
    }

    /// A clip that keeps the whole frame: an axis-aligned, square-cornered box over every pixel.
    fn covers_frame(&self, clip: &crate::scene::node::ClipSnapshot) -> bool {
        let [x, y, w, h] = clip.bounds;
        clip.transform.is_none()
            && clip.radius == 0.0
            && clip.radii.is_none()
            && x <= 0.0
            && y <= 0.0
            && x + w >= f32::from(self.width)
            && y + h >= f32::from(self.height)
    }

    pub(crate) fn composition_span(
        &self,
        span: Range<usize>,
    ) -> Result<(bool, Range<usize>), String> {
        if span.start > span.end || span.end > self.commands.len() + 1 {
            return Err("UI_COMPOSITE: draw span outside display list".into());
        }
        Ok((
            span.start == 0 && !span.is_empty(),
            span.start.saturating_sub(1)..span.end.saturating_sub(1),
        ))
    }

    pub(crate) fn transform_before(&self, index: usize) -> Affine {
        self.commands[..index]
            .iter()
            .rev()
            .find_map(|paint| match paint {
                Paint::Transform(transform) => Some(*transform),
                _ => None,
            })
            .unwrap_or(Affine::IDENTITY)
    }
}

/// How a group's painted box changes as it composites into its parent.
enum Exit {
    Keep,
    /// Its clip cuts it.
    Cut([f32; 4]),
    /// Its backdrop fills this box.
    Hold([f32; 4]),
    /// Its blur spreads it by this many device pixels.
    Spread(f32),
}

/// Nothing painted, as `[x0, y0, x1, y1]`.
const EMPTY: [f32; 4] = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];

fn union(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}

fn intersect(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ]
}

#[cfg(test)]
#[path = "tests/animation.rs"]
mod animation_tests;
