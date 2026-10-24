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
    pub edit: Option<crate::text::input::Snapshot>,
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

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ClipSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radii: Option<[[f32; 2]; 4]>,
    pub bounds: [f32; 4],
    pub radius: f32,
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
    pub(crate) fn contains(self, point: [f32; 2]) -> bool {
        self.rounded_box().contains(point.map(f64::from))
    }
    pub(crate) fn intersects(self, bounds: [f32; 4]) -> bool {
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
    pub(crate) fn index(self) -> usize {
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
