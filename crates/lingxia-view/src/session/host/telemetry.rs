//! Per-page presentation counters for the automation `frame` request.
use super::{FrameSnapshot, FrameTiming, Inner};
use crate::platform::PlatformAdapter;
use std::collections::VecDeque;
use std::time::Duration;

/// Worker-queue samples kept for the median.
const SAMPLES: usize = 64;

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
    pub(super) stats: super::frame_stats::FrameStats,
}

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
    }

    pub(super) fn woke(&mut self) {
        self.wakeups += 1;
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
        // The presenter's own counters ride in the presented frame's metrics.
        let presented = main
            .snapshot
            .as_ref()
            .and_then(|snapshot| serde_json::from_str::<serde_json::Value>(&snapshot.json).ok())
            .and_then(|frame| frame.get("metrics").cloned());
        let presenter = |key: &str| {
            presented
                .as_ref()
                .and_then(|metrics| metrics.get(key).cloned())
        };
        serde_json::json!({
            "composite": presenter("composite"),
            "backdropCache": presenter("backdropCache"),
            "renderer": "gpu",
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
            "hud": main.hud,
            // Nodes the presented frame laid out: a virtual list mounts only rows near the viewport.
            "nodes": main.snapshot.as_ref().map(|snapshot| snapshot.metrics.nodes),
            "pacing": telemetry
                .stats
                .summary(now, window.unwrap_or(super::frame_stats::DEFAULT_WINDOW)),
        })
    }
}
