//! Scroll edge events. Only presented frames deliver them: `onReachStart`/`onReachEnd` once
//! per arrival, `onScrollEdge` once per change of being at an edge.
use super::frame::{PresentedFrame, visible};
use std::collections::HashMap;

// One worker batch; remaining edges are delivered with a later presented frame.
const BATCH: usize = 32;

#[derive(Default)]
pub struct ReachTracker {
    entered: HashMap<String, [bool; 2]>,
    /// What `onScrollEdge` last told the page, per edge; a new scroll starts away from both.
    told: HashMap<String, [bool; 2]>,
}

pub struct Reached {
    pub handles: Vec<u64>,
    /// `onScrollEdge` callbacks and their payloads.
    pub edges: Vec<(u64, String)>,
    /// More edges are waiting for the next presentation.
    pub more: bool,
}

impl ReachTracker {
    pub fn clear(&mut self) {
        self.entered.clear();
        self.told.clear();
    }

    pub fn presented(&mut self, frame: &PresentedFrame) -> Reached {
        let mut handles = Vec::new();
        let mut edges = Vec::new();
        let mut more = false;
        let mut alive = Vec::new();
        for node in &frame.nodes {
            let Some(scroll) = &node.scroll else { continue };
            alive.push(scroll.id.clone());
            let at = [
                scroll.offset <= 0.01,
                scroll.offset >= scroll.max_offset - 0.01,
            ];
            if let Some(handle) = scroll.on_scroll_edge.filter(|handle| *handle != 0)
                && visible(node)
            {
                let told = self.told.entry(scroll.id.clone()).or_default();
                for (edge, name) in ["start", "end"].into_iter().enumerate() {
                    if told[edge] == at[edge] {
                        continue;
                    }
                    if handles.len() + edges.len() == BATCH {
                        more = true;
                        continue;
                    }
                    edges.push((handle, format!(r#"{{"edge":"{name}","at":{}}}"#, at[edge])));
                    told[edge] = at[edge];
                }
            }
            let state = self.entered.entry(scroll.id.clone()).or_default();
            for (edge, entered) in state.iter_mut().enumerate() {
                let handle = if edge == 0 {
                    scroll.on_reach_start
                } else {
                    scroll.on_reach_end
                };
                let Some(handle) = handle.filter(|handle| *handle != 0 && at[edge]) else {
                    *entered = false;
                    continue;
                };
                if *entered || !visible(node) {
                    continue;
                }
                if handles.len() + edges.len() == BATCH {
                    more = true;
                    continue;
                }
                handles.push(handle);
                *entered = true;
            }
        }
        self.entered.retain(|id, _| alive.contains(id));
        self.told.retain(|id, _| alive.contains(id));
        Reached {
            handles,
            edges,
            more,
        }
    }
}
