//! The Logic bridge: connection handshake, inbound frames, Logic actions and
//! the bootstrap deadline.
use super::{BOOTSTRAP, Guard, Inner};
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
    pub errors: Vec<String>,
}

/// Collect queued Logic actions after a View step.
pub(super) fn update(
    session: &mut ViewSession,
    update: StateUpdate,
) -> Result<BridgeUpdate, String> {
    let mut outbound: Vec<String> = update.outbound.iter().map(|m| m.to_string()).collect();
    outbound.extend(session.take_action_requests()?);
    Ok(BridgeUpdate {
        ready: session.ready_to_render(),
        changed: update.changed,
        local_changed: session.has_local_updates()?,
        outbound,
        errors: session.take_action_errors()?,
    })
}

type Drained = Option<(ConnectionTicket, BridgeUpdate)>;

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
        for message in &update.outbound {
            self.adapter.message(connection, message);
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
