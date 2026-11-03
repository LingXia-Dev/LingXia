//! `mask-image` fades: up to four alpha gradients to a side, intersected over a subtree.
use crate::scene::gradient::GradientBrush;
use serde::Deserialize;
use std::sync::Arc;
use vello_cpu::{
    color::{AlphaColor, Srgb},
    kurbo::Affine,
    peniko::Gradient,
};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Stop {
    #[serde(default)]
    offset: Option<f64>,
    #[serde(default)]
    px: Option<f64>,
    alpha: f64,
}

/// One fade: alpha stops along the box toward `to`, at fractions or pixels from the start.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct MaskLayer {
    to: Side,
    stops: Vec<Stop>,
}

pub(crate) fn parse(source: &str) -> Result<Vec<MaskLayer>, String> {
    let invalid = || "UI_STYLE_VALUE: maskImage".to_string();
    let layers: Vec<MaskLayer> = serde_json::from_str(source).map_err(|_| invalid())?;
    let valid = !layers.is_empty()
        && layers.len() <= 4
        && layers.iter().all(|layer| {
            (2..=16).contains(&layer.stops.len())
                && layer.stops.iter().all(|stop| {
                    (0.0..=1.0).contains(&stop.alpha)
                        && match (stop.offset, stop.px) {
                            (Some(offset), None) => offset.is_finite() && offset.abs() <= 16.0,
                            (None, Some(px)) => px.is_finite() && px.abs() <= 16384.0,
                            _ => false,
                        }
                })
        });
    if valid { Ok(layers) } else { Err(invalid()) }
}

impl MaskLayer {
    /// A black-to-transparent gradient over `bounds`; later stops never precede earlier ones.
    pub(crate) fn brush(&self, [x, y, w, h]: [f32; 4]) -> GradientBrush {
        let [x, y, w, h] = [x, y, w, h].map(f64::from);
        let (start, end, length) = match self.to {
            Side::Bottom => ((x, y), (x, y + h), h),
            Side::Top => ((x, y + h), (x, y), h),
            Side::Right => ((x, y), (x + w, y), w),
            Side::Left => ((x + w, y), (x, y), w),
        };
        let mut offsets: Vec<f64> = self
            .stops
            .iter()
            .map(|stop| match (stop.offset, stop.px) {
                (Some(offset), _) => offset,
                (_, Some(px)) if length > 0.0 => px / length,
                _ => 0.0,
            })
            .collect();
        for i in 1..offsets.len() {
            offsets[i] = offsets[i].max(offsets[i - 1]);
        }
        let low = offsets[0].min(0.0);
        let high = offsets[offsets.len() - 1].max(1.0).max(low + f64::EPSILON);
        let at = |t: f64| {
            (
                start.0 + (end.0 - start.0) * t,
                start.1 + (end.1 - start.1) * t,
            )
        };
        let stops: Vec<(f32, AlphaColor<Srgb>)> = offsets
            .iter()
            .zip(&self.stops)
            .map(|(offset, stop)| {
                (
                    ((offset - low) / (high - low)) as f32,
                    AlphaColor::<Srgb>::new([0.0, 0.0, 0.0, stop.alpha as f32]),
                )
            })
            .collect();
        GradientBrush {
            gradient: Gradient::new_linear(at(low), at(high)).with_stops(stops.as_slice()),
            transform: Affine::IDENTITY,
        }
    }
}

/// A masked subtree's fades, resolved to its box, with the transform they draw under.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Mask {
    pub layers: Vec<GradientBrush>,
    pub bounds: [f32; 4],
    pub transform: Affine,
}

impl Mask {
    pub(crate) fn new(layers: &[MaskLayer], bounds: [f32; 4], transform: Affine) -> Arc<Self> {
        Arc::new(Self {
            layers: layers.iter().map(|layer| layer.brush(bounds)).collect(),
            bounds,
            transform,
        })
    }
}

/// Keeps what is below only as opaque as the fade drawn next.
pub(crate) fn blend() -> vello_cpu::peniko::BlendMode {
    vello_cpu::peniko::BlendMode::new(
        vello_cpu::peniko::Mix::Normal,
        vello_cpu::peniko::Compose::DestIn,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fades_are_bounded_and_pixel_stops_resolve_against_the_box() {
        let layers =
            parse(r#"[{"to":"bottom","stops":[{"px":0,"alpha":0},{"px":16,"alpha":1}]}]"#).unwrap();
        let brush = layers[0].brush([0.0, 10.0, 100.0, 64.0]);
        assert_eq!(brush.gradient.stops.len(), 2);
        assert!((brush.gradient.stops[1].offset - 0.25).abs() < 1e-6);
        for broken in [
            "[]",
            r#"[{"to":"up","stops":[{"offset":0,"alpha":0},{"offset":1,"alpha":1}]}]"#,
            r#"[{"to":"top","stops":[{"offset":0,"alpha":2},{"offset":1,"alpha":1}]}]"#,
            r#"[{"to":"top","stops":[{"offset":0,"px":1,"alpha":0},{"offset":1,"alpha":1}]}]"#,
            r#"[{"to":"top","stops":[{"offset":0,"alpha":0}]}]"#,
        ] {
            assert!(parse(broken).is_err(), "{broken}");
        }
    }
}
