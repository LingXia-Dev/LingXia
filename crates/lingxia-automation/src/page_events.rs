//! `pageEvents`: the page failures a host test run grades its tests by.

use crate::resolve::json_to_js;
use crate::{auto_err, host_automation_authority};
use lxapp::automation::page_events;
use rong::{
    FromJSObject, HostError, JSContext, JSResult, JSValue, function::Optional, js_class, js_method,
};

#[js_class(clone)]
pub(crate) struct JSPageEventDriver {}

impl JSPageEventDriver {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObject, Default)]
struct ReadOptions {
    since: Option<u64>,
}

#[js_class(rename = "PageEventDriver")]
impl JSPageEventDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use rawAutomation().pageEvents",
        )
        .into())
    }

    /// Events after `since`, oldest first, with how many were lost to the bound.
    #[js_method]
    async fn read(&self, ctx: JSContext, options: Optional<ReadOptions>) -> JSResult<JSValue> {
        let Some(authority) = host_automation_authority(&ctx) else {
            return Err(auto_err(
                "pageEvents is only read by a host test run (lxdev test)",
            ));
        };
        authority.check()?;
        let since = options.0.unwrap_or_default().since.unwrap_or(0);
        let Some((events, dropped)) = page_events::read(since) else {
            return Err(auto_err("no host test run is recording page events"));
        };
        let value = serde_json::json!({ "events": events, "dropped": dropped });
        json_to_js(&ctx, &value)
    }
}
