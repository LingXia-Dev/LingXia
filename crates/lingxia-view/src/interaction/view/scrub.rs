//! `onPointer`: an element hears where a pointer is over it, in points from the top-left of its
//! content box. It observes: a button inside still gets its click, and a scroll still takes a
//! gesture on an axis the element does not keep (`pointerAxis`), which ends the report with
//! `cancel`.
use super::{Dispatch, Effect, Pointer, PointerPhase, ViewInteraction};
use crate::interaction::frame::{PresentedFrame, contains, hit_target};

/// A pointer down on an observing element.
#[derive(Clone, Debug)]
pub(super) struct Scrub {
    identity: String,
    /// The axes the element keeps from scrolls, horizontal then vertical.
    keeps: [bool; 2],
    /// Past the slop on a kept axis: scrolls no longer take the gesture.
    owned: bool,
}

/// The innermost element with `onPointer` that the topmost target at `point` is, or sits in.
fn observer(frame: &PresentedFrame, point: [f32; 2]) -> Option<usize> {
    let (_, target) = frame.hit_order().find(|(_, node)| {
        (node.on_pointer_event.is_some() || hit_target(node)) && contains(node, point, false)
    })?;
    let target = &*target.identity;
    frame
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.on_pointer_event.is_some())
        .filter(|(_, node)| {
            let own = &*node.identity;
            target == own
                || (target.len() > own.len()
                    && target.starts_with(own)
                    && target.as_bytes()[own.len()] == b'/')
        })
        .max_by_key(|(_, node)| node.identity.len())
        .map(|(index, _)| index)
}

impl ViewInteraction {
    /// Queues `phase` for the element `identity`, if it is still in the frame.
    fn pointer_report(&mut self, identity: &str, phase: &'static str, point: [f32; 2]) -> bool {
        let pointer = if self.fine_pointer { "mouse" } else { "touch" };
        self.pointer_report_from(identity, phase, point, pointer)
    }

    fn pointer_report_from(
        &mut self,
        identity: &str,
        phase: &'static str,
        point: [f32; 2],
        pointer: &'static str,
    ) -> bool {
        let Some(frame) = self.frame.as_ref() else {
            return false;
        };
        let Some(node) = frame.index_of(identity).map(|index| &frame.nodes[index]) else {
            return false;
        };
        let Some((handle, _)) = node.on_pointer_event else {
            return false;
        };
        let [left, top, width, height] = node.content_box;
        // A step has no position.
        let [x, y] = if pointer == "keyboard" {
            [0.0, 0.0]
        } else {
            [
                point[0] - node.bounds[0] - left,
                point[1] - node.bounds[1] - top,
            ]
        };
        let payload = serde_json::json!({
            "phase": phase,
            "x": x,
            "y": y,
            "width": width,
            "height": height,
            "pointer": pointer,
        });
        self.dispatches.push(Dispatch::PointerEvent {
            commit: frame.commit,
            handle,
            identity: identity.to_owned(),
            payload: payload.to_string(),
            latest: matches!(phase, "move" | "hover"),
        });
        true
    }

    /// Before the gesture machine sees `event`: a movement on an axis the element keeps is the
    /// element's, so no scroll or drag element starts on it.
    pub(super) fn scrub_before(&mut self, event: Pointer) -> Vec<Effect> {
        let Some(scrub) = self.scrub.as_mut().filter(|scrub| !scrub.owned) else {
            return Vec::new();
        };
        if event.phase != PointerPhase::Move {
            return Vec::new();
        }
        let dx = (event.x - self.gesture.down[0]).abs();
        let dy = (event.y - self.gesture.down[1]).abs();
        if dx.max(dy) <= event.slop || !scrub.keeps[usize::from(dy > dx)] {
            return Vec::new();
        }
        scrub.owned = true;
        self.gesture.target = None;
        self.gesture.drag = None;
        vec![Effect::Claim { exclusive: true }]
    }

    /// After the gesture machine: report the phase, or `cancel` once a scroll took the gesture.
    /// True when an observing element took the event.
    pub(super) fn scrub_after(&mut self, event: Pointer) -> bool {
        let point = self.logical(event.x, event.y);
        if event.phase == PointerPhase::Down {
            self.scrub = None;
            let Some(frame) = self.frame.clone().filter(|_| !self.inert) else {
                return false;
            };
            let Some(index) = observer(&frame, point) else {
                return false;
            };
            let node = &frame.nodes[index];
            let keeps = node.on_pointer_event.map_or([true; 2], |(_, keeps)| keeps);
            let identity = node.identity.to_string();
            self.hovering = None;
            self.scrub = Some(Scrub {
                owned: false,
                keeps,
                identity: identity.clone(),
            });
            return self.pointer_report(&identity, "down", point);
        }
        let Some(scrub) = self.scrub.clone() else {
            return false;
        };
        let taken = self.gesture.dragging || self.gesture.holding;
        let phase = match event.phase {
            PointerPhase::Move if !taken => "move",
            PointerPhase::Up if !taken => "up",
            _ => "cancel",
        };
        if phase != "move" {
            self.scrub = None;
        }
        self.pointer_report(&scrub.identity, phase, point)
    }

    /// A fine pointer with no button down moved to a logical point, or left the page (`None`).
    pub fn pointer_hover(&mut self, point: Option<[f32; 2]>) -> Vec<Effect> {
        if self.scrub.is_some() || self.inert {
            return Vec::new();
        }
        let over = self.frame.clone().zip(point).and_then(|(frame, point)| {
            observer(&frame, point).map(|index| frame.nodes[index].identity.to_string())
        });
        let mut reported = false;
        if let Some((left, last)) = self
            .hovering
            .take()
            .filter(|(was, _)| Some(was) != over.as_ref())
        {
            reported |= self.pointer_report(&left, "leave", point.unwrap_or(last));
        }
        if let (Some(identity), Some(point)) = (over, point) {
            reported |= self.pointer_report(&identity, "hover", point);
            self.hovering = Some((identity, point));
        }
        if reported {
            vec![Effect::Dispatch]
        } else {
            Vec::new()
        }
    }

    /// A key or assistive input stepped a value-reporting element: its handler hears
    /// `increment` or `decrement`.
    pub(super) fn pointer_step(&mut self, identity: &str, forward: bool) -> Vec<Effect> {
        let phase = if forward { "increment" } else { "decrement" };
        if self.pointer_report_from(identity, phase, [0.0; 2], "keyboard") {
            vec![Effect::Dispatch]
        } else {
            Vec::new()
        }
    }

    /// The gesture ended without a release (pause, focus loss, failure): the element hears `cancel`.
    pub(super) fn scrub_cancel(&mut self) -> bool {
        self.hovering = None;
        let Some(scrub) = self.scrub.take() else {
            return false;
        };
        let point = self.logical(self.gesture.down[0], self.gesture.down[1]);
        self.pointer_report(&scrub.identity, "cancel", point)
    }
}
