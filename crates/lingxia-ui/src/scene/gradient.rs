use serde::Deserialize;
use vello_cpu::{
    color::{AlphaColor, ColorSpaceTag, Srgb},
    kurbo::Affine,
    peniko::Gradient,
};

/// A gradient in normalized geometry plus the paint transform that places it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GradientBrush {
    pub gradient: Gradient,
    pub transform: Affine,
}

impl GradientBrush {
    pub(crate) fn then(&self, transform: Affine) -> Self {
        Self {
            gradient: self.gradient.clone(),
            transform: transform * self.transform,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Kind {
    #[default]
    Linear,
    Radial,
    Conic,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Shape {
    #[default]
    Ellipse,
    Circle,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Extent {
    ClosestSide,
    ClosestCorner,
    FarthestSide,
    #[default]
    FarthestCorner,
}

/// A CSS `linear-`, `radial-` or `conic-gradient` background.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BackgroundGradient {
    #[serde(default)]
    kind: Kind,
    #[serde(default, deserialize_with = "present")]
    direction: Option<Direction>,
    #[serde(default, deserialize_with = "present")]
    angle: Option<f64>,
    #[serde(default, deserialize_with = "present")]
    shape: Option<Shape>,
    #[serde(default, deserialize_with = "present")]
    size: Option<Extent>,
    /// Center as fractions of the box.
    #[serde(default, deserialize_with = "present")]
    at: Option<[f64; 2]>,
    /// Conic start angle in degrees, clockwise from the top.
    #[serde(default, deserialize_with = "present")]
    from: Option<f64>,
    #[serde(default)]
    color_space: Space,
    stops: Vec<Stop>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum Direction {
    Top,
    Right,
    Bottom,
    Left,
    TopRight,
    BottomRight,
    BottomLeft,
    TopLeft,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Space {
    #[default]
    Srgb,
    Oklab,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Stop {
    offset: f64,
    color: String,
}

fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

fn hex(color: &str) -> bool {
    matches!(color.len(), 7 | 9)
        && color.starts_with('#')
        && color[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

impl BackgroundGradient {
    pub fn parse(source: &str) -> Result<Option<Self>, String> {
        if source == "none" {
            return Ok(None);
        }
        let invalid = || "UI_GRADIENT: invalid gradient".to_string();
        if source.len() > 8192 {
            return Err(invalid());
        }
        let value: Self = serde_json::from_str(source).map_err(|_| invalid())?;
        let angle = |a: Option<f64>| a.is_some_and(|a| !a.is_finite() || a.abs() > 36000.0);
        // Each kind admits only its own geometry keys.
        let foreign = match value.kind {
            Kind::Linear => {
                value.shape.is_some()
                    || value.size.is_some()
                    || value.at.is_some()
                    || value.from.is_some()
                    || value.direction.is_some() && value.angle.is_some()
            }
            Kind::Radial => {
                value.direction.is_some() || value.angle.is_some() || value.from.is_some()
            }
            Kind::Conic => {
                value.direction.is_some()
                    || value.angle.is_some()
                    || value.shape.is_some()
                    || value.size.is_some()
            }
        };
        // Radial and conic stops have no box line to extend beyond.
        let range = if value.kind == Kind::Linear {
            -16.0..=16.0
        } else if value.kind == Kind::Radial {
            0.0..=16.0
        } else {
            0.0..=1.0
        };
        if foreign
            || angle(value.angle)
            || angle(value.from)
            || value
                .at
                .is_some_and(|at| at.iter().any(|v| !v.is_finite() || v.abs() > 16.0))
            || !(2..=16).contains(&value.stops.len())
            || value.stops.windows(2).any(|s| s[0].offset > s[1].offset)
            || value
                .stops
                .iter()
                .any(|s| !s.offset.is_finite() || !range.contains(&s.offset) || !hex(&s.color))
        {
            return Err(invalid());
        }
        Ok(Some(value))
    }

    fn endpoints(&self, [x, y, w, h]: [f32; 4]) -> [(f64, f64); 2] {
        let (x, y, w, h) = (x as f64, y as f64, w as f64, h as f64);
        let (dx, dy) = if let Some(angle) = self.angle {
            let angle = angle.rem_euclid(360.0).to_radians();
            (angle.sin(), -angle.cos())
        } else {
            match self.direction.unwrap_or(Direction::Bottom) {
                Direction::Top => (0.0, -1.0),
                Direction::Right => (1.0, 0.0),
                Direction::Bottom => (0.0, 1.0),
                Direction::Left => (-1.0, 0.0),
                // CSS corner directions are perpendicular to the adjacent-corner diagonal.
                Direction::TopRight => (h, -w),
                Direction::BottomRight => (h, w),
                Direction::BottomLeft => (-h, w),
                Direction::TopLeft => (-h, -w),
            }
        };
        let norm = dx.hypot(dy).max(f64::MIN_POSITIVE);
        let (dx, dy) = (dx / norm, dy / norm);
        let half = (w * dx).abs() / 2.0 + (h * dy).abs() / 2.0;
        let center = (x + w / 2.0, y + h / 2.0);
        [
            (center.0 - dx * half, center.1 - dy * half),
            (center.0 + dx * half, center.1 + dy * half),
        ]
    }

    /// CSS ending-shape radii for a center inside (or outside) the box.
    fn radii(&self, [w, h]: [f64; 2], [cx, cy]: [f64; 2]) -> [f64; 2] {
        let sides = [cx.abs(), (w - cx).abs(), cy.abs(), (h - cy).abs()];
        let [near_x, far_x] = [sides[0].min(sides[1]), sides[0].max(sides[1])];
        let [near_y, far_y] = [sides[2].min(sides[3]), sides[2].max(sides[3])];
        let size = self.size.unwrap_or_default();
        let radii = match (self.shape.unwrap_or_default(), size) {
            (Shape::Circle, Extent::ClosestSide) => [near_x.min(near_y); 2],
            (Shape::Circle, Extent::FarthestSide) => [far_x.max(far_y); 2],
            (Shape::Circle, Extent::ClosestCorner) => [near_x.hypot(near_y); 2],
            (Shape::Circle, Extent::FarthestCorner) => [far_x.hypot(far_y); 2],
            (Shape::Ellipse, Extent::ClosestSide) => [near_x, near_y],
            (Shape::Ellipse, Extent::FarthestSide) => [far_x, far_y],
            // Corner ellipses keep the side ratio and pass through the corner.
            (Shape::Ellipse, Extent::ClosestCorner) => {
                [near_x, near_y].map(|r| r * std::f64::consts::SQRT_2)
            }
            (Shape::Ellipse, Extent::FarthestCorner) => {
                [far_x, far_y].map(|r| r * std::f64::consts::SQRT_2)
            }
        };
        // A degenerate ending shape still paints its last stop, as CSS does.
        radii.map(|r| r.max(1e-3))
    }

    fn stops(&self, low: f64, high: f64) -> Vec<(f32, AlphaColor<Srgb>)> {
        self.stops
            .iter()
            .map(|s| {
                let [r, g, b, a] =
                    crate::protocol::color(&s.color).expect("validated gradient color");
                (
                    ((s.offset - low) / (high - low)) as f32,
                    AlphaColor::<Srgb>::from_rgba8(r, g, b, a),
                )
            })
            .collect()
    }

    pub fn brush(&self, bounds: [f32; 4]) -> GradientBrush {
        let [x, y, w, h] = bounds.map(f64::from);
        let space = match self.color_space {
            Space::Srgb => ColorSpaceTag::Srgb,
            Space::Oklab => ColorSpaceTag::Oklab,
        };
        let [ax, ay] = self.at.unwrap_or([0.5, 0.5]);
        let center = [ax * w, ay * h];
        let (gradient, transform) = match self.kind {
            Kind::Linear => {
                let [start, end] = self.endpoints(bounds);
                // Peniko accepts normalized offsets; extend the line for stops outside the box.
                let low = self.stops[0].offset.min(0.0);
                let high = self.stops[self.stops.len() - 1].offset.max(1.0);
                let at = |t: f64| {
                    (
                        start.0 + (end.0 - start.0) * t,
                        start.1 + (end.1 - start.1) * t,
                    )
                };
                (
                    Gradient::new_linear(at(low), at(high))
                        .with_stops(self.stops(low, high).as_slice()),
                    Affine::IDENTITY,
                )
            }
            Kind::Radial => {
                let [rx, ry] = self.radii([w, h], center);
                let high = self.stops[self.stops.len() - 1].offset.max(1.0);
                (
                    Gradient::new_radial((0.0, 0.0), high as f32)
                        .with_stops(self.stops(0.0, high).as_slice()),
                    Affine::translate((x + center[0], y + center[1]))
                        * Affine::scale_non_uniform(rx, ry),
                )
            }
            Kind::Conic => (
                Gradient::new_sweep((0.0, 0.0), 0.0, std::f32::consts::TAU)
                    .with_stops(self.stops(0.0, 1.0).as_slice()),
                // CSS starts at the top and turns clockwise; peniko starts at +x.
                Affine::translate((x + center[0], y + center[1]))
                    * Affine::rotate((self.from.unwrap_or(0.0) - 90.0).to_radians()),
            ),
        };
        GradientBrush {
            gradient: gradient.with_interpolation_cs(space),
            transform,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn gradient(direction: &str) -> BackgroundGradient {
        BackgroundGradient::parse(&format!(r##"{{{direction},"stops":[{{"offset":0,"color":"#ff0000"}},{{"offset":1,"color":"#0000ff"}}]}}"##)).unwrap().unwrap()
    }
    #[test]
    fn corner_geometry_preserves_css_magic_corners_on_rectangles() {
        let g = gradient(r#""direction":"bottom-right""#);
        let [a, b] = g.endpoints([10.0, 20.0, 200.0, 100.0]);
        assert!((a.0 - 70.0).abs() < 1e-8 && (a.1 + 10.0).abs() < 1e-8);
        assert!((b.0 - 150.0).abs() < 1e-8 && (b.1 - 150.0).abs() < 1e-8);
        let projection = |p: (f64, f64)| {
            ((p.0 - a.0) * (b.0 - a.0) + (p.1 - a.1) * (b.1 - a.1))
                / ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2))
        };
        for corner in [(210.0, 20.0), (10.0, 120.0)] {
            assert!((projection(corner) - 0.5).abs() < 1e-8);
        }
        assert_ne!(
            g.endpoints([0.0, 0.0, 200.0, 100.0]),
            gradient(r#""angle":135"#).endpoints([0.0, 0.0, 200.0, 100.0])
        );
    }
    #[test]
    fn angles_follow_css_clockwise_orientation() {
        for (angle, direction) in [
            (0, "top"),
            (90, "right"),
            (180, "bottom"),
            (270, "left"),
            (-90, "left"),
        ] {
            let a = gradient(&format!(r#""angle":{angle}"#)).endpoints([5.0, 10.0, 200.0, 100.0]);
            let b = gradient(&format!(r#""direction":"{direction}""#))
                .endpoints([5.0, 10.0, 200.0, 100.0]);
            for (a, b) in a.into_iter().zip(b) {
                assert!((a.0 - b.0).abs() < 1e-8 && (a.1 - b.1).abs() < 1e-8);
            }
        }
    }
    #[test]
    fn rejects_unbounded_ambiguous_or_unrecognized_input() {
        for source in [
            "{}",
            "null",
            r#"{"stops":[]}"#,
            r#"{"stops":[],"unknown":1}"#,
        ] {
            assert!(BackgroundGradient::parse(source).is_err());
        }
        let valid =
            r##"{"stops":[{"offset":0,"color":"#ff0000"},{"offset":1,"color":"#0000ff"}]}"##;
        for source in [
            valid.replace("\"stops\"", "\"angle\":0,\"direction\":\"top\",\"stops\""),
            valid.replace("#ff0000", "red"),
            valid.replace("\"offset\":0", "\"offset\":2"),
            valid.replace("\"offset\":1", "\"offset\":17"),
        ] {
            assert!(BackgroundGradient::parse(&source).is_err());
        }
        assert!(BackgroundGradient::parse("none").unwrap().is_none());
    }

    fn pixel(source: &str, size: u16, [x, y]: [usize; 2]) -> [u8; 4] {
        let source = crate::session::tests::source(&format!(
            "function render(){{return h('column',{{style:{{width:{size},height:{size},backgroundImage:{source}}}}})}}"
        ));
        let mut page = crate::ViewSession::new(&source).unwrap();
        let frame = page.layout(size, size, 1.0).unwrap().rasterize().unwrap();
        let at = (y * usize::from(size) + x) * 4;
        frame.pixels.data_as_u8_slice()[at..at + 4]
            .try_into()
            .unwrap()
    }

    fn near(actual: [u8; 4], expected: [u8; 3]) -> bool {
        actual
            .iter()
            .zip(expected)
            .all(|(a, e)| a.abs_diff(e) <= 24)
    }

    #[test]
    fn radial_gradients_follow_css_ending_shapes_and_centers() {
        let stops = r##"stops:[{offset:0,color:'#ff0000'},{offset:1,color:'#0000ff'}]"##;
        let ellipse = format!("{{kind:'radial',{stops}}}");
        assert!(near(pixel(&ellipse, 64, [32, 32]), [255, 0, 0]));
        // Farthest-corner ellipses pass through the corners.
        assert!(near(pixel(&ellipse, 64, [1, 1]), [0, 0, 255]));
        let closest =
            format!("{{kind:'radial',shape:'circle',size:'closest-side',at:[0.25,0.25],{stops}}}");
        assert!(near(pixel(&closest, 64, [16, 16]), [255, 0, 0]));
        // The closest side is 16px away, so 20px out is past the last stop.
        assert!(near(pixel(&closest, 64, [36, 16]), [0, 0, 255]));
        let corner = format!("{{kind:'radial',size:'closest-side',at:[0,0],{stops}}}");
        assert!(near(pixel(&corner, 64, [20, 20]), [0, 0, 255]));
        let circle = format!("{{kind:'radial',shape:'circle',size:'farthest-side',{stops}}}");
        let edge = pixel(&circle, 64, [63, 32]);
        assert!(near(edge, [0, 0, 255]), "{edge:?}");
    }

    #[test]
    fn conic_gradients_start_at_the_top_and_turn_clockwise() {
        let stops = r##"stops:[{offset:0,color:'#ff0000'},{offset:0.25,color:'#ff0000'},{offset:0.25,color:'#00ff00'},{offset:0.5,color:'#00ff00'},{offset:0.5,color:'#0000ff'},{offset:1,color:'#0000ff'}]"##;
        let conic = format!("{{kind:'conic',{stops}}}");
        // Top-right quadrant is the first quarter turn, bottom-right the second.
        assert!(near(pixel(&conic, 64, [48, 16]), [255, 0, 0]));
        assert!(near(pixel(&conic, 64, [48, 48]), [0, 255, 0]));
        assert!(near(pixel(&conic, 64, [16, 48]), [0, 0, 255]));
        let rotated = format!("{{kind:'conic',from:90,{stops}}}");
        assert!(near(pixel(&rotated, 64, [48, 48]), [255, 0, 0]));
    }

    #[test]
    fn each_kind_admits_only_its_geometry() {
        let stops = r##""stops":[{"offset":0,"color":"#ff0000"},{"offset":1,"color":"#0000ff"}]"##;
        for source in [
            format!(r#"{{"kind":"radial","angle":10,{stops}}}"#),
            format!(r#"{{"kind":"conic","shape":"circle",{stops}}}"#),
            format!(r#"{{"at":[0.5,0.5],{stops}}}"#),
            format!(r#"{{"kind":"radial","at":[17,0],{stops}}}"#),
            format!(r#"{{"kind":"spiral",{stops}}}"#),
            r##"{"kind":"conic","stops":[{"offset":0,"color":"#ff0000"},{"offset":2,"color":"#0000ff"}]}"##.into(),
            r##"{"kind":"radial","stops":[{"offset":-1,"color":"#ff0000"},{"offset":1,"color":"#0000ff"}]}"##.into(),
        ] {
            assert!(BackgroundGradient::parse(&source).is_err(), "{source}");
        }
        for source in [
            format!(
                r#"{{"kind":"radial","shape":"circle","size":"closest-corner","at":[0,1],{stops}}}"#
            ),
            format!(r#"{{"kind":"conic","from":-45,"at":[0.25,0.5],{stops}}}"#),
        ] {
            assert!(BackgroundGradient::parse(&source).unwrap().is_some());
        }
    }
}
