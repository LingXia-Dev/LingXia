use crate::i18n::{js_internal_error, js_invalid_parameter_error, js_service_unavailable_error};
use lxapp::LxApp;
use lxapp::tabbar::TabBarPatch;
use rong::{JSContext, JSObject, JSResult};
use serde_json::{Map, Value};

/// The tab bar declared by this lxapp: visibility, items, badges, and style.
fn namespace(ctx: &JSContext) -> JSResult<JSObject> {
    crate::namespace::child(ctx, &crate::namespace::app(ctx)?, "tabBar")
}

/// Patch this lxapp's tab bar: items by tab name, badges and visibility. Unset
/// fields stay; colors stay in `lxapp.json` `tabBar.style`.
///
/// @platform macOS, Windows: the bar is the sidebar, which follows the host theme; `backgroundColor` does not paint it, and a dark host ignores every style key.
/// @throws {E_INVALID_ARG} A `style` field, an unknown one, or a name that is not a tab.
async fn update(ctx: JSContext, patch: JSObject) -> JSResult<()> {
    let app = LxApp::from_ctx(&ctx)?;
    let json = patch
        .to_json_string()
        .map_err(|_| js_invalid_parameter_error("tabBar: expected a JSON-compatible object"))?;
    let value: Value = serde_json::from_str(&json)
        .map_err(|error| js_invalid_parameter_error(format!("tabBar: {error}")))?;
    let tabs: Vec<String> = app
        .get_tabbar()
        .map(|tabbar| tabbar.items.into_iter().map(|item| item.page).collect())
        .unwrap_or_default();
    let patch = indexed_patch(value, &tabs).map_err(js_invalid_parameter_error)?;
    app.commit_tabbar(patch).await.map_err(|error| match error {
        lxapp::LxAppError::InvalidParameter(_) => js_invalid_parameter_error(error.to_string()),
        lxapp::LxAppError::ResourceNotFound(_) => js_service_unavailable_error(error.to_string()),
        _ => js_internal_error(error.to_string()),
    })
}

/// Map the by-name author shape onto the index-addressed patch the tab bar applies.
fn indexed_patch(value: Value, tabs: &[String]) -> Result<TabBarPatch, String> {
    let Value::Object(mut top) = value else {
        return Err("tabBar: expected an object".to_string());
    };
    if let Some(items) = top.remove("items") {
        let Value::Object(items) = items else {
            return Err("tabBar.items: expected an object keyed by tab name".to_string());
        };
        let mut indexed = Vec::with_capacity(items.len());
        for (name, item) in items {
            let index = tabs
                .iter()
                .position(|tab| *tab == name)
                .ok_or_else(|| format!("tabBar.items: {name:?} is not a tab"))?;
            let Value::Object(item) = item else {
                return Err(format!("tabBar.items.{name}: expected an object"));
            };
            let mut patch = Map::new();
            patch.insert("index".to_string(), Value::from(index));
            for (key, field) in item {
                match key.as_str() {
                    "label" => {
                        patch.insert("text".to_string(), field);
                    }
                    "icon" => {
                        patch.insert("iconPath".to_string(), field);
                    }
                    "badge" => match field {
                        Value::Bool(true) => {
                            patch.insert("redDot".to_string(), Value::Bool(true));
                        }
                        Value::Null => {
                            patch.insert("badge".to_string(), Value::Null);
                            patch.insert("redDot".to_string(), Value::Bool(false));
                        }
                        Value::String(_) => {
                            patch.insert("badge".to_string(), field);
                        }
                        _ => {
                            return Err(format!(
                                "tabBar.items.{name}.badge: expected a string, true, or null"
                            ));
                        }
                    },
                    other => return Err(format!("tabBar.items.{name}: unknown field {other:?}")),
                }
            }
            indexed.push(Value::Object(patch));
        }
        top.insert("items".to_string(), Value::Array(indexed));
    }
    serde_json::from_value(Value::Object(top)).map_err(|error| format!("tabBar: {error}"))
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_property(ctx)?;
    register_api(ctx)
}

rong::js_api! {
    fn register_property(ctx) {
        namespace LxAppApi = crate::namespace::app(ctx)?;
        const tabBar: "TabBarApi" = namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_api(ctx) {
        namespace TabBarApi = namespace(ctx)?;
        fn update(ts_params = "patch: TabBarPatch") = update;
    }
}

#[cfg(test)]
mod tests {
    use super::indexed_patch;
    use serde_json::json;

    fn tabs() -> Vec<String> {
        vec!["home".to_string(), "inbox".to_string()]
    }

    #[test]
    fn items_are_addressed_by_tab_name() {
        let patch = indexed_patch(
            json!({ "items": { "inbox": { "badge": "3", "label": "Mail" } } }),
            &tabs(),
        )
        .unwrap();
        let expected: lxapp::tabbar::TabBarPatch = serde_json::from_value(
            json!({ "items": [{ "index": 1, "badge": "3", "text": "Mail" }] }),
        )
        .unwrap();
        assert_eq!(patch, expected);
    }

    #[test]
    fn badge_true_is_a_dot_and_null_clears_both() {
        let dot =
            indexed_patch(json!({ "items": { "home": { "badge": true } } }), &tabs()).unwrap();
        let expected: lxapp::tabbar::TabBarPatch =
            serde_json::from_value(json!({ "items": [{ "index": 0, "redDot": true }] })).unwrap();
        assert_eq!(dot, expected);
        let cleared =
            indexed_patch(json!({ "items": { "home": { "badge": null } } }), &tabs()).unwrap();
        let expected: lxapp::tabbar::TabBarPatch = serde_json::from_value(
            json!({ "items": [{ "index": 0, "badge": null, "redDot": false }] }),
        )
        .unwrap();
        assert_eq!(cleared, expected);
    }

    #[test]
    fn unknown_tabs_and_style_are_refused() {
        assert!(indexed_patch(json!({ "items": { "nope": {} } }), &tabs()).is_err());
        assert!(indexed_patch(json!({ "style": {} }), &tabs()).is_err());
        assert!(
            indexed_patch(json!({ "items": { "home": { "redDot": true } } }), &tabs()).is_err()
        );
    }
}
