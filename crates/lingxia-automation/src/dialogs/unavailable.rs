//! `DialogDriver` for builds without the automation `runtime` feature.
//!
//! Such a host cannot run test programs, so no dialog could ever be watched.
//! Reading `lxapp().dialogs` still works; every call rejects.

use crate::auto_err;
use lxapp::LxApp;
use rong::{HostError, JSResult, JSValue, js_class, js_method};
use std::sync::Weak;

const UNAVAILABLE: &str = "dialog watching is not built into this host; \
    it works only inside a host automation run (lxdev test)";

#[js_class(clone)]
pub(crate) struct JSDialogDriver {
    _lxapp: Weak<LxApp>,
}

impl JSDialogDriver {
    pub(crate) fn new(lxapp: Weak<LxApp>) -> Self {
        Self { _lxapp: lxapp }
    }
}

#[js_class(rename = "DialogDriver")]
impl JSDialogDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.automation().lxapp().dialogs",
        )
        .into())
    }

    #[js_method]
    fn watch(&self) -> JSResult<()> {
        Err(auto_err(UNAVAILABLE))
    }

    #[js_method]
    fn unwatch(&self) -> JSResult<()> {
        Err(auto_err(UNAVAILABLE))
    }

    #[js_method]
    async fn unanswered(&self) -> JSResult<()> {
        Err(auto_err(UNAVAILABLE))
    }

    #[js_method]
    fn toasts(&self) -> JSResult<()> {
        Err(auto_err(UNAVAILABLE))
    }

    #[js_method]
    fn modals(&self) -> JSResult<()> {
        Err(auto_err(UNAVAILABLE))
    }

    #[js_method(rename = "actionSheets")]
    fn action_sheets(&self) -> JSResult<()> {
        Err(auto_err(UNAVAILABLE))
    }

    #[js_method(rename = "answerNextModal")]
    fn answer_next_modal(&self, _answer: JSValue) -> JSResult<()> {
        Err(auto_err(UNAVAILABLE))
    }

    #[js_method(rename = "answerNextActionSheet")]
    fn answer_next_action_sheet(&self, _answer: JSValue) -> JSResult<()> {
        Err(auto_err(UNAVAILABLE))
    }
}
