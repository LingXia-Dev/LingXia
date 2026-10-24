//! View lifecycle: load → connect → render → present, with hide/show, reload,
//! failure recovery and disposal.
//!
//! Async work captures a [`Ticket`] when it starts and is applied only if that
//! ticket is still current when it lands. Each scope advances independently:
//! a page ticket names one loaded module instance, a surface ticket one
//! presentation target, a connection ticket one Logic bridge connection and a
//! commit ticket one committed View description. Adapters carry tickets; only
//! this module and [`crate::ViewSession`] issue them.
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

/// A captured validity token for asynchronous work in scope `S`.
pub struct Ticket<S> {
    value: u64,
    scope: PhantomData<fn() -> S>,
}

impl<S> Ticket<S> {
    pub const fn new(value: u64) -> Self {
        Self {
            value,
            scope: PhantomData,
        }
    }
    /// Raw value for transport across an FFI boundary.
    pub const fn get(self) -> u64 {
        self.value
    }
}
impl<S> Clone for Ticket<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S> Copy for Ticket<S> {}
impl<S> PartialEq for Ticket<S> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl<S> Eq for Ticket<S> {}
impl<S> serde::Serialize for Ticket<S> {
    fn serialize<T: serde::Serializer>(&self, serializer: T) -> Result<T::Ok, T::Error> {
        serializer.serialize_u64(self.value)
    }
}
impl<'de, S> serde::Deserialize<'de> for Ticket<S> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u64::deserialize(deserializer).map(Self::new)
    }
}
impl<S> std::fmt::Debug for Ticket<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}#{}",
            std::any::type_name::<S>().rsplit("::").next().unwrap_or(""),
            self.value
        )
    }
}

pub enum PageScope {}
pub enum SurfaceScope {}
pub enum ConnectionScope {}
pub enum CommitScope {}
/// One loaded module instance; reload, park, session failure and disposal end it.
pub type PageTicket = Ticket<PageScope>;
/// One presentation target; surface, visibility, environment and connection changes end it.
pub type SurfaceTicket = Ticket<SurfaceScope>;
/// One Logic bridge connection, issued by the host and only ever increasing.
pub type ConnectionTicket = Ticket<ConnectionScope>;
/// One committed View description; input from older frames is ignored.
pub type CommitTicket = Ticket<CommitScope>;

/// Why rendering stopped and what the host offers next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    /// One candidate failed; the committed page is intact.
    Candidate,
    /// The bundle needs a different native runtime; retrying cannot help.
    Compatibility,
    /// The isolate was interrupted and must be rebuilt.
    Session,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retry {
    None,
    /// Prepare another candidate against the current session.
    Redraw,
    /// Reload a standalone page.
    Reload,
    /// Ask the host to restart the Logic connection.
    Restart,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recovery {
    pub kind: FailureKind,
    pub retry: Retry,
}

impl FailureKind {
    pub fn classify(error: &str) -> Self {
        if error.contains("UI_COMPAT") {
            Self::Compatibility
        } else if error.contains("UI_TIMEOUT:") {
            Self::Session
        } else {
            Self::Candidate
        }
    }
}

#[derive(Default)]
struct State {
    page: u64,
    surface: u64,
    connection: u64,
    loaded: bool,
    bridge_ready: bool,
    session_failed: bool,
    visible: bool,
    disposed: bool,
}

/// Shared by an adapter's UI and worker threads; every transition is atomic.
#[derive(Clone)]
pub struct Lifecycle {
    logic: bool,
    state: Arc<Mutex<State>>,
}

impl Lifecycle {
    pub fn new(logic: bool) -> Self {
        Self {
            logic,
            state: Default::default(),
        }
    }

    fn with<T>(&self, work: impl FnOnce(&mut State) -> T) -> T {
        work(
            &mut self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()),
        )
    }

    /// Start a fresh module instance. Earlier page, surface and connection work becomes stale.
    pub fn load(&self) -> PageTicket {
        self.with(|state| {
            state.page += 1;
            state.surface += 1;
            state.connection = 0;
            state.loaded = false;
            state.bridge_ready = false;
            state.session_failed = false;
            PageTicket::new(state.page)
        })
    }

    /// The module evaluated; ignored if the instance was replaced meanwhile.
    pub fn loaded(&self, page: PageTicket) -> bool {
        self.with(|state| {
            let current = !state.disposed && state.page == page.get();
            if current {
                state.loaded = true;
            }
            current
        })
    }

    /// Release local state but keep the module; the next connection rebuilds it.
    pub fn park(&self) -> PageTicket {
        self.with(|state| {
            state.page += 1;
            state.surface += 1;
            state.connection = 0;
            state.bridge_ready = false;
            PageTicket::new(state.page)
        })
    }

    pub fn dispose(&self) {
        self.with(|state| {
            state.disposed = true;
            state.page += 1;
            state.surface += 1;
        })
    }

    /// Accept only a newer host connection; it invalidates in-flight frames.
    pub fn connect(&self, connection: ConnectionTicket) -> bool {
        self.with(|state| {
            if state.disposed || connection.get() <= state.connection {
                return false;
            }
            state.connection = connection.get();
            state.bridge_ready = false;
            state.surface += 1;
            true
        })
    }

    /// Record whether the connection delivered ready plus its first state.
    pub fn set_bridge_ready(&self, connection: ConnectionTicket, ready: bool) -> bool {
        self.with(|state| {
            let current = state.connection == connection.get();
            if current {
                state.bridge_ready = ready;
            }
            current
        })
    }

    /// Present only to a visible page; hiding ends the current surface ticket.
    pub fn set_visible(&self, visible: bool) {
        self.with(|state| {
            if state.visible && !visible {
                state.surface += 1;
            }
            state.visible = visible;
        })
    }

    /// The presentation target or its inputs changed.
    pub fn invalidate_surface(&self) -> SurfaceTicket {
        self.with(|state| {
            state.surface += 1;
            SurfaceTicket::new(state.surface)
        })
    }

    pub fn page(&self) -> PageTicket {
        self.with(|state| PageTicket::new(state.page))
    }
    pub fn surface(&self) -> SurfaceTicket {
        self.with(|state| SurfaceTicket::new(state.surface))
    }
    pub fn connection(&self) -> ConnectionTicket {
        self.with(|state| ConnectionTicket::new(state.connection))
    }
    pub fn is_current_page(&self, page: PageTicket) -> bool {
        self.with(|state| !state.disposed && state.page == page.get())
    }
    pub fn is_current_surface(&self, surface: SurfaceTicket) -> bool {
        self.with(|state| !state.disposed && state.surface == surface.get())
    }
    pub fn is_current_connection(&self, connection: ConnectionTicket) -> bool {
        self.with(|state| !state.disposed && state.connection == connection.get())
    }

    /// A frame may be prepared now; returns the surface ticket it must present against.
    pub fn frame(&self) -> Option<SurfaceTicket> {
        self.with(|state| {
            let ready = state.loaded
                && state.visible
                && !state.disposed
                && !state.session_failed
                && (!self.logic || state.bridge_ready);
            ready.then_some(SurfaceTicket::new(state.surface))
        })
    }

    /// Input from the user is accepted only by a live, unfailed page.
    pub fn accepts_input(&self) -> bool {
        self.with(|state| state.loaded && !state.disposed && !state.session_failed)
    }

    /// Classify a failure, end the session when needed, and decide the offered retry.
    pub fn fail(&self, error: &str) -> Recovery {
        let kind = FailureKind::classify(error);
        self.with(|state| {
            let loaded = state.loaded;
            let retry = match kind {
                FailureKind::Compatibility => Retry::None,
                FailureKind::Candidate if loaded && (!self.logic || state.bridge_ready) => {
                    Retry::Redraw
                }
                FailureKind::Candidate => Retry::None,
                // A session that failed again before reloading still offers the restart.
                FailureKind::Session if loaded || state.session_failed => {
                    if self.logic {
                        Retry::Restart
                    } else {
                        Retry::Reload
                    }
                }
                FailureKind::Session => Retry::None,
            };
            if kind == FailureKind::Session {
                state.session_failed |= loaded;
                state.loaded = false;
                state.page += 1;
                state.surface += 1;
            }
            Recovery { kind, retry }
        })
    }

    /// Whether the presented page has stopped accepting input after an interrupted session.
    pub fn session_failed(&self) -> bool {
        self.with(|state| state.session_failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready(lifecycle: &Lifecycle) -> (PageTicket, ConnectionTicket) {
        let page = lifecycle.load();
        assert!(lifecycle.loaded(page));
        lifecycle.set_visible(true);
        let connection = ConnectionTicket::new(1);
        assert!(lifecycle.connect(connection));
        assert!(lifecycle.set_bridge_ready(connection, true));
        (page, connection)
    }

    #[test]
    fn frames_wait_for_load_visibility_and_the_logic_bridge() {
        let lifecycle = Lifecycle::new(true);
        let page = lifecycle.load();
        assert!(lifecycle.frame().is_none());
        lifecycle.loaded(page);
        lifecycle.set_visible(true);
        assert!(lifecycle.frame().is_none(), "Logic pages wait for ready");
        let connection = ConnectionTicket::new(4);
        lifecycle.connect(connection);
        lifecycle.set_bridge_ready(connection, true);
        assert!(lifecycle.frame().is_some());
        let standalone = Lifecycle::new(false);
        let page = standalone.load();
        standalone.loaded(page);
        standalone.set_visible(true);
        assert!(standalone.frame().is_some());
    }

    #[test]
    fn reload_park_and_dispose_end_earlier_tickets() {
        let lifecycle = Lifecycle::new(true);
        let (page, connection) = ready(&lifecycle);
        let surface = lifecycle.frame().unwrap();
        let reloaded = lifecycle.load();
        assert!(!lifecycle.is_current_page(page));
        assert!(!lifecycle.is_current_surface(surface));
        assert!(!lifecycle.is_current_connection(connection));
        assert!(
            !lifecycle.loaded(page),
            "a late load of the old module is ignored"
        );
        assert!(lifecycle.loaded(reloaded));
        let parked = lifecycle.park();
        assert!(!lifecycle.is_current_page(reloaded));
        lifecycle.dispose();
        assert!(!lifecycle.is_current_page(parked));
        assert!(!lifecycle.connect(ConnectionTicket::new(9)));
    }

    #[test]
    fn connections_only_advance_and_hiding_ends_the_surface() {
        let lifecycle = Lifecycle::new(true);
        let (_, connection) = ready(&lifecycle);
        assert!(
            !lifecycle.connect(connection),
            "the same connection cannot reconnect"
        );
        assert!(!lifecycle.connect(ConnectionTicket::new(0)));
        let surface = lifecycle.frame().unwrap();
        lifecycle.set_visible(false);
        assert!(!lifecycle.is_current_surface(surface));
        assert!(lifecycle.frame().is_none());
        assert!(!lifecycle.set_bridge_ready(ConnectionTicket::new(7), true));
    }

    #[test]
    fn failures_choose_redraw_restart_or_nothing() {
        let logic = Lifecycle::new(true);
        ready(&logic);
        assert_eq!(logic.fail("UI_LAYOUT: bad").retry, Retry::Redraw);
        assert_eq!(
            logic.fail("UI_COMPAT: newer host").kind,
            FailureKind::Compatibility
        );
        assert_eq!(logic.fail("UI_COMPAT: x").retry, Retry::None);
        let page = logic.page();
        let session = logic.fail("UI_TIMEOUT: budget");
        assert_eq!(
            session,
            Recovery {
                kind: FailureKind::Session,
                retry: Retry::Restart
            }
        );
        assert!(logic.session_failed() && !logic.accepts_input());
        assert!(!logic.is_current_page(page));
        assert_eq!(logic.fail("UI_TIMEOUT: again").retry, Retry::Restart);
        let standalone = Lifecycle::new(false);
        let page = standalone.load();
        assert_eq!(standalone.fail("UI_LAYOUT: early").retry, Retry::None);
        standalone.loaded(page);
        assert_eq!(standalone.fail("UI_TIMEOUT: x").retry, Retry::Reload);
        standalone.load();
        assert!(
            !standalone.session_failed(),
            "reload clears the failed session"
        );
    }
}
