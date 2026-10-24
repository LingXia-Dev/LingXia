use serde::Deserialize;
use vello_cpu::kurbo::{BezPath, Rect, RoundedRect, Shape};

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BoxShadow {
    #[serde(default)]
    pub offset_x: f32,
    #[serde(default)]
    pub offset_y: f32,
    #[serde(default)]
    pub blur: f32,
    #[serde(default)]
    pub spread: f32,
    #[serde(default = "current_color")]
    pub color: String,
}

fn current_color() -> String {
    "currentColor".into()
}

pub(crate) struct ShadowGeometry {
    pub rect: Rect,
    pub radius: f32,
    pub sigma: f32,
    pub extent: [f32; 4],
    corners: CornerShadowGeometry,
}

pub(crate) struct CornerShadowGeometry {
    pub shape: crate::border::RoundedBox,
    pub owner: crate::border::RoundedBox,
    pub sigma: f32,
    pub extent: [f32; 4],
}

impl BoxShadow {
    pub fn parse(source: &str) -> Result<Vec<Self>, String> {
        if source == "none" {
            return Ok(Vec::new());
        }
        let invalid = || "UI_SHADOW: invalid box shadow".to_string();
        if source.len() > 8192 {
            return Err(invalid());
        }
        let shadows: Vec<Self> = serde_json::from_str(source).map_err(|_| invalid())?;
        if shadows.len() > 8
            || shadows.iter().any(|s| {
                [s.offset_x, s.offset_y, s.spread]
                    .iter()
                    .any(|v| !v.is_finite() || v.abs() > 1024.0)
                    || !s.blur.is_finite()
                    || !(0.0..=256.0).contains(&s.blur)
                    || (s.color != "currentColor"
                        && (!matches!(s.color.len(), 7 | 9)
                            || !s.color.starts_with('#')
                            || !s.color[1..].bytes().all(|b| b.is_ascii_hexdigit())))
            })
        {
            return Err(invalid());
        }
        Ok(shadows)
    }

    pub fn geometry(&self, bounds: [f32; 4], radius: f32) -> Option<ShadowGeometry> {
        let geometry = self.corner_geometry(bounds, [radius; 4])?;
        let [x, y, w, h] = geometry.shape.bounds;
        Some(ShadowGeometry {
            rect: Rect::new(x, y, x + w, y + h),
            radius: geometry.shape.radii[0][0] as f32,
            sigma: geometry.sigma,
            extent: geometry.extent,
            corners: geometry,
        })
    }

    pub fn corner_geometry(
        &self,
        bounds: [f32; 4],
        radii: [f32; 4],
    ) -> Option<CornerShadowGeometry> {
        let [x, y, w, h] = bounds;
        let width = w + 2.0 * self.spread;
        let height = h + 2.0 * self.spread;
        if w <= 0.0 || h <= 0.0 || width <= 0.0 || height <= 0.0 {
            return None;
        }
        let owner = crate::border::RoundedBox::normalized(
            bounds.map(f64::from),
            radii.map(|r| [f64::from(r); 2]),
        );
        let spread = f64::from(self.spread);
        let grown = owner.radii.map(|pair| {
            pair.map(|radius| {
                // Apply the CSS sharp-corner correction independently at each corner.
                let adjustment = if spread > radius {
                    spread * (1.0 + (radius / spread - 1.0).powi(3))
                } else {
                    spread
                };
                (radius + adjustment).max(0.0)
            })
        });
        let left = x + self.offset_x - self.spread;
        let top = y + self.offset_y - self.spread;
        let shape =
            crate::border::RoundedBox::normalized([left, top, width, height].map(f64::from), grown);
        let sigma = self.blur / 2.0;
        let reach = 2.5 * sigma;
        Some(CornerShadowGeometry {
            shape,
            owner,
            sigma,
            extent: [
                left - reach,
                top - reach,
                width + 2.0 * reach,
                height + 2.0 * reach,
            ],
        })
    }
}

impl CornerShadowGeometry {
    pub fn knockout(&self) -> BezPath {
        let [x, y, w, h] = self.extent;
        let rect = Rect::new(x as f64, y as f64, (x + w) as f64, (y + h) as f64)
            .union(self.owner.path().bounding_box());
        let mut path = rect.to_path(0.1);
        path.extend(
            self.owner
                .path()
                .reverse_subpaths()
                .elements()
                .iter()
                .copied(),
        );
        path
    }
}

impl ShadowGeometry {
    pub fn knockout(&self) -> BezPath {
        self.corners.knockout()
    }
    pub fn shape(&self) -> RoundedRect {
        RoundedRect::from_rect(self.rect, self.radius as f64)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RasterShadow {
    pub bitmap: crate::image::Bitmap,
    pub bounds: [f32; 4],
}

/// Position-independent masks shared by immutable frames; no resource IO.
#[derive(Default)]
pub(crate) struct MaskCache {
    entries: std::collections::VecDeque<(Vec<u32>, RasterShadow)>,
    bytes: usize,
}
impl MaskCache {
    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
    pub fn get(
        &mut self,
        size: [f32; 2],
        corners: [f32; 4],
        shadow: &BoxShadow,
        color: [u8; 4],
        scale: f32,
    ) -> Result<Option<RasterShadow>, String> {
        if !scale.is_finite()
            || !(0.01..=8.0).contains(&scale)
            || size
                .iter()
                .chain(corners.iter())
                .any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err("UI_SHADOW: invalid mask geometry".into());
        }
        let mut key: Vec<_> = size
            .into_iter()
            .chain(corners)
            .chain([
                shadow.offset_x,
                shadow.offset_y,
                shadow.blur,
                shadow.spread,
                scale,
            ])
            .map(f32::to_bits)
            .collect();
        key.push(u32::from_le_bytes(color));
        if let Some(index) = self.entries.iter().position(|(stored, _)| *stored == key) {
            let entry = self.entries.remove(index).unwrap();
            let mask = entry.1.clone();
            self.entries.push_back(entry);
            return Ok(Some(mask));
        }
        let Some(geometry) = shadow.corner_geometry([0.0, 0.0, size[0], size[1]], corners) else {
            return Ok(None);
        };
        let [x, y, w, h] = geometry.extent;
        let left = (x * scale).floor();
        let top = (y * scale).floor();
        let width = ((x + w) * scale).ceil() - left;
        let height = ((y + h) * scale).ceil() - top;
        if ![left, top, width, height].iter().all(|v| v.is_finite())
            || width < 1.0
            || height < 1.0
            || width > 4096.0
            || height > 4096.0
            || width as f64 * height as f64 > 4.0 * 1024.0 * 1024.0
        {
            return Err("UI_SHADOW: raster mask budget exceeded".into());
        }
        use vello_cpu::color::{AlphaColor, Srgb};
        use vello_cpu::filter_effects::{Filter, FilterFunction};
        use vello_cpu::{Pixmap, RenderContext, Resources};
        let mut context = RenderContext::new(width as u16, height as u16);
        context.set_transform(
            vello_cpu::kurbo::Affine::translate((-f64::from(left), -f64::from(top)))
                * vello_cpu::kurbo::Affine::scale(f64::from(scale)),
        );
        context.push_clip_layer(&geometry.knockout());
        if geometry.sigma > 0.0 {
            context.push_filter_layer(Filter::from_function(FilterFunction::Blur {
                radius: geometry.sigma,
            }));
        }
        context.set_paint(AlphaColor::<Srgb>::from_rgba8(
            color[0], color[1], color[2], color[3],
        ));
        context.fill_path(&geometry.shape.path());
        if geometry.sigma > 0.0 {
            context.pop_layer();
        }
        context.pop_layer();
        context.flush();
        let mut pixels = Pixmap::new(width as u16, height as u16);
        context.render(&mut pixels, &mut Resources::new());
        let mask = RasterShadow {
            bitmap: crate::image::Bitmap::from_pixels(pixels),
            bounds: [left / scale, top / scale, width / scale, height / scale],
        };
        let cost = mask.bitmap.byte_len();
        while self.entries.len() >= 64 || self.bytes + cost > 16 * 1024 * 1024 {
            if let Some((_, old)) = self.entries.pop_front() {
                self.bytes -= old.bitmap.byte_len();
            } else {
                break;
            }
        }
        self.bytes += cost;
        self.entries.push_back((key, mask.clone()));
        Ok(Some(mask))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_corners_grow_without_rounding_sharp_corners() {
        let s = BoxShadow {
            offset_x: 7.0,
            offset_y: 3.0,
            blur: 4.0,
            spread: 10.0,
            color: "#000000".into(),
        };
        let geometry = s
            .corner_geometry([0.0, 0.0, 100.0, 80.0], [24.0, 24.0, 6.0, 0.0])
            .unwrap();
        assert_eq!(geometry.shape.bounds, [-3.0, -7.0, 120.0, 100.0]);
        assert_eq!(geometry.shape.radii[0], [34.0; 2]);
        assert!((geometry.shape.radii[2][0] - 15.36).abs() < 1e-10);
        assert_eq!(geometry.shape.radii[3], [0.0; 2]);
        assert_eq!(
            geometry.owner.radii,
            [[24.0; 2], [24.0; 2], [6.0; 2], [0.0; 2]]
        );
        assert_eq!(geometry.extent, [-8.0, -12.0, 130.0, 110.0]);
        let mask = geometry.knockout();
        assert_eq!(mask.winding((98.0, 78.0).into()), 0);
        assert_eq!(mask.winding((50.0, 40.0).into()), 0);
        assert_ne!(mask.winding((98.0, 2.0).into()), 0);
        let contracted = BoxShadow { spread: -8.0, ..s }
            .corner_geometry([0.0, 0.0, 100.0, 80.0], [24.0, 24.0, 6.0, 0.0])
            .unwrap();
        assert_eq!(
            contracted.shape.radii,
            [[16.0; 2], [16.0; 2], [0.0; 2], [0.0; 2]]
        );
    }
    #[test]
    fn overlapping_owner_corners_normalize_before_shadow_spread() {
        let s = BoxShadow::parse(r#"[{"spread":10}]"#).unwrap().remove(0);
        let geometry = s
            .corner_geometry([0.0, 0.0, 100.0, 60.0], [80.0, 40.0, 10.0, 20.0])
            .unwrap();
        assert_eq!(
            geometry.owner.radii,
            [[48.0; 2], [24.0; 2], [6.0; 2], [12.0; 2]]
        );
        assert_eq!(geometry.shape.radii[0], [58.0; 2]);
        assert_eq!(geometry.shape.radii[1], [34.0; 2]);
        assert!((geometry.shape.radii[2][0] - 15.36).abs() < 1e-10);
        assert_eq!(geometry.shape.radii[3], [22.0; 2]);
    }
    #[test]
    fn filtered_corner_path_preserves_small_corner_shadow_and_owner_knockout() {
        use vello_cpu::color::{AlphaColor, Srgb};
        use vello_cpu::filter_effects::{Filter, FilterFunction};
        use vello_cpu::{Pixmap, RenderContext, Resources};
        let s = BoxShadow::parse(r#"[{"blur":8}]"#).unwrap().remove(0);
        let geometry = s
            .corner_geometry([20.0, 20.0, 80.0, 60.0], [20.0, 20.0, 4.0, 20.0])
            .unwrap();
        let mut context = RenderContext::new(120, 100);
        context.push_clip_layer(&geometry.knockout());
        context.push_filter_layer(Filter::from_function(FilterFunction::Blur {
            radius: geometry.sigma,
        }));
        context.set_paint(AlphaColor::<Srgb>::from_rgba8(0, 0, 0, 128));
        context.fill_path(&geometry.shape.path());
        context.pop_layer();
        context.pop_layer();
        context.flush();
        let mut image = Pixmap::new(120, 100);
        context.render(&mut image, &mut Resources::new());
        let pixels = image.data_as_u8_slice();
        let alpha = |x: usize, y: usize| pixels[(y * 120 + x) * 4 + 3];
        assert_eq!(alpha(60, 50), 0, "shadow excludes owner interior");
        assert_eq!(alpha(96, 76), 0, "small owner corner is also excluded");
        assert!(
            alpha(101, 79) > alpha(101, 21) + 10,
            "small corner casts its own contour"
        );
        assert_eq!(alpha(0, 0), 0);
    }
    #[test]
    fn mask_cache_reuses_pixels_and_bounds_memory_before_allocation() {
        let s = BoxShadow::parse(r#"[{"blur":0}]"#).unwrap().remove(0);
        let mut cache = MaskCache::default();
        let first = cache
            .get(
                [100.0, 60.0],
                [24.0, 24.0, 6.0, 24.0],
                &s,
                [0, 0, 0, 128],
                1.0,
            )
            .unwrap()
            .unwrap();
        let reused = cache
            .get(
                [100.0, 60.0],
                [24.0, 24.0, 6.0, 24.0],
                &s,
                [0, 0, 0, 128],
                1.0,
            )
            .unwrap()
            .unwrap();
        assert_eq!(first.bitmap, reused.bitmap);
        let scaled = cache
            .get(
                [100.0, 60.0],
                [24.0, 24.0, 6.0, 24.0],
                &s,
                [0, 0, 0, 128],
                2.0,
            )
            .unwrap()
            .unwrap();
        assert_ne!(first.bitmap, scaled.bitmap);
        assert_eq!(scaled.bitmap.byte_len(), first.bitmap.byte_len() * 4);
        for size in [[4097.0, 1.0], [4096.0, 1025.0], [f32::INFINITY, 1.0]] {
            assert!(cache.get(size, [6.0; 4], &s, [0; 4], 1.0).is_err());
        }
        for value in 0..4 {
            cache
                .get(
                    [1280.0, 1024.0],
                    [24.0, 24.0, 6.0, 24.0],
                    &s,
                    [value, 0, 0, 128],
                    1.0,
                )
                .unwrap();
        }
        assert!(cache.bytes <= 16 * 1024 * 1024);
        assert!(cache.entries.len() <= 3);
        cache.clear();
        assert_eq!(cache.bytes, 0);
        assert!(cache.entries.is_empty());
    }
    #[test]
    fn spread_preserves_sharp_corners_and_collapses_empty_shadows() {
        let s = BoxShadow::parse(r##"[{"spread":20,"color":"#000000"}]"##)
            .unwrap()
            .remove(0);
        assert_eq!(
            s.geometry([0.0, 0.0, 100.0, 100.0], 10.0).unwrap().radius,
            27.5
        );
        assert_eq!(
            s.geometry([0.0, 0.0, 100.0, 100.0], 0.0).unwrap().radius,
            0.0
        );
        let negative = BoxShadow { spread: -50.0, ..s };
        assert!(negative.geometry([0.0, 0.0, 100.0, 100.0], 10.0).is_none());
    }
    #[test]
    fn rejects_unbounded_and_unsupported_shadows() {
        for source in [
            "null",
            "{}",
            r#"[{"blur":-1}]"#,
            r#"[{"blur":257}]"#,
            r#"[{"spread":1025}]"#,
            r#"[{"inset":true}]"#,
            r#"[{"color":"red"}]"#,
        ] {
            assert!(BoxShadow::parse(source).is_err(), "{source}");
        }
        assert!(BoxShadow::parse("none").unwrap().is_empty());
        assert!(BoxShadow::parse("[]").unwrap().is_empty());
    }
}
