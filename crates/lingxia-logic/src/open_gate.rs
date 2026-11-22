//! The state a page handle keeps between being returned and its page being on
//! screen, so `close()`, `show()` and `hide()` can be called at any time.

use futures::channel::oneshot;
use rong::{HostError, JSContext, JSObject, JSResult, RongJSError};
use std::cell::{Cell, RefCell};

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Opening,
    Canceled,
    Open,
    Failed,
}

pub(crate) struct OpenGate {
    state: Cell<State>,
    waiters: RefCell<Vec<oneshot::Sender<bool>>>,
}

impl OpenGate {
    pub(crate) fn new() -> Self {
        Self {
            state: Cell::new(State::Opening),
            waiters: RefCell::default(),
        }
    }

    /// Asks a pending open to stop; true when the open was still pending.
    pub(crate) fn cancel(&self) -> bool {
        if self.state.get() != State::Opening {
            return false;
        }
        self.state.set(State::Canceled);
        true
    }

    pub(crate) fn canceled(&self) -> bool {
        self.state.get() == State::Canceled
    }

    /// Ends the pending state and wakes every waiter with whether the page is open.
    pub(crate) fn settle(&self, open: bool) {
        if self.state.get() == State::Opening || !open {
            self.state
                .set(if open { State::Open } else { State::Failed });
        }
        for waiter in self.waiters.borrow_mut().drain(..) {
            let _ = waiter.send(open);
        }
    }

    /// Resolves with whether the page opened; at once when the open has ended.
    pub(crate) fn wait(&self) -> oneshot::Receiver<bool> {
        let (tx, rx) = oneshot::channel();
        match self.state.get() {
            State::Open => {
                let _ = tx.send(true);
            }
            State::Failed => {
                let _ = tx.send(false);
            }
            State::Opening | State::Canceled => self.waiters.borrow_mut().push(tx),
        }
        rx
    }
}

/// How `opened` rejects when `close()` stopped the open.
pub(crate) fn canceled_open_error() -> RongJSError {
    HostError::new(
        rong::error::E_ABORT,
        "the page did not open: close() was called first",
    )
    .with_name("AbortError")
    .into()
}

/// What `result` resolves to when the page never opened because of `close()`.
pub(crate) fn canceled_result(ctx: &JSContext) -> JSResult<JSObject> {
    let result = JSObject::new(ctx);
    result.set("status", "canceled")?;
    result.set("reason", "user")?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_close_before_the_open_ends_it_and_wakes_waiters_closed() {
        let gate = OpenGate::new();
        let mut waiter = gate.wait();
        assert!(gate.cancel());
        assert!(gate.canceled());
        gate.settle(false);
        assert_eq!(waiter.try_recv().unwrap(), Some(false));
        assert_eq!(gate.wait().try_recv().unwrap(), Some(false));
    }

    #[test]
    fn calls_made_while_opening_run_once_it_is_open() {
        let gate = OpenGate::new();
        let mut waiter = gate.wait();
        gate.settle(true);
        assert_eq!(waiter.try_recv().unwrap(), Some(true));
        assert!(!gate.cancel(), "an open page is closed, not canceled");
        assert_eq!(gate.wait().try_recv().unwrap(), Some(true));
    }
}
