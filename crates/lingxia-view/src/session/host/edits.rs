//! Accessibility and automation edits reach the platform input method one at a
//! time. The platform says whether each applied; one whose editor is still
//! connecting is sent again shortly, and the next waits until a presented frame
//! shows the previous applied. An automation call that queued an edit settles
//! once a frame shows it, or with the reason it never will.
use super::Inner;
use crate::interaction::view::Effect;
use crate::platform::{EditOutcome, PlatformAdapter};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

/// Presented frames, or time when nothing presents, an applied edit may take to show.
const PATIENCE: u32 = 30;
const PATIENCE_TIME: Duration = Duration::from_millis(500);
/// How soon an edit whose editor was connecting is offered again, and for how long.
const RETRY: Duration = Duration::from_millis(50);
const CONNECT_TIME: Duration = Duration::from_secs(3);
/// Sends of an applied edit that never shows before it is given up.
const ATTEMPTS: u32 = 4;

#[derive(Clone, Debug)]
struct Edit {
    identity: String,
    text: Option<String>,
    start: i32,
    end: i32,
    sends: u32,
    /// Platform clock time of the first not-ready answer.
    waiting_since: Option<Duration>,
    /// The automation call this edit completes.
    call: Option<u64>,
}

#[derive(Debug, Default)]
pub(super) struct Edits {
    waiting: VecDeque<Edit>,
    /// Applied, not yet seen in a frame, and frames presented since.
    sent: Option<(Edit, u32)>,
    /// A not-ready edit waits for its retry.
    retrying: bool,
    /// Bumped per send to revoke its timers.
    serial: u64,
    /// An edit failed outright while the current call delivered it.
    failed: bool,
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
    /// Take `EditText` effects into the queue; the rest go to the platform as they are.
    pub(super) fn queue_edits(self: &Arc<Self>, effects: &mut Vec<Effect>) {
        if !effects.iter().any(|e| matches!(e, Effect::EditText { .. })) {
            return;
        }
        {
            let mut main = self.main();
            let Some(snapshot) = main.snapshot.clone() else {
                effects.retain(|e| !matches!(e, Effect::EditText { .. }));
                return;
            };
            // The call that asked for these edits (a fill) settles when the last one shows.
            let call = main.act_call.take().map(|(call, _)| call);
            let queued = main.edits.waiting.len();
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
                        sends: 0,
                        waiting_since: None,
                        call: None,
                    });
                }
                false
            });
            match main.edits.waiting.iter_mut().skip(queued).last() {
                Some(last) => last.call = call,
                None => main.act_call = call.map(|call| (call, String::new())),
            }
        }
        self.send_next_edit();
    }

    /// Whether an edit failed outright since the last call; clears the flag.
    pub(super) fn take_edit_failure(&self) -> bool {
        std::mem::take(&mut self.main().edits.failed)
    }

    /// A frame reached the screen: a shown edit lets the next one go, and a
    /// connecting editor may be ready now.
    pub(super) fn edits_presented(self: &Arc<Self>) {
        let unshown = {
            let mut main = self.main();
            let snapshot = main.snapshot.clone();
            let retrying = main.edits.retrying;
            match main.edits.sent.as_mut() {
                None if retrying => {
                    main.edits.retrying = false;
                    None
                }
                None => return,
                Some((edit, frames)) => {
                    *frames += 1;
                    let shown = snapshot.is_some_and(|s| {
                        s.frame
                            .nodes
                            .iter()
                            .any(|node| node.identity == edit.identity && edit.shown_in(node))
                    });
                    if shown {
                        let call = edit.call;
                        main.edits.sent = None;
                        settle(&mut main, call, Ok(serde_json::Value::Null));
                        None
                    } else if *frames > PATIENCE {
                        main.edits.sent.take().map(|(edit, _)| edit)
                    } else {
                        return;
                    }
                }
            }
        };
        self.edit_unshown(unshown);
    }

    /// An applied edit that never showed is sent again, a few times.
    fn edit_unshown(self: &Arc<Self>, unshown: Option<Edit>) {
        if let Some(edit) = unshown {
            if edit.sends < ATTEMPTS {
                self.main().edits.waiting.push_front(edit);
            } else {
                self.adapter.warn(&format!(
                    "accessibility edit of {} never showed after {ATTEMPTS} sends",
                    edit.identity
                ));
                settle(
                    &mut self.main(),
                    edit.call,
                    Err(format!(
                        "E_EDIT_FAILED: {} never showed the edit",
                        edit.identity
                    )),
                );
            }
        }
        self.send_next_edit();
    }

    fn send_next_edit(self: &Arc<Self>) {
        loop {
            let (mut edit, index, serial) = {
                let mut main = self.main();
                if main.disposed {
                    main.edits = Edits::default();
                    return;
                }
                if main.edits.sent.is_some() || main.edits.retrying {
                    return;
                }
                let Some(snapshot) = main.snapshot.clone() else {
                    return;
                };
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
                    let reason = format!("E_EDIT_FAILED: {} left before the edit", edit.identity);
                    settle(&mut main, edit.call, Err(reason));
                    continue;
                };
                main.edits.serial += 1;
                (edit, index, main.edits.serial)
            };
            let outcome = self
                .adapter
                .edit_text(index, edit.text.as_deref(), edit.start, edit.end);
            match outcome {
                EditOutcome::Applied => {
                    edit.sends += 1;
                    self.main().edits.sent = Some((edit, 0));
                    self.edit_timer(serial, PATIENCE_TIME);
                    return;
                }
                EditOutcome::NotReady => {
                    let now = self.adapter.now();
                    let since = *edit.waiting_since.get_or_insert(now);
                    if now.saturating_sub(since) >= CONNECT_TIME {
                        self.adapter
                            .warn(&format!("editor {} never became ready", edit.identity));
                        let mut main = self.main();
                        main.edits.failed = true;
                        let reason = format!("E_EDIT_FAILED: {} never became ready", edit.identity);
                        settle(&mut main, edit.call, Err(reason));
                        continue;
                    }
                    {
                        let mut main = self.main();
                        main.edits.waiting.push_front(edit);
                        main.edits.retrying = true;
                    }
                    self.edit_timer(serial, RETRY);
                    return;
                }
                EditOutcome::Failed => {
                    let mut main = self.main();
                    main.edits.failed = true;
                    let reason = format!("E_EDIT_FAILED: {} refused the edit", edit.identity);
                    settle(&mut main, edit.call, Err(reason));
                    continue;
                }
            }
        }
    }

    /// Frames stop when nothing changes; time moves a pending edit along.
    fn edit_timer(self: &Arc<Self>, serial: u64, delay: Duration) {
        let host = self.clone();
        self.adapter.post_after(
            delay,
            Box::new(move || {
                let unshown = {
                    let mut main = host.main();
                    if main.edits.serial != serial {
                        return;
                    }
                    main.edits.retrying = false;
                    main.edits.sent.take().map(|(edit, _)| edit)
                };
                host.edit_unshown(unshown);
            }),
        );
    }
}

/// Report automation call `call`, if one waits on this edit.
fn settle<F>(
    main: &mut super::Main<F>,
    call: Option<u64>,
    outcome: Result<serde_json::Value, String>,
) {
    if let Some(call) = call
        && let Some(slot) = main.automation_calls.get_mut(&call)
    {
        *slot = Some(outcome);
    }
}
