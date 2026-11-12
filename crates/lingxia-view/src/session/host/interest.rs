//! Interest invokers: a pointer resting on an `interestFor` button, or a key focusing it, shows
//! its hint after a delay; losing interest hides it after a grace, so the pointer can cross the
//! gap into the hint. The View only learns the outcome.
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
}

impl Interest {
    pub(super) fn shown(&self) -> Option<&str> {
        self.shown.as_deref()
    }

    /// The page reloaded or parked: its hints are gone. The timer stays, revoking any delay.
    pub(super) fn reset(&mut self) {
        (self.hovered, self.focused, self.shown) = (None, None, None);
        self.timer += 1;
    }
}

impl<A: PlatformAdapter> Inner<A> {
    /// The window lost focus or the page hid: every hint goes after the grace.
    pub(super) fn lose_interest(self: &Arc<Self>) {
        {
            let mut main = self.main();
            main.interest.hovered = None;
            main.interest.focused = None;
        }
        self.interest_changed();
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
