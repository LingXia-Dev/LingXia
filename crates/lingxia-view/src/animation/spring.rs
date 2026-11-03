//! A damped spring set by perceptual duration and bounce, as SwiftUI sets one. Each channel is
//! closed-form, `x(t) = to + (from - to)·A(t) + v0·B(t)`, so sampling stays a pure function of time.
use std::time::Duration;

/// Motion under this fraction of its initial size has settled.
const SETTLED: f64 = 1e-3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    /// Undamped angular frequency, rad/s.
    omega: f64,
    /// Damping ratio in (0, 1].
    zeta: f64,
}

impl Spring {
    /// `bounce` 0 is critically damped; towards 1 it oscillates longer.
    pub fn new(duration: Duration, bounce: f64) -> Result<Self, String> {
        let seconds = duration.as_secs_f64();
        if !(bounce.is_finite() && (0.0..=0.9).contains(&bounce)) || seconds <= 0.0 {
            return Err("UI_TRANSITION: spring bounce is 0..0.9".into());
        }
        Ok(Self {
            omega: std::f64::consts::TAU / seconds,
            zeta: 1.0 - bounce,
        })
    }

    /// `[A, B, A', B']` at `t` seconds.
    pub fn basis(&self, t: f64) -> [f64; 4] {
        let Self { omega, zeta } = *self;
        let envelope = (-zeta * omega * t).exp();
        if zeta >= 1.0 {
            return [
                envelope * (1.0 + omega * t),
                t * envelope,
                -omega * omega * t * envelope,
                envelope * (1.0 - omega * t),
            ];
        }
        let damped = omega * (1.0 - zeta * zeta).sqrt();
        let (sin, cos) = (damped * t).sin_cos();
        let ratio = zeta * omega / damped;
        let b = envelope * sin / damped;
        [
            envelope * (cos + ratio * sin),
            b,
            -omega * omega * b,
            envelope * (cos - ratio * sin),
        ]
    }

    /// The value and velocity of one channel at `t`.
    pub fn at(&self, from: f64, to: f64, velocity: f64, t: f64) -> (f64, f64) {
        let [a, b, da, db] = self.basis(t);
        let displacement = from - to;
        (
            to + displacement * a + velocity * b,
            displacement * da + velocity * db,
        )
    }

    /// Seconds until a channel moving `displacement` with `velocity` stays settled, at most `limit`.
    pub fn settle(&self, displacement: f64, velocity: f64, limit: f64) -> f64 {
        let scale = displacement.abs() + velocity.abs() / self.omega;
        if scale == 0.0 {
            return 0.0;
        }
        // A bound on the remaining motion: the envelope times the largest amplitude it can reach.
        let damped = self.omega * (1.0 - self.zeta * self.zeta).max(0.0).sqrt();
        let reach = |t: f64| {
            let envelope = (-self.zeta * self.omega * t).exp();
            let amplitude = if self.zeta >= 1.0 {
                displacement.abs() * (1.0 + self.omega * t) + velocity.abs() * t
            } else {
                displacement.abs() * (1.0 + self.zeta * self.omega / damped)
                    + velocity.abs() / damped
            };
            envelope * amplitude / scale
        };
        let step = std::f64::consts::TAU / self.omega / 64.0;
        let mut t = 0.0;
        while t < limit && reach(t) > SETTLED {
            t += step;
        }
        t.min(limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spring_starts_where_it_is_with_its_velocity_and_settles_on_target() {
        for bounce in [0.0, 0.3] {
            let spring = Spring::new(Duration::from_millis(400), bounce).unwrap();
            assert_eq!(spring.at(100.0, 0.0, -50.0, 0.0), (100.0, -50.0));
            let end = spring.settle(100.0, -50.0, 10.0);
            assert!(end > 0.2 && end < 2.0, "{bounce}: {end}");
            let (x, v) = spring.at(100.0, 0.0, -50.0, end);
            assert!(x.abs() < 0.2 && v.abs() < 5.0, "{bounce}: {x} {v}");
        }
    }

    #[test]
    fn velocity_is_the_derivative_of_position() {
        let spring = Spring::new(Duration::from_millis(300), 0.25).unwrap();
        for t in [0.01, 0.1, 0.25] {
            let h = 1e-6;
            let (a, _) = spring.at(40.0, 0.0, 300.0, t - h);
            let (b, _) = spring.at(40.0, 0.0, 300.0, t + h);
            let (_, v) = spring.at(40.0, 0.0, 300.0, t);
            assert!(((b - a) / (2.0 * h) - v).abs() < 1e-3, "{t}");
        }
    }

    #[test]
    fn a_critically_damped_spring_never_overshoots_and_a_bouncy_one_does() {
        let samples = |bounce| {
            let spring = Spring::new(Duration::from_millis(400), bounce).unwrap();
            (0..100).map(move |i| spring.at(1.0, 0.0, 0.0, i as f64 * 0.01).0)
        };
        assert!(samples(0.0).all(|x| x >= -1e-9));
        assert!(samples(0.4).any(|x| x < -0.01));
        assert!(Spring::new(Duration::from_millis(400), 1.0).is_err());
    }
}
