use crate::schema::{Record, color};

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
        if record.string(41).is_none_or(|s| s.trim().is_empty()) {
            return Err("UI_PLOT: accessibilityLabel is required".into());
        }
        let kind = record.string(52).ok_or("UI_PLOT: missing kind")?;
        if !matches!(kind, "bar" | "line" | "pie") {
            return Err("UI_PLOT: invalid kind".into());
        }
        let values: Vec<f64> =
            serde_json::from_str(record.string(51).ok_or("UI_PLOT: missing values")?)
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
        let palette: Vec<String> =
            serde_json::from_str(record.string(53).ok_or("UI_PLOT: missing colors")?)
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
        let smooth = record.number(54) == Some(1.0);
        let area = record.number(55) == Some(1.0);
        let marker_emphasis_scale = record.number(94).unwrap_or(1.0);
        if record.number(94).is_some()
            && (kind != "line" || !(1.0..=4.0).contains(&marker_emphasis_scale))
        {
            return Err("UI_PLOT: marker emphasis scale requires a line and range 1..4".into());
        }
        let selected_index = record
            .fields
            .iter()
            .find(|(id, _)| *id == 57)
            .map(|(_, value)| value.as_i64().ok_or("UI_PLOT: invalid selected index"))
            .transpose()?
            .unwrap_or(-1);
        if selected_index < -1 || selected_index >= values.len() as i64 {
            return Err("UI_PLOT: selected index outside values".into());
        }
        let on_select = record
            .fields
            .iter()
            .find(|(id, _)| *id == 56)
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
        let pie_layout = match record.string(88) {
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
mod tests {
    use super::*;
    #[test]
    fn shared_selection_handles_categories_empty_slices_and_expanded_edges() {
        let mut plot = Plot::from_record(&record("bar", "[10,20,30,40]"))
            .unwrap()
            .snapshot();
        plot.on_select = Some(1);
        let bounds = [10.0, 20.0, 100.0, 100.0];
        for (x, expected) in [(10.0, 0), (36.9, 0), (37.0, 1), (60.0, 2), (109.9, 3)] {
            assert_eq!(plot.hit_test(bounds, [x, 70.0]), Some(expected));
        }
        for point in [[110.0, 70.0], [60.0, 120.0], [f32::NAN, 70.0]] {
            assert_eq!(plot.hit_test(bounds, point), None);
        }
        plot.kind = "pie".into();
        plot.values = vec![0.0, 1.0, 0.0, 1.0];
        plot.pie_layout = PieLayout {
            center: [0.5, 0.5],
            inner_radius: 0.4,
            outer_radius: 0.8,
            emphasis_scale: 5.0,
        };
        plot.selected_index = 1;
        assert_eq!(plot.next_selection(), Some(3));
        assert_eq!(plot.hit_test(bounds, [103.0, 70.0]), Some(1));
        assert_eq!(plot.hit_test(bounds, [17.0, 70.0]), None);
        assert_eq!(plot.hit_test(bounds, [60.0, 70.0]), None);
        assert_eq!(plot.hit_test(bounds, [80.0, 70.0]), Some(1));
        plot.on_select = None;
        assert_eq!(plot.next_selection(), None);
        assert_eq!(plot.hit_test(bounds, [103.0, 70.0]), None);
    }

    #[test]
    fn shared_plot_hit_uses_rendered_content_and_ancestor_clips() {
        let source = crate::session::tests::source(
            "function render(){return h('scroll',{style:{width:80,height:80,radius:30}},h('plot',{kind:'bar',values:[1,2],colors:['#ff0000'],onSelect:()=>{},accessibilityLabel:'Series',style:{width:100,height:100}}))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(120, 120, 1.0).unwrap();
        let node = display
            .nodes
            .iter()
            .find(|node| node.plot.is_some())
            .unwrap();
        assert_eq!(node.hit_test_plot([40.0, 40.0]), Some(0));
        assert_eq!(node.hit_test_plot([1.0, 1.0]), None);
        assert_eq!(node.hit_test_plot([90.0, 40.0]), None);
    }

    #[test]
    fn shared_selection_dispatch_rejects_stale_frames_and_wrong_nodes() {
        use crate::PlotSelection::{Index, Next, Point};
        let source = crate::session::tests::source(
            "function render(){const selected=useLocal('selected',-1);return h('plot',{kind:'pie',values:[1,0,1],colors:['#0000ff'],accessibilityLabel:'Series',selectedIndex:selected.value,onSelect:index=>selected.set(index),style:{width:100,height:100}})}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let frame = page.layout(100, 100, 1.0).unwrap();
        let identity = frame.nodes[0].identity.clone();
        let generation = page.generation();
        assert!(!page.select_plot(generation, "missing", Next).unwrap());
        assert!(!page.select_plot(generation, &identity, Index(1)).unwrap());
        assert!(!page.select_plot(generation, &identity, Index(-2)).unwrap());
        assert!(
            !page
                .select_plot(generation, &identity, Point([f32::NAN, 50.0]))
                .unwrap()
        );
        assert!(
            page.select_plot(generation, &identity, Point([90.0, 50.0]))
                .unwrap()
        );
        let frame = page.layout(100, 100, 1.0).unwrap();
        assert_eq!(frame.nodes[0].plot.as_ref().unwrap().selected_index, 0);
        assert!(!page.select_plot(generation, &identity, Index(-1)).unwrap());
        assert!(
            page.select_plot(page.generation(), &identity, Next)
                .unwrap()
        );
        let frame = page.layout(100, 100, 1.0).unwrap();
        assert_eq!(frame.nodes[0].plot.as_ref().unwrap().selected_index, 2);
        assert!(
            page.select_plot(page.generation(), &identity, Point([50.0, 50.0]))
                .unwrap()
        );
        let frame = page.layout(100, 100, 1.0).unwrap();
        assert_eq!(frame.nodes[0].plot.as_ref().unwrap().selected_index, -1);
        assert!(
            !page
                .select_plot(page.generation(), &identity, Index(-1))
                .unwrap()
        );
    }

    #[test]
    fn marker_emphasis_preserves_stroke_and_changes_only_selected_circle() {
        let mut line_record = record("line", "[10,20]");
        line_record
            .fields
            .extend([(94, serde_json::json!(1.1)), (57, serde_json::json!(0))]);
        let plot = Plot::from_record(&line_record).unwrap();
        let shapes = plot.shapes([0.0, 0.0, 100.0, 100.0]);
        let diameters: Vec<f32> = shapes
            .iter()
            .filter_map(|shape| match shape {
                Shape::Circle(bounds, _) => Some(bounds[2]),
                _ => None,
            })
            .collect();
        for (actual, expected) in diameters.iter().zip([8.6, 4.6, 8.0, 4.0]) {
            assert!((actual - expected).abs() < 0.001);
        }
        assert_eq!(diameters.len(), 4);
        for kind in ["bar", "pie"] {
            let mut record = record(kind, "[10,20]");
            record.fields.push((94, serde_json::json!(1.1)));
            assert!(Plot::from_record(&record).is_err());
        }
    }
    #[test]
    fn marker_emphasis_raster_expands_white_center_and_blue_ring() {
        for selected in [-1, 0, 1] {
            let source = crate::session::tests::source(&format!(
                "function render(){{return h('plot',{{kind:'line',values:[10,20],colors:['#0000ff'],selectedIndex:{selected},markerEmphasisScale:2,accessibilityLabel:'Series',style:{{width:100,height:100,background:'#ffffff'}}}})}}"
            ));
            let mut page = crate::ViewSession::new(&source).unwrap();
            let frame = page.layout(100, 100, 1.0).unwrap().rasterize().unwrap();
            let pixels = frame.pixels.data_as_u8_slice();
            let pixel = |x: usize| &pixels[(50 * 100 + x) * 4..(50 * 100 + x) * 4 + 4];
            let white = [255, 255, 255, 255];
            let (ring, outside) = if selected == 0 {
                (pixel(33), pixel(30))
            } else {
                (pixel(30), pixel(33))
            };
            assert!(ring[0] < 64 && ring[1] < 64 && ring[2] == 255 && ring[3] == 255);
            assert_eq!(outside, &white);
            assert_eq!(pixel(35), &white);
        }
        for (tag, value) in [
            (18, serde_json::json!(0.9)),
            (18, serde_json::json!(4.1)),
            (1, serde_json::json!(1.1)),
        ] {
            let wire = serde_json::json!([tag, -1, "", "", "", 1, 94, value]);
            assert!(crate::schema::decode(wire.as_array().unwrap()).is_err());
        }
    }
    #[test]
    fn expanded_pie_is_clipped_to_its_unbordered_plot_viewport() {
        let source = crate::session::tests::source(
            "function render(){return h('column',{style:{background:'#ffffff',padding:20}},h('plot',{kind:'pie',values:[1,1],colors:['#ff0000','#0000ff'],selectedIndex:0,accessibilityLabel:'Series',pieLayout:{center:[0.5,0.5],innerRadius:0.1,outerRadius:0.8,emphasisScale:64},style:{width:100,height:100}}))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(160, 160, 1.0).unwrap();
        let frame = display.rasterize().unwrap();
        let pixels = frame.pixels.data_as_u8_slice();
        let pixel = |x: usize, y: usize| &pixels[(y * 160 + x) * 4..(y * 160 + x) * 4 + 4];
        assert_eq!(pixel(118, 70), &[255, 0, 0, 255]);
        for (x, y) in [(121, 70), (150, 70), (100, 19), (100, 121)] {
            assert_eq!(pixel(x, y), &[255, 255, 255, 255]);
        }
    }
    #[test]
    fn pie_emphasis_expands_only_selected_outer_edge_in_logical_pixels() {
        for scale in [1.0, 2.0] {
            for selected in [-1, 0, 1] {
                let source = crate::session::tests::source(&format!(
                    "function render(){{return h('plot',{{kind:'pie',values:[1,1],colors:['#ff0000','#0000ff'],selectedIndex:{selected},accessibilityLabel:'Series',pieLayout:{{center:[0.5,0.44],innerRadius:0.4,outerRadius:0.68,emphasisScale:5}},style:{{width:300,height:210,background:'#ffffff'}}}})}}"
                ));
                let mut page = crate::ViewSession::new(&source).unwrap();
                let display = page
                    .layout((300.0 * scale) as u16, (210.0 * scale) as u16, scale)
                    .unwrap();
                let frame = display.rasterize().unwrap();
                let pixels = frame.pixels.data_as_u8_slice();
                let pixel = |x: f32| {
                    let index = ((92.0 * scale) as usize * (300.0 * scale) as usize
                        + (x * scale) as usize)
                        * 4;
                    &pixels[index..index + 4]
                };
                let white = [255, 255, 255, 255];
                assert_eq!(
                    pixel(223.0),
                    if selected == 0 {
                        &[255, 0, 0, 255]
                    } else {
                        &white
                    }
                );
                assert_eq!(
                    pixel(76.0),
                    if selected == 1 {
                        &[0, 0, 255, 255]
                    } else {
                        &white
                    }
                );
                for x in [110.0, 190.0, 72.0, 228.0] {
                    assert_eq!(pixel(x), &white);
                }
            }
        }
        for value in [-1.0, 65.0] {
            let source = format!(
                r#"{{"center":[0.5,0.5],"innerRadius":0.4,"outerRadius":0.68,"emphasisScale":{value}}}"#
            );
            assert!(PieLayout::parse(&source).is_err());
        }
    }
    #[test]
    fn configured_pie_radii_and_center_match_painted_pixels() {
        let source = crate::session::tests::source(
            "function render(){return h('plot',{kind:'pie',values:[1,1],colors:['#ff0000','#0000ff'],accessibilityLabel:'Series',pieLayout:{center:[0.5,0.44],innerRadius:0.4,outerRadius:0.68},style:{width:300,height:210,background:'#ffffff'}})}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(300, 210, 1.0).unwrap();
        assert_eq!(
            display.nodes[0].plot.as_ref().unwrap().pie_layout.center,
            [0.5, 0.44]
        );
        let frame = display.rasterize().unwrap();
        let pixels = frame.pixels.data_as_u8_slice();
        let pixel = |x: usize, y: usize| &pixels[(y * 300 + x) * 4..(y * 300 + x) * 4 + 4];
        for x in [150, 190, 223] {
            assert_eq!(pixel(x, 92), &[255, 255, 255, 255]);
        }
        for x in [194, 219] {
            assert_eq!(pixel(x, 92), &[255, 0, 0, 255]);
        }
        assert_eq!(pixel(95, 92), &[0, 0, 255, 255]);
    }
    #[test]
    fn pie_layout_wire_requires_bounded_json_on_plot() {
        for (tag, value) in [
            (1, serde_json::json!("{}")),
            (18, serde_json::json!(42)),
            (18, serde_json::json!(" ".repeat(1025))),
        ] {
            let wire = serde_json::json!([tag, -1, "", "", "", 1, 88, value]);
            assert!(crate::schema::decode(wire.as_array().unwrap()).is_err());
        }
    }
    #[test]
    fn pie_layout_rejects_invalid_geometry_and_non_pies() {
        for source in [
            "{}",
            "null",
            r#"{"center":[0.5,0.5],"innerRadius":0.8,"outerRadius":0.6}"#,
            r#"{"center":[-1,0.5],"innerRadius":0.4,"outerRadius":0.6}"#,
            r#"{"center":[0.5,0.5],"innerRadius":0.4,"outerRadius":0.6,"extra":1}"#,
        ] {
            assert!(PieLayout::parse(source).is_err());
        }
        let mut record = record("bar", "[1]");
        record.fields.push((
            88,
            serde_json::json!(r#"{"center":[0.5,0.44],"innerRadius":0.4,"outerRadius":0.68}"#),
        ));
        assert!(Plot::from_record(&record).is_err());
    }
    #[test]
    fn signed_bars_round_only_top_corners() {
        let source = crate::session::tests::source(
            "function render(){return h('plot',{kind:'bar',values:[10,-10],domainMin:-20,domainMax:20,colors:['#3b82f6'],accessibilityLabel:'Series',style:{width:100,height:128,background:'#101010'}})}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let frame = page.layout(400, 512, 4.0).unwrap().rasterize().unwrap();
        let pixels = frame.pixels.data_as_u8_slice();
        let pixel = |x: usize, y: usize| &pixels[(y * 400 + x) * 4..(y * 400 + x) * 4 + 4];
        for (x, top, bottom) in [
            (49, 137, 254),
            (166, 137, 254),
            (233, 257, 374),
            (350, 257, 374),
        ] {
            assert_eq!(pixel(x, top), &[16, 16, 16, 255], "top corner");
            assert_eq!(
                pixel(x, bottom),
                &[59, 130, 246, 255],
                "square bottom corner"
            );
        }
    }
    #[test]
    fn line_markers_have_white_centers_blue_rings_and_round_corners() {
        let source = crate::session::tests::source(
            "function render(){return h('plot',{kind:'line',values:[10,10],domainMin:0,domainMax:20,colors:['#3b82f6'],accessibilityLabel:'Series',style:{width:100,height:128,background:'#101010'}})}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let frame = page.layout(400, 512, 4.0).unwrap().rasterize().unwrap();
        let pixels = frame.pixels.data_as_u8_slice();
        let pixel = |x: usize, y: usize| &pixels[(y * 400 + x) * 4..(y * 400 + x) * 4 + 4];
        assert_eq!(pixel(108, 256), &[255, 255, 255, 255]);
        assert_eq!(pixel(120, 256), &[59, 130, 246, 255]);
        assert_eq!(pixel(121, 269), &[16, 16, 16, 255]);
    }
    #[test]
    fn selection_snapshot_and_events_follow_the_committed_generation() {
        let source = crate::session::tests::source(
            "function render(){const selected=useLocal('selected',-1);return h('plot',{kind:'line',values:[10,20],colors:['#0000ff'],accessibilityLabel:'Series',selectedIndex:selected.value,onSelect:index=>selected.set(index)})}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let frame = page.layout(100, 100, 1.0).unwrap();
        let plot = frame.nodes[0].plot.as_ref().unwrap();
        assert_eq!(plot.selected_index, -1);
        assert_eq!(plot.values, [10.0, 20.0]);
        let generation = page.generation();
        let handle = plot.on_select.unwrap();
        page.number_event(generation, handle, 2.0).unwrap();
        assert_eq!(
            page.layout(100, 100, 1.0).unwrap().nodes[0]
                .plot
                .as_ref()
                .unwrap()
                .selected_index,
            -1
        );
        let frame = page.layout(100, 100, 1.0).unwrap();
        let current = frame.nodes[0].plot.as_ref().unwrap().on_select.unwrap();
        page.number_event(page.generation(), current, 1.0).unwrap();
        assert_eq!(
            page.layout(100, 100, 1.0).unwrap().nodes[0]
                .plot
                .as_ref()
                .unwrap()
                .selected_index,
            1
        );
        assert!(!page.number_event(generation, handle, 0.0).unwrap());
        let frame = page.layout(100, 100, 1.0).unwrap();
        let clear = frame.nodes[0].plot.as_ref().unwrap().on_select.unwrap();
        page.number_event(page.generation(), clear, -1.0).unwrap();
        assert_eq!(
            page.layout(100, 100, 1.0).unwrap().nodes[0]
                .plot
                .as_ref()
                .unwrap()
                .selected_index,
            -1
        );
        for value in [-2, 2, 128] {
            let mut record = record("line", "[10,20]");
            record.fields.push((57, value.into()));
            assert!(Plot::from_record(&record).is_err());
        }
    }
    #[test]
    fn selection_wire_rejects_invalid_handles_indices_and_other_tags() {
        let wire = serde_json::json!([
            18,
            -1,
            "",
            "",
            "",
            6,
            41,
            "Chart",
            51,
            "[10,20]",
            52,
            "line",
            53,
            "[\"#123456\"]",
            56,
            1,
            57,
            -1
        ]);
        let base = wire.as_array().unwrap();
        assert!(crate::schema::decode(base).is_ok());
        for bad in [
            serde_json::json!(0),
            serde_json::json!(-1),
            serde_json::json!(true),
            serde_json::json!(1.5),
            serde_json::json!(9_007_199_254_740_992_u64),
        ] {
            let mut buffer = base.clone();
            buffer[15] = bad;
            assert!(crate::schema::decode(&buffer).is_err());
        }
        for bad in [
            serde_json::json!(-2),
            serde_json::json!(2),
            serde_json::json!("0"),
            serde_json::json!(0.5),
            serde_json::json!(true),
        ] {
            let mut buffer = base.clone();
            buffer[17] = bad;
            assert!(crate::schema::decode(&buffer).is_err());
        }
        for field in [56, 57] {
            let wire = serde_json::json!([3, -1, "", "", "", 1, field, 1]);
            assert!(crate::schema::decode(wire.as_array().unwrap()).is_err());
        }
    }
    fn record(kind: &str, values: &str) -> Record {
        Record {
            tag: 18,
            parent: None,
            key: String::new(),
            test_id: String::new(),
            text: String::new(),
            fields: vec![
                (41, "Chart".into()),
                (51, values.into()),
                (52, kind.into()),
                (53, "[\"#435DD9\",\"#10B981\"]".into()),
            ],
        }
    }
    #[test]
    fn explicit_domain_scales_geometry_and_preserves_exact_data() {
        for kind in ["bar", "line"] {
            let mut data = record(kind, "[-10,20]");
            data.fields
                .extend([(85, (-20.0).into()), (86, 40.0.into())]);
            let plot = Plot::from_record(&data).unwrap();
            assert_eq!(plot.snapshot().domain, [-20.0, 40.0]);
            assert_eq!(plot.snapshot().values, [-10.0, 20.0]);
            let shapes = plot.shapes([0.0, 0.0, 100.0, 128.0]);
            if kind == "bar" {
                let Shape::Bar(first, _) = shapes[0] else {
                    panic!("bar")
                };
                let Shape::Bar(second, _) = shapes[1] else {
                    panic!("bar")
                };
                assert_eq!([first[1], first[3]], [84.0, 20.0]);
                assert_eq!([second[1], second[3]], [44.0, 40.0]);
            } else {
                let Shape::Stroke(ref points, _, _) = shapes[0] else {
                    panic!("line")
                };
                assert_eq!([points[0][1], points[1][1]], [104.0, 44.0]);
            }
        }
        for (kind, min, max) in [
            ("bar", 0.0, 10.0),
            ("bar", 10.0, 30.0),
            ("bar", 20.0, 20.0),
            ("pie", 0.0, 30.0),
        ] {
            let mut data = record(kind, "[10,20]");
            data.fields.extend([(85, min.into()), (86, max.into())]);
            assert!(Plot::from_record(&data).is_err());
        }
        let mut data = record("line", "[0.1,0.3]");
        data.fields.extend([(85, 0.0.into()), (86, 0.3.into())]);
        assert!(
            Plot::from_record(&data).is_ok(),
            "bounds retain f64 precision"
        );
        data.fields.pop();
        assert!(Plot::from_record(&data).is_err());
    }
    #[test]
    fn wire_domain_fields_require_numeric_bounds_on_plots() {
        for (tag, min, max) in [
            (18, serde_json::json!(0), serde_json::json!(30)),
            (18, serde_json::json!("0"), serde_json::json!(30)),
            (18, serde_json::json!(0), serde_json::json!(1e10)),
            (18, serde_json::json!(0), serde_json::json!(10)),
            (0, serde_json::json!(0), serde_json::json!(30)),
        ] {
            let mut fields = record("bar", "[10,20]").fields;
            fields.extend([(85, min.clone()), (86, max.clone())]);
            let mut wire = vec![
                serde_json::json!(tag),
                serde_json::json!(-1),
                "".into(),
                "".into(),
                "".into(),
                fields.len().into(),
            ];
            for (id, value) in fields {
                wire.extend([id.into(), value]);
            }
            assert_eq!(
                crate::schema::decode(&wire).is_ok(),
                tag == 18 && min == 0 && max == 30
            );
        }
    }
    #[test]
    fn smoothing_preserves_samples_without_overshooting_each_interval() {
        let points = [[0.0, 0.0], [10.0, 30.0], [20.0, -10.0], [30.0, 12.0]];
        let curve = smooth_points(&points);
        assert_eq!(curve.len(), 25);
        for (i, pair) in points.windows(2).enumerate() {
            assert_eq!(curve[i * 8], pair[0]);
            for point in &curve[i * 8..=i * 8 + 8] {
                assert!((pair[0][0]..=pair[1][0]).contains(&point[0]));
                assert!(
                    (pair[0][1].min(pair[1][1])..=pair[0][1].max(pair[1][1])).contains(&point[1])
                );
            }
        }
        assert_eq!(curve[24], points[3]);
    }

    #[test]
    fn area_fades_toward_zero_and_translates_with_scroll() {
        let source = crate::session::tests::source(
            "function render(){return h('scroll',{style:{height:80}},h('plot',{kind:'line',values:[10,10],colors:['#ff0000'],smooth:true,area:true,accessibilityLabel:'Area',style:{width:100,height:200,background:'#ffffff'}}))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let first = page.layout(100, 80, 1.0).unwrap();
        let id = first.nodes[0].scroll.as_ref().unwrap().id.clone();
        let pixels = first.rasterize().unwrap();
        let at = |frame: &crate::Frame, x: usize, y: usize| {
            frame.pixels.data_as_u8_slice()[(y * 100 + x) * 4 + 1]
        };
        assert!(at(&pixels, 50, 15) < at(&pixels, 50, 60));
        assert!(at(&pixels, 50, 15) > 150);
        assert_eq!(at(&pixels, 10, 60), 255);
        let shifted = page
            .prepare_native(
                100,
                80,
                1.0,
                false,
                Some(&std::collections::HashMap::from([(id, 40.0)])),
                Ok,
            )
            .unwrap()
            .rasterize()
            .unwrap();
        assert_eq!(at(&pixels, 50, 60), at(&shifted, 50, 20));
    }

    #[test]
    fn area_flags_and_expanded_geometry_are_bounded_on_the_wire() {
        let wire = serde_json::json!([
            18,
            -1,
            "",
            "",
            "",
            6,
            41,
            "Chart",
            51,
            "[1,2]",
            52,
            "line",
            53,
            "[\"#ff0000\"]",
            54,
            1,
            55,
            1
        ]);
        let mut buffer = wire.as_array().unwrap().clone();
        assert!(crate::schema::decode(&buffer).is_ok());
        for value in [
            serde_json::json!(2),
            serde_json::json!(true),
            serde_json::json!(-1),
        ] {
            buffer[17] = value;
            assert!(crate::schema::decode(&buffer).is_err());
        }
        for kind in ["bar", "pie"] {
            let mut item = record(kind, "[1,2]");
            item.fields.push((54, 1.into()));
            assert!(Plot::from_record(&item).is_err());
        }
        let mut item = record("line", &serde_json::to_string(&vec![1; 128]).unwrap());
        item.fields.extend([(54, 1.into()), (55, 1.into())]);
        let plot = Plot::from_record(&item).unwrap();
        assert_eq!(plot.work(), 2292);
        let geometry = plot.shapes([0.0, 0.0, 100.0, 100.0]);
        assert!(matches!(&geometry[0], Shape::Area(points, _, _) if points.len() == 1019));
        let mut many = serde_json::json!([0, -1, "", "", "", 0])
            .as_array()
            .unwrap()
            .clone();
        for i in 0..8 {
            let mut node = wire.as_array().unwrap().clone();
            node[1] = 0.into();
            node[2] = i.to_string().into();
            node[9] = serde_json::to_string(&vec![1; 128]).unwrap().into();
            many.extend(node);
        }
        assert!(
            crate::schema::decode(&many)
                .unwrap_err()
                .contains("plot geometry budget")
        );
    }

    #[test]
    fn negative_area_fades_to_its_top_baseline_on_a_dark_background() {
        let source = crate::session::tests::source(
            "function render(){return h('plot',{kind:'line',values:[-10,-10],colors:['#0000ff'],smooth:true,area:true,accessibilityLabel:'Negative area',style:{width:100,height:100,background:'#101010'}})}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let frame = page.layout(100, 100, 1.0).unwrap().rasterize().unwrap();
        let blue = |x: usize, y: usize| frame.pixels.data_as_u8_slice()[(y * 100 + x) * 4 + 2];
        assert!(blue(50, 15) > 16);
        assert!(blue(50, 85) > blue(50, 15));
        assert_eq!(blue(10, 50), 16);
    }
    #[test]
    fn rejects_unbounded_or_invalid_series() {
        for (kind, values) in [
            ("pie", "[-1]"),
            ("bar", "[]"),
            ("line", "[1e10]"),
            ("other", "[1]"),
            ("bar", "[null]"),
        ] {
            assert!(Plot::from_record(&record(kind, values)).is_err());
        }
        assert!(
            Plot::from_record(&record(
                "bar",
                &serde_json::to_string(&vec![1; 129]).unwrap()
            ))
            .is_err()
        );
    }
    #[test]
    fn negative_bars_share_zero_baseline_and_donut_work_is_bounded() {
        let plot = Plot::from_record(&record("bar", "[-10,10]")).unwrap();
        let shapes = plot.shapes([0.0, 0.0, 100.0, 100.0]);
        let Shape::Bar(first, _) = shapes[0] else {
            panic!()
        };
        let Shape::Bar(second, _) = shapes[1] else {
            panic!()
        };
        assert_eq!(first[1], 50.0);
        assert_eq!(second[1] + second[3], 50.0);
        let pie = Plot::from_record(&record(
            "pie",
            &serde_json::to_string(&vec![1; 128]).unwrap(),
        ))
        .unwrap();
        let slices = pie.shapes([0.0, 0.0, 100.0, 100.0]);
        assert_eq!(slices.len(), 128);
        let segments: usize = slices
            .iter()
            .map(|shape| match shape {
                Shape::Stroke(points, _, _) => points.len() - 1,
                Shape::Bar(..) | Shape::Circle(..) | Shape::Area(..) => {
                    panic!("pie must contain continuous arcs")
                }
            })
            .sum();
        assert!(segments <= 384);
        assert!(
            Plot::from_record(&record("pie", "[0,0]"))
                .unwrap()
                .shapes([0.0, 0.0, 100.0, 100.0])
                .is_empty()
        );
    }

    #[test]
    fn native_decode_and_reference_pixels_cover_each_plot_kind() {
        for kind in ["bar", "line", "pie"] {
            let source = crate::session::tests::source(&format!(
                "function render(){{return h('plot',{{accessibilityLabel:'Chart',kind:'{kind}',values:[10,20],colors:['#ff0000','#0000ff'],style:{{width:100,height:100,background:'#ffffff'}}}})}}"
            ));
            let mut page = crate::ViewSession::new(&source).unwrap();
            let display = page.layout(100, 100, 1.0).unwrap();
            assert_eq!(display.nodes[0].tag, "plot");
            let frame = display.rasterize().unwrap();
            let pixels = frame.pixels.data_as_u8_slice();
            assert!(
                pixels.chunks_exact(4).filter(|p| p[1] < 100).count() > 100,
                "{kind}"
            );
            if kind == "pie" {
                let center = (50 * 100 + 50) * 4;
                assert_eq!(&pixels[center..center + 4], &[255, 255, 255, 255]);
                // Interior samples must remain opaque across tessellation joins.
                for degree in 0..360 {
                    if degree % 120 < 4 || degree % 120 > 116 {
                        continue;
                    }
                    let angle = (degree as f64).to_radians() - std::f64::consts::FRAC_PI_2;
                    let x = (50.0 + angle.cos() * 36.0) as usize;
                    let y = (50.0 + angle.sin() * 36.0) as usize;
                    assert!(pixels[(y * 100 + x) * 4 + 1] < 10, "seam at {degree}");
                }
            }
        }
        let mut invalid = vec![
            serde_json::json!(18),
            serde_json::json!(-1),
            serde_json::json!(""),
            serde_json::json!(""),
            serde_json::json!(""),
            serde_json::json!(3),
        ];
        invalid.extend([
            serde_json::json!(51),
            serde_json::json!("[-1]"),
            serde_json::json!(52),
            serde_json::json!("pie"),
            serde_json::json!(53),
            serde_json::json!("[\"#ffffff\"]"),
        ]);
        assert!(crate::schema::decode(&invalid).is_err());
        let mut many = vec![
            serde_json::json!(0),
            serde_json::json!(-1),
            serde_json::json!(""),
            serde_json::json!(""),
            serde_json::json!(""),
            serde_json::json!(0),
        ];
        for i in 0..64 {
            many.extend([
                serde_json::json!(18),
                serde_json::json!(0),
                serde_json::json!(i.to_string()),
                serde_json::json!(""),
                serde_json::json!(""),
                serde_json::json!(4),
                serde_json::json!(41),
                serde_json::json!("Chart"),
                serde_json::json!(51),
                serde_json::json!("[1,2]"),
                serde_json::json!(52),
                serde_json::json!("pie"),
                serde_json::json!(53),
                serde_json::json!("[\"#ffffff\"]"),
            ]);
        }
        assert!(
            crate::schema::decode(&many)
                .unwrap_err()
                .contains("plot geometry budget")
        );
    }
}
