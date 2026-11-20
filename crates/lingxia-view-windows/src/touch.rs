//! Touch release velocity, as Android's tracker measures it: from the last stretch of the
//! gesture only, and zero once the finger rested before lifting.

/// Samples older than this, counted back from the newest, do not shape the velocity.
const HORIZON_MS: f64 = 100.0;
/// A finger still this long before lifting releases without velocity.
const REST_MS: f64 = 40.0;

#[derive(Default)]
pub(crate) struct Velocity {
    /// `(ms, x, y)`, oldest first.
    samples: Vec<(f64, f32, f32)>,
}

impl Velocity {
    pub(crate) fn reset(&mut self) {
        self.samples.clear();
    }

    pub(crate) fn add(&mut self, ms: f64, x: f32, y: f32) {
        self.samples.push((ms, x, y));
        self.samples.retain(|&(t, _, _)| ms - t <= HORIZON_MS);
    }

    /// Pixels per second at release time `ms`.
    pub(crate) fn release(&self, ms: f64) -> [f32; 2] {
        let Some(&(last_moved, x1, y1)) = self.last_move() else {
            return [0.0, 0.0];
        };
        if ms - last_moved > REST_MS {
            return [0.0, 0.0];
        }
        let Some(&(t0, x0, y0)) = self.samples.first() else {
            return [0.0, 0.0];
        };
        let dt = (last_moved - t0) / 1000.0;
        if dt <= 0.0 {
            return [0.0, 0.0];
        }
        [
            ((x1 - x0) as f64 / dt) as f32,
            ((y1 - y0) as f64 / dt) as f32,
        ]
    }

    /// The newest sample that moved from the one before it.
    fn last_move(&self) -> Option<&(f64, f32, f32)> {
        self.samples
            .windows(2)
            .rev()
            .find(|pair| (pair[0].1, pair[0].2) != (pair[1].1, pair[1].2))
            .map(|pair| &pair[1])
    }
}

#[cfg(test)]
mod tests {
    use super::Velocity;

    #[test]
    fn a_flick_releases_at_the_speed_of_its_last_stretch() {
        let mut v = Velocity::default();
        // Slow start, then 10 px per 8 ms: only the last 100 ms count.
        v.add(0.0, 0.0, 0.0);
        v.add(200.0, 0.0, 10.0);
        for i in 1..=10 {
            v.add(200.0 + 8.0 * i as f64, 0.0, 10.0 + 10.0 * i as f32);
        }
        let [vx, vy] = v.release(280.0);
        assert_eq!(vx, 0.0);
        assert!((vy - 1250.0).abs() < 1.0, "{vy}");
    }

    #[test]
    fn a_finger_that_rests_before_lifting_releases_still() {
        let mut v = Velocity::default();
        for i in 0..=10 {
            v.add(8.0 * i as f64, 0.0, 10.0 * i as f32);
        }
        // Held in place, still reporting its point, as a touch screen does.
        for i in 1..=6 {
            v.add(80.0 + 16.0 * i as f64, 0.0, 100.0);
        }
        assert_eq!(v.release(176.0), [0.0, 0.0]);
    }
}
