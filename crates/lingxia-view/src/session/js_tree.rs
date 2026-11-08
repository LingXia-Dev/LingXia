//! View isolate values to and from owned trees for Logic: walked through the engine, never serialized.
use rong::quickjs::{JSArray, JSContext, JSFunc, JSObject, JSValue, PropertyDescriptor};
use rong::{IntoJSValue, JSResult, error::HostError};
use serde_json::{Map, Number, Value};

const MAX_DEPTH: usize = 128;

fn too_deep() -> rong::RongJSError {
    HostError::new(rong::error::E_INTERNAL, "value nests too deeply").into()
}

/// A JS value as `JSON.stringify` would see it: `toJSON` honoured, functions, symbols and
/// `undefined` dropped from objects and `null` in arrays, non-finite numbers `null`.
pub(crate) fn from_js(value: JSValue) -> JSResult<Value> {
    walk(value, 0).map(|value| value.unwrap_or(Value::Null))
}

fn walk(value: JSValue, depth: usize) -> JSResult<Option<Value>> {
    if depth > MAX_DEPTH {
        return Err(too_deep());
    }
    if value.is_undefined() || value.is_function() || value.is_symbol() {
        return Ok(None);
    }
    if value.is_null() {
        return Ok(Some(Value::Null));
    }
    if value.is_boolean() {
        return Ok(Some(Value::Bool(value.into_value().try_into()?)));
    }
    if value.is_number() {
        let number: f64 = value.into_value().try_into()?;
        return Ok(Some(number_value(number)));
    }
    if value.is_string() {
        return Ok(Some(Value::String(value.into_value().try_into()?)));
    }
    let Some(object) = value.into_object() else {
        return Ok(Some(Value::Null));
    };
    if let Ok(to_json) = object.get::<_, JSFunc>("toJSON") {
        let replaced: JSValue = to_json.call(Some(object.clone()), ())?;
        return walk(replaced, depth + 1);
    }
    if let Some(array) = JSArray::from_object(object.clone()) {
        let len = array.len()?;
        let mut items = Vec::with_capacity(len as usize);
        for index in 0..len {
            items.push(walk(array.get_value(index)?, depth + 1)?.unwrap_or(Value::Null));
        }
        return Ok(Some(Value::Array(items)));
    }
    let mut map = Map::new();
    for entry in object.entries()? {
        let (key, value) = entry.into_tuple();
        let key: String = key.into_value().try_into()?;
        if let Some(value) = walk(value, depth + 1)? {
            map.insert(key, value);
        }
    }
    Ok(Some(Value::Object(map)))
}

/// Integral numbers stay integers, as `JSON.stringify` prints them.
pub(crate) fn number_value(number: f64) -> Value {
    if number.fract() == 0.0 && number.abs() < 9_007_199_254_740_992.0 {
        Value::Number(Number::from(number as i64))
    } else {
        Number::from_f64(number).map_or(Value::Null, Value::Number)
    }
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
                    let own = PropertyDescriptor::from_value(item)
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
