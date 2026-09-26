//! `NetworkDriver` for builds without the automation `runtime` feature.
//!
//! Such a host cannot run test programs, so no route could ever be owned.
//! Reading `lxapp().network` still works; every call rejects.

use crate::auto_err;
use lxapp::LxApp;
use rong::{
    HostError, JSContext, JSObject, JSResult, JSValue, function::Optional, js_class, js_method,
};
use std::sync::Weak;

const UNAVAILABLE: &str = "network routing is not built into this host; \
    it works only inside a host automation run (lxdev test)";

#[js_class(clone)]
pub(crate) struct JSNetworkDriver {
    _lxapp: Weak<LxApp>,
}

impl JSNetworkDriver {
    pub(crate) fn new(lxapp: Weak<LxApp>) -> Self {
        Self { _lxapp: lxapp }
    }
}

#[js_class(rename = "NetworkDriver")]
impl JSNetworkDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.automation().lxapp().network",
        )
        .into())
    }

    #[js_method]
    async fn route(&self, _pattern: JSValue, _handler: JSValue) -> JSResult<JSValue> {
        Err(auto_err(UNAVAILABLE))
    }

    #[js_method(rename = "unrouteAll")]
    async fn unroute_all(&self) -> JSResult<u32> {
        Err(auto_err(UNAVAILABLE))
    }

    #[js_method]
    async fn requests(&self) -> JSResult<JSValue> {
        Err(auto_err(UNAVAILABLE))
    }

    #[js_method(rename = "captureResponses")]
    async fn capture_responses(&self, _options: Optional<JSValue>) -> JSResult<()> {
        Err(auto_err(UNAVAILABLE))
    }

    #[js_method]
    async fn responses(&self, _options: Optional<JSValue>) -> JSResult<JSValue> {
        Err(auto_err(UNAVAILABLE))
    }
}

/// `lxapp().scenario()` in a build without the automation runtime.
pub(crate) async fn install_scenario(
    _ctx: JSContext,
    _lxapp: &Weak<LxApp>,
    _definition: JSObject,
    _variant: Option<String>,
) -> JSResult<JSObject> {
    Err(auto_err(
        "scenarios are not built into this host; they work only inside a host automation run \
         (lxdev test)",
    ))
}
