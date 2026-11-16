//! `closeAfter` popovers: each open one counts down only while the page shows it and nothing
//! holds it (a press, a resting pointer or focus inside), then closes as a light dismiss would.
//! A paused countdown keeps its remainder, so toasts shown together never close together late.
use super::{Guard, Inner};
use crate::platform::PlatformAdapter;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[derive(Default)]
pub(super) struct Autoclose {
    /// Open countdowns by their top layer's identity.
    clocks: HashMap<Arc<str>, Countdown>,
    /// The fine pointer resting on the page, and a press held down, in logical points.
    pointer: Option<[f32; 2]>,
    press: Option<[f32; 2]>,
    /// Bumped by every pause, resume and reset, so only the latest wakeup acts.
    token: u64,
}

struct Countdown {
    id: String,
    ms: u32,
    spent: Duration,
    /// Running since this host time; `None` while paused.
    since: Option<Duration>,
    token: u64,
    expired: bool,
}

impl Countdown {
    fn remaining(&self, now: Duration) -> Duration {
        let spent = self.spent + self.since.map_or(Duration::ZERO, |since| now - since);
        Duration::from_millis(self.ms.into()).saturating_sub(spent)
    }
}

impl Autoclose {
    /// The page reloaded or parked: its popovers are gone.
    pub(super) fn reset(&mut self) {
        self.clocks.clear();
        (self.pointer, self.press) = (None, None);
        self.token += 1;
    }

    pub(super) fn set_pointer(&mut self, point: Option<[f32; 2]>) {
        self.pointer = point;
    }

    pub(super) fn set_press(&mut self, point: Option<[f32; 2]>) {
        self.press = point;
    }
}

impl<A: PlatformAdapter> Inner<A> {
    /// Start, pause or resume every countdown against the presented frame and what holds it.
    pub(super) fn sync_autoclose(self: &Arc<Self>) {
        let now = self.adapter.now();
        let stopped = self.window_unwatched();
        let mut wake = Vec::new();
        {
            let mut main = self.main();
            let main = &mut *main;
            let Some(frame) = main.interaction.frame() else {
                if !main.autoclose.clocks.is_empty() {
                    main.autoclose.reset();
                }
                return;
            };
            let running = main.visible && !stopped && !main.interaction.parked();
            let open: Vec<(Arc<str>, String, u32)> = frame
                .nodes
                .iter()
                .filter_map(|node| {
                    let (id, ms) = node.close_after.as_ref()?;
                    Some((Arc::<str>::from(&*node.identity), id.clone(), *ms))
                })
                .collect();
            if open.is_empty() && main.autoclose.clocks.is_empty() {
                return;
            }
            let state = &mut main.autoclose;
            state.clocks.retain(|identity, clock| {
                open.iter()
                    .any(|(own, id, ms)| own == identity && *id == clock.id && *ms == clock.ms)
            });
            for (identity, id, ms) in open {
                let inside = |point: Option<[f32; 2]>| {
                    point.is_some_and(|point| {
                        frame.nodes.iter().any(|child| {
                            let own = &*child.identity;
                            own.len() > identity.len()
                                && own.starts_with(&*identity)
                                && own.as_bytes()[identity.len()] == b'/'
                                && crate::interaction::frame::contains(child, point, false)
                        })
                    })
                };
                let held = inside(state.press)
                    || inside(state.pointer)
                    || main.interaction.focus_within(&identity);
                let clock = state.clocks.entry(identity.clone()).or_insert(Countdown {
                    id,
                    ms,
                    spent: Duration::ZERO,
                    since: None,
                    token: 0,
                    expired: false,
                });
                if clock.expired {
                    continue;
                }
                match (running && !held, clock.since) {
                    (true, None) => {
                        state.token += 1;
                        clock.since = Some(now);
                        clock.token = state.token;
                        wake.push((identity, clock.token, clock.remaining(now)));
                    }
                    (false, Some(since)) => {
                        state.token += 1;
                        clock.spent += now - since;
                        clock.since = None;
                        clock.token = state.token;
                    }
                    _ => {}
                }
            }
        }
        for (identity, token, delay) in wake {
            self.autoclose_after(identity, token, delay);
        }
    }

    fn autoclose_after(self: &Arc<Self>, identity: Arc<str>, token: u64, delay: Duration) {
        let (inner, page) = (self.clone(), self.lifecycle.page());
        self.adapter.post_after(
            delay,
            Box::new(move || {
                if !inner.lifecycle.is_current_page(page) {
                    return;
                }
                let now = inner.adapter.now();
                let id = {
                    let mut main = inner.main();
                    if main.disposed {
                        return;
                    }
                    let Some(clock) = main.autoclose.clocks.get_mut(&identity) else {
                        return;
                    };
                    if clock.token != token || clock.since.is_none() || clock.expired {
                        return;
                    }
                    let left = clock.remaining(now);
                    if !left.is_zero() {
                        drop(main);
                        inner.autoclose_after(identity, token, left);
                        return;
                    }
                    clock.expired = true;
                    clock.id.clone()
                };
                inner.work(
                    Guard::Page(page),
                    move |_, session| session.close_after(&id),
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
