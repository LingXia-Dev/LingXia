//! The schema's `frame` section against what the core actually hands adapters: every key a
//! message carries is declared with its type, and every required key is present.
use serde_json::Value;

fn schema() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/lingxia-ui/contracts/schema.json"
    );
    let text = std::fs::read_to_string(path).expect("contract schema");
    serde_json::from_str::<Value>(&text).expect("schema JSON")["frame"].clone()
}

/// Problems with `value` as the schema message `message` (e.g. `snapshot`), as `path: why`.
pub(crate) fn conform(message: &str, value: &Value) -> Vec<String> {
    let frame = schema();
    let ty = frame["messages"][message]
        .as_str()
        .unwrap_or_else(|| panic!("unknown frame message {message}"))
        .to_owned();
    let mut problems = Vec::new();
    check(&frame["types"], value, &ty, message, &mut problems);
    problems
}

fn check(types: &Value, value: &Value, ty: &str, path: &str, problems: &mut Vec<String>) {
    if let Some(inner) = ty.strip_suffix('?') {
        if !value.is_null() {
            check(types, value, inner, path, problems);
        }
        return;
    }
    if let Some(inner) = ty.strip_suffix("[]") {
        match value.as_array() {
            Some(items) => {
                for (index, item) in items.iter().enumerate() {
                    check(types, item, inner, &format!("{path}[{index}]"), problems);
                }
            }
            None => problems.push(format!("{path}: expected a list of {inner}")),
        }
        return;
    }
    if let Some((inner, count)) = ty.strip_suffix(']').and_then(|t| t.rsplit_once('[')) {
        let count: usize = count.parse().expect("fixed list length");
        match value.as_array() {
            Some(items) if items.len() == count => {
                for (index, item) in items.iter().enumerate() {
                    check(types, item, inner, &format!("{path}[{index}]"), problems);
                }
            }
            _ => problems.push(format!("{path}: expected {count} of {inner}")),
        }
        return;
    }
    let fits = match ty {
        "any" => true,
        "string" => value.is_string(),
        "bool" => value.is_boolean(),
        "u8" => value.as_u64().is_some_and(|n| n <= u8::MAX as u64),
        "u32" => value.as_u64().is_some_and(|n| n <= u32::MAX as u64),
        "u64" | "usize" => value.is_u64(),
        "i64" => value.is_i64() || value.is_u64(),
        "f32" | "f64" => value.is_number(),
        name => {
            let def = &types[name];
            assert!(def.is_object(), "{path}: unknown frame type {name}");
            if let Some(cases) = def["enum"].as_array() {
                cases.contains(value)
            } else if let Some(tag) = def["tagged"].as_str() {
                let variant = value[tag].as_str().and_then(|v| def["variants"].get(v));
                match variant {
                    Some(fields) => {
                        object(types, value, fields, Some(tag), path, problems);
                        true
                    }
                    None => false,
                }
            } else {
                object(types, value, def, None, path, problems);
                true
            }
        }
    };
    if !fits {
        problems.push(format!("{path}: {value} is not {ty}"));
    }
}

fn object(
    types: &Value,
    value: &Value,
    def: &Value,
    tag: Option<&str>,
    path: &str,
    problems: &mut Vec<String>,
) {
    let (Some(fields), Some(object)) = (def.as_object(), value.as_object()) else {
        problems.push(format!("{path}: expected an object"));
        return;
    };
    let open = fields.contains_key("...");
    for (key, item) in object {
        match fields.get(key).and_then(Value::as_str) {
            Some(ty) => check(types, item, ty, &format!("{path}.{key}"), problems),
            None if Some(key.as_str()) == tag || open => {}
            None => problems.push(format!("{path}.{key}: not declared in the frame schema")),
        }
    }
    for (key, ty) in fields {
        let optional = key == "..." || ty.as_str().is_some_and(|ty| ty.ends_with('?'));
        if !optional && !object.contains_key(key) {
            problems.push(format!("{path}.{key}: required but missing"));
        }
    }
}

#[test]
fn the_checker_catches_undeclared_missing_and_mistyped_keys() {
    let ok = serde_json::json!({"min": 0.0, "max": 1.0, "value": 0.5});
    assert!(conform_type("SemanticRange", &ok).is_empty());
    let bad = serde_json::json!({"min": "0", "value": 0.5, "extra": true});
    let problems = conform_type("SemanticRange", &bad);
    assert_eq!(problems.len(), 3, "{problems:?}");
}

#[cfg(test)]
fn conform_type(ty: &str, value: &Value) -> Vec<String> {
    let frame = schema();
    let mut problems = Vec::new();
    check(&frame["types"], value, ty, ty, &mut problems);
    problems
}
