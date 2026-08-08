use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActivityTarget {
    Display(usize),
    Window(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Transition {
    Ignored,
    Nothing,
    Open { generation: u64, epoch: u64 },
    Repoint { epoch: u64 },
}

/// Pure viewer lifecycle state. Platform code owns windows and capture; this
/// owns the ordering rules that keep a delayed UI update from undoing dismiss.
pub(crate) struct ActivityState {
    pub(crate) target: Option<ActivityTarget>,
    pub(crate) marker: Option<(i32, i32, Instant)>,
    pub(crate) last_activity: Option<Instant>,
    pub(crate) dismissed: bool,
    pub(crate) generation: u64,
    pub(crate) epoch: u64,
}

impl ActivityState {
    pub(crate) const fn new() -> Self {
        Self {
            target: None,
            marker: None,
            last_activity: None,
            dismissed: false,
            generation: 0,
            epoch: 0,
        }
    }

    pub(crate) fn note(
        &mut self,
        wanted: Option<ActivityTarget>,
        point: Option<(i32, i32)>,
        now: Instant,
        idle_rest: Duration,
    ) -> Transition {
        // Callers can prepare activity on different threads before serializing
        // it here. A delayed event must not move the clock, marker, or target
        // backwards.
        if self.last_activity.is_some_and(|previous| now < previous) {
            return if self.dismissed {
                Transition::Ignored
            } else {
                Transition::Nothing
            };
        }
        let starts_new_run = self
            .last_activity
            .is_none_or(|previous| now.saturating_duration_since(previous) > idle_rest);
        self.last_activity = Some(now);

        if self.dismissed {
            if !starts_new_run {
                return Transition::Ignored;
            }
            self.dismissed = false;
        }
        if let Some((x, y)) = point {
            self.marker = Some((x, y, now));
        }

        match (&self.target, wanted) {
            (None, wanted) => {
                self.target = Some(wanted.unwrap_or(ActivityTarget::Display(1)));
                self.generation = self.generation.wrapping_add(1);
                self.epoch = self.epoch.wrapping_add(1);
                Transition::Open {
                    generation: self.generation,
                    epoch: self.epoch,
                }
            }
            (Some(current), Some(wanted)) if current != &wanted => {
                self.target = Some(wanted);
                self.epoch = self.epoch.wrapping_add(1);
                Transition::Repoint { epoch: self.epoch }
            }
            _ => Transition::Nothing,
        }
    }

    pub(crate) fn dismiss(&mut self) -> u64 {
        self.dismissed = true;
        self.clear_visible_state()
    }

    pub(crate) fn rest(&mut self, generation: u64, epoch: u64) -> Option<u64> {
        if !self.current(generation, epoch) {
            return None;
        }
        Some(self.clear_visible_state())
    }

    pub(crate) fn current(&self, generation: u64, epoch: u64) -> bool {
        !self.dismissed
            && self.target.is_some()
            && self.generation == generation
            && self.epoch == epoch
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn active_generation(&self, generation: u64) -> bool {
        !self.dismissed && self.target.is_some() && self.generation == generation
    }

    fn clear_visible_state(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.epoch = self.epoch.wrapping_add(1);
        self.target = None;
        self.marker = None;
        self.epoch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDLE: Duration = Duration::from_secs(12);

    #[test]
    fn dismiss_invalidates_a_queued_open() {
        let mut state = ActivityState::new();
        let now = Instant::now();
        let Transition::Open { generation, epoch } =
            state.note(Some(ActivityTarget::Display(1)), None, now, IDLE)
        else {
            panic!("first activity must open");
        };

        state.dismiss();
        assert!(!state.current(generation, epoch));
        assert_eq!(
            state.note(None, None, now + Duration::from_secs(1), IDLE),
            Transition::Ignored
        );
    }

    #[test]
    fn activity_after_a_dismissed_idle_gap_starts_a_new_run() {
        let mut state = ActivityState::new();
        let now = Instant::now();
        let _ = state.note(None, None, now, IDLE);
        state.dismiss();

        let transition = state.note(None, None, now + IDLE + Duration::from_millis(1), IDLE);
        assert!(matches!(transition, Transition::Open { .. }));
        assert!(!state.dismissed);
    }

    #[test]
    fn stale_idle_worker_cannot_hide_a_new_viewer() {
        let mut state = ActivityState::new();
        let now = Instant::now();
        let Transition::Open {
            generation: old, ..
        } = state.note(None, None, now, IDLE)
        else {
            panic!("first activity must open");
        };
        let old_epoch = state.epoch;
        state.rest(old, old_epoch).expect("old viewer may rest");
        let Transition::Open {
            generation: new, ..
        } = state.note(None, None, now + IDLE, IDLE)
        else {
            panic!("later activity must reopen");
        };

        assert_ne!(old, new);
        assert!(state.rest(old, old_epoch).is_none());
        assert!(state.target.is_some());
    }

    #[test]
    fn target_change_gets_an_epoch_without_restarting_refresh() {
        let mut state = ActivityState::new();
        let now = Instant::now();
        let Transition::Open { generation, epoch } =
            state.note(Some(ActivityTarget::Display(1)), None, now, IDLE)
        else {
            panic!("first activity must open");
        };
        let Transition::Repoint { epoch: next_epoch } = state.note(
            Some(ActivityTarget::Window("0x42".into())),
            None,
            now + Duration::from_secs(1),
            IDLE,
        ) else {
            panic!("changed target must repoint");
        };

        assert_eq!(state.generation, generation);
        assert_ne!(epoch, next_epoch);
        assert_eq!(state.target, Some(ActivityTarget::Window("0x42".into())));
        assert!(state.rest(generation, epoch).is_none());
        assert!(state.target.is_some());
    }

    #[test]
    fn out_of_order_activity_never_moves_the_idle_clock_backwards() {
        let mut state = ActivityState::new();
        let start = Instant::now();
        let latest = start + Duration::from_secs(8);
        let latest_target = ActivityTarget::Window("0x2".into());
        let _ = state.note(Some(latest_target.clone()), Some((20, 30)), latest, IDLE);
        let _ = state.note(
            Some(ActivityTarget::Window("0x1".into())),
            Some((1, 2)),
            start,
            IDLE,
        );

        assert_eq!(state.last_activity, Some(latest));
        assert_eq!(state.target, Some(latest_target));
        assert_eq!(state.marker, Some((20, 30, latest)));
    }
}
