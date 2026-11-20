//! Pull to refresh on a GPU page, measured as on macOS: a downward scroll with the page at its
//! top pulls instead of scrolling, along the rubber band iOS and Android use, and a release
//! past the trigger refreshes. The host draws the indicator and runs the refresh.

use crate::Page;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};

/// A step of a pull, for the host.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pull {
    /// Travel in points after the rubber band, and whether it is past the trigger.
    Moved { travel: f32, armed: bool },
    /// Dropped short of the trigger, or taken away.
    Cancelled,
    /// Let go past the trigger: the page refreshes.
    Released,
}

/// Measured after the rubber band, so the distance means what it does on the other platforms.
const TRIGGER: f32 = 80.0;
const MAX_PULL: f32 = 150.0;
/// A wheel has no release: its pull ends once the notches stop.
const WHEEL_IDLE: Duration = Duration::from_millis(200);
/// Windows' default three lines a notch, of macOS's 16-point line.
const NOTCH_POINTS: f32 = 48.0;
pub(crate) const TIMER_ID: usize = 0x4c58_5052;

#[derive(Default)]
pub(crate) struct PullState {
    pulling: bool,
    /// Points the fingers or wheel travelled down since the pull began.
    pull: f32,
    /// Asked of the host when a gesture begins: a check per event would cross into the app.
    enabled: bool,
    last_wheel: Option<Instant>,
}

/// Where a phased scroll is in its gesture.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Phase {
    Began,
    Changed,
    Ended,
    Momentum,
}

fn rubber_band(distance: f32) -> f32 {
    if distance <= 0.0 {
        return 0.0;
    }
    let x = distance / MAX_PULL;
    MAX_PULL * (1.0 - (-0.55 * x).exp()) / (1.0 - (-0.55f32).exp())
}

fn report(page: &Page, pull: Pull) {
    let adapter = page.host().adapter();
    adapter.shell.pull_refresh(adapter.token, pull);
}

fn can_pull(page: &Page) -> bool {
    let adapter = page.host().adapter();
    adapter.shell.can_pull_refresh(adapter.token)
}

fn at_top(page: &Page, at: [f32; 2]) -> bool {
    !page.host().can_scroll_vertically_at(at, -1)
}

/// Adds `delta` points to the pull under way; the step to report, and whether it is still on.
fn accumulate(state: &mut PullState, delta: f32) -> Pull {
    state.pull += delta;
    if state.pull <= 0.0 {
        // Pushed back past the top: the scroll is the page's again.
        state.pulling = false;
        state.pull = 0.0;
        return Pull::Cancelled;
    }
    let travel = rubber_band(state.pull);
    Pull::Moved {
        travel,
        armed: travel >= TRIGGER,
    }
}

/// A phased scroll whose fingers travelled `delta` points down; true when the pull took the
/// movement. Boundary events still go on to the page, so it sees whole gestures.
pub(crate) fn scroll(page: &Page, phase: Phase, at: [f32; 2], delta: f32) -> bool {
    let step = {
        let mut input = page.input.borrow_mut();
        let state = &mut input.pull;
        match phase {
            // What the page coasts on after the fingers lift never pulls.
            Phase::Momentum => return state.pulling,
            Phase::Ended => {
                if !std::mem::take(&mut state.pulling) {
                    return false;
                }
                let armed = rubber_band(std::mem::take(&mut state.pull)) >= TRIGGER;
                Some(if armed {
                    Pull::Released
                } else {
                    Pull::Cancelled
                })
            }
            Phase::Began | Phase::Changed => {
                if phase == Phase::Began {
                    state.pulling = false;
                    state.pull = 0.0;
                    drop(input);
                    let enabled = can_pull(page);
                    input = page.input.borrow_mut();
                    input.pull.enabled = enabled;
                }
                None
            }
        }
    };
    if let Some(step) = step {
        report(page, step);
        return false;
    }
    if delta == 0.0 {
        return false;
    }
    let (enabled, pulling) = {
        let input = page.input.borrow();
        (input.pull.enabled, input.pull.pulling)
    };
    if !enabled || (!pulling && (delta <= 0.0 || !at_top(page, at))) {
        return false;
    }
    let step = {
        let mut input = page.input.borrow_mut();
        input.pull.pulling = true;
        accumulate(&mut input.pull, delta)
    };
    report(page, step);
    step != Pull::Cancelled
}

/// A wheel turned `delta` points toward pulling; true when the pull took it.
pub(crate) fn wheel(page: &Page, hwnd: HWND, at: [f32; 2], notches: f32) -> bool {
    let delta = notches * NOTCH_POINTS;
    let now = Instant::now();
    let pulling = {
        let mut input = page.input.borrow_mut();
        let state = &mut input.pull;
        // Only a fresh burst pulls: the notches that carried the page to its top, and their
        // overshoot once there, belong to that scroll.
        let continuing = state
            .last_wheel
            .is_some_and(|last| now.duration_since(last) < WHEEL_IDLE);
        state.last_wheel = Some(now);
        if !state.pulling && continuing {
            return false;
        }
        state.pulling
    };
    if !pulling && (delta <= 0.0 || !can_pull(page) || !at_top(page, at)) {
        return false;
    }
    let step = {
        let mut input = page.input.borrow_mut();
        input.pull.pulling = true;
        match accumulate(&mut input.pull, delta) {
            // Crossing the trigger is the commitment: there is no release to wait for.
            Pull::Moved { armed: true, .. } => {
                input.pull.pulling = false;
                input.pull.pull = 0.0;
                Pull::Released
            }
            step => step,
        }
    };
    report(page, step);
    if matches!(step, Pull::Moved { .. }) {
        // SAFETY: a timer on this thread's window; it replaces the previous one.
        unsafe { SetTimer(Some(hwnd), TIMER_ID, WHEEL_IDLE.as_millis() as u32, None) };
    }
    step != Pull::Cancelled
}

/// No notch since the timer was armed: the wheel stopped short of the trigger.
pub(crate) fn wheel_idle(page: &Page, hwnd: HWND) {
    // SAFETY: this thread's timer.
    let _ = unsafe { KillTimer(Some(hwnd), TIMER_ID) };
    let pulling = {
        let mut input = page.input.borrow_mut();
        input.pull.pull = 0.0;
        std::mem::take(&mut input.pull.pulling)
    };
    if pulling {
        report(page, Pull::Cancelled);
    }
}
