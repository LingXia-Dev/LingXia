use super::*;

/// Where a wheel or trackpad event sits in its gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WheelPhase {
    /// A mouse wheel notch: no gesture around it.
    Discrete,
    Began,
    Changed,
    Ended,
    /// The platform's inertia after the fingers lift.
    Momentum,
}

/// Device-pixel wheel input. `precise` deltas are pixels the content should move with
/// (positive toward the end of the axis, as AppKit reports); otherwise they count notches.
#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
    pub precise: bool,
    pub phase: WheelPhase,
}

/// Logical pixels one wheel notch scrolls when the platform gives no pixel distance.
const NOTCH: f32 = 3.0 * 16.0;

impl ViewInteraction {
    /// Whether the regions under a device-pixel point can scroll vertically toward
    /// `direction`: a platform pull-to-refresh over wheel input arms only when not.
    pub fn can_scroll_vertically_at(&self, x: f32, y: f32, direction: i32) -> bool {
        let Some(frame) = self.frame.as_ref() else {
            return false;
        };
        let hit = frame.scroll_hit(self.logical(x, y));
        self.scroll
            .can_scroll(hit.as_deref(), direction, ScrollAxis::Vertical)
    }

    /// Scrolls the region under the gesture's start; the whole gesture, momentum included,
    /// stays on that region. Returns whether a region moved.
    pub fn wheel(&mut self, event: Wheel) -> (bool, Vec<Effect>) {
        if self.inert
            || ![event.x, event.y, event.dx, event.dy]
                .iter()
                .all(|v| v.is_finite())
        {
            return (false, Vec::new());
        }
        let Some(frame) = self.frame.clone() else {
            return (false, Vec::new());
        };
        let axis = if event.dx.abs() > event.dy.abs() {
            ScrollAxis::Horizontal
        } else {
            ScrollAxis::Vertical
        };
        let fresh = matches!(event.phase, WheelPhase::Began | WheelPhase::Discrete);
        if fresh || self.wheel_target.is_none() {
            let point = self.logical(event.x, event.y);
            let hit = frame.scroll_hit(point);
            self.wheel_target = self.scroll.target_for_axis(hit.as_deref(), axis);
        }
        let Some(target) = self.wheel_target.clone() else {
            return (false, Vec::new());
        };
        let along = match self.scroll.axis(&target) {
            ScrollAxis::Horizontal => event.dx,
            ScrollAxis::Vertical => event.dy,
        };
        let scale = self.scale();
        let distance = if event.precise {
            along / scale
        } else {
            along * NOTCH
        };
        if event.phase == WheelPhase::Ended {
            // Momentum may follow and keeps the latch; a new Began replaces it.
            return (true, Vec::new());
        }
        if fresh {
            self.scroll.wheel_began(&target);
        }
        // Momentum stays on the latched region, as a platform scroll view's does: a list
        // that reaches its end stops there instead of handing the glide to the page.
        let moved = distance != 0.0
            && if event.phase == WheelPhase::Momentum {
                self.scroll.drag_region(&target, -distance) != 0.0
            } else {
                self.scroll.drag_by(&target, -distance)
            };
        let mut effects = Vec::new();
        if moved {
            effects.push(Effect::Redraw);
        }
        if event.phase == WheelPhase::Discrete {
            self.wheel_target = None;
        }
        (moved, effects)
    }
}
