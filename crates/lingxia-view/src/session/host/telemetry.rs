//! Per-page presentation counters for the automation `frame` request.
use super::{FrameSnapshot, FrameTiming, Inner};
use crate::platform::PlatformAdapter;
use std::collections::VecDeque;
use std::time::Duration;

/// Worker-queue samples kept for the median.
const SAMPLES: usize = 64;
/// Per-stage samples of presented frames, enough for 5 s at 60 Hz.
const STAGE_SAMPLES: usize = 512;

#[derive(Debug, Default)]
pub(super) struct Telemetry {
    opened: Duration,
    first_present: Option<Duration>,
    last_present: Option<Duration>,
    frames: u64,
    commit: u64,
    queue: VecDeque<u64>,
    queue_max: u64,
    eval_last_us: u64,
    eval_max_us: u64,
    animation_frames: usize,
    wakeups: u64,
    /// Frames dropped for finishing past the View budget since the module loaded.
    overruns: u64,
    /// When each presented frame showed and what its preparation cost, by stage.
    stages: VecDeque<(Duration, Stages)>,
    pub(super) stats: super::frame_stats::FrameStats,
    transition: Option<Transition>,
}

/// The platform's push or pop slide of this page, reported frame by frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransitionEvent {
    Start {
        pop: bool,
    },
    /// A rendered frame with how far the slide has moved the page, in any unit.
    Frame {
        at: f32,
    },
    End,
}

#[derive(Debug)]
struct Transition {
    pop: bool,
    started: Duration,
    ended: Option<Duration>,
    frames: u32,
    moving: u32,
    first_move: Option<Duration>,
    last: Option<f32>,
}

/// Total, view, view eval, layout, text, compose and build µs; view and layout passes; then the
/// frame's queue, worker, vsync wait and main-thread µs; then an off-worker draw's scene, encode,
/// submit and present µs; then eval phases (render, resolve, recall, rows, encode, vnodes) when
/// the page opted into them.
type Stages = [u64; 24];

impl Telemetry {
    pub(super) fn opened(&mut self, at: Duration) {
        // Frame statistics belong to the surface: a reveal may start before the module loads.
        let stats = std::mem::take(&mut self.stats);
        *self = Self {
            opened: at,
            stats,
            ..Self::default()
        };
    }

    pub(super) fn presented(
        &mut self,
        at: Duration,
        snapshot: &FrameSnapshot,
        timing: &FrameTiming,
    ) {
        self.first_present.get_or_insert(at);
        self.last_present = Some(at);
        self.frames += 1;
        self.commit = snapshot.generation.get();
        if self.queue.len() == SAMPLES {
            self.queue.pop_front();
        }
        self.queue.push_back(timing.queue_us);
        self.queue_max = self.queue_max.max(timing.queue_us);
        if timing.rebuild {
            self.eval_last_us = snapshot.metrics.view_eval_us;
            self.eval_max_us = self.eval_max_us.max(self.eval_last_us);
        }
        self.animation_frames = snapshot.metrics.animation_frames;
        if self.stages.len() == STAGE_SAMPLES {
            self.stages.pop_front();
        }
        let m = &snapshot.metrics;
        let p = m.view_phases.unwrap_or_default();
        self.stages.push_back((
            at,
            [
                m.total_us,
                m.view_us,
                m.view_eval_us,
                m.layout_us,
                m.text_us,
                m.compose_us,
                m.build_us,
                u64::from(m.view_passes),
                u64::from(m.layout_passes),
                timing.queue_us,
                timing.worker_us,
                timing.wait_us,
                timing.main_us,
                timing.gpu_us[0],
                timing.gpu_us[1],
                timing.gpu_us[2],
                timing.gpu_us[3],
                timing.main_work_us,
                p.render,
                p.resolve,
                p.recall,
                p.rows,
                p.encode + p.windows,
                p.h,
            ],
        ));
    }

    /// Median preparation cost by stage of the frames presented since `since`, or `null`.
    fn stage_medians(&self, since: Duration) -> serde_json::Value {
        let samples: Vec<&Stages> = self
            .stages
            .iter()
            .filter(|(at, _)| *at >= since)
            .map(|(_, stages)| stages)
            .collect();
        if samples.is_empty() {
            return serde_json::Value::Null;
        }
        let median = |i: usize| {
            let mut values: Vec<u64> = samples.iter().map(|stages| stages[i]).collect();
            values.sort_unstable();
            values[values.len() / 2]
        };
        let ms = |i: usize| median(i) as f64 / 1000.0;
        serde_json::json!({
            "frames": samples.len(),
            "total": ms(0),
            "view": ms(1),
            "viewEval": ms(2),
            "layout": ms(3),
            "text": ms(4),
            "compose": ms(5),
            "build": ms(6),
            "viewPasses": median(7),
            "layoutPasses": median(8),
            "queue": ms(9),
            "worker": ms(10),
            "wait": ms(11),
            "main": ms(12),
            "gpuScene": ms(13),
            "gpuEncode": ms(14),
            "gpuSubmit": ms(15),
            "gpuPresent": ms(16),
            "mainWork": ms(17),
            "viewPhases": self.phase_medians(&samples),
        })
    }

    /// Eval phase medians over the frames that ran the View, or `null` without phases.
    fn phase_medians(&self, samples: &[&Stages]) -> serde_json::Value {
        let evaluated: Vec<&Stages> = samples.iter().copied().filter(|s| s[2] > 0).collect();
        if evaluated.iter().all(|s| s[18..].iter().all(|v| *v == 0)) {
            return serde_json::Value::Null;
        }
        let median = |i: usize| {
            let mut values: Vec<u64> = evaluated.iter().map(|s| s[i]).collect();
            values.sort_unstable();
            values[values.len() / 2]
        };
        let ms = |i: usize| median(i) as f64 / 1000.0;
        serde_json::json!({
            "frames": evaluated.len(),
            "eval": ms(2),
            "render": ms(18),
            "resolve": ms(19),
            "recall": ms(20),
            "rows": ms(21),
            "encode": ms(22),
            "vnodes": median(23),
        })
    }

    pub(super) fn budget_overrun(&mut self) {
        self.overruns += 1;
    }

    pub(super) fn woke(&mut self) {
        self.wakeups += 1;
    }

    pub(super) fn transition(&mut self, now: Duration, event: TransitionEvent) {
        match event {
            TransitionEvent::Start { pop } => {
                self.transition = Some(Transition {
                    pop,
                    started: now,
                    ended: None,
                    frames: 0,
                    moving: 0,
                    first_move: None,
                    last: None,
                });
            }
            TransitionEvent::Frame { at } => {
                let Some(slide) = self
                    .transition
                    .as_mut()
                    .filter(|slide| slide.ended.is_none())
                else {
                    return;
                };
                slide.frames += 1;
                // Below half a unit is float noise in the platform's readout.
                if slide.last.is_some_and(|last| (at - last).abs() >= 0.5) {
                    slide.moving += 1;
                    slide.first_move.get_or_insert(now);
                }
                slide.last = Some(at);
            }
            TransitionEvent::End => {
                if let Some(slide) = self.transition.as_mut() {
                    slide.ended.get_or_insert(now);
                }
            }
        }
    }

    fn transition_metrics(&self) -> serde_json::Value {
        let Some(slide) = &self.transition else {
            return serde_json::Value::Null;
        };
        let ms = |span: Duration| span.as_secs_f64() * 1000.0;
        serde_json::json!({
            "kind": if slide.pop { "pop" } else { "push" },
            "durationMs": slide.ended.map(|ended| ms(ended.saturating_sub(slide.started))),
            "frames": slide.frames,
            "movingFrames": slide.moving,
            "firstMoveMs": slide.first_move.map(|first| ms(first.saturating_sub(slide.started))),
            "atMs": self.since_open(Some(slide.started)),
        })
    }

    fn since_open(&self, at: Option<Duration>) -> serde_json::Value {
        at.map_or(serde_json::Value::Null, |at| {
            (at.saturating_sub(self.opened).as_millis() as u64).into()
        })
    }
}

impl<A: PlatformAdapter> Inner<A> {
    /// `window`: how far back `pacing` reaches; frame stats default to 5 s.
    pub(super) fn frame_metrics(&self, window: Option<Duration>) -> serde_json::Value {
        let now = self.adapter.now();
        let main = self.main();
        let telemetry = &main.telemetry;
        let mut sorted: Vec<u64> = telemetry.queue.iter().copied().collect();
        sorted.sort_unstable();
        let p50 = sorted.get(sorted.len() / 2).copied().unwrap_or(0);
        let (bytes, entries) = super::decode::retained();
        // The presenter's own counters ride in the presented frame's metrics, or arrive with
        // the draw when the frame was drawn off the worker.
        let presented = main.drawn.clone().or_else(|| {
            main.snapshot
                .as_ref()
                .map(|snapshot| snapshot.reported.clone())
        });
        let presenter = |key: &str| {
            presented
                .as_ref()
                .and_then(|metrics| metrics.get(key).cloned())
        };
        serde_json::json!({
            "composite": presenter("composite"),
            "backdropCache": presenter("backdropCache"),
            "passes": presenter("passes"),
            "presentUs": presenter("acquireEncodeSubmitPresentUs"),
            "video": crate::renderer::video_stats(),
            "renderer": "gpu",
            // Frame budgets hold for an optimized core only.
            "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "page": self.lifecycle.page().get().to_string(),
            "commit": telemetry.commit,
            "firstPresentAt": telemetry.since_open(telemetry.first_present),
            "lastPresentAt": telemetry.since_open(telemetry.last_present),
            "frames": telemetry.frames,
            "workerQueueUs": {
                "last": telemetry.queue.back().copied().unwrap_or(0),
                "p50": p50,
                "max": telemetry.queue_max,
            },
            "evalMs": {
                "last": telemetry.eval_last_us as f64 / 1000.0,
                "max": telemetry.eval_max_us as f64 / 1000.0,
            },
            "imageCacheBytes": bytes,
            "imageCacheEntries": entries,
            "animationFrames": telemetry.animation_frames,
            "wakeups": telemetry.wakeups,
            "reveal": main.reveal.metrics(telemetry.opened),
            "transition": telemetry.transition_metrics(),
            "budget": { "overruns": telemetry.overruns },
            "hud": main.hud,
            // What the page renders for (`env.pointer`), so specs can require one.
            "pointer": main
                .presented_environment
                .as_ref()
                .and_then(|environment| environment.value["pointer"].as_str())
                .map(str::to_owned),
            // The bar-snapshot generation the presented frame draws under its top layer; 0 when none.
            "chromeSnapshot": main.snapshot.as_ref().map_or(0, |snapshot| snapshot.metrics.chrome_snapshot),
            // Nodes the presented frame laid out: a virtual list mounts only rows near the viewport.
            "nodes": main.snapshot.as_ref().map(|snapshot| snapshot.metrics.nodes),
            "pacing": telemetry
                .stats
                .summary(now, window.unwrap_or(super::frame_stats::DEFAULT_WINDOW)),
            "stagesMs": telemetry.stage_medians(
                now.saturating_sub(window.unwrap_or(super::frame_stats::DEFAULT_WINDOW)),
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Telemetry, TransitionEvent};
    use std::time::Duration;

    #[test]
    fn a_slide_counts_frames_that_moved_the_page_on_the_host_clock() {
        let ms = Duration::from_millis;
        let mut telemetry = Telemetry::default();
        telemetry.opened(ms(1000));
        assert!(telemetry.transition_metrics().is_null());

        telemetry.transition(ms(1100), TransitionEvent::Start { pop: false });
        telemetry.transition(ms(1116), TransitionEvent::Frame { at: 400.0 });
        for (step, at) in [(2, 300.0), (3, 150.0), (4, 0.0)] {
            telemetry.transition(ms(1100 + step * 16), TransitionEvent::Frame { at });
        }
        telemetry.transition(ms(1180), TransitionEvent::Frame { at: 0.0 });
        telemetry.transition(ms(1400), TransitionEvent::End);
        telemetry.transition(ms(1416), TransitionEvent::Frame { at: 10.0 });

        let slide = telemetry.transition_metrics();
        assert_eq!(slide["kind"], "push");
        assert_eq!(slide["frames"], 5);
        assert_eq!(slide["movingFrames"], 3);
        assert_eq!(slide["firstMoveMs"], 32.0);
        assert_eq!(slide["durationMs"], 300.0);
        assert_eq!(slide["atMs"], 100);

        telemetry.opened(ms(2000));
        assert!(telemetry.transition_metrics().is_null());
    }
}
