//! Bounded numeric keyframes and ordered 2D transforms for retained animation.
use vello_cpu::kurbo::Affine;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Easing(Kind);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Linear,
    StepEnd,
    Bezier([f64; 4]),
}

impl Easing {
    pub const LINEAR: Self = Self(Kind::Linear);
    pub const STEP_END: Self = Self(Kind::StepEnd);

    pub fn cubic_bezier(points: [f64; 4]) -> Result<Self, String> {
        if points.iter().any(|v| !v.is_finite() || v.abs() > 16.0)
            || !(0.0..=1.0).contains(&points[0])
            || !(0.0..=1.0).contains(&points[2])
        {
            return Err("UI_ANIMATION: invalid easing control points".into());
        }
        Ok(Self(Kind::Bezier(points)))
    }

    pub fn sample(self, progress: f64) -> Result<f64, String> {
        valid_progress(progress)?;
        if progress == 0.0 || progress == 1.0 {
            return Ok(progress);
        }
        Ok(match self.0 {
            Kind::Linear => progress,
            Kind::StepEnd => 0.0,
            Kind::Bezier([x1, y1, x2, y2]) => {
                let curve = |t: f64, a: f64, b: f64| {
                    let inverse = 1.0 - t;
                    3.0 * inverse * inverse * t * a + 3.0 * inverse * t * t * b + t * t * t
                };
                // Invert x, rather than treating input progress as the curve parameter.
                // Bisection also handles zero endpoint derivatives without division.
                let (mut low, mut high) = (0.0, 1.0);
                for _ in 0..52 {
                    let t = (low + high) * 0.5;
                    if curve(t, x1, x2) < progress {
                        low = t;
                    } else {
                        high = t;
                    }
                }
                curve((low + high) * 0.5, y1, y2)
            }
        })
    }
}

fn valid_progress(value: f64) -> Result<(), String> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err("UI_ANIMATION: progress outside interval".into())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframe {
    pub offset: f64,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    keys: Vec<Keyframe>,
    easing: Easing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NextChange {
    Continuous,
    At(f64),
    None,
}

impl Track {
    pub(crate) fn storage_bytes(&self) -> usize {
        64 + self.keys.len() * 16
    }
    pub(crate) fn values_within(&self, low: f64, high: f64) -> bool {
        self.keys
            .iter()
            .all(|key| (low..=high).contains(&key.value))
    }
    pub(crate) fn is_constant(&self) -> bool {
        self.keys.iter().all(|key| key.value == self.keys[0].value)
    }
    /// The compiler must resolve implicit endpoints and duplicate offsets first.
    pub fn new(keys: Vec<Keyframe>, easing: Easing) -> Result<Self, String> {
        if !(2..=64).contains(&keys.len())
            || keys[0].offset != 0.0
            || keys[keys.len() - 1].offset != 1.0
            || keys.iter().any(|k| {
                valid_progress(k.offset).is_err() || !k.value.is_finite() || k.value.abs() > 16384.0
            })
            || keys.windows(2).any(|pair| pair[0].offset >= pair[1].offset)
        {
            return Err("UI_ANIMATION: invalid numeric keyframes".into());
        }
        Ok(Self { keys, easing })
    }

    pub fn sample(&self, progress: f64) -> Result<f64, String> {
        valid_progress(progress)?;
        if progress == 1.0 {
            return Ok(self.keys[self.keys.len() - 1].value);
        }
        let index = self.keys.partition_point(|key| key.offset <= progress) - 1;
        let (left, right) = (self.keys[index], self.keys[index + 1]);
        let eased = self
            .easing
            .sample((progress - left.offset) / (right.offset - left.offset))?;
        let value = left.value + (right.value - left.value) * eased;
        if !value.is_finite() || value.abs() > 16384.0 {
            return Err("UI_ANIMATION: sampled value outside bounds".into());
        }
        Ok(value)
    }

    /// Only this iteration; the timing owner also considers the next loop boundary.
    pub fn next_change(&self, progress: f64) -> Result<NextChange, String> {
        valid_progress(progress)?;
        for pair in self.keys.windows(2) {
            if pair[1].offset <= progress || pair[0].value == pair[1].value {
                continue;
            }
            return Ok(if self.easing == Easing::STEP_END {
                NextChange::At(pair[1].offset)
            } else if progress < pair[0].offset {
                NextChange::At(pair[0].offset)
            } else {
                NextChange::Continuous
            });
        }
        Ok(NextChange::None)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Transform {
    Scale([f64; 2]),
    Translate([f64; 2]),
}

/// CSS transform-list order around an explicit border-box origin.
pub fn compose(origin: [f64; 2], operations: &[Transform]) -> Result<Affine, String> {
    if operations.len() > 8 || origin.iter().any(|v| !v.is_finite() || v.abs() > 1e12) {
        return Err("UI_ANIMATION: transform budget exceeded".into());
    }
    let mut transform = Affine::translate((origin[0], origin[1]));
    for operation in operations {
        let (values, bound) = match operation {
            Transform::Scale(values) => (values, 16.0),
            Transform::Translate(values) => (values, 16384.0),
        };
        if values.iter().any(|v| !v.is_finite() || v.abs() > bound) {
            return Err("UI_ANIMATION: invalid transform value".into());
        }
        transform *= match operation {
            Transform::Scale([x, y]) => Affine::scale_non_uniform(*x, *y),
            Transform::Translate([x, y]) => Affine::translate((*x, *y)),
        };
    }
    transform *= Affine::translate((-origin[0], -origin[1]));
    if transform
        .as_coeffs()
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 1e12)
    {
        return Err("UI_ANIMATION: transformed geometry outside bounds".into());
    }
    Ok(transform)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn track(values: &[(f64, f64)], easing: Easing) -> Track {
        Track::new(
            values
                .iter()
                .map(|&(offset, value)| Keyframe { offset, value })
                .collect(),
            easing,
        )
        .unwrap()
    }

    #[test]
    fn reference_blink_jumps_only_at_half_and_iteration_restart() {
        let blink = track(
            &[(0.0, 1.0), (0.49, 1.0), (0.5, 0.0), (1.0, 0.0)],
            Easing::STEP_END,
        );
        for p in [0.0, 0.1, 0.49, 0.499999] {
            assert_eq!(blink.sample(p).unwrap(), 1.0);
            assert_eq!(blink.next_change(p).unwrap(), NextChange::At(0.5));
        }
        for p in [0.5, 0.9, 1.0] {
            assert_eq!(blink.sample(p).unwrap(), 0.0);
            assert_eq!(blink.next_change(p).unwrap(), NextChange::None);
        }
        let timing = super::super::Timing::new(
            std::time::Duration::from_millis(900),
            std::time::Duration::ZERO,
            None,
        )
        .unwrap();
        for (elapsed, value) in [(449, 1.0), (450, 0.0), (899, 0.0), (900, 1.0)] {
            assert_eq!(
                blink
                    .sample(
                        timing
                            .sample(std::time::Duration::from_millis(elapsed))
                            .progress
                    )
                    .unwrap(),
                value
            );
        }
    }

    #[test]
    fn bezier_inverts_input_axis_and_preserves_overshoot() {
        let linear = Easing::cubic_bezier([0.0, 0.0, 1.0, 1.0]).unwrap();
        for p in [0.0, 0.000001, 0.25, 0.5, 0.999999, 1.0] {
            assert!((linear.sample(p).unwrap() - p).abs() < 1e-12);
        }
        // x(t)=t^3, y(t)=1-(1-t)^3; x=1/8 therefore y=7/8.
        let curve = Easing::cubic_bezier([0.0, 1.0, 0.0, 1.0]).unwrap();
        assert!((curve.sample(0.125).unwrap() - 0.875).abs() < 1e-12);
        let overshoot = Easing::cubic_bezier([0.0, 2.0, 1.0, 2.0]).unwrap();
        assert!(overshoot.sample(0.5).unwrap() > 1.0);
    }

    #[test]
    fn chart_transform_order_and_center_origin_match_reference_endpoints() {
        let origin = [50.0, 25.0];
        let first = compose(
            origin,
            &[
                Transform::Scale([0.96; 2]),
                Transform::Translate([0.0, 8.0]),
            ],
        )
        .unwrap();
        let point = first * vello_cpu::kurbo::Point::new(0.0, 0.0);
        assert!((point.x - 2.0).abs() < 1e-12);
        assert!((point.y - 8.68).abs() < 1e-12);
        let reversed = compose(
            origin,
            &[
                Transform::Translate([0.0, 8.0]),
                Transform::Scale([0.96; 2]),
            ],
        )
        .unwrap();
        assert_ne!(first, reversed);
        assert_eq!(
            compose(
                origin,
                &[Transform::Scale([1.0; 2]), Transform::Translate([0.0; 2])]
            )
            .unwrap(),
            Affine::IDENTITY
        );
        let ease = Easing::cubic_bezier([0.16, 1.0, 0.3, 1.0]).unwrap();
        let fade = track(&[(0.0, 0.0), (1.0, 1.0)], ease);
        let scale = track(&[(0.0, 0.96), (1.0, 1.0)], ease);
        let shift = track(&[(0.0, 8.0), (1.0, 0.0)], ease);
        assert!(fade.sample(0.5).unwrap() > 0.9);
        assert!(scale.sample(0.5).unwrap() > 0.996);
        assert!(shift.sample(0.5).unwrap() < 0.8);
    }

    #[test]
    fn constant_spans_do_not_request_continuous_frames() {
        let t = track(
            &[(0.0, 2.0), (0.3, 2.0), (0.6, 4.0), (1.0, 4.0)],
            Easing::LINEAR,
        );
        assert_eq!(t.next_change(0.0).unwrap(), NextChange::At(0.3));
        assert_eq!(t.next_change(0.3).unwrap(), NextChange::Continuous);
        assert_eq!(t.next_change(0.6).unwrap(), NextChange::None);
        assert!((t.sample(0.45).unwrap() - 3.0).abs() < 1e-12);
    }

    #[test]
    fn invalid_numeric_input_fails_before_sampling_or_composition() {
        for points in [
            [-0.1, 0.0, 1.0, 1.0],
            [0.0, 0.0, 1.1, 1.0],
            [0.0, f64::NAN, 1.0, 1.0],
        ] {
            assert!(Easing::cubic_bezier(points).is_err());
        }
        for p in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            assert!(Easing::LINEAR.sample(p).is_err());
        }
        for values in [
            vec![],
            vec![(0.1, 0.0), (1.0, 1.0)],
            vec![(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
            vec![(0.0, 0.0), (1.0, f64::INFINITY)],
        ] {
            assert!(
                Track::new(
                    values
                        .into_iter()
                        .map(|(offset, value)| Keyframe { offset, value })
                        .collect(),
                    Easing::LINEAR
                )
                .is_err()
            );
        }
        assert!(compose([0.0; 2], &[Transform::Scale([17.0; 2])]).is_err());
        assert!(compose([0.0; 2], &[Transform::Translate([f64::NAN, 0.0])]).is_err());
        assert!(compose([1e12; 2], &[Transform::Scale([16.0; 2])]).is_err());
    }
}
