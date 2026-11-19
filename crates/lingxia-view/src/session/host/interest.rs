//! Interest invokers: a pointer resting on an `interestFor` button, or a key focusing it, shows
//! its hint after a delay; losing interest hides it after a grace, so the pointer can cross the
//! gap into the hint. The View only learns the outcome. A press anywhere but on a shown hint or
//! its control closes it, and the press still reaches what it landed on.
use super::{Guard, Inner};
use crate::platform::PlatformAdapter;
use std::sync::Arc;
use std::time::Duration;

const SHOW: Duration = Duration::from_millis(500);
const HIDE: Duration = Duration::from_millis(250);

#[derive(Default)]
pub(super) struct Interest {
    /// The trigger the pointer rests on, or the shown one while the pointer is over its hint.
    hovered: Option<String>,
    /// The trigger focused from a key.
    focused: Option<String>,
    /// The trigger whose hint this state last showed.
    shown: Option<String>,
    /// Bumped by every change, so only the latest delay acts.
    timer: u64,
    /// A test's pinned hover (styled element, trigger); pointer moves do not replace it.
    pinned: Option<(Option<String>, Option<String>)>,
    /// What the real pointer last hovered, restored when the pin lifts.
    pointer: (Option<String>, Option<String>),
}

impl Interest {
    pub(super) fn shown(&self) -> Option<&str> {
        self.shown.as_deref()
    }

    /// The page reloaded or parked: its hints are gone. The timer stays, revoking any delay.
    pub(super) fn reset(&mut self) {
        (self.hovered, self.focused, self.shown) = (None, None, None);
        (self.pinned, self.pointer) = (None, (None, None));
        self.timer += 1;
    }
}

impl<A: PlatformAdapter> Inner<A> {
    /// The window lost focus or the page hid: every hint goes after the grace.
    pub(super) fn lose_interest(self: &Arc<Self>) {
        {
            let mut main = self.main();
            main.interest.hovered = main.interest.pinned.as_ref().and_then(|pin| pin.1.clone());
            main.interest.focused = None;
        }
        self.interest_changed();
    }

    /// The real pointer rests on `identity` (styled) and `trigger` (`interestFor`), or neither.
    pub(super) fn pointer_hovered(
        self: &Arc<Self>,
        identity: Option<String>,
        trigger: Option<String>,
    ) {
        {
            let mut main = self.main();
            main.interest.pointer = (identity.clone(), trigger.clone());
            if main.interest.pinned.is_some() {
                return;
            }
        }
        self.set_hovered(identity);
        self.interest_hovered(trigger);
    }

    /// Hold the hover on `pin` (styled element, trigger) as a resting pointer would, or lift it
    /// back to what the real pointer last hovered.
    pub(super) fn pin_hover(self: &Arc<Self>, pin: Option<(Option<String>, Option<String>)>) {
        let (identity, trigger) = {
            let mut main = self.main();
            main.interest.pinned = pin.clone();
            pin.unwrap_or_else(|| main.interest.pointer.clone())
        };
        self.set_hovered(identity);
        self.interest_hovered(trigger);
    }

    /// A press at a device-pixel point closes every open hint it is not on, nor on its control.
    pub(super) fn dismiss_hints_at(self: &Arc<Self>, point: [f32; 2]) {
        let keep = {
            let main = self.main();
            let Some(frame) = main.interaction.frame() else {
                return;
            };
            if !frame.nodes.iter().any(|node| node.hint.is_some()) {
                return;
            }
            let scale = if frame.scale > 0.0 { frame.scale } else { 1.0 };
            let point = [point[0] / scale, point[1] / scale];
            let on = |identity: &str| {
                frame.nodes.iter().any(|node| {
                    let own = &*node.identity;
                    (own == identity
                        || own.len() > identity.len()
                            && own.starts_with(identity)
                            && own.as_bytes()[identity.len()] == b'/')
                        && crate::interaction::frame::contains(node, point, false)
                })
            };
            frame
                .nodes
                .iter()
                .filter_map(|node| {
                    let (id, invoker) = node.hint.as_ref()?;
                    // The layer itself may span the page; only its panel counts as the hint.
                    let panel = frame.nodes.iter().any(|child| {
                        let own = &*child.identity;
                        own.len() > node.identity.len()
                            && own.starts_with(&*node.identity)
                            && own.as_bytes()[node.identity.len()] == b'/'
                            && crate::interaction::frame::contains(child, point, false)
                    });
                    (panel || invoker.as_deref().is_some_and(on)).then(|| id.clone())
                })
                .collect::<Vec<_>>()
        };
        {
            // The press decides now; a delayed interest show must not reopen what it closed.
            let mut main = self.main();
            main.interest.timer += 1;
            let kept = main.interest.shown.as_deref().is_some_and(|trigger| {
                main.interaction.frame().is_some_and(|frame| {
                    frame
                        .nodes
                        .iter()
                        .find(|node| &*node.identity == trigger)
                        .and_then(|node| node.interest_for.as_ref())
                        .is_some_and(|target| keep.contains(target))
                })
            });
            if !kept {
                main.interest.shown = None;
                main.interest.hovered = None;
            }
        }
        self.work(
            Guard::Page(self.lifecycle.page()),
            move |_, session| session.dismiss_hints(&keep),
            |inner, result| {
                if matches!(result, Some(Ok(true))) && !inner.main().disposed {
                    inner.main().dirty = true;
                    inner.schedule();
                }
            },
        );
    }

    /// The View names a trigger by its hint's id and its place among that hint's triggers,
    /// since the View's identities are not the core's.
    fn interest_trigger(&self, identity: &str) -> Option<(String, usize)> {
        let main = self.main();
        let nodes = &main.interaction.frame()?.nodes;
        let at = nodes.iter().position(|node| &*node.identity == identity)?;
        let target = nodes[at].interest_for.clone()?;
        let nth = nodes[..at]
            .iter()
            .filter(|node| node.interest_for.as_deref() == Some(&target))
            .count();
        Some((target, nth))
    }

    pub(super) fn interest_hovered(self: &Arc<Self>, trigger: Option<String>) {
        {
            let mut main = self.main();
            if main.interest.hovered == trigger {
                return;
            }
            main.interest.hovered = trigger;
        }
        self.interest_changed();
    }

    pub(super) fn interest_focused(self: &Arc<Self>, trigger: Option<String>) {
        {
            let mut main = self.main();
            if main.interest.focused == trigger {
                return;
            }
            main.interest.focused = trigger;
        }
        self.interest_changed();
    }

    fn interest_changed(self: &Arc<Self>) {
        let (wanted, token) = {
            let mut main = self.main();
            let interest = &mut main.interest;
            interest.timer += 1;
            let wanted = interest
                .hovered
                .clone()
                .or_else(|| interest.focused.clone());
            // Escape or a tap may have closed the shown hint in the View, so a wanted one is
            // shown again (a no-op when still open); nothing wanted and nothing shown is settled.
            if wanted.is_none() && interest.shown.is_none() {
                return;
            }
            (wanted, interest.timer)
        };
        let delay = if wanted.is_some() { SHOW } else { HIDE };
        let (inner, page) = (self.clone(), self.lifecycle.page());
        self.adapter.post_after(
            delay,
            Box::new(move || {
                if !inner.lifecycle.is_current_page(page) {
                    return;
                }
                let previous = {
                    let mut main = inner.main();
                    if main.disposed || main.interest.timer != token {
                        return;
                    }
                    std::mem::replace(&mut main.interest.shown, wanted.clone())
                };
                // Showing a hint closes the previous one in the View.
                let (trigger, show) = match (wanted, previous) {
                    (Some(trigger), _) => (trigger, true),
                    (None, Some(trigger)) => (trigger, false),
                    (None, None) => return,
                };
                let Some((target, nth)) = inner.interest_trigger(&trigger) else {
                    return;
                };
                inner.work(
                    Guard::Page(page),
                    move |_, session| session.interest(&target, nth, show),
                    |inner, result| {
                        if matches!(result, Some(Ok(true))) && !inner.main().disposed {
                            inner.main().dirty = true;
                            inner.schedule();
                        }
                    },
                );
            }),
        );
    }
}
