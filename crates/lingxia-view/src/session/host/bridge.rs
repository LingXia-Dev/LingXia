//! The Logic bridge: connection, typed Logic messages and state, Logic actions and
//! the bootstrap deadline.
use super::{BOOTSTRAP, Guard, Inner};

/// The longest a state write waits for a frame to show it before it is acknowledged anyway.
const ACK_DEADLINE: std::time::Duration = std::time::Duration::from_millis(250);
use crate::platform::PlatformAdapter;
use crate::session::ViewSession;
use crate::session::lifecycle::ConnectionTicket;
use crate::session::state::{StateSignal, StateUpdate, StateWrite};
use std::sync::Arc;

/// What a bridge step produced for the main thread.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BridgeUpdate {
    /// Ready plus first state arrived; frames may render.
    pub ready: bool,
    pub changed: bool,
    pub local_changed: bool,
    /// Revisions to acknowledge once a frame from that state is built.
    pub acks: Vec<u64>,
    /// Ask Logic for a state snapshot.
    pub resync: bool,
    pub errors: Vec<String>,
    /// Settled automation action calls.
    pub automation: Vec<crate::session::bridge::AutomationOutcome>,
    /// Toast deadlines to schedule: `(token, milliseconds)`.
    pub overlay_timers: Vec<(u64, u64)>,
}

/// Send what the View queued for Logic, from the worker, and collect the rest for main.
pub(super) fn update<A: PlatformAdapter>(
    inner: &Inner<A>,
    session: &mut ViewSession,
    update: StateUpdate,
) -> Result<BridgeUpdate, String> {
    session.take_handler_failure()?;
    let mut messages = update.outbound;
    messages.extend(session.take_action_requests()?);
    let connection = inner.lifecycle.connection();
    if !messages.is_empty() && inner.lifecycle.is_current_connection(connection) {
        inner.adapter.logic(connection, messages);
    }
    Ok(BridgeUpdate {
        ready: session.ready_to_render(),
        changed: update.changed,
        local_changed: session.has_local_updates()?,
        acks: update.ack.into_iter().collect(),
        resync: update.resync,
        errors: session.take_action_errors()?,
        automation: session.take_automation_results()?,
        overlay_timers: session.take_overlay_timers()?,
    })
}

pub(super) type Drained = Option<(ConnectionTicket, BridgeUpdate)>;

impl<A: PlatformAdapter> Inner<A> {
    /// Logic actions a changed View queued; `None` for standalone pages.
    pub(super) fn drained(&self, session: &mut ViewSession) -> Result<Drained, String> {
        if !self.logic {
            session.take_handler_failure()?;
            for error in session.take_action_errors()? {
                self.adapter.warn(&error);
            }
            return Ok(None);
        }
        let update = update(
            self,
            session,
            StateUpdate {
                changed: true,
                ..Default::default()
            },
        )?;
        let connection = self
            .session_connection
            .load(std::sync::atomic::Ordering::SeqCst);
        Ok(Some((ConnectionTicket::new(connection), update)))
    }

    /// State acknowledgements leave the View before any rendering attempt.
    pub(super) fn publish(self: &Arc<Self>, connection: ConnectionTicket, update: BridgeUpdate) {
        for error in &update.errors {
            self.adapter.warn(error);
        }
        if self.main().disposed || !self.lifecycle.is_current_connection(connection) {
            return;
        }
        if !update.automation.is_empty() {
            let mut main = self.main();
            for (call, result) in update.automation {
                if let Some(slot) = main.automation_calls.get_mut(&call) {
                    *slot = Some(result);
                }
            }
        }
        if update.resync {
            self.adapter.state(connection, StateSignal::Resync);
        }
        if !update.acks.is_empty() {
            // A write is acknowledged once the frame that shows it is built and queued for the
            // display, so a Logic loop that awaits its writes runs once per frame, covered window
            // included. A page that is not shown or a stopped window acknowledges at once; a
            // write that never reaches a frame is not held past the deadline.
            let showing = {
                let main = self.main();
                main.visible && !main.disposed
            };
            if showing && (!self.window_hidden() || self.paced_hidden()) {
                // The deadline frees these writes only, not ones that arrive after them.
                let latest = update.acks.iter().copied().max().unwrap_or(0);
                self.main()
                    .acks
                    .extend(update.acks.into_iter().map(|rev| (connection, rev)));
                let inner = self.clone();
                self.adapter
                    .post_after(ACK_DEADLINE, Box::new(move || inner.release_acks(latest)));
            } else {
                for rev in update.acks {
                    self.adapter.state(connection, StateSignal::Acked(rev));
                }
            }
        }
        for (token, ms) in update.overlay_timers {
            self.overlay_deadline(token, ms);
        }
        self.lifecycle.set_bridge_ready(connection, update.ready);
        if update.ready {
            self.main().bootstrap_timer += 1;
        }
        if update.changed {
            self.revive();
            self.main().dirty = true;
            self.schedule();
        }
    }

    /// Send the held acknowledgements up to revision `shown`, for the current connection.
    pub(super) fn release_acks(self: &Arc<Self>, shown: u64) {
        let due: Vec<(ConnectionTicket, u64)> = {
            let mut main = self.main();
            let (due, held) = std::mem::take(&mut main.acks)
                .into_iter()
                .partition(|(_, rev)| *rev <= shown);
            main.acks = held;
            due
        };
        for (connection, rev) in due {
            if self.lifecycle.is_current_connection(connection) {
                self.adapter.state(connection, StateSignal::Acked(rev));
            }
        }
    }

    /// Hide the toast `token` once its duration passes, unless another replaced it.
    fn overlay_deadline(self: &Arc<Self>, token: u64, ms: u64) {
        let (inner, page) = (self.clone(), self.lifecycle.page());
        self.adapter.post_after(
            std::time::Duration::from_millis(ms),
            Box::new(move || {
                if !inner.lifecycle.is_current_page(page) {
                    return;
                }
                inner.work(
                    Guard::Page(page),
                    move |_, session| session.overlay_timer(token),
                    move |inner, result| {
                        if matches!(result, Some(Ok(true))) && !inner.main().disposed {
                            inner.main().dirty = true;
                            inner.schedule();
                        }
                    },
                );
            }),
        );
    }

    /// Never a blank page: a first state that is overdue shows the slow-load panel, whose
    /// Keep waiting calls this again; a late first state still draws and clears it.
    pub(super) fn watch_bootstrap(self: &Arc<Self>, connection: ConnectionTicket) {
        let timer = {
            let mut main = self.main();
            main.bootstrap_timer += 1;
            main.bootstrap_timer
        };
        let inner = self.clone();
        self.adapter.post_after(
            BOOTSTRAP,
            Box::new(move || {
                if inner.main().bootstrap_timer == timer
                    && inner.lifecycle.is_current_connection(connection)
                    && !inner.lifecycle.bridge_ready()
                {
                    inner.fail(crate::session::lifecycle::BOOTSTRAP_TIMEOUT.into());
                }
            }),
        );
    }

    pub(super) fn connect(self: &Arc<Self>, connection: ConnectionTicket) {
        if self.main().disposed || !self.lifecycle.connect(connection) {
            return;
        }
        // A module this host cannot run stays refused: a new connection does not change it.
        if self.main().failure.as_ref().is_some_and(|failure| {
            failure.recovery.kind == crate::session::lifecycle::FailureKind::Compatibility
        }) {
            return;
        }
        self.main().failure = None;
        log::info!(target: "lingxia_view", "recovery cleared: the logic connected");
        self.adapter.recovery(None);
        self.watch_bootstrap(connection);
        self.work(
            Guard::Connection(connection),
            // Logic loads the page only after this, so its first write finds the View connected.
            move |inner, session| {
                session.begin_bridge()?;
                inner
                    .session_connection
                    .store(connection.get(), std::sync::atomic::Ordering::SeqCst);
                if inner.lifecycle.is_current_connection(connection) {
                    inner
                        .adapter
                        .logic(connection, vec![crate::session::logic::ToLogic::Connected]);
                }
                Ok(())
            },
            move |inner, result| match result {
                Some(Err(error)) if inner.lifecycle.is_current_connection(connection) => {
                    inner.fail(error);
                }
                _ => {}
            },
        );
    }

    pub(super) fn deliver_logic(
        self: &Arc<Self>,
        connection: ConnectionTicket,
        message: crate::session::logic::ToView,
    ) {
        self.bridge_step(connection, move |inner, session| {
            let state = session.receive_logic(message)?;
            update(inner, session, state)
        });
    }

    /// A state write skips the action queues: it runs no View code.
    pub(super) fn write_state(self: &Arc<Self>, connection: ConnectionTicket, write: StateWrite) {
        let sent = crate::session::probe::now_us();
        self.bridge_step(connection, move |_, session| {
            let state = session.write_state(write)?;
            crate::session::probe::record(crate::session::probe::Probe::WriteApply, true, sent);
            Ok(BridgeUpdate {
                ready: session.ready_to_render(),
                changed: state.changed,
                acks: state.ack.into_iter().collect(),
                resync: state.resync,
                ..BridgeUpdate::default()
            })
        });
    }

    pub(super) fn request_leave(self: &Arc<Self>, connection: ConnectionTicket, reason: String) {
        self.bridge_step(connection, move |inner, session| {
            session.request_leave(&reason)?;
            update(
                inner,
                session,
                StateUpdate {
                    changed: true,
                    ..Default::default()
                },
            )
        });
    }

    fn bridge_step(
        self: &Arc<Self>,
        connection: ConnectionTicket,
        step: impl FnOnce(&Inner<A>, &mut ViewSession) -> Result<BridgeUpdate, String> + Send + 'static,
    ) {
        if self.main().disposed || !self.lifecycle.is_current_connection(connection) {
            return;
        }
        self.work(
            Guard::Connection(connection),
            move |inner, session| step(inner, session),
            move |inner, result| match result {
                Some(Ok(update)) => inner.publish(connection, update),
                Some(Err(error)) if inner.lifecycle.is_current_connection(connection) => {
                    inner.fail(error);
                }
                _ => {}
            },
        );
    }
}
