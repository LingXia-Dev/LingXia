//! The shell's "an AI assistant is in control" indicator, platform side.
//!
//! The control runtime decides when a session is running and what Stop does;
//! it sits above this crate, so it hands the Stop action down as a handler
//! and each desktop shell calls [`request_agent_control_stop`] when the user
//! presses Stop.

use std::sync::{Arc, Mutex};

pub type AgentControlStopHandler = Arc<dyn Fn() + Send + Sync>;

static STOP_HANDLER: Mutex<Option<AgentControlStopHandler>> = Mutex::new(None);

/// Install what the indicator's Stop button does.
pub fn set_agent_control_stop_handler(handler: AgentControlStopHandler) {
    *STOP_HANDLER
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some(handler);
}

/// The user pressed Stop. Returns whether anything handled it.
pub fn request_agent_control_stop() -> bool {
    let handler = STOP_HANDLER
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    match handler {
        Some(handler) => {
            handler();
            true
        }
        None => false,
    }
}
