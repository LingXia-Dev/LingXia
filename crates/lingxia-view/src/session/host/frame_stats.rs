//! Frame records for one page: when each frame was wanted, when it showed and what it cost.
//! Every platform feeds the same records, so the numbers mean the same thing everywhere.
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::time::Duration;

pub(crate) use crate::scene::hud::Hud;

/// About 10 s at 120 Hz.
const CAPACITY: usize = 1200;
/// How long the HUD's finger and frame trails last, and how many points they keep.
const TRAIL: Duration = Duration::from_millis(500);
const TRAIL_POINTS: usize = 64;
/// An input no frame picked up this long is not what a later frame shows.
const INPUT_STALE: Duration = Duration::from_millis(500);
/// Frame times the HUD's sparkline draws.
pub(crate) const SPARK: usize = 60;
pub(super) const DEFAULT_WINDOW: Duration = Duration::from_secs(5);

/// One presented frame, in [`crate::platform::Clock::now`] time.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct FrameRecord {
    /// When something first asked for this frame.
    pub demand: Duration,
    pub present: Duration,
    pub interval: Duration,
    pub cpu_us: u64,
    pub queue_us: u64,
    pub gpu_us: Option<u64>,
    /// Since the previous present, for a frame wanted before that one showed: a frame
    /// that starts after idle has no frame time, only latency.
    pub delta: Option<Duration>,
    /// Shown after the display refresh it was meant for.
    pub late: bool,
    pub input: Option<Duration>,
}

impl FrameRecord {
    /// Display refreshes skipped since the previous frame.
    fn skipped(&self) -> u64 {
        match self.delta {
            Some(delta) if !self.interval.is_zero() => {
                let refreshes = delta.as_secs_f64() / self.interval.as_secs_f64();
                (refreshes.round() as u64).saturating_sub(1)
            }
            _ => 0,
        }
    }

    fn janky(&self) -> bool {
        self.delta.is_some_and(|delta| delta > self.interval * 2)
    }
}

/// What a frame reports once it reached the screen.
pub(super) struct Presented {
    pub demand: Duration,
    /// The display refresh the frame was meant for.
    pub target: Duration,
    /// When the host finished handing the frame to the display.
    pub done: Duration,
    pub interval: Duration,
    pub cpu_us: u64,
    pub queue_us: u64,
    pub gpu_us: Option<u64>,
    pub input: Option<Duration>,
}

/// The first refresh at or after both `target` and `done`, and whether that is later than
/// `target`.
pub(super) fn present_time(
    target: Duration,
    done: Duration,
    interval: Duration,
) -> (Duration, bool) {
    if done <= target {
        return (target, false);
    }
    if interval.is_zero() {
        return (done, true);
    }
    let behind = (done - target).as_nanos().div_ceil(interval.as_nanos());
    (
        target + interval * u32::try_from(behind).unwrap_or(u32::MAX),
        true,
    )
}

#[derive(Debug, Default)]
pub(super) struct FrameStats {
    records: VecDeque<FrameRecord>,
    /// Earliest input that no requested frame carries yet.
    input: Option<Duration>,
    /// A reveal waiting for its first present.
    reveal_started: Option<Duration>,
    /// The most recent reveal's wait for a frame: `(ms, reason)`.
    first_present: Option<(f64, &'static str)>,
    reveals: u64,
    last_reveal: Option<Duration>,
    /// Each reveal's wait for a frame, by when it showed: `(at, ms)`.
    reveal_waits: VecDeque<(Duration, f64)>,
    /// Pointer positions (device px) as they arrived, and as of each frame that showed them.
    finger: VecDeque<(Duration, [f32; 2])>,
    shown: VecDeque<(Duration, [f32; 2])>,
    point: Option<[f32; 2]>,
}

fn trail_push(trail: &mut VecDeque<(Duration, [f32; 2])>, at: Duration, point: [f32; 2]) {
    while trail.len() >= TRAIL_POINTS
        || trail
            .front()
            .is_some_and(|(t, _)| at.saturating_sub(*t) > TRAIL)
    {
        trail.pop_front();
    }
    trail.push_back((at, point));
}

impl FrameStats {
    pub(super) fn input(&mut self, at: Duration) {
        self.input = Some(self.input.map_or(at, |held| held.min(at)));
    }

    /// A pointer moved to `point` (device px) at `at`.
    pub(super) fn pointer(&mut self, at: Duration, point: [f32; 2]) {
        trail_push(&mut self.finger, at, point);
        self.point = Some(point);
    }

    /// The newest pointer position a frame requested now will show.
    pub(super) fn take_point(&mut self) -> Option<[f32; 2]> {
        self.point.take()
    }

    /// The input a frame requested now will show.
    pub(super) fn take_input(&mut self, now: Duration) -> Option<Duration> {
        self.input
            .take()
            .filter(|at| now.saturating_sub(*at) <= INPUT_STALE)
    }

    /// The page started to show. Hosts report showing a page more than once (visibility, then
    /// the reveal); reports this close together are one reveal.
    pub(super) fn reveal_started(&mut self, at: Duration) {
        if self.reveal_started.is_some() || self.same_reveal(at) {
            return;
        }
        self.reveals += 1;
        self.last_reveal = Some(at);
        self.reveal_started = Some(at);
    }

    /// The reveal showed a frame already on its surface.
    pub(super) fn reveal_current(&mut self, at: Duration) {
        let Some(started) = self.reveal_started.take() else {
            return;
        };
        self.first_present = Some((0.0, "current"));
        self.reveal_wait(started.max(at), 0.0);
    }

    fn same_reveal(&self, at: Duration) -> bool {
        self.last_reveal
            .is_some_and(|last| at.saturating_sub(last) < Duration::from_millis(100))
    }

    fn reveal_wait(&mut self, at: Duration, ms: f64) {
        if self.reveal_waits.len() == TRAIL_POINTS {
            self.reveal_waits.pop_front();
        }
        self.reveal_waits.push_back((at, ms));
    }

    pub(super) fn record(&mut self, frame: Presented, point: Option<[f32; 2]>) -> FrameRecord {
        let (present, late) = present_time(frame.target, frame.done, frame.interval);
        let previous = self.records.back().map(|record| record.present);
        // Wanted by the refresh after the previous frame: content that changes every frame
        // (animation, scroll, a fast data feed) rather than after a pause.
        let delta = previous
            .filter(|previous| frame.demand <= *previous + frame.interval && present > *previous)
            .map(|previous| present - previous);
        let record = FrameRecord {
            demand: frame.demand,
            present,
            interval: frame.interval,
            cpu_us: frame.cpu_us,
            queue_us: frame.queue_us,
            gpu_us: frame.gpu_us,
            delta,
            late,
            input: frame.input.map(|at| present.saturating_sub(at)),
        };
        if let Some(started) = self.reveal_started.take() {
            let wait = ms(present.saturating_sub(started));
            self.first_present = Some((wait, "frame"));
            self.reveal_wait(present, wait);
        }
        if let Some(point) = point {
            trail_push(&mut self.shown, present, point);
        }
        if self.records.len() == CAPACITY {
            self.records.pop_front();
        }
        self.records.push_back(record);
        record
    }

    fn window(&self, now: Duration, window: Duration) -> impl Iterator<Item = &FrameRecord> {
        let from = now.saturating_sub(window);
        self.records
            .iter()
            .filter(move |record| record.present >= from)
    }

    /// Frame time, cost and latency percentiles over the frames shown in the last `window`.
    pub(super) fn summary(&self, now: Duration, window: Duration) -> Value {
        let records: Vec<&FrameRecord> = self.window(now, window).collect();
        let deltas: Vec<f64> = records.iter().filter_map(|r| r.delta).map(ms).collect();
        let spent: f64 = deltas.iter().sum();
        let interval = records.last().map(|r| ms(r.interval));
        json!({
            "windowMs": window.as_millis() as u64,
            "frames": records.len(),
            "samples": deltas.len(),
            "refreshMs": interval.map(round2),
            "frameTimeMs": percentiles(&deltas),
            "fps": (spent > 0.0).then(|| round2(deltas.len() as f64 * 1000.0 / spent)),
            "dropped": records.iter().map(|r| r.skipped()).sum::<u64>(),
            "jank": records.iter().filter(|r| r.janky()).count(),
            "late": records.iter().filter(|r| r.late).count(),
            "cpuMs": percentiles(&records.iter().map(|r| r.cpu_us as f64 / 1000.0).collect::<Vec<_>>()),
            "queueMs": percentiles(&records.iter().map(|r| r.queue_us as f64 / 1000.0).collect::<Vec<_>>()),
            "gpuMs": percentiles(&records.iter().filter_map(|r| r.gpu_us).map(|us| us as f64 / 1000.0).collect::<Vec<_>>()),
            "inputToPresent": {
                "count": records.iter().filter(|r| r.input.is_some()).count(),
                "lastMs": self.records.iter().rev().find_map(|r| r.input).map(|d| round2(ms(d))),
                "statsMs": percentiles(&records.iter().filter_map(|r| r.input).map(ms).collect::<Vec<_>>()),
            },
            "firstPresentMs": self.first_present.map(|(ms, _)| round2(ms)),
            "firstPresentReason": self.first_present.map(|(_, reason)| reason),
            "reveals": self.reveals,
            "revealMs": percentiles(
                &self
                    .reveal_waits
                    .iter()
                    .filter(|(at, _)| *at >= now.saturating_sub(window))
                    .map(|(_, ms)| *ms)
                    .collect::<Vec<_>>(),
            ),
        })
    }

    /// The HUD's lines and the newest frame times.
    pub(super) fn hud(&self, now: Duration) -> Hud {
        let summary = self.summary(now, Duration::from_secs(2));
        let number = |value: &Value| value.as_f64();
        let times = &summary["frameTimeMs"];
        let refresh = self
            .records
            .back()
            .map(|r| ms(r.interval))
            .unwrap_or(16.667);
        let fmt = |value: Option<f64>| value.map_or("-".to_owned(), |v| format!("{v:.1}"));
        let lines = vec![
            format!(
                "frame {} ms  {} fps",
                fmt(number(&times["p50"])),
                summary["fps"]
                    .as_f64()
                    .map_or("-".to_owned(), |f| format!("{f:.0}"))
            ),
            format!(
                "p95 {}  p99 {}",
                fmt(number(&times["p95"])),
                fmt(number(&times["p99"]))
            ),
            format!("dropped {}  jank {}", summary["dropped"], summary["jank"]),
            format!(
                "input {} ms",
                fmt(number(&summary["inputToPresent"]["lastMs"]))
            ),
            format!("first {} ms", fmt(number(&summary["firstPresentMs"]))),
        ];
        let spark = self
            .records
            .iter()
            .rev()
            .filter_map(|r| r.delta.map(ms))
            .take(SPARK)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let recent = |trail: &VecDeque<(Duration, [f32; 2])>| {
            trail
                .iter()
                .filter(|(at, _)| now.saturating_sub(*at) <= TRAIL)
                .map(|(_, point)| *point)
                .collect()
        };
        Hud {
            lines,
            spark,
            refresh,
            finger: recent(&self.finger),
            shown: recent(&self.shown),
        }
    }
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// `{p50, p95, p99, max}` by nearest rank, or null without samples.
fn percentiles(values: &[f64]) -> Value {
    if values.is_empty() {
        return Value::Null;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let rank = |p: f64| {
        let index = ((p * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len()) - 1;
        round2(sorted[index])
    };
    json!({
        "p50": rank(0.50),
        "p95": rank(0.95),
        "p99": rank(0.99),
        "max": round2(sorted[sorted.len() - 1]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    impl FrameStats {
        fn record_test(&mut self, frame: Presented) -> FrameRecord {
            self.record(frame, None)
        }
    }

    const HZ60: Duration = Duration::from_micros(16_667);

    fn frame(demand: Duration, target: Duration, done: Duration) -> Presented {
        Presented {
            demand,
            target,
            done,
            interval: HZ60,
            cpu_us: 4_000,
            queue_us: 100,
            gpu_us: None,
            input: None,
        }
    }

    #[test]
    fn a_frame_done_after_its_refresh_shows_on_a_later_one() {
        let target = Duration::from_millis(100);
        assert_eq!(present_time(target, target - HZ60, HZ60), (target, false));
        assert_eq!(
            present_time(target, target + Duration::from_millis(1), HZ60),
            (target + HZ60, true)
        );
        assert_eq!(
            present_time(target, target + HZ60 + Duration::from_millis(1), HZ60),
            (target + HZ60 * 2, true)
        );
    }

    #[test]
    fn continuous_frames_give_frame_times_and_skipped_refreshes_count_as_dropped() {
        let mut stats = FrameStats::default();
        let mut at = Duration::from_secs(1);
        for index in 0..30u32 {
            // Every tenth frame misses two refreshes.
            let step = if index % 10 == 9 { HZ60 * 3 } else { HZ60 };
            let demand = at;
            at += step;
            stats.record_test(frame(demand, at, at - Duration::from_millis(2)));
        }
        let summary = stats.summary(at, Duration::from_secs(5));
        assert_eq!(summary["frames"], 30);
        assert_eq!(summary["samples"], 29);
        assert_eq!(summary["frameTimeMs"]["p50"], 16.67);
        assert_eq!(summary["frameTimeMs"]["max"], 50.0);
        assert_eq!(
            summary["dropped"], 6,
            "three slow frames, two refreshes each"
        );
        assert_eq!(summary["jank"], 3);
        assert_eq!(summary["late"], 0);
    }

    #[test]
    fn a_frame_after_idle_has_latency_but_no_frame_time() {
        let mut stats = FrameStats::default();
        let first = Duration::from_secs(1);
        stats.record_test(frame(first - HZ60, first, first));
        let later = first + Duration::from_secs(2);
        stats.input(later - Duration::from_millis(30));
        let input = stats.take_input(later - Duration::from_millis(20));
        let mut next = frame(later - Duration::from_millis(20), later, later);
        next.input = input;
        stats.record_test(next);
        let summary = stats.summary(later, Duration::from_secs(5));
        assert_eq!(summary["samples"], 0);
        assert_eq!(summary["frameTimeMs"], Value::Null);
        assert_eq!(summary["dropped"], 0);
        assert_eq!(summary["inputToPresent"]["lastMs"], 30.0);
        assert_eq!(summary["inputToPresent"]["count"], 1);
    }

    #[test]
    fn a_stale_input_is_never_charged_to_a_later_frame() {
        let mut stats = FrameStats::default();
        stats.input(Duration::from_secs(1));
        assert_eq!(stats.take_input(Duration::from_secs(2)), None);
        stats.input(Duration::from_secs(3));
        stats.input(Duration::from_millis(2_990));
        assert_eq!(
            stats.take_input(Duration::from_secs(3)),
            Some(Duration::from_millis(2_990)),
            "the earliest input since the last frame"
        );
        assert_eq!(stats.take_input(Duration::from_secs(3)), None);
    }

    #[test]
    fn the_window_keeps_only_recent_frames_and_a_reveal_reports_its_first_present() {
        let mut stats = FrameStats::default();
        stats.record_test(frame(Duration::ZERO, HZ60, HZ60));
        stats.reveal_started(Duration::from_secs(10));
        let shown = Duration::from_secs(10) + Duration::from_millis(48);
        stats.record_test(frame(Duration::from_secs(10), shown, shown));
        let summary = stats.summary(shown, Duration::from_secs(1));
        assert_eq!(summary["frames"], 1);
        assert_eq!(summary["firstPresentMs"], 48.0);
        assert_eq!(summary["firstPresentReason"], "frame");
        assert_eq!(summary["reveals"], 1);
        let back = shown + Duration::from_millis(500);
        stats.reveal_started(back);
        stats.reveal_started(back + Duration::from_millis(5));
        stats.reveal_current(back);
        let summary = stats.summary(back, Duration::from_secs(1));
        assert_eq!(
            summary["reveals"], 2,
            "one show reported twice is one reveal"
        );
        assert_eq!(summary["firstPresentMs"], 0.0);
        assert_eq!(summary["revealMs"]["max"], 48.0);
        assert_eq!(summary["revealMs"]["p50"], 0.0);
    }

    #[test]
    fn the_hud_shows_milliseconds_and_the_newest_frame_times() {
        let mut stats = FrameStats::default();
        let mut at = Duration::from_secs(1);
        for _ in 0..100 {
            let demand = at;
            at += HZ60;
            stats.record_test(frame(demand, at, at));
        }
        let hud = stats.hud(at);
        assert_eq!(hud.spark.len(), SPARK);
        assert_eq!(hud.lines[0], "frame 16.7 ms  60 fps");
        assert_eq!(hud.lines[2], "dropped 0  jank 0");
    }
}
