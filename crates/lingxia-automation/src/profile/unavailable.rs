//! `ProfileDriver` for builds without the automation `runtime` feature.
//!
//! Such a host cannot run test programs, so no run can own an isolated
//! profile. Reading `lxapp().profile` still works; every call rejects.

use crate::error::{E_PROFILE_NOT_ISOLATED, coded};
use lxapp::LxApp;
use rong::{HostError, JSResult, JSValue, function::Optional, js_class, js_method};
use std::sync::Weak;

const UNAVAILABLE: &str = "profile rollback is not built into this host; \
    it works only inside an isolated host automation run (lxdev test --profile)";

#[js_class(clone)]
pub(crate) struct JSProfileDriver {
    _lxapp: Weak<LxApp>,
}

impl JSProfileDriver {
    pub(crate) fn new(lxapp: Weak<LxApp>) -> Self {
        Self { _lxapp: lxapp }
    }
}

#[js_class(rename = "ProfileDriver")]
impl JSProfileDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.automation().lxapp().profile",
        )
        .into())
    }

    #[js_method]
    async fn checkpoint(&self) -> JSResult<String> {
        Err(coded(E_PROFILE_NOT_ISOLATED, UNAVAILABLE).into())
    }

    #[js_method]
    async fn restore(&self, _id: String, _options: Optional<JSValue>) -> JSResult<()> {
        Err(coded(E_PROFILE_NOT_ISOLATED, UNAVAILABLE).into())
    }

    #[js_method]
    async fn drop(&self, _id: String) -> JSResult<()> {
        Err(coded(E_PROFILE_NOT_ISOLATED, UNAVAILABLE).into())
    }
}
