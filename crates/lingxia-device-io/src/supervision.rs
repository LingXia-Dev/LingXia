//! Persistent session disclosure plus the transient activity preview.
//!
//! The Cargo feature only compiles this mechanism. A product session must
//! hold [`SupervisionGuard`] for its lifetime. Controlled input goes through
//! the guard. A remote caller cannot dismiss disclosure.

use crate::error::{Error, Result};
use crate::model::Acted;
use std::sync::atomic::{AtomicU64, Ordering};

pub use crate::supervision_state::SessionKind;

static NEXT_GUARD: AtomicU64 = AtomicU64::new(1);
static ACTIVE: AtomicU64 = AtomicU64::new(0);

/// RAII hold on persistent session disclosure.
///
/// Dropping the guard (or calling [`SupervisionGuard::end`]) is the trusted
/// host/local end of the session. There is no remote dismiss path.
pub struct SupervisionGuard {
    id: u64,
    kind: SessionKind,
}

impl SupervisionGuard {
    /// Begin persistent disclosure for an observed or controlled session.
    pub fn begin(kind: SessionKind) -> Result<Self> {
        let id = NEXT_GUARD.fetch_add(1, Ordering::Relaxed);
        ACTIVE.store(id, Ordering::SeqCst);
        begin_native(kind);
        Ok(Self { id, kind })
    }

    pub fn kind(&self) -> SessionKind {
        self.kind
    }

    pub fn is_current(&self) -> bool {
        ACTIVE.load(Ordering::SeqCst) == self.id
    }

    /// Controlled input must go through this facade so it cannot outlive
    /// the disclosed session.
    pub fn input(&self) -> Result<GuardedInput<'_>> {
        if !self.is_current() {
            return Err(Error::Unavailable(
                "supervision session is no longer current".into(),
            ));
        }
        Ok(GuardedInput { _guard: self })
    }

    /// Local / trusted-host end. Equivalent to dropping the guard.
    pub fn end(self) {}
}

impl Drop for SupervisionGuard {
    fn drop(&mut self) {
        if ACTIVE
            .compare_exchange(self.id, 0, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            end_native();
        }
    }
}

/// Input methods that require a live supervision session.
pub struct GuardedInput<'a> {
    _guard: &'a SupervisionGuard,
}

#[cfg(feature = "input")]
impl GuardedInput<'_> {
    pub fn pointer_move(&self, x: i32, y: i32, target: Option<u32>) -> Result<crate::model::Ack> {
        crate::input::pointer_move(x, y, target)
    }

    pub fn pointer_click(
        &self,
        x: i32,
        y: i32,
        button: crate::model::MouseButton,
        count: u32,
        target: Option<u32>,
    ) -> Result<crate::model::Ack> {
        crate::input::pointer_click(x, y, button, count, target)
    }

    pub fn pointer_down(
        &self,
        x: i32,
        y: i32,
        button: crate::model::MouseButton,
        target: Option<u32>,
    ) -> Result<crate::model::Ack> {
        crate::input::pointer_down(x, y, button, target)
    }

    pub fn pointer_up(
        &self,
        x: i32,
        y: i32,
        button: crate::model::MouseButton,
        target: Option<u32>,
    ) -> Result<crate::model::Ack> {
        crate::input::pointer_up(x, y, button, target)
    }

    pub fn pointer_drag(
        &self,
        x: i32,
        y: i32,
        to_x: i32,
        to_y: i32,
        button: crate::model::MouseButton,
        target: Option<u32>,
    ) -> Result<crate::model::Ack> {
        crate::input::pointer_drag(x, y, to_x, to_y, button, target)
    }

    pub fn pointer_scroll(
        &self,
        x: i32,
        y: i32,
        dx: i32,
        dy: i32,
        target: Option<u32>,
    ) -> Result<crate::model::Ack> {
        crate::input::pointer_scroll(x, y, dx, dy, target)
    }

    pub fn key_type(&self, text: &str, target: Option<u32>) -> Result<crate::model::Ack> {
        crate::input::key_type(text, target)
    }

    pub fn key_press(
        &self,
        key: &str,
        modifiers: &[crate::model::Modifier],
        target: Option<u32>,
    ) -> Result<crate::model::Ack> {
        crate::input::key_press(key, modifiers, target)
    }

    pub fn key_down(&self, key: &str, target: Option<u32>) -> Result<crate::model::Ack> {
        crate::input::key_down(key, target)
    }

    pub fn key_up(&self, key: &str, target: Option<u32>) -> Result<crate::model::Ack> {
        crate::input::key_up(key, target)
    }
}

/// Transient activity preview. Does not end persistent disclosure.
pub fn note_activity(acted: Acted) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    crate::backend::pip_note_activity(acted);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = acted;
}

/// Local dismiss of the *activity preview* only. Persistent disclosure stays.
pub fn dismiss() {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    crate::backend::pip_dismiss();
}

/// Remote/command-surface dismiss is always rejected.
pub fn dismiss_remote() -> Result<()> {
    Err(Error::Permission(
        "remote callers cannot hide or dismiss session disclosure".into(),
    ))
}

fn begin_native(kind: SessionKind) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    crate::backend::pip_begin_session(kind);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = kind;
}

fn end_native() {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    crate::backend::pip_end_session();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_dismiss_is_rejected() {
        assert!(matches!(dismiss_remote(), Err(Error::Permission(_))));
    }

    #[test]
    fn dropping_the_guard_clears_the_active_session() {
        let guard = SupervisionGuard::begin(SessionKind::Observation).unwrap();
        assert!(guard.is_current());
        drop(guard);
        let next = SupervisionGuard::begin(SessionKind::Control).unwrap();
        assert!(next.is_current());
        next.end();
    }
}
