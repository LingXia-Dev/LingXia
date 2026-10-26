//! Interaction effects and the worker dispatch they queue.
use super::{Guard, Inner};
use crate::PlotSelection;
use crate::interaction::view::{Dispatch, Effect};
use crate::platform::PlatformAdapter;
use crate::session::ViewSession;
use crate::session::lifecycle::PageTicket;
use std::sync::Arc;

type Activation = Box<dyn FnOnce(&mut ViewSession) -> Result<bool, String> + Send>;

impl<A: PlatformAdapter> Inner<A> {
    /// Run host-owned effects and hand the rest to the adapter, in order.
    pub(super) fn deliver(self: &Arc<Self>, effects: Vec<Effect>) {
        if effects.is_empty() {
            return;
        }
        let mut platform = Vec::new();
        let mut events = Vec::new();
        let (mut redraw, mut dispatch, mut number) = (false, false, false);
        let mut press = None;
        for effect in effects {
            match effect {
                Effect::Redraw => redraw = true,
                Effect::Dispatch => dispatch = true,
                Effect::Number => number = true,
                Effect::Accessibility(event) => events.push(event),
                // `active:` styles repaint in the session; only the transition is sent.
                Effect::Press {
                    styled: true,
                    identity,
                    ..
                } => press = Some(Some(identity)),
                Effect::ClearPress => {
                    press = Some(None);
                    platform.push(Effect::ClearPress);
                }
                other => platform.push(other),
            }
        }
        if !platform.is_empty() {
            self.adapter.apply(&platform);
        }
        if !events.is_empty() {
            self.adapter.announce(&events);
        }
        if let Some(identity) = press {
            self.set_pressed(identity);
        }
        if dispatch {
            self.drain_dispatch();
        }
        if number {
            self.drain_number();
        }
        if redraw {
            self.schedule();
        }
    }

    fn set_pressed(self: &Arc<Self>, identity: Option<String>) {
        {
            let mut main = self.main();
            if main.styled_press == identity {
                return;
            }
            main.styled_press = identity.clone();
        }
        self.work(
            Guard::Page(self.lifecycle.page()),
            move |_, session| Ok(session.set_pointer_state(identity.as_deref(), None)),
            |inner, result| match result {
                Some(Ok(true)) => inner.schedule(),
                Some(Err(error)) => inner.fail(error),
                _ => {}
            },
        );
    }

    /// Run queued activations against the frame the core resolved them on.
    fn drain_dispatch(self: &Arc<Self>) {
        let (dispatches, snapshot) = {
            let mut main = self.main();
            if main.busy || main.disposed {
                return;
            }
            (main.interaction.take_dispatch(), main.snapshot.clone())
        };
        let page = self.lifecycle.page();
        for dispatch in dispatches {
            match dispatch {
                Dispatch::Editor { .. } => {
                    if let Some(snapshot) = &snapshot {
                        self.adapter.focus(&dispatch, snapshot);
                    }
                }
                Dispatch::Click { commit, handle } => {
                    self.adapter.activation();
                    self.activate(
                        page,
                        true,
                        Box::new(move |session| session.click(commit, handle)),
                    );
                }
                Dispatch::Plot {
                    commit,
                    identity,
                    mode,
                    point,
                    index,
                } => {
                    self.adapter.activation();
                    let selection = match mode {
                        0 => PlotSelection::Point(point),
                        1 => PlotSelection::Next,
                        _ => PlotSelection::Index(index),
                    };
                    self.activate(
                        page,
                        true,
                        Box::new(move |session| session.select_plot(commit, &identity, selection)),
                    );
                }
                Dispatch::DismissPlots { commit, identities } => self.activate(
                    page,
                    false,
                    Box::new(move |session| {
                        let mut changed = false;
                        for identity in &identities {
                            changed |=
                                session.select_plot(commit, identity, PlotSelection::Index(-1))?;
                        }
                        Ok(changed)
                    }),
                ),
                Dispatch::SelectText {
                    commit,
                    identity,
                    point,
                } => {
                    self.adapter.activation();
                    self.activate(
                        page,
                        false,
                        Box::new(move |session| session.select_text(commit, &identity, point)),
                    );
                }
                Dispatch::ClearTextSelection => self.activate(
                    page,
                    false,
                    Box::new(|session| Ok(session.clear_text_selection())),
                ),
                reach @ Dispatch::Reach { .. } => self.reach(page, reach),
            }
        }
    }

    /// Run an activation; a changed View drains Logic actions and rebuilds.
    fn activate(self: &Arc<Self>, page: PageTicket, announce: bool, activation: Activation) {
        self.work(
            Guard::Page(page),
            move |inner, session| {
                if !activation(session)? {
                    return Ok(None);
                }
                inner.drained(session).map(Some)
            },
            move |inner, result| match result {
                Some(Ok(Some(update))) => {
                    if let Some((connection, update)) = update {
                        inner.publish(connection, update);
                    }
                    if inner.main().disposed || !inner.lifecycle.is_current_page(page) {
                        return;
                    }
                    if announce {
                        inner.adapter.clicked();
                    }
                    inner.main().dirty = true;
                    inner.schedule();
                }
                Some(Err(error)) if inner.lifecycle.is_current_page(page) => inner.fail(error),
                _ => {}
            },
        );
    }

    /// Edges reached by a presented frame; queued before the next preparation
    /// so generation validation sees the frame that reached them.
    pub(super) fn reach(self: &Arc<Self>, page: PageTicket, reach: Dispatch) {
        let Dispatch::Reach {
            commit,
            handles,
            more,
        } = reach
        else {
            return;
        };
        self.work(
            Guard::Page(page),
            move |inner, session| {
                let mut changed = false;
                for handle in handles {
                    changed |= session.click(commit, handle)?;
                }
                let update = if changed {
                    inner.drained(session)?
                } else {
                    None
                };
                Ok((changed, update))
            },
            move |inner, result| match result {
                Some(Ok((changed, update))) => {
                    if let Some((connection, update)) = update {
                        inner.publish(connection, update);
                    }
                    if (changed || more)
                        && !inner.main().disposed
                        && inner.lifecycle.is_current_page(page)
                    {
                        inner.main().dirty = true;
                        inner.schedule();
                    }
                }
                Some(Err(error)) if inner.lifecycle.is_current_page(page) => inner.fail(error),
                _ => {}
            },
        );
    }

    /// One slider proposal at a time; the next is taken once this one settles.
    fn drain_number(self: &Arc<Self>) {
        let proposal = {
            let mut main = self.main();
            if main.busy || main.number_busy || main.disposed {
                return;
            }
            let proposal = main.interaction.take_number();
            main.number_busy = proposal.is_some();
            proposal
        };
        let Some((commit, handle, value, proposal)) = proposal else {
            return;
        };
        let page = proposal.page;
        self.work(
            Guard::Page(page),
            move |inner, session| {
                if !session.number_event(commit, handle, value)? {
                    return Ok(None);
                }
                inner.drained(session).map(Some)
            },
            move |inner, result| {
                let pending = {
                    let mut main = inner.main();
                    main.number_busy = false;
                    if main.disposed {
                        return;
                    }
                    main.interaction.number_done();
                    main.pending
                };
                match result {
                    Some(Err(error)) => {
                        let effects = inner.main().interaction.cancel_all();
                        inner.deliver(effects);
                        if inner.lifecycle.is_current_page(page) {
                            inner.fail(error);
                        }
                    }
                    Some(Ok(Some(update))) => {
                        if let Some((connection, update)) = update {
                            inner.publish(connection, update);
                        }
                        inner.main().dirty = true;
                        inner.schedule();
                    }
                    _ if pending => inner.schedule(),
                    _ => inner.drain_number(),
                }
            },
        );
    }

    /// An input-method request; edits that need the View drain Logic actions.
    pub(super) fn input(
        self: &Arc<Self>,
        request: String,
        reply: Box<dyn FnOnce(Option<String>) + Send>,
    ) {
        let page = self.lifecycle.page();
        self.work(
            Guard::Page(page),
            move |inner, session| {
                let parsed: serde_json::Value =
                    serde_json::from_str(&request).unwrap_or_default();
                let edit = parsed["op"] == "edit";
                let result = session.input_request(&request);
                if result.is_err() && edit {
                    // Reject later queued edits against the failed mirror before they run.
                    let blur = serde_json::json!({"op":"blur","identity":parsed["identity"],"token":parsed["token"]});
                    let _ = session.input_request(&blur.to_string());
                }
                let needs_view = result.as_ref().map_or(true, |reply| {
                    serde_json::from_str::<serde_json::Value>(reply)
                        .ok()
                        .and_then(|reply| reply.get("needs_view_render").and_then(|v| v.as_bool()))
                        .unwrap_or(false)
                });
                let update = if edit && needs_view {
                    inner.drained(session)?
                } else {
                    None
                };
                Ok((result, update))
            },
            move |inner, result| {
                let Some(result) = result else {
                    return;
                };
                let (value, update, error) = match result {
                    Ok((Ok(value), update)) => (Some(value), update, None),
                    Ok((Err(error), update)) => (None, update, Some(error)),
                    Err(error) => (None, None, Some(error)),
                };
                if let Some((connection, update)) = update {
                    inner.publish(connection, update);
                }
                if let Some(error) = error
                    && inner.lifecycle.is_current_page(page)
                {
                    inner.fail(error);
                }
                if !inner.main().disposed && inner.lifecycle.is_current_page(page) {
                    reply(value);
                }
            },
        );
    }
}
