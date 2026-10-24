//! Scroll edge arrival events. Only presented frames deliver them, once per arrival.
use super::frame::{PresentedFrame, visible};
use std::collections::HashMap;

// One worker batch; remaining edges are delivered with a later presented frame.
const BATCH: usize = 32;

#[derive(Default)]
pub struct ReachTracker {
    entered: HashMap<String, [bool; 2]>,
}

pub struct Reached {
    pub handles: Vec<u64>,
    /// More edges are waiting for the next presentation.
    pub more: bool,
}

impl ReachTracker {
    pub fn clear(&mut self) {
        self.entered.clear();
    }

    pub fn presented(&mut self, frame: &PresentedFrame) -> Reached {
        let mut handles = Vec::new();
        let mut more = false;
        let mut alive = Vec::new();
        for node in &frame.nodes {
            let Some(scroll) = &node.scroll else { continue };
            alive.push(scroll.id.clone());
            let state = self.entered.entry(scroll.id.clone()).or_default();
            for (edge, entered) in state.iter_mut().enumerate() {
                let handle = if edge == 0 {
                    scroll.on_reach_start
                } else {
                    scroll.on_reach_end
                };
                let at_edge = if edge == 0 {
                    scroll.offset <= 0.01
                } else {
                    scroll.offset >= scroll.max_offset - 0.01
                };
                let Some(handle) = handle.filter(|handle| *handle != 0 && at_edge) else {
                    *entered = false;
                    continue;
                };
                if *entered || !visible(node) {
                    continue;
                }
                if handles.len() == BATCH {
                    more = true;
                    continue;
                }
                handles.push(handle);
                *entered = true;
            }
        }
        self.entered.retain(|id, _| alive.contains(id));
        Reached { handles, more }
    }
}
