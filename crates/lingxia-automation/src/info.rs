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

fn normalize_surface_layout_keys(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                normalize_surface_layout_keys(value);
            }
        }
        serde_json::Value::Object(fields) => {
            for value in fields.values_mut() {
                normalize_surface_layout_keys(value);
            }
            for (rust, js) in [
                ("app_id", "appId"),
                ("surface_id", "surfaceId"),
                ("active_id", "activeId"),
            ] {
                if let Some(value) = fields.remove(rust) {
                    fields.insert(js.to_string(), value);
                }
            }
        }
        _ => {}
    }
}

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
    /// Resolve to `{ value, calls }`, where `calls` lists the `lx.*` members the
    /// script reached. Off by default: it changes the shape of the result, and
    /// only the test runner has a use for it.
    #[js_name = "captureCalls"]
    capture_calls: Option<bool>,
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

    /// Read the authoritative surface layout that the host skin reconciles.
    /// This is intentionally automation-only: app behavior should use the
    /// public SurfaceHandle contract instead of inspecting host layout state.
    #[js_method(rename = "surfaceLayout")]
    async fn surface_layout(&self, ctx: JSContext) -> JSResult<JSValue> {
        let app = upgrade(&self.lxapp)?;
        let layout = app
            .surface_derived_layout()
            .ok_or_else(|| crate::auto_err("surface layout is unavailable"))?;
        let mut layout = serde_json::to_value(layout)
            .map_err(|err| crate::auto_err(format!("serialize surface layout: {err}")))?;
        normalize_surface_layout_keys(&mut layout);
        json_to_js(&ctx, &layout)
    }

    /// Evaluate in the selected lxapp's Logic runtime. A driver created by a
    /// session test is safe; evaluating the calling Logic context rejects to
    /// avoid a re-entrant deadlock.
    #[js_method]
    async fn eval(&self, ctx: JSContext, options: JSEvalOptions) -> JSResult<JSValue> {
        let app = upgrade(&self.lxapp)?;
        host::reject_self(&ctx, &app, "eval")?;
        let timeout = Duration::from_millis(options.timeout_ms.unwrap_or(5_000));
        let capture_calls = options.capture_calls.unwrap_or(false);
        let script = options.script;
        let evaluation = async move {
            if capture_calls {
                app.eval_logic_capturing_calls(script).await
            } else {
                app.eval_logic(script).await
            }
        };
        let value = tokio::time::timeout(timeout, evaluation)
            .await
            .map_err(|_| crate::auto_err("lxapp eval timed out"))?
            .map_err(|err| crate::auto_err(err.to_string()))?;
        json_to_js(&ctx, &value)
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_surface_layout_keys;

    #[test]
    fn surface_layout_snapshot_uses_javascript_field_names_recursively() {
        let mut value = serde_json::json!({
            "content": { "app_id": "demo" },
            "tree": {
                "active_id": "root",
                "children": [{ "surface_id": "root" }]
            }
        });

        normalize_surface_layout_keys(&mut value);

        assert_eq!(value["content"]["appId"], "demo");
        assert_eq!(value["tree"]["activeId"], "root");
        assert_eq!(value["tree"]["children"][0]["surfaceId"], "root");
        assert!(value["content"].get("app_id").is_none());
    }
}
