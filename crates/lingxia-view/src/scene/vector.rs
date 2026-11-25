//! Bounded SVG path geometry; no XML, resources, scripts or network access.
use crate::scene::gradient::GradientBrush;
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};
use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::kurbo::{Affine, BezPath, Cap, Join, Shape, Stroke, StrokeOpts, stroke};
use vello_cpu::peniko::{Extend, Gradient};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Path {
    key: Option<String>,
    d: String,
    fill: Option<String>,
    stroke: Option<String>,
    stroke_width: Option<f64>,
    stroke_dasharray: Option<Vec<f64>>,
    stroke_dashoffset: Option<f64>,
    stroke_linecap: Option<String>,
    stroke_linejoin: Option<String>,
    transform: Option<String>,
    vector_effect: Option<String>,
    path_length: Option<f64>,
}

#[derive(Deserialize)]
struct GradientStop {
    offset: f64,
    color: String,
    opacity: Option<f64>,
}

#[derive(Deserialize)]
struct VectorGradient {
    r#type: String,
    x1: Option<f64>,
    y1: Option<f64>,
    x2: Option<f64>,
    y2: Option<f64>,
    cx: Option<f64>,
    cy: Option<f64>,
    r: Option<f64>,
    fx: Option<f64>,
    fy: Option<f64>,
    fr: Option<f64>,
    /// Conic start angle in degrees, clockwise from the top.
    from: Option<f64>,
    units: Option<String>,
    spread: Option<String>,
    stops: Vec<GradientStop>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Source {
    view_box: [f64; 4],
    paths: Vec<Path>,
    #[serde(default)]
    gradients: BTreeMap<String, VectorGradient>,
    fit: Option<String>,
}

/// What fills one shape: a hex color, `currentColor`, or a gradient in user space.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tint {
    Color(String),
    Gradient(Arc<GradientBrush>),
}

pub(crate) struct Vector {
    view_box: [f64; 4],
    pub shapes: Vec<(BezPath, Tint)>,
    /// Per shape, a stroke outlined only after the viewBox scale, keeping its pixel width.
    fixed: Vec<Option<Fixed>>,
    pub work: usize,
    fill_bounds: bool,
    /// Each source path's own geometry and stroke: what a vector's transition moves.
    base: PathValues,
    /// How each source path becomes shapes, in `shapes` order.
    recipes: Vec<Recipe>,
}

/// A non-scaling stroke and the `pathLength` its dashes are measured against.
type Fixed = (Stroke, Option<f64>);

/// A shape in viewBox space, with the stroke still to outline after the viewBox scale.
type Outline = (BezPath, Option<Fixed>);

/// A shape on screen, with the solid colour it paints in when it has one.
type Painted = (BezPath, Option<[u8; 4]>);

struct Recipe {
    transform: Affine,
    fill: bool,
    /// Caps and join of the stroke, and whether it keeps its pixel width.
    line: Option<(Cap, Join, bool)>,
    /// `pathLength`: what the path's length counts as when measuring dashes.
    length: Option<f64>,
}

/// One source path's stroke, which a transition moves with its geometry.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PathStroke {
    width: f64,
    dashes: Vec<f64>,
    offset: f64,
    /// Solid fill and stroke colours; `None` for none or a gradient, which do not interpolate.
    fill: Option<[u8; 4]>,
    stroke: Option<[u8; 4]>,
}

/// A vector's source paths, keyed for pairing with another source's.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PathValues {
    keys: Vec<Option<String>>,
    paths: Vec<BezPath>,
    strokes: Vec<PathStroke>,
}

impl PathValues {
    /// `self` reordered to pair with `to`, path by path, when every pair has the same
    /// commands: by key when both sides key every path alike, otherwise by position. A dash
    /// pattern of another length, and a colour either side lacks, take `to`'s at once.
    pub(crate) fn aligned(&self, to: &Self) -> Option<Self> {
        let keyed = |values: &Self| values.keys.iter().all(Option::is_some);
        let order = if keyed(self) && keyed(to) && self.keys.len() == to.keys.len() {
            to.keys
                .iter()
                .map(|key| self.keys.iter().position(|own| own == key))
                .collect::<Option<Vec<_>>>()?
        } else if self.paths.len() == to.paths.len() {
            (0..self.paths.len()).collect()
        } else {
            return None;
        };
        let same = |a: &BezPath, b: &BezPath| {
            a.elements().len() == b.elements().len()
                && a.elements()
                    .iter()
                    .zip(b.elements())
                    .all(|(a, b)| std::mem::discriminant(a) == std::mem::discriminant(b))
        };
        if !order
            .iter()
            .zip(&to.paths)
            .all(|(&at, b)| same(&self.paths[at], b))
        {
            return None;
        }
        let strokes = order
            .iter()
            .zip(&to.strokes)
            .map(|(&at, end)| {
                let mut start = self.strokes[at].clone();
                if start.dashes.len() != end.dashes.len() {
                    start.dashes.clone_from(&end.dashes);
                }
                if start.fill.is_none() || end.fill.is_none() {
                    start.fill = end.fill;
                }
                if start.stroke.is_none() || end.stroke.is_none() {
                    start.stroke = end.stroke;
                }
                start
            })
            .collect();
        Some(Self {
            keys: to.keys.clone(),
            paths: order.iter().map(|&at| self.paths[at].clone()).collect(),
            strokes,
        })
    }

    /// Point-wise and value-wise mix of two aligned values.
    pub(crate) fn mix(&self, to: &Self, t: f64) -> Self {
        use vello_cpu::kurbo::PathEl::{ClosePath, CurveTo, LineTo, MoveTo, QuadTo};
        let lerp = |a: f64, b: f64| a + (b - a) * t;
        let color = |a: Option<[u8; 4]>, b: Option<[u8; 4]>| match (a, b) {
            (Some(a), Some(b)) => Some(crate::animation::transition::mix_oklab(a, b, t)),
            (_, b) => b,
        };
        let paths = self
            .paths
            .iter()
            .zip(&to.paths)
            .map(|(a, b)| {
                let mixed = a
                    .elements()
                    .iter()
                    .zip(b.elements())
                    .map(|pair| match pair {
                        (MoveTo(a), MoveTo(b)) => MoveTo(a.lerp(*b, t)),
                        (LineTo(a), LineTo(b)) => LineTo(a.lerp(*b, t)),
                        (QuadTo(a1, a2), QuadTo(b1, b2)) => {
                            QuadTo(a1.lerp(*b1, t), a2.lerp(*b2, t))
                        }
                        (CurveTo(a1, a2, a3), CurveTo(b1, b2, b3)) => {
                            CurveTo(a1.lerp(*b1, t), a2.lerp(*b2, t), a3.lerp(*b3, t))
                        }
                        (ClosePath, _) => ClosePath,
                        (_, other) => *other,
                    });
                BezPath::from_vec(mixed.collect())
            })
            .collect();
        let strokes = self
            .strokes
            .iter()
            .zip(&to.strokes)
            .map(|(a, b)| PathStroke {
                width: lerp(a.width, b.width),
                dashes: a
                    .dashes
                    .iter()
                    .zip(&b.dashes)
                    .map(|(a, b)| lerp(*a, *b))
                    .collect(),
                offset: lerp(a.offset, b.offset),
                fill: color(a.fill, b.fill),
                stroke: color(a.stroke, b.stroke),
            })
            .collect();
        Self {
            keys: to.keys.clone(),
            paths,
            strokes,
        }
    }
}

fn tint<'a>(
    value: Option<String>,
    default: &str,
    gradients: &'a BTreeMap<String, VectorGradient>,
) -> Result<Option<Paint<'a>>, String> {
    let value = value.unwrap_or_else(|| default.into());
    if value == "none" {
        return Ok(None);
    }
    if let Some(id) = value
        .strip_prefix("url(#")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return gradients
            .get(id)
            .map(|gradient| Some(Paint::Gradient(gradient)))
            .ok_or_else(|| format!("UI_VECTOR: unknown gradient {id}"));
    }
    if value != "currentColor" {
        crate::theme::parse(&value).map_err(|_| {
            "UI_VECTOR: expected a hex color, role, currentColor, url(#id) or none".to_string()
        })?;
    }
    Ok(Some(Paint::Color(value)))
}

enum Paint<'a> {
    Color(String),
    Gradient(&'a VectorGradient),
}

fn bounded(value: f64, limit: f64) -> bool {
    value.is_finite() && value.abs() <= limit
}

impl VectorGradient {
    fn check(&self, id: &str) -> Result<(), String> {
        let invalid = || format!("UI_VECTOR: invalid gradient {id}");
        let linear = self.r#type == "linear";
        let coordinates = [
            self.x1, self.y1, self.x2, self.y2, self.cx, self.cy, self.fx, self.fy,
        ];
        if id.is_empty()
            || id.len() > 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || !matches!(self.r#type.as_str(), "linear" | "radial" | "conic")
            || (linear
                && [
                    self.cx, self.cy, self.r, self.fx, self.fy, self.fr, self.from,
                ]
                .iter()
                .any(Option::is_some))
            || (self.r#type == "conic"
                && [self.r, self.fx, self.fy, self.fr]
                    .iter()
                    .any(Option::is_some))
            || (self.r#type == "radial" && self.from.is_some())
            || self.from.is_some_and(|a| !bounded(a, 36000.0))
            || (!linear
                && [self.x1, self.y1, self.x2, self.y2]
                    .iter()
                    .any(Option::is_some))
            || coordinates.iter().flatten().any(|v| !bounded(*v, 16384.0))
            || [self.r, self.fr]
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || !(0.0..=16384.0).contains(v))
            || !matches!(
                self.units.as_deref(),
                None | Some("objectBoundingBox" | "userSpaceOnUse")
            )
            || !matches!(
                self.spread.as_deref(),
                None | Some("pad" | "reflect" | "repeat")
            )
            || !(1..=16).contains(&self.stops.len())
            || self.stops.iter().any(|stop| {
                !(0.0..=1.0).contains(&stop.offset)
                    || stop
                        .opacity
                        .is_some_and(|o| !o.is_finite() || !(0.0..=1.0).contains(&o))
                    || crate::theme::parse(&stop.color).is_err()
            })
        {
            return Err(invalid());
        }
        Ok(())
    }

    /// The brush in the path's user space; `bounds` is the geometry box SVG
    /// maps `objectBoundingBox` units onto.
    fn brush(
        &self,
        bounds: vello_cpu::kurbo::Rect,
        transform: Affine,
        palette: &crate::theme::Palette,
    ) -> GradientBrush {
        // SVG clamps each stop to at least the previous offset.
        let mut previous = 0.0f64;
        let stops: Vec<(f32, AlphaColor<Srgb>)> = self
            .stops
            .iter()
            .map(|stop| {
                previous = previous.max(stop.offset);
                let [r, g, b, a] = palette.resolve(&stop.color).expect("checked stop");
                let alpha = (f64::from(a) * stop.opacity.unwrap_or(1.0)).round() as u8;
                (
                    previous as f32,
                    AlphaColor::<Srgb>::from_rgba8(r, g, b, alpha),
                )
            })
            .collect();
        let stops = if stops.len() == 1 {
            vec![stops[0], (1.0, stops[0].1)]
        } else {
            stops
        };
        let center = (self.cx.unwrap_or(0.5), self.cy.unwrap_or(0.5));
        // A conic sweep starts at the top and turns clockwise, like CSS; peniko starts at +x.
        let sweep = (self.r#type == "conic").then(|| {
            Affine::translate(center)
                * Affine::rotate((self.from.unwrap_or(0.0) - 90.0).to_radians())
                * Affine::translate((-center.0, -center.1))
        });
        let gradient = if sweep.is_some() {
            Gradient::new_sweep(center, 0.0, std::f32::consts::TAU)
        } else if self.r#type == "linear" {
            Gradient::new_linear(
                (self.x1.unwrap_or(0.0), self.y1.unwrap_or(0.0)),
                (self.x2.unwrap_or(1.0), self.y2.unwrap_or(0.0)),
            )
        } else {
            Gradient::new_two_point_radial(
                (self.fx.unwrap_or(center.0), self.fy.unwrap_or(center.1)),
                self.fr.unwrap_or(0.0) as f32,
                center,
                self.r.unwrap_or(0.5) as f32,
            )
        }
        .with_extend(match self.spread.as_deref() {
            Some("reflect") => Extend::Reflect,
            Some("repeat") => Extend::Repeat,
            _ => Extend::Pad,
        })
        .with_stops(stops.as_slice());
        let units = if self.units.as_deref() == Some("userSpaceOnUse") {
            Affine::IDENTITY
        } else {
            Affine::translate((bounds.x0, bounds.y0))
                * Affine::scale_non_uniform(bounds.width().max(1e-9), bounds.height().max(1e-9))
        };
        GradientBrush {
            gradient,
            transform: transform * units * sweep.unwrap_or(Affine::IDENTITY),
        }
    }
}

impl Vector {
    /// Validates with the built-in palette; painting resolves roles with [`Self::parse_with`].
    pub fn parse(json: &str) -> Result<Self, String> {
        Self::parse_with(json, &crate::theme::Palette::builtin(false))
    }

    /// Fills, strokes and gradient stops may be hex or roles; roles resolve against `palette`.
    pub fn parse_with(json: &str, palette: &crate::theme::Palette) -> Result<Self, String> {
        if json.len() > crate::protocol::limits::VECTOR_SOURCE_BYTES {
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
        if source.gradients.len() > 8 {
            return Err("UI_VECTOR: at most 8 gradients".into());
        }
        for (id, gradient) in &source.gradients {
            gradient.check(id)?;
        }
        if !matches!(source.fit.as_deref(), None | Some("contain" | "fill")) {
            return Err("UI_VECTOR: invalid fit".into());
        }
        let fill_bounds = source.fit.as_deref() == Some("fill");
        let mut tints = Vec::new();
        let mut strokes = Vec::new();
        let mut recipes = Vec::new();
        let mut keys = Vec::new();
        let mut locals = Vec::new();
        for path_source in &source.paths {
            if path_source.key.as_ref().is_some_and(|key| {
                key.is_empty() || key.len() > 64 || keys.contains(&Some(key.clone()))
            }) {
                return Err("UI_VECTOR: path keys are unique and 1..64 characters".into());
            }
            if path_source.d.is_empty()
                || path_source.d.len() > crate::protocol::limits::VECTOR_PATH_BYTES
            {
                return Err("UI_VECTOR: invalid path length".into());
            }
            bounded_numbers(&path_source.d)?;
            let local =
                BezPath::from_svg(&path_source.d).map_err(|_| "UI_VECTOR: invalid SVG path")?;
            let segments = local.elements().len();
            if segments > crate::protocol::limits::VECTOR_PATH_SEGMENTS {
                return Err(format!(
                    "UI_VECTOR: a path has {segments} segments; the limit is {} per path: split it into several paths",
                    crate::protocol::limits::VECTOR_PATH_SEGMENTS
                ));
            }
            if segments == 0 || !local.is_finite() || out_of_bounds(&local) {
                return Err("UI_VECTOR: invalid path geometry".into());
            }
            let transform = path_source
                .transform
                .as_deref()
                .map(parse_transform)
                .transpose()?
                .unwrap_or(Affine::IDENTITY);
            let path = transform * local.clone();
            if !path.is_finite() || out_of_bounds(&path) {
                return Err("UI_VECTOR: transformed path exceeds geometry limit".into());
            }
            let fill = tint(path_source.fill.clone(), "currentColor", &source.gradients)?;
            let line = tint(path_source.stroke.clone(), "none", &source.gradients)?;
            let width = path_source.stroke_width.unwrap_or(1.0);
            if !width.is_finite() || !(0.0..=1024.0).contains(&width) {
                return Err("UI_VECTOR: invalid stroke width".into());
            }
            let dashes = path_source.stroke_dasharray.clone().unwrap_or_default();
            if dashes.len() > 16
                || dashes
                    .iter()
                    .any(|value| !value.is_finite() || !(1.0 / 64.0..=1024.0).contains(value))
            {
                return Err("UI_VECTOR: invalid dash pattern".into());
            }
            let offset = path_source.stroke_dashoffset.unwrap_or(0.0);
            if !bounded(offset, 16384.0) {
                return Err("UI_VECTOR: invalid dash offset".into());
            }
            let cap = match path_source.stroke_linecap.as_deref() {
                None | Some("butt") => Cap::Butt,
                Some("round") => Cap::Round,
                Some("square") => Cap::Square,
                _ => return Err("UI_VECTOR: invalid stroke-linecap".into()),
            };
            let join = match path_source.stroke_linejoin.as_deref() {
                None | Some("miter") => Join::Miter,
                Some("round") => Join::Round,
                Some("bevel") => Join::Bevel,
                _ => return Err("UI_VECTOR: invalid stroke-linejoin".into()),
            };
            if !matches!(
                path_source.vector_effect.as_deref(),
                None | Some("none" | "non-scaling-stroke")
            ) {
                return Err("UI_VECTOR: invalid vector effect".into());
            }
            let resolve = |paint: Paint| match paint {
                Paint::Color(color) => Tint::Color(color),
                Paint::Gradient(gradient) => Tint::Gradient(Arc::new(gradient.brush(
                    local.bounding_box(),
                    transform,
                    palette,
                ))),
            };
            if path_source
                .path_length
                .is_some_and(|length| !length.is_finite() || length <= 0.0 || length > 16384.0)
            {
                return Err("UI_VECTOR: invalid pathLength".into());
            }
            let line = line.filter(|_| width > 0.0);
            let fixed = path_source.vector_effect.as_deref() == Some("non-scaling-stroke");
            recipes.push(Recipe {
                transform,
                fill: fill.is_some(),
                line: line.is_some().then_some((cap, join, fixed)),
                length: path_source.path_length,
            });
            tints.extend(fill.into_iter().chain(line).map(resolve));
            keys.push(path_source.key.clone());
            locals.push(local);
            strokes.push(PathStroke {
                width,
                dashes,
                offset,
                fill: None,
                stroke: None,
            });
        }
        let mut vector = Self {
            view_box: source.view_box,
            fill_bounds,
            shapes: Vec::new(),
            fixed: Vec::new(),
            work: 0,
            base: PathValues {
                keys,
                paths: locals,
                strokes,
            },
            recipes,
        };
        let (shapes, work) = vector.shape_paths(&vector.base)?;
        (vector.shapes, vector.fixed) = shapes
            .into_iter()
            .zip(tints)
            .map(|((path, fixed), tint)| ((path, tint), fixed))
            .unzip();
        vector.work = work;
        Ok(vector)
    }

    /// The source paths and strokes, with solid colours as `color` resolves each shape's tint.
    pub fn values(&self, color: impl Fn(&Tint) -> Option<[u8; 4]>) -> PathValues {
        let mut values = self.base.clone();
        let mut shape = 0;
        for (recipe, stroke) in self.recipes.iter().zip(&mut values.strokes) {
            if recipe.fill {
                stroke.fill = color(&self.shapes[shape].1);
                shape += 1;
            }
            if recipe.line.is_some() {
                stroke.stroke = color(&self.shapes[shape].1);
                shape += 1;
            }
        }
        values
    }

    /// The shapes `values` make under this vector's recipes, in viewBox space, each with the
    /// stroke still to outline after the viewBox scale; and their work.
    fn shape_paths(&self, values: &PathValues) -> Result<(Vec<Outline>, usize), String> {
        let mut shapes = Vec::new();
        let mut work = 0;
        for ((recipe, local), stroke) in self.recipes.iter().zip(&values.paths).zip(&values.strokes)
        {
            let path = recipe.transform * local.clone();
            work += path.elements().len();
            if recipe.fill {
                shapes.push((path.clone(), None));
            }
            if let Some((cap, join, fixed)) = recipe.line {
                let style = Stroke::new(stroke.width)
                    .with_caps(cap)
                    .with_join(join)
                    .with_dashes(stroke.offset, stroke.dashes.iter().copied());
                if fixed {
                    shapes.push((path, Some((style, recipe.length))));
                } else {
                    // Strokes scale with the path transform, as in SVG.
                    let outline = recipe.transform
                        * stroke_outline(local, &measured(style, local, recipe.length))?;
                    work += outline.elements().len();
                    shapes.push((outline, None));
                }
            }
            if work > crate::protocol::limits::VECTOR_PATH_WORK {
                return Err("UI_VECTOR: geometry budget exceeded".into());
            }
        }
        Ok((shapes, work))
    }

    pub fn resolved_shapes(
        &self,
        bounds: [f32; 4],
    ) -> Result<(Vec<(BezPath, Tint)>, usize), String> {
        let transform = self.transform(bounds);
        let shapes = self
            .shapes
            .iter()
            .zip(&self.fixed)
            .map(|((path, _), fixed)| (path.clone(), fixed.clone()))
            .collect();
        let (paths, work) = self.resolved_paths(shapes, self.work, bounds)?;
        let shapes = paths
            .into_iter()
            .zip(&self.shapes)
            .map(|(path, (_, tint))| {
                let tint = match tint {
                    Tint::Gradient(brush) => Tint::Gradient(Arc::new(brush.then(transform))),
                    color => color.clone(),
                };
                (path, tint)
            })
            .collect();
        Ok((shapes, work))
    }

    /// The screen paths of `values`, a transition's mix of this vector's own, in `shapes`
    /// order, each with the solid colour it paints in when it has one.
    pub fn resolved_morph(
        &self,
        values: &PathValues,
        bounds: [f32; 4],
    ) -> Result<Vec<Painted>, String> {
        let (shapes, work) = self.shape_paths(values)?;
        let paths = self.resolved_paths(shapes, work, bounds)?.0;
        let mut colors = Vec::with_capacity(paths.len());
        for (recipe, stroke) in self.recipes.iter().zip(&values.strokes) {
            if recipe.fill {
                colors.push(stroke.fill);
            }
            if recipe.line.is_some() {
                colors.push(stroke.stroke);
            }
        }
        Ok(paths.into_iter().zip(colors).collect())
    }

    fn resolved_paths(
        &self,
        shapes: Vec<Outline>,
        mut work: usize,
        bounds: [f32; 4],
    ) -> Result<(Vec<BezPath>, usize), String> {
        let transform = self.transform(bounds);
        let mut resolved = Vec::with_capacity(shapes.len());
        for (path, fixed) in shapes {
            let mut path = transform * path;
            if let Some((style, length)) = fixed {
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
                path = stroke_outline(&path, &measured(style, &path, length))?;
                work += path.elements().len();
                if work > crate::protocol::limits::VECTOR_PATH_WORK {
                    return Err("UI_VECTOR: geometry budget exceeded".into());
                }
            }
            resolved.push(path);
        }
        Ok((resolved, work))
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

fn out_of_bounds(path: &BezPath) -> bool {
    let b = path.control_box();
    [b.x0, b.y0, b.x1, b.y1].iter().any(|v| v.abs() > 16384.0)
}

/// An SVG transform list: `matrix`, `translate`, `scale`, `rotate`, `skewX`, `skewY`.
fn parse_transform(source: &str) -> Result<Affine, String> {
    let invalid = || "UI_VECTOR: invalid transform".to_string();
    if source.len() > 512 {
        return Err(invalid());
    }
    let mut result = Affine::IDENTITY;
    let mut rest = source.trim_start_matches([' ', ',', '\t', '\n', '\r']);
    let mut count = 0;
    while !rest.is_empty() {
        count += 1;
        if count > 16 {
            return Err(invalid());
        }
        let open = rest.find('(').ok_or_else(invalid)?;
        let close = rest.find(')').ok_or_else(invalid)?;
        if close < open {
            return Err(invalid());
        }
        let name = rest[..open].trim();
        let args = rest[open + 1..close]
            .split([' ', ',', '\t', '\n', '\r'])
            .filter(|part| !part.is_empty())
            .map(|part| part.parse::<f64>().ok().filter(|v| bounded(*v, 16384.0)))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(invalid)?;
        let step = match (name, args.as_slice()) {
            ("matrix", [a, b, c, d, e, f]) => Affine::new([*a, *b, *c, *d, *e, *f]),
            ("translate", [x]) => Affine::translate((*x, 0.0)),
            ("translate", [x, y]) => Affine::translate((*x, *y)),
            ("scale", [s]) => Affine::scale(*s),
            ("scale", [x, y]) => Affine::scale_non_uniform(*x, *y),
            ("rotate", [a]) => Affine::rotate(a.to_radians()),
            ("rotate", [a, x, y]) => {
                Affine::rotate_about(a.to_radians(), vello_cpu::kurbo::Point::new(*x, *y))
            }
            ("skewX", [a]) => Affine::skew(a.to_radians().tan(), 0.0),
            ("skewY", [a]) => Affine::skew(0.0, a.to_radians().tan()),
            _ => return Err(invalid()),
        };
        result *= step;
        rest = rest[close + 1..].trim_start_matches([' ', ',', '\t', '\n', '\r']);
    }
    if result.as_coeffs().iter().any(|v| !bounded(*v, 1e6)) || result.determinant().abs() < 1e-12 {
        return Err(invalid());
    }
    Ok(result)
}

/// `style` with its dashes in `path`'s own units: a `pathLength` stands for the whole path.
fn measured(style: Stroke, path: &BezPath, length: Option<f64>) -> Stroke {
    match length {
        Some(length) if !style.dash_pattern.is_empty() => {
            let scale = path.perimeter(0.01) / length;
            let offset = style.dash_offset * scale;
            let dashes: Vec<f64> = style.dash_pattern.iter().map(|dash| dash * scale).collect();
            style.with_dashes(offset, dashes)
        }
        _ => style,
    }
}

fn stroke_outline(path: &BezPath, style: &Stroke) -> Result<BezPath, String> {
    if !style.dash_pattern.is_empty() {
        let shortest = style
            .dash_pattern
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        // Bound subdivision before the stroke library allocates the outline.
        if path.perimeter(0.01) / shortest + path.elements().len() as f64 > 4096.0 {
            return Err("UI_VECTOR: dash geometry budget exceeded".into());
        }
    }
    let outline = stroke(path.iter(), style, &StrokeOpts::default(), 0.01);
    if !outline.is_finite() {
        return Err("UI_VECTOR: invalid stroke geometry".into());
    }
    if outline.elements().len() > crate::protocol::limits::VECTOR_PATH_WORK {
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
                "function render(){{return h('vector',{{viewBox:[0,0,1,1],paths:[{{d:'M0 .5H1',fill:'none',stroke:'#000000',strokeWidth:2,strokeDasharray:[4,2],vectorEffect:'non-scaling-stroke'}}],style:{{width:{width},height:8,backgroundColor:'#ffffff',objectFit:'fill'}}}})}}"
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
            "function render(){return h('view',{},Array.from({length:80},(_,i)=>h('vector',{key:String(i),viewBox:[0,0,1,1],paths:[{d:'M0 .5H1',fill:'none',stroke:'#000000',strokeDasharray:[1,1],vectorEffect:'non-scaling-stroke'}],style:{width:200,height:1,objectFit:'fill'}})))}",
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
            "function render(){return h('vector',{viewBox:[0,0,24,24],paths:[{d:'M0 12H24',fill:'none',stroke:'#000000',strokeWidth:2,strokeDasharray:[4,2]}],style:{width:96,height:96,backgroundColor:'#ffffff'}})}",
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
        let long = |segments: usize| format!("M0 0{}", "h1".repeat(segments - 1));
        assert!(
            Vector::parse(&json!({"viewBox":[0,0,24,24],"paths":[{"d":long(1024)}]}).to_string())
                .is_ok()
        );
        let error =
            Vector::parse(&json!({"viewBox":[0,0,24,24],"paths":[{"d":long(1025)}]}).to_string())
                .err()
                .unwrap();
        assert_eq!(
            error,
            "UI_VECTOR: a path has 1025 segments; the limit is 1024 per path: split it into several paths"
        );
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

    fn raster(paths: &str, extra: &str) -> Vec<u8> {
        let source = crate::session::tests::source(&format!(
            "function render(){{return h('vector',{{viewBox:[0,0,24,24],paths:{paths}{extra},style:{{width:24,height:24,backgroundColor:'#ffffff',color:'#000000'}}}})}}"
        ));
        let mut page = crate::ViewSession::new(&source).unwrap();
        let frame = page.layout(24, 24, 1.0).unwrap().rasterize().unwrap();
        frame.pixels.data_as_u8_slice().to_vec()
    }

    fn at(pixels: &[u8], x: usize, y: usize) -> [u8; 4] {
        pixels[(y * 24 + x) * 4..(y * 24 + x) * 4 + 4]
            .try_into()
            .unwrap()
    }

    #[test]
    fn caps_and_joins_follow_svg_stroke_geometry() {
        let line = |cap: &str| {
            Vector::parse(&json!({"viewBox":[0,0,24,24],"paths":[{"d":"M6 12H18","fill":"none","stroke":"#000000","strokeWidth":4,"strokeLinecap":cap}]}).to_string()).unwrap()
        };
        let outline = |vector: &Vector| vector.shapes[0].0.clone();
        assert!(!outline(&line("butt")).contains((5.0, 12.0).into()));
        assert!(outline(&line("square")).contains((4.5, 13.5).into()));
        assert!(outline(&line("round")).contains((4.5, 12.0).into()));
        assert!(!outline(&line("round")).contains((4.5, 13.5).into()));
        let corner = |join: &str| {
            Vector::parse(&json!({"viewBox":[0,0,24,24],"paths":[{"d":"M4 20L12 4L20 20","fill":"none","stroke":"#000000","strokeWidth":4,"strokeLinejoin":join}]}).to_string()).unwrap().shapes[0].0.clone()
        };
        // The miter tip extends above the apex; bevel and round stay below it.
        assert!(corner("miter").contains((12.0, 1.2).into()));
        assert!(!corner("bevel").contains((12.0, 1.2).into()));
        assert!(!corner("round").contains((12.0, 1.2).into()));
    }

    #[test]
    fn dash_offset_shifts_the_pattern_along_the_path() {
        let dashed = |offset: f64| {
            Vector::parse(&json!({"viewBox":[0,0,24,24],"paths":[{"d":"M0 12H24","fill":"none","stroke":"#000000","strokeWidth":2,"strokeDasharray":[4,4],"strokeDashoffset":offset}]}).to_string()).unwrap().shapes[0].0.clone()
        };
        assert!(dashed(0.0).contains((2.0, 12.0).into()));
        assert!(!dashed(4.0).contains((2.0, 12.0).into()));
        assert!(dashed(4.0).contains((6.0, 12.0).into()));
    }

    #[test]
    fn path_transforms_move_geometry_and_scale_strokes() {
        let pixels = raster(
            "[{d:'M0 0h4v4H0z',fill:'#ff0000',transform:'translate(10 10) scale(2)'}]",
            "",
        );
        assert_eq!(at(&pixels, 14, 14), [255, 0, 0, 255]);
        assert_eq!(at(&pixels, 4, 4), [255, 255, 255, 255]);
        let rotated = Vector::parse(&json!({"viewBox":[0,0,24,24],"paths":[{"d":"M12 2V22","fill":"none","stroke":"#000000","strokeWidth":2,"transform":"rotate(90 12 12)"}]}).to_string()).unwrap();
        assert!(rotated.shapes[0].0.contains((3.0, 12.0).into()));
        assert!(!rotated.shapes[0].0.contains((12.0, 3.0).into()));
        for transform in [
            "perspective(2)",
            "scale(0)",
            "rotate(1 2)",
            "translate(1e9)",
            "matrix(1 0 0 1 0)",
            "translate(1 2",
        ] {
            assert!(
                Vector::parse(
                    &json!({"viewBox":[0,0,24,24],"paths":[{"d":"M0 0h1","transform":transform}]})
                        .to_string()
                )
                .is_err(),
                "{transform}"
            );
        }
    }

    #[test]
    fn named_gradients_paint_fills_in_bounding_box_or_user_space() {
        let gradients = ",gradients:{g:{type:'linear',stops:[{offset:0,color:'#ff0000'},{offset:1,color:'#0000ff'}]},u:{type:'linear',units:'userSpaceOnUse',x1:0,x2:48,stops:[{offset:0,color:'#ff0000'},{offset:1,color:'#0000ff'}]}}";
        let bbox = raster("[{d:'M12 0h12v24H12z',fill:'url(#g)'}]", gradients);
        let [left, right] = [at(&bbox, 12, 12), at(&bbox, 23, 12)];
        assert!(left[0] > 200 && left[2] < 60, "{left:?}");
        assert!(right[2] > 200 && right[0] < 60, "{right:?}");
        assert_eq!(at(&bbox, 4, 12), [255, 255, 255, 255]);
        // User-space units span the whole viewBox width twice over.
        let user = raster("[{d:'M12 0h12v24H12z',fill:'url(#u)'}]", gradients);
        let middle = at(&user, 23, 12);
        assert!(middle[0] > 100 && middle[2] > 60, "{middle:?}");
        assert!(
            Vector::parse(
                &json!({"viewBox":[0,0,24,24],"paths":[{"d":"M0 0h1","fill":"url(#missing)"}]})
                    .to_string()
            )
            .err()
            .unwrap()
            .contains("unknown gradient missing")
        );
        let radial = raster(
            "[{d:'M0 0h24v24H0z',fill:'url(#r)'}]",
            ",gradients:{r:{type:'radial',stops:[{offset:0,color:'#00ff00'},{offset:1,color:'#000000',opacity:0}]}}",
        );
        assert!(at(&radial, 12, 12)[1] > 200);
        assert_eq!(at(&radial, 0, 0), [255, 255, 255, 255]);
    }

    #[test]
    fn conic_gradients_sweep_clockwise_from_the_top() {
        let quarters = ",gradients:{c:{type:'conic',stops:[{offset:0,color:'#ff0000'},{offset:0.25,color:'#ff0000'},{offset:0.25,color:'#00ff00'},{offset:0.5,color:'#00ff00'},{offset:0.5,color:'#0000ff'},{offset:1,color:'#0000ff'}]},r:{type:'conic',from:90,stops:[{offset:0,color:'#ff0000'},{offset:0.25,color:'#ff0000'},{offset:0.25,color:'#00ff00'},{offset:1,color:'#00ff00'}]}}";
        let pixels = raster("[{d:'M0 0h24v24H0z',fill:'url(#c)'}]", quarters);
        assert!(at(&pixels, 18, 4)[0] > 200, "{:?}", at(&pixels, 18, 4));
        assert!(at(&pixels, 18, 18)[1] > 200);
        assert!(at(&pixels, 4, 18)[2] > 200);
        let rotated = raster("[{d:'M0 0h24v24H0z',fill:'url(#r)'}]", quarters);
        assert!(at(&rotated, 18, 18)[0] > 200, "{:?}", at(&rotated, 18, 18));
        assert!(
            Vector::parse(&json!({"viewBox":[0,0,24,24],"gradients":{"c":{"type":"conic","r":1,"stops":[{"offset":0,"color":"#000000"}]}},"paths":[{"d":"M0 0h1","fill":"url(#c)"}]}).to_string()).is_err()
        );
    }

    #[test]
    fn a_key_this_host_does_not_know_is_ignored() {
        let later = json!({"viewBox":[0,0,1,1],"later":1,"paths":[{"d":"M0 0H1V1z","fill":"url(#g)","later":1}],
            "gradients":{"g":{"type":"linear","later":1,"stops":[{"offset":0,"color":"#000000","later":1},{"offset":1,"color":"#ffffff"}]}}});
        assert!(Vector::parse(&later.to_string()).is_ok());
    }
}
