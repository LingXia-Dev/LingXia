use serde::Deserialize;
use vello_cpu::{
    color::{AlphaColor, ColorSpaceTag, Srgb},
    peniko::Gradient,
};

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LinearGradient {
    #[serde(default, deserialize_with = "present")]
    direction: Option<Direction>,
    #[serde(default, deserialize_with = "present")]
    angle: Option<f64>,
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

impl LinearGradient {
    pub fn parse(source: &str) -> Result<Option<Self>, String> {
        if source == "none" {
            return Ok(None);
        }
        let invalid = || "UI_GRADIENT: invalid linear gradient".to_string();
        if source.len() > 8192 {
            return Err(invalid());
        }
        let value: Self = serde_json::from_str(source).map_err(|_| invalid())?;
        if value.direction.is_some() && value.angle.is_some()
            || value
                .angle
                .is_some_and(|a| !a.is_finite() || a.abs() > 36000.0)
            || !(2..=16).contains(&value.stops.len())
            || value.stops.windows(2).any(|s| s[0].offset > s[1].offset)
            || value.stops.iter().any(|s| {
                !s.offset.is_finite()
                    || s.offset.abs() > 16.0
                    || !matches!(s.color.len(), 7 | 9)
                    || !s.color.starts_with('#')
                    || !s.color[1..].bytes().all(|b| b.is_ascii_hexdigit())
            })
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

    pub fn brush(&self, bounds: [f32; 4]) -> Gradient {
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
        Gradient::new_linear(at(low), at(high))
            .with_interpolation_cs(match self.color_space {
                Space::Srgb => ColorSpaceTag::Srgb,
                Space::Oklab => ColorSpaceTag::Oklab,
            })
            .with_stops(
                self.stops
                    .iter()
                    .map(|s| {
                        let [r, g, b, a] =
                            crate::schema::color(&s.color).expect("validated gradient color");
                        (
                            ((s.offset - low) / (high - low)) as f32,
                            AlphaColor::<Srgb>::from_rgba8(r, g, b, a),
                        )
                    })
                    .collect::<Vec<_>>()
                    .as_slice(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn gradient(direction: &str) -> LinearGradient {
        LinearGradient::parse(&format!(r##"{{{direction},"stops":[{{"offset":0,"color":"#ff0000"}},{{"offset":1,"color":"#0000ff"}}]}}"##)).unwrap().unwrap()
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
            assert!(LinearGradient::parse(source).is_err());
        }
        let valid =
            r##"{"stops":[{"offset":0,"color":"#ff0000"},{"offset":1,"color":"#0000ff"}]}"##;
        for source in [
            valid.replace("\"stops\"", "\"angle\":0,\"direction\":\"top\",\"stops\""),
            valid.replace("#ff0000", "red"),
            valid.replace("\"offset\":0", "\"offset\":2"),
            valid.replace("\"offset\":1", "\"offset\":17"),
        ] {
            assert!(LinearGradient::parse(&source).is_err());
        }
        assert!(LinearGradient::parse("none").unwrap().is_none());
    }
}
