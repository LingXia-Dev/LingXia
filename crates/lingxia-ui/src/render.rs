use crate::schema::{Record, color};
use parley::style::{FontStyle, FontWeight, GenericFamily, LineHeight};
use parley::{
    Alignment, AlignmentOptions, FontContext, Layout, LayoutContext, PositionedLayoutItem,
    StyleProperty,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    ops::Range,
    sync::Arc,
};
use taffy::prelude::*;
use taffy::util::ResolveOrZero;
use taffy::{Overflow, Point};
use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::kurbo::{Affine, Rect as PaintRect, RoundedRect, Shape};
use vello_cpu::{Glyph, Pixmap, RenderContext, Resources};

fn dimension<T: taffy::style_helpers::FromLength + taffy::style_helpers::FromPercent>(
    record: &Record,
    field: usize,
) -> Option<T> {
    let index = [0, 1, 28, 29, 30, 31].iter().position(|id| *id == field)?;
    record
        .number(74 + index)
        .map(|value| percent(value / 100.0))
        .or_else(|| record.number(field).map(length))
}

fn cross_alignment(value: &str) -> AlignItems {
    match value {
        "center" => AlignItems::SAFE_CENTER,
        "end" => AlignItems::SAFE_FLEX_END,
        "stretch" => AlignItems::STRETCH,
        "baseline" => AlignItems::BASELINE,
        _ => AlignItems::FLEX_START,
    }
}

fn check_path(points: &[[f32; 2]]) -> vello_cpu::kurbo::BezPath {
    let mut path = vello_cpu::kurbo::BezPath::new();
    path.move_to((points[0][0] as f64, points[0][1] as f64));
    for point in &points[1..] {
        path.line_to((point[0] as f64, point[1] as f64));
    }
    path
}

fn check_stroke(width: f32, round: bool) -> vello_cpu::kurbo::Stroke {
    vello_cpu::kurbo::Stroke::new(width as f64)
        .with_caps(if round {
            vello_cpu::kurbo::Cap::Round
        } else {
            vello_cpu::kurbo::Cap::Butt
        })
        .with_join(vello_cpu::kurbo::Join::Round)
}

fn plot_bar_path(bounds: [f32; 4]) -> vello_cpu::kurbo::BezPath {
    let [x, y, w, h] = bounds.map(f64::from);
    let r = 4.0_f64.min(w / 2.0).min(h);
    let k = r * 0.552_284_749_830_793_6;
    let mut path = vello_cpu::kurbo::BezPath::new();
    path.move_to((x + r, y));
    path.line_to((x + w - r, y));
    path.curve_to((x + w - r + k, y), (x + w, y + r - k), (x + w, y + r));
    path.line_to((x + w, y + h));
    path.line_to((x, y + h));
    path.line_to((x, y + r));
    path.curve_to((x, y + r - k), (x + r - k, y), (x + r, y));
    path.close_path();
    path
}

fn area_path(points: &[[f32; 2]]) -> vello_cpu::kurbo::BezPath {
    let mut path = check_path(points);
    path.close_path();
    path
}

fn area_gradient(
    [top, zero, bottom]: [f32; 3],
    [r, g, b, a]: [u8; 4],
) -> vello_cpu::peniko::Gradient {
    let color = AlphaColor::<Srgb>::from_rgba8(r, g, b, a);
    let clear = AlphaColor::<Srgb>::from_rgba8(r, g, b, 0);
    let gradient = vello_cpu::peniko::Gradient::new_linear((0.0, top as f64), (0.0, bottom as f64));
    if zero <= top {
        gradient.with_stops([(0.0, clear), (1.0, color)])
    } else if zero >= bottom {
        gradient.with_stops([(0.0, color), (1.0, clear)])
    } else {
        gradient.with_stops([
            (0.0, color),
            ((zero - top) / (bottom - top), clear),
            (1.0, color),
        ])
    }
}

fn check_reserve(tag: usize) -> f32 {
    match tag {
        10 | 13 => 36.0,
        11 => 64.0,
        _ => 0.0,
    }
}
fn text_extent(layout: &Layout<()>, lines: Option<usize>) -> (f32, f32) {
    let Some(limit) = lines else {
        return (layout.width(), layout.height());
    };
    let mut width: f32 = 0.0;
    let mut height = 0.0;
    for line in layout.lines().take(limit) {
        let metrics = line.metrics();
        width = width.max(metrics.advance - metrics.hanging_advance);
        height = metrics.block_max_coord;
    }
    (width, height)
}

fn check_height(tag: usize) -> f32 {
    match tag {
        10 | 13 => 24.0,
        11 => 32.0,
        _ => 0.0,
    }
}

#[derive(Clone, Serialize)]
pub struct NodeSnapshot {
    pub identity: String,
    pub tag: &'static str,
    pub test_id: String,
    pub text: String,
    pub bounds: [f32; 4],
    pub visible_bounds: [f32; 4],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub clips: Vec<ClipSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll: Option<ScrollSnapshot>,
    pub on_click: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<crate::SliderSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plot: Option<crate::plot::PlotSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radio_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll_parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility_hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radii: Option<[[f32; 2]; 4]>,
    pub radius: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_clip: Option<ClipSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_bounds: Option<[f32; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<InputSnapshot>,
    #[serde(skip)]
    plot_local: Option<([f32; 4], Affine)>,
}

impl NodeSnapshot {
    /// Resolve a plot item using the same rounded clips as rendering.
    pub fn hit_test_plot(&self, point: [f32; 2]) -> Option<usize> {
        if point.iter().any(|value| !value.is_finite())
            || !(ClipSnapshot {
                bounds: self.visible_bounds,
                radius: 0.0,
                radii: None,
            })
            .contains(point)
            || self.clips.iter().any(|clip| !clip.contains(point))
            || self.content_clip.is_some_and(|clip| !clip.contains(point))
        {
            return None;
        }
        let (bounds, point) = if let Some((bounds, transform)) = self.plot_local {
            let point = transform.inverse()
                * vello_cpu::kurbo::Point::new(point[0] as f64, point[1] as f64);
            (bounds, [point.x as f32, point.y as f32])
        } else {
            (self.control_bounds.unwrap_or(self.bounds), point)
        };
        self.plot.as_ref()?.hit_test(bounds, point)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct InputSnapshot {
    pub value: String,
    pub placeholder: String,
    pub secure: bool,
    pub keyboard: String,
    pub auto_focus: bool,
    pub submit_on_enter: bool,
    pub enabled: bool,
    pub on_input: Option<u64>,
    pub on_submit: Option<u64>,
    pub edit: Option<crate::text_input::Snapshot>,
    pub edit_token: Option<u64>,
    pub geometry: Option<InputGeometry>,
    pub scroll_offset: [f32; 2],
}

#[derive(Clone, Debug, Serialize)]
pub struct InputGeometry {
    /// Logical selection anchor, in surface dp; caret is the logical focus.
    pub anchor: [f32; 4],
    pub caret: [f32; 4],
    /// Content viewport; endpoint rectangles are not clamped to it.
    pub content_bounds: [f32; 4],
    pub selection: Vec<[f32; 4]>,
    pub composition: Vec<[f32; 4]>,
}

#[derive(Clone)]
struct InputLayout {
    layout: Layout<()>,
    padding: [f32; 2],
    text_offset_y: f32,
    content_size: [f32; 2],
    color: [u8; 4],
}

/// Interactive geometry from a successfully built frame; shares its shaped layout.
pub(crate) struct InputHitMap {
    retained: Arc<RetainedLayout>,
    nodes: Vec<(usize, NodeSnapshot)>,
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
        }) || !contains(ClipSnapshot {
            radii: node.radii,
            bounds: node.bounds,
            radius: node.radius,
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

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ClipSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radii: Option<[[f32; 2]; 4]>,
    pub bounds: [f32; 4],
    pub radius: f32,
}
impl ClipSnapshot {
    fn rounded_box(self) -> crate::border::RoundedBox {
        let radius = self
            .radius
            .max(0.0)
            .min(self.bounds[2].min(self.bounds[3]) / 2.0);
        crate::border::RoundedBox::normalized(
            self.bounds.map(f64::from),
            self.radii
                .unwrap_or([[radius; 2]; 4])
                .map(|pair| pair.map(f64::from)),
        )
    }
    fn contains(self, point: [f32; 2]) -> bool {
        self.rounded_box().contains(point.map(f64::from))
    }
    fn intersects(self, bounds: [f32; 4]) -> bool {
        if self.radii.is_some() {
            return self.rounded_box().intersects(bounds.map(f64::from));
        }
        let [x, y, w, h] = intersect(self.bounds, bounds);
        if w <= 0.0 || h <= 0.0 {
            return false;
        }
        let [left, top, width, height] = self.bounds;
        let r = self.radius.min(width.min(height) / 2.0).max(0.0);
        let px = (left + width / 2.0).clamp(x, x + w);
        let py = (top + height / 2.0).clamp(y, y + h);
        let dx = px - px.clamp(left + r, left + width - r);
        let dy = py - py.clamp(top + r, top + height - r);
        dx * dx + dy * dy <= r * r
    }
    pub(crate) fn path(self) -> vello_cpu::kurbo::BezPath {
        if self.radii.is_some() {
            return self.rounded_box().path();
        }
        let [x, y, w, h] = self.bounds;
        RoundedRect::new(
            x as f64,
            y as f64,
            (x + w) as f64,
            (y + h) as f64,
            self.radius as f64,
        )
        .to_path(0.1)
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrollAxis {
    Horizontal,
    Vertical,
}

impl ScrollAxis {
    fn index(self) -> usize {
        match self {
            Self::Horizontal => 0,
            Self::Vertical => 1,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ScrollSnapshot {
    /// Resolved layout edges, including borders, in top/right/bottom/left order.
    #[serde(skip)]
    pub(crate) content_insets: [f32; 4],
    #[serde(skip)]
    pub(crate) content_size: [f32; 2],
    pub axis: ScrollAxis,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_reach_start: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_reach_end: Option<u64>,
    pub id: String,
    pub parent: Option<String>,
    pub offset: f32,
    pub max_offset: f32,
}

pub struct Frame {
    pub pixels: Pixmap,
    pub nodes: Vec<NodeSnapshot>,
}

fn checked_paint_transform(transform: Affine) -> Result<Affine, String> {
    let [sx, b, c, sy, _, _] = transform.as_coeffs();
    if transform
        .as_coeffs()
        .iter()
        .any(|n| !n.is_finite() || n.abs() > 1e12)
        || sx <= 0.0
        || sy <= 0.0
        || b != 0.0
        || c != 0.0
    {
        return Err("UI_ANIMATION: unsupported or singular paint transform".into());
    }
    Ok(transform)
}

#[cfg(test)]
#[path = "render_animation_tests.rs"]
mod animation_tests;

fn transformed_clip(mut clip: ClipSnapshot, transform: Affine) -> Result<ClipSnapshot, String> {
    if transform == Affine::IDENTITY {
        return Ok(clip);
    }
    let [sx, _, _, sy, tx, ty] = checked_paint_transform(transform)?.as_coeffs();
    let [x, y, w, h] = clip.bounds.map(f64::from);
    let bounds = [x * sx + tx, y * sy + ty, w * sx, h * sy];
    if bounds.iter().any(|n| !n.is_finite() || n.abs() > 1e12) {
        return Err("UI_ANIMATION: transformed clip outside bounds".into());
    }
    clip.bounds = bounds.map(|n| n as f32);
    if sx == sy {
        clip.radius *= sx as f32;
        clip.radii = clip
            .radii
            .map(|r| r.map(|pair| pair.map(|v| v * sx as f32)));
    } else {
        clip.radii = Some(
            clip.radii
                .unwrap_or([[clip.radius; 2]; 4])
                .map(|[x, y]| [(f64::from(x) * sx) as f32, (f64::from(y) * sy) as f32]),
        );
        clip.radius = 0.0;
    }
    Ok(clip)
}

fn transformed_bounds(bounds: [f32; 4], transform: Affine) -> Result<[f32; 4], String> {
    Ok(transformed_clip(
        ClipSnapshot {
            bounds,
            radius: 0.0,
            radii: None,
        },
        transform,
    )?
    .bounds)
}

#[derive(Clone, PartialEq)]
enum Paint {
    // Absolute logical-space state, retained across composition span boundaries.
    Transform(Affine),
    RasterShadow {
        raster: crate::shadow::RasterShadow,
    },
    Vector {
        path: Arc<vello_cpu::kurbo::BezPath>,
        color: [u8; 4],
        bounds: [f32; 4],
    },
    Shadow {
        shadow: crate::shadow::BoxShadow,
        color: [u8; 4],
        bounds: [f32; 4],
        radius: f32,
    },
    Gradient {
        gradient: Arc<crate::gradient::LinearGradient>,
        bounds: [f32; 4],
        radius: f32,
        corners: Option<[[f32; 2]; 4]>,
    },
    Border {
        color: [u8; 4],
        bounds: [f32; 4],
        widths: [f32; 4],
        radius: f32,
        corners: Option<[[f32; 2]; 4]>,
    },
    Area {
        points: Vec<[f32; 2]>,
        range: [f32; 3],
        color: [u8; 4],
    },
    Stroke {
        round: bool,
        color: [u8; 4],
        points: Vec<[f32; 2]>,
        width: f32,
    },
    Image {
        src: String,
        bounds: [f32; 4],
        radius: f32,
        fit: crate::image::Fit,
    },
    Clip(ClipSnapshot),
    PopClip,
    Opacity {
        // Equal alpha on adjacent siblings must still form separate groups.
        node: usize,
        alpha: f32,
    },
    PopOpacity,
    Backdrop {
        node: usize,
        clip: ClipSnapshot,
        blur: f32,
        saturation: f32,
        opacity: f32,
    },
    PopBackdrop,
    Rect {
        color: [u8; 4],
        bounds: [f32; 4],
        radius: f32,
        corners: Option<[[f32; 2]; 4]>,
    },
    Text {
        color: [u8; 4],
        font: parley::FontData,
        size: f32,
        coordinates: Vec<i16>,
        skew: f64,
        glyphs: Vec<Glyph>,
        ink: Option<[f32; 4]>,
    },
}

/// Immutable layout output shared by CPU reference rendering and GPU scene encoding.
pub struct DisplayList {
    images: HashMap<String, crate::image::Bitmap>,
    commands: Vec<Paint>,
    input_paints: Vec<(usize, Range<usize>)>,
    caret_paint: Option<usize>,
    pub nodes: Vec<NodeSnapshot>,
    pub width: u16,
    pub height: u16,
    pub scale: f32,
    pub(crate) retained: Arc<RetainedLayout>,
}

#[cfg(feature = "gpu")]
pub(crate) struct SceneKey {
    width: u16,
    height: u16,
    scale: f32,
    commands: Vec<Paint>,
    images: HashMap<String, crate::image::Bitmap>,
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
            .map(crate::image::Bitmap::byte_len)
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

/// Unscrolled geometry and shaped glyphs from one committed View render.
#[derive(Clone)]
pub(crate) struct RetainedLayout {
    nodes: Vec<NodeSnapshot>,
    commands: Vec<Paint>,
    ranges: Vec<Range<usize>>,
    parents: Vec<Option<usize>>,
    opacities: Vec<f32>,
    backdrops: Vec<Option<(f32, f32)>>,
    width: u16,
    height: u16,
    scale: f32,
    inputs: Vec<Option<InputLayout>>,
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
            let mut a = old.fields.iter().filter(|(id, _)| *id != 16);
            let mut b = new.fields.iter().filter(|(id, _)| *id != 16);
            loop {
                match (a.next(), b.next()) {
                    (None, None) => break,
                    (Some(a), Some(b))
                        if a.0 == b.0
                            && (matches!(
                                a.0,
                                8 | 17 | 18 | 26 | 27 | 40 | 46 | 56 | 107 | 108
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
                node.on_click = handle(if matches!(record.tag, 10 | 11 | 13) {
                    40
                } else {
                    8
                });
            }
            if let Some(input) = &mut node.input {
                input.on_input = handle(26);
                input.on_submit = handle(27);
            }
            if let Some(scroll) = &mut node.scroll {
                scroll.on_reach_start = handle(17);
                scroll.on_reach_end = handle(18);
            }
            if let Some(range) = &mut node.range {
                if range.on_change.is_some() {
                    range.on_change = handle(46);
                }
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
        let mut transforms: Vec<Affine> = Vec::with_capacity(self.nodes.len());
        let viewport = [
            0.0,
            0.0,
            self.width as f32 / self.scale,
            self.height as f32 / self.scale,
        ];
        let mut nodes = self.nodes.clone();
        let mut translations: Vec<[f32; 2]> = Vec::with_capacity(nodes.len());
        let mut clips = Vec::with_capacity(nodes.len());
        let mut commands = Vec::new();
        let mut rounded: Vec<Vec<ClipSnapshot>> = Vec::with_capacity(nodes.len());
        let mut snapshot_clips: Vec<Vec<ClipSnapshot>> = Vec::with_capacity(nodes.len());
        let mut active_layers: Vec<Paint> = Vec::new();
        let mut child_layers: Vec<Vec<Paint>> = Vec::with_capacity(nodes.len());
        let mut painted = Vec::new();
        let mut input_paints = Vec::new();
        for (index, node) in nodes.iter_mut().enumerate() {
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
            transforms.push(matrix);
            if animated {
                desired.push(Paint::Transform(matrix));
            }
            let clip = transformed_bounds(world_clip, matrix.inverse())?;
            node.visible_bounds = intersect(node.bounds, clip);
            let world_clips = parent.map(|p| rounded[p].clone()).unwrap_or_default();
            let mut children_clips = world_clips.clone();
            if animated {
                snapshot_clips.push(world_clips.clone());
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
            if node.scroll.is_some() {
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
            if node.scroll.is_some() {
                descendants.push(Paint::Clip(node.content_clip.unwrap_or(ClipSnapshot {
                    bounds: node.bounds,
                    radius: node.radius,
                    radii: node.radii,
                })));
            }
            child_layers.push(descendants);
            rounded.push(children_clips);
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
            translations.push(child_translation);
            clips.push(transformed_bounds(
                if node.scroll.is_some() {
                    node.content_clip
                        .map(|content| intersect(node.visible_bounds, content.bounds))
                        .unwrap_or(node.visible_bounds)
                } else {
                    clip
                },
                matrix,
            )?);
            if clip[2] == 0.0 || clip[3] == 0.0 {
                continue;
            }
            painted.clear();
            for paint in &self.commands[self.ranges[index].clone()] {
                match paint {
                    Paint::Vector {
                        path,
                        color,
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
                            painted.push(Paint::Vector {
                                path: Arc::new(
                                    Affine::translate((
                                        -f64::from(translation[0]),
                                        -f64::from(translation[1]),
                                    )) * path.as_ref(),
                                ),
                                color: *color,
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
    /// Reveal through existing scroll regions before resize can invalidate editor focus.
    pub(crate) fn reveal(
        self,
        identity: &str,
        offsets: &mut HashMap<String, f32>,
    ) -> Result<Self, String> {
        let Some(target) = self.nodes.iter().find(|node| node.identity == identity) else {
            return Ok(self);
        };
        let mut chain = Vec::new();
        let mut parent = target.scroll_parent.as_deref();
        let mut starts = [target.bounds[0], target.bounds[1]];
        let mut ends = [starts[0] + target.bounds[2], starts[1] + target.bounds[3]];
        while let Some(id) = parent {
            let Some(owner) = self
                .nodes
                .iter()
                .find(|node| node.scroll.as_ref().is_some_and(|scroll| scroll.id == id))
            else {
                return Ok(self);
            };
            let scroll = owner.scroll.as_ref().unwrap();
            let bounds = if scroll.parent.is_none() {
                owner.visible_bounds
            } else {
                owner.bounds
            };
            if bounds[2] <= 0.0 || bounds[3] <= 0.0 || chain.len() >= 128 {
                return Ok(self);
            }
            chain.push((scroll, bounds));
            parent = scroll.parent.as_deref();
        }
        let mut changes = Vec::new();
        for (scroll, bounds) in chain {
            let axis = scroll.axis.index();
            let cross = 1 - axis;
            starts[cross] = starts[cross].max(bounds[cross]);
            ends[cross] = ends[cross].min(bounds[cross] + bounds[cross + 2]);
            if ends[cross] <= starts[cross] {
                return Ok(self);
            }
            let start = bounds[axis];
            let end = start + bounds[axis + 2];
            let delta = if starts[axis] < start || ends[axis] - starts[axis] > bounds[axis + 2] {
                starts[axis] - start
            } else {
                (ends[axis] - end).max(0.0)
            };
            let next = (scroll.offset + delta).clamp(0.0, scroll.max_offset);
            let movement = next - scroll.offset;
            if movement.abs() > 0.01 {
                changes.push((scroll.id.clone(), next));
                starts[axis] -= movement;
                ends[axis] -= movement;
            }
            starts[axis] = starts[axis].max(start);
            ends[axis] = ends[axis].min(end);
            if ends[axis] <= starts[axis] {
                return Ok(self);
            }
        }
        if changes.is_empty() {
            Ok(self)
        } else {
            let mut candidate = offsets.clone();
            candidate.extend(changes);
            let display = self.retained.compose(&candidate)?;
            *offsets = candidate;
            Ok(display)
        }
    }
    #[cfg(any(target_os = "android", test))]
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
            for axis in 0..2 {
                origin[axis] -= input.scroll_offset[axis];
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
    /// Bind authorized, decoded pixels without rerunning View or changing layout.
    pub fn bind_images(
        &mut self,
        images: &HashMap<String, crate::image::Bitmap>,
    ) -> Result<(), String> {
        if self
            .image_sources()
            .collect::<std::collections::HashSet<_>>()
            .len()
            > 128
        {
            return Err("UI_IMAGE: too many visible sources".into());
        }
        let bound: HashMap<_, _> = self
            .image_sources()
            .filter_map(|src| images.get(src).map(|image| (src.to_owned(), image.clone())))
            .collect();
        if bound.len() > 128
            || bound.values().map(|image| image.byte_len()).sum::<usize>() > 64 * 1024 * 1024
        {
            return Err("UI_IMAGE: visible image budget exceeded".into());
        }
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
    pub(crate) fn bitmaps(&self) -> impl Iterator<Item = &crate::image::Bitmap> {
        self.images
            .values()
            .chain(self.commands.iter().filter_map(|paint| match paint {
                Paint::RasterShadow { raster } => Some(&raster.bitmap),
                _ => None,
            }))
    }

    /// Lower retained paint order into shared group scopes. Index zero draws the
    /// surface background; remaining span indices address paints plus one.
    pub fn composition_plan(&self) -> Result<crate::composite::Plan, String> {
        use crate::composite::{Layer, Plan, Step};
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
                    let mut clip = transformed_clip(*clip, transform)?;
                    clip.bounds = clip.bounds.map(|value| value * self.scale);
                    clip.radius *= self.scale;
                    clip.radii = clip
                        .radii
                        .map(|radii| radii.map(|pair| pair.map(|value| value * self.scale)));
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
                    if sx != sy {
                        return Err(
                            "UI_ANIMATION: nonuniform backdrop transform unsupported".into()
                        );
                    }
                    let mut clip = transformed_clip(*clip, transform)?;
                    clip.bounds = clip.bounds.map(|v| v * self.scale);
                    clip.radius *= self.scale;
                    clip.radii = clip.radii.map(|r| r.map(|p| p.map(|v| v * self.scale)));
                    groups.push(2);
                    Some(Step::Push(Layer::Backdrop {
                        clip,
                        blur: blur * self.scale * sx as f32,
                        saturation: crate::color_filter::Saturation::new(*saturation)?,
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

    fn composition_span(&self, span: Range<usize>) -> Result<(bool, Range<usize>), String> {
        if span.start > span.end || span.end > self.commands.len() + 1 {
            return Err("UI_COMPOSITE: draw span outside display list".into());
        }
        Ok((
            span.start == 0 && !span.is_empty(),
            span.start.saturating_sub(1)..span.end.saturating_sub(1),
        ))
    }

    fn transform_before(&self, index: usize) -> Affine {
        self.commands[..index]
            .iter()
            .rev()
            .find_map(|paint| match paint {
                Paint::Transform(transform) => Some(*transform),
                _ => None,
            })
            .unwrap_or(Affine::IDENTITY)
    }

    /// CPU reference execution of the multipass path; ordinary pages retain
    /// the single-context fast path until backdrop authoring is integrated.
    pub fn rasterize_composited(self) -> Result<Frame, String> {
        let plan = self.composition_plan()?;
        let mut backend = crate::composite_cpu::Cpu::new(
            |span, context: &mut RenderContext, resources: &mut Resources| {
                let (background, paints) = self.composition_span(span)?;
                if background {
                    context.set_paint(AlphaColor::<Srgb>::from_rgb8(246, 247, 251));
                    context.fill_rect(&PaintRect::new(
                        0.0,
                        0.0,
                        self.width as f64,
                        self.height as f64,
                    ));
                }
                self.paint_cpu(
                    context,
                    resources,
                    self.transform_before(paints.start),
                    self.commands[paints].iter().cloned(),
                )
            },
        );
        let pixels = plan.render(&mut backend)?;
        drop(backend);
        let pixels = Arc::try_unwrap(pixels)
            .map_err(|_| "UI_COMPOSITE: output still borrowed".to_owned())?;
        Ok(Frame {
            pixels,
            nodes: self.nodes,
        })
    }

    pub fn requires_composition(&self) -> bool {
        self.commands
            .iter()
            .any(|p| matches!(p, Paint::Backdrop { .. }))
    }

    pub fn rasterize(mut self) -> Result<Frame, String> {
        if self.requires_composition() {
            return self.rasterize_composited();
        }
        let mut context = RenderContext::new(self.width, self.height);
        let mut resources = Resources::new();
        context.set_paint(AlphaColor::<Srgb>::from_rgb8(246, 247, 251));
        context.fill_rect(&PaintRect::new(
            0.0,
            0.0,
            self.width as f64,
            self.height as f64,
        ));
        let commands = std::mem::take(&mut self.commands);
        self.paint_cpu(&mut context, &mut resources, Affine::IDENTITY, commands)?;
        context.flush();
        let mut pixels = Pixmap::new(self.width, self.height);
        context.render(&mut pixels, &mut resources);
        Ok(Frame {
            pixels,
            nodes: self.nodes,
        })
    }

    fn paint_cpu(
        &self,
        context: &mut RenderContext,
        resources: &mut Resources,
        initial_transform: Affine,
        commands: impl IntoIterator<Item = Paint>,
    ) -> Result<(), String> {
        context.set_transform(
            Affine::scale(self.scale as f64) * checked_paint_transform(initial_transform)?,
        );
        for command in commands {
            match command {
                Paint::Transform(matrix) => context.set_transform(
                    Affine::scale(self.scale as f64) * checked_paint_transform(matrix)?,
                ),
                Paint::Vector {
                    path,
                    color: [r, g, b, a],
                    bounds,
                } => {
                    context.push_clip_layer(
                        &ClipSnapshot {
                            bounds,
                            radius: 0.0,
                            radii: None,
                        }
                        .path(),
                    );
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(r, g, b, a));
                    context.fill_path(&path);
                    context.pop_layer();
                }
                Paint::RasterShadow { raster } => {
                    let (rect, transform, clip) = image_geometry(
                        &raster.bitmap,
                        raster.bounds,
                        0.0,
                        crate::image::Fit::Fill,
                    )?;
                    context.push_clip_layer(&clip);
                    context.set_paint(vello_cpu::Image {
                        image: vello_cpu::ImageSource::Pixmap(raster.bitmap.pixels().clone()),
                        sampler: Default::default(),
                    });
                    context.set_paint_transform(transform);
                    context.fill_rect(&rect);
                    context.reset_paint_transform();
                    context.pop_layer();
                }
                Paint::Shadow {
                    shadow,
                    color: [r, g, b, a],
                    bounds,
                    radius,
                } => {
                    if let Some(shape) = shadow.geometry(bounds, radius) {
                        context.push_clip_layer(&shape.knockout());
                        context.set_paint(AlphaColor::<Srgb>::from_rgba8(r, g, b, a));
                        if shape.sigma == 0.0 {
                            context.fill_path(&shape.shape().to_path(0.1));
                        } else {
                            context.fill_blurred_rounded_rect(
                                &shape.rect,
                                shape.radius,
                                shape.sigma,
                                false,
                            );
                        }
                        context.pop_layer();
                    }
                }
                Paint::Area {
                    points,
                    range,
                    color,
                } => {
                    context.set_paint(area_gradient(range, color));
                    context.fill_path(&area_path(&points));
                }
                Paint::Stroke {
                    round,
                    color: [r, g, b, a],
                    points,
                    width,
                } => {
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(r, g, b, a));
                    context.set_stroke(check_stroke(width, round));
                    context.stroke_path(&check_path(&points));
                }
                Paint::Clip(shape) => context.push_clip_layer(&shape.path()),
                Paint::PopClip | Paint::PopOpacity => context.pop_layer(),
                Paint::Opacity { alpha, .. } => context.push_opacity_layer(alpha),
                Paint::Backdrop { .. } | Paint::PopBackdrop => {
                    return Err("UI_COMPOSITE: backdrop requires composition".into());
                }
                Paint::Border {
                    corners,
                    color: [r, g, b, a],
                    bounds,
                    widths,
                    radius,
                } => {
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(r, g, b, a));
                    context.fill_path(
                        &crate::border::Border::with_radii(
                            bounds,
                            widths,
                            corners.unwrap_or([[radius; 2]; 4]),
                        )
                        .path(),
                    );
                }
                Paint::Gradient {
                    corners,
                    gradient,
                    bounds: [x, y, w, h],
                    radius,
                } => {
                    context.set_paint(gradient.brush([x, y, w, h]));
                    context.fill_path(
                        &ClipSnapshot {
                            bounds: [x, y, w, h],
                            radius,
                            radii: corners,
                        }
                        .path(),
                    );
                }
                Paint::Rect {
                    corners,
                    color: [r, g, b, a],
                    bounds: [x, y, w, h],
                    radius,
                } => {
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(r, g, b, a));
                    context.fill_path(
                        &ClipSnapshot {
                            bounds: [x, y, w, h],
                            radius,
                            radii: corners,
                        }
                        .path(),
                    );
                }
                Paint::Image {
                    src,
                    bounds,
                    radius,
                    fit,
                } => {
                    if let Some(bitmap) = self.images.get(&src) {
                        let (rect, transform, clip) = image_geometry(bitmap, bounds, radius, fit)?;
                        context.push_clip_layer(&clip);
                        context.set_paint(vello_cpu::Image {
                            image: vello_cpu::ImageSource::Pixmap(bitmap.pixels().clone()),
                            sampler: Default::default(),
                        });
                        context.set_paint_transform(transform);
                        context.fill_rect(&rect);
                        context.reset_paint_transform();
                        context.pop_layer();
                    }
                }
                Paint::Text {
                    color: [r, g, b, a],
                    font,
                    size,
                    coordinates,
                    skew,
                    glyphs,
                    ..
                } => {
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(r, g, b, a));
                    context
                        .glyph_run(resources, &font)
                        .font_size(size)
                        .glyph_transform(Affine::new([1.0, 0.0, -skew, 1.0, 0.0, 0.0]))
                        .normalized_coords(&coordinates)
                        .fill_glyphs(glyphs.into_iter())
                        .map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(())
    }

    #[cfg(feature = "gpu")]
    pub(crate) fn sparse_scene(
        &self,
        cache: &mut glifo::GlyphPrepCache,
        context: &mut vello_gpu::Scene,
        image_ids: &HashMap<usize, vello_gpu::ImageSource>,
        glyph_images: &mut HashMap<usize, Arc<vello_gpu::Pixmap>>,
    ) -> Result<(), String> {
        context.reset_and_resize(self.width, self.height);
        self.sparse_span(
            cache,
            context,
            image_ids,
            glyph_images,
            0..self.commands.len() + 1,
        )
    }

    #[cfg(feature = "gpu")]
    pub(crate) fn sparse_span(
        &self,
        cache: &mut glifo::GlyphPrepCache,
        context: &mut vello_gpu::Scene,
        image_ids: &HashMap<usize, vello_gpu::ImageSource>,
        glyph_images: &mut HashMap<usize, Arc<vello_gpu::Pixmap>>,
        span: Range<usize>,
    ) -> Result<(), String> {
        let (background, paints) = self.composition_span(span)?;
        if background {
            context.set_paint(AlphaColor::<Srgb>::from_rgb8(246, 247, 251));
            context.fill_rect(&PaintRect::new(
                0.0,
                0.0,
                self.width as f64,
                self.height as f64,
            ));
        }
        self.sparse_paints(
            cache,
            context,
            image_ids,
            glyph_images,
            self.transform_before(paints.start),
            &self.commands[paints],
        )
    }

    #[cfg(feature = "gpu")]
    #[allow(clippy::too_many_arguments)]
    fn sparse_paints(
        &self,
        cache: &mut glifo::GlyphPrepCache,
        context: &mut vello_gpu::Scene,
        image_ids: &HashMap<usize, vello_gpu::ImageSource>,
        glyph_images: &mut HashMap<usize, Arc<vello_gpu::Pixmap>>,
        initial_transform: Affine,
        commands: &[Paint],
    ) -> Result<(), String> {
        let mut transform =
            Affine::scale(self.scale as f64) * checked_paint_transform(initial_transform)?;
        context.set_transform(transform);
        for command in commands {
            match command {
                Paint::Transform(matrix) => {
                    transform =
                        Affine::scale(self.scale as f64) * checked_paint_transform(*matrix)?;
                    context.set_transform(transform);
                }
                Paint::Vector {
                    path,
                    color: [r, g, b, a],
                    bounds,
                } => {
                    context.push_clip_layer(
                        &ClipSnapshot {
                            bounds: *bounds,
                            radius: 0.0,
                            radii: None,
                        }
                        .path(),
                    );
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(*r, *g, *b, *a));
                    context.fill_path(path);
                    context.pop_layer();
                }
                Paint::RasterShadow { raster } => {
                    let (rect, transform, clip) = image_geometry(
                        &raster.bitmap,
                        raster.bounds,
                        0.0,
                        crate::image::Fit::Fill,
                    )?;
                    let id = image_ids
                        .get(&(Arc::as_ptr(raster.bitmap.pixels()) as usize))
                        .ok_or("UI_SHADOW: texture not uploaded")?;
                    context.push_clip_path(&clip);
                    context.set_paint(vello_gpu::Image {
                        image: id.clone(),
                        sampler: Default::default(),
                    });
                    context.set_paint_transform(transform);
                    context.fill_rect(&rect);
                    context.reset_paint_transform();
                    context.pop_clip();
                }
                Paint::Shadow {
                    shadow,
                    color: [r, g, b, a],
                    bounds,
                    radius,
                } => {
                    if let Some(shape) = shadow.geometry(*bounds, *radius) {
                        context.push_clip_layer(&shape.knockout());
                        context.set_paint(AlphaColor::<Srgb>::from_rgba8(*r, *g, *b, *a));
                        if shape.sigma == 0.0 {
                            context.fill_path(&shape.shape().to_path(0.1));
                        } else {
                            context.fill_blurred_rounded_rect(
                                &shape.rect,
                                shape.radius,
                                shape.sigma,
                                false,
                            );
                        }
                        context.pop_layer();
                    }
                }
                Paint::Area {
                    points,
                    range,
                    color,
                } => {
                    context.set_paint(area_gradient(*range, *color));
                    context.fill_path(&area_path(points));
                }
                Paint::Stroke {
                    round,
                    color: [r, g, b, a],
                    points,
                    width,
                } => {
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(*r, *g, *b, *a));
                    context.set_stroke(check_stroke(*width, *round));
                    context.stroke_path(&check_path(points));
                }
                Paint::Clip(shape) => {
                    if shape.radius == 0.0 && shape.radii.is_none() {
                        let [x, y, w, h] = shape.bounds;
                        context.push_clip_rect(&PaintRect::new(
                            x as f64,
                            y as f64,
                            (x + w) as f64,
                            (y + h) as f64,
                        ));
                    } else {
                        context.push_clip_path(&shape.path());
                    }
                }
                Paint::PopClip => context.pop_clip(),
                Paint::PopOpacity => context.pop_layer(),
                Paint::Opacity { alpha, .. } => context.push_opacity_layer(*alpha),
                Paint::Backdrop { .. } | Paint::PopBackdrop => {
                    return Err("UI_COMPOSITE: backdrop requires composition".into());
                }
                Paint::Border {
                    corners,
                    color: [r, g, b, a],
                    bounds,
                    widths,
                    radius,
                } => {
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(*r, *g, *b, *a));
                    context.fill_path(
                        &crate::border::Border::with_radii(
                            *bounds,
                            *widths,
                            corners.unwrap_or([[*radius; 2]; 4]),
                        )
                        .path(),
                    );
                }
                Paint::Gradient {
                    corners,
                    gradient,
                    bounds: [x, y, w, h],
                    radius,
                } => {
                    context.set_paint(gradient.brush([*x, *y, *w, *h]));
                    context.fill_path(
                        &ClipSnapshot {
                            bounds: [*x, *y, *w, *h],
                            radius: *radius,
                            radii: *corners,
                        }
                        .path(),
                    );
                }
                Paint::Rect {
                    corners,
                    color: [r, g, b, a],
                    bounds: [x, y, w, h],
                    radius,
                } => {
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(*r, *g, *b, *a));
                    context.fill_path(
                        &ClipSnapshot {
                            bounds: [*x, *y, *w, *h],
                            radius: *radius,
                            radii: *corners,
                        }
                        .path(),
                    );
                }
                Paint::Image {
                    src,
                    bounds,
                    radius,
                    fit,
                } => {
                    if let Some(bitmap) = self.images.get(src) {
                        let (rect, transform, clip) =
                            image_geometry(bitmap, *bounds, *radius, *fit)?;
                        let id = image_ids
                            .get(&(Arc::as_ptr(bitmap.pixels()) as usize))
                            .ok_or("UI_IMAGE: texture not uploaded")?;
                        context.push_clip_path(&clip);
                        context.set_paint(vello_gpu::Image {
                            image: id.clone(),
                            sampler: Default::default(),
                        });
                        context.set_paint_transform(transform);
                        context.fill_rect(&rect);
                        context.reset_paint_transform();
                        context.pop_clip();
                    }
                }
                Paint::Text {
                    color: [r, g, b, a],
                    font,
                    size,
                    coordinates,
                    skew,
                    glyphs,
                    ..
                } => {
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(*r, *g, *b, *a));
                    glifo::GlyphRunBuilder::new(
                        font.clone(),
                        transform,
                        Affine::IDENTITY,
                        crate::gpu_text::Backend {
                            sink: crate::gpu_text::GlyphSink {
                                scene: context,
                                images: glyph_images,
                            },
                            cache,
                        },
                    )
                    .font_size(*size)
                    .glyph_transform(Affine::new([1.0, 0.0, -*skew, 1.0, 0.0, 0.0]))
                    .normalized_coords(coordinates)
                    .fill_glyphs(glyphs.iter().copied())
                    .map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(())
    }

    #[cfg(feature = "gpu")]
    pub fn scene(&self) -> Result<vello::Scene, String> {
        use vello::peniko::{
            Fill, color::AlphaColor as Color, color::Srgb as Space, kurbo::Affine,
        };
        let mut scene = vello::Scene::new();
        let mut transform = Affine::scale(self.scale as f64);
        for command in &self.commands {
            match command {
                Paint::Transform(matrix) => {
                    transform =
                        Affine::scale(self.scale as f64) * checked_paint_transform(*matrix)?;
                }
                Paint::Vector {
                    path,
                    color: [r, g, b, a],
                    bounds,
                } => {
                    scene.push_clip_layer(
                        Fill::NonZero,
                        transform,
                        &ClipSnapshot {
                            bounds: *bounds,
                            radius: 0.0,
                            radii: None,
                        }
                        .path(),
                    );
                    scene.fill(
                        Fill::NonZero,
                        transform,
                        Color::<Space>::from_rgba8(*r, *g, *b, *a),
                        None,
                        path.as_ref(),
                    );
                    scene.pop_layer();
                }
                Paint::RasterShadow { raster } => {
                    let Ok((_, placed, clip)) =
                        image_geometry(&raster.bitmap, raster.bounds, 0.0, crate::image::Fit::Fill)
                    else {
                        continue;
                    };
                    let pixels = raster.bitmap.pixels();
                    let image = vello::peniko::ImageData {
                        data: vello::peniko::Blob::new(Arc::new(
                            pixels.data_as_u8_slice().to_vec(),
                        )),
                        format: vello::peniko::ImageFormat::Rgba8,
                        alpha_type: vello::peniko::ImageAlphaType::AlphaPremultiplied,
                        width: pixels.width().into(),
                        height: pixels.height().into(),
                    };
                    scene.push_clip_layer(Fill::NonZero, transform, &clip);
                    scene.draw_image(
                        vello::peniko::ImageBrush::new(image).as_ref(),
                        transform * placed,
                    );
                    scene.pop_layer();
                }
                Paint::Shadow {
                    shadow,
                    color: [r, g, b, a],
                    bounds,
                    radius,
                } => {
                    if let Some(shape) = shadow.geometry(*bounds, *radius) {
                        scene.push_layer(
                            Fill::NonZero,
                            vello::peniko::BlendMode::default(),
                            1.0,
                            transform,
                            &shape.knockout(),
                        );
                        let color = Color::<Space>::from_rgba8(*r, *g, *b, *a);
                        if shape.sigma == 0.0 {
                            scene.fill(Fill::NonZero, transform, color, None, &shape.shape());
                        } else {
                            scene.draw_blurred_rounded_rect(
                                transform,
                                shape.rect,
                                color,
                                shape.radius as f64,
                                shape.sigma as f64,
                            );
                        }
                        scene.pop_layer();
                    }
                }
                Paint::Area {
                    points,
                    range,
                    color,
                } => {
                    scene.fill(
                        Fill::NonZero,
                        transform,
                        &area_gradient(*range, *color),
                        None,
                        &area_path(points),
                    );
                }
                Paint::Stroke {
                    round,
                    color: [r, g, b, a],
                    points,
                    width,
                } => {
                    scene.stroke(
                        &check_stroke(*width, *round),
                        transform,
                        Color::<Space>::from_rgba8(*r, *g, *b, *a),
                        None,
                        &check_path(points),
                    );
                }
                Paint::Clip(shape) => {
                    scene.push_clip_layer(Fill::NonZero, transform, &shape.path())
                }
                Paint::PopClip | Paint::PopOpacity => scene.pop_layer(),
                Paint::Backdrop { .. } | Paint::PopBackdrop => {
                    return Err("UI_COMPOSITE: backdrop requires composition".into());
                }
                Paint::Opacity { alpha, .. } => scene.push_layer(
                    Fill::NonZero,
                    vello::peniko::BlendMode::default(),
                    *alpha,
                    transform,
                    &PaintRect::new(
                        0.0,
                        0.0,
                        self.width as f64 / self.scale as f64,
                        self.height as f64 / self.scale as f64,
                    ),
                ),
                Paint::Border {
                    corners,
                    color: [r, g, b, a],
                    bounds,
                    widths,
                    radius,
                } => {
                    scene.fill(
                        Fill::NonZero,
                        transform,
                        Color::<Space>::from_rgba8(*r, *g, *b, *a),
                        None,
                        &crate::border::Border::with_radii(
                            *bounds,
                            *widths,
                            corners.unwrap_or([[*radius; 2]; 4]),
                        )
                        .path(),
                    );
                }
                Paint::Gradient {
                    corners,
                    gradient,
                    bounds: [x, y, w, h],
                    radius,
                } => {
                    scene.fill(
                        Fill::NonZero,
                        transform,
                        &gradient.brush([*x, *y, *w, *h]),
                        None,
                        &ClipSnapshot {
                            bounds: [*x, *y, *w, *h],
                            radius: *radius,
                            radii: *corners,
                        }
                        .path(),
                    );
                }
                Paint::Rect {
                    corners,
                    color: [r, g, b, a],
                    bounds: [x, y, w, h],
                    radius,
                } => {
                    scene.fill(
                        Fill::NonZero,
                        transform,
                        Color::<Space>::from_rgba8(*r, *g, *b, *a),
                        None,
                        &ClipSnapshot {
                            bounds: [*x, *y, *w, *h],
                            radius: *radius,
                            radii: *corners,
                        }
                        .path(),
                    );
                }
                Paint::Image {
                    src,
                    bounds,
                    radius,
                    fit,
                } => {
                    if let Some(bitmap) = self.images.get(src) {
                        let Ok((_, placed, clip)) = image_geometry(bitmap, *bounds, *radius, *fit)
                        else {
                            continue;
                        };
                        let pixels = bitmap.pixels();
                        let image = vello::peniko::ImageData {
                            data: vello::peniko::Blob::new(Arc::new(
                                pixels.data_as_u8_slice().to_vec(),
                            )),
                            format: vello::peniko::ImageFormat::Rgba8,
                            alpha_type: vello::peniko::ImageAlphaType::AlphaPremultiplied,
                            width: pixels.width().into(),
                            height: pixels.height().into(),
                        };
                        scene.push_clip_layer(Fill::NonZero, transform, &clip);
                        scene.draw_image(
                            vello::peniko::ImageBrush::new(image).as_ref(),
                            transform * placed,
                        );
                        scene.pop_layer();
                    }
                }
                Paint::Text {
                    color: [r, g, b, a],
                    font,
                    size,
                    coordinates,
                    skew,
                    glyphs,
                    ..
                } => {
                    scene
                        .draw_glyphs(font)
                        .font_size(*size)
                        .glyph_transform(Some(Affine::new([1.0, 0.0, -*skew, 1.0, 0.0, 0.0])))
                        .transform(transform)
                        .normalized_coords(coordinates)
                        .brush(Color::<Space>::from_rgba8(*r, *g, *b, *a))
                        .draw(
                            Fill::NonZero,
                            glyphs.iter().map(|g| vello::Glyph {
                                id: g.id,
                                x: g.x,
                                y: g.y,
                            }),
                        );
                }
            }
        }
        Ok(scene)
    }
}

pub struct Renderer {
    pub(crate) text_scale: crate::text_scale::TextScale,
    fonts: FontContext,
    text: LayoutContext<()>,
    text_cache: crate::text_cache::TextCache,
    shadow_cache: crate::shadow::MaskCache,
    pub(crate) text_time: std::time::Duration,
}

#[cfg(test)]
#[path = "text_scale_layout_tests.rs"]
mod text_scale_layout_tests;

impl Default for Renderer {
    fn default() -> Self {
        let fonts = FontContext::new();
        #[cfg(target_os = "android")]
        let fonts = {
            let mut fonts = fonts;
            // Fontique does not yet resolve Android's fonts.xml family aliases.
            // Preserve an existing generic mapping; otherwise use a scanned mono face.
            if fonts
                .collection
                .generic_families(GenericFamily::Monospace)
                .next()
                .is_none()
            {
                for name in ["Droid Sans Mono", "Noto Sans Mono", "Roboto Mono"] {
                    if let Some(family) = fonts.collection.family_by_name(name) {
                        fonts.collection.set_generic_families(
                            GenericFamily::Monospace,
                            core::iter::once(family.id()),
                        );
                        break;
                    }
                }
            }
            fonts
        };
        Self {
            text_scale: Default::default(),
            fonts,
            text: LayoutContext::new(),
            text_cache: Default::default(),
            shadow_cache: Default::default(),
            text_time: std::time::Duration::ZERO,
        }
    }
}

impl Renderer {
    pub(crate) fn clear_text_cache(&mut self) {
        self.text_cache.clear();
        self.shadow_cache.clear();
    }
    pub(crate) fn edit_text(
        &mut self,
        editor: &mut crate::text_input::Editor,
        edits: &[crate::text_input::Edit],
    ) -> Result<crate::text_input::Change, String> {
        editor
            .apply_batch(edits, &mut self.fonts, &mut self.text)
            .map_err(|e| format!("UI_INPUT: {e:?}"))
    }

    pub fn render(
        &mut self,
        records: &[Record],
        width: u16,
        height: u16,
        scale: f32,
    ) -> Result<Frame, String> {
        self.layout(records, width, height, scale)?.rasterize()
    }

    pub fn layout(
        &mut self,
        records: &[Record],
        width: u16,
        height: u16,
        scale: f32,
    ) -> Result<DisplayList, String> {
        self.layout_scrolled(records, width, height, scale, &HashMap::new())
    }

    /// Offsets belong to the native presenter; scrolling does not require a View render call.
    pub fn layout_scrolled(
        &mut self,
        records: &[Record],
        width: u16,
        height: u16,
        scale: f32,
        offsets: &HashMap<String, f32>,
    ) -> Result<DisplayList, String> {
        if records
            .iter()
            .any(|record| record.fields.iter().any(|(field, _)| *field == 101))
        {
            return Err("UI_ANIMATION: page requires animation presentation scheduling".into());
        }
        self.text_time = std::time::Duration::ZERO;
        if offsets.values().any(|value| !value.is_finite()) {
            return Err("UI_SCROLL: non-finite offset".into());
        }
        if width == 0
            || height == 0
            || width > 4096
            || height > 4096
            || !scale.is_finite()
            || scale <= 0.0
        {
            return Err("UI_VIEWPORT: invalid dimensions".into());
        }
        let mut overflow_wraps: Vec<u8> = Vec::with_capacity(records.len());
        let mut word_breaks: Vec<u8> = Vec::with_capacity(records.len());
        let mut strike_owners: Vec<Option<usize>> = Vec::with_capacity(records.len());
        let mut text_alignments: Vec<Alignment> = Vec::with_capacity(records.len());
        let mut button_owners: Vec<Option<usize>> = Vec::with_capacity(records.len());
        let mut button_hashes: HashMap<usize, std::collections::hash_map::DefaultHasher> =
            HashMap::new();
        for record in records {
            let owner = record.parent.and_then(|parent| {
                if matches!(records[parent].tag, 3 | 10) {
                    Some(parent)
                } else {
                    button_owners[parent]
                }
            });
            button_owners.push(owner);
            if let Some(owner) = owner {
                // A gesture cannot survive a changed child label or image merely
                // because the outer button's identity and bounds stayed the same.
                let hash = button_hashes.entry(owner).or_default();
                record.tag.hash(hash);
                record.key.hash(hash);
                record.text.hash(hash);
                record.parent.hash(hash);
                serde_json::to_string(&record.fields)
                    .map_err(|error| error.to_string())?
                    .hash(hash);
            }
        }
        let mut tree: TaffyTree<usize> = TaffyTree::new();
        let mut ids = Vec::new();
        let mut layouts: Vec<Option<Layout<()>>> = Vec::new();
        let mut font_sizes = Vec::new();
        let mut font_families = Vec::new();
        let mut font_italics = Vec::new();
        let mut font_weights = Vec::new();
        let mut line_heights = Vec::new();
        let mut letter_spacings = Vec::new();
        let mut colors: Vec<String> = Vec::new();
        let texts: Vec<String> = records
            .iter()
            .map(|record| {
                if !matches!(record.tag, 8 | 9 | 14) {
                    return record.text.clone();
                }
                let value = record.string(21).unwrap_or("");
                let text = if value.is_empty() {
                    record.string(22).unwrap_or("").to_owned()
                } else if record.number(23) == Some(1.0) {
                    "\u{2022}".repeat(value.chars().count())
                } else {
                    value.to_owned()
                };
                if matches!(record.tag, 8 | 14) {
                    text.replace(['\r', '\n', '\u{2028}', '\u{2029}'], " ")
                } else {
                    text
                }
            })
            .collect();
        for (index, record) in records.iter().enumerate() {
            let font_size = record
                .number(4)
                .unwrap_or_else(|| record.parent.map(|p| font_sizes[p]).unwrap_or(14.0));
            font_sizes.push(font_size);
            // Decorations propagate through layout descendants; `none` only
            // removes a node's own decoration, not one drawn by its ancestor.
            strike_owners.push(
                record
                    .parent
                    .and_then(|p| strike_owners[p])
                    .or_else(|| (record.string(81) == Some("line-through")).then_some(index)),
            );
            text_alignments.push(match record.string(71) {
                Some("left") => Alignment::Left,
                Some("right") => Alignment::Right,
                Some("center") => Alignment::Center,
                Some("justify") => Alignment::Justify,
                Some("end") => Alignment::End,
                Some("start") => Alignment::Start,
                _ if record.tag == 3 => Alignment::Center,
                _ => record
                    .parent
                    .map(|parent| text_alignments[parent])
                    .unwrap_or(Alignment::Start),
            });
            let word_break = match record.string(63) {
                Some("break-all") => 1,
                Some("keep-all") => 2,
                Some("normal") => 0,
                _ => record.parent.map(|parent| word_breaks[parent]).unwrap_or(0),
            };
            word_breaks.push(word_break);
            let overflow_wrap = match record.string(64) {
                Some("break-word") => 1,
                Some("anywhere") => 2,
                Some("normal") => 0,
                _ => record
                    .parent
                    .map(|parent| overflow_wraps[parent])
                    .unwrap_or(0),
            };
            overflow_wraps.push(overflow_wrap);
            let family = record
                .string(38)
                .unwrap_or_else(|| record.parent.map(|p| font_families[p]).unwrap_or("sans"));
            font_families.push(family);
            let italic = record
                .string(84)
                .map(|style| style == "italic")
                .unwrap_or_else(|| record.parent.map(|p| font_italics[p]).unwrap_or(false));
            font_italics.push(italic);
            let weight = record
                .number(36)
                .unwrap_or_else(|| record.parent.map(|p| font_weights[p]).unwrap_or(400.0));
            font_weights.push(weight);
            let line_height = record
                .number(37)
                .or_else(|| record.parent.and_then(|p| line_heights[p]));
            line_heights.push(line_height);
            let letter_spacing = record
                .number(89)
                .unwrap_or_else(|| record.parent.map(|p| letter_spacings[p]).unwrap_or(0.0));
            letter_spacings.push(letter_spacing);
            // Keep inherited values in author units; scaling inherited output again
            // would compound with every nested layout container.
            let scaled_font_size = self.text_scale.map(font_size);
            let text_ratio = if font_size > 0.0 {
                scaled_font_size / font_size
            } else {
                1.0
            };
            let line_height = line_height.map(|height| height * text_ratio);
            let letter_spacing = letter_spacing * text_ratio;
            let font_size = scaled_font_size;
            let color_name = record.string(5).map(str::to_owned).unwrap_or_else(|| {
                if record.tag == 3 {
                    "primaryText".into()
                } else {
                    record
                        .parent
                        .map(|p| colors[p].clone())
                        .unwrap_or("text".into())
                }
            });
            colors.push(color_name);
            let text = if matches!(record.tag, 2 | 3 | 8..=11 | 13 | 14) {
                let started = std::time::Instant::now();
                let key = (!matches!(record.tag, 8 | 9 | 14))
                    .then(|| {
                        crate::text_cache::Key::new(
                            &texts[index],
                            (font_size, letter_spacing),
                            weight,
                            line_height,
                            (family == "mono", italic),
                            word_break,
                            overflow_wrap,
                        )
                    })
                    .flatten();
                let cached = key.as_ref().and_then(|key| self.text_cache.get(key));
                let mut layout = if let Some(layout) = cached {
                    layout
                } else {
                    let mut builder =
                        self.text
                            .ranged_builder(&mut self.fonts, &texts[index], 1.0, true);
                    builder.push_default(StyleProperty::FontSize(font_size));
                    builder.push_default(StyleProperty::LetterSpacing(letter_spacing));
                    builder.push_default(StyleProperty::FontStyle(if italic {
                        FontStyle::Italic
                    } else {
                        FontStyle::Normal
                    }));
                    if matches!(record.tag, 8 | 14) {
                        builder.push_default(StyleProperty::TextWrapMode(
                            parley::TextWrapMode::NoWrap,
                        ));
                    }
                    builder.push_default(StyleProperty::OverflowWrap(match overflow_wrap {
                        1 => parley::OverflowWrap::BreakWord,
                        2 => parley::OverflowWrap::Anywhere,
                        _ => parley::OverflowWrap::Normal,
                    }));
                    builder.push_default(StyleProperty::WordBreak(match word_break {
                        1 => parley::WordBreak::BreakAll,
                        2 => parley::WordBreak::KeepAll,
                        _ => parley::WordBreak::Normal,
                    }));
                    builder.push_default(StyleProperty::FontWeight(FontWeight::new(weight)));
                    builder.push_default(if family == "mono" {
                        GenericFamily::Monospace
                    } else {
                        GenericFamily::SansSerif
                    });
                    if let Some(height) = line_height {
                        builder
                            .push_default(StyleProperty::LineHeight(LineHeight::Absolute(height)));
                    }
                    let layout = builder.build(&texts[index]);
                    if let Some(key) = key {
                        self.text_cache.insert(key, &layout);
                    }
                    layout
                };
                layout.break_all_lines(None);
                self.text_time += started.elapsed();
                Some(layout)
            } else {
                None
            };
            layouts.push(text);
            let padding =
                record
                    .number(3)
                    .unwrap_or(if matches!(record.tag, 3 | 8..=11 | 13..=15) {
                        12.0
                    } else {
                        0.0
                    });
            let [padding_top, padding_right, padding_bottom, padding_left] =
                [59, 60, 61, 62].map(|field| record.number(field).unwrap_or(padding));
            let border = record.number(65).unwrap_or(0.0);
            let [border_top, border_right, border_bottom, border_left] =
                [67, 68, 69, 70].map(|id| record.number(id).unwrap_or(border));
            let style = Style {
                display: if matches!(record.tag, 16 | 17) {
                    Display::Grid
                } else {
                    Display::Flex
                },
                grid_column: if record.parent.is_some_and(|p| records[p].tag == 17) {
                    Line {
                        start: line(1),
                        end: line(2),
                    }
                } else {
                    Default::default()
                },
                grid_row: if record.parent.is_some_and(|p| records[p].tag == 17) {
                    Line {
                        start: line(1),
                        end: line(2),
                    }
                } else {
                    Default::default()
                },
                inset: Rect {
                    left: record.number(48).map(length).unwrap_or(auto()),
                    top: record.number(49).map(length).unwrap_or(auto()),
                    right: auto(),
                    bottom: auto(),
                },
                grid_template_columns: if record.tag == 16 {
                    vec![flex(1.0); record.number(47).unwrap_or(1.0) as usize]
                } else {
                    Vec::new()
                },
                flex_direction: match record.string(103) {
                    Some("row") => FlexDirection::Row,
                    Some("row-reverse") => FlexDirection::RowReverse,
                    Some("column") => FlexDirection::Column,
                    Some("column-reverse") => FlexDirection::ColumnReverse,
                    _ if record.tag == 1 => FlexDirection::Row,
                    _ => FlexDirection::Column,
                },
                flex_wrap: match record.string(87) {
                    Some("wrap") => FlexWrap::Wrap,
                    Some("wrap-reverse") => FlexWrap::WrapReverse,
                    _ => FlexWrap::NoWrap,
                },
                flex_grow: record.number(7).unwrap_or(0.0),
                flex_basis: match record
                    .fields
                    .iter()
                    .find(|(id, _)| *id == 99)
                    .map(|(_, value)| crate::schema::flex_basis(value))
                    .transpose()?
                    .flatten()
                {
                    Some((value, true)) => percent(value / 100.0),
                    Some((value, false)) => length(value),
                    None => auto(),
                },
                flex_shrink: record.number(32).unwrap_or(
                    if (matches!(record.tag, 5 | 6) || record.number(7).unwrap_or(0.0) > 0.0)
                        && dimension::<Dimension>(record, 1).is_none()
                    {
                        1.0
                    } else {
                        0.0
                    },
                ),
                align_items: record.string(33).map(cross_alignment),
                align_self: record.string(35).map(cross_alignment),
                justify_content: record.string(34).map(|value| match value {
                    "center" => JustifyContent::SAFE_CENTER,
                    "end" => JustifyContent::SAFE_FLEX_END,
                    "between" => JustifyContent::SPACE_BETWEEN,
                    _ => JustifyContent::FLEX_START,
                }),
                overflow: Point {
                    x: if record.string(100) == Some("horizontal") {
                        Overflow::Scroll
                    } else if matches!(record.tag, 5 | 6) {
                        Overflow::Hidden
                    } else {
                        Overflow::Visible
                    },
                    y: if record.string(100) == Some("horizontal") {
                        Overflow::Hidden
                    } else if matches!(record.tag, 5 | 6) {
                        Overflow::Scroll
                    } else {
                        Overflow::Visible
                    },
                },
                min_size: Size {
                    width: dimension(record, 28).unwrap_or(if record.tag == 15 {
                        length(120.0)
                    } else if matches!(record.tag, 10 | 11 | 13) {
                        length(
                            check_reserve(record.tag)
                                + padding_left
                                + padding_right
                                + border_left
                                + border_right,
                        )
                    } else if matches!(record.tag, 5 | 6) || record.number(7).unwrap_or(0.0) > 0.0 {
                        length(0.0)
                    } else {
                        auto()
                    }),
                    height: dimension(record, 29).unwrap_or(
                        if matches!(record.tag, 8 | 14)
                            && dimension::<taffy::Dimension>(record, 1).is_none()
                            && record.number(82) != Some(1.0)
                        {
                            length(48.0)
                        } else if matches!(record.tag, 10 | 11 | 13 | 14 | 15) {
                            length(44.0)
                        } else if matches!(record.tag, 5 | 6)
                            || record.number(7).unwrap_or(0.0) > 0.0
                        {
                            length(0.0)
                        } else {
                            auto()
                        },
                    ),
                },
                max_size: Size {
                    width: dimension(record, 30).unwrap_or(auto()),
                    height: dimension(record, 31).unwrap_or(auto()),
                },
                size: Size {
                    width: dimension(record, 0).unwrap_or(if index == 0 {
                        length(width as f32 / scale)
                    } else if record.tag == 19 {
                        length(24.0)
                    } else if record.tag == 7 {
                        length(64.0)
                    } else {
                        auto()
                    }),
                    height: dimension(record, 1).unwrap_or(
                        if record.number(82) == Some(1.0) || matches!(record.tag, 8 | 14) {
                            auto()
                        } else if record.tag == 15 {
                            length(48.0)
                        } else if record.tag == 9 {
                            length(120.0)
                        } else if index == 0 {
                            length(height as f32 / scale)
                        } else if record.tag == 19 {
                            length(24.0)
                        } else if record.tag == 7 {
                            length(64.0)
                        } else {
                            auto()
                        },
                    ),
                },
                gap: Size {
                    width: length(if record.tag == 6 {
                        0.0
                    } else {
                        record.number(2).unwrap_or(0.0)
                    }),
                    height: length(if record.tag == 6 {
                        0.0
                    } else {
                        record.number(2).unwrap_or(0.0)
                    }),
                },
                border: Rect {
                    left: length(border_left),
                    right: length(border_right),
                    top: length(border_top),
                    bottom: length(border_bottom),
                },
                padding: Rect {
                    left: length(padding_left + if record.tag == 14 { 32.0 } else { 0.0 }),
                    right: length(padding_right),
                    top: length(padding_top),
                    bottom: length(padding_bottom),
                },
                margin: Rect {
                    bottom: length(
                        record
                            .parent
                            .filter(|p| records[*p].tag == 6)
                            .filter(|p| {
                                record.number(15).unwrap_or(0.0) + 1.0
                                    < records[*p].number(12).unwrap_or(0.0)
                            })
                            .and_then(|p| records[p].number(2))
                            .unwrap_or(0.0),
                    ),
                    left: length(0.0),
                    right: length(0.0),
                    top: length(0.0),
                },
                ..Default::default()
            };
            let id = tree
                .new_leaf_with_context(style, index)
                .map_err(|e| e.to_string())?;
            if let Some(parent) = record.parent {
                tree.add_child(ids[parent], id).map_err(|e| e.to_string())?;
            }
            if record.tag == 6 {
                // Zero-width spacers do not contribute to Taffy's scroll overflow.
                let spacer = tree
                    .new_leaf(Style {
                        size: Size {
                            width: Dimension::AUTO,
                            height: length(record.number(13).unwrap_or(0.0)),
                        },
                        flex_shrink: 0.0,
                        ..Default::default()
                    })
                    .map_err(|e| e.to_string())?;
                tree.add_child(id, spacer).map_err(|e| e.to_string())?;
            }
            ids.push(id);
        }
        for (index, record) in records.iter().enumerate().filter(|(_, r)| r.tag == 6) {
            let spacer = tree
                .new_leaf(Style {
                    size: Size {
                        width: Dimension::AUTO,
                        height: length(record.number(14).unwrap_or(0.0)),
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                })
                .map_err(|e| e.to_string())?;
            tree.add_child(ids[index], spacer)
                .map_err(|e| e.to_string())?;
        }
        let root = *ids.first().ok_or("UI_ROOT: missing")?;
        let needs_baselines = records.iter().any(|record| {
            record.string(33) == Some("baseline") || record.string(35) == Some("baseline")
        });
        tree.compute_layout_with_measure(
            root,
            Size {
                width: AvailableSpace::Definite(width as f32 / scale),
                height: AvailableSpace::Definite(height as f32 / scale),
            },
            |inputs, _, context, style| {
                let index = context.as_ref().map(|index| **index);
                let mut output = taffy::compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, available| {
                        let tag = context.as_ref().map(|i| records[**i].tag).unwrap_or(0);
                        let single_line = matches!(tag, 8 | 14);
                        let limit = context
                            .as_ref()
                            .and_then(|i| records[**i].number(50))
                            .map(|n| n as usize);
                        let Some(layout) = context.and_then(|i| layouts[*i].as_mut()) else {
                            return Size::ZERO;
                        };
                        // Taffy supplies border-box known dimensions but content-box
                        // available space. Wrapping must match the painted content box.
                        let max_width = match available.width {
                            AvailableSpace::Definite(w) => Some(w),
                            _ => None,
                        };
                        let started = std::time::Instant::now();
                        layout.break_all_lines(if single_line {
                            None
                        } else {
                            max_width.map(|w| (w - check_reserve(tag)).max(0.0))
                        });
                        self.text_time += started.elapsed();
                        let (width, height) = text_extent(layout, limit);
                        Size {
                            width: known.width.unwrap_or(width.ceil() + check_reserve(tag)),
                            height: known.height.unwrap_or(height.ceil().max(check_height(tag))),
                        }
                    },
                );
                if let Some(index) = index.filter(|_| needs_baselines) {
                    if let Some(text) = layouts[index].as_mut() {
                        let record = &records[index];
                        let padding = style
                            .padding
                            .resolve_or_zero(inputs.parent_size.width, &|_, _| 0.0);
                        let border = style
                            .border
                            .resolve_or_zero(inputs.parent_size.width, &|_, _| 0.0);
                        let top = padding.top + border.top;
                        let bottom = padding.bottom + border.bottom;
                        // Known dimensions can bypass measurement. Reflow against
                        // the final content width so the reported baseline matches paint.
                        let started = std::time::Instant::now();
                        text.break_all_lines(if matches!(record.tag, 8 | 14) {
                            None
                        } else {
                            Some(
                                (output.size.width
                                    - padding.left
                                    - padding.right
                                    - border.left
                                    - border.right
                                    - check_reserve(record.tag))
                                .max(0.0),
                            )
                        });
                        self.text_time += started.elapsed();
                        let label_y = if matches!(record.tag, 3 | 8 | 10 | 11 | 13 | 14) {
                            ((output.size.height - top - bottom - text.height()) / 2.0).max(0.0)
                        } else {
                            0.0
                        };
                        output.baselines.first = text
                            .lines()
                            .next()
                            .map(|line| top + label_y + line.metrics().baseline);
                    }
                }
                output
            },
        )
        .map_err(|e| e.to_string())?;
        let mut commands = Vec::new();
        let mut vector_work = 0;
        let mut shadow_masks = std::collections::HashSet::new();
        let mut shadow_bytes = 0;
        let mut origins: Vec<(f32, f32)> = Vec::new();
        let mut nodes = Vec::new();
        let viewport = [0.0, 0.0, width as f32 / scale, height as f32 / scale];
        let mut ranges = Vec::new();
        let mut input_layouts = Vec::new();
        let identities = crate::schema::identities(records);
        let mut scroll_parents: Vec<Option<String>> = Vec::new();
        for (index, record) in records.iter().enumerate() {
            let mut layout = *tree.layout(ids[index]).map_err(|e| e.to_string())?;
            layout.padding.left += layout.border.left;
            layout.padding.right += layout.border.right;
            layout.padding.top += layout.border.top;
            layout.padding.bottom += layout.border.bottom;
            let (px, py) = record.parent.map(|p| origins[p]).unwrap_or((0.0, 0.0));
            let x = px + layout.location.x;
            let y = py + layout.location.y;
            let identity = identities[index].clone();
            let bounds = [x, y, layout.size.width, layout.size.height];
            // Nested percentages can multiply otherwise bounded authored lengths.
            if [
                x,
                y,
                x + layout.size.width,
                y + layout.size.height,
                layout.size.width,
                layout.size.height,
            ]
            .iter()
            .any(|value| {
                !value.is_finite()
                    || value.abs() > 1e12
                    || !(value * scale).is_finite()
                    || (value * scale).abs() > 1e12
            }) {
                return Err("UI_LIMIT: layout geometry exceeds coordinate budget".into());
            }

            let border_widths = [
                layout.border.top,
                layout.border.right,
                layout.border.bottom,
                layout.border.left,
            ];
            let default_radius =
                record
                    .number(10)
                    .unwrap_or(if matches!(record.tag, 3 | 8 | 9 | 14) {
                        12.0
                    } else {
                        0.0
                    });
            let authored = [90, 91, 92, 93].map(|id| record.number(id).unwrap_or(default_radius));
            let geometry = if authored.iter().all(|r| *r == authored[0]) {
                crate::border::Border::new(bounds, border_widths, authored[0])
            } else {
                crate::border::Border::with_radii(bounds, border_widths, authored.map(|r| [r; 2]))
            };
            let corners = (!authored.iter().all(|r| *r == authored[0]))
                .then(|| geometry.outer.radii.map(|pair| pair.map(|v| v as f32)));
            let radius = authored[0];
            let border = border_widths
                .iter()
                .any(|w| *w > 0.0)
                .then_some(geometry.clone());
            let content_clip =
                (border.is_some() || corners.is_some() || record.tag == 18).then(|| ClipSnapshot {
                    bounds: geometry.inner.bounds.map(|v| v as f32),
                    radius: 0.0,
                    radii: Some(geometry.inner.radii.map(|pair| pair.map(|v| v as f32))),
                });
            let control_bounds = content_clip.map(|clip| clip.bounds).unwrap_or(bounds);
            let visible_bounds = intersect(bounds, viewport);
            let scroll_parent = record.parent.and_then(|p| scroll_parents[p].clone());
            let scroll = (matches!(record.tag, 5 | 6)).then(|| {
                let axis = if record.string(100) == Some("horizontal") {
                    ScrollAxis::Horizontal
                } else {
                    ScrollAxis::Vertical
                };
                let max_offset = match axis {
                    ScrollAxis::Horizontal => layout.scroll_width(),
                    ScrollAxis::Vertical => layout.scroll_height(),
                };
                ScrollSnapshot {
                    content_insets: [
                        layout.padding.top,
                        layout.padding.right,
                        layout.padding.bottom,
                        layout.padding.left,
                    ],
                    content_size: [
                        (layout.size.width - layout.padding.left - layout.padding.right).max(0.0),
                        (layout.size.height - layout.padding.top - layout.padding.bottom).max(0.0),
                    ],
                    axis,
                    on_reach_start: record
                        .fields
                        .iter()
                        .find(|(id, _)| *id == 17)
                        .and_then(|(_, v)| v.as_u64()),
                    on_reach_end: record
                        .fields
                        .iter()
                        .find(|(id, _)| *id == 18)
                        .and_then(|(_, v)| v.as_u64()),
                    parent: scroll_parent.clone(),
                    offset: 0.0,
                    max_offset,
                    id: record
                        .string(11)
                        .map(str::to_owned)
                        .unwrap_or_else(|| identity.clone()),
                }
            });
            scroll_parents.push(
                scroll
                    .as_ref()
                    .map(|s| s.id.clone())
                    .or(scroll_parent.clone()),
            );
            origins.push((x, y));
            let paint_start = commands.len();
            if let Some(source) = record.string(73) {
                for shadow in crate::shadow::BoxShadow::parse(source)?.into_iter().rev() {
                    let tint = if shadow.color == "currentColor" {
                        &colors[index]
                    } else {
                        &shadow.color
                    };
                    let color = color(tint)?;
                    if color[3] != 0 {
                        if let Some(corners) = corners {
                            if let Some(mut raster) = self.shadow_cache.get(
                                [bounds[2], bounds[3]],
                                corners.map(|r| r[0]),
                                &shadow,
                                color,
                                scale,
                            )? {
                                let identity = Arc::as_ptr(raster.bitmap.pixels()) as usize;
                                if shadow_masks.insert(identity) {
                                    shadow_bytes += raster.bitmap.byte_len();
                                }
                                if shadow_bytes > 16 * 1024 * 1024 {
                                    return Err("UI_SHADOW: frame mask budget exceeded".into());
                                }
                                raster.bounds[0] += bounds[0];
                                raster.bounds[1] += bounds[1];
                                commands.push(Paint::RasterShadow { raster });
                            }
                        } else {
                            commands.push(Paint::Shadow {
                                shadow,
                                color,
                                bounds,
                                radius,
                            });
                        }
                    }
                }
            }

            if let Some(background) = record.string(6).or(if record.tag == 3 {
                Some("primary")
            } else if matches!(record.tag, 8 | 9 | 14) {
                Some("surface")
            } else if index == 0 {
                Some("background")
            } else {
                None
            }) {
                let [r, g, b, a] = color(background)?;
                commands.push(Paint::Rect {
                    corners,
                    color: [r, g, b, a],
                    bounds: [x, y, layout.size.width, layout.size.height],
                    radius,
                });
            }
            if let Some(source) = record.string(72) {
                if let Some(gradient) = crate::gradient::LinearGradient::parse(source)? {
                    commands.push(Paint::Gradient {
                        corners,
                        gradient: Arc::new(gradient),
                        bounds: [x, y, layout.size.width, layout.size.height],
                        radius,
                    });
                }
            }
            if border.is_some() {
                let tint = record
                    .string(66)
                    .filter(|value| *value != "currentColor")
                    .unwrap_or(&colors[index]);
                commands.push(Paint::Border {
                    corners,
                    color: color(tint)?,
                    bounds,
                    widths: border_widths,
                    radius,
                });
            }
            if record.tag == 19 {
                let vector = crate::vector::Vector::parse(record.string(80).unwrap())?;
                let vector_bounds = [
                    control_bounds[0] + layout.padding.left,
                    control_bounds[1] + layout.padding.top,
                    (control_bounds[2] - layout.padding.left - layout.padding.right).max(0.0),
                    (control_bounds[3] - layout.padding.top - layout.padding.bottom).max(0.0),
                ];
                let (shapes, work) = vector.resolved_shapes(vector_bounds)?;
                vector_work += work;
                if vector_work > 32768 {
                    return Err("UI_LIMIT: vector geometry budget exceeded".into());
                }
                for (path, tint) in shapes {
                    let box_ = path.control_box();
                    if !path.is_finite()
                        || [box_.x0, box_.y0, box_.x1, box_.y1]
                            .iter()
                            .any(|value| value.abs() * f64::from(scale.max(1.0)) > 1e12)
                    {
                        return Err("UI_LIMIT: vector geometry exceeds coordinate budget".into());
                    }
                    commands.push(Paint::Vector {
                        path: Arc::new(path),
                        color: color(if tint == "currentColor" {
                            &colors[index]
                        } else {
                            &tint
                        })?,
                        bounds: vector_bounds,
                    });
                }
            }
            let mut plot_snapshot = None;
            if record.tag == 18 {
                let plot = crate::plot::Plot::from_record(record)?;
                plot_snapshot = Some(plot.snapshot());
                for shape in plot.shapes(control_bounds) {
                    commands.push(match shape {
                        crate::plot::Shape::Area(points, range, color) => Paint::Area {
                            points,
                            range,
                            color,
                        },
                        crate::plot::Shape::Circle(bounds, color) => Paint::Rect {
                            corners: None,
                            color,
                            bounds,
                            radius: bounds[2] / 2.0,
                        },
                        crate::plot::Shape::Bar(bounds, color) => Paint::Vector {
                            color,
                            bounds,
                            path: Arc::new(plot_bar_path(bounds)),
                        },
                        crate::plot::Shape::Stroke(points, width, color) => Paint::Stroke {
                            color,
                            points,
                            width,
                            round: plot.kind != "pie",
                        },
                    });
                }
            }
            let range = if record.tag == 15 {
                Some(crate::SliderSnapshot::from_record(record)?)
            } else {
                None
            };
            if let Some(range) = &range {
                let left = control_bounds[0] + range.inset;
                let track_width = (control_bounds[2] - 2.0 * range.inset).max(0.0);
                let center = control_bounds[1] + control_bounds[3] / 2.0;
                let fraction = ((range.value - range.min) / (range.max - range.min)) as f32;
                let accent = if range.enabled {
                    color(record.string(5).unwrap_or("primary"))?
                } else {
                    [160, 168, 190, 255]
                };
                commands.push(Paint::Rect {
                    corners: None,
                    color: [193, 199, 210, 255],
                    bounds: [left, center - 2.0, track_width, 4.0],
                    radius: 2.0,
                });
                commands.push(Paint::Rect {
                    corners: None,
                    color: accent,
                    bounds: [left, center - 2.0, track_width * fraction, 4.0],
                    radius: 2.0,
                });
                commands.push(Paint::Rect {
                    corners: None,
                    color: accent,
                    bounds: [
                        left + track_width * fraction - 12.0,
                        center - 12.0,
                        24.0,
                        24.0,
                    ],
                    radius: 12.0,
                });
            }
            if record.tag == 14 {
                let cx = x + layout.padding.left - 30.0;
                let cy = y
                    + layout.padding.top
                    + (layout.size.height - layout.padding.top - layout.padding.bottom).max(0.0)
                        / 2.0;
                let tint = if record.number(9) == Some(0.0) {
                    [193, 199, 210, 255]
                } else {
                    color(&colors[index])?
                };
                commands.push(Paint::Rect {
                    corners: None,
                    color: tint,
                    bounds: [cx, cy - 10.0, 18.0, 18.0],
                    radius: 9.0,
                });
                commands.push(Paint::Rect {
                    corners: None,
                    color: color(record.string(6).unwrap_or("surface"))?,
                    bounds: [cx + 2.0, cy - 8.0, 14.0, 14.0],
                    radius: 7.0,
                });
                commands.push(Paint::Stroke {
                    round: true,
                    color: tint,
                    points: vec![
                        [cx + 15.0, cy + 5.0],
                        [cx + 18.0, cy + 8.0],
                        [cx + 21.0, cy + 11.0],
                    ],
                    width: 2.0,
                });
            }
            let mut text_offset_y = 0.0;
            if let Some(text) = &mut layouts[index] {
                let started = std::time::Instant::now();
                let content_width = (layout.size.width
                    - layout.padding.left
                    - layout.padding.right
                    - check_reserve(record.tag))
                .max(0.0);
                // Overflowing single-line editors retain a nonnegative text origin;
                // caret scrolling, not alignment, reveals the hidden text.
                let alignment_width = if matches!(record.tag, 8 | 14) {
                    content_width.max(text.full_width())
                } else {
                    content_width
                };
                text.break_all_lines(Some(alignment_width));
                text.align(text_alignments[index], AlignmentOptions::default());
                let placeholder =
                    matches!(record.tag, 8 | 9 | 14) && record.string(21).unwrap_or("").is_empty();
                let tint = if placeholder {
                    record.string(97).unwrap_or(&colors[index])
                } else {
                    &colors[index]
                };
                let [r, g, b, a] = color(tint)?;
                self.text_time += started.elapsed();

                let label_x = if matches!(record.tag, 10 | 13) {
                    check_reserve(record.tag)
                } else {
                    0.0
                };
                let label_y = if matches!(record.tag, 3 | 8 | 10 | 11 | 13 | 14) {
                    ((layout.size.height
                        - layout.padding.top
                        - layout.padding.bottom
                        - text.height())
                        / 2.0)
                        .max(0.0)
                } else {
                    0.0
                };
                text_offset_y = label_y;
                for line in text
                    .lines()
                    .take(record.number(50).map(|n| n as usize).unwrap_or(usize::MAX))
                {
                    for item in line.items() {
                        if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                            let run = glyph_run.run();
                            let coordinates: Vec<i16> = run
                                .normalized_coords()
                                .iter()
                                .map(|v| v.to_bits())
                                .collect();
                            let mut advance = glyph_run.offset();
                            let baseline = glyph_run.baseline();
                            let glyphs = glyph_run.glyphs().map(move |g| {
                                let glyph = Glyph {
                                    id: g.id,
                                    x: x + layout.padding.left + label_x + advance + g.x,
                                    y: y + layout.padding.top + label_y + baseline - g.y,
                                };
                                advance += g.advance;
                                glyph
                            });
                            let glyphs: Vec<Glyph> = glyphs.collect();
                            let skew =
                                f64::from(run.synthesis().skew().unwrap_or(0.0).to_radians().tan());
                            let ink = glyph_ink(
                                run.font(),
                                run.font_size(),
                                &coordinates,
                                &glyphs,
                                skew as f32,
                                (1.0 / scale).max(1.0),
                            );
                            commands.push(Paint::Text {
                                ink,
                                color: [r, g, b, a],
                                font: run.font().clone(),
                                size: run.font_size(),
                                coordinates,
                                skew,
                                glyphs,
                            });
                            if let Some(owner) = strike_owners[index] {
                                let metrics = run.font_metrics();
                                commands.push(Paint::Rect {
                                    corners: None,
                                    color: color(&colors[owner])?,
                                    bounds: [
                                        x + layout.padding.left + label_x + glyph_run.offset(),
                                        y + layout.padding.top + label_y + baseline
                                            - metrics.strikethrough_offset,
                                        glyph_run.advance(),
                                        metrics.strikethrough_size.max(1.0 / scale),
                                    ],
                                    radius: 0.0,
                                });
                            }
                        }
                    }
                }
            }
            if matches!(record.tag, 10 | 11 | 13)
                && !(record.tag == 10 && button_hashes.contains_key(&index))
            {
                let checked = record.number(39) == Some(1.0);
                let enabled = record.number(9) != Some(0.0)
                    && record
                        .parent
                        .is_none_or(|p| records[p].tag != 12 || records[p].number(9) != Some(0.0))
                    && record.number(40).is_some();
                let accent = if enabled {
                    [67, 93, 217, 255]
                } else {
                    [160, 168, 190, 255]
                };
                let neutral = if enabled {
                    [131, 143, 163, 255]
                } else {
                    [193, 199, 210, 255]
                };
                let cy = y
                    + layout.padding.top
                    + (layout.size.height - layout.padding.top - layout.padding.bottom).max(0.0)
                        / 2.0;
                if record.tag == 13 {
                    let cx = x + layout.padding.left;
                    commands.push(Paint::Rect {
                        corners: None,
                        color: if checked { accent } else { neutral },
                        bounds: [cx, cy - 12.0, 24.0, 24.0],
                        radius: 12.0,
                    });
                    commands.push(Paint::Rect {
                        corners: None,
                        color: color(record.string(6).unwrap_or("surface"))?,
                        bounds: [cx + 2.0, cy - 10.0, 20.0, 20.0],
                        radius: 10.0,
                    });
                    if checked {
                        commands.push(Paint::Rect {
                            corners: None,
                            color: accent,
                            bounds: [cx + 6.0, cy - 6.0, 12.0, 12.0],
                            radius: 6.0,
                        });
                    }
                } else if record.tag == 10 {
                    let cx = x + layout.padding.left;
                    commands.push(Paint::Rect {
                        corners: None,
                        color: if checked { accent } else { neutral },
                        bounds: [cx, cy - 12.0, 24.0, 24.0],
                        radius: 5.0,
                    });
                    if checked {
                        commands.push(Paint::Stroke {
                            round: true,
                            color: [255; 4],
                            points: vec![
                                [cx + 6.0, cy],
                                [cx + 10.0, cy + 4.0],
                                [cx + 18.0, cy - 5.0],
                            ],
                            width: 2.5,
                        });
                    } else {
                        commands.push(Paint::Rect {
                            corners: None,
                            color: color(record.string(6).unwrap_or("surface"))?,
                            bounds: [cx + 2.0, cy - 10.0, 20.0, 20.0],
                            radius: 3.0,
                        });
                    }
                } else {
                    let cx = x + layout.size.width - layout.padding.right - 52.0;
                    commands.push(Paint::Rect {
                        corners: None,
                        color: if checked { accent } else { neutral },
                        bounds: [cx, cy - 16.0, 52.0, 32.0],
                        radius: 16.0,
                    });
                    commands.push(Paint::Rect {
                        corners: None,
                        color: [255; 4],
                        bounds: [cx + if checked { 24.0 } else { 4.0 }, cy - 12.0, 24.0, 24.0],
                        radius: 12.0,
                    });
                }
            }
            if record.tag == 7 {
                commands.push(Paint::Image {
                    src: record
                        .string(19)
                        .ok_or("UI_IMAGE: missing source")?
                        .to_owned(),
                    bounds: control_bounds,
                    radius: if content_clip.is_some() { 0.0 } else { radius },
                    fit: match record.string(20) {
                        Some("contain") => crate::image::Fit::Contain,
                        Some("fill") => crate::image::Fit::Fill,
                        _ => crate::image::Fit::Cover,
                    },
                });
            }
            input_layouts.push(if matches!(record.tag, 8 | 9 | 14) {
                Some(InputLayout {
                    layout: layouts[index].take().unwrap(),
                    padding: [layout.padding.left, layout.padding.top],
                    text_offset_y,
                    content_size: [
                        (layout.size.width - layout.padding.left - layout.padding.right).max(0.0),
                        (layout.size.height - layout.padding.top - layout.padding.bottom).max(0.0),
                    ],
                    color: color(&colors[index])?,
                })
            } else {
                None
            });
            ranges.push(paint_start..commands.len());
            nodes.push(NodeSnapshot {
                plot_local: None,
                radii: corners,
                plot: plot_snapshot,
                content_clip,
                control_bounds: (border.is_some() && matches!(record.tag, 7 | 15 | 18))
                    .then_some(control_bounds),
                scroll_parent,
                radio_group: (record.tag == 13).then(|| identities[record.parent.unwrap()].clone()),
                accessibility_label: record
                    .string(41)
                    .or_else(|| record.string(105))
                    .filter(|label| !label.is_empty())
                    .map(str::to_owned),
                accessibility_hidden: (button_owners[index].is_some()
                    || record.string(105) == Some(""))
                .then_some(true),
                button_content: button_hashes
                    .get(&index)
                    .map(|hash| format!("{:016x}", hash.finish())),
                identity,
                checked: matches!(record.tag, 10 | 11 | 13).then(|| record.number(39) == Some(1.0)),
                range,
                input: matches!(record.tag, 8 | 9 | 14).then(|| InputSnapshot {
                    edit: None,
                    edit_token: None,
                    geometry: None,
                    scroll_offset: [0.0; 2],
                    value: record.string(21).unwrap_or("").into(),
                    placeholder: record.string(22).unwrap_or("").into(),
                    secure: record.number(23) == Some(1.0),
                    keyboard: record
                        .string(24)
                        .unwrap_or(if record.tag == 14 { "search" } else { "text" })
                        .into(),
                    auto_focus: record.number(25) == Some(1.0),
                    submit_on_enter: record.number(83) == Some(1.0),
                    enabled: record.number(9) != Some(0.0),
                    on_input: record
                        .fields
                        .iter()
                        .find(|(id, _)| *id == 26)
                        .and_then(|(_, v)| v.as_u64()),
                    on_submit: record
                        .fields
                        .iter()
                        .find(|(id, _)| *id == 27)
                        .and_then(|(_, v)| v.as_u64()),
                }),
                tag: crate::schema::TAGS[record.tag],
                test_id: record.test_id.clone(),
                text: if matches!(record.tag, 3 | 10) {
                    record.string(41).unwrap_or(&texts[index]).to_owned()
                } else {
                    texts[index].clone()
                },
                bounds,
                visible_bounds,
                clips: Vec::new(),
                scroll,
                radius,
                on_click: if record.number(9) == Some(0.0)
                    || record
                        .parent
                        .is_some_and(|p| records[p].tag == 12 && records[p].number(9) == Some(0.0))
                {
                    None
                } else {
                    record
                        .fields
                        .iter()
                        .find(|(id, _)| {
                            *id == if matches!(record.tag, 10 | 11 | 13) {
                                40
                            } else {
                                8
                            }
                        })
                        .and_then(|(_, value)| value.as_u64())
                },
            });
        }
        Arc::new(RetainedLayout {
            commands,
            nodes,
            ranges,
            parents: records.iter().map(|record| record.parent).collect(),
            backdrops: records
                .iter()
                .map(|r| {
                    (r.number(95).is_some() || r.number(96).is_some())
                        .then(|| (r.number(95).unwrap_or(0.0), r.number(96).unwrap_or(1.0)))
                })
                .collect(),
            opacities: records
                .iter()
                .map(|record| record.number(58).unwrap_or(1.0))
                .collect(),
            width,
            height,
            scale,
            inputs: input_layouts,
        })
        .compose(offsets)
    }
}

fn reveal_caret(previous: f32, start: f32, end: f32, viewport: f32, extent: f32) -> f32 {
    let max = (extent.max(end) - viewport).max(0.0);
    let mut offset = previous.clamp(0.0, max);
    if end > offset + viewport {
        offset = end - viewport;
    }
    if start < offset {
        offset = start;
    }
    offset.clamp(0.0, max)
}

fn display_byte(value: &str, offset: u32, secure: bool, single_line: bool) -> Option<usize> {
    let mut units = 0;
    let mut bytes = 0;
    for ch in value.chars() {
        if units == offset {
            return Some(bytes);
        }
        units += ch.len_utf16() as u32;
        if units > offset {
            return None;
        }
        bytes += if secure {
            '\u{2022}'.len_utf8()
        } else if single_line && matches!(ch, '\r' | '\n' | '\u{2028}' | '\u{2029}') {
            1
        } else {
            ch.len_utf8()
        };
    }
    (units == offset).then_some(bytes)
}

fn display_offset(value: &str, byte: usize, secure: bool, single_line: bool) -> Option<u32> {
    let mut units = 0;
    let mut bytes = 0;
    for ch in value.chars() {
        if bytes == byte {
            return Some(units);
        }
        bytes += if secure {
            '\u{2022}'.len_utf8()
        } else if single_line && matches!(ch, '\r' | '\n' | '\u{2028}' | '\u{2029}') {
            1
        } else {
            ch.len_utf8()
        };
        if bytes > byte {
            return None;
        }
        units += ch.len_utf16() as u32;
    }
    (bytes == byte).then_some(units)
}

fn glyph_ink(
    font: &parley::FontData,
    size: f32,
    coordinates: &[i16],
    glyphs: &[Glyph],
    skew: f32,
    fringe: f32,
) -> Option<[f32; 4]> {
    use skrifa::{
        FontRef, GlyphId, MetadataProvider,
        instance::{NormalizedCoord, Size},
    };
    let font = FontRef::from_index(font.data.as_ref(), font.index).ok()?;
    let coordinates: Vec<_> = coordinates
        .iter()
        .map(|value| NormalizedCoord::from_bits(*value))
        .collect();
    let metrics = font.glyph_metrics(Size::new(size), coordinates.as_slice());
    let mut bounds: Option<[f32; 4]> = None;
    for glyph in glyphs {
        let ink = metrics.bounds(GlyphId::new(glyph.id))?;
        if ink.x_min == ink.x_max || ink.y_min == ink.y_max {
            continue;
        }
        let next = [
            glyph.x + ink.x_min + (skew * ink.y_min).min(skew * ink.y_max),
            glyph.y - ink.y_max,
            glyph.x + ink.x_max + (skew * ink.y_min).max(skew * ink.y_max),
            glyph.y - ink.y_min,
        ];
        bounds = Some(match bounds {
            Some(old) => [
                old[0].min(next[0]),
                old[1].min(next[1]),
                old[2].max(next[2]),
                old[3].max(next[3]),
            ],
            None => next,
        });
    }
    // Keep an antialiasing fringe; missing font metrics conservatively disable culling.
    Some(
        bounds
            .map(|b| {
                [
                    b[0] - fringe,
                    b[1] - fringe,
                    b[2] - b[0] + 2.0 * fringe,
                    b[3] - b[1] + 2.0 * fringe,
                ]
            })
            .unwrap_or([0.0; 4]),
    )
}

fn intersect(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let x = a[0].max(b[0]);
    let y = a[1].max(b[1]);
    [
        x,
        y,
        (a[0] + a[2]).min(b[0] + b[2]).max(x) - x,
        (a[1] + a[3]).min(b[1] + b[3]).max(y) - y,
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn flex_basis_is_independent_of_growth_in_both_axes() {
        for (axis, size, index) in [("row", "width", 2), ("column", "height", 3)] {
            for (basis, expected) in [
                ("", [125.0, 175.0]),
                (",flexBasis:'auto'", [125.0, 175.0]),
                (",flexBasis:0", [150.0, 150.0]),
                (",flexBasis:'25%'", [150.0, 150.0]),
            ] {
                let source = crate::session::tests::source(&format!(
                    "function render(){{return h('{axis}',{{style:{{width:300,height:300}}}},h('column',{{testId:'a',style:{{{size}:50,grow:1{basis}}}}}),h('column',{{testId:'b',style:{{{size}:100,grow:1{basis}}}}}))}}"
                ));
                let mut page = crate::ViewSession::new(&source).unwrap();
                for scale in [1.0, 2.0] {
                    let display = page.layout(600, 600, scale).unwrap();
                    for (id, wanted) in ["a", "b"].into_iter().zip(expected) {
                        let actual = display
                            .nodes
                            .iter()
                            .find(|node| node.test_id == id)
                            .unwrap()
                            .bounds[index];
                        assert!(
                            (actual - wanted).abs() < 0.01,
                            "{axis} {basis} scale={scale}: {actual} vs {wanted}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn percentage_basis_recomputes_after_parent_resize() {
        let source = crate::session::tests::source(
            "function render(){const wide=useLocal('wide',false);return h('column',{},h('row',{style:{width:wide.value?240:160}},h('column',{testId:'half',style:{flexBasis:'50%',height:20,shrink:0}})),h('button',{testId:'resize',onClick:()=>wide.set(!wide.value)},'Resize'))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(300, 300, 1.0).unwrap();
        assert_eq!(
            display
                .nodes
                .iter()
                .find(|n| n.test_id == "half")
                .unwrap()
                .bounds[2],
            80.0
        );
        let button = display
            .nodes
            .iter()
            .find(|n| n.test_id == "resize")
            .unwrap();
        page.click(page.generation(), button.on_click.unwrap())
            .unwrap();
        let display = page.layout(300, 300, 1.0).unwrap();
        assert_eq!(
            display
                .nodes
                .iter()
                .find(|n| n.test_id == "half")
                .unwrap()
                .bounds[2],
            120.0
        );
    }
    #[test]
    fn flex_wrap_reflows_items_with_gaps_and_reverse_order() {
        for mode in ["nowrap", "wrap", "wrap-reverse"] {
            let source = crate::session::tests::source(&format!(
                "function render(){{return h('column',{{}},h('row',{{testId:'wrap',style:{{flexWrap:'{mode}',gap:10}}}},[0,1,2].map(i=>h('column',{{key:String(i),testId:'item'+i,style:{{width:70,height:20}}}}))))}}"
            ));
            let mut page = crate::ViewSession::new(&source).unwrap();
            let display = page.layout(150, 200, 1.0).unwrap();
            let bounds = |id: &str| {
                display
                    .nodes
                    .iter()
                    .find(|node| node.test_id == id)
                    .unwrap()
                    .bounds
            };
            assert_eq!(bounds("item1")[0], 80.0);
            if mode == "nowrap" {
                assert_eq!(bounds("item2")[0], 160.0);
                assert_eq!(bounds("wrap")[3], 20.0);
            } else {
                assert_eq!(bounds("wrap")[3], 50.0);
                assert_eq!(bounds("item2")[0], 0.0);
                assert_eq!(
                    (bounds("item0")[1], bounds("item2")[1]),
                    if mode == "wrap" {
                        (0.0, 30.0)
                    } else {
                        (30.0, 0.0)
                    }
                );
                let narrow = page.layout(70, 200, 1.0).unwrap();
                assert_eq!(
                    narrow
                        .nodes
                        .iter()
                        .find(|node| node.test_id == "wrap")
                        .unwrap()
                        .bounds[3],
                    80.0
                );
            }
        }
    }
    #[test]
    fn bar_paths_stay_inside_short_narrow_and_zero_bounds() {
        use vello_cpu::kurbo::Shape;
        for (width, height) in [
            (36.0, 30.0),
            (1.0, 30.0),
            (36.0, 0.5),
            (0.25, 0.125),
            (36.0, 0.0),
        ] {
            let path = super::plot_bar_path([10.0, 20.0, width, height]);
            assert_eq!(path.elements().len(), 8);
            let bounds = path.bounding_box();
            for (actual, expected) in [
                (bounds.x0, 10.0),
                (bounds.y0, 20.0),
                (bounds.x1, 10.0 + f64::from(width)),
                (bounds.y1, 20.0 + f64::from(height)),
            ] {
                assert!((actual - expected).abs() < 1e-9);
            }
            assert!(path.area().is_finite());
        }
    }
    #[test]
    fn zero_opacity_contains_input_decorations_without_removing_semantics() {
        let module = crate::session::tests::source(
            "function render(){return h('column',{style:{background:'#ffffff',width:100,height:100}},h('input',{testId:'editor',value:'Hello',autoFocus:true,style:{opacity:0,width:100,height:60}}))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let display = page.layout(100, 100, 1.0).unwrap();
        let editor = display
            .nodes
            .iter()
            .find(|node| node.test_id == "editor")
            .unwrap();
        assert!(editor.input.as_ref().unwrap().geometry.is_some());
        assert!(editor.visible_bounds[2] > 0.0);
        let frame = display.rasterize().unwrap();
        assert!(
            frame
                .pixels
                .data_as_u8_slice()
                .chunks_exact(4)
                .all(|pixel| pixel == [255, 255, 255, 255])
        );
    }

    #[test]
    fn borders_preserve_input_content_and_paint_after_editor_foreground() {
        let module = crate::session::tests::source(
            "function render(){return h('input',{autoFocus:true,value:'abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz',style:{width:160,height:56,padding:8,borderWidth:6,borderColor:'#ff0000',radius:0,background:'#ffffff'}})}",
        );
        for scale in [1.0, 2.0] {
            let mut page = crate::ViewSession::new(&module).unwrap();
            let display = page
                .layout((200.0 * scale) as u16, (100.0 * scale) as u16, scale)
                .unwrap();
            let geometry = display.nodes[0]
                .input
                .as_ref()
                .unwrap()
                .geometry
                .as_ref()
                .unwrap();
            assert_eq!(geometry.content_bounds, [14.0, 14.0, 132.0, 28.0]);
            assert_eq!(
                display.nodes[0].content_clip.unwrap().bounds,
                [6.0, 6.0, 148.0, 44.0]
            );
            let frame = display.rasterize().unwrap();
            for (x, y) in [(2.0, 28.0), (158.0, 28.0), (80.0, 2.0), (80.0, 54.0)] {
                let offset =
                    ((y * scale) as usize * (200.0 * scale) as usize + (x * scale) as usize) * 4;
                assert_eq!(
                    &frame.pixels.data_as_u8_slice()[offset..offset + 4],
                    [255, 0, 0, 255]
                );
            }
        }
    }

    #[test]
    fn retained_scroll_combines_axes_and_reveals_through_nested_regions() {
        let module = crate::session::tests::source(
            "function render(){return h('scroll',{testId:'outer',style:{width:180,height:140,background:'#ffffff'}},h('spacer',{style:{height:30}}),h('scroll',{testId:'inner',style:{scrollAxis:'horizontal',width:180,height:80,borderWidth:2,borderColor:'#000000',radius:12}},h('row',{style:{width:400,height:60}},h('spacer',{style:{width:220}}),h('button',{testId:'target',onClick:()=>{},style:{width:80,height:50,background:'#0000ff',color:'#ffffff'}},'Target'))),h('spacer',{style:{height:300}}))}",
        );
        for scale in [1.0, 2.0] {
            let mut page = crate::ViewSession::new(&module).unwrap();
            let original = page
                .layout((180.0 * scale) as u16, (140.0 * scale) as u16, scale)
                .unwrap();
            let retained = original.retained.clone();
            let outer = retained
                .nodes
                .iter()
                .find(|n| n.test_id == "outer")
                .unwrap()
                .scroll
                .as_ref()
                .unwrap()
                .id
                .clone();
            let inner = retained
                .nodes
                .iter()
                .find(|n| n.test_id == "inner")
                .unwrap()
                .scroll
                .as_ref()
                .unwrap();
            assert!(matches!(inner.axis, super::ScrollAxis::Horizontal));
            assert!(inner.max_offset >= 220.0);
            let inner = inner.id.clone();
            let before = retained.compose(&HashMap::new()).unwrap();
            let target = before.nodes.iter().find(|n| n.test_id == "target").unwrap();
            let identity = target.identity.clone();
            let original_bounds = target.bounds;
            assert_eq!(target.visible_bounds[2], 0.0);
            let offsets = HashMap::from([(outer.clone(), 20.0), (inner.clone(), 180.0)]);
            let shifted = retained.compose(&offsets).unwrap();
            let target = shifted
                .nodes
                .iter()
                .find(|n| n.test_id == "target")
                .unwrap();
            assert_eq!(
                target.bounds,
                [
                    original_bounds[0] - 180.0,
                    original_bounds[1] - 20.0,
                    original_bounds[2],
                    original_bounds[3]
                ]
            );
            assert!(target.visible_bounds[2] > 0.0);
            let inner_node = shifted.nodes.iter().find(|n| n.test_id == "inner").unwrap();
            assert_eq!(inner_node.bounds[0], 0.0);
            assert_eq!(inner_node.bounds[1], 10.0);
            assert_eq!(
                target.clips.last().unwrap().bounds,
                inner_node.content_clip.unwrap().bounds
            );
            let pixels = shifted.rasterize().unwrap().pixels;
            let pixel = |x: usize, y: usize| {
                let index = ((y as f32 * scale) as usize * (180.0 * scale) as usize
                    + (x as f32 * scale) as usize)
                    * 4;
                pixels.data_as_u8_slice()[index..index + 4].to_vec()
            };
            assert_eq!(pixel(50, 20), [0, 0, 255, 255]);
            assert_eq!(pixel(20, 20), [255, 255, 255, 255]);
            let mut revealed_offsets = HashMap::new();
            let revealed = before.reveal(&identity, &mut revealed_offsets).unwrap();
            assert!(revealed_offsets[&inner] > 0.0);
            assert!(!revealed_offsets.contains_key(&outer));
            let target = revealed
                .nodes
                .iter()
                .find(|n| n.test_id == "target")
                .unwrap();
            assert!(target.visible_bounds[2] > 0.0);
            let restored = retained.compose(&HashMap::new()).unwrap();
            assert_eq!(
                restored
                    .nodes
                    .iter()
                    .find(|n| n.test_id == "target")
                    .unwrap()
                    .bounds,
                original_bounds
            );
        }
    }

    #[test]
    fn bordered_scroll_keeps_children_inside_the_inner_ellipse() {
        let module = crate::session::tests::source(
            "function render(){return h('column',{style:{background:'#ffffff'}},h('scroll',{testId:'scroll',style:{width:100,height:80,borderTopWidth:2,borderRightWidth:4,borderBottomWidth:6,borderLeftWidth:8,borderColor:'#ff0000',radius:20,background:'#ffffff'}},h('spacer',{testId:'child',style:{height:200,background:'#0000ff'}})))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let first = page.layout(120, 160, 1.0).unwrap();
        let scroll = first
            .nodes
            .iter()
            .find(|node| node.test_id == "scroll")
            .unwrap();
        let id = scroll.scroll.as_ref().unwrap().id.clone();
        assert_eq!(scroll.content_clip.unwrap().bounds, [8.0, 2.0, 88.0, 72.0]);
        let shifted = first
            .retained
            .compose(&HashMap::from([(id, 50.0)]))
            .unwrap();
        let child = shifted
            .nodes
            .iter()
            .find(|node| node.test_id == "child")
            .unwrap();
        assert_eq!(child.bounds[0], 8.0);
        assert_eq!(child.clips.last().unwrap().radii.unwrap()[0], [12.0, 18.0]);
        assert!(!child.clips.last().unwrap().contains([9.0, 3.0]));
        let frame = shifted.rasterize().unwrap();
        for (x, y, color) in [
            (5usize, 40usize, [255, 0, 0, 255]),
            (50, 77, [255, 0, 0, 255]),
            (50, 40, [0, 0, 255, 255]),
            (10, 5, [255, 0, 0, 255]),
        ] {
            let offset = (y * 120 + x) * 4;
            assert_eq!(
                &frame.pixels.data_as_u8_slice()[offset..offset + 4],
                color,
                "pixel {x},{y}"
            );
        }
    }

    #[test]
    fn strikethrough_tracks_wrapped_runs_and_cached_text_without_changing_layout() {
        let mut page = crate::ViewSession::new(&crate::session::tests::source(
            "function render(){const strike=useLocal('strike',false);return h('column',{style:{textDecorationLine:strike.value?'line-through':'none',color:'#123456'}},h('text',{style:{width:80,fontSize:20,textDecorationLine:'none',textAlign:'right'}},'one two three four'),h('button',{onClick:()=>strike.set(!strike.value)},'Toggle'))}",
        )).unwrap();
        let mut plain = None;
        for strike in [false, true, false] {
            let frame = page.layout(200, 200, 1.0).unwrap();
            let commands = &frame.retained.commands[frame.retained.ranges[1].clone()];
            let text_runs = commands
                .iter()
                .filter(|p| matches!(p, Paint::Text { .. }))
                .count();
            let strikes: Vec<_> = commands
                .iter()
                .filter_map(|p| match p {
                    Paint::Rect {
                        corners: _,
                        bounds,
                        color: [18, 52, 86, 255],
                        radius: 0.0,
                    } => Some(bounds),
                    _ => None,
                })
                .collect();
            assert!(text_runs > 1, "fixture must wrap");
            assert_eq!(strikes.len(), if strike { text_runs } else { 0 });
            for bounds in strikes {
                assert!(bounds[2] > 0.0 && bounds[2] <= 80.0 && bounds[3] >= 1.0);
            }
            let callback = frame.nodes[2].on_click.unwrap();
            let pixels = frame
                .rasterize()
                .unwrap()
                .pixels
                .data_as_u8_slice()
                .to_vec();
            if strike {
                assert_ne!(plain.as_ref().unwrap(), &pixels);
            } else if let Some(expected) = &plain {
                assert_eq!(expected, &pixels);
            } else {
                plain = Some(pixels);
            }
            page.click(page.generation(), callback).unwrap();
        }
    }

    #[test]
    fn text_alignment_updates_cached_glyph_positions_and_preserves_overrides() {
        let source = crate::session::tests::source(
            "function render(){const right=useLocal('right',true);return h('column',{style:{textAlign:right.value?'right':'left'}},h('text',{style:{width:160,padding:8,borderWidth:2}},'MMMM\\nM'),h('text',{style:{width:160,padding:8,borderWidth:2,textAlign:'center'}},'MMMM\\nM'),h('button',{onClick:()=>right.set(!right.value)},'Toggle'))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let mut right_pixels = None;
        let mut centered = None;
        for right in [true, false, true] {
            let display = page.layout(200, 240, 1.0).unwrap();
            let origins = |index: usize| {
                display.retained.commands[display.retained.ranges[index].clone()]
                    .iter()
                    .filter_map(|paint| {
                        if let Paint::Text { glyphs, .. } = paint {
                            glyphs.first().map(|glyph| glyph.x)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            };
            let positions = origins(1usize);
            assert_eq!(positions.len(), 2);
            if right {
                assert!(positions[1] > positions[0] + 20.0);
            } else {
                assert!((positions[0] - positions[1]).abs() < 0.01);
            }
            if let Some(previous) = &centered {
                assert_eq!(&origins(2), previous);
            } else {
                centered = Some(origins(2));
            }
            let callback = display.nodes[3].on_click.unwrap();
            let pixels = display
                .rasterize()
                .unwrap()
                .pixels
                .data_as_u8_slice()
                .to_vec();
            if right {
                if let Some(previous) = &right_pixels {
                    assert_eq!(&pixels, previous);
                } else {
                    right_pixels = Some(pixels);
                }
            }
            page.click(page.generation(), callback).unwrap();
        }
    }

    #[test]
    fn inherited_text_alignment_respects_each_line_and_direction() {
        for rtl in [false, true] {
            for alignment in ["left", "center", "right", "start", "end", "justify"] {
                let value = if rtl {
                    "\\u05d0\\u05d1\\u05d2\\n\\u05d0"
                } else {
                    "MMMM\\nM"
                };
                let source = crate::session::tests::source(&format!(
                    "function render(){{return h('column',{{style:{{textAlign:'{alignment}'}}}},h('textarea',{{value:'{value}',style:{{width:160,height:100,padding:8,borderWidth:2}}}}))}}"
                ));
                let mut page = crate::ViewSession::new(&source).unwrap();
                let display = page.layout(200, 160, 1.0).unwrap();
                let layout = &display.retained.inputs[1].as_ref().unwrap().layout;
                assert_eq!(layout.len(), 2);
                assert_eq!(layout.is_rtl(), rtl);
                let fraction = match alignment {
                    "center" => 0.5,
                    "right" => 1.0,
                    "left" => 0.0,
                    "end" => {
                        if rtl {
                            0.0
                        } else {
                            1.0
                        }
                    }
                    _ => {
                        if rtl {
                            1.0
                        } else {
                            0.0
                        }
                    }
                };
                for line in layout.lines() {
                    let metrics = line.metrics();
                    assert!(
                        (metrics.offset - (140.0 - metrics.advance) * fraction).abs() < 0.02,
                        "{alignment}, rtl={rtl}: {metrics:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn aligned_single_line_editors_keep_caret_and_hit_geometry_together() {
        for tag in ["input", "search"] {
            for alignment in ["left", "center", "right"] {
                for value in ["MMM", "MMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMM"] {
                    let source = crate::session::tests::source(&format!(
                        "function render(){{return h('{tag}',{{autoFocus:true,value:'{value}',style:{{width:180,height:56,padding:8,borderWidth:2,textAlign:'{alignment}'}}}})}}"
                    ));
                    for scale in [1.0, 2.0] {
                        let mut page = crate::ViewSession::new(&source).unwrap();
                        let display = page
                            .prepare_native(
                                (200.0 * scale) as u16,
                                (100.0 * scale) as u16,
                                scale,
                                true,
                                None,
                                Ok,
                            )
                            .unwrap();
                        let node = &display.nodes[0];
                        let layout = &display.retained.inputs[0].as_ref().unwrap().layout;
                        assert_eq!(layout.len(), 1);
                        let input = node.input.as_ref().unwrap();
                        let geometry = input.geometry.as_ref().unwrap();
                        let caret = geometry.caret;
                        assert!(caret[0] >= geometry.content_bounds[0] - 0.1);
                        assert!(
                            caret[0] <= geometry.content_bounds[0] + geometry.content_bounds[2]
                        );
                        assert_eq!(
                            page.hit_test_input(
                                page.generation(),
                                &node.identity,
                                [caret[0], caret[1] + caret[3] / 2.0],
                                false
                            )
                            .unwrap(),
                            Some([value.len() as u32, value.len() as u32])
                        );
                        if value.len() == 3 {
                            let offset = layout.lines().next().unwrap().metrics().offset;
                            // Right-aligned text still reserves the visible caret width.
                            assert!(input.scroll_offset[0] <= 1.5);
                            if alignment == "left" {
                                assert_eq!(offset, 0.0);
                            } else {
                                assert!(offset > 20.0);
                            }
                        } else {
                            assert!(input.scroll_offset[0] > 0.0);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn word_break_wraps_inherited_text_and_keeps_cached_modes_distinct() {
        for (property, value) in [
            ("wordBreak", "break-all"),
            ("overflowWrap", "anywhere"),
            ("overflowWrap", "break-word"),
        ] {
            let source = crate::session::tests::source(
            "function render(){const wrap=useLocal('wrap',false);return h('column',{style:{width:320,padding:20,wordBreak:wrap.value?'break-all':'normal'}},h('row',{style:{gap:12}},h('text',{style:{fontSize:14}},'IPv6'),h('column',{testId:'cell',style:{grow:1,minWidth:0,align:'end'}},h('text',{testId:'address',style:{fontSize:14,padding:12}},'2001:db8:1234:5678:90ab:cdef:1234:5678'))),h('button',{testId:'toggle',onClick:()=>wrap.set(!wrap.value)},'Toggle'))}".replace("wordBreak", property).replace("break-all", value).as_str(),
        );
            for scale in [1.0, 2.0] {
                let mut page = crate::ViewSession::new(&source).unwrap();
                let mut normal_height = 0.0;
                for wrapped in [false, true, false, true] {
                    let display = page.layout(320, 600, scale).unwrap();
                    let address = display
                        .nodes
                        .iter()
                        .find(|node| node.test_id == "address")
                        .unwrap();
                    let cell = display
                        .nodes
                        .iter()
                        .find(|node| node.test_id == "cell")
                        .unwrap();
                    assert_eq!(address.text, "2001:db8:1234:5678:90ab:cdef:1234:5678");
                    if wrapped {
                        assert!(
                            address.bounds[2] <= cell.bounds[2] + 0.1,
                            "address {:?}, cell {:?}",
                            address.bounds,
                            cell.bounds
                        );
                        assert!(address.bounds[3] > normal_height);
                    } else {
                        normal_height = address.bounds[3];
                    }
                    let callback = display
                        .nodes
                        .iter()
                        .find(|node| node.test_id == "toggle")
                        .unwrap()
                        .on_click
                        .unwrap();
                    display.rasterize().unwrap();
                    page.click(page.generation(), callback).unwrap();
                }
            }
        }
    }

    #[test]
    fn composed_button_lays_out_content_and_groups_semantics_at_each_scale() {
        for scale in [1.0, 2.0] {
            let module = crate::session::tests::source(
                "function render(){const n=useLocal('n',0);return h('column',{},h('button',{testId:'action',accessibilityLabel:'Open inbox',style:{padding:16,width:240,background:'#ffffff'},onClick:()=>n.set(n.value+1)},h('column',{style:{gap:4}},h('text',{testId:'title',style:{fontSize:18}},'Inbox'),h('text',{testId:'description',style:{fontSize:12}},'Unread '+n.value))))}",
            );
            let mut page = crate::ViewSession::new(&module).unwrap();
            let first = page.layout(300, 200, scale).unwrap();
            let button = first
                .nodes
                .iter()
                .find(|node| node.test_id == "action")
                .unwrap();
            let title = first
                .nodes
                .iter()
                .find(|node| node.test_id == "title")
                .unwrap();
            let description = first
                .nodes
                .iter()
                .find(|node| node.test_id == "description")
                .unwrap();
            assert_eq!(button.text, "Open inbox");
            assert_eq!(button.accessibility_hidden, None);
            assert_eq!(title.accessibility_hidden, Some(true));
            assert_eq!(description.accessibility_hidden, Some(true));
            assert_eq!(title.bounds[0], button.bounds[0] + 16.0);
            assert!(description.bounds[1] >= title.bounds[1] + title.bounds[3]);
            assert!(
                description.bounds[1] + description.bounds[3]
                    <= button.bounds[1] + button.bounds[3]
            );
            let old_content = button.button_content.clone();
            let old_bounds = button.bounds;
            let generation = page.generation();
            let callback = button.on_click.unwrap();
            first.rasterize().unwrap();
            assert!(page.click(generation, callback).unwrap());
            let second = page.layout(300, 200, scale).unwrap();
            let changed = second
                .nodes
                .iter()
                .find(|node| node.test_id == "action")
                .unwrap();
            assert_ne!(
                old_content, changed.button_content,
                "child changes invalidate gesture rebinding"
            );
            assert_eq!(changed.bounds, old_bounds);
            assert!(!page.click(generation, callback).unwrap());
        }
    }

    #[test]
    fn composed_checkbox_lays_out_content_and_groups_semantics_at_each_scale() {
        for scale in [1.0, 2.0] {
            let module = crate::session::tests::source(
                "function render(){const n=useLocal('n',0);return h('column',{},h('checkbox',{value:n.value%2===1,testId:'action',accessibilityLabel:'Open inbox',style:{padding:16,width:240,background:'#ffffff'},onChange:()=>n.set(n.value+1)},h('column',{style:{gap:4}},h('text',{testId:'title',style:{fontSize:18}},'Inbox'),h('text',{testId:'description',style:{fontSize:12}},'Unread '+n.value))))}",
            );
            let mut page = crate::ViewSession::new(&module).unwrap();
            let first = page.layout(300, 200, scale).unwrap();
            let button = first
                .nodes
                .iter()
                .find(|node| node.test_id == "action")
                .unwrap();
            let title = first
                .nodes
                .iter()
                .find(|node| node.test_id == "title")
                .unwrap();
            let description = first
                .nodes
                .iter()
                .find(|node| node.test_id == "description")
                .unwrap();
            assert_eq!(button.text, "Open inbox");
            assert_eq!(button.checked, Some(false));
            assert_eq!(button.accessibility_hidden, None);
            assert_eq!(title.accessibility_hidden, Some(true));
            assert_eq!(description.accessibility_hidden, Some(true));
            assert_eq!(title.bounds[0], button.bounds[0] + 16.0);
            assert!(description.bounds[1] >= title.bounds[1] + title.bounds[3]);
            assert!(
                description.bounds[1] + description.bounds[3]
                    <= button.bounds[1] + button.bounds[3]
            );
            let old_content = button.button_content.clone();
            let old_bounds = button.bounds;
            let generation = page.generation();
            let callback = button.on_click.unwrap();
            first.rasterize().unwrap();
            assert!(page.click(generation, callback).unwrap());
            let second = page.layout(300, 200, scale).unwrap();
            let changed = second
                .nodes
                .iter()
                .find(|node| node.test_id == "action")
                .unwrap();
            assert_ne!(
                old_content, changed.button_content,
                "child changes invalidate gesture rebinding"
            );
            assert_eq!(changed.checked, Some(true));
            assert_eq!(changed.bounds, old_bounds);
            assert!(!page.click(generation, callback).unwrap());
        }
    }

    #[test]
    fn subtree_opacity_composites_overlap_once_and_keeps_sibling_groups_separate() {
        let module = crate::session::tests::source(
            "function render(){return h('stack',{style:{width:100,height:100,background:'#ffffff'}},h('stack',{style:{width:80,height:80,opacity:0.5,background:'#ff0000'}},h('spacer',{style:{width:60,height:60,background:'#0000ff'}})),h('spacer',{style:{offset:{x:40,y:40},width:40,height:40,opacity:0.5,background:'#00ff00'}}))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let display = page.layout(100, 100, 1.0).unwrap();
        let retained = display.retained.clone();
        let frame = display.rasterize().unwrap();
        let pixel = |x: usize, y: usize| {
            &frame.pixels.data_as_u8_slice()[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4]
        };
        assert_eq!(
            pixel(10, 10),
            [127, 127, 255, 255],
            "blue covers red before group blending"
        );
        assert_eq!(pixel(70, 10), [255, 127, 127, 255]);
        let overlap = pixel(50, 50);
        assert!(
            overlap[0] >= 63
                && overlap[0] <= 65
                && overlap[1] >= 191
                && overlap[1] <= 193
                && overlap[2] >= 127
                && overlap[2] <= 129
        );
        let recomposed = retained
            .compose(&HashMap::new())
            .unwrap()
            .rasterize()
            .unwrap();
        assert_eq!(
            frame.pixels.data_as_u8_slice(),
            recomposed.pixels.data_as_u8_slice()
        );
    }

    #[test]
    fn percentage_growth_cannot_publish_unbounded_geometry() {
        let source = crate::session::tests::source(
            "function render(){let child=h('text',{},'value');for(let i=0;i<12;i++)child=h('column',{style:{width:'16384%'}},child);return h('column',{style:{width:300}},child)}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        assert!(
            page.layout(300, 300, 1.0)
                .err()
                .unwrap()
                .contains("coordinate budget")
        );
    }

    #[test]
    fn vector_paths_inherit_color_fit_clip_and_move_with_retained_scroll() {
        let source = crate::session::tests::source(
            "function render(){return h('column',{style:{background:'#ffffff',color:'#ff0000'}},h('scroll',{style:{height:40}},h('vector',{testId:'vector',viewBox:[0,0,24,24],paths:[{d:'M-12 -12H36V36H-12z'}],style:{width:80,height:80,padding:10,shrink:0}}),h('spacer',{style:{height:80,shrink:0}})))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(100, 100, 1.0).unwrap();
        let scroll = display
            .nodes
            .iter()
            .find_map(|n| n.scroll.as_ref())
            .unwrap()
            .id
            .clone();
        let shifted = display
            .retained
            .compose(&HashMap::from([(scroll, 30.0)]))
            .unwrap();
        let frame = shifted.rasterize().unwrap();
        let pixel = |x: usize, y: usize| {
            &frame.pixels.data_as_u8_slice()[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4]
        };
        assert_eq!(pixel(5, 10), [255, 255, 255, 255]);
        assert_eq!(pixel(15, 10), [255, 0, 0, 255]);
        assert_eq!(pixel(75, 10), [255, 255, 255, 255]);
        assert_eq!(pixel(15, 45), [255, 255, 255, 255]);
    }

    #[test]
    fn vector_fill_and_butt_strokes_preserve_holes_and_device_scale() {
        let source = crate::session::tests::source(
            "function render(){return h('column',{style:{background:'#ffffff'}},h('vector',{viewBox:[0,0,24,24],paths:[{d:'M2 2h20v20H2z',fill:'none',stroke:'#0000ff',strokeWidth:2}],style:{width:48,height:48}}))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        for scale in [1.0, 2.0] {
            let frame = page.layout(100, 100, scale).unwrap().rasterize().unwrap();
            let pixel = |x: usize, y: usize| {
                let x = (x as f32 * scale) as usize;
                let y = (y as f32 * scale) as usize;
                &frame.pixels.data_as_u8_slice()[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4]
            };
            assert_eq!(pixel(4, 24), [0, 0, 255, 255]);
            assert_eq!(pixel(24, 24), [255, 255, 255, 255]);
            assert_eq!(pixel(0, 24), [255, 255, 255, 255]);
        }
    }

    #[test]
    fn vector_transforms_cannot_expand_bounded_paths_past_coordinate_budget() {
        let source = crate::session::tests::source(
            "function render(){let child=h('vector',{viewBox:[0,0,.015625,.015625],paths:[{d:'M0 0H16000V16000H0z'}],style:{width:'100%',height:'100%'}});for(let i=0;i<3;i++)child=h('column',{style:{width:'16384%',height:'16384%'}},child);return h('column',{style:{width:300,height:300}},child)}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        assert!(
            page.layout(300, 300, 1.0)
                .err()
                .unwrap()
                .contains("vector geometry exceeds coordinate budget")
        );
    }

    #[test]
    fn percentage_dimensions_follow_parent_content_boxes_and_scale() {
        let source = crate::session::tests::source(
            "function render(){return h('column',{style:{width:'100%',height:'100%',padding:20,borderWidth:5}},h('column',{testId:'half',style:{width:'50%',height:'50%',background:'#ff0000'}}))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        for (width, height, scale) in [(300, 200, 1.0), (600, 400, 2.0), (500, 300, 1.0)] {
            let display = page.layout(width, height, scale).unwrap();
            let half = display.nodes.iter().find(|n| n.test_id == "half").unwrap();
            let expected = [
                25.0,
                25.0,
                (width as f32 / scale - 50.0) / 2.0,
                (height as f32 / scale - 50.0) / 2.0,
            ];
            for (actual, expected) in half.bounds.into_iter().zip(expected) {
                assert!((actual - expected).abs() < 0.01, "{actual} vs {expected}");
            }
        }
    }

    #[test]
    fn percentage_max_width_wraps_text_and_min_width_overrides_a_smaller_width() {
        let source = crate::session::tests::source(
            "function render(){return h('column',{style:{width:'100%',padding:20,align:'end'}},h('text',{testId:'address',style:{maxWidth:'72%',wordBreak:'break-all',fontSize:16}},'2001:db8:1234:5678:90ab:cdef:1234:5678'),h('column',{testId:'limited',style:{width:'10%',minWidth:'50%',maxWidth:'80%',height:10}}))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let mut heights = Vec::new();
        for width in [200, 400] {
            let display = page.layout(width, 400, 1.0).unwrap();
            let content = width as f32 - 40.0;
            let address = display
                .nodes
                .iter()
                .find(|n| n.test_id == "address")
                .unwrap();
            assert!(
                address.bounds[2] <= content * 0.72 + 0.01,
                "{:?}",
                address.bounds
            );
            assert!((address.bounds[0] + address.bounds[2] - (width as f32 - 20.0)).abs() < 0.01);
            heights.push(address.bounds[3]);
            let limited = display
                .nodes
                .iter()
                .find(|n| n.test_id == "limited")
                .unwrap();
            assert!((limited.bounds[2] - content * 0.5).abs() < 0.01);
        }
        assert!(heights[0] > heights[1]);
    }

    #[test]
    fn outer_shadows_knock_out_transparent_owners_and_preserve_layer_order() {
        let source = crate::session::tests::source(
            "function render(){return h('stack',{style:{width:150,height:100,background:'#ffffff'}},h('column',{testId:'owner',style:{width:80,height:40,offset:{x:20,y:20},boxShadow:[{offsetX:10,color:'#ff000080'},{offsetX:15,color:'#0000ff'}]}}))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(150, 100, 1.0).unwrap();
        let owner = display.nodes.iter().find(|n| n.test_id == "owner").unwrap();
        assert_eq!(owner.bounds, [20.0, 20.0, 80.0, 40.0]);
        let frame = display.rasterize().unwrap();
        let sample = |x: usize, y: usize| {
            &frame.pixels.data_as_u8_slice()[(y * 150 + x) * 4..(y * 150 + x) * 4 + 4]
        };
        assert_eq!(sample(50, 40), [255, 255, 255, 255]);
        assert_eq!(sample(105, 40), [128, 0, 127, 255]);
        assert_eq!(sample(113, 40), [0, 0, 255, 255]);
        assert_eq!(sample(116, 40), [255, 255, 255, 255]);
    }

    #[test]
    fn equal_corner_overrides_keep_the_image_clip_and_uniform_fast_path() {
        let source = crate::session::tests::source(
            "function render(){return h('image',{src:'photo',style:{width:100,height:80,radiusTopLeft:8,radiusTopRight:8,radiusBottomRight:8,radiusBottomLeft:8}})}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(100, 80, 1.0).unwrap();
        assert_eq!(display.nodes[0].radius, 8.0);
        assert!(display.nodes[0].radii.is_none());
        assert!(
            display
                .commands
                .iter()
                .any(|paint| matches!(paint,Paint::Image {radius,..} if *radius==8.0))
        );
    }
    #[test]
    fn authored_corner_styles_render_shadow_and_reuse_mask_without_external_images() {
        let source = crate::session::tests::source(
            "function render(){return h('column',{style:{background:'#ffffff'}},h('column',{testId:'bubble',style:{width:100,height:60,radius:24,radiusBottomRight:6,background:'#0000ff',boxShadow:[{blur:8,color:'#00000080'}]}}))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let mut first = page.layout(140, 100, 1.0).unwrap();
        assert_eq!(
            first.nodes[1].radii,
            Some([[24.0; 2], [24.0; 2], [6.0; 2], [24.0; 2]])
        );
        assert_eq!(first.image_sources().count(), 0);
        first.bind_images(&HashMap::new()).unwrap();
        let masks: Vec<_> = first.bitmaps().map(|b| b.pixels().clone()).collect();
        assert_eq!(masks.len(), 1);
        let next = page.layout(140, 100, 1.0).unwrap();
        assert!(Arc::ptr_eq(
            &masks[0],
            next.bitmaps().next().unwrap().pixels()
        ));
        let frame = first.rasterize().unwrap();
        let pixels = frame.pixels.data_as_u8_slice();
        let pixel = |x: usize, y: usize| &pixels[(y * 140 + x) * 4..(y * 140 + x) * 4 + 4];
        assert_eq!(pixel(97, 57), &[0, 0, 255, 255]);
        assert!(
            pixel(101, 59)[0] < pixel(101, 1)[0],
            "small corner casts a larger nearby shadow"
        );
    }
    #[test]
    fn independent_corner_snapshot_controls_editor_hits_and_scroll_clips() {
        let source = crate::session::tests::source(
            "function render(){return h('column',{},h('scroll',{style:{width:100,height:60,radius:24}},h('input',{value:'ABCD',style:{width:100,height:60,radius:0,shrink:0}}),h('spacer',{style:{height:100,shrink:0}})))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(120, 100, 1.0).unwrap();
        let corners = Some([[24.0; 2], [24.0; 2], [6.0; 2], [24.0; 2]]);
        let mut retained = display.retained.as_ref().clone();
        retained.nodes[1].radii = corners;
        let mut display = Arc::new(retained).compose(&HashMap::new()).unwrap();
        assert_eq!(display.nodes[2].clips[0].radii, corners);
        let identity = display.nodes[2].identity.clone();
        let hits = InputHitMap::capture(&display);
        assert!(hits.hit(&identity, [97.0, 2.0], false).is_none());
        assert!(hits.hit(&identity, [97.0, 57.0], false).is_some());
        display.nodes[2].clips.clear();
        display.nodes[2].radii = corners;
        let hits = InputHitMap::capture(&display);
        assert!(hits.hit(&identity, [97.0, 2.0], false).is_none());
        assert!(hits.hit(&identity, [97.0, 57.0], false).is_some());
    }
    #[test]
    fn independent_corner_paints_survive_retained_scroll_and_keep_contours() {
        for paint_style in [
            "background:'#0000ff'",
            "backgroundGradient:{direction:'bottom',stops:[{offset:0,color:'#0000ff'},{offset:1,color:'#0000ff'}]}",
            "borderWidth:4,borderColor:'#0000ff'",
        ] {
            let source = crate::session::tests::source(&format!(
                "function render(){{return h('column',{{style:{{background:'#ffffff'}}}},h('scroll',{{style:{{width:100,height:80}}}},h('column',{{style:{{width:100,height:60,shrink:0,radius:24,{paint_style}}}}}),h('spacer',{{style:{{height:100,shrink:0}}}})))}}"
            ));
            let mut page = crate::ViewSession::new(&source).unwrap();
            let display = page.layout(120, 100, 1.0).unwrap();
            let mut retained = display.retained.as_ref().clone();
            let mut changed = 0;
            for paint in &mut retained.commands[retained.ranges[2].clone()] {
                match paint {
                    Paint::Rect { corners, .. }
                    | Paint::Gradient { corners, .. }
                    | Paint::Border { corners, .. } => {
                        *corners = Some([[24.0; 2], [24.0; 2], [6.0; 2], [24.0; 2]]);
                        changed += 1;
                    }
                    _ => {}
                }
            }
            assert!(changed > 0);
            let id = retained.nodes[1].scroll.as_ref().unwrap().id.clone();
            let retained = Arc::new(retained);
            for offset in [0, 10] {
                let frame = retained
                    .compose(&HashMap::from([(id.clone(), offset as f32)]))
                    .unwrap()
                    .rasterize()
                    .unwrap();
                let pixels = frame.pixels.data_as_u8_slice();
                let pixel = |x: usize, y: usize| &pixels[(y * 120 + x) * 4..(y * 120 + x) * 4 + 4];
                assert_eq!(
                    pixel(97, 57 - offset),
                    &[0, 0, 255, 255],
                    "small bottom-right corner: {paint_style}"
                );
                if offset == 0 {
                    assert_eq!(
                        pixel(97, 2),
                        &[255, 255, 255, 255],
                        "large top-right corner"
                    );
                }
            }
        }
    }
    #[test]
    fn blurred_shadows_fade_outside_input_clips() {
        for tag in ["column", "input"] {
            let source = crate::session::tests::source(&format!(
                "function render(){{return h('stack',{{style:{{width:150,height:100,background:'#ffffff'}}}},h('{tag}',{{style:{{width:80,height:40,radius:0,offset:{{x:20,y:20}},boxShadow:[{{blur:10,color:'#000000'}}]}}}}))}}"
            ));
            let mut page = crate::ViewSession::new(&source).unwrap();
            let frame = page.layout(150, 100, 1.0).unwrap().rasterize().unwrap();
            let sample = |x: usize| frame.pixels.data_as_u8_slice()[(40 * 150 + x) * 4];
            assert!(
                sample(101) > 120 && sample(101) < 180,
                "{tag}: {}",
                sample(101)
            );
            assert!(
                sample(101) < sample(105) && sample(105) < sample(110),
                "{tag}"
            );
            assert_eq!(sample(114), 255);
        }
    }

    #[test]
    fn offscreen_owner_shadow_remains_visible_without_increasing_scroll_extent() {
        let source = crate::session::tests::source(
            "function render(){return h('column',{style:{width:100,height:100,background:'#ffffff'}},h('scroll',{testId:'scroll',style:{height:60}},h('column',{testId:'owner',style:{width:40,height:20,shrink:0,boxShadow:[{offsetY:40,color:'#ff0000'}]}}),h('spacer',{style:{height:100,shrink:0}})))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(100, 100, 1.0).unwrap();
        let scroll = display
            .nodes
            .iter()
            .find_map(|n| n.scroll.as_ref())
            .unwrap();
        assert_eq!(scroll.max_offset, 60.0);
        let shifted = display
            .retained
            .compose(&HashMap::from([(scroll.id.clone(), 30.0)]))
            .unwrap();
        assert_eq!(
            shifted
                .nodes
                .iter()
                .find(|n| n.test_id == "owner")
                .unwrap()
                .visible_bounds[3],
            0.0
        );
        let frame = shifted.rasterize().unwrap();
        assert_eq!(
            &frame.pixels.data_as_u8_slice()[(15 * 100 + 10) * 4..(15 * 100 + 10) * 4 + 4],
            [255, 0, 0, 255]
        );
        assert_eq!(
            &frame.pixels.data_as_u8_slice()[(65 * 100 + 10) * 4..(65 * 100 + 10) * 4 + 4],
            [255, 255, 255, 255]
        );
    }

    #[test]
    fn linear_gradient_preserves_alpha_radius_borders_and_scroll_coordinates() {
        let module = crate::session::tests::source(
            "function render(){return h('column',{style:{width:100,height:100,background:'#ffffff'}},h('scroll',{testId:'scroll',style:{width:80,height:40}},h('column',{style:{width:80,height:80,shrink:0,radius:12,borderWidth:4,borderColor:'#00ff00',backgroundGradient:{direction:'bottom',stops:[{offset:0,color:'#ff000080'},{offset:1,color:'#0000ff80'}]}}})),h('spacer',{style:{height:20}}))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let display = page.layout(100, 100, 1.0).unwrap();
        let scroll = display
            .nodes
            .iter()
            .find_map(|n| n.scroll.as_ref())
            .unwrap()
            .id
            .clone();
        let shifted = display
            .retained
            .compose(&HashMap::from([(scroll, 20.0)]))
            .unwrap();
        let before = display.rasterize().unwrap();
        let after = shifted.rasterize().unwrap();
        let sample = |frame: &Frame, x: usize, y: usize| {
            frame.pixels.data_as_u8_slice()[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4].to_vec()
        };
        assert_eq!(sample(&before, 0, 0), [255, 255, 255, 255]);
        assert_eq!(sample(&before, 40, 1), [0, 255, 0, 255]);
        assert_eq!(sample(&before, 40, 45), [255, 255, 255, 255]);
        assert_eq!(sample(&after, 40, 45), [255, 255, 255, 255]);
        let original = sample(&before, 40, 30);
        assert_eq!(original, sample(&after, 40, 10));
        assert!(original[0] > original[2] && original[1] >= 126 && original[1] <= 128);
    }

    #[test]
    fn gradient_interpolation_space_and_premultiplied_alpha_affect_pixels() {
        let sample = |space: &str, first: &str, last: &str| {
            let render = format!(
                "function render(){{return h('column',{{style:{{width:101,height:20,background:'#000000',backgroundGradient:{{direction:'right',colorSpace:'{space}',stops:[{{offset:0,color:'{first}'}},{{offset:1,color:'{last}'}}]}}}}}})}}"
            );
            let mut page =
                crate::ViewSession::new(&crate::session::tests::source(&render)).unwrap();
            let frame = page.layout(101, 20, 1.0).unwrap().rasterize().unwrap();
            frame.pixels.data_as_u8_slice()[(10 * 101 + 50) * 4..(10 * 101 + 50) * 4 + 4].to_vec()
        };
        let srgb = sample("srgb", "#ff0000", "#0000ff");
        let oklab = sample("oklab", "#ff0000", "#0000ff");
        assert!((srgb[0] as i32 - 128).abs() <= 2 && (srgb[2] as i32 - 128).abs() <= 2);
        assert!(oklab[1] > 20, "{oklab:?}");
        let alpha = sample("srgb", "#ff000000", "#0000ffff");
        assert_eq!(alpha[0], 0, "{alpha:?}");
        assert!((alpha[2] as i32 - 128).abs() <= 2, "{alpha:?}");
    }

    #[test]
    fn nested_opacity_and_scroll_clipping_preserve_group_boundaries() {
        let module = crate::session::tests::source(
            "function render(){return h('column',{style:{width:100,height:100,background:'#ffffff'}},h('scroll',{testId:'scroll',style:{width:80,height:40,opacity:0.5}},h('column',{style:{opacity:0.5}},h('spacer',{style:{width:80,height:80,background:'#000000'}}))),h('spacer',{style:{width:100,height:20,background:'#ff0000'}}))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let display = page.layout(100, 100, 1.0).unwrap();
        let scroll = display
            .nodes
            .iter()
            .find_map(|node| node.scroll.as_ref())
            .unwrap()
            .id
            .clone();
        let shifted = display
            .retained
            .compose(&HashMap::from([(scroll, 20.0)]))
            .unwrap();
        for frame in [display.rasterize().unwrap(), shifted.rasterize().unwrap()] {
            let bytes = frame.pixels.data_as_u8_slice();
            assert_eq!(
                &bytes[(10 * 100 + 10) * 4..(10 * 100 + 10) * 4 + 4],
                [191, 191, 191, 255]
            );
            assert_eq!(
                &bytes[(50 * 100 + 10) * 4..(50 * 100 + 10) * 4 + 4],
                [255, 0, 0, 255]
            );
            assert_eq!(
                &bytes[(70 * 100 + 10) * 4..(70 * 100 + 10) * 4 + 4],
                [255, 255, 255, 255]
            );
        }
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn scene_key_compares_paint_content_and_not_semantic_metadata() {
        use super::*;
        let module = crate::session::tests::source(
            "function render(){return h('scroll',{style:{height:60,radius:12,background:'#ffffff'}},h('text',{},'Hello world'),h('spacer',{style:{height:200}}))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let mut display = page.layout(240, 100, 1.0).unwrap();
        let key = SceneKey::capture(&display).unwrap();
        display.nodes[0].test_id = "new-semantics".into();
        assert!(key.matches(&display));
        for index in 0..display.commands.len() {
            let original = display.commands[index].clone();
            let changed = match &mut display.commands[index] {
                Paint::Rect { bounds, .. } => {
                    bounds[0] += 1.0;
                    true
                }
                Paint::Clip(clip) => {
                    clip.radius += 1.0;
                    true
                }
                Paint::Text { glyphs, .. } if !glyphs.is_empty() => {
                    glyphs[0].x += 1.0;
                    true
                }
                _ => false,
            };
            if changed {
                assert!(!key.matches(&display));
            }
            display.commands[index] = original;
        }
        let text = display
            .commands
            .iter()
            .position(|p| matches!(p, Paint::Text { .. }))
            .unwrap();
        let original = display.commands[text].clone();
        for change in 0..4 {
            if let Paint::Text {
                color,
                font,
                size,
                coordinates,
                ..
            } = &mut display.commands[text]
            {
                match change {
                    0 => color[0] ^= 1,
                    1 => font.index += 1,
                    2 => *size += 1.0,
                    _ => coordinates.push(1),
                }
            }
            assert!(!key.matches(&display));
            display.commands[text] = original.clone();
        }
        display.scale = 2.0;
        assert!(!key.matches(&display));
        display.scale = 1.0;
        display.width += 1;
        assert!(!key.matches(&display));
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn scene_key_tracks_immutable_image_identity_and_input_bounds() {
        use super::*;
        use ::image::ImageEncoder;
        let mut png = Vec::new();
        ::image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&[255, 0, 0, 255], 1, 1, ::image::ExtendedColorType::Rgba8)
            .unwrap();
        let bitmap = crate::image::Bitmap::decode(&png).unwrap();
        let module = crate::session::tests::source(
            "function render(){return h('image',{src:'public/a.png',style:{width:20,height:20}})}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let mut display = page.layout(100, 100, 1.0).unwrap();
        display.images.insert("public/a.png".into(), bitmap.clone());
        let key = SceneKey::capture(&display).unwrap();
        display.images.insert("public/a.png".into(), bitmap);
        assert!(key.matches(&display));
        display.images.insert(
            "public/a.png".into(),
            crate::image::Bitmap::decode(&png).unwrap(),
        );
        assert!(
            !key.matches(&display),
            "new pixels under the same source invalidate the scene"
        );
        display.images.clear();
        assert!(!key.matches(&display));
        let command = display.commands[0].clone();
        display.commands.resize(257, command);
        assert!(SceneKey::capture(&display).is_none());
    }

    #[test]
    fn search_keeps_icon_fixed_while_text_selection_and_caret_scroll() {
        use crate::text_input::Edit;
        let module = crate::session::tests::source(
            "function render(){return h('search',{value:'A long search query that must scroll',autoFocus:true,style:{fontSize:18,background:'#ffffff',color:'#18243a'}})}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let first = page.layout(180, 80, 1.0).unwrap();
        let input = first.nodes[0].input.as_ref().unwrap();
        assert!(input.scroll_offset[0] > 0.0);
        assert!(input.geometry.as_ref().unwrap().caret[0] >= 44.0);
        let token = input.edit_token.unwrap();
        let id = first.nodes[0].identity.clone();
        let end = input.edit.as_ref().unwrap().selection[1];
        let baseline = first.rasterize().unwrap().pixels;
        page.edit_input(
            token,
            &id,
            &[Edit::Selection {
                anchor: end,
                focus: 0,
            }],
            false,
        )
        .unwrap()
        .unwrap();
        let selected = page.prepare_native(180, 80, 1.0, false, None, Ok).unwrap();
        let input = selected.nodes[0].input.as_ref().unwrap();
        assert_eq!(input.scroll_offset[0], 0.0);
        assert_eq!(input.geometry.as_ref().unwrap().caret[0], 44.0);
        let hits = super::InputHitMap::capture(&selected);
        let pixels = selected.rasterize().unwrap().pixels;
        let mut ink = 0;
        for y in 0..48 {
            for x in 0..44 {
                let offset = (y * 180 + x) * 4;
                let before = &baseline.data_as_u8_slice()[offset..offset + 4];
                assert_eq!(before, &pixels.data_as_u8_slice()[offset..offset + 4]);
                if x >= 12 && x < 36 && y >= 12 && y < 36 && before != [255, 255, 255, 255] {
                    ink += 1;
                }
            }
        }
        assert!(
            ink > 30,
            "magnifier must be visible outside the editable content clip"
        );
        assert_eq!(hits.hit(&id, [15.0, 24.0], false), Some([0, 0]));
    }

    #[test]
    fn letter_spacing_inherits_resets_and_changes_input_geometry() {
        let module = crate::session::tests::source(
            "function render(){return h('column',{style:{fontFamily:'mono',fontSize:20,letterSpacing:4}},h('input',{value:'ABCD',style:{height:50}}),h('input',{value:'ABCD',style:{height:50,letterSpacing:0}}),h('input',{value:'ABCD',style:{height:50,letterSpacing:-1}}))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let frame = page.layout(320, 200, 1.0).unwrap();
        let widths: Vec<_> = [1, 2, 3]
            .map(|i| frame.retained.inputs[i].as_ref().unwrap().layout.width())
            .to_vec();
        assert!(widths[0] > widths[1] + 10.0);
        assert!(widths[2] < widths[1] - 2.0);
    }
    #[test]
    fn letter_spacing_changes_wrapping_without_stale_cached_layouts() {
        let module = crate::session::tests::source(
            "function render(v){return h('column',{},h('text',{style:{width:80,fontFamily:'mono',fontSize:16,wordBreak:'break-all',letterSpacing:v.data.spacing}},'ABCDEFGH'))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let mut heights = Vec::new();
        for spacing in [0, 8, 0] {
            page.receive_state(&serde_json::json!({"v":2,"kind":"state.snapshot","rev":heights.len(),"state":{"spacing":spacing}}))
                .unwrap();
            let frame = page.layout(200, 200, 1.0).unwrap();
            heights.push(frame.nodes[1].bounds[3]);
        }
        assert!(heights[1] > heights[0]);
        assert_eq!(heights[2], heights[0]);
    }
    #[test]
    fn typography_inherits_family_and_absolute_line_height_with_local_overrides() {
        let module = crate::session::tests::source(
            "function render(){return h('column',{style:{fontFamily:'mono',fontSize:18,lineHeight:36}},h('input',{value:'iiii',style:{height:70}}),h('input',{value:'WWWW',style:{height:70}}),h('input',{value:'WWWW',style:{height:70,fontFamily:'sans',lineHeight:28}}))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let frame = page.layout(320, 240, 1.0).unwrap();
        let a = &frame.retained.inputs[1].as_ref().unwrap().layout;
        let b = &frame.retained.inputs[2].as_ref().unwrap().layout;
        let c = &frame.retained.inputs[3].as_ref().unwrap().layout;
        assert!(
            (a.width() - b.width()).abs() < 0.01,
            "monospaced advances must match"
        );
        assert_eq!(a.height(), 36.0);
        assert_eq!(b.height(), 36.0);
        assert_eq!(c.height(), 28.0);
        assert!(
            (c.width() - b.width()).abs() > 1.0,
            "local sans override must select a different face"
        );
    }

    #[test]
    fn synthetic_italic_leans_right_and_ink_bounds_contain_pixels() {
        let module = crate::session::tests::source(
            "function render(){return h('text',{style:{fontSize:48,padding:16,color:'#000000',background:'#ffffff'}},'IIII')}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let mut centers = Vec::new();
        for slope in [0.0, 12f64.to_radians().tan()] {
            let mut display = page.layout(240, 100, 1.0).unwrap();
            let mut bounds = None;
            for paint in &mut display.commands {
                if let Paint::Text {
                    font,
                    size,
                    coordinates,
                    glyphs,
                    skew,
                    ink,
                    ..
                } = paint
                {
                    *skew = slope;
                    *ink = glyph_ink(font, *size, coordinates, glyphs, slope as f32, 1.0);
                    bounds = *ink;
                }
            }
            let bounds = bounds.unwrap();
            let raster = display.rasterize().unwrap();
            let mut sum = 0.0;
            let mut count = 0.0;
            for (index, pixel) in raster.pixels.data_as_u8_slice().chunks_exact(4).enumerate() {
                if pixel[0] < 128 {
                    let x = (index % 240) as f32;
                    let y = (index / 240) as f32;
                    assert!(
                        x >= bounds[0]
                            && x <= bounds[0] + bounds[2]
                            && y >= bounds[1]
                            && y <= bounds[1] + bounds[3]
                    );
                    sum += x;
                    count += 1.0;
                }
            }
            assert!(count > 0.0);
            centers.push(sum / count);
        }
        assert!(
            centers[1] > centers[0] + 1.0,
            "synthetic italic must lean right: {centers:?}"
        );
    }

    #[test]
    fn italic_inherits_resets_and_does_not_reuse_upright_shaping() {
        let module = crate::session::tests::source(
            "function render(v){return h('column',{style:{background:'#ffffff',color:'#000000',fontSize:32,fontStyle:v.data.parent}},h('text',{style:v.data.child?{fontStyle:v.data.child}:{}},'Italic text ffi'))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let mut pixels = Vec::new();
        for (parent, child) in [
            ("normal", ""),
            ("italic", ""),
            ("italic", "normal"),
            ("normal", "italic"),
            ("normal", ""),
        ] {
            page.receive_state(&serde_json::json!({"v":2,"kind":"state.snapshot","rev":pixels.len(),"state":{"parent":parent,"child":child}}))
                .unwrap();
            pixels.push(
                page.layout(320, 100, 1.0)
                    .unwrap()
                    .rasterize()
                    .unwrap()
                    .pixels
                    .data_as_u8_slice()
                    .to_vec(),
            );
        }
        assert_ne!(pixels[0], pixels[1]);
        assert_eq!(pixels[0], pixels[2]);
        assert_eq!(pixels[1], pixels[3]);
        assert_eq!(pixels[0], pixels[4]);
    }

    #[test]
    fn font_weight_inherits_and_explicit_children_override_it() {
        let ink = |parent_weight, weight| {
            let weight = if weight == 0 {
                String::new()
            } else {
                format!("fontWeight:{weight},")
            };
            let module = crate::session::tests::source(&format!(
                "function render(){{return h('column',{{style:{{background:'#ffffff',color:'#000000',fontSize:32,fontWeight:{parent_weight}}}}},h('text',{{style:{{{weight}lineHeight:44}}}},'Hello world'))}}"
            ));
            let mut page = crate::ViewSession::new(&module).unwrap();
            let frame = page.layout(300, 80, 1.0).unwrap().rasterize().unwrap();
            frame
                .pixels
                .data_as_u8_slice()
                .chunks_exact(4)
                .map(|pixel| (255 - pixel[0]) as u64)
                .sum::<u64>()
        };
        let normal = ink(400, 400);
        assert_eq!(normal, ink(700, 400));
        assert_eq!(ink(400, 700), ink(700, 0));
        assert!(ink(400, 700) > normal, "bold glyphs must paint more ink");
    }

    #[test]
    fn styled_textarea_caret_and_hits_share_the_retained_line_spacing() {
        use crate::text_input::Edit;
        let module = crate::session::tests::source(
            "function render(){return h('textarea',{value:'one\\ntwo\\nthree',autoFocus:true,style:{height:160,padding:8,fontFamily:'mono',fontWeight:700,fontSize:18,lineHeight:36}})}",
        );
        for scale in [1.0, 2.0] {
            let mut page = crate::ViewSession::new(&module).unwrap();
            let first = page
                .prepare_native(
                    (300.0 * scale) as u16,
                    (200.0 * scale) as u16,
                    scale,
                    true,
                    None,
                    Ok,
                )
                .unwrap();
            let id = first.nodes[0].identity.clone();
            let token = first.nodes[0].input.as_ref().unwrap().edit_token.unwrap();
            page.edit_input(
                token,
                &id,
                &[Edit::Selection {
                    anchor: 4,
                    focus: 4,
                }],
                false,
            )
            .unwrap();
            let frame = page
                .prepare_native(
                    (300.0 * scale) as u16,
                    (200.0 * scale) as u16,
                    scale,
                    false,
                    None,
                    Ok,
                )
                .unwrap();
            let layout = &frame.retained.inputs[0].as_ref().unwrap().layout;
            assert_eq!(layout.height(), 108.0);
            let lines: Vec<_> = layout.lines().collect();
            assert!(
                (lines[1].metrics().baseline - lines[0].metrics().baseline - 36.0).abs() < 0.01
            );
            let caret = frame.nodes[0]
                .input
                .as_ref()
                .unwrap()
                .geometry
                .as_ref()
                .unwrap()
                .caret;
            assert_eq!(
                page.hit_test_input(
                    page.generation(),
                    &id,
                    [caret[0], caret[1] + caret[3] / 2.0],
                    false
                )
                .unwrap(),
                Some([4, 4])
            );
        }
    }

    #[test]
    fn automatic_textarea_height_tracks_wrapping_and_respects_limits() {
        let mut heights = Vec::new();
        for value in [
            "",
            "Hello",
            "A long sentence with several words that wraps over the available width.",
            &"More words to wrap. ".repeat(30),
        ] {
            let module = crate::session::tests::source(&format!(
                "function render(){{return h('column',{{style:{{width:180}}}},h('textarea',{{testId:'editor',value:{},style:{{height:'auto',minHeight:42,maxHeight:120,fontSize:14,lineHeight:21,padding:10}}}}))}}",
                serde_json::to_string(value).unwrap()
            ));
            let mut page = crate::ViewSession::new(&module).unwrap();
            let frame = page.layout(240, 400, 1.0).unwrap();
            heights.push(
                frame
                    .nodes
                    .iter()
                    .find(|n| n.test_id == "editor")
                    .unwrap()
                    .bounds[3],
            );
        }
        assert_eq!(heights[0], 42.0);
        assert_eq!(heights[1], 42.0);
        assert!(
            heights[2] > heights[1] && heights[2] <= 120.0,
            "{heights:?}"
        );
        assert_eq!(heights[3], 120.0);
    }

    #[test]
    fn padded_multiline_text_matches_equivalent_container_height() {
        let value = "A standalone window with no sidebar or shell. Desktop only.";
        for (scale, border) in [(1.0, 0), (2.0, 0), (1.0, 2), (2.0, 2)] {
            for width in 300..341 {
                let module = crate::session::tests::source(&format!(
                    "function render(){{return h('column',{{style:{{width:{width},gap:10}}}},h('text',{{testId:'padded',style:{{fontSize:12,lineHeight:20,borderWidth:{border},paddingLeft:12,paddingRight:12,paddingTop:8,paddingBottom:8}}}},'{value}'),h('column',{{testId:'wrapped',style:{{borderWidth:{border},paddingLeft:12,paddingRight:12,paddingTop:8,paddingBottom:8}}}},h('text',{{style:{{fontSize:12,lineHeight:20}}}},'{value}')))}}"
                ));
                let mut page = crate::ViewSession::new(&module).unwrap();
                let frame = page
                    .layout((420.0 * scale) as u16, (800.0 * scale) as u16, scale)
                    .unwrap();
                let padded = frame.nodes.iter().find(|n| n.test_id == "padded").unwrap();
                let wrapped = frame.nodes.iter().find(|n| n.test_id == "wrapped").unwrap();
                assert!(
                    (padded.bounds[3] - wrapped.bounds[3]).abs() <= 1.0,
                    "width {width}, scale {scale}, border {border}: padded {:?}, wrapped {:?}",
                    padded.bounds,
                    wrapped.bounds
                );
            }
        }
    }

    #[test]
    fn asymmetric_padding_controls_child_geometry_and_painted_insets() {
        let module = crate::session::tests::source(
            "function render(){return h('column',{style:{padding:10,paddingLeft:40,paddingRight:20,paddingBottom:30,background:'#ffffff'}},h('spacer',{style:{grow:1,background:'#ff0000'}}))}",
        );
        for scale in [1.0, 2.0] {
            let mut page = crate::ViewSession::new(&module).unwrap();
            let width = (300.0 * scale) as u16;
            let frame = page.layout(width, (100.0 * scale) as u16, scale).unwrap();
            assert_eq!(frame.nodes[1].bounds, [40.0, 10.0, 240.0, 60.0]);
            let raster = frame.rasterize().unwrap();
            let pixel = |x: f32, y: f32| {
                let offset = (((y * scale) as usize * width as usize) + (x * scale) as usize) * 4;
                &raster.pixels.data_as_u8_slice()[offset..offset + 4]
            };
            assert_eq!(pixel(39.0, 15.0), &[255, 255, 255, 255]);
            assert_eq!(pixel(41.0, 15.0), &[255, 0, 0, 255]);
            assert_eq!(pixel(281.0, 15.0), &[255, 255, 255, 255]);
            assert_eq!(pixel(50.0, 9.0), &[255, 255, 255, 255]);
            assert_eq!(pixel(50.0, 71.0), &[255, 255, 255, 255]);
        }
    }

    #[test]
    fn asymmetric_input_padding_keeps_caret_hits_and_search_reserve_consistent() {
        for (tag, reserve) in [("input", 0.0), ("textarea", 0.0), ("search", 32.0)] {
            let module = crate::session::tests::source(&format!(
                "function render(){{return h('{tag}',{{value:'abc',autoFocus:true,style:{{width:200,height:80,paddingTop:10,paddingRight:20,paddingBottom:5,paddingLeft:30}}}})}}",
            ));
            for scale in [1.0, 2.0] {
                let mut page = crate::ViewSession::new(&module).unwrap();
                let frame = page
                    .layout((240.0 * scale) as u16, (100.0 * scale) as u16, scale)
                    .unwrap();
                let shaped = frame.retained.inputs[0].as_ref().unwrap();
                assert_eq!(shaped.padding, [30.0 + reserve, 10.0]);
                assert_eq!(shaped.content_size, [150.0 - reserve, 65.0]);
                let node = &frame.nodes[0];
                let caret = node
                    .input
                    .as_ref()
                    .unwrap()
                    .geometry
                    .as_ref()
                    .unwrap()
                    .caret;
                assert!(caret[0] > 30.0 + reserve);
                assert!(caret[1] >= 10.0);
                if tag != "textarea" {
                    assert!((caret[1] + caret[3] / 2.0 - 42.5).abs() < 0.01);
                } else {
                    assert_eq!(shaped.text_offset_y, 0.0);
                }
                assert_eq!(
                    page.hit_test_input(
                        page.generation(),
                        &node.identity,
                        [caret[0], caret[1] + caret[3] / 2.0],
                        false
                    )
                    .unwrap(),
                    Some([3, 3])
                );
            }
        }
    }

    #[test]
    fn mixed_text_and_control_baselines_match_painted_glyphs() {
        for align in ["align:'baseline'", "align:'start'"] {
            let own = if align.contains("start") {
                "alignSelf:'baseline',"
            } else {
                ""
            };
            let module = crate::session::tests::source(&format!(
                "function render(){{return h('row',{{style:{{{align},gap:12}}}},
                    h('text',{{style:{{{own}fontSize:36,lineHeight:40}}}},'100'),
                    h('column',{{style:{{{own}padding:7,borderWidth:2}}}},
                        h('text',{{style:{{width:50,fontSize:18,lineHeight:28}}}},'11 22 33')),
                    h('button',{{style:{{{own}width:75,height:64,fontSize:13,padding:4}}}},'Go'))}}"
            ));
            for scale in [1.0, 2.0] {
                let mut page = crate::ViewSession::new(&module).unwrap();
                let frame = page
                    .layout((360.0 * scale) as u16, (180.0 * scale) as u16, scale)
                    .unwrap();
                let mut baselines = Vec::new();
                for size in [36.0, 18.0, 13.0] {
                    let baseline = frame
                        .commands
                        .iter()
                        .find_map(|paint| match paint {
                            Paint::Text {
                                size: actual,
                                glyphs,
                                ..
                            } if (*actual - size).abs() < 0.01 => {
                                glyphs.first().map(|glyph| glyph.y)
                            }
                            _ => None,
                        })
                        .expect("painted text run");
                    baselines.push(baseline);
                }
                for baseline in &baselines[1..] {
                    assert!(
                        (baseline - baselines[0]).abs() <= scale,
                        "{align}, scale {scale}: {baselines:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn button_labels_center_vertically_without_artificial_padding() {
        let baseline = |height| {
            let module = crate::session::tests::source(&format!(
                "function render(){{return h('button',{{style:{{width:160,height:{height},padding:0,fontSize:17}}}},'Go')}}",
            ));
            let mut page = crate::ViewSession::new(&module).unwrap();
            let frame = page.layout(200, 160, 1.0).unwrap();
            frame
                .commands
                .iter()
                .find_map(|paint| match paint {
                    Paint::Text { glyphs, .. } => glyphs.first().map(|glyph| glyph.y),
                    _ => None,
                })
                .unwrap()
        };
        assert!((baseline(120) - baseline(80) - 20.0).abs() < 0.01);
    }

    #[test]
    fn layout_alignment_and_self_override_use_logical_dimensions_at_each_scale() {
        let module = crate::session::tests::source(
            "function render(){return h('row',{style:{padding:10,align:'center',justify:'between'}},h('spacer',{style:{width:40,height:20}}),h('spacer',{style:{width:60,height:30,alignSelf:'end'}}))}",
        );
        for scale in [1.0, 2.0] {
            let mut page = crate::ViewSession::new(&module).unwrap();
            let frame = page
                .layout((300.0 * scale) as u16, (100.0 * scale) as u16, scale)
                .unwrap();
            assert_eq!(frame.nodes[1].bounds, [10.0, 40.0, 40.0, 20.0]);
            assert_eq!(frame.nodes[2].bounds, [230.0, 60.0, 60.0, 30.0]);
        }
    }

    #[test]
    fn layout_size_limits_and_shrink_preserve_fixed_siblings() {
        let module = crate::session::tests::source(
            "function render(){return h('row',{},h('spacer',{style:{width:200,height:30,minWidth:80,maxWidth:120,minHeight:40,maxHeight:50,shrink:1}}),h('spacer',{style:{width:160,height:20,shrink:0}}))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let narrow = page.layout(240, 100, 1.0).unwrap();
        assert_eq!(narrow.nodes[1].bounds, [0.0, 0.0, 80.0, 40.0]);
        assert_eq!(narrow.nodes[2].bounds, [80.0, 0.0, 160.0, 20.0]);
        let wide = page.layout(400, 100, 1.0).unwrap();
        assert_eq!(wide.nodes[1].bounds, [0.0, 0.0, 120.0, 40.0]);
        assert_eq!(wide.nodes[2].bounds, [120.0, 0.0, 160.0, 20.0]);
    }

    #[test]
    fn column_alignment_respects_min_width_and_max_height() {
        let module = crate::session::tests::source(
            "function render(){return h('column',{style:{align:'center',justify:'center'}},h('spacer',{style:{width:20,height:200,minWidth:60,maxWidth:80,minHeight:40,maxHeight:50}}))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let frame = page.layout(100, 200, 1.0).unwrap();
        assert_eq!(frame.nodes[1].bounds, [20.0, 75.0, 60.0, 50.0]);
    }

    #[test]
    fn centered_overflow_keeps_scroll_content_start_reachable() {
        let module = crate::session::tests::source(
            "function render(){return h('scroll',{style:{align:'center',justify:'end'}},h('spacer',{style:{width:200,height:180}}))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let frame = page.layout(100, 100, 1.0).unwrap();
        assert_eq!(frame.nodes[1].bounds, [0.0, 0.0, 200.0, 180.0]);
        assert!(frame.nodes[0].scroll.as_ref().unwrap().max_offset >= 80.0);
    }

    #[test]
    fn selection_endpoints_preserve_logical_direction_across_display_mappings() {
        use crate::text_input::Edit;
        for (tag, value, secure) in [
            ("input", "a\u{1f600}z", false),
            ("input", "a\u{1f600}z", true),
            ("input", "ab\r\ncd", false),
            ("textarea", "first\nsecond", false),
            ("input", "abc \u{5d0}\u{5d1}\u{5d2} def", false),
        ] {
            let module = crate::session::tests::source(&format!(
                "function render(){{return h('{tag}',{{value:{},secure:{secure},autoFocus:true,style:{{fontSize:20,padding:8}}}})}}",
                serde_json::to_string(value).unwrap()
            ));
            let mut page = crate::ViewSession::new(&module).unwrap();
            let first = page.layout(400, 160, 1.0).unwrap();
            let node = &first.nodes[0];
            let input = node.input.as_ref().unwrap();
            let initial = input.geometry.as_ref().unwrap();
            assert_eq!(initial.anchor, initial.caret);
            let token = input.edit_token.unwrap();
            let end = value.encode_utf16().count() as u32;
            page.edit_input(
                token,
                &node.identity,
                &[Edit::Selection {
                    anchor: 0,
                    focus: end,
                }],
                false,
            )
            .unwrap();
            let forward = page.prepare_native(400, 160, 1.0, false, None, Ok).unwrap();
            let forward = forward.nodes[0]
                .input
                .as_ref()
                .unwrap()
                .geometry
                .clone()
                .unwrap();
            page.edit_input(
                token,
                &node.identity,
                &[Edit::Selection {
                    anchor: end,
                    focus: 0,
                }],
                false,
            )
            .unwrap();
            let reverse = page.prepare_native(800, 320, 2.0, false, None, Ok).unwrap();
            let reverse = reverse.nodes[0]
                .input
                .as_ref()
                .unwrap()
                .geometry
                .as_ref()
                .unwrap();
            assert_eq!(forward.anchor, reverse.caret, "{tag}, {secure}");
            assert_eq!(forward.caret, reverse.anchor, "{tag}, {secure}");
            assert_eq!(forward.content_bounds, reverse.content_bounds);
            assert_eq!(forward.selection, reverse.selection);
        }
    }

    #[test]
    fn endpoint_geometry_scrolls_with_text_without_pinning_offscreen_handles() {
        use crate::text_input::Edit;
        for (tag, value, axis) in [
            (
                "input",
                "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz",
                0,
            ),
            ("textarea", "one\ntwo\nthree\nfour\nfive\nsix", 1),
        ] {
            let module = crate::session::tests::source(&format!(
                "function render(){{return h('{tag}',{{value:{},autoFocus:true,style:{{height:60,fontSize:20,padding:8}}}})}}",
                serde_json::to_string(value).unwrap()
            ));
            let mut page = crate::ViewSession::new(&module).unwrap();
            let first = page.layout(150, 80, 1.0).unwrap();
            let id = first.nodes[0].identity.clone();
            let token = first.nodes[0].input.as_ref().unwrap().edit_token.unwrap();
            let end = value.encode_utf16().count() as u32;
            page.edit_input(
                token,
                &id,
                &[Edit::Selection {
                    anchor: 0,
                    focus: end,
                }],
                false,
            )
            .unwrap();
            let forward = page.prepare_native(150, 80, 1.0, false, None, Ok).unwrap();
            let a = forward.nodes[0].input.as_ref().unwrap();
            let ga = a.geometry.as_ref().unwrap();
            assert!(ga.anchor[axis] < ga.content_bounds[axis]);
            page.edit_input(
                token,
                &id,
                &[Edit::Selection {
                    anchor: end,
                    focus: 0,
                }],
                false,
            )
            .unwrap();
            let reverse = page.prepare_native(150, 80, 1.0, false, None, Ok).unwrap();
            let b = reverse.nodes[0].input.as_ref().unwrap();
            let gb = b.geometry.as_ref().unwrap();
            assert!(gb.anchor[axis] > gb.content_bounds[axis] + gb.content_bounds[axis + 2]);
            assert_eq!(ga.content_bounds, gb.content_bounds);
            assert!(
                (ga.anchor[axis] + a.scroll_offset[axis] - gb.caret[axis] - b.scroll_offset[axis])
                    .abs()
                    < 0.01
            );
            assert!(
                (ga.caret[axis] + a.scroll_offset[axis] - gb.anchor[axis] - b.scroll_offset[axis])
                    .abs()
                    < 0.01
            );
        }
    }

    #[test]
    fn displayed_offsets_round_trip_without_splitting_unicode() {
        let value = "a\u{1f600}\r\n\u{4f60}e\u{301}";
        for secure in [false, true] {
            for single in [false, true] {
                for offset in 0..=value.encode_utf16().count() as u32 {
                    if let Some(byte) = super::display_byte(value, offset, secure, single) {
                        assert_eq!(
                            super::display_offset(value, byte, secure, single),
                            Some(offset)
                        );
                    } else {
                        assert_eq!(offset, 2);
                    }
                }
                assert_eq!(
                    super::display_offset(value, usize::MAX, secure, single),
                    None
                );
            }
        }
    }

    #[test]
    fn drag_edges_reach_both_ends_through_presented_shaped_cursors() {
        use crate::text_input::Edit;
        for (tag, value, secure, axis) in [
            (
                "input",
                "WiiiiWiiiiWiiiiWiiiiWiiiiWiiiiWiiiiWiiii",
                false,
                0,
            ),
            ("input", "abc\u{1f600}defghijklmnopqrstuvwxyz", true, 0),
            (
                "textarea",
                "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight",
                false,
                1,
            ),
        ] {
            let module = crate::session::tests::source(&format!(
                "function render(){{return h('{tag}',{{value:{},secure:{secure},autoFocus:true,style:{{height:60,fontSize:20,padding:8}}}})}}",
                serde_json::to_string(value).unwrap()
            ));
            let mut page = crate::ViewSession::new(&module).unwrap();
            let mut display = page.prepare_native(150, 80, 1.0, true, None, Ok).unwrap();
            let id = display.nodes[0].identity.clone();
            let token = display.nodes[0].input.as_ref().unwrap().edit_token.unwrap();
            let end = value.encode_utf16().count() as u32;
            for direction in [-1, 1] {
                let target = if direction < 0 { 0 } else { end };
                let mut last = if direction < 0 { end } else { 0 };
                for _ in 0..100 {
                    let input = display.nodes[0].input.as_ref().unwrap();
                    let bounds = input.geometry.as_ref().unwrap().content_bounds;
                    let mut point = [bounds[0] + 0.5, bounds[1] + 0.5];
                    if direction > 0 {
                        point = [bounds[0] + bounds[2] - 0.5, bounds[1] + bounds[3] - 0.5];
                    }
                    let mut edge = [0, 0];
                    edge[axis] = direction;
                    let generation = page.generation();
                    assert_eq!(
                        page.drag_hit_test_input(generation, &id, token + 1, point, edge)
                            .unwrap(),
                        None
                    );
                    assert_eq!(
                        page.drag_hit_test_input(generation, &id, token, point, [2, 0])
                            .unwrap(),
                        None
                    );
                    let next = page
                        .drag_hit_test_input(generation, &id, token, point, edge)
                        .unwrap()
                        .unwrap()[1];
                    if last == target {
                        assert_eq!(next, target);
                        break;
                    }
                    assert!(
                        if direction < 0 {
                            next < last
                        } else {
                            next > last
                        },
                        "{tag} secure={secure}: {last} -> {next}"
                    );
                    page.edit_input(
                        token,
                        &id,
                        &[Edit::Selection {
                            anchor: 0,
                            focus: next,
                        }],
                        false,
                    )
                    .unwrap();
                    assert_eq!(
                        page.drag_hit_test_input(generation, &id, token, point, edge)
                            .unwrap(),
                        None
                    );
                    display = page.prepare_native(150, 80, 1.0, false, None, Ok).unwrap();
                    last = next;
                }
                assert_eq!(last, target, "{tag}, secure={secure}");
            }
        }
    }

    #[test]
    fn input_hit_testing_uses_displayed_scroll_and_mask_mapping() {
        for (tag, value, secure) in [
            ("input", "abc\u{1f600}defghijklmnopqrstuvwxyz", false),
            ("input", "abc\u{1f600}defghijklmnopqrstuvwxyz", true),
            ("textarea", "one\ntwo\nthree\nfour\nfive\nsix", false),
        ] {
            let module = crate::session::tests::source(&format!(
                "function render(){{return h('{tag}',{{value:{},secure:{secure},autoFocus:true,style:{{height:60,fontSize:20,padding:8}}}})}}",
                serde_json::to_string(value).unwrap()
            ));
            let mut page = crate::ViewSession::new(&module).unwrap();
            let display = page.prepare_native(150, 80, 1.0, true, None, Ok).unwrap();
            let node = &display.nodes[0];
            let input = node.input.as_ref().unwrap();
            let caret = input.geometry.as_ref().unwrap().caret;
            let point = [caret[0], caret[1] + caret[3] / 2.0];
            let hits = super::InputHitMap::capture(&display);
            let end = value.encode_utf16().count() as u32;
            assert_eq!(
                hits.hit(&node.identity, point, false),
                Some([end, end]),
                "{tag}, secure={secure}"
            );
            assert_eq!(hits.hit(&node.identity, [-1.0, point[1]], false), None);
            assert_eq!(hits.hit(&node.identity, [f32::NAN, point[1]], false), None);
            assert_eq!(hits.hit("missing", point, false), None);
            if secure {
                assert_eq!(hits.hit(&node.identity, point, true), Some([0, end]));
            }
        }
    }

    #[test]
    fn placeholder_tint_is_local_and_does_not_recolor_value_or_caret() {
        for tag in ["input", "textarea", "search"] {
            for value in ["", "typed", " "] {
                let source = crate::session::tests::source(&format!(
                    "function render(){{return h('{tag}',{{value:'{value}',placeholder:'Hint',style:{{color:'#0000ff',placeholderColor:'#ff000080'}}}})}}"
                ));
                let mut page = crate::ViewSession::new(&source).unwrap();
                let display = page.layout(200, 80, 1.0).unwrap();
                let expected = if value.is_empty() {
                    [255, 0, 0, 128]
                } else {
                    [0, 0, 255, 255]
                };
                let paints: Vec<_> = display
                    .commands
                    .iter()
                    .filter_map(|p| match p {
                        Paint::Text { color, .. } => Some(*color),
                        _ => None,
                    })
                    .collect();
                assert!(!paints.is_empty());
                assert!(paints.iter().all(|c| *c == expected), "{tag}: {value:?}");
                assert_eq!(
                    display.retained.inputs[0].as_ref().unwrap().color,
                    [0, 0, 255, 255]
                );
            }
        }
        let source = crate::session::tests::source(
            "function render(){return h('column',{style:{color:'#0000ff',placeholderColor:'#ff0000'}},h('input',{placeholder:'Hint'}))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(200, 80, 1.0).unwrap();
        assert!(
            display
                .commands
                .iter()
                .filter_map(|p| match p {
                    Paint::Text { color, .. } => Some(color),
                    _ => None,
                })
                .all(|c| *c == [0, 0, 255, 255])
        );
    }

    #[test]
    fn word_hit_testing_and_empty_placeholder_do_not_use_character_width_estimates() {
        let module = crate::session::tests::source(
            "function render(){return h('column',{},h('input',{value:'hello world',style:{height:40,fontSize:20,padding:8}}),h('input',{placeholder:'not editable text',style:{height:40}}))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let display = page.prepare_native(300, 100, 1.0, true, None, Ok).unwrap();
        let hits = super::InputHitMap::capture(&display);
        let node = &display.nodes[1];
        let selected = hits
            .hit(
                &node.identity,
                [node.bounds[0] + 12.0, node.bounds[1] + 20.0],
                true,
            )
            .unwrap();
        assert_eq!(selected, [0, 5]);
        let empty = &display.nodes[2];
        assert_eq!(
            hits.hit(
                &empty.identity,
                [empty.bounds[0] + 100.0, empty.bounds[1] + 20.0],
                false
            ),
            Some([0, 0])
        );
    }

    #[test]
    fn blink_hides_only_caret_without_changing_editor_or_rerunning_view() {
        let module = crate::session::tests::source(
            "let renders=0;function render(){renders++;return h('column',{},h('input',{value:'hello',autoFocus:true}),h('text',{},String(renders)))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let first = page.layout(200, 100, 1.0).unwrap();
        let id = first.nodes[1].identity.clone();
        let token = first.nodes[1].input.as_ref().unwrap().edit_token.unwrap();
        page.edit_input(
            token,
            &id,
            &[crate::text_input::Edit::Composition { start: 0, end: 5 }],
            false,
        )
        .unwrap()
        .unwrap();
        let visible = page.prepare_native(200, 100, 1.0, false, None, Ok).unwrap();
        let geometry = visible.nodes[1]
            .input
            .as_ref()
            .unwrap()
            .geometry
            .clone()
            .unwrap();
        let mut hidden = page.prepare_native(200, 100, 1.0, false, None, Ok).unwrap();
        assert_eq!(hidden.nodes[2].text, "1");
        assert_eq!(
            hidden.nodes[1]
                .input
                .as_ref()
                .unwrap()
                .geometry
                .as_ref()
                .unwrap()
                .composition,
            geometry.composition
        );
        hidden.hide_caret();
        hidden.hide_caret();
        let on = visible.rasterize().unwrap().pixels;
        let off = hidden.rasterize().unwrap().pixels;
        let mut changed = 0;
        for (index, (a, b)) in on
            .data_as_u8_slice()
            .chunks_exact(4)
            .zip(off.data_as_u8_slice().chunks_exact(4))
            .enumerate()
        {
            if a == b {
                continue;
            }
            changed += 1;
            let x = (index % 200) as f32;
            let y = (index / 200) as f32;
            assert!(
                x >= geometry.caret[0].floor()
                    && x < (geometry.caret[0] + geometry.caret[2]).ceil()
            );
            assert!(
                y >= geometry.caret[1].floor()
                    && y < (geometry.caret[1] + geometry.caret[3]).ceil()
            );
        }
        assert!(changed > 5);
        let resumed = page.prepare_native(200, 100, 1.0, false, None, Ok).unwrap();
        assert_eq!(resumed.nodes[2].text, "1");
        assert!(resumed.caret_paint.is_some());
    }

    #[test]
    fn long_inputs_reveal_selection_focus_without_resetting_the_viewport() {
        use crate::text_input::Edit;
        for (tag, value, axis) in [
            ("input", "abcdefghij".repeat(12), 0),
            ("textarea", "line\\n".repeat(12) + "last", 1),
        ] {
            let module = crate::session::tests::source(&format!(
                "function render(){{return h('{tag}',{{value:'{value}',autoFocus:true,style:{{height:100,fontSize:20,background:'#ffffff'}}}})}}"
            ));
            let mut page = crate::ViewSession::new(&module).unwrap();
            let first = page.layout(180, 100, 1.0).unwrap();
            let input = first.nodes[0].input.as_ref().unwrap();
            let token = input.edit_token.unwrap();
            let id = first.nodes[0].identity.clone();
            let original_offset = input.scroll_offset;
            assert!(original_offset[axis] > 100.0, "{original_offset:?}");
            let caret = input.geometry.as_ref().unwrap().caret;
            assert!(
                caret[0] >= 12.0 && caret[0] + caret[2] <= 168.01,
                "{caret:?}"
            );
            assert!(
                caret[1] >= 12.0 && caret[1] + caret[3] <= 88.01,
                "{caret:?}"
            );
            let old_glyph = first
                .commands
                .iter()
                .find_map(|p| match p {
                    Paint::Text { glyphs, .. } => glyphs.first(),
                    _ => None,
                })
                .unwrap()
                .clone();
            // Selecting toward the beginning must reveal the focus, not the anchor.
            let end = input.edit.as_ref().unwrap().selection[1];
            page.edit_input(
                token,
                &id,
                &[Edit::Selection {
                    anchor: end,
                    focus: 0,
                }],
                false,
            )
            .unwrap()
            .unwrap();
            let failed = page.prepare_native(180, 100, 1.0, false, None, |_| {
                Err::<(), _>("surface unavailable".into())
            });
            assert!(failed.is_err());
            let restored = page.prepare_native(180, 100, 1.0, false, None, Ok).unwrap();
            assert_eq!(
                restored.nodes[0].input.as_ref().unwrap().scroll_offset,
                original_offset
            );
            let token = restored.nodes[0]
                .input
                .as_ref()
                .unwrap()
                .edit_token
                .unwrap();
            page.edit_input(
                token,
                &id,
                &[Edit::Selection {
                    anchor: end,
                    focus: 0,
                }],
                false,
            )
            .unwrap()
            .unwrap();
            let selected = page.prepare_native(180, 100, 1.0, false, None, Ok).unwrap();
            let selected_input = selected.nodes[0].input.as_ref().unwrap();
            assert_eq!(selected_input.scroll_offset, [0.0; 2]);
            let glyph = selected
                .commands
                .iter()
                .find_map(|p| match p {
                    Paint::Text { glyphs, .. } => glyphs.first(),
                    _ => None,
                })
                .unwrap();
            let delta = if axis == 0 {
                glyph.x - old_glyph.x
            } else {
                glyph.y - old_glyph.y
            };
            assert!((delta - original_offset[axis]).abs() < 0.01);
            let pixels = selected.rasterize().unwrap().pixels;
            // Selection and scrolled text must not cover the input padding.
            assert_eq!(
                &pixels.data_as_u8_slice()[(50 * 180 + 5) * 4..(50 * 180 + 5) * 4 + 3],
                &[255, 255, 255]
            );
            page.edit_input(
                token,
                &id,
                &[Edit::Replace {
                    text: "x".into(),
                    cursor: 1,
                    composing: false,
                }],
                false,
            )
            .unwrap()
            .unwrap();
            let shortened = page.prepare_native(180, 100, 1.0, false, None, Ok).unwrap();
            assert_eq!(
                shortened.nodes[0].input.as_ref().unwrap().scroll_offset,
                [0.0; 2]
            );
        }
    }

    #[test]
    fn displayed_offsets_preserve_unicode_and_password_boundaries() {
        let text = "a\u{1f600}\n\u{2028}z";
        assert_eq!(display_byte(text, 2, false, true), None);
        assert_eq!(display_byte(text, 3, false, true), Some(5));
        assert_eq!(display_byte(text, 5, false, true), Some(7));
        assert_eq!(display_byte(text, 5, false, false), Some(9));
        assert_eq!(display_byte(text, 3, true, true), Some(6));
        assert_eq!(display_byte(text, 6, true, true), Some(15));
        assert_eq!(display_byte(text, 7, true, true), None);
    }

    #[test]
    fn input_geometry_uses_displayed_font_and_paints_selection_below_text() {
        use crate::text_input::Edit;
        let module = crate::session::tests::source(
            "function render(){return h('input',{value:'Hello world',autoFocus:true,style:{fontSize:32,color:'#000000',background:'#ffffff'}})}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let first = page.layout(300, 80, 1.0).unwrap();
        let input = first.nodes[0].input.as_ref().unwrap();
        let caret = input.geometry.as_ref().unwrap().caret;
        assert!(caret[0] > 100.0 && caret[3] > 25.0, "{caret:?}");
        let token = input.edit_token.unwrap();
        let id = first.nodes[0].identity.clone();
        page.edit_input(
            token,
            &id,
            &[
                Edit::Selection {
                    anchor: 0,
                    focus: 5,
                },
                Edit::Composition { start: 0, end: 5 },
            ],
            false,
        )
        .unwrap()
        .unwrap();
        let selected = page.prepare_native(300, 80, 1.0, false, None, Ok).unwrap();
        let geometry = selected.nodes[0]
            .input
            .as_ref()
            .unwrap()
            .geometry
            .as_ref()
            .unwrap();
        assert_eq!(geometry.selection.len(), 1);
        assert_eq!(geometry.composition.len(), 1);
        assert!(geometry.selection[0][2] > 50.0);
        assert_eq!(geometry.composition[0][3], 1.0);
        let selection_paint = selected
            .commands
            .iter()
            .position(|p| {
                matches!(
                    p,
                    Paint::Rect {
                        color: [216, 227, 255, 255],
                        ..
                    }
                )
            })
            .unwrap();
        let glyph_paint = selected
            .commands
            .iter()
            .position(|p| matches!(p, Paint::Text { .. }))
            .unwrap();
        assert!(selection_paint < glyph_paint);
        selected.rasterize().unwrap();
        page.blur_input(token, &id).unwrap();
        let blurred = page.prepare_native(300, 80, 1.0, false, None, Ok).unwrap();
        assert!(blurred.nodes[0].input.as_ref().unwrap().geometry.is_none());
    }

    #[test]
    fn empty_and_secure_inputs_keep_a_caret_and_multiline_selection_wraps() {
        use crate::text_input::Edit;
        for (tag, value, extra) in [
            ("input", "", "placeholder:'Type here',"),
            ("input", "a\\u{1f600}z", "secure:true,"),
            ("textarea", "first line\\nsecond line", ""),
        ] {
            let module = crate::session::tests::source(&format!(
                "function render(){{return h('{tag}',{{value:'{value}',{extra}autoFocus:true,style:{{fontSize:24}}}})}}"
            ));
            let mut page = crate::ViewSession::new(&module).unwrap();
            let first = page.layout(300, 150, 1.0).unwrap();
            let input = first.nodes[0].input.as_ref().unwrap();
            let caret = input.geometry.as_ref().unwrap().caret;
            assert!(caret.iter().all(|v| v.is_finite()) && caret[3] > 20.0);
            let end = input.edit.as_ref().unwrap().selection[1];
            let token = input.edit_token.unwrap();
            let id = first.nodes[0].identity.clone();
            page.edit_input(
                token,
                &id,
                &[Edit::Selection {
                    anchor: end,
                    focus: 0,
                }],
                false,
            )
            .unwrap()
            .unwrap();
            let selected = page.prepare_native(300, 150, 1.0, false, None, Ok).unwrap();
            let geometry = selected.nodes[0]
                .input
                .as_ref()
                .unwrap()
                .geometry
                .as_ref()
                .unwrap();
            assert_eq!(
                geometry.selection.len(),
                if tag == "textarea" {
                    2
                } else if value.is_empty() {
                    0
                } else {
                    1
                }
            );
            assert!(geometry.caret[0] <= caret[0]);
            selected.rasterize().unwrap();
        }
    }

    #[test]
    fn caret_moves_with_retained_scroll_and_disappears_when_hidden() {
        let module = crate::session::tests::source(
            "function render(){return h('scroll',{style:{radius:20}},h('input',{value:'hello',autoFocus:true}),h('spacer',{style:{height:300}}))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let first = page.layout(200, 100, 1.0).unwrap();
        let geometry = first.nodes[1]
            .input
            .as_ref()
            .unwrap()
            .geometry
            .as_ref()
            .unwrap();
        let id = first.nodes[0].scroll.as_ref().unwrap().id.clone();
        let moved = page
            .prepare_native(
                200,
                100,
                1.0,
                false,
                Some(&HashMap::from([(id.clone(), 10.0)])),
                Ok,
            )
            .unwrap();
        let next = moved.nodes[1]
            .input
            .as_ref()
            .unwrap()
            .geometry
            .as_ref()
            .unwrap();
        for (before, after) in [
            (geometry.anchor, next.anchor),
            (geometry.caret, next.caret),
            (geometry.content_bounds, next.content_bounds),
        ] {
            assert_eq!(after[0], before[0]);
            assert!((after[1] - before[1] + 10.0).abs() < 0.01);
        }
        assert_eq!(
            moved.nodes[1].clips[0].bounds,
            first.nodes[1].clips[0].bounds
        );
        let hidden = page
            .prepare_native(
                200,
                100,
                1.0,
                false,
                Some(&HashMap::from([(id, 100.0)])),
                Ok,
            )
            .unwrap();
        assert!(hidden.nodes[1].input.as_ref().unwrap().geometry.is_none());
    }

    #[test]
    fn rounded_scroll_masks_are_shared_and_stay_fixed_during_scroll() {
        let source = crate::session::tests::source(
            "function render(){return h('column',{style:{padding:10,background:'#ffffff'}},h('scroll',{style:{height:80,radius:24}},h('column',{style:{height:40,background:'#ff0000'}}),h('column',{style:{height:120,background:'#0000ff'}})))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(100, 100, 1.0).unwrap();
        assert_eq!(
            display
                .commands
                .iter()
                .filter(|p| matches!(p, Paint::Clip(_)))
                .count(),
            1
        );
        assert_eq!(display.nodes[2].clips, display.nodes[3].clips);
        assert_eq!(display.nodes[2].clips[0].bounds, [10., 10., 80., 80.]);
        let id = display.nodes[1].scroll.as_ref().unwrap().id.clone();
        let retained = display.retained.clone();
        for offset in [0., 20., 80.] {
            let frame = retained
                .compose(&HashMap::from([(id.clone(), offset)]))
                .unwrap();
            assert_eq!(frame.nodes[3].clips[0].bounds, [10., 10., 80., 80.]);
            let image = frame.rasterize().unwrap().pixels;
            for (x, y) in [(10, 10), (89, 10), (10, 89), (89, 89)] {
                assert_eq!(
                    &image.data_as_u8_slice()[(y * 100 + x) * 4..(y * 100 + x) * 4 + 3],
                    &[255, 255, 255]
                );
            }
            assert_eq!(
                &image.data_as_u8_slice()[(80 * 100 + 50) * 4..(80 * 100 + 50) * 4 + 3],
                &[0, 0, 255]
            );
        }
    }
    #[test]
    fn nested_rounding_uses_original_bounds_and_hides_fully_masked_nodes() {
        let source = crate::session::tests::source(
            "function render(){return h('scroll',{style:{background:'#ffffff'}},h('scroll',{style:{height:80,radius:30}},h('column',{style:{height:200,background:'#0000ff'}})),h('spacer',{style:{height:100}}))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(100, 100, 1.0).unwrap();
        let id = display.nodes[0].scroll.as_ref().unwrap().id.clone();
        let frame = display
            .retained
            .compose(&HashMap::from([(id, 20.)]))
            .unwrap();
        assert_eq!(frame.nodes[2].clips[0].bounds, [0., -20., 100., 80.]);
        let pixels = frame.rasterize().unwrap().pixels;
        assert_eq!(
            &pixels.data_as_u8_slice()[(100 + 10) * 4..(100 + 10) * 4 + 3],
            &[0, 0, 255]
        );
        let mut page = crate::ViewSession::new(&crate::session::tests::source("function render(){return h('scroll',{style:{radius:40}},h('image',{src:'hidden',style:{width:1,height:1}}))}")).unwrap();
        let display = page.layout(100, 100, 1.0).unwrap();
        assert_eq!(display.image_sources().count(), 0);
        assert_eq!(display.nodes[1].visible_bounds[2], 0.0);
    }

    #[test]
    fn late_images_share_layout_and_obey_fit_rounding_and_scroll_clipping() {
        use ::image::ImageEncoder;
        let mut bytes = Vec::new();
        ::image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(
                &[255, 0, 0, 255, 0, 0, 255, 255],
                2,
                1,
                ::image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        let images = HashMap::from([(
            "photo".to_string(),
            crate::image::Bitmap::decode(&bytes).unwrap(),
        )]);
        let source = crate::session::tests::source(
            "function render(){return h('column',{style:{background:'#ffffff'}},h('image',{src:'photo',fit:'contain',style:{width:100,height:100,radius:20}}))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let pending = page.layout(100, 100, 1.0).unwrap();
        assert_eq!(pending.image_sources().collect::<Vec<_>>(), vec!["photo"]);
        let retained = pending.retained.clone();
        let placeholder = pending.rasterize().unwrap();
        assert_eq!(
            &placeholder.pixels.data_as_u8_slice()[0..4],
            &[255, 255, 255, 255]
        );
        let mut display = retained.compose(&HashMap::new()).unwrap();
        display.bind_images(&images).unwrap();
        let loaded = display.rasterize().unwrap();
        let pixel = |x: usize, y: usize| {
            &loaded.pixels.data_as_u8_slice()[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4]
        };
        assert_eq!(pixel(50, 10), &[255, 255, 255, 255]);
        assert!(pixel(10, 50)[0] > 240 && pixel(10, 50)[2] < 15);
        assert!(pixel(90, 50)[2] > 240 && pixel(90, 50)[0] < 15);
        let source = crate::session::tests::source(
            "function render(){return h('scroll',{},h('spacer',{style:{height:150}}),h('image',{src:'photo',style:{width:100,height:100,radius:30}}))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let first = page.layout(100, 100, 1.0).unwrap();
        assert_eq!(first.image_sources().count(), 0);
        let id = first.nodes[0].scroll.as_ref().unwrap().id.clone();
        let mut visible = first
            .retained
            .compose(&HashMap::from([(id, 150.)]))
            .unwrap();
        assert_eq!(visible.image_sources().count(), 1);
        visible.bind_images(&images).unwrap();
        let frame = visible.rasterize().unwrap();
        assert_eq!(&frame.pixels.data_as_u8_slice()[..4], &[246, 247, 251, 255]);
        assert_ne!(
            &frame.pixels.data_as_u8_slice()[(50 * 100 + 50) * 4..(50 * 100 + 50) * 4 + 3],
            &[246, 247, 251]
        );
    }
    use super::*;
    use serde_json::json;

    fn node(
        tag: usize,
        parent: Option<usize>,
        key: &str,
        fields: &[(usize, serde_json::Value)],
    ) -> Record {
        Record {
            tag,
            parent,
            key: key.into(),
            test_id: key.into(),
            text: String::new(),
            fields: fields.to_vec(),
        }
    }

    fn page() -> Vec<Record> {
        vec![
            node(0, None, "root", &[(6, json!("#ffffff"))]),
            node(4, Some(0), "header", &[(1, json!(20))]),
            node(5, Some(0), "feed", &[(1, json!(60))]),
            node(
                3,
                Some(2),
                "first",
                &[
                    (1, json!(50)),
                    (6, json!("#ff0000")),
                    (10, json!(0)),
                    (8, json!(1)),
                ],
            ),
            node(
                3,
                Some(2),
                "second",
                &[
                    (1, json!(50)),
                    (6, json!("#0000ff")),
                    (10, json!(0)),
                    (8, json!(2)),
                ],
            ),
            node(4, Some(0), "footer", &[(1, json!(20))]),
        ]
    }

    #[test]
    fn scroll_clips_paint_and_input_geometry_and_clamps_offsets() {
        let mut renderer = Renderer::default();
        let initial = renderer.layout(&page(), 100, 100, 1.0).unwrap();
        let scroll = initial.nodes[2].scroll.as_ref().unwrap();
        assert_eq!(scroll.max_offset, 40.0);
        let offsets = HashMap::from([(scroll.id.clone(), 999.0)]);
        let scrolled = renderer
            .layout_scrolled(&page(), 100, 100, 1.0, &offsets)
            .unwrap();
        assert_eq!(scrolled.nodes[2].scroll.as_ref().unwrap().offset, 40.0);
        assert_eq!(scrolled.nodes[3].bounds, [0.0, -20.0, 100.0, 50.0]);
        assert_eq!(scrolled.nodes[3].visible_bounds, [0.0, 20.0, 100.0, 10.0]);
        assert_eq!(scrolled.nodes[4].visible_bounds, [0.0, 30.0, 100.0, 50.0]);
        let frame = scrolled.rasterize().unwrap();
        let pixels = frame.pixels.data();
        assert_eq!(pixels[10 * 100 + 50], pixels[90 * 100 + 50]);
        assert_ne!(pixels[25 * 100 + 50], pixels[35 * 100 + 50]);
        assert_ne!(pixels[35 * 100 + 50], pixels[90 * 100 + 50]);
        assert!(
            renderer
                .layout_scrolled(
                    &page(),
                    100,
                    100,
                    1.0,
                    &HashMap::from([("bad".into(), f32::NAN)])
                )
                .is_err()
        );
    }

    #[test]
    fn node_identity_tracks_keyed_ancestry_and_tag_not_callback_or_label() {
        let mut renderer = Renderer::default();
        let mut records = vec![
            node(0, None, "root", &[]),
            node(3, Some(0), "save", &[(8, json!(1))]),
        ];
        let initial = renderer.layout(&records, 200, 200, 1.0).unwrap();
        let id = initial.nodes[1].identity.clone();
        records[1].fields = vec![(8, json!(99))];
        records[1].text = "Saved".into();
        records.insert(1, node(4, Some(0), "new", &[(1, json!(10))]));
        let reordered = renderer.layout(&records, 200, 200, 1.0).unwrap();
        assert_eq!(reordered.nodes[2].identity, id);
        records[2].tag = 2;
        assert_ne!(
            renderer.layout(&records, 200, 200, 1.0).unwrap().nodes[2].identity,
            id
        );
        records[2].tag = 3;
        records[0].key = "other-page".into();
        assert_ne!(
            renderer.layout(&records, 200, 200, 1.0).unwrap().nodes[2].identity,
            id
        );
    }

    #[test]
    fn keyed_scroll_identity_survives_sibling_insertion_and_content_shrink() {
        let mut renderer = Renderer::default();
        let mut records = page();
        let initial = renderer.layout(&records, 100, 100, 1.0).unwrap();
        let id = initial.nodes[2].scroll.as_ref().unwrap().id.clone();
        records.insert(1, node(4, Some(0), "inserted", &[(1, json!(5))]));
        records[4].parent = Some(3);
        records[5].parent = Some(3);
        let frame = renderer
            .layout_scrolled(
                &records,
                100,
                100,
                1.0,
                &HashMap::from([(id.clone(), 40.0)]),
            )
            .unwrap();
        assert_eq!(frame.nodes[3].scroll.as_ref().unwrap().id, id);
        assert_eq!(frame.nodes[3].scroll.as_ref().unwrap().offset, 40.0);
        records.remove(5);
        let frame = renderer
            .layout_scrolled(&records, 100, 100, 1.0, &HashMap::from([(id, 40.0)]))
            .unwrap();
        assert_eq!(frame.nodes[3].scroll.as_ref().unwrap().offset, 0.0);
    }

    #[test]
    fn nested_scroll_clips_intersect_without_expanding_outer_content() {
        let records = vec![
            node(5, None, "outer", &[]),
            node(4, Some(0), "space", &[(1, json!(80))]),
            node(5, Some(0), "inner", &[(1, json!(50))]),
            node(3, Some(2), "item", &[(1, json!(200))]),
        ];
        let frame = Renderer::default().layout(&records, 100, 100, 1.0).unwrap();
        let inner = &frame.nodes[2];
        assert_eq!(inner.scroll.as_ref().unwrap().max_offset, 150.0);
        let visible = frame.nodes[3].visible_bounds;
        assert!(visible[1] >= inner.visible_bounds[1]);
        assert!(visible[1] + visible[3] <= 100.0);
        assert_eq!(frame.nodes[0].scroll.as_ref().unwrap().max_offset, 30.0);
    }

    #[test]
    fn growing_ancestors_reserve_a_bounded_scroll_viewport() {
        let records = vec![
            node(0, None, "root", &[]),
            node(4, Some(0), "header", &[(1, json!(20))]),
            node(1, Some(0), "row", &[(7, json!(1))]),
            node(0, Some(2), "content", &[(7, json!(1))]),
            node(4, Some(3), "tabs", &[(1, json!(20))]),
            node(5, Some(3), "feed", &[(7, json!(1))]),
            node(4, Some(5), "items", &[(1, json!(1000))]),
        ];
        let frame = Renderer::default().layout(&records, 100, 100, 1.0).unwrap();
        assert_eq!(frame.nodes[5].bounds, [0.0, 40.0, 100.0, 60.0]);
        assert_eq!(frame.nodes[5].scroll.as_ref().unwrap().max_offset, 940.0);
    }

    #[test]
    fn retained_scroll_reuses_shaping_and_culls_only_invisible_paint() {
        let mut records = vec![node(5, None, "feed", &[])];
        for index in 0..20 {
            let mut item = node(
                3,
                Some(0),
                &format!("item-{index}"),
                &[(1, json!(50)), (8, json!(index + 1))],
            );
            item.text = format!("Item {index}");
            records.push(item);
        }
        let mut renderer = Renderer::default();
        let first = renderer.layout(&records, 200, 100, 1.0).unwrap();
        let retained = Arc::clone(&first.retained);
        let id = first.nodes[0].scroll.as_ref().unwrap().id.clone();
        assert_eq!(first.nodes[1].scroll_parent.as_deref(), Some(id.as_str()));
        let offsets = HashMap::from([(id, 425.0)]);
        let scrolled = retained.compose(&offsets).unwrap();
        assert!(Arc::ptr_eq(&first.retained, &scrolled.retained));
        assert_eq!(first.nodes[1].bounds[1], 0.0);
        assert_eq!(scrolled.nodes[1].bounds[1], -425.0);
        assert_eq!(
            scrolled.nodes[1].scroll_parent,
            first.nodes[1].scroll_parent
        );
        assert!(
            scrolled
                .commands
                .iter()
                .filter(|command| matches!(command, Paint::Text { .. }))
                .count()
                <= 3
        );
        assert!(scrolled.commands.len() < retained.commands.len());
        let expected = renderer
            .layout_scrolled(&records, 200, 100, 1.0, &offsets)
            .unwrap();
        assert_eq!(
            serde_json::to_value(&scrolled.nodes).unwrap(),
            serde_json::to_value(&expected.nodes).unwrap()
        );
        assert_eq!(
            scrolled.rasterize().unwrap().pixels.data(),
            expected.rasterize().unwrap().pixels.data()
        );
        assert!(
            retained
                .compose(&HashMap::from([("bad".into(), f32::INFINITY)]))
                .is_err()
        );
    }

    #[test]
    fn boolean_controls_present_checked_state_and_distinct_pixels_at_each_scale() {
        for scale in [1.0, 2.75] {
            for tag in [10, 11] {
                let mut control = node(
                    tag,
                    Some(0),
                    "preference",
                    &[(39, json!(0)), (40, json!(42))],
                );
                control.text = "Preference".into();
                let mut records = vec![node(0, None, "root", &[]), control];
                let mut renderer = Renderer::default();
                let width = (240.0 * scale) as u16;
                let height = (100.0 * scale) as u16;
                let off = renderer.layout(&records, width, height, scale).unwrap();
                assert_eq!(off.nodes[1].checked, Some(false));
                assert_eq!(off.nodes[1].on_click, Some(42));
                assert!(off.nodes[1].bounds[3] >= 44.0);
                records[1].fields[0].1 = json!(1);
                let on = renderer.layout(&records, width, height, scale).unwrap();
                assert_eq!(on.nodes[1].checked, Some(true));
                assert_eq!(off.nodes[1].bounds, on.nodes[1].bounds);
                let on_pixels = on.rasterize().unwrap().pixels;
                assert_ne!(off.rasterize().unwrap().pixels.data(), on_pixels.data());
                records[1].fields.push((9, json!(0)));
                let disabled = renderer.layout(&records, width, height, scale).unwrap();
                assert_eq!(disabled.nodes[1].checked, Some(true));
                assert_eq!(disabled.nodes[1].on_click, None);
                assert_ne!(
                    on_pixels.data(),
                    disabled.rasterize().unwrap().pixels.data()
                );
            }
        }
    }

    #[test]
    fn rebound_controls_match_fresh_geometry_paint_and_event_handles() {
        let records = vec![
            node(0, None, "root", &[]),
            node(
                8,
                Some(0),
                "editor",
                &[(21, json!("text")), (26, json!(10)), (27, json!(11))],
            ),
            node(10, Some(0), "check", &[(39, json!(0)), (40, json!(12))]),
            node(
                15,
                Some(0),
                "slider",
                &[(41, json!("Volume")), (42, json!(3)), (46, json!(13))],
            ),
            node(
                5,
                Some(0),
                "scroll",
                &[(1, json!(80)), (17, json!(14)), (18, json!(15))],
            ),
            node(3, Some(4), "button", &[(8, json!(16))]),
            node(
                18,
                Some(0),
                "plot",
                &[
                    (1, json!(100)),
                    (41, json!("Series")),
                    (51, json!("[10,20]")),
                    (52, json!("line")),
                    (53, json!("[\"#123456\"]")),
                    (56, json!(17)),
                    (57, json!(-1)),
                ],
            ),
        ];
        let mut renderer = Renderer::default();
        let first = renderer.layout(&records, 240, 400, 1.0).unwrap();
        let mut changed = records.clone();
        for record in &mut changed {
            for (id, value) in &mut record.fields {
                if matches!(*id, 8 | 17 | 18 | 26 | 27 | 40 | 46 | 56) {
                    *value = json!(value.as_u64().unwrap() + 100);
                }
            }
        }
        let rebound = first
            .retained
            .rebind(&records, &changed)
            .unwrap()
            .compose(&HashMap::new())
            .unwrap();
        let fresh = renderer.layout(&changed, 240, 400, 1.0).unwrap();
        assert_eq!(
            serde_json::to_value(&rebound.nodes).unwrap(),
            serde_json::to_value(&fresh.nodes).unwrap()
        );
        assert_eq!(
            rebound.rasterize().unwrap().pixels.data_as_u8_slice(),
            fresh.rasterize().unwrap().pixels.data_as_u8_slice()
        );
        for (index, field, value) in [
            (1, 21, json!("edited")),
            (2, 39, json!(1)),
            (3, 42, json!(6)),
            (5, 9, json!(0)),
            (6, 51, json!("[20,10]")),
            (6, 57, json!(1)),
            (0, 58, json!(0.5)),
            (0, 59, json!(8)),
            (0, 60, json!(8)),
            (0, 61, json!(8)),
            (0, 62, json!(8)),
        ] {
            let mut different = changed.clone();
            if let Some((_, old)) = different[index]
                .fields
                .iter_mut()
                .find(|(id, _)| *id == field)
            {
                *old = value;
            } else {
                different[index].fields.push((field, value));
            }
            assert!(first.retained.rebind(&records, &different).is_none());
        }
    }

    #[test]
    fn cached_shaping_preserves_pixels_when_width_style_and_text_change() {
        let mut cached = Renderer::default();
        let mut fresh = Renderer::default();
        for (width, size, weight, family, height, text) in [
            (
                240,
                18,
                400,
                "sans",
                24,
                "A long title wraps across narrow cards",
            ),
            (
                90,
                18,
                400,
                "sans",
                24,
                "A long title wraps across narrow cards",
            ),
            (
                240,
                18,
                400,
                "sans",
                24,
                "A long title wraps across narrow cards",
            ),
            (150, 22, 700, "mono", 32, "Wi il 12345"),
            (150, 22, 400, "sans", 28, "Wi il 12345"),
            (
                160,
                18,
                400,
                "sans",
                24,
                "Latin \u{4e2d}\u{6587} \u{1f600} \u{5e9}\u{5dc}\u{5d5}\u{5dd}",
            ),
            (
                80,
                18,
                400,
                "sans",
                24,
                "Latin \u{4e2d}\u{6587} \u{1f600} \u{5e9}\u{5dc}\u{5d5}\u{5dd}",
            ),
        ] {
            let mut label = node(
                2,
                Some(0),
                "label",
                &[
                    (4, json!(size)),
                    (36, json!(weight)),
                    (37, json!(height)),
                    (38, json!(family)),
                ],
            );
            label.text = text.into();
            let records = vec![node(0, None, "root", &[]), label];
            fresh.clear_text_cache();
            let actual = cached.layout(&records, width, 300, 1.0).unwrap();
            let expected = fresh.layout(&records, width, 300, 1.0).unwrap();
            assert_eq!(
                serde_json::to_value(&actual.nodes).unwrap(),
                serde_json::to_value(&expected.nodes).unwrap()
            );
            assert_eq!(
                actual.rasterize().unwrap().pixels.data_as_u8_slice(),
                expected.rasterize().unwrap().pixels.data_as_u8_slice()
            );
        }
    }

    #[test]
    fn text_ink_outside_a_zero_height_layout_box_is_not_culled() {
        let mut text = node(2, Some(0), "text", &[(1, json!(0)), (4, json!(24))]);
        text.text = "Visible overflow".into();
        let records = vec![node(0, None, "root", &[]), text];
        let display = Renderer::default().layout(&records, 240, 100, 1.0).unwrap();
        assert_eq!(display.nodes[1].visible_bounds[3], 0.0);
        assert!(
            display
                .commands
                .iter()
                .any(|command| matches!(command, Paint::Text { .. }))
        );
    }
}

fn image_geometry(
    bitmap: &crate::image::Bitmap,
    bounds: [f32; 4],
    radius: f32,
    fit: crate::image::Fit,
) -> Result<(PaintRect, Affine, vello_cpu::kurbo::BezPath), String> {
    let pixels = bitmap.pixels();
    let [x, y, w, h] = crate::image::placement([pixels.width(), pixels.height()], bounds, fit)?;
    let rect = PaintRect::new(x as f64, y as f64, (x + w) as f64, (y + h) as f64);
    let transform = Affine::translate((x as f64, y as f64))
        * Affine::scale_non_uniform(
            w as f64 / f64::from(pixels.width()),
            h as f64 / f64::from(pixels.height()),
        );
    let [x, y, w, h] = bounds;
    let clip = RoundedRect::new(
        x as f64,
        y as f64,
        (x + w) as f64,
        (y + h) as f64,
        radius as f64,
    )
    .to_path(0.1);
    Ok((rect, transform, clip))
}
