//! Interaction effects and the worker dispatch they queue.
use super::{Guard, Inner};
use crate::PlotSelection;
use crate::interaction::view::{Dispatch, Effect};
use crate::platform::PlatformAdapter;
use crate::session::ViewSession;
use crate::session::lifecycle::PageTicket;
use std::sync::Arc;

type Activation = Box<dyn FnOnce(&mut ViewSession) -> Result<bool, String> + Send>;

/// What one queued input did: the clicks it ran and whether the View changed.
type Ran = Option<Result<(Vec<crate::session::ClickRecord>, bool), String>>;

/// Main-thread follow-up of one queued input; `rendered` when the frame that ran it
/// already shows its change.
type Done<A> = Box<dyn FnOnce(&Arc<Inner<A>>, Ran, bool) + Send>;

/// Input waiting for the worker.
pub(super) struct Queued<A: PlatformAdapter> {
    page: PageTicket,
    run: Activation,
    done: Done<A>,
}

/// Queued input a worker job ran, for the main thread.
pub(super) struct InputRun<A: PlatformAdapter> {
    ran: Vec<(Done<A>, Ran)>,
    /// Some input changed the View.
    pub(super) changed: bool,
    update: Result<super::bridge::Drained, String>,
}

impl<A: PlatformAdapter> Inner<A> {
    /// Run host-owned effects and hand the rest to the adapter, in order.
    pub(super) fn deliver(self: &Arc<Self>, mut effects: Vec<Effect>) {
        // Pointer, keys, assistive input and frames all hand drag releases over here.
        self.drag_later();
        self.snap_later();
        self.queue_edits(&mut effects);
        if effects.is_empty() {
            return;
        }
        let mut platform = Vec::new();
        let mut events = Vec::new();
        let (mut redraw, mut dispatch, mut number) = (false, false, false);
        let mut press = None;
        let mut focus = None;
        for effect in effects {
            match effect {
                // A styled `<select>` repaints its `focus:` delta in the session; the ring still draws.
                Effect::FocusRing { node } => {
                    focus = Some(self.main().interaction.styled_focus(node));
                    platform.push(effect);
                }
                Effect::ClearFocusRing => {
                    focus = Some(None);
                    platform.push(effect);
                }
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
        if let Some(identity) = focus {
            self.set_control_focus(identity);
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
        let hovered = {
            let mut main = self.main();
            if main.styled_press == identity {
                return;
            }
            main.styled_press = identity.clone();
            main.styled_hover.clone()
        };
        self.apply_pointer_state(identity, hovered);
    }

    pub(super) fn set_hovered(self: &Arc<Self>, identity: Option<String>) {
        let pressed = {
            let mut main = self.main();
            if main.styled_hover == identity {
                return;
            }
            main.styled_hover = identity.clone();
            main.styled_press.clone()
        };
        self.apply_pointer_state(pressed, identity);
    }

    fn apply_pointer_state(self: &Arc<Self>, pressed: Option<String>, hovered: Option<String>) {
        self.work(
            Guard::Page(self.lifecycle.page()),
            move |_, session| Ok(session.set_pointer_state(pressed.as_deref(), hovered.as_deref())),
            |inner, result| match result {
                Some(Ok(true)) => inner.schedule(),
                Some(Err(error)) => inner.fail(error),
                _ => {}
            },
        );
    }

    fn set_control_focus(self: &Arc<Self>, identity: Option<String>) {
        {
            let mut main = self.main();
            if main.styled_focus == identity {
                return;
            }
            main.styled_focus = identity.clone();
        }
        self.work(
            Guard::Page(self.lifecycle.page()),
            move |_, session| Ok(session.set_control_focus(identity.as_deref())),
            |inner, result| match result {
                Some(Ok(true)) => inner.schedule(),
                Some(Err(error)) => inner.fail(error),
                _ => {}
            },
        );
    }

    /// Resolve activations against the presented frame and queue them for the worker at
    /// once, also while a frame is in flight: input never waits for an idle moment.
    pub(super) fn drain_dispatch(self: &Arc<Self>) {
        let (dispatches, dropped, snapshot) = {
            let mut main = self.main();
            if main.disposed {
                return;
            }
            let dispatches = main.interaction.take_dispatch();
            (
                dispatches,
                main.interaction.take_dropped(),
                main.snapshot.clone(),
            )
        };
        let page = self.lifecycle.page();
        for dropped in dropped {
            self.report_dropped(
                page,
                &dropped.identity,
                dropped.reason,
                dropped.commit,
                dropped.frame,
            );
        }
        for dispatch in dispatches {
            match dispatch {
                Dispatch::Editor { node, commit, .. }
                    if let Some(identity) = snapshot.as_ref().and_then(|snapshot| {
                        let node = snapshot.frame.nodes.get(node)?;
                        node.input.as_ref()?.attributes.picker.as_ref()?;
                        Some(node.identity.clone())
                    }) =>
                {
                    // A date or time editor opens the View's own picker; no input method connects.
                    self.activate(
                        page,
                        false,
                        Box::new(move |session| session.open_picker(commit, &identity)),
                    );
                }
                Dispatch::Editor { node, word, .. } => {
                    if let Some(snapshot) = &snapshot {
                        self.adapter.focus(&dispatch, snapshot);
                        // A long-press word selection offers the editor's menu.
                        if word && let Some(node) = snapshot.frame.nodes.get(node) {
                            self.request_editor_menu(node.identity.clone());
                        }
                    }
                }
                Dispatch::Click { commit, handle } => {
                    self.adapter.activation();
                    let identity = snapshot
                        .as_ref()
                        .and_then(|snapshot| {
                            snapshot
                                .frame
                                .nodes
                                .iter()
                                .find(|node| node.on_click == Some(handle))
                        })
                        .map(|node| node.identity.clone())
                        .unwrap_or_default();
                    self.activate_reporting(
                        page,
                        true,
                        identity.clone(),
                        Box::new(move |session| session.click_control(commit, handle, &identity)),
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
                Dispatch::DragSelectText {
                    commit,
                    identity,
                    from,
                    to,
                } => {
                    self.adapter.activation();
                    self.activate(
                        page,
                        false,
                        Box::new(move |session| {
                            session.drag_select_text(commit, &identity, from, to)
                        }),
                    );
                }
                Dispatch::Select {
                    commit,
                    identity,
                    index,
                    value,
                } => self.activate(
                    page,
                    false,
                    Box::new(move |session| {
                        session.choose_option(commit, &identity, index, &value)
                    }),
                ),
                Dispatch::SubmitEditor { commit, identity } => self.activate(
                    page,
                    true,
                    Box::new(move |session| session.submit_editor(commit, &identity)),
                ),
                Dispatch::SnapChange {
                    commit,
                    handle,
                    region,
                    index,
                } => self.activate(
                    page,
                    false,
                    Box::new(move |session| session.snap_event(commit, handle, &region, index)),
                ),
                Dispatch::DragEvent {
                    commit,
                    handle,
                    identity,
                    payload,
                } => self.activate(
                    page,
                    false,
                    Box::new(move |session| {
                        session.drag_event(commit, handle, &identity, &payload)
                    }),
                ),
                Dispatch::Escape => {
                    self.activate(page, false, Box::new(|session| session.escape()))
                }
                Dispatch::PickerKey { key } => self.activate(
                    page,
                    false,
                    Box::new(move |session| session.picker_key(&key)),
                ),
                Dispatch::OpenSelect { commit, identity } => self.activate(
                    page,
                    false,
                    Box::new(move |session| session.open_select(commit, &identity)),
                ),
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
    pub(super) fn activate(
        self: &Arc<Self>,
        page: PageTicket,
        announce: bool,
        activation: Activation,
    ) {
        self.activate_reporting(page, announce, String::new(), activation);
    }

    /// Like `activate`, naming `identity` when the session refuses a superseded click.
    fn activate_reporting(
        self: &Arc<Self>,
        page: PageTicket,
        announce: bool,
        identity: String,
        activation: Activation,
    ) {
        let act = {
            let mut main = self.main();
            main.activating += 1;
            main.act_call.take()
        };
        self.queue_input(
            page,
            activation,
            Box::new(move |inner, result, rendered| {
                inner.main().activating -= 1;
                let mut dropped = None;
                let result = result.map(|result| {
                    result.map(|(clicks, changed)| {
                        for click in clicks {
                            dropped = dropped.or(match click.outcome {
                                Ok(false) => Some("its callback is no longer registered"),
                                Err(reason) => Some(reason),
                                Ok(true) => None,
                            });
                            inner.report_click(page, &identity, &click);
                        }
                        changed
                    })
                });
                if let Some((call, css)) = act {
                    let outcome = match (&result, dropped) {
                        (Some(Err(error)), _) => Err(error.clone()),
                        (None, _) => Err("E_PAGE_NOT_READY: the page went away".into()),
                        (_, Some(reason)) => Err(format!("E_ACT_DROPPED: {css} {reason}")),
                        _ => Ok(serde_json::Value::Null),
                    };
                    if let Some(slot) = inner.main().automation_calls.get_mut(&call) {
                        *slot = Some(outcome);
                    }
                }
                inner.activated(page, announce, result, rendered)
            }),
        );
    }

    /// Queue input for the worker; the next job of this page runs it before anything else.
    fn queue_input(self: &Arc<Self>, page: PageTicket, run: Activation, done: Done<A>) {
        self.input
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push_back(Queued { page, run, done });
        {
            // Two frames drawn before this input would only delay it; a fresh one replaces
            // both, as the session builds ahead only on a frame that will present.
            let mut main = self.main();
            main.input_seq += 1;
            if main.next_prepared.take().is_some() {
                main.prepared = None;
                main.dirty = true;
            }
        }
        // A frame starting now runs the input first; this job runs it if no frame did.
        self.schedule();
        self.work(
            Guard::Page(page),
            |inner, session| Ok(inner.run_input(session)),
            move |inner, result| match result {
                Some(Ok(run)) => inner.input_ran(run, false),
                Some(Err(error)) => {
                    inner.abandon_input(false);
                    if inner.lifecycle.is_current_page(page) {
                        inner.fail(error);
                    }
                }
                None => inner.abandon_input(true),
            },
        );
    }

    /// Run queued input against `session`, oldest first. Worker only.
    pub(super) fn run_input(self: &Arc<Self>, session: &mut ViewSession) -> InputRun<A> {
        let queued: Vec<_> = self
            .input
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .drain(..)
            .collect();
        let mut changed = false;
        let mut ran = Vec::with_capacity(queued.len());
        for Queued { page, run, done } in queued {
            let result = self.lifecycle.is_current_page(page).then(|| {
                let result =
                    run(session).and_then(|changed| Ok(session.apply_picker_results()? || changed));
                let clicks = session.take_clicks();
                result.map(|c| (clicks, c))
            });
            changed |= matches!(result, Some(Ok((_, true))));
            ran.push((done, result));
        }
        let update = if changed {
            self.drained(session)
        } else {
            Ok(None)
        };
        InputRun {
            ran,
            changed,
            update,
        }
    }

    /// Finish queued input on the main thread; `rendered` when the frame that ran it
    /// already rebuilt with its change.
    pub(super) fn input_ran(self: &Arc<Self>, run: InputRun<A>, rendered: bool) {
        let InputRun { ran, update, .. } = run;
        match update {
            Ok(Some((connection, mut update))) => {
                // The frame that ran the input renders what it changed.
                if rendered {
                    update.changed = false;
                }
                self.publish(connection, update);
            }
            Ok(None) => {}
            Err(error) if self.lifecycle.is_current_page(self.lifecycle.page()) => self.fail(error),
            Err(_) => {}
        }
        for (done, result) in ran {
            done(self, result, rendered);
        }
    }

    /// Queued input whose page went away (`stale`), or that can no longer run at all.
    fn abandon_input(self: &Arc<Self>, stale: bool) {
        let abandoned: Vec<_> = {
            let mut queue = self.input.lock().unwrap_or_else(|p| p.into_inner());
            if stale {
                let (gone, kept): (std::collections::VecDeque<_>, _) = std::mem::take(&mut *queue)
                    .into_iter()
                    .partition(|queued| !self.lifecycle.is_current_page(queued.page));
                *queue = kept;
                gone.into()
            } else {
                queue.drain(..).collect()
            }
        };
        for queued in abandoned {
            (queued.done)(self, None, false);
        }
    }

    fn report_click(&self, page: PageTicket, identity: &str, click: &crate::session::ClickRecord) {
        match click.outcome {
            Ok(true) => {
                self.main().activations += 1;
                self.adapter.debug(&format!(
                    "UI_INPUT: click page={} node={identity:?} commit={} current={} reresolved={}",
                    page.get(),
                    click.commit.get(),
                    click.current.get(),
                    click.reresolved
                ));
            }
            Ok(false) => self.report_dropped(
                page,
                identity,
                "its callback is no longer registered",
                click.commit,
                click.current,
            ),
            Err(reason) => self.report_dropped(page, identity, reason, click.commit, click.current),
        }
    }

    /// A user activation that did nothing is never silent: warn and count it.
    pub(super) fn report_dropped(
        &self,
        page: PageTicket,
        identity: &str,
        reason: &str,
        commit: crate::session::lifecycle::CommitTicket,
        current: crate::session::lifecycle::CommitTicket,
    ) {
        self.main().dropped_activations += 1;
        self.adapter.warn(&format!(
            "UI_INPUT: dropped activation page={} node={identity:?} reason={reason} commit={} current={}",
            page.get(),
            commit.get(),
            current.get()
        ));
    }

    fn activated(
        self: &Arc<Self>,
        page: PageTicket,
        announce: bool,
        result: Option<Result<bool, String>>,
        rendered: bool,
    ) {
        let inner = self;
        match result {
            Some(Ok(true)) => {
                if inner.main().disposed || !inner.lifecycle.is_current_page(page) {
                    return;
                }
                if announce {
                    inner.adapter.clicked();
                }
                if !rendered {
                    inner.main().dirty = true;
                    inner.schedule();
                }
            }
            Some(Err(error)) if inner.lifecycle.is_current_page(page) => inner.fail(error),
            // A running animation whose queued frames gave way to this input resumes.
            _ if inner.main().animating => inner.schedule(),
            _ => {}
        }
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
                    // Edges a superseded frame reached are reached again by the current one.
                    changed |= session.event(commit, handle)?;
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
            if main.number_busy || main.disposed {
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
        let identity = proposal.identity.clone();
        self.queue_input(
            page,
            Box::new(move |session| session.number_control(commit, handle, &identity, value)),
            Box::new(move |inner, result, rendered| {
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
                    Some(Ok((_, true))) if !rendered => {
                        inner.main().dirty = true;
                        inner.schedule();
                    }
                    // The next proposal goes once the frame showing this one presents.
                    Some(Ok((_, true))) => {}
                    _ if pending => inner.schedule(),
                    _ => inner.drain_number(),
                }
            }),
        );
    }

    /// An input-method request; edits that need the View drain Logic actions.
    pub(super) fn input(
        self: &Arc<Self>,
        request: String,
        reply: Box<dyn FnOnce(Option<String>) + Send>,
    ) {
        let page = self.lifecycle.page();
        self.main().inputs_in_flight += 1;
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
                inner.main().inputs_in_flight -= 1;
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
