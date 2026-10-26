//! Per-node geometry snapshots published with each frame.
use serde::Serialize;
use vello_cpu::Pixmap;
use vello_cpu::kurbo::{Affine, RoundedRect, Shape};

#[derive(Clone, Serialize)]
pub struct NodeSnapshot {
    pub identity: String,
    pub tag: &'static str,
    pub test_id: String,
    pub text: String,
    pub bounds: [f32; 4],
    pub visible_bounds: [f32; 4],
    /// `text` or `all` when the text may be selected by long press.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selectable: Option<&'static str>,
    /// The current static-text selection, in the shape chrome reads from editors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_selection: Option<InputGeometry>,
    /// The selection as UTF-16 offsets into the node text, for accessibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_range: Option<[u32; 2]>,
    /// Line pieces of a text run; hits must land in one of them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hit_rects: Vec<[f32; 4]>,
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
    pub plot: Option<crate::scene::plot::PlotSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radio_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll_parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility_hidden: Option<bool>,
    /// Authored semantic role (`heading`, `list`, …); controls keep their own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading_level: Option<u8>,
    /// `polite` or `assertive`; alerts and statuses imply one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_region: Option<String>,
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
    /// Children are clipped to this node's padding box (`overflow: hidden`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub clips_children: bool,
    /// An `active:` paint delta exists; the session repaints it while pressed.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub pressed_style: bool,
    /// Position in paint order; later nodes draw above and receive hits first.
    pub order: u32,
    #[serde(skip)]
    pub(crate) plot_local: Option<([f32; 4], Affine)>,
}

impl NodeSnapshot {
    /// Resolve a plot item using the same rounded clips as rendering.
    pub fn hit_test_plot(&self, point: [f32; 2]) -> Option<usize> {
        if point.iter().any(|value| !value.is_finite())
            || !(ClipSnapshot {
                bounds: self.visible_bounds,
                radius: 0.0,
                radii: None,
                transform: None,
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
    /// Attributes the platform input connection maps onto its IME and autofill.
    #[serde(flatten)]
    pub attributes: EditorAttributes,
    pub edit: Option<crate::text::input::Snapshot>,
    pub edit_token: Option<u64>,
    pub geometry: Option<InputGeometry>,
    pub scroll_offset: [f32; 2],
    /// The platform keyboard's configuration, decided from the fields above.
    pub ime: crate::interaction::ime::ImeConfig,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct EditorAttributes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_complete: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enter_key_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_capitalize: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spell_check: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    /// `date` or `time`: the platform opens its picker instead of a keyboard.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picker: Option<String>,
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

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ClipSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radii: Option<[[f32; 2]; 4]>,
    pub bounds: [f32; 4],
    pub radius: f32,
    /// A rotating, skewing or mirroring affine applied to the rounded box;
    /// axis-aligned scales fold into `bounds` instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<[f64; 6]>,
}
impl ClipSnapshot {
    pub(crate) fn rounded_box(self) -> crate::scene::border::RoundedBox {
        let radius = self
            .radius
            .max(0.0)
            .min(self.bounds[2].min(self.bounds[3]) / 2.0);
        crate::scene::border::RoundedBox::normalized(
            self.bounds.map(f64::from),
            self.radii
                .unwrap_or([[radius; 2]; 4])
                .map(|pair| pair.map(f64::from)),
        )
    }
    pub(crate) fn affine(self) -> Option<Affine> {
        self.transform.map(Affine::new)
    }
    /// The axis-aligned box the clip shape covers.
    pub(crate) fn extent(self) -> [f32; 4] {
        let Some(transform) = self.affine() else {
            return self.bounds;
        };
        let [x, y, w, h] = self.bounds.map(f64::from);
        let b = transform.transform_rect_bbox(vello_cpu::kurbo::Rect::new(x, y, x + w, y + h));
        [b.x0, b.y0, b.width(), b.height()].map(|v| v as f32)
    }
    pub(crate) fn contains(self, point: [f32; 2]) -> bool {
        let point = match self.affine() {
            Some(transform) => {
                let local = transform.inverse()
                    * vello_cpu::kurbo::Point::new(f64::from(point[0]), f64::from(point[1]));
                [local.x as f32, local.y as f32]
            }
            None => point,
        };
        self.rounded_box().contains(point.map(f64::from))
    }
    pub(crate) fn intersects(self, bounds: [f32; 4]) -> bool {
        // Transformed shapes cull by their extent; paint clips them exactly.
        if self.transform.is_some() {
            let [_, _, w, h] = intersect(self.extent(), bounds);
            return w > 0.0 && h > 0.0;
        }
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
        let path = if self.radii.is_some() {
            self.rounded_box().path()
        } else {
            let [x, y, w, h] = self.bounds;
            RoundedRect::new(
                x as f64,
                y as f64,
                (x + w) as f64,
                (y + h) as f64,
                self.radius as f64,
            )
            .to_path(0.1)
        };
        match self.affine() {
            Some(transform) => transform * path,
            None => path,
        }
    }
    /// The clip in device pixels.
    pub(crate) fn scaled(mut self, scale: f32) -> Self {
        if let Some(transform) = self.affine() {
            self.transform = Some((Affine::scale(f64::from(scale)) * transform).as_coeffs());
            return self;
        }
        self.bounds = self.bounds.map(|value| value * scale);
        self.radius *= scale;
        self.radii = self
            .radii
            .map(|radii| radii.map(|pair| pair.map(|value| value * scale)));
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrollAxis {
    Horizontal,
    Vertical,
}

impl ScrollAxis {
    pub(crate) fn index(self) -> usize {
        match self {
            Self::Horizontal => 0,
            Self::Vertical => 1,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ScrollSnapshot {
    /// A right-to-left horizontal scroll: offsets grow toward the left.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub rtl: bool,
    /// `overscroll-behavior: contain`: leftover distance never reaches ancestors.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub contain: bool,
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

pub(crate) fn intersect(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let x = a[0].max(b[0]);
    let y = a[1].max(b[1]);
    [
        x,
        y,
        (a[0] + a[2]).min(b[0] + b[2]).max(x) - x,
        (a[1] + a[3]).min(b[1] + b[3]).max(y) - y,
    ]
}
