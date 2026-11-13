//! `onPointerEnter`/`onPointerLeave`: a fine pointer crossing an element's box. Like the DOM's
//! enter and leave they do not bubble: the topmost element under the pointer and each of its
//! ancestors with a callback hear their own crossing, outermost first on entry and innermost
//! first on exit.
use super::{Dispatch, Effect, ViewInteraction};
use crate::interaction::frame::{Acts, PresentedFrame, acts, contains, hit_target};

/// An element the pointer is over: its identity and its leave callback.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Over {
    identity: String,
    leave: Option<u64>,
}

/// The elements with a crossing callback the pointer is over, outermost first.
fn over(frame: &PresentedFrame, point: [f32; 2]) -> Vec<(usize, Option<u64>)> {
    // Layout wrappers and transparent layers take no pointer; a control or a box with a crossing
    // callback does, so a dialog's backdrop over the box makes the pointer leave it.
    let Some((_, target)) = frame.hit_order().find(|(_, node)| {
        (acts(node).any(Acts::CROSSING) || hit_target(node)) && contains(node, point, false)
    }) else {
        return Vec::new();
    };
    let target = &*target.identity;
    frame
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.on_pointer.iter().any(Option::is_some))
        .filter(|(_, node)| {
            let own = &*node.identity;
            target == own
                || (target.len() > own.len()
                    && target.starts_with(own)
                    && target.as_bytes()[own.len()] == b'/')
        })
        .map(|(index, node)| (index, node.on_pointer[1]))
        .collect()
}

impl ViewInteraction {
    /// A fine pointer moved to a logical point, or left the page (`None`).
    pub fn pointer_crossing(&mut self, point: Option<[f32; 2]>) -> Vec<Effect> {
        let Some(frame) = self.frame.clone() else {
            return self.pointer_gone();
        };
        let now: Vec<Over> = point
            .map(|point| over(&frame, point))
            .unwrap_or_default()
            .into_iter()
            .map(|(index, leave)| Over {
                identity: frame.nodes[index].identity.to_string(),
                leave,
            })
            .collect();
        let commit = frame.commit;
        let mut crossed = false;
        for left in self.pointer_over.iter().rev() {
            if !now.iter().any(|over| over.identity == left.identity)
                && let Some(handle) = left.leave
            {
                self.dispatches.push(Dispatch::PointerCrossing {
                    commit,
                    handle,
                    identity: left.identity.clone(),
                    enter: false,
                });
                crossed = true;
            }
        }
        for entered in &now {
            if self
                .pointer_over
                .iter()
                .any(|over| over.identity == entered.identity)
            {
                continue;
            }
            let handle = frame
                .nodes
                .iter()
                .find(|node| *node.identity == *entered.identity)
                .and_then(|node| node.on_pointer[0]);
            if let Some(handle) = handle {
                self.dispatches.push(Dispatch::PointerCrossing {
                    commit,
                    handle,
                    identity: entered.identity.clone(),
                    enter: true,
                });
                crossed = true;
            }
        }
        self.pointer_over = now;
        if crossed {
            vec![Effect::Dispatch]
        } else {
            Vec::new()
        }
    }

    /// The pointer left without a position (window blur, page hidden, a finger took over).
    fn pointer_gone(&mut self) -> Vec<Effect> {
        self.pointer_over.clear();
        Vec::new()
    }
}
