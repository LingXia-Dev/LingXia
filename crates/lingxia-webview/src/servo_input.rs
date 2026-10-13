/// A DOM measurement is usable only while Servo still has that viewport.
/// Keep this check before either touch event so callers can safely re-query.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ClickTarget {
    pub point: [f32; 2],
    pub viewport: [f64; 2],
    pub dpr: f64,
}

impl ClickTarget {
    pub fn matches_viewport(self, size: [u32; 2]) -> bool {
        if !self.dpr.is_finite() || self.dpr <= 0.0 {
            return false;
        }
        // innerWidth/innerHeight are integer CSS pixels; allow their rounding
        // at fractional device scale, but never accept an IME-sized resize.
        let tolerance = self.dpr.max(1.0);
        (0..2).all(|axis| {
            let expected = self.viewport[axis] * self.dpr;
            let actual = f64::from(size[axis]);
            let point = f64::from(self.point[axis]);
            expected.is_finite()
                && expected > 0.0
                && actual > 0.0
                && (actual - expected).abs() <= tolerance
                && point.is_finite()
                && point >= 0.0
                && point < actual
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ClickTarget;

    #[test]
    fn rejects_coordinates_measured_before_keyboard_resize() {
        let before = ClickTarget {
            point: [500.0, 1000.0],
            viewport: [400.0, 800.0],
            dpr: 2.5,
        };
        assert!(before.matches_viewport([1000, 2000]));
        assert!(!before.matches_viewport([1000, 1200]));
        let after = ClickTarget {
            viewport: [400.0, 480.0],
            ..before
        };
        assert!(after.matches_viewport([1000, 1200]));
        assert!(!after.matches_viewport([1000, 2000]));
        assert!(!before.matches_viewport([2000, 1000]));
    }

    #[test]
    fn accepts_css_rounding_but_rejects_invalid_or_offscreen_points() {
        let target = ClickTarget {
            point: [300.0, 700.0],
            viewport: [411.0, 600.0],
            dpr: 2.625,
        };
        assert!(target.matches_viewport([1080, 1575]));
        for point in [
            [-1.0, 20.0],
            [1080.0, 20.0],
            [20.0, 1575.0],
            [f32::NAN, 20.0],
        ] {
            assert!(!ClickTarget { point, ..target }.matches_viewport([1080, 1575]));
        }
        for dpr in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(!ClickTarget { dpr, ..target }.matches_viewport([1080, 1575]));
        }
        assert!(!target.matches_viewport([0, 0]));
    }
}
