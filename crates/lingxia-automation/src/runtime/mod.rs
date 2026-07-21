//! Isolated, host-owned JavaScript automation runtime.
//!
//! Each [`AutomationRuntime`] owns one worker and executes one automation
//! program at a time in a fresh context. Programs receive `lx.automation()`,
//! console, timers/fetch, and `__LINGXIA_AUTOMATION_HOST__` for string args,
//! structured events, and bounded artifacts. They never impersonate an lxapp.
//!
//! On Apple platforms, enable `runtime-interrupt` when non-yielding programs
//! must be preempted. It uses JavaScriptCore's private execution-time-limit SPI.

mod context;
mod manager;
mod protocol;
mod run;

pub use manager::AutomationRuntime;
pub use protocol::{
    AutomationCancelArgs, AutomationCancelResponse, AutomationEvent, AutomationEventPayload,
    AutomationPollArgs, AutomationPollResponse, AutomationRunError, AutomationRunResult,
    AutomationRunState, AutomationStartArgs, AutomationStartResponse,
};
