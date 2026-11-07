//! Slider value math and drag ownership. One proposal is in flight; newer
//! input replaces the pending one, and identities survive presented frames.
use super::frame::slider_enabled;
use crate::SliderSnapshot;
use crate::scene::node::NodeSnapshot;
use crate::session::lifecycle::PageTicket;

pub fn normalize(range: &SliderSnapshot, value: f64) -> f64 {
    let (min, max, step) = (range.min, range.max, range.step);
    let value = value.clamp(min, max);
    let snapped = (min + ((value - min) / step + 0.5).floor() * step).clamp(min, max);
    if (max - value).abs() < (snapped - value).abs() {
        max
    } else {
        snapped
    }
}

fn track(node: &NodeSnapshot) -> [f32; 4] {
    node.control_bounds.unwrap_or(node.bounds)
}

/// The value under a logical x coordinate along the track.
pub fn value_at(node: &NodeSnapshot, x: f32) -> Option<f64> {
    let range = node.range.as_ref()?;
    let [left, _, width, _] = track(node);
    let width = f64::from(width - 2.0 * range.inset);
    if width <= 0.0 {
        return Some(range.value);
    }
    let fraction = f64::from(x - left - range.inset) / width;
    Some(normalize(
        range,
        range.min + (range.max - range.min) * fraction,
    ))
}

/// One keyboard or accessibility step; the last step down from max lands on the final grid value.
pub fn shift(range: &SliderSnapshot, value: f64, direction: i32) -> f64 {
    if direction < 0 && value == range.max {
        let last = range.min + ((range.max - range.min) / range.step).floor() * range.step;
        if last < range.max {
            return normalize(range, last);
        }
    }
    normalize(range, value + f64::from(direction) * range.step)
}

fn definition(range: &SliderSnapshot) -> [f64; 3] {
    [range.min, range.max, range.step]
}

#[derive(Clone)]
pub struct Proposal {
    pub identity: String,
    definition: [f64; 3],
    pub value: f64,
    pub page: PageTicket,
    epoch: u64,
}

impl Proposal {
    pub fn matches(&self, node: &NodeSnapshot) -> bool {
        slider_enabled(node)
            && node.identity == self.identity
            && node
                .range
                .as_ref()
                .is_some_and(|range| definition(range) == self.definition)
    }
}

struct Gesture {
    node: NodeSnapshot,
    down: [f32; 2],
    claimed: bool,
}

#[derive(Default)]
pub struct SliderInput {
    epoch: u64,
    pub pending: Option<Proposal>,
    pub flight: Option<Proposal>,
    gesture: Option<Gesture>,
}

impl SliderInput {
    /// Drop pending input and any drag; in-flight proposals become stale.
    pub fn cancel(&mut self) {
        self.epoch += 1;
        self.pending = None;
        self.gesture = None;
    }

    pub fn propose(&mut self, node: &NodeSnapshot, value: f64, page: PageTicket) -> bool {
        let Some(range) = node.range.as_ref().filter(|_| slider_enabled(node)) else {
            return false;
        };
        if !value.is_finite() || value.abs() > 1e9 {
            return false;
        }
        self.pending = Some(Proposal {
            identity: node.identity.to_string(),
            definition: definition(range),
            value: normalize(range, value),
            page,
            epoch: self.epoch,
        });
        true
    }

    /// The value the user currently sees for a slider, including unacknowledged input.
    pub fn value(&self, node: &NodeSnapshot) -> Option<f64> {
        if let Some(pending) = self.pending.as_ref().filter(|p| p.matches(node)) {
            return Some(pending.value);
        }
        if let Some(flight) = self
            .flight
            .as_ref()
            .filter(|p| p.epoch == self.epoch && p.matches(node))
        {
            return Some(flight.value);
        }
        node.range.as_ref().map(|range| range.value)
    }

    /// Take the pending proposal for dispatch; it is current only while its epoch holds.
    pub fn begin_flight(&mut self) -> Option<Proposal> {
        let proposal = self.pending.take()?;
        (proposal.epoch == self.epoch).then(|| {
            self.flight = Some(proposal.clone());
            proposal
        })
    }

    pub fn end_flight(&mut self) {
        self.flight = None;
    }

    pub fn is_current(&self, proposal: &Proposal) -> bool {
        proposal.epoch == self.epoch
    }

    pub fn begin(&mut self, node: Option<&NodeSnapshot>, point: [f32; 2]) -> bool {
        match node.filter(|node| slider_enabled(node)) {
            Some(node) => {
                self.gesture = Some(Gesture {
                    node: node.clone(),
                    down: point,
                    claimed: false,
                });
                true
            }
            None => false,
        }
    }

    pub fn has_gesture(&self) -> bool {
        self.gesture.is_some()
    }

    pub fn dragging(&self) -> bool {
        self.gesture.as_ref().is_some_and(|gesture| gesture.claimed)
    }

    pub fn gesture_node(&self) -> Option<&NodeSnapshot> {
        self.gesture.as_ref().map(|gesture| &gesture.node)
    }

    /// Horizontal movement past slop claims the drag; vertical movement yields to scrolling.
    pub fn moved(&mut self, point: [f32; 2], slop: f32) -> bool {
        let Some(gesture) = &mut self.gesture else {
            return false;
        };
        let dx = (point[0] - gesture.down[0]).abs();
        let dy = (point[1] - gesture.down[1]).abs();
        if !gesture.claimed && dy > slop && dy > dx {
            self.cancel();
            return false;
        }
        if dx > slop && dx >= dy {
            gesture.claimed = true;
        }
        gesture.claimed
    }

    pub fn end(&mut self) -> Option<NodeSnapshot> {
        self.gesture.take().map(|gesture| gesture.node)
    }

    /// Keep a drag only while its slider is unchanged in the new frame.
    pub fn presented(&mut self, nodes: &[NodeSnapshot], scale_changed: bool) {
        let Some(gesture) = &self.gesture else { return };
        let current = nodes
            .iter()
            .find(|node| node.identity == gesture.node.identity);
        let keep = current.is_some_and(|current| {
            slider_enabled(current)
                && !scale_changed
                && current.visible_bounds[2] > 0.0
                && current.visible_bounds[3] > 0.0
                && current.range.as_ref().map(definition)
                    == gesture.node.range.as_ref().map(definition)
                && current.bounds == gesture.node.bounds
                && track(current) == track(&gesture.node)
        });
        match current.filter(|_| keep) {
            Some(current) => self.gesture.as_mut().unwrap().node = current.clone(),
            None => self.cancel(),
        }
    }
}
