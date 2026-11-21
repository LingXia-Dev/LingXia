//! `HostTarget`, the shape a notification tap or banner action takes: the
//! targets `lx.nav.go` takes, plus `{ activate }`, `{ route }` and `{ appLink }`.
//! It is read into the `kind` form the navigation service persists and opens.

use serde_json::{Map, Value, json};

/// `own_appid` is the calling guest lxapp's id, so its `{ page }` opens itself;
/// `None` for the Control app, whose pages are the home lxapp's.
pub(crate) fn to_navigation_json(
    value: Value,
    own_appid: Option<&str>,
    is_tab: impl Fn(&str) -> bool,
) -> Result<Value, String> {
    let Value::Object(object) = value else {
        return Err("target must be an object".to_string());
    };
    let named = ["activate", "page", "tab", "app", "route", "appLink", "url"]
        .into_iter()
        .filter(|key| object.contains_key(*key))
        .collect::<Vec<_>>();
    let page_of = |page: Value, params: Option<Value>| match own_appid {
        None => json!({ "kind": "page", "page": page, "query": params }),
        Some(appid) => json!({ "kind": "app", "appId": appid, "page": page, "query": params }),
    };
    let text = |key: &str| -> Result<String, String> {
        object
            .get(key)
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
            .ok_or_else(|| format!("target.{key} must be a non-empty string"))
    };
    let params = object
        .get("params")
        .cloned()
        .filter(|params| !params.is_null());
    let target = match named.as_slice() {
        ["activate"] => {
            only(&object, &["activate"])?;
            if object.get("activate") != Some(&Value::Bool(true)) {
                return Err("target.activate must be true".to_string());
            }
            json!({ "kind": "activate" })
        }
        ["page"] => {
            only(&object, &["page", "params"])?;
            let page = text("page")?;
            if is_tab(&page) {
                return Err(format!(
                    "'{page}' is a tab; target it with {{ tab: '{page}' }}"
                ));
            }
            page_of(Value::String(page), params)
        }
        ["tab"] => {
            only(&object, &["tab"])?;
            let tab = text("tab")?;
            if !is_tab(&tab) {
                return Err(format!(
                    "'{tab}' is not a tab; target it with {{ page: '{tab}' }}"
                ));
            }
            page_of(Value::String(tab), None)
        }
        ["page", "app"] | ["app"] => {
            if object.contains_key("channel") || object.contains_key("version") {
                return Err(
                    "target.channel and target.version are not kept on a notification target"
                        .to_string(),
                );
            }
            only(&object, &["app", "page", "params"])?;
            json!({
                "kind": "app",
                "appId": text("app")?,
                "page": object.get("page").cloned(),
                "query": params,
            })
        }
        ["route"] => {
            only(&object, &["route", "params"])?;
            json!({ "kind": "route", "name": text("route")?, "params": params })
        }
        ["appLink"] => {
            only(&object, &["appLink"])?;
            json!({ "kind": "appLink", "url": text("appLink")? })
        }
        ["url"] => {
            return Err(
                "a notification cannot open the in-app browser; use { appLink } for a product link"
                    .to_string(),
            );
        }
        [] => {
            return Err(
                "target names no destination: give one of activate, page, tab, app, route, appLink"
                    .to_string(),
            );
        }
        _ => {
            return Err(format!(
                "target names more than one destination: {}",
                named.join(", ")
            ));
        }
    };
    Ok(strip_nulls(target))
}

fn only(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    match object.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(format!("target.{key} does not belong with {}", allowed[0])),
        None => Ok(()),
    }
}

fn strip_nulls(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            Value::Object(object.into_iter().filter(|(_, v)| !v.is_null()).collect())
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabs(name: &str) -> bool {
        name == "home"
    }

    #[test]
    fn nav_targets_read_into_the_persisted_kinds() {
        let product = |value| to_navigation_json(value, None, tabs);
        assert_eq!(
            product(json!({ "page": "order", "params": { "id": "7" } })),
            Ok(json!({ "kind": "page", "page": "order", "query": { "id": "7" } }))
        );
        assert_eq!(
            product(json!({ "tab": "home" })),
            Ok(json!({ "kind": "page", "page": "home" }))
        );
        assert_eq!(
            product(json!({ "app": "com.x", "page": "chat" })),
            Ok(json!({ "kind": "app", "appId": "com.x", "page": "chat" }))
        );
        assert_eq!(
            product(json!({ "activate": true })),
            Ok(json!({ "kind": "activate" }))
        );
        assert_eq!(
            product(json!({ "route": "orders.detail", "params": { "orderId": "42" } })),
            Ok(json!({ "kind": "route", "name": "orders.detail", "params": { "orderId": "42" } }))
        );
        assert_eq!(
            product(json!({ "appLink": "https://x.test/a" })),
            Ok(json!({ "kind": "appLink", "url": "https://x.test/a" }))
        );
    }

    #[test]
    fn a_guest_page_opens_the_guest() {
        assert_eq!(
            to_navigation_json(json!({ "page": "chat" }), Some("com.guest"), tabs),
            Ok(json!({ "kind": "app", "appId": "com.guest", "page": "chat" }))
        );
    }

    #[test]
    fn a_tab_and_a_page_are_not_interchangeable() {
        let refused = |value| to_navigation_json(value, None, tabs).unwrap_err();
        assert!(refused(json!({ "page": "home" })).contains("{ tab: 'home' }"));
        assert!(refused(json!({ "tab": "order" })).contains("{ page: 'order' }"));
        assert!(refused(json!({ "url": "https://x.test" })).contains("appLink"));
        assert!(refused(json!({ "page": "a", "route": "b" })).contains("more than one"));
        assert!(refused(json!({ "app": "com.x", "version": "1.0.0" })).contains("version"));
        assert!(refused(json!({ "kind": "page", "page": "a" })).contains("kind"));
    }
}
