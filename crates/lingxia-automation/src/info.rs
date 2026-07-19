//! `LxAppDriver` — one selected lxapp's page, navigation, and Logic surfaces.

use crate::resolve::{json_to_js, upgrade};
use crate::{host, nav, page};
use lxapp::LxApp;
use rong::{
    Class, FromJSObject, HostError, IntoJSObject, JSContext, JSObject, JSResult, JSValue, js_class,
    js_method,
};
use std::sync::{Arc, Weak};
use std::time::Duration;

#[js_class(clone)]
pub(crate) struct JSLxAppDriver {
    lxapp: Weak<LxApp>,
}

impl JSLxAppDriver {
    pub(crate) fn new(lxapp: &Arc<LxApp>) -> Self {
        Self {
            lxapp: Arc::downgrade(lxapp),
        }
    }
}

#[derive(FromJSObject)]
struct JSEvalOptions {
    script: String,
    #[js_name = "timeoutMs"]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, IntoJSObject)]
struct JSPageConfig {
    name: String,
    path: String,
}

#[js_class(rename = "LxAppDriver")]
impl JSLxAppDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.automation().lxapp()",
        )
        .into())
    }

    #[js_method(getter, enumerable)]
    fn page(&self, ctx: JSContext) -> JSResult<JSObject> {
        let app = upgrade(&self.lxapp)?;
        Ok(Class::lookup::<page::JSPageDriver>(&ctx)?.instance(page::JSPageDriver::new(&app)))
    }

    #[js_method(getter, enumerable)]
    fn nav(&self, ctx: JSContext) -> JSResult<JSObject> {
        let app = upgrade(&self.lxapp)?;
        Ok(Class::lookup::<nav::JSNavDriver>(&ctx)?.instance(nav::JSNavDriver::new(&app)))
    }

    #[js_method]
    async fn info(&self, ctx: JSContext) -> JSResult<JSValue> {
        let app = upgrade(&self.lxapp)?;
        let info = serde_json::to_value(app.runtime_info())
            .map_err(|err| crate::auto_err(err.to_string()))?;
        json_to_js(&ctx, &info)
    }

    #[js_method]
    async fn pages(&self, _ctx: JSContext) -> JSResult<Vec<JSPageConfig>> {
        let app = upgrade(&self.lxapp)?;
        let pages = app
            .runtime_info()
            .page_entries
            .into_iter()
            .map(|entry| JSPageConfig {
                name: entry.name,
                path: entry.path,
            })
            .collect();
        Ok(pages)
    }

    /// Evaluate in the selected lxapp's Logic runtime. A driver created by a
    /// session test is safe; evaluating the calling Logic context rejects to
    /// avoid a re-entrant deadlock.
    #[js_method]
    async fn eval(&self, ctx: JSContext, options: JSEvalOptions) -> JSResult<JSValue> {
        let app = upgrade(&self.lxapp)?;
        host::reject_self(&ctx, &app, "eval")?;
        let timeout = Duration::from_millis(options.timeout_ms.unwrap_or(5_000));
        let value = tokio::time::timeout(timeout, app.eval_logic(options.script))
            .await
            .map_err(|_| crate::auto_err("lxapp eval timed out"))?
            .map_err(|err| crate::auto_err(err.to_string()))?;
        json_to_js(&ctx, &value)
    }
}
