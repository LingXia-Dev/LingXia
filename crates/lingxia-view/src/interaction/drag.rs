//! `drag` elements: offsets owned by the input thread, like scroll offsets. A finger moves an
//! element between its snap points; on release the host springs it to one (`Release`).
use super::frame::PresentedFrame;
use crate::scene::node::{DragSnapshot, ScrollAxis};
use std::collections::{HashMap, HashSet};

/// How far past its last point a finger pulls an element, as a share of the distance.
const ELASTIC: f32 = 0.35;
/// How far a release velocity carries an element before it picks a point, seconds.
const PROJECTION: f32 = 0.15;
/// Offsets kept for elements gone from the frame: an exit plays from where they were.
const KEPT: usize = 256;

/// A released drag: spring `identity` from `from` to `to` (snap point `index`) with `velocity`.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub identity: String,
    pub from: f32,
    pub to: f32,
    pub velocity: f32,
    pub index: usize,
    pub handle: u64,
    pub duration: std::time::Duration,
    pub bounce: f64,
}

#[derive(Default)]
pub struct DragModel {
    offsets: HashMap<String, f32>,
    regions: HashMap<String, DragSnapshot>,
    /// Elements a finger holds or a release still springs: frames keep their offsets.
    moving: HashSet<String>,
    /// Elements under a finger: a snap index the View sets meanwhile waits for the release.
    held: HashSet<String>,
    released: Vec<Release>,
    /// The snap point each element rests on or springs to, once input moved it.
    targets: HashMap<String, usize>,
}

impl DragModel {
    /// A frame keeps each element where the finger or its spring left it. One that enters sits
    /// at its resting point; one whose snap index the View changed springs there.
    pub fn accept(&mut self, frame: &PresentedFrame) {
        let previous = std::mem::take(&mut self.regions);
        for node in &frame.nodes {
            let Some(drag) = &node.drag else { continue };
            match previous.get(&node.identity) {
                None => {
                    if !self.moving.contains(&node.identity) {
                        self.offsets.insert(node.identity.clone(), drag.rest());
                        self.targets.remove(&node.identity);
                    }
                }
                Some(before)
                    if before.index != drag.index
                        && !self.held.contains(&node.identity)
                        && self.targets.get(&node.identity) != Some(&drag.index) =>
                {
                    let from = self.offset(&node.identity);
                    self.regions.insert(node.identity.clone(), drag.clone());
                    self.spring_to(&node.identity, from, drag.index, 0.0);
                }
                // Points that moved (a resize resolving `%` anew) carry an element at rest along.
                Some(before)
                    if before.points != drag.points && !self.moving.contains(&node.identity) =>
                {
                    let index = self
                        .targets
                        .get(&node.identity)
                        .copied()
                        .filter(|index| *index < drag.points.len())
                        .unwrap_or(drag.index);
                    self.offsets
                        .insert(node.identity.clone(), drag.points[index]);
                }
                Some(_) => {}
            }
            self.regions.insert(node.identity.clone(), drag.clone());
        }
        if self.offsets.len() > KEPT {
            let regions = &self.regions;
            self.offsets.retain(|id, _| regions.contains_key(id));
            self.targets.retain(|id, _| regions.contains_key(id));
        }
    }

    pub fn region(&self, identity: &str) -> Option<&DragSnapshot> {
        self.regions.get(identity)
    }

    pub fn offset(&self, identity: &str) -> f32 {
        self.offsets
            .get(identity)
            .copied()
            .or_else(|| self.regions.get(identity).map(DragSnapshot::rest))
            .unwrap_or(0.0)
    }

    /// Offsets for the next frame, keyed as composition reads them.
    pub fn snapshot_into(&self, offsets: &mut HashMap<String, f32>) {
        for (identity, offset) in &self.offsets {
            offsets.insert(DragSnapshot::key(identity), *offset);
        }
    }

    /// The innermost drag element under `point` on `axis`, by paint order.
    pub fn hit(&self, frame: &PresentedFrame, point: [f32; 2]) -> Option<String> {
        frame
            .hit_order()
            .find(|(_, node)| {
                node.drag.is_some()
                    && super::frame::visible(node)
                    && point[0] >= node.bounds[0]
                    && point[0] <= node.bounds[0] + node.bounds[2]
                    && point[1] >= node.bounds[1]
                    && point[1] <= node.bounds[1] + node.bounds[3]
            })
            .map(|(_, node)| node.identity.clone())
    }

    pub fn axis(&self, identity: &str) -> Option<ScrollAxis> {
        self.regions.get(identity).map(|drag| drag.axis)
    }

    /// A finger takes hold: a release in flight stops where it is.
    pub fn hold(&mut self, identity: &str) {
        self.moving.insert(identity.to_owned());
        self.held.insert(identity.to_owned());
        self.released.retain(|release| release.identity != identity);
    }

    /// Move by a finger `delta`; past the ends the element follows only part of it.
    pub fn drag_by(&mut self, identity: &str, delta: f32) -> bool {
        let Some(drag) = self.regions.get(identity) else {
            return false;
        };
        if !delta.is_finite() || delta == 0.0 {
            return false;
        }
        let (min, max) = drag.range();
        let offset = self.offset(identity);
        // Where the finger alone would put it; only the stretch beyond the points resists.
        let inside = offset.clamp(min, max);
        let raw = inside + (offset - inside) / ELASTIC + delta;
        let resting = raw.clamp(min, max);
        self.offsets
            .insert(identity.to_owned(), resting + (raw - resting) * ELASTIC);
        true
    }

    /// The finger let go with `velocity` (logical px/s, screen direction); `fling` is the
    /// least speed that carries it at least one point on.
    pub fn release(&mut self, identity: &str, velocity: f32, fling: f32) -> Option<Release> {
        let drag = self.regions.get(identity)?;
        let from = self.offset(identity);
        let velocity = if velocity.is_finite() { velocity } else { 0.0 };
        let projected = from + velocity * PROJECTION;
        let nearest = |at: f32| {
            (0..drag.points.len())
                .min_by(|a, b| {
                    (drag.points[*a] - at)
                        .abs()
                        .total_cmp(&(drag.points[*b] - at).abs())
                })
                .unwrap_or(0)
        };
        let mut index = nearest(projected);
        if velocity.abs() >= fling && index == nearest(from) {
            // A flick moves on to the next point in its direction.
            let next = (0..drag.points.len())
                .filter(|i| {
                    let point = drag.points[*i];
                    if velocity > 0.0 {
                        point > from + 0.5
                    } else {
                        point < from - 0.5
                    }
                })
                .min_by(|a, b| {
                    (drag.points[*a] - from)
                        .abs()
                        .total_cmp(&(drag.points[*b] - from).abs())
                });
            if let Some(next) = next {
                index = next;
            }
        }
        Some(self.spring_to(identity, from, index, velocity))
    }

    /// Spring `identity` from `from` to snap point `index`.
    fn spring_to(&mut self, identity: &str, from: f32, index: usize, velocity: f32) -> Release {
        self.held.remove(identity);
        let drag = &self.regions[identity];
        let release = Release {
            identity: identity.to_owned(),
            from,
            to: drag.points[index],
            velocity,
            index,
            handle: drag.handle,
            duration: drag.duration,
            bounce: drag.bounce,
        };
        self.moving.insert(identity.to_owned());
        self.targets.insert(identity.to_owned(), index);
        self.released.retain(|other| other.identity != identity);
        self.released.push(release.clone());
        release
    }

    /// The snap point `identity` rests on or springs to.
    pub fn target(&self, identity: &str) -> Option<usize> {
        let drag = self.regions.get(identity)?;
        Some(
            self.targets
                .get(identity)
                .copied()
                .filter(|index| *index < drag.points.len())
                .unwrap_or(drag.index),
        )
    }

    /// Step to the neighbouring snap point by index, for keys and assistive input.
    pub fn step(&mut self, identity: &str, direction: i32) -> Option<Release> {
        let count = self.regions.get(identity)?.points.len();
        let index = self.target(identity)?;
        let next = match direction {
            d if d > 0 && index + 1 < count => index + 1,
            d if d < 0 && index > 0 => index - 1,
            _ => return None,
        };
        let from = self.offset(identity);
        Some(self.spring_to(identity, from, next, 0.0))
    }

    /// Spring to snap point `index` directly (Home, End).
    pub fn go(&mut self, identity: &str, index: usize) -> Option<Release> {
        let count = self.regions.get(identity)?.points.len();
        if index >= count || self.target(identity) == Some(index) {
            return None;
        }
        let from = self.offset(identity);
        Some(self.spring_to(identity, from, index, 0.0))
    }

    /// A finger let go of, or never moved, an element whose spring it stopped.
    pub fn resume(&mut self, identity: &str) -> Option<Release> {
        if !self.moving.contains(identity) {
            return None;
        }
        let index = self.target(identity)?;
        let from = self.offset(identity);
        Some(self.spring_to(identity, from, index, 0.0))
    }

    pub fn take_released(&mut self) -> Vec<Release> {
        std::mem::take(&mut self.released)
    }

    /// A release's spring places the element; `done` ends its hold on the frame's offset.
    pub fn set(&mut self, identity: &str, offset: f32, done: bool) {
        if offset.is_finite() {
            self.offsets.insert(identity.to_owned(), offset);
        }
        if done {
            self.moving.remove(identity);
        }
    }
}
