//! Reveal a node through its scroll ancestors.
use crate::scene::display::*;
use std::collections::HashMap;

impl DisplayList {
    /// `margin` is room kept around the node, so a field does not sit flush on the keyboard.
    pub(crate) fn reveal(
        self,
        identity: &str,
        margin: f32,
        offsets: &mut HashMap<String, f32>,
    ) -> Result<Self, String> {
        let Some(target) = self.nodes.iter().find(|node| node.identity == identity) else {
            return Ok(self);
        };
        let (bounds, parent) = (target.bounds, target.scroll_parent.clone());
        self.reveal_rect(identity, bounds, parent.as_deref(), margin, offsets)
    }

    /// Scrolls `rect` (logical, as composed) into view through the scroll ancestors from
    /// `parent`; `identity` names it in the log.
    pub(crate) fn reveal_rect(
        self,
        identity: &str,
        rect: [f32; 4],
        parent: Option<&str>,
        margin: f32,
        offsets: &mut HashMap<String, f32>,
    ) -> Result<Self, String> {
        let mut chain = Vec::new();
        let mut parent = parent;
        let mut starts = [rect[0] - margin, rect[1] - margin];
        let mut ends = [rect[0] + rect[2] + margin, rect[1] + rect[3] + margin];
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
            // Device runs read this to see why a field did or did not scroll clear.
            log::info!(
                target: "lingxia_view",
                "reveal: {identity} in {}: target {:.0}..{:.0}, visible {start:.0}..{end:.0}, offset {:.0} -> {next:.0} of {:.0}",
                scroll.id, starts[axis], ends[axis], scroll.offset, scroll.max_offset
            );
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

    /// After a keyboard shrank: brings a revealed field in `scroll` back down until it keeps
    /// `margin` above the visible end, never scrolling back past `floor` (where the page was
    /// before the keyboard moved it).
    pub(crate) fn settle_reveal(
        self,
        identity: &str,
        margin: f32,
        offsets: &mut HashMap<String, f32>,
        scroll_id: &str,
        floor: f32,
    ) -> Result<Self, String> {
        let Some(target) = self.nodes.iter().find(|node| node.identity == identity) else {
            return Ok(self);
        };
        if target.scroll_parent.as_deref() != Some(scroll_id) {
            return Ok(self);
        }
        let Some((scroll, bounds)) = self.nodes.iter().find_map(|node| {
            let scroll = node
                .scroll
                .as_ref()
                .filter(|scroll| scroll.id == scroll_id)?;
            let bounds = if scroll.parent.is_none() {
                node.visible_bounds
            } else {
                node.bounds
            };
            Some((scroll, bounds))
        }) else {
            return Ok(self);
        };
        let axis = scroll.axis.index();
        let room = bounds[axis] + bounds[axis + 2]
            - (target.bounds[axis] + target.bounds[axis + 2] + margin);
        let floor = floor.clamp(0.0, scroll.max_offset);
        if room <= 0.5 || scroll.offset <= floor {
            return Ok(self);
        }
        let next = (scroll.offset - room).max(floor);
        log::info!(
            target: "lingxia_view",
            "reveal: {identity} in {}: {room:.0} past the margin, offset {:.0} -> {next:.0} (floor {floor:.0})",
            scroll.id, scroll.offset
        );
        let mut candidate = offsets.clone();
        candidate.insert(scroll.id.clone(), next);
        let display = self.retained.compose(&candidate)?;
        *offsets = candidate;
        Ok(display)
    }
}
