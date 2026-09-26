//! Isolated, host-owned JavaScript automation runtime.
//!
//! Each [`AutomationRuntime`] owns one worker and executes one automation
//! program at a time in a fresh context. Programs receive `lx.automation()`,
//! console, timers/fetch, and `__LINGXIA_AUTOMATION_HOST__` for string args,
//! structured events, and bounded artifacts. They never impersonate an lxapp.
//!
//! On Apple platforms JavaScriptCore offers no public way to preempt running
//! JavaScript, so interruption is cooperative there: a program that never
//! yields is timed out and its worker marked unhealthy.

mod context;
mod manager;
mod profile;
mod protocol;
mod run;

pub use manager::AutomationRuntime;

/// Network scenarios and recordings a dev session drives outside test runs
/// (`lxdev scenario …`, `lxdev network …`).
pub mod network {
    pub use crate::network::companion::{Upstream, UpstreamError, UpstreamFuture, set_upstream};
    pub use crate::network::dev::{
        clear_scenario, record_start, record_stop, session_ended, status, use_scenario,
    };
}
pub use profile::{AutomationProfile, ProfileExport, discard_retained, export_retained};
pub use protocol::{
    AutomationActiveRun, AutomationCancelArgs, AutomationCancelResponse, AutomationEvent,
    AutomationEventPayload, AutomationPollArgs, AutomationPollResponse, AutomationRunError,
    AutomationRunResult, AutomationRunState, AutomationStartArgs, AutomationStartResponse,
};
