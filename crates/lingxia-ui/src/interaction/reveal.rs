//! Reveal a node through its scroll ancestors.
use crate::scene::display::*;
use std::collections::HashMap;

impl DisplayList {
    pub(crate) fn reveal(
        self,
        identity: &str,
        offsets: &mut HashMap<String, f32>,
    ) -> Result<Self, String> {
        let Some(target) = self.nodes.iter().find(|node| node.identity == identity) else {
            return Ok(self);
        };
        let mut chain = Vec::new();
        let mut parent = target.scroll_parent.as_deref();
        let mut starts = [target.bounds[0], target.bounds[1]];
        let mut ends = [starts[0] + target.bounds[2], starts[1] + target.bounds[3]];
        while let Some(id) = parent {
            let Some(owner) = self
                .nodes
                .iter()
                .find(|node| node.scroll.as_ref().is_some_and(|scroll| scroll.id == id))
            else {
                return Ok(self);
            };
            let scroll = owner.scroll.as_ref().unwrap();
            let bounds = if scroll.parent.is_none() {
                owner.visible_bounds
            } else {
                owner.bounds
            };
            if bounds[2] <= 0.0 || bounds[3] <= 0.0 || chain.len() >= 128 {
                return Ok(self);
            }
            chain.push((scroll, bounds));
            parent = scroll.parent.as_deref();
        }
        let mut changes = Vec::new();
        for (scroll, bounds) in chain {
            let axis = scroll.axis.index();
            let cross = 1 - axis;
            starts[cross] = starts[cross].max(bounds[cross]);
            ends[cross] = ends[cross].min(bounds[cross] + bounds[cross + 2]);
            if ends[cross] <= starts[cross] {
                return Ok(self);
            }
            let start = bounds[axis];
            let end = start + bounds[axis + 2];
            let delta = if starts[axis] < start || ends[axis] - starts[axis] > bounds[axis + 2] {
                starts[axis] - start
            } else {
                (ends[axis] - end).max(0.0)
            };
            let next = (scroll.offset + delta).clamp(0.0, scroll.max_offset);
            let movement = next - scroll.offset;
            if movement.abs() > 0.01 {
                changes.push((scroll.id.clone(), next));
                starts[axis] -= movement;
                ends[axis] -= movement;
            }
            starts[axis] = starts[axis].max(start);
            ends[axis] = ends[axis].min(end);
            if ends[axis] <= starts[axis] {
                return Ok(self);
            }
        }
        if changes.is_empty() {
            Ok(self)
        } else {
            let mut candidate = offsets.clone();
            candidate.extend(changes);
            let display = self.retained.compose(&candidate)?;
            *offsets = candidate;
            Ok(display)
        }
    }
}
