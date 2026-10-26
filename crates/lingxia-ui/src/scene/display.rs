//! Immutable display lists shared by every renderer.
use crate::scene::node::*;
use crate::scene::paint::*;
use crate::scene::retained::*;
use crate::text::editing::*;
use std::{collections::HashMap, ops::Range, sync::Arc};
use vello_cpu::kurbo::Affine;

/// Immutable layout output shared by CPU reference rendering and GPU scene encoding.
pub struct DisplayList {
    pub(crate) images: HashMap<String, crate::scene::image::Bitmap>,
    pub(crate) commands: Vec<Paint>,
    pub(crate) input_paints: Vec<(usize, Range<usize>)>,
    pub(crate) caret_paint: Option<usize>,
    pub nodes: Vec<NodeSnapshot>,
    pub width: u16,
    pub height: u16,
    pub scale: f32,
    pub(crate) retained: Arc<RetainedLayout>,
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
        if display.commands.len() > 256
            || glyphs > 4096
            || display.images.len() > 16
            || image_bytes > 8 * 1024 * 1024
            || u32::from(display.width) * u32::from(display.height) > 4 * 1024 * 1024
            || display
                .commands
                .iter()
                .any(|paint| matches!(paint, Paint::Text { size, .. } if *size > 256.0))
        {
            return None;
        }
        Some(Self {
            width: display.width,
            height: display.height,
            scale: display.scale,
            commands: display.commands.clone(),
            images: display.images.clone(),
        })
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
    /// The caret blink's off phase.
    pub(crate) fn hide_caret(&mut self) {
        if let Some(index) = self.caret_paint.take() {
            self.commands.remove(index);
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
            self.commands.splice(range.end..range.end, foreground);
            let light_text = shaped.color.iter().take(3).map(|c| *c as u32).sum::<u32>() > 384;
            let fill = if light_text {
                [50, 78, 131, 255]
            } else {
                [216, 227, 255, 255]
            };
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
        let mut steps = Vec::new();
        let mut groups = Vec::new();
        let mut start = 0;
        let mut transform = Affine::IDENTITY;
        for (index, paint) in self.commands.iter().enumerate() {
            let marker = match paint {
                Paint::Transform(matrix) => {
                    transform = checked_paint_transform(*matrix)?;
                    None
                }
                Paint::Clip(clip) => {
                    let clip = transformed_clip(*clip, transform)?.scaled(self.scale);
                    groups.push(0);
                    Some(Step::Push(Layer::Clip(clip)))
                }
                Paint::Opacity { alpha, .. } => {
                    groups.push(1);
                    Some(Step::Push(Layer::Opacity(*alpha)))
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
                    Some(Step::Push(Layer::Backdrop {
                        clip,
                        blur: blur * self.scale * sx as f32,
                        saturation: crate::scene::color_filter::Saturation::new(*saturation)?,
                        opacity: *opacity,
                    }))
                }
                Paint::PopClip | Paint::PopOpacity | Paint::PopBackdrop => {
                    let kind = match paint {
                        Paint::PopClip => 0,
                        Paint::PopOpacity => 1,
                        _ => 2,
                    };
                    if groups.pop() != Some(kind) {
                        return Err("UI_COMPOSITE: mismatched display-list group".into());
                    }
                    Some(Step::Pop)
                }
                _ => None,
            };
            if let Some(marker) = marker {
                let end = index + 1;
                if start < end {
                    steps.push(Step::Draw(start..end));
                }
                steps.push(marker);
                start = end + 1;
            }
        }
        if start < self.commands.len() + 1 {
            steps.push(Step::Draw(start..self.commands.len() + 1));
        }
        Plan::new(self.width, self.height, self.commands.len() + 1, steps)
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

#[cfg(test)]
#[path = "tests/animation.rs"]
mod animation_tests;
