//! Engines driven through Rong's portable script API. They neither drain jobs
//! under a budget nor preempt a running loop, so they advertise no bounded jobs.
use super::{Engine, Feature};

fn evaluate(context: &rong::JSContext, source: &str) -> Result<(), String> {
    context
        .eval::<()>(rong::Source::from_bytes(source))
        .map_err(|e| e.to_string())
}

/// JavaScriptCore. Its watchdog is private SPI, so a runaway loop cannot be interrupted.
#[cfg(target_vendor = "apple")]
pub(crate) struct JavaScriptCore;

#[cfg(target_vendor = "apple")]
impl Engine for JavaScriptCore {
    const FEATURES: &'static [Feature] = &[Feature::CandidateTransactions];
    const PREEMPTIBLE: bool = false;
    fn evaluate_module(context: &rong::JSContext, source: &str) -> Result<(), String> {
        evaluate(context, source)
    }
    fn drain_jobs(_: &rong::JSContext) -> Result<(), String> {
        Ok(())
    }
    fn discard_jobs(_: &rong::JSContext) {}
}

/// ArkJS. It has no terminate API; interruption is cooperative only.
#[cfg(target_env = "ohos")]
pub(crate) struct ArkJs;

#[cfg(target_env = "ohos")]
impl Engine for ArkJs {
    const FEATURES: &'static [Feature] = &[Feature::CandidateTransactions];
    const PREEMPTIBLE: bool = false;
    fn evaluate_module(context: &rong::JSContext, source: &str) -> Result<(), String> {
        evaluate(context, source)
    }
    fn drain_jobs(_: &rong::JSContext) -> Result<(), String> {
        Ok(())
    }
    fn discard_jobs(_: &rong::JSContext) {}
}
