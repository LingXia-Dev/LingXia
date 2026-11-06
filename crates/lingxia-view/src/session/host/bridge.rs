//! The Logic bridge: connection handshake, inbound frames, Logic actions and
//! the bootstrap deadline.
use super::{BOOTSTRAP, Guard, Inner};

/// The longest a state write waits for a frame to show it before it is acknowledged anyway.
const ACK_DEADLINE: std::time::Duration = std::time::Duration::from_millis(250);
use crate::platform::PlatformAdapter;
use crate::session::ViewSession;
use crate::session::lifecycle::ConnectionTicket;
use crate::session::state::StateUpdate;
use std::sync::Arc;

/// What a bridge step produced for the main thread.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BridgeUpdate {
    /// Ready plus first state arrived; frames may render.
    pub ready: bool,
    pub changed: bool,
    pub local_changed: bool,
    pub outbound: Vec<String>,
    /// State acknowledgements by revision; sent once a frame from that state is built.
    pub acks: Vec<(u64, String)>,
    pub errors: Vec<String>,
    /// Settled automation action calls.
    pub automation: Vec<crate::session::bridge::AutomationOutcome>,
    /// Toast deadlines to schedule: `(token, milliseconds)`.
    pub overlay_timers: Vec<(u64, u64)>,
}

/// Collect queued Logic actions after a View step.
pub(super) fn update(
    session: &mut ViewSession,
    update: StateUpdate,
) -> Result<BridgeUpdate, String> {
    let (acks, rest): (Vec<_>, Vec<_>) = update
        .outbound
        .iter()
        .partition(|m| m["kind"] == "state.ack" && m["rev"].is_u64());
    let mut outbound: Vec<String> = rest.iter().map(|m| m.to_string()).collect();
    outbound.extend(session.take_action_requests()?);
    Ok(BridgeUpdate {
        ready: session.ready_to_render(),
        changed: update.changed,
        local_changed: session.has_local_updates()?,
        outbound,
        acks: acks
            .iter()
            .map(|m| (m["rev"].as_u64().unwrap_or(0), m.to_string()))
            .collect(),
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
            return Ok(None);
        }
        let update = update(
            session,
            StateUpdate {
                changed: true,
                outbound: Vec::new(),
            },
        )?;
        Ok(Some((self.lifecycle.connection(), update)))
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
        for message in &update.outbound {
            self.adapter.message(connection, message);
        }
        if !update.acks.is_empty() {
            // A write is acknowledged once the frame that shows it is built and queued for the
            // display, so a Logic loop that awaits its writes runs once per frame. A page that is
            // not shown, or a write that never reaches a frame, is not held past the deadline.
            let showing = {
                let main = self.main();
                main.visible && !main.disposed
            };
            if showing && !self.window_hidden() {
                self.main()
                    .acks
                    .extend(update.acks.into_iter().map(|(rev, m)| (connection, rev, m)));
                let inner = self.clone();
                self.adapter
                    .post_after(ACK_DEADLINE, Box::new(move || inner.release_acks(u64::MAX)));
            } else {
                for (_, message) in &update.acks {
                    self.adapter.message(connection, message);
                }
            }
        }
        for (token, ms) in update.overlay_timers {
            self.overlay_deadline(token, ms);
        }
        self.lifecycle.set_bridge_ready(connection, update.ready);
        if update.ready {
            self.main().bootstrap_timer += 1;
            self.adapter.bootstrap_overdue(false);
        }
        if update.changed {
            self.main().dirty = true;
            self.schedule();
        }
    }

    /// Send the held acknowledgements up to revision `shown`, for the current connection.
    pub(super) fn release_acks(self: &Arc<Self>, shown: u64) {
        let due: Vec<(ConnectionTicket, String)> = {
            let mut main = self.main();
            let (due, held) = std::mem::take(&mut main.acks)
                .into_iter()
                .partition(|(_, rev, _)| *rev <= shown);
            main.acks = held;
            due.into_iter()
                .map(|(connection, _, m)| (connection, m))
                .collect()
        };
        for (connection, message) in due {
            if self.lifecycle.is_current_connection(connection) {
                self.adapter.message(connection, &message);
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

    pub(super) fn connect(self: &Arc<Self>, connection: ConnectionTicket, nonce: String) {
        if self.main().disposed || !self.lifecycle.connect(connection) {
            return;
        }
        let timer = {
            let mut main = self.main();
            main.failure = None;
            main.bootstrap_timer += 1;
            main.bootstrap_timer
        };
        self.adapter.recovery(None);
        self.adapter.bootstrap_overdue(false);
        let inner = self.clone();
        self.adapter.post_after(
            BOOTSTRAP,
            Box::new(move || {
                if inner.main().bootstrap_timer == timer
                    && inner.lifecycle.is_current_connection(connection)
                    && !inner.lifecycle.bridge_ready()
                {
                    inner
                        .adapter
                        .warn("UI_BOOTSTRAP_TIMEOUT: bridge ready and initial state not received within 5 seconds");
                    inner.adapter.bootstrap_overdue(true);
                }
            }),
        );
        self.work(
            Guard::Connection(connection),
            move |_, session| Ok(session.begin_bridge(&nonce)?.to_string()),
            move |inner, result| match result {
                Some(Ok(hello)) if inner.lifecycle.is_current_connection(connection) => {
                    inner.adapter.message(connection, &hello);
                }
                Some(Err(error)) if inner.lifecycle.is_current_connection(connection) => {
                    inner.fail(error);
                }
                _ => {}
            },
        );
    }

    pub(super) fn receive(self: &Arc<Self>, connection: ConnectionTicket, frame: String) {
        self.bridge_step(connection, move |session| {
            let frame = serde_json::from_str(&frame).map_err(|e| format!("UI_BRIDGE: {e}"))?;
            let state = session.receive_bridge(&frame)?;
            update(session, state)
        });
    }

    pub(super) fn request_leave(self: &Arc<Self>, connection: ConnectionTicket, reason: String) {
        self.bridge_step(connection, move |session| {
            session.request_leave(&reason)?;
            update(
                session,
                StateUpdate {
                    changed: true,
                    outbound: Vec::new(),
                },
            )
        });
    }

    fn bridge_step(
        self: &Arc<Self>,
        connection: ConnectionTicket,
        step: impl FnOnce(&mut ViewSession) -> Result<BridgeUpdate, String> + Send + 'static,
    ) {
        if self.main().disposed || !self.lifecycle.is_current_connection(connection) {
            return;
        }
        self.work(
            Guard::Connection(connection),
            move |_, session| step(session),
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
