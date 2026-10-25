use crate::protocol::field;
use crate::protocol::{Record, color};

/// Platform-independent plot activation from the displayed frame.
pub enum PlotSelection {
    Point([f32; 2]),
    Next,
    Index(i64),
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PieLayout {
    pub center: [f64; 2],
    pub inner_radius: f64,
    pub outer_radius: f64,
    #[serde(default)]
    pub emphasis_scale: f64,
}
impl Default for PieLayout {
    fn default() -> Self {
        Self {
            center: [0.5, 0.5],
            inner_radius: 0.92 * 0.58,
            outer_radius: 0.92,
            emphasis_scale: 0.0,
        }
    }
}
impl PieLayout {
    fn parse(source: &str) -> Result<Self, String> {
        let layout: Self =
            serde_json::from_str(source).map_err(|_| "UI_PLOT: invalid pie layout")?;
        if layout
            .center
            .iter()
            .chain([&layout.inner_radius, &layout.outer_radius])
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || layout.inner_radius >= layout.outer_radius
            || !layout.emphasis_scale.is_finite()
            || !(0.0..=64.0).contains(&layout.emphasis_scale)
        {
            return Err("UI_PLOT: invalid pie layout".into());
        }
        Ok(layout)
    }
}

#[derive(Clone, serde::Serialize)]
pub struct PlotSnapshot {
    pub kind: String,
    pub values: Vec<f64>,
    pub domain: [f64; 2],
    pub pie_layout: PieLayout,
    pub selected_index: i64,
    pub on_select: Option<u64>,
}

impl PlotSnapshot {
    pub fn selectable(&self, index: usize) -> bool {
        self.on_select.is_some_and(|handle| handle != 0)
            && self
                .values
                .get(index)
                .is_some_and(|value| self.kind != "pie" || *value > 0.0)
    }

    pub fn next_selection(&self) -> Option<usize> {
        let count = self.values.len();
        if count == 0 {
            return None;
        }
        let start = usize::try_from(self.selected_index)
            .ok()
            .filter(|index| *index < count)
            .map_or(0, |index| (index + 1) % count);
        (0..count)
            .map(|offset| (start + offset) % count)
            .find(|index| self.selectable(*index))
    }

    /// Logical coordinates in the plot content box. The caller applies ancestor clips.
    pub fn hit_test(&self, bounds: [f32; 4], point: [f32; 2]) -> Option<usize> {
        let [left, top, width, height] = bounds.map(f64::from);
        let [x, y] = point.map(f64::from);
        if bounds
            .iter()
            .chain(point.iter())
            .any(|value| !value.is_finite())
            || width <= 0.0
            || height <= 0.0
            || self.values.is_empty()
            || x < left
            || y < top
            || x >= left + width
            || y >= top + height
        {
            return None;
        }
        if self.kind != "pie" {
            let padding = 4.0_f64.min(width / 4.0).min(height / 4.0);
            let index =
                (((x - left - padding) * self.values.len() as f64 / (width - 2.0 * padding))
                    .floor()
                    .max(0.0) as usize)
                    .min(self.values.len() - 1);
            return self.selectable(index).then_some(index);
        }
        let layout = &self.pie_layout;
        let dx = x - left - width * layout.center[0];
        let dy = y - top - height * layout.center[1];
        let radius = dx.hypot(dy);
        let unit = width.min(height) / 2.0;
        if radius < unit * layout.inner_radius {
            return None;
        }
        let total: f64 = self.values.iter().sum();
        if total <= 0.0 {
            return None;
        }
        let angle = (dy.atan2(dx) + std::f64::consts::FRAC_PI_2).rem_euclid(std::f64::consts::TAU);
        let target = angle / std::f64::consts::TAU * total;
        let mut accumulated = 0.0;
        for (index, value) in self.values.iter().enumerate() {
            accumulated += value;
            if *value > 0.0 && target < accumulated {
                let emphasis = if self.selected_index == index as i64 {
                    layout.emphasis_scale
                } else {
                    0.0
                };
                return (self.selectable(index) && radius <= unit * layout.outer_radius + emphasis)
                    .then_some(index);
            }
        }
        None
    }
}

pub(crate) struct Plot {
    pub kind: String,
    pub values: Vec<f64>,
    pub domain: [f64; 2],
    pub pie_layout: PieLayout,
    pub colors: Vec<[u8; 4]>,
    pub smooth: bool,
    pub area: bool,
    pub marker_emphasis_scale: f32,
    pub selected_index: i64,
    pub on_select: Option<u64>,
}

pub(crate) enum Shape {
    Bar([f32; 4], [u8; 4]),
    Circle([f32; 4], [u8; 4]),
    Stroke(Vec<[f32; 2]>, f32, [u8; 4]),
    Area(Vec<[f32; 2]>, [f32; 3], [u8; 4]),
}

impl Plot {
    pub fn from_record(record: &Record) -> Result<Self, String> {
        if record
            .string(field::ACCESSIBILITY_LABEL)
            .is_none_or(|s| s.trim().is_empty())
        {
            return Err("UI_PLOT: accessibilityLabel is required".into());
        }
        let kind = record
            .string(field::PLOT_KIND)
            .ok_or("UI_PLOT: missing kind")?;
        if !matches!(kind, "bar" | "line" | "pie") {
            return Err("UI_PLOT: invalid kind".into());
        }
        let values: Vec<f64> = serde_json::from_str(
            record
                .string(field::PLOT_VALUES)
                .ok_or("UI_PLOT: missing values")?,
        )
        .map_err(|_| "UI_PLOT: invalid values")?;
        if values.is_empty()
            || values.len() > 128
            || values
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1e9 || (kind == "pie" && *v < 0.0))
        {
            return Err(
                "UI_PLOT: expected 1..128 bounded values; pie values must be nonnegative".into(),
            );
        }
        let palette: Vec<String> = serde_json::from_str(
            record
                .string(field::PLOT_COLORS)
                .ok_or("UI_PLOT: missing colors")?,
        )
        .map_err(|_| "UI_PLOT: invalid colors")?;
        if palette.is_empty()
            || palette.len() > 16
            || palette
                .iter()
                .any(|s| !s.starts_with('#') || !matches!(s.len(), 7 | 9))
        {
            return Err("UI_PLOT: expected 1..16 colors".into());
        }
        let colors = palette.iter().map(|s| color(s)).collect::<Result<_, _>>()?;
        let smooth = record.number(field::PLOT_SMOOTH) == Some(1.0);
        let area = record.number(field::PLOT_AREA) == Some(1.0);
        let marker_emphasis_scale = record
            .number(field::PLOT_MARKER_EMPHASIS_SCALE)
            .unwrap_or(1.0);
        if record.number(field::PLOT_MARKER_EMPHASIS_SCALE).is_some()
            && (kind != "line" || !(1.0..=4.0).contains(&marker_emphasis_scale))
        {
            return Err("UI_PLOT: marker emphasis scale requires a line and range 1..4".into());
        }
        let selected_index = record
            .fields
            .iter()
            .find(|(id, _)| *id == field::PLOT_SELECTED_INDEX)
            .map(|(_, value)| value.as_i64().ok_or("UI_PLOT: invalid selected index"))
            .transpose()?
            .unwrap_or(-1);
        if selected_index < -1 || selected_index >= values.len() as i64 {
            return Err("UI_PLOT: selected index outside values".into());
        }
        let on_select = record
            .fields
            .iter()
            .find(|(id, _)| *id == field::ON_PLOT_SELECT)
            .and_then(|(_, value)| value.as_u64());
        if kind != "line" && (smooth || area) {
            return Err("UI_PLOT: smooth and area require a line plot".into());
        }
        let low = values.iter().copied().fold(0.0, f64::min);
        let high = values.iter().copied().fold(0.0, f64::max);
        let bound = |id| {
            record
                .fields
                .iter()
                .find(|(field, _)| *field == id)
                .and_then(|(_, value)| value.as_f64())
        };
        let domain = match (bound(85), bound(86)) {
            (None, None) => [low, high],
            (Some(min), Some(max)) if kind != "pie" && min < max && min <= low && max >= high => {
                [min, max]
            }
            _ => return Err("UI_PLOT: domain must contain zero and all values".into()),
        };
        let pie_layout = match record.string(field::PLOT_PIE_LAYOUT) {
            Some(source) if kind == "pie" => PieLayout::parse(source)?,
            Some(_) => return Err("UI_PLOT: pie layout requires a pie".into()),
            None => PieLayout::default(),
        };
        Ok(Self {
            pie_layout,
            kind: kind.into(),
            domain,
            values,
            colors,
            smooth,
            area,
            marker_emphasis_scale,
            selected_index,
            on_select,
        })
    }

    pub fn snapshot(&self) -> PlotSnapshot {
        PlotSnapshot {
            kind: self.kind.clone(),
            domain: self.domain,
            pie_layout: self.pie_layout.clone(),
            values: self.values.clone(),
            selected_index: self.selected_index,
            on_select: self.on_select,
        }
    }

    pub fn work(&self) -> usize {
        let count = self.values.len();
        match self.kind.as_str() {
            "pie" => 256 + count,
            "line" => {
                let points = (count - 1) * if self.smooth { 8 } else { 1 } + 1;
                points * if self.area { 2 } else { 1 } + 2 * count + if self.area { 2 } else { 0 }
            }
            _ => 8 * count,
        }
    }

    pub fn shapes(&self, bounds: [f32; 4]) -> Vec<Shape> {
        let [x, y, w, h] = bounds;
        if w <= 0.0 || h <= 0.0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        if self.kind == "pie" {
            let total: f64 = self.values.iter().sum();
            if total == 0.0 {
                return out;
            }
            let outer = w.min(h) * 0.5 * self.pie_layout.outer_radius as f32;
            let inner = w.min(h) * 0.5 * self.pie_layout.inner_radius as f32;
            let center = [
                x + w * self.pie_layout.center[0] as f32,
                y + h * self.pie_layout.center[1] as f32,
            ];
            let mut angle = -std::f64::consts::FRAC_PI_2;
            for (i, value) in self.values.iter().enumerate() {
                if *value == 0.0 {
                    continue;
                }
                let sweep = value / total * std::f64::consts::TAU;
                let outer = outer
                    + if self.selected_index == i as i64 {
                        self.pie_layout.emphasis_scale as f32
                    } else {
                        0.0
                    };
                let thickness = outer - inner;
                let radius = outer - thickness / 2.0;
                // Keep each slice in one path so antialiasing cannot expose
                // seams between independently composited angular segments.
                let steps = (sweep / std::f64::consts::TAU * 256.0).ceil() as usize;
                let points = (0..=steps)
                    .map(|step| {
                        let theta = angle + sweep * step as f64 / steps as f64;
                        [
                            center[0] + theta.cos() as f32 * radius,
                            center[1] + theta.sin() as f32 * radius,
                        ]
                    })
                    .collect();
                out.push(Shape::Stroke(
                    points,
                    thickness,
                    self.colors[i % self.colors.len()],
                ));
                angle += sweep;
            }
            return out;
        }
        let [low, high] = self.domain;
        let span = if high > low { high - low } else { 1.0 };
        let padding = 4.0_f32.min(w / 4.0).min(h / 4.0);
        let width = w - 2.0 * padding;
        let height = h - 2.0 * padding;
        let map_y = |v: f64| y + padding + ((high - v) / span) as f32 * height;
        let baseline = map_y(0.0);
        let step = width / self.values.len() as f32;
        let points: Vec<[f32; 2]> = self
            .values
            .iter()
            .enumerate()
            .map(|(i, v)| [x + padding + step * (i as f32 + 0.5), map_y(*v)])
            .collect();
        if self.kind == "bar" {
            let bar_width = (step * 0.65).min(36.0);
            for (i, point) in points.iter().enumerate() {
                out.push(Shape::Bar(
                    [
                        point[0] - bar_width / 2.0,
                        point[1].min(baseline),
                        bar_width,
                        (point[1] - baseline).abs(),
                    ],
                    self.colors[i % self.colors.len()],
                ));
            }
        } else {
            let curve = if self.smooth {
                smooth_points(&points)
            } else {
                points.clone()
            };
            if self.area && curve.len() > 1 {
                let mut polygon = curve.clone();
                polygon.push([curve.last().unwrap()[0], baseline]);
                polygon.push([curve[0][0], baseline]);
                let mut color = self.colors[0];
                color[3] = (u16::from(color[3]) * 56 / 255) as u8;
                out.push(Shape::Area(
                    polygon,
                    [y + padding, baseline, y + h - padding],
                    color,
                ));
            }
            if curve.len() > 1 {
                out.push(Shape::Stroke(
                    curve,
                    2.5_f32.min(padding * 2.0),
                    self.colors[0],
                ));
            }
            for (index, point) in points.into_iter().enumerate() {
                // ECharts' six-pixel emptyCircle has a centered two-pixel stroke.
                let diameter = 8.0_f32.min(padding * 2.0);
                let stroke = diameter / 4.0;
                let body = (diameter - stroke)
                    * if self.selected_index == index as i64 {
                        self.marker_emphasis_scale
                    } else {
                        1.0
                    };
                // Empty-circle emphasis scales the path, not its two-pixel stroke.
                for (size, color) in [
                    (body + stroke, self.colors[0]),
                    (body - stroke, [255, 255, 255, 255]),
                ] {
                    out.push(Shape::Circle(
                        [point[0] - size / 2.0, point[1] - size / 2.0, size, size],
                        color,
                    ));
                }
            }
        }
        out
    }
}

fn smooth_points(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let slopes: Vec<f64> = points
        .windows(2)
        .map(|p| f64::from(p[1][1] - p[0][1]))
        .collect();
    let mut tangents = vec![slopes[0]];
    for pair in slopes.windows(2) {
        tangents.push(if pair[0] * pair[1] <= 0.0 {
            0.0
        } else {
            2.0 * pair[0] * pair[1] / (pair[0] + pair[1])
        });
    }
    tangents.push(*slopes.last().unwrap());
    let mut result = vec![points[0]];
    for (index, pair) in points.windows(2).enumerate() {
        for step in 1..=8 {
            let t = f64::from(step) / 8.0;
            let t2 = t * t;
            let t3 = t2 * t;
            let y = (2.0 * t3 - 3.0 * t2 + 1.0) * f64::from(pair[0][1])
                + (t3 - 2.0 * t2 + t) * tangents[index]
                + (-2.0 * t3 + 3.0 * t2) * f64::from(pair[1][1])
                + (t3 - t2) * tangents[index + 1];
            result.push([pair[0][0] + (pair[1][0] - pair[0][0]) * t as f32, y as f32]);
        }
    }
    result
}

#[cfg(test)]
#[path = "tests/plot.rs"]
mod tests;
