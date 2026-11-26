//! `useNow`: one timer to the next step boundary of the finest granularity the page reads,
//! armed only while the page is shown and its clock is not pinned.
use super::Inner;
use crate::platform::PlatformAdapter;
use std::sync::Arc;
use std::time::Duration;

/// Past the boundary, so a timer that fires a little early still lands on the new step.
const SLACK_MS: u64 = 2;

#[derive(Default)]
pub(super) struct Clock {
    /// The finest step the View's last run read, in ms; 0 when none.
    pub(super) step: u64,
    /// The clock a test pinned or `still` froze, epoch ms.
    pub(super) pin: Option<u64>,
    /// Bumped by every arm or stop; a timer that no longer matches does nothing.
    serial: u64,
    /// The boundary the armed timer waits for, epoch ms.
    due: Option<u64>,
    running: bool,
}

impl Clock {
    /// Stops for good: the armed timer, if any, finds its serial gone and does nothing.
    pub(super) fn stop(&mut self) {
        self.running = false;
        self.due = None;
        self.serial += 1;
    }
}

pub(super) fn wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64)
}

impl<A: PlatformAdapter> Inner<A> {
    /// Arms, keeps or stops the tick for the page as it now stands. Coming back on screen
    /// re-delivers the time at once and counts the boundary afresh.
    pub(super) fn sync_clock(self: &Arc<Self>) {
        let stopped = self.window_unwatched();
        let (serial, resumed, delay) = {
            let mut main = self.main();
            let main = &mut *main;
            let running = main.visible
                && !main.disposed
                && !stopped
                && !main.interaction.parked()
                && main.clock.step > 0
                && main.clock.pin.is_none();
            let resumed = running && !main.clock.running;
            main.clock.running = running;
            if !running {
                if main.clock.due.take().is_some() {
                    main.clock.serial += 1;
                }
                return;
            }
            let now = wall_ms();
            let step = main.clock.step;
            let due = (now / step + 1) * step;
            if main.clock.due == Some(due) && !resumed {
                return;
            }
            main.clock.serial += 1;
            main.clock.due = Some(due);
            (main.clock.serial, resumed, due - now)
        };
        if resumed {
            self.clock_tick();
        }
        let inner = self.clone();
        self.adapter.post_after(
            Duration::from_millis(delay + SLACK_MS),
            Box::new(move || inner.clock_fired(serial)),
        );
    }

    /// The boundary passed. Each tick reads the wall clock and counts the next boundary from
    /// it, so a clock set forward or back between ticks is caught here at the latest.
    fn clock_fired(self: &Arc<Self>, serial: u64) {
        {
            let mut main = self.main();
            if main.clock.serial != serial {
                return;
            }
            main.clock.due = None;
        }
        self.clock_tick();
        self.sync_clock();
    }

    /// The wall clock or time zone changed: the time goes to the page at once.
    pub(super) fn clock_changed(self: &Arc<Self>) {
        let running = {
            let mut main = self.main();
            main.clock.due = None;
            main.clock.running
        };
        if running {
            self.clock_tick();
        }
        self.sync_clock();
    }

    /// A test pinned the clock, froze it under `still`, or let it go.
    pub(super) fn pin_clock(self: &Arc<Self>, pin: Option<u64>) {
        self.main().clock.pin = pin;
        self.activate(
            self.lifecycle.page(),
            false,
            Box::new(move |session| {
                session.set_clock_pin(pin);
                session.clock_tick()
            }),
        );
        self.sync_clock();
    }

    fn clock_tick(self: &Arc<Self>) {
        self.activate(
            self.lifecycle.page(),
            false,
            Box::new(|session| session.clock_tick()),
        );
    }
}
