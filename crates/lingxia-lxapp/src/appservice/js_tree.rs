//! Owned trees from a GPU View as Logic values: built through the engine, never serialized.
use rong::{IntoJSValue, JSArray, JSContext, JSObject, JSResult, JSValue, error::HostError};
use serde_json::Value;

const MAX_DEPTH: usize = 128;

fn too_deep() -> rong::RongJSError {
    HostError::new(rong::error::E_INTERNAL, "value nests too deeply").into()
}

/// Build the JS value for `value` in `ctx`.
pub(crate) fn to_js(ctx: &JSContext, value: &Value) -> JSResult<JSValue> {
    build(ctx, value, 0)
}

fn build(ctx: &JSContext, value: &Value, depth: usize) -> JSResult<JSValue> {
    if depth > MAX_DEPTH {
        return Err(too_deep());
    }
    Ok(match value {
        Value::Null => JSValue::null(ctx),
        Value::Bool(value) => (*value).into_js_value(ctx),
        Value::Number(number) => number.as_f64().unwrap_or(f64::NAN).into_js_value(ctx),
        Value::String(value) => value.as_str().into_js_value(ctx),
        Value::Array(items) => {
            let array = JSArray::new(ctx)?;
            for (index, item) in items.iter().enumerate() {
                array.set_value(index as u32, build(ctx, item, depth + 1)?)?;
            }
            array.into_js_value(ctx)
        }
        Value::Object(map) => {
            let object = JSObject::new(ctx);
            for (key, item) in map {
                let item = build(ctx, item, depth + 1)?;
                if key == "__proto__" {
                    // An own property, as JSON.parse makes it; a plain set would replace the prototype.
                    let own = rong::PropertyDescriptor::from_value(item)
                        .writable()
                        .enumerable()
                        .configurable();
                    object.define_property(key.as_str(), own)?;
                } else {
                    object.set(key.as_str(), item)?;
                }
            }
            object.into_js_value()
        }
    })
}
