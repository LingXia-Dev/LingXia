//! When a page being shown replaces the one on screen. The outgoing page stays,
//! frozen, until the incoming one presents, fails or runs out of time; adapters
//! only execute [`crate::platform::HostEvents::revealed`].
use super::{Guard, Inner};
use crate::platform::PlatformAdapter;
use std::sync::Arc;
use std::time::Duration;

/// A returning page whose surface still holds a frame is shown stale after this,
/// never left under the previous page.
pub const REVEAL_DEADLINE: Duration = Duration::from_millis(300);
/// A first visit waits for its first frame; this only catches one that never comes.
pub const FIRST_FRAME_WATCHDOG: Duration = Duration::from_secs(2);

#[derive(Debug, Default)]
pub(super) struct Reveal {
    /// The deadline of the pending reveal, if one waits.
    pending: Option<Duration>,
    /// Bumped to revoke a pending deadline.
    epoch: u64,
    ever_presented: bool,
    /// Revealed without a current frame; showing the surface waits for one.
    holding: bool,
    window_hidden: bool,
    deadlines: u64,
    watchdogs: u64,
    /// Why and when (since the platform clock's origin) the page was last revealed.
    last: Option<(&'static str, Duration)>,
}

impl Reveal {
    /// `{deadlines, watchdogs, lastReveal: {reason, atMs}}`, times since `opened`.
    pub(super) fn metrics(&self, opened: Duration) -> serde_json::Value {
        serde_json::json!({
            "deadlines": self.deadlines,
            "watchdogs": self.watchdogs,
            "lastReveal": self.last.map(|(reason, at)| serde_json::json!({
                "reason": reason,
                "atMs": at.saturating_sub(opened).as_millis() as u64,
            })),
        })
    }
}

impl<A: PlatformAdapter> Inner<A> {
    pub(super) fn begin_reveal(self: &Arc<Self>, surface_holds_frame: bool) {
        let at = self.adapter.now();
        self.main().telemetry.stats.reveal_started(at);
        if self.frame_is_current() {
            {
                let mut main = self.main();
                main.reveal.pending = None;
                main.telemetry.stats.reveal_current(at);
            }
            return self.finish_reveal(false, "current");
        }
        let (epoch, delay) = {
            let mut main = self.main();
            let reveal = &mut main.reveal;
            // Only a surface that still holds this page's frame can be shown stale.
            let delay = if reveal.ever_presented && surface_holds_frame {
                REVEAL_DEADLINE
            } else {
                FIRST_FRAME_WATCHDOG
            };
            reveal.epoch += 1;
            reveal.pending = Some(delay);
            (reveal.epoch, delay)
        };
        self.reveal_after(epoch, delay);
    }

    fn reveal_after(self: &Arc<Self>, epoch: u64, delay: Duration) {
        let host = self.clone();
        self.adapter
            .post_after(delay, Box::new(move || host.reveal_deadline(epoch)));
    }

    fn reveal_deadline(self: &Arc<Self>, epoch: u64) {
        let (delay, hidden) = {
            let main = self.main();
            let Some(delay) = main.reveal.pending.filter(|_| main.reveal.epoch == epoch) else {
                return;
            };
            (delay, main.reveal.window_hidden)
        };
        // A stopped window renders nothing; the wait resumes with it.
        if hidden {
            return self.reveal_after(epoch, delay);
        }
        let hold = !self.frame_is_current();
        let deadline = delay == REVEAL_DEADLINE;
        {
            let mut main = self.main();
            main.reveal.pending = None;
            if deadline {
                main.reveal.deadlines += 1;
            } else {
                main.reveal.watchdogs += 1;
            }
        }
        self.adapter.warn(&if deadline {
            format!("reveal deadline: shown before a current frame (stale={hold})")
        } else {
            "first-frame watchdog: page never presented".to_owned()
        });
        self.finish_reveal(hold, if deadline { "deadline" } else { "watchdog" });
    }

    fn finish_reveal(self: &Arc<Self>, hold: bool, reason: &'static str) {
        let at = self.adapter.now();
        {
            let mut main = self.main();
            main.reveal.holding = hold;
            main.reveal.last = Some((reason, at));
        }
        self.adapter.revealed(hold);
    }

    /// A window nothing of which can be seen completes frames without presenting them (a
    /// drawable for it may never come back) and redraws when it shows again. Finite
    /// animations run to completion meanwhile; looping keyframe ones park.
    pub(super) fn window_visible(self: &Arc<Self>, visible: bool) {
        let changed = {
            let mut main = self.main();
            let changed = main.reveal.window_hidden == visible;
            main.reveal.window_hidden = !visible;
            changed
        };
        if !changed {
            return;
        }
        self.work(
            Guard::Page(self.lifecycle.page()),
            move |_, session| {
                session.set_animation_covered(!visible);
                Ok(())
            },
            |_, _| {},
        );
        if visible {
            // Frames completed while hidden never reached the screen.
            self.main().dirty = true;
            self.request_frame();
            self.schedule();
        }
    }

    pub(super) fn window_hidden(&self) -> bool {
        self.main().reveal.window_hidden
    }

    /// A frame reached the screen: a waiting or held reveal completes.
    pub(super) fn revealed_by_frame(self: &Arc<Self>) {
        let waiting = {
            let mut main = self.main();
            let reveal = &mut main.reveal;
            reveal.ever_presented = true;
            let waiting = reveal.pending.take().is_some() || reveal.holding;
            reveal.holding = false;
            waiting
        };
        if waiting {
            self.finish_reveal(false, "frame");
        }
    }

    /// Recovery chrome is shown at once.
    pub(super) fn revealed_by_failure(self: &Arc<Self>) {
        if self.main().reveal.pending.take().is_some() {
            self.finish_reveal(false, "failure");
        }
    }
}
