//! Bounded SVG path geometry; no XML, resources, scripts or network access.
use serde::Deserialize;
use vello_cpu::kurbo::{Affine, BezPath, Cap, Join, Shape, Stroke, StrokeOpts, stroke};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Path {
    d: String,
    fill: Option<String>,
    stroke: Option<String>,
    stroke_width: Option<f64>,
    stroke_dasharray: Option<Vec<f64>>,
    vector_effect: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Source {
    view_box: [f64; 4],
    paths: Vec<Path>,
    fit: Option<String>,
}

pub(crate) struct Vector {
    view_box: [f64; 4],
    pub shapes: Vec<(BezPath, String)>,
    pub work: usize,
    fill_bounds: bool,
    fixed_strokes: std::collections::HashMap<usize, (f64, Vec<f64>)>,
}

fn tint(value: Option<String>, default: &str) -> Result<String, String> {
    let value = value.unwrap_or_else(|| default.into());
    if !matches!(value.as_str(), "none" | "currentColor") {
        if !value.starts_with('#') {
            return Err("UI_VECTOR: expected hex color, currentColor or none".into());
        }
        crate::schema::color(&value)?;
    }
    Ok(value)
}

impl Vector {
    pub fn parse(json: &str) -> Result<Self, String> {
        if json.len() > 32768 {
            return Err("UI_VECTOR: source too large".into());
        }
        let source: Source = serde_json::from_str(json).map_err(|_| "UI_VECTOR: invalid source")?;
        if source.paths.is_empty()
            || source.paths.len() > 32
            || source
                .view_box
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 16384.0)
            || source.view_box[2] < 1.0 / 64.0
            || source.view_box[3] < 1.0 / 64.0
        {
            return Err("UI_VECTOR: invalid paths or viewBox".into());
        }
        if !matches!(source.fit.as_deref(), None | Some("contain" | "fill")) {
            return Err("UI_VECTOR: invalid fit".into());
        }
        let fill_bounds = source.fit.as_deref() == Some("fill");
        let mut fixed_strokes = std::collections::HashMap::new();
        let mut shapes = Vec::new();
        let mut work = 0;
        for source in source.paths {
            if source.d.is_empty() || source.d.len() > 16384 {
                return Err("UI_VECTOR: invalid path length".into());
            }
            bounded_numbers(&source.d)?;
            let path = BezPath::from_svg(&source.d).map_err(|_| "UI_VECTOR: invalid SVG path")?;
            if path.elements().is_empty() || path.elements().len() > 1024 || !path.is_finite() || {
                let b = path.control_box();
                [b.x0, b.y0, b.x1, b.y1].iter().any(|v| v.abs() > 16384.0)
            } {
                return Err("UI_VECTOR: invalid path geometry".into());
            }
            let fill = tint(source.fill, "currentColor")?;
            let line = tint(source.stroke, "none")?;
            let width = source.stroke_width.unwrap_or(1.0);
            if !width.is_finite() || !(0.0..=1024.0).contains(&width) {
                return Err("UI_VECTOR: invalid stroke width".into());
            }
            let dashes = source.stroke_dasharray.unwrap_or_default();
            if dashes.len() > 16
                || dashes
                    .iter()
                    .any(|value| !value.is_finite() || !(1.0 / 64.0..=1024.0).contains(value))
            {
                return Err("UI_VECTOR: invalid dash pattern".into());
            }
            if !matches!(
                source.vector_effect.as_deref(),
                None | Some("none" | "non-scaling-stroke")
            ) {
                return Err("UI_VECTOR: invalid vector effect".into());
            }
            work += path.elements().len();
            if fill != "none" {
                shapes.push((path.clone(), fill));
            }
            if line != "none" && width > 0.0 {
                if source.vector_effect.as_deref() == Some("non-scaling-stroke") {
                    fixed_strokes.insert(shapes.len(), (width, dashes));
                    shapes.push((path.clone(), line));
                } else {
                    let outline = stroke_outline(&path, width, &dashes)?;
                    work += outline.elements().len();
                    shapes.push((outline, line));
                }
            }
            if work > 16384 {
                return Err("UI_VECTOR: geometry budget exceeded".into());
            }
        }
        Ok(Self {
            view_box: source.view_box,
            fill_bounds,
            fixed_strokes,
            shapes,
            work,
        })
    }

    pub fn resolved_shapes(
        &self,
        bounds: [f32; 4],
    ) -> Result<(Vec<(BezPath, String)>, usize), String> {
        let transform = self.transform(bounds);
        let mut work = self.work;
        let mut shapes = Vec::new();
        for (index, (path, tint)) in self.shapes.iter().enumerate() {
            let mut path = transform * path.clone();
            if let Some((width, dashes)) = self.fixed_strokes.get(&index) {
                let bbox = path.control_box();
                if !path.is_finite() || bbox.width() > 32768.0 || bbox.height() > 32768.0 {
                    return Err("UI_VECTOR: transformed stroke exceeds geometry limit".into());
                }
                if [bbox.x0, bbox.y0, bbox.x1, bbox.y1]
                    .iter()
                    .any(|v| v.abs() > 1e12)
                {
                    return Err("UI_VECTOR: transformed stroke exceeds coordinate limit".into());
                }
                path = stroke_outline(&path, *width, dashes)?;
                work += path.elements().len();
                if work > 16384 {
                    return Err("UI_VECTOR: geometry budget exceeded".into());
                }
            }
            shapes.push((path, tint.clone()));
        }
        Ok((shapes, work))
    }

    pub fn transform(&self, bounds: [f32; 4]) -> Affine {
        let [x, y, w, h] = bounds.map(f64::from);
        let [vx, vy, vw, vh] = self.view_box;
        if self.fill_bounds {
            return Affine::translate((x, y))
                * Affine::scale_non_uniform(w / vw, h / vh)
                * Affine::translate((-vx, -vy));
        }
        let scale = (w / vw).min(h / vh);
        Affine::translate((x + (w - vw * scale) / 2.0, y + (h - vh * scale) / 2.0))
            * Affine::scale(scale)
            * Affine::translate((-vx, -vy))
    }
}

fn stroke_outline(path: &BezPath, width: f64, dashes: &[f64]) -> Result<BezPath, String> {
    if !dashes.is_empty() {
        let shortest = dashes.iter().copied().fold(f64::INFINITY, f64::min);
        // Bound subdivision before the stroke library allocates the outline.
        if path.perimeter(0.01) / shortest + path.elements().len() as f64 > 4096.0 {
            return Err("UI_VECTOR: dash geometry budget exceeded".into());
        }
    }
    let outline = stroke(
        path.iter(),
        &Stroke::new(width)
            .with_caps(Cap::Butt)
            .with_join(Join::Miter)
            .with_dashes(0.0, dashes),
        &StrokeOpts::default(),
        0.01,
    );
    if !outline.is_finite() {
        return Err("UI_VECTOR: invalid stroke geometry".into());
    }
    if outline.elements().len() > 16384 {
        return Err("UI_VECTOR: geometry budget exceeded".into());
    }
    Ok(outline)
}

// Bound numbers before arc conversion can expand them into cubic segments.
fn bounded_numbers(source: &str) -> Result<(), String> {
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if b"MmLlHhVvCcSsQqTtAaZz ,\t\r\n".contains(&bytes[i]) {
            i += 1;
            continue;
        }
        let start = i;
        if matches!(bytes[i], b'+' | b'-') {
            i += 1;
        }
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if bytes.get(i) == Some(&b'.') {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
        if matches!(bytes.get(i), Some(b'e' | b'E')) {
            i += 1;
            if matches!(bytes.get(i), Some(b'+' | b'-')) {
                i += 1;
            }
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
        if i == start {
            return Err("UI_VECTOR: invalid path token".into());
        }
        let number: f64 = source[start..i]
            .parse()
            .map_err(|_| "UI_VECTOR: invalid path number")?;
        if !number.is_finite() || number.abs() > 16384.0 {
            return Err("UI_VECTOR: path number exceeds limit".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fixed_stroke_keeps_pixel_width_and_dashes_in_stretched_viewboxes() {
        for width in [64, 128] {
            let source = crate::session::tests::source(&format!(
                "function render(){{return h('vector',{{fit:'fill',viewBox:[0,0,1,1],paths:[{{d:'M0 .5H1',fill:'none',stroke:'#000000',strokeWidth:2,strokeDasharray:[4,2],vectorEffect:'non-scaling-stroke'}}],style:{{width:{width},height:8,background:'#ffffff'}}}})}}"
            ));
            let mut page = crate::ViewSession::new(&source).unwrap();
            let frame = page.layout(width, 8, 1.0).unwrap().rasterize().unwrap();
            let pixels = frame.pixels.data_as_u8_slice();
            for (x, y, expected) in [
                (2, 4, 0),
                (5, 4, 255),
                (8, 4, 0),
                (11, 4, 255),
                (2, 1, 255),
                (2, 6, 255),
            ] {
                let offset = (y * usize::from(width) + x) * 4;
                assert_eq!(
                    &pixels[offset..offset + 4],
                    &[expected, expected, expected, 255]
                );
            }
        }
    }

    #[test]
    fn fixed_stroke_expansion_has_per_vector_and_frame_budgets() {
        let json = json!({"viewBox":[0,0,1,1],"fit":"fill","paths":[{"d":"M0 .5H1","fill":"none","stroke":"#000000","strokeDasharray":[0.015625,0.015625],"vectorEffect":"non-scaling-stroke"}]}).to_string();
        let vector = Vector::parse(&json).unwrap();
        assert!(
            vector
                .resolved_shapes([0.0, 0.0, 10000.0, 1.0])
                .err()
                .unwrap()
                .contains("dash geometry budget")
        );
        let source = crate::session::tests::source(
            "function render(){return h('column',{},Array.from({length:80},(_,i)=>h('vector',{key:String(i),fit:'fill',viewBox:[0,0,1,1],paths:[{d:'M0 .5H1',fill:'none',stroke:'#000000',strokeDasharray:[1,1],vectorEffect:'non-scaling-stroke'}],style:{width:200,height:1}})))}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        assert!(
            page.layout(200, 100, 1.0)
                .err()
                .unwrap()
                .contains("vector geometry budget")
        );
        for field in ["fit", "vectorEffect"] {
            let value = if field == "fit" {
                json!({"fit":"cover","viewBox":[0,0,1,1],"paths":[{"d":"M0 0H1"}]})
            } else {
                json!({"viewBox":[0,0,1,1],"paths":[{"d":"M0 0H1","vectorEffect":"other"}]})
            };
            assert!(Vector::parse(&value.to_string()).is_err());
        }
    }

    #[test]
    fn dashed_vector_raster_keeps_gaps_after_viewbox_scaling() {
        let source = crate::session::tests::source(
            "function render(){return h('vector',{viewBox:[0,0,24,24],paths:[{d:'M0 12H24',fill:'none',stroke:'#000000',strokeWidth:2,strokeDasharray:[4,2]}],style:{width:96,height:96,background:'#ffffff'}})}",
        );
        let mut page = crate::ViewSession::new(&source).unwrap();
        let frame = page.layout(96, 96, 1.0).unwrap().rasterize().unwrap();
        let pixels = frame.pixels.data_as_u8_slice();
        for (x, expected) in [(8, 0), (20, 255), (32, 0), (44, 255)] {
            assert_eq!(
                &pixels[(48 * 96 + x) * 4..(48 * 96 + x) * 4 + 4],
                &[expected, expected, expected, 255]
            );
        }
    }

    #[test]
    fn dashed_strokes_preserve_gaps_and_repeat_odd_patterns() {
        let source = |pattern| {
            json!({"viewBox":[0,0,24,24],"paths":[{
                "d":"M0 12H24", "fill":"none", "stroke":"#000000", "strokeWidth":2,
                "strokeDasharray":pattern
            }]})
            .to_string()
        };
        let vector = Vector::parse(&source(json!([4, 2]))).unwrap();
        let outline = &vector.shapes[0].0;
        for (x, painted) in [(2.0, true), (5.0, false), (8.0, true), (11.0, false)] {
            assert_eq!(outline.contains((x, 12.0).into()), painted);
        }
        assert_eq!(
            Vector::parse(&source(json!([4, 2, 1]))).unwrap().shapes,
            Vector::parse(&source(json!([4, 2, 1, 4, 2, 1])))
                .unwrap()
                .shapes
        );
        assert!(
            Vector::parse(&source(json!([]))).unwrap().shapes[0]
                .0
                .contains((5.0, 12.0).into())
        );
        for pattern in [
            json!([0, 2]),
            json!([-1, 2]),
            json!([0.001, 2]),
            json!([1025]),
            json!("4 2"),
            json!(vec![1; 17]),
        ] {
            assert!(Vector::parse(&source(pattern)).is_err());
        }
        let excessive = json!({"viewBox":[0,0,24,24],"paths":[{
            "d":"M0 12H16384", "fill":"none", "stroke":"#000000", "strokeDasharray":[0.015625,0.015625]
        }]}).to_string();
        assert!(
            Vector::parse(&excessive)
                .err()
                .unwrap()
                .contains("dash geometry budget")
        );
    }

    #[test]
    fn parses_curves_arcs_subpaths_and_preserves_fit() {
        let vector = Vector::parse(&json!({"viewBox":[10,20,24,24],"paths":[
            {"d":"M10 20h24v24H10z","fill":"#123456"},
            {"d":"M10 20C12 20 14 22 16 24Q20 30 24 24A4 4 0 0 1 28 28","fill":"none","stroke":"currentColor","strokeWidth":1.5}
        ]}).to_string()).unwrap();
        assert_eq!(vector.shapes.len(), 2);
        assert!(vector.work > 10);
        let t = vector.transform([0.0, 0.0, 96.0, 48.0]);
        assert_eq!(
            t * vello_cpu::kurbo::Point::new(10.0, 20.0),
            vello_cpu::kurbo::Point::new(24.0, 0.0)
        );
        assert_eq!(
            t * vello_cpu::kurbo::Point::new(34.0, 44.0),
            vello_cpu::kurbo::Point::new(72.0, 48.0)
        );
    }

    #[test]
    fn rejects_invalid_or_unbounded_geometry_before_drawing() {
        for d in [
            "",
            "M",
            "L0 0",
            "M0 0 wat",
            "M0 0L1e309 2",
            "M0 0a1e300 1e300 0 0 1 1 1",
            "M0 0L20000 0",
            "M0 0<script>",
        ] {
            assert!(
                Vector::parse(&json!({"viewBox":[0,0,24,24],"paths":[{"d":d}]}).to_string())
                    .is_err(),
                "{d}"
            );
        }
        for extra in [
            json!({"d":"M0 0h2","strokeWidth":-1}),
            json!({"d":"M0 0h2","fill":"red"}),
            json!({"d":"M0 0h2","filter":"url(x)"}),
        ] {
            assert!(
                Vector::parse(&json!({"viewBox":[0,0,24,24],"paths":[extra]}).to_string()).is_err()
            );
        }
        for view_box in [[0, 0, 0, 24], [0, 0, -1, 24], [0, 0, 20000, 24]] {
            assert!(
                Vector::parse(&json!({"viewBox":view_box,"paths":[{"d":"M0 0h2"}]}).to_string())
                    .is_err()
            );
        }
    }
}
