//! `ClockDriver` for builds without the automation `runtime` feature.
//!
//! Such a host cannot run test programs, so no clock could ever be owned.
//! Reading `lxapp().clock` still works; every call rejects.

use crate::auto_err;
use lxapp::LxApp;
use rong::{HostError, JSResult, JSValue, function::Optional, js_class, js_method};
use std::sync::Weak;

const UNAVAILABLE: &str = "the test clock is not built into this host; \
    it works only inside a host automation run (lxdev test)";

#[js_class(clone)]
pub(crate) struct JSClockDriver {
    _lxapp: Weak<LxApp>,
}

impl JSClockDriver {
    pub(crate) fn new(lxapp: Weak<LxApp>) -> Self {
        Self { _lxapp: lxapp }
    }
}

#[js_class(rename = "ClockDriver")]
impl JSClockDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.automation().lxapp().clock",
        )
        .into())
    }

    #[js_method]
    async fn install(&self, _options: Optional<JSValue>) -> JSResult<f64> {
        Err(auto_err(UNAVAILABLE))
    }

    #[js_method]
    async fn tick(&self, _ms: JSValue) -> JSResult<()> {
        Err(auto_err(UNAVAILABLE))
    }

    #[js_method(rename = "runAll")]
    async fn run_all(&self, _options: Optional<JSValue>) -> JSResult<()> {
        Err(auto_err(UNAVAILABLE))
    }

    #[js_method(rename = "setSystemTime")]
    async fn set_system_time(&self, _time: JSValue) -> JSResult<f64> {
        Err(auto_err(UNAVAILABLE))
    }

    #[js_method]
    async fn uninstall(&self) -> JSResult<()> {
        Err(auto_err(UNAVAILABLE))
    }
}
