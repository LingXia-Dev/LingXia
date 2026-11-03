//! Paint commands and their transform helpers.
use crate::scene::node::*;
use std::sync::Arc;
use vello_cpu::Glyph;
use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::kurbo::Affine;

pub(crate) fn check_path(points: &[[f32; 2]]) -> vello_cpu::kurbo::BezPath {
    let mut path = vello_cpu::kurbo::BezPath::new();
    path.move_to((points[0][0] as f64, points[0][1] as f64));
    for point in &points[1..] {
        path.line_to((point[0] as f64, point[1] as f64));
    }
    path
}

pub(crate) fn check_stroke(width: f32, round: bool) -> vello_cpu::kurbo::Stroke {
    vello_cpu::kurbo::Stroke::new(width as f64)
        .with_caps(if round {
            vello_cpu::kurbo::Cap::Round
        } else {
            vello_cpu::kurbo::Cap::Butt
        })
        .with_join(vello_cpu::kurbo::Join::Round)
}

pub(crate) fn plot_bar_path(bounds: [f32; 4]) -> vello_cpu::kurbo::BezPath {
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

pub(crate) fn area_path(points: &[[f32; 2]]) -> vello_cpu::kurbo::BezPath {
    let mut path = check_path(points);
    path.close_path();
    path
}

pub(crate) fn area_gradient(
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

pub(crate) fn checked_paint_transform(transform: Affine) -> Result<Affine, String> {
    if transform
        .as_coeffs()
        .iter()
        .any(|n| !n.is_finite() || n.abs() > 1e12)
        || transform.determinant().abs() < 1e-12
    {
        return Err("UI_TRANSFORM: unsupported or singular paint transform".into());
    }
    Ok(transform)
}

/// Positive scales and translations keep boxes axis-aligned.
pub(crate) fn axis_aligned(transform: Affine) -> bool {
    let [sx, b, c, sy, _, _] = transform.as_coeffs();
    sx > 0.0 && sy > 0.0 && b == 0.0 && c == 0.0
}

pub(crate) fn transformed_clip(
    mut clip: ClipSnapshot,
    transform: Affine,
) -> Result<ClipSnapshot, String> {
    if transform == Affine::IDENTITY {
        return Ok(clip);
    }
    let transform = checked_paint_transform(transform)?;
    if clip.transform.is_some() || !axis_aligned(transform) {
        let combined = transform * clip.affine().unwrap_or(Affine::IDENTITY);
        clip.transform = (combined != Affine::IDENTITY).then(|| combined.as_coeffs());
        if clip
            .extent()
            .iter()
            .any(|n| !n.is_finite() || n.abs() > 1e12)
        {
            return Err("UI_TRANSFORM: transformed clip outside bounds".into());
        }
        return Ok(clip);
    }
    let [sx, _, _, sy, tx, ty] = transform.as_coeffs();
    let [x, y, w, h] = clip.bounds.map(f64::from);
    let bounds = [x * sx + tx, y * sy + ty, w * sx, h * sy];
    if bounds.iter().any(|n| !n.is_finite() || n.abs() > 1e12) {
        return Err("UI_TRANSFORM: transformed clip outside bounds".into());
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

/// The axis-aligned extent of `bounds` under `transform`.
pub(crate) fn transformed_bounds(bounds: [f32; 4], transform: Affine) -> Result<[f32; 4], String> {
    Ok(transformed_clip(
        ClipSnapshot {
            bounds,
            radius: 0.0,
            radii: None,
            transform: None,
        },
        transform,
    )?
    .extent())
}

#[derive(Clone, PartialEq)]
pub(crate) enum Paint {
    // Absolute logical-space state, retained across composition span boundaries.
    Transform(Affine),
    RasterShadow {
        raster: crate::scene::shadow::RasterShadow,
    },
    Vector {
        path: Arc<vello_cpu::kurbo::BezPath>,
        color: [u8; 4],
        /// Replaces `color` when present.
        gradient: Option<Arc<crate::scene::gradient::GradientBrush>>,
        bounds: [f32; 4],
    },
    Shadow {
        shadow: crate::scene::shadow::BoxShadow,
        color: [u8; 4],
        bounds: [f32; 4],
        radius: f32,
    },
    Gradient {
        gradient: Arc<crate::scene::gradient::BackgroundGradient>,
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
        style: crate::scene::border::LineStyle,
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
        fit: crate::scene::image::Fit,
        /// Box fractions the image aligns to, as CSS `object-position`.
        position: [f32; 2],
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
    /// `mask-image`: the subtree draws into its own layer, faded on the way out.
    Mask {
        node: usize,
        mask: Arc<crate::scene::mask::Mask>,
    },
    PopMask(Arc<crate::scene::mask::Mask>),
    /// `blur-*`: the subtree draws into a layer blurred by `radius` (a standard deviation) in
    /// logical pixels.
    Blur {
        node: usize,
        radius: f32,
    },
    PopBlur,
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
        /// `bg-clip-text`: the glyphs show this gradient instead of `color`.
        gradient: Option<Arc<crate::scene::gradient::GradientBrush>>,
    },
}
