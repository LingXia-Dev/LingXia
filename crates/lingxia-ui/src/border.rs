use vello_cpu::kurbo::BezPath;

/// Clockwise corner radii: top-left, top-right, bottom-right, bottom-left.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RoundedBox {
    pub bounds: [f64; 4],
    pub radii: [[f64; 2]; 4],
}

impl RoundedBox {
    pub fn normalized(bounds: [f64; 4], mut radii: [[f64; 2]; 4]) -> Self {
        for radius in &mut radii {
            if radius[0] <= 0.0 || radius[1] <= 0.0 {
                *radius = [0.0; 2];
            }
        }
        let mut factor = 1.0_f64;
        for (extent, sum) in [
            (bounds[2], radii[0][0] + radii[1][0]),
            (bounds[2], radii[3][0] + radii[2][0]),
            (bounds[3], radii[0][1] + radii[3][1]),
            (bounds[3], radii[1][1] + radii[2][1]),
        ] {
            if sum > 0.0 {
                factor = factor.min(extent.max(0.0) / sum);
            }
        }
        for radius in &mut radii {
            radius[0] *= factor;
            radius[1] *= factor;
        }
        Self { bounds, radii }
    }

    pub fn contains(&self, point: [f64; 2]) -> bool {
        let [x, y, w, h] = self.bounds;
        let [px, py] = point;
        if w <= 0.0
            || h <= 0.0
            || !px.is_finite()
            || !py.is_finite()
            || px < x
            || px > x + w
            || py < y
            || py > y + h
        {
            return false;
        }
        for (index, [rx, ry]) in self.radii.iter().copied().enumerate() {
            if rx == 0.0 || ry == 0.0 {
                continue;
            }
            let right = index == 1 || index == 2;
            let bottom = index >= 2;
            let cx = if right { x + w - rx } else { x + rx };
            let cy = if bottom { y + h - ry } else { y + ry };
            if (if right { px > cx } else { px < cx }) && (if bottom { py > cy } else { py < cy }) {
                return ((px - cx) / rx).powi(2) + ((py - cy) / ry).powi(2) <= 1.0;
            }
        }
        true
    }

    pub fn intersects(&self, rect: [f64; 4]) -> bool {
        let [x, y, w, h] = self.bounds;
        let left = x.max(rect[0]);
        let top = y.max(rect[1]);
        let right = (x + w).min(rect[0] + rect[2]);
        let bottom = (y + h).min(rect[1] + rect[3]);
        if left >= right || top >= bottom {
            return false;
        }
        self.radii.iter().enumerate().any(|(index, [rx, ry])| {
            let cx = if index == 1 || index == 2 {
                x + w - rx
            } else {
                x + rx
            };
            let cy = if index >= 2 { y + h - ry } else { y + ry };
            self.contains([cx.clamp(left, right), cy.clamp(top, bottom)])
        })
    }

    pub fn path(&self) -> BezPath {
        let [x, y, w, h] = self.bounds;
        let mut path = BezPath::new();
        if w <= 0.0 || h <= 0.0 {
            return path;
        }
        let [[ax, ay], [bx, by], [cx, cy], [dx, dy]] = self.radii;
        let k = 4.0 * (2.0_f64.sqrt() - 1.0) / 3.0;
        path.move_to((x + ax, y));
        path.line_to((x + w - bx, y));
        path.curve_to(
            (x + w - bx + k * bx, y),
            (x + w, y + by - k * by),
            (x + w, y + by),
        );
        path.line_to((x + w, y + h - cy));
        path.curve_to(
            (x + w, y + h - cy + k * cy),
            (x + w - cx + k * cx, y + h),
            (x + w - cx, y + h),
        );
        path.line_to((x + dx, y + h));
        path.curve_to(
            (x + dx - k * dx, y + h),
            (x, y + h - dy + k * dy),
            (x, y + h - dy),
        );
        path.line_to((x, y + ay));
        path.curve_to((x, y + ay - k * ay), (x + ax - k * ax, y), (x + ax, y));
        path.close_path();
        path
    }
}

/// One nonzero-fill ring, including unequal border widths and elliptical inner corners.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Border {
    pub outer: RoundedBox,
    pub inner: RoundedBox,
}

impl Border {
    /// Inputs are validated finite nonnegative lengths; widths use top/right/bottom/left order.
    pub fn new(bounds: [f32; 4], widths: [f32; 4], radius: f32) -> Self {
        Self::with_radii(bounds, widths, [[radius; 2]; 4])
    }

    pub fn with_radii(bounds: [f32; 4], widths: [f32; 4], radii: [[f32; 2]; 4]) -> Self {
        let bounds = bounds.map(f64::from);
        let [x, y, w, h] = bounds;
        let [top, right, bottom, left] = widths.map(f64::from);
        let outer = RoundedBox::normalized(bounds, radii.map(|pair| pair.map(f64::from)));
        // Derive the inner corners from the normalized outer contour, so border
        // thickness stays consistent when oversized adjacent radii overlap.
        let [[ax, ay], [bx, by], [cx, cy], [dx, dy]] = outer.radii;
        let inner = RoundedBox::normalized(
            [
                x + left,
                y + top,
                (w - left - right).max(0.0),
                (h - top - bottom).max(0.0),
            ],
            [
                [ax - left, ay - top],
                [bx - right, by - top],
                [cx - right, cy - bottom],
                [dx - left, dy - bottom],
            ],
        );
        Self { outer, inner }
    }

    pub fn path(&self) -> BezPath {
        let mut path = self.outer.path();
        path.extend(
            self.inner
                .path()
                .reverse_subpaths()
                .elements()
                .iter()
                .copied(),
        );
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vello_cpu::kurbo::{Point, Shape};

    #[test]
    fn independent_corners_preserve_small_bubble_tail_and_unequal_border_widths() {
        let border = Border::with_radii(
            [0.0, 0.0, 100.0, 80.0],
            [2.0, 4.0, 6.0, 8.0],
            [[24.0; 2], [24.0; 2], [6.0; 2], [24.0; 2]],
        );
        assert_eq!(
            border.outer.radii,
            [[24.0; 2], [24.0; 2], [6.0; 2], [24.0; 2]]
        );
        assert_eq!(
            border.inner.radii,
            [[16.0, 22.0], [20.0, 22.0], [0.0; 2], [16.0, 18.0]]
        );
        assert!(!border.outer.contains([98.0, 2.0]));
        assert!(border.outer.contains([98.0, 78.0]));
        assert_eq!(border.path().winding((98.0, 2.0).into()), 0);
        assert_ne!(border.path().winding((98.0, 78.0).into()), 0);
        assert!(!border.inner.contains([9.0, 3.0]));
        assert!(border.inner.contains([95.0, 73.0]));
    }

    #[test]
    fn unequal_corner_overlap_scales_all_radii_before_deriving_inner_contour() {
        let border = Border::with_radii(
            [0.0, 0.0, 100.0, 60.0],
            [2.0; 4],
            [[80.0; 2], [40.0; 2], [10.0; 2], [20.0; 2]],
        );
        // The left edge (80+20 > 60) determines one shared 0.6 scale.
        assert_eq!(
            border.outer.radii,
            [[48.0; 2], [24.0; 2], [6.0; 2], [12.0; 2]]
        );
        assert_eq!(
            border.inner.radii,
            [[46.0; 2], [22.0; 2], [4.0; 2], [10.0; 2]]
        );
        let zero = Border::with_radii(
            [0.0, 0.0, 100.0, 60.0],
            [0.0; 4],
            [[80.0; 2], [40.0; 2], [10.0; 2], [20.0; 2]],
        );
        for y in 0..60 {
            for x in 0..100 {
                assert_eq!(
                    zero.path().winding((x as f64 + 0.5, y as f64 + 0.5).into()),
                    0
                );
            }
        }
    }

    #[test]
    fn independent_corner_border_pixels_preserve_translucency_and_inner_hole() {
        use vello_cpu::color::{AlphaColor, Srgb};
        use vello_cpu::{Pixmap, RenderContext, Resources};
        let mut context = RenderContext::new(100, 80);
        context.set_paint(AlphaColor::<Srgb>::from_rgb8(255, 0, 0));
        context.fill_rect(&vello_cpu::kurbo::Rect::new(0.0, 0.0, 100.0, 80.0));
        let border = Border::with_radii(
            [0.0, 0.0, 100.0, 80.0],
            [4.0; 4],
            [[24.0; 2], [24.0; 2], [6.0; 2], [24.0; 2]],
        );
        context.set_paint(AlphaColor::<Srgb>::from_rgba8(0, 0, 255, 128));
        context.fill_path(&border.path());
        context.flush();
        let mut image = Pixmap::new(100, 80);
        context.render(&mut image, &mut Resources::new());
        let pixels = image.data_as_u8_slice();
        let pixel = |x: usize, y: usize| &pixels[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4];
        assert_eq!(pixel(97, 2), &[255, 0, 0, 255]);
        assert_eq!(pixel(97, 77), &[127, 0, 128, 255]);
        assert_eq!(pixel(94, 74), &[255, 0, 0, 255]);
    }

    #[test]
    fn inner_clip_hit_testing_and_culling_use_elliptical_corners() {
        let inner = Border::new([0.0, 0.0, 100.0, 80.0], [2.0, 4.0, 6.0, 8.0], 20.0).inner;
        for point in [[9.0, 3.0], [95.0, 73.0], [7.0, 20.0], [f64::NAN, 20.0]] {
            assert!(!inner.contains(point));
        }
        for point in [[20.0, 3.0], [9.0, 20.0], [80.0, 73.0]] {
            assert!(inner.contains(point));
            assert_ne!(inner.path().winding((point[0], point[1]).into()), 0);
        }
        assert!(!inner.intersects([8.0, 2.0, 1.0, 1.0]));
        assert!(!inner.intersects([95.0, 73.0, 1.0, 1.0]));
        assert!(inner.intersects([8.0, 19.0, 2.0, 2.0]));
        assert!(inner.intersects([80.0, 73.0, 1.0, 1.0]));
        assert!(inner.intersects([0.0, 0.0, 100.0, 80.0]));
    }

    #[test]
    fn translucent_ring_paints_corners_once_and_preserves_center() {
        use vello_cpu::color::{AlphaColor, Srgb};
        use vello_cpu::kurbo::Rect;
        use vello_cpu::{Pixmap, RenderContext, Resources};
        let mut context = RenderContext::new(100, 80);
        context.set_paint(AlphaColor::<Srgb>::from_rgb8(255, 0, 0));
        context.fill_rect(&Rect::new(0.0, 0.0, 100.0, 80.0));
        context.set_paint(AlphaColor::<Srgb>::from_rgba8(0, 0, 255, 128));
        context.fill_path(&Border::new([0.0, 0.0, 100.0, 80.0], [8.0; 4], 0.0).path());
        context.flush();
        let mut pixels = Pixmap::new(100, 80);
        context.render(&mut pixels, &mut Resources::new());
        let pixel = |x: usize, y: usize| {
            &pixels.data_as_u8_slice()[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4]
        };
        assert_eq!(pixel(50, 40), [255, 0, 0, 255]);
        assert_eq!(pixel(2, 2), pixel(50, 2));
        assert_eq!(pixel(2, 2), [127, 0, 128, 255]);
    }

    #[test]
    fn unequal_edges_keep_the_center_transparent() {
        let border = Border::new([10.0, 20.0, 100.0, 80.0], [2.0, 4.0, 6.0, 8.0], 0.0);
        assert_eq!(border.inner.bounds, [18.0, 22.0, 88.0, 72.0]);
        let path = border.path();
        for point in [(60.0, 21.0), (108.0, 60.0), (60.0, 97.0), (14.0, 60.0)] {
            assert_ne!(path.winding(Point::from(point)), 0);
        }
        for point in [(60.0, 60.0), (19.0, 23.0), (9.0, 60.0)] {
            assert_eq!(path.winding(Point::from(point)), 0);
        }
    }

    #[test]
    fn rounded_edges_have_elliptical_inner_corners() {
        let border = Border::new([0.0, 0.0, 100.0, 80.0], [2.0, 4.0, 6.0, 8.0], 20.0);
        assert_eq!(
            border.inner.radii,
            [[12.0, 18.0], [16.0, 18.0], [16.0, 14.0], [12.0, 14.0]]
        );
        let path = border.path();
        assert_eq!(path.winding((1.0, 1.0).into()), 0);
        assert_ne!(path.winding((7.0, 7.0).into()), 0);
        assert_eq!(path.winding((20.0, 20.0).into()), 0);
        assert_eq!(path.bounding_box(), border.outer.path().bounding_box());
    }

    #[test]
    fn zero_and_oversized_borders_do_not_invert_the_hole() {
        let empty = Border::new([0.0, 0.0, 100.0, 80.0], [0.0; 4], 20.0).path();
        for point in [(1.0, 40.0), (50.0, 1.0), (50.0, 40.0)] {
            assert_eq!(empty.winding(point.into()), 0);
        }
        let solid = Border::new([0.0, 0.0, 100.0, 80.0], [50.0; 4], 500.0);
        assert_eq!(solid.inner.bounds[2..], [0.0, 0.0]);
        assert_ne!(solid.path().winding((50.0, 40.0).into()), 0);
        assert_eq!(solid.path().winding((1.0, 1.0).into()), 0);
    }
}
