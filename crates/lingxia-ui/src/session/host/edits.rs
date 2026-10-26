//! Accessibility and automation edits reach the platform input method one at a
//! time: the next waits until the presented frame shows the previous applied, so
//! an input connection still settling an edit never has to refuse one.
use super::Inner;
use crate::interaction::view::Effect;
use crate::platform::PlatformAdapter;
use std::collections::VecDeque;
use std::sync::Arc;

/// Presented frames, or time when nothing presents, an edit may take to show
/// before the next one goes anyway.
const PATIENCE: u32 = 30;
const PATIENCE_TIME: std::time::Duration = std::time::Duration::from_millis(500);

#[derive(Debug)]
struct Edit {
    identity: String,
    text: Option<String>,
    start: i32,
    end: i32,
}

#[derive(Debug, Default)]
pub(super) struct Edits {
    waiting: VecDeque<Edit>,
    /// Sent, not yet seen in a frame, and frames presented since.
    sent: Option<(Edit, u32)>,
    /// Bumped per sent edit to revoke its time limit.
    serial: u64,
}

impl Edit {
    fn shown_in(&self, node: &crate::scene::node::NodeSnapshot) -> bool {
        let Some(input) = &node.input else {
            return false;
        };
        let (text, selection) = match &input.edit {
            Some(edit) => (edit.text.as_str(), Some(edit.selection)),
            None => (input.value.as_str(), None),
        };
        match &self.text {
            Some(expected) => text == expected,
            None if self.start < 0 => true,
            None => selection == Some([self.start as u32, self.end as u32]),
        }
    }
}

impl<A: PlatformAdapter> Inner<A> {
    /// Queue `EditText` effects; the rest go to the platform as they are.
    pub(super) fn queue_edits(self: &Arc<Self>, effects: &mut Vec<Effect>) {
        if !effects.iter().any(|e| matches!(e, Effect::EditText { .. })) {
            return;
        }
        {
            let mut main = self.main();
            let Some(snapshot) = main.snapshot.clone() else {
                return;
            };
            effects.retain(|effect| {
                let Effect::EditText {
                    node,
                    text,
                    start,
                    end,
                } = effect
                else {
                    return true;
                };
                if let Some(node) = snapshot.frame.nodes.get(*node) {
                    main.edits.waiting.push_back(Edit {
                        identity: node.identity.clone(),
                        text: text.clone(),
                        start: *start,
                        end: *end,
                    });
                }
                false
            });
        }
        self.send_next_edit();
    }

    /// A frame reached the screen: a shown edit lets the next one go.
    pub(super) fn edits_presented(self: &Arc<Self>) {
        let next = {
            let mut main = self.main();
            let snapshot = main.snapshot.clone();
            let Some((edit, frames)) = main.edits.sent.as_mut() else {
                return;
            };
            *frames += 1;
            let shown = snapshot.is_some_and(|s| {
                s.frame
                    .nodes
                    .iter()
                    .any(|node| node.identity == edit.identity && edit.shown_in(node))
            });
            let stale = *frames > PATIENCE;
            if !(shown || stale) {
                return;
            }
            if stale && !shown {
                self.adapter.warn(&format!(
                    "accessibility edit of {} never showed",
                    edit.identity
                ));
            }
            main.edits.sent = None;
            true
        };
        if next {
            self.send_next_edit();
        }
    }

    fn send_next_edit(self: &Arc<Self>) {
        let effect = {
            let mut main = self.main();
            if main.edits.sent.is_some() {
                return;
            }
            let Some(snapshot) = main.snapshot.clone() else {
                return;
            };
            loop {
                let Some(edit) = main.edits.waiting.pop_front() else {
                    return;
                };
                // Nodes move between frames; an editor that left drops its edit.
                let Some(index) = snapshot
                    .frame
                    .nodes
                    .iter()
                    .position(|node| node.identity == edit.identity && node.input.is_some())
                else {
                    continue;
                };
                let effect = Effect::EditText {
                    node: index,
                    text: edit.text.clone(),
                    start: edit.start,
                    end: edit.end,
                };
                main.edits.sent = Some((edit, 0));
                main.edits.serial += 1;
                break (effect, main.edits.serial);
            }
        };
        let (effect, serial) = effect;
        self.adapter.apply(&[effect]);
        let host = self.clone();
        self.adapter.post_after(
            PATIENCE_TIME,
            Box::new(move || {
                let expired = {
                    let mut main = host.main();
                    let expired = main.edits.serial == serial && main.edits.sent.is_some();
                    if expired {
                        main.edits.sent = None;
                    }
                    expired
                };
                if expired {
                    host.send_next_edit();
                }
            }),
        );
    }
}
