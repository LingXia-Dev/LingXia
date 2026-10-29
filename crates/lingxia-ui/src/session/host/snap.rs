//! `scroll-snap-type` regions settle on a snap point once input stops: after a finger, a wheel
//! or a platform fling goes quiet, the region eases to its nearest point. A new movement cancels it.
use super::Inner;
use crate::platform::PlatformAdapter;
use std::sync::Arc;
use std::time::Duration;

/// Quiet time after the last movement before a region settles; covers a fling's last steps.
const IDLE: Duration = Duration::from_millis(120);
const STEP: Duration = Duration::from_millis(16);
const SETTLE_MS: f32 = 200.0;

#[derive(Default)]
pub(super) struct Snaps {
    /// Bumped by every movement; a settle or its animation stops once it no longer matches.
    serial: u64,
    pending: std::collections::BTreeSet<String>,
}

impl<A: PlatformAdapter> Inner<A> {
    /// After input moved regions: settle the snapping ones once it goes quiet.
    pub(super) fn snap_later(self: &Arc<Self>) {
        let serial = {
            let mut main = self.main();
            let moved = main.interaction.take_moved_snaps();
            if moved.is_empty() {
                return;
            }
            main.snaps.pending.extend(moved);
            main.snaps.serial += 1;
            main.snaps.serial
        };
        let inner = self.clone();
        self.adapter
            .post_after(IDLE, Box::new(move || inner.settle_snaps(serial)));
    }

    fn settle_snaps(self: &Arc<Self>, serial: u64) {
        let reduce = self.adapter.environment().is_some_and(|env| {
            env.value["reducedMotion"] == true || env.value["animatorDurationScale"] == 0.0
        });
        let moves = {
            let mut main = self.main();
            if main.snaps.serial != serial {
                return;
            }
            // A finger still down holds the region where it is.
            if main.interaction.holding_scroll() {
                drop(main);
                let inner = self.clone();
                self.adapter
                    .post_after(IDLE, Box::new(move || inner.settle_snaps(serial)));
                return;
            }
            let pending = std::mem::take(&mut main.snaps.pending);
            pending
                .into_iter()
                .filter_map(|id| {
                    let to = main.interaction.scroll().snap_target(&id)?;
                    Some((id.clone(), main.interaction.scroll().offset(&id), to))
                })
                .collect::<Vec<_>>()
        };
        if !moves.is_empty() {
            self.snap_step(serial, moves, if reduce { SETTLE_MS } else { 0.0 });
        }
    }

    fn snap_step(self: &Arc<Self>, serial: u64, moves: Vec<(String, f32, f32)>, elapsed: f32) {
        let done = elapsed >= SETTLE_MS;
        {
            let mut main = self.main();
            if main.snaps.serial != serial {
                return;
            }
            // Ease out: fast first, landing gently.
            let t = (elapsed / SETTLE_MS).min(1.0);
            let eased = 1.0 - (1.0 - t).powi(3);
            for (id, from, to) in &moves {
                main.interaction
                    .set_scroll_offset(id, from + (to - from) * eased);
            }
        }
        self.deliver(vec![crate::interaction::view::Effect::Redraw]);
        if !done {
            let inner = self.clone();
            self.adapter.post_after(
                STEP,
                Box::new(move || inner.snap_step(serial, moves, elapsed + STEP.as_millis() as f32)),
            );
        }
    }
}
