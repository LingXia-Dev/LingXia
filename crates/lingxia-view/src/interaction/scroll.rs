//! Desired scroll offsets owned by the input thread. A late frame never
//! overwrites newer finger movement: offsets carry a revision.
use super::frame::PresentedFrame;
use crate::scene::node::{NodeSnapshot, ScrollAxis, ScrollSnapshot};
use std::collections::HashMap;

// The native schema bounds nesting; this also rejects malformed parent cycles.
const MAX_DEPTH: usize = 129;

#[derive(Default)]
pub struct ScrollModel {
    offsets: HashMap<String, f32>,
    regions: HashMap<String, ScrollSnapshot>,
    revision: u64,
    /// Snapping regions a finger, wheel or fling moved since the last settle.
    moved_snaps: std::collections::BTreeSet<String>,
}

impl ScrollModel {
    pub fn clear(&mut self) {
        self.offsets.clear();
        self.regions.clear();
        self.revision += 1;
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Offsets to request with the next frame.
    pub fn snapshot(&self) -> HashMap<String, f32> {
        self.offsets.clone()
    }

    pub fn offset(&self, id: &str) -> f32 {
        self.offsets.get(id).copied().unwrap_or(0.0)
    }

    fn parent(region: &ScrollSnapshot) -> Option<&str> {
        region.parent.as_deref()
    }

    /// The first region at or above `id` that scrolls along `axis`.
    pub fn target_for_axis<'a>(
        &'a self,
        mut id: Option<&'a str>,
        axis: ScrollAxis,
    ) -> Option<String> {
        for _ in 0..MAX_DEPTH {
            let region = self.regions.get(id?)?;
            if region.axis == axis && region.max_offset > 0.0 {
                return Some(region.id.clone());
            }
            if region.axis == axis && region.contain {
                return None;
            }
            id = Self::parent(region);
        }
        None
    }

    pub fn axis(&self, id: &str) -> ScrollAxis {
        self.regions
            .get(id)
            .map(|region| region.axis)
            .unwrap_or(ScrollAxis::Vertical)
    }

    /// Adopt a presented frame's regions, reapplying finger movement made since it was requested.
    pub fn accept(
        &mut self,
        frame: &PresentedFrame,
        requested_revision: u64,
        requested: &HashMap<String, f32>,
    ) {
        self.regions.clear();
        for scroll in frame.nodes.iter().filter_map(|node| node.scroll.as_ref()) {
            self.regions.insert(scroll.id.clone(), scroll.clone());
            // Measurement can move the frame's origin while newer finger deltas are pending.
            let correction = scroll.offset - requested.get(&scroll.id).copied().unwrap_or(0.0);
            let desired = if requested_revision == self.revision {
                scroll.offset
            } else {
                self.offset(&scroll.id) + correction
            };
            self.offsets.insert(
                scroll.id.clone(),
                desired.clamp(0.0, scroll.max_offset.max(0.0)),
            );
        }
        let regions = &self.regions;
        self.offsets.retain(|id, _| regions.contains_key(id));
    }

    /// Put one region at `offset`, clamped; nothing hands over to ancestors.
    pub fn scroll_to(&mut self, id: &str, offset: f32) -> bool {
        let Some(region) = self.regions.get(id) else {
            return false;
        };
        let offset = offset.clamp(0.0, region.max_offset.max(0.0));
        if !offset.is_finite() || self.offset(id) == offset {
            return false;
        }
        self.offsets.insert(id.to_owned(), offset);
        self.revision += 1;
        true
    }

    /// Snapping regions moved by input since the last call.
    pub fn take_moved_snaps(&mut self) -> Vec<String> {
        std::mem::take(&mut self.moved_snaps).into_iter().collect()
    }

    /// The snap point nearest region `id`'s offset, when it snaps and is not on one.
    pub fn snap_target(&self, id: &str) -> Option<f32> {
        let region = self.regions.get(id)?;
        let offset = self.offset(id);
        let nearest = region
            .snap
            .iter()
            .copied()
            .min_by(|a, b| (a - offset).abs().total_cmp(&(b - offset).abs()))?;
        ((nearest - offset).abs() > 0.25).then_some(nearest)
    }

    /// Move a region, handing remaining distance to ancestors on the same axis.
    pub fn scroll_by(&mut self, id: &str, delta: f32) -> bool {
        self.move_by(id, delta, false)
    }

    /// A finger or fling distance: positive moves content toward the start of
    /// the screen axis, which advances an RTL horizontal region's offset backwards.
    pub fn drag_by(&mut self, id: &str, delta: f32) -> bool {
        self.move_by(id, delta, true)
    }

    fn move_by(&mut self, id: &str, mut delta: f32, visual: bool) -> bool {
        if !delta.is_finite() {
            return false;
        }
        let axis = self.axis(id);
        let mut id = Some(id.to_owned());
        let mut changed = false;
        for _ in 0..MAX_DEPTH {
            let Some(current) = id.take() else { break };
            if delta.abs() <= 0.001 {
                break;
            }
            let Some(region) = self.regions.get(&current) else {
                break;
            };
            let parent = region.parent.clone();
            if region.axis == axis {
                let sign = if visual && region.rtl { -1.0 } else { 1.0 };
                let before = self.offset(&current);
                let after = (before + sign * delta).clamp(0.0, region.max_offset.max(0.0));
                if after != before {
                    if !region.snap.is_empty() {
                        self.moved_snaps.insert(current.clone());
                    }
                    self.offsets.insert(current, after);
                    changed = true;
                }
                delta -= sign * (after - before);
                if region.contain {
                    break;
                }
            }
            id = parent;
        }
        if changed {
            self.revision += 1;
        }
        changed
    }

    /// Move region `id` alone by a finger distance (as `drag_by`); returns the distance it took.
    pub fn drag_region(&mut self, id: &str, delta: f32) -> f32 {
        let Some(region) = self.regions.get(id) else {
            return 0.0;
        };
        if !delta.is_finite() {
            return 0.0;
        }
        let sign = if region.rtl { -1.0 } else { 1.0 };
        let before = self.offset(id);
        let after = (before + sign * delta).clamp(0.0, region.max_offset.max(0.0));
        if after == before {
            return 0.0;
        }
        if !region.snap.is_empty() {
            self.moved_snaps.insert(id.to_owned());
        }
        self.offsets.insert(id.to_owned(), after);
        self.revision += 1;
        sign * (after - before)
    }

    pub fn contains_region(&self, id: &str) -> bool {
        self.regions.get(id).is_some_and(|region| region.contain)
    }

    /// Whether a region or an ancestor on `axis` can still move in `direction`.
    pub fn can_scroll<'a>(
        &'a self,
        mut id: Option<&'a str>,
        direction: i32,
        axis: ScrollAxis,
    ) -> bool {
        for _ in 0..MAX_DEPTH {
            let Some(region) = id.and_then(|id| self.regions.get(id)) else {
                return false;
            };
            let value = self.offset(&region.id);
            if region.axis == axis
                && (if direction < 0 {
                    value > 0.001
                } else {
                    value < region.max_offset - 0.001
                })
            {
                return true;
            }
            if region.axis == axis && region.contain {
                return false;
            }
            id = Self::parent(region);
        }
        false
    }

    /// Offsets that bring `target` into view through every scroll ancestor, or `None` if it cannot be.
    pub fn reveal_offsets(
        &self,
        frame: &PresentedFrame,
        target: &NodeSnapshot,
    ) -> Option<HashMap<String, f32>> {
        let valid = |[x, y, w, h]: [f32; 4]| {
            [x, y, w, h].iter().all(|v| v.is_finite()) && w > 0.0 && h > 0.0
        };
        if !valid(target.bounds) {
            return None;
        }
        let mut start = [target.bounds[0], target.bounds[1]];
        let mut end = [start[0] + target.bounds[2], start[1] + target.bounds[3]];
        let mut changes = HashMap::new();
        let mut id = target.scroll_parent.clone();
        for _ in 0..MAX_DEPTH {
            let Some(current) = id.take() else {
                return Some(changes);
            };
            let owner = frame.nodes.iter().find(|node| {
                node.scroll
                    .as_ref()
                    .is_some_and(|scroll| scroll.id == current)
            })?;
            let region = owner.scroll.as_ref()?;
            let bounds = if region.parent.is_none() {
                owner.visible_bounds
            } else {
                owner.bounds
            };
            if !valid(bounds) {
                return None;
            }
            let axis = match region.axis {
                ScrollAxis::Horizontal => 0,
                ScrollAxis::Vertical => 1,
            };
            let cross = 1 - axis;
            start[cross] = start[cross].max(bounds[cross]);
            end[cross] = end[cross].min(bounds[cross] + bounds[cross + 2]);
            if end[cross] <= start[cross] {
                return None;
            }
            let before = self.offset(&current);
            let pending = before - region.offset;
            start[axis] -= pending;
            end[axis] -= pending;
            let low = bounds[axis];
            let high = low + bounds[axis + 2];
            let delta = if start[axis] < low || end[axis] - start[axis] > high - low {
                start[axis] - low
            } else {
                (end[axis] - high).max(0.0)
            };
            let after = (before + delta).clamp(0.0, region.max_offset.max(0.0));
            if after != before {
                changes.insert(current.clone(), after);
            }
            start[axis] = low.max(start[axis] - (after - before));
            end[axis] = high.min(end[axis] - (after - before));
            if end[axis] <= start[axis] {
                return None;
            }
            id = region.parent.clone();
        }
        None
    }

    pub fn can_reveal(&self, frame: &PresentedFrame, target: &NodeSnapshot) -> bool {
        target.scroll_parent.is_some() && self.reveal_offsets(frame, target).is_some()
    }

    /// Apply reveal offsets; returns whether any region moved.
    pub fn reveal(&mut self, frame: &PresentedFrame, target: &NodeSnapshot) -> bool {
        if target.scroll_parent.is_none() {
            return false;
        }
        match self.reveal_offsets(frame, target) {
            Some(changes) if !changes.is_empty() => {
                self.offsets.extend(changes);
                self.revision += 1;
                true
            }
            _ => false,
        }
    }
}
