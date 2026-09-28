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

pub(crate) mod authority;
mod context;
mod manager;
mod profile;
mod protocol;
mod run;

pub use manager::AutomationRuntime;

/// Mocks, network scenarios and recordings a dev session drives outside
/// test runs (`lxdev mock …`, `lxdev network …`).
pub mod network {
    pub use crate::network::companion::{Upstream, UpstreamError, UpstreamFuture, set_upstream};
    pub use crate::network::dev::{
        clear_scenario, mock_load, mock_reset, mock_set, mock_status, mock_unload, mocks_ready,
        pause_scenario, record_start, record_stop, resume_scenario, session_ended,
        session_starting, status, use_scenario, use_scenario_generation,
    };
}
pub use profile::{AutomationProfile, ProfileExport, discard_retained, export_retained};
pub use protocol::{
    AutomationActiveRun, AutomationCancelArgs, AutomationCancelResponse, AutomationEvent,
    AutomationEventPayload, AutomationPollArgs, AutomationPollResponse, AutomationRunError,
    AutomationRunResult, AutomationRunState, AutomationStartArgs, AutomationStartResponse,
};
