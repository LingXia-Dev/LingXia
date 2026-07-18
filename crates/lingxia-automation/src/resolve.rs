//! JS-side glue over the shared automation lower half (`lxapp::automation`).
//!
//! Target resolution, navigation semantics, and the DOM query script live in
//! `lxapp::automation`, shared with the devtool handlers so lxdev and
//! `lx.automation()` never drift. This module only adapts errors and values
//! into the JS runtime.

use crate::auto_err;
use lxapp::LxApp;
use rong::{IntoJSValue, JSArray, JSContext, JSObject, JSResult, JSValue};
use serde_json::Value;
use std::sync::{Arc, Weak};

pub(crate) fn upgrade(weak: &Weak<LxApp>) -> JSResult<Arc<LxApp>> {
    weak.upgrade()
        .ok_or_else(|| auto_err("automation owner LxApp has been released"))
}

/// Resolve any running lxapp by id ("current" = the active app).
pub(crate) fn resolve_lxapp_by_id(raw: &str) -> JSResult<Arc<LxApp>> {
    lxapp::automation::resolve_lxapp(raw).map_err(auto_err)
}

/// Convert a JS object (e.g. a `query` option) into a serde value.
pub(crate) fn js_object_to_json(object: &JSObject) -> JSResult<Value> {
    let json = object.to_json_string()?;
    serde_json::from_str(&json).map_err(|err| auto_err(format!("invalid object: {err}")))
}

/// Recursively convert a serde_json value into a JS value (for runtime data
/// that has no dedicated IntoJSObject struct: query payloads, browser tabs,
/// app windows, eval results).
pub(crate) fn json_to_js(ctx: &JSContext, value: &Value) -> JSResult<JSValue> {
    Ok(match value {
        Value::Null => JSValue::null(ctx),
        Value::Bool(b) => (*b).into_js_value(ctx),
        Value::Number(n) => n.as_f64().unwrap_or(0.0).into_js_value(ctx),
        Value::String(s) => s.as_str().into_js_value(ctx),
        Value::Array(items) => {
            let arr = JSArray::new(ctx)?;
            for item in items {
                arr.push_value(json_to_js(ctx, item)?)?;
            }
            arr.into_js_value(ctx)
        }
        Value::Object(map) => {
            let obj = JSObject::new(ctx);
            for (k, v) in map {
                obj.set(k.as_str(), json_to_js(ctx, v)?)?;
            }
            obj.into_js_value()
        }
    })
}
