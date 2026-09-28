//! JS result names are independent of the shared CLI wire format.
use serde_json::Value;

fn rename(value: &mut Value, fields: &[(&str, &str)]) {
    if let Some(object) = value.as_object_mut() {
        for &(wire, js) in fields {
            if let Some(value) = object.remove(wire) {
                object.insert(js.into(), value);
            }
        }
    }
}

pub(crate) fn runtime_info(mut value: Value) -> Value {
    rename(
        &mut value,
        &[
            ("app_name", "appName"),
            ("release_type", "releaseType"),
            ("session_id", "sessionId"),
            ("in_stack", "inStack"),
            ("is_home", "isHome"),
            ("current_page", "currentPage"),
            ("initial_route", "initialRoute"),
            ("pages_count", "pagesCount"),
            ("page_entries", "pageEntries"),
            ("page_stack", "pageStack"),
            ("tab_bar", "tabBar"),
            ("navigation_bar", "navigationBar"),
            ("lxapp_dir", "lxappDir"),
            ("data_dir", "dataDir"),
            ("cache_dir", "cacheDir"),
            ("logic_features", "logicFeatures"),
            ("appid", "appId"),
        ],
    );
    rename(
        &mut value["navigationBar"],
        &[
            ("home_button", "homeButton"),
            ("home_button_visible", "homeButtonVisible"),
            ("runtime_style", "runtimeStyle"),
        ],
    );
    if let Some(style) = value["navigationBar"].get_mut("runtimeStyle") {
        rename(
            style,
            &[
                ("background_color", "backgroundColor"),
                ("foreground_color", "foregroundColor"),
                ("divider_color", "dividerColor"),
            ],
        );
    }
    rename(
        &mut value["tabBar"],
        &[
            ("route_visible", "routeVisible"),
            ("effective_visible", "effectiveVisible"),
            ("selected_index", "selectedIndex"),
        ],
    );
    if let Some(items) = value["tabBar"]
        .get_mut("items")
        .and_then(Value::as_array_mut)
    {
        for item in items {
            rename(item, &[("icon_path", "iconPath"), ("red_dot", "redDot")]);
        }
    }
    value
}

fn element(value: &mut Value) {
    rename(
        value,
        &[
            ("aria_label", "ariaLabel"),
            ("text_truncated", "textTruncated"),
            ("value_truncated", "valueTruncated"),
        ],
    );
    if let Some(rect) = value.get_mut("rect") {
        rename(
            rect,
            &[
                ("center_x", "centerX"),
                ("center_y", "centerY"),
                ("viewport_width", "viewportWidth"),
                ("viewport_height", "viewportHeight"),
            ],
        );
    }
}

pub(crate) fn page_query(mut value: Value) -> Value {
    if let Some(items) = value.get_mut("items").and_then(Value::as_array_mut) {
        for item in items {
            element(item);
        }
    } else {
        element(&mut value);
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn runtime_info_keeps_cli_wire_names_and_preserves_dictionary_keys() {
        let info = lxapp::LxAppRuntimeInfo {
            appid: "demo".into(),
            app_name: "Demo".into(),
            version: "1".into(),
            release_type: "draft".into(),
            session_id: 1,
            status: "opened".into(),
            in_stack: true,
            is_home: true,
            current_page: None,
            initial_route: "pages/home/index".into(),
            pages_count: 0,
            page_entries: vec![],
            page_stack: vec![],
            tab_bar: None,
            navigation_bar: None,
            lxapp_dir: "lxapp".into(),
            data_dir: "data".into(),
            cache_dir: "cache".into(),
            logic_features: [(
                "context_with_underscores".into(),
                vec!["feature_name".into()],
            )]
            .into(),
        };
        let cli = serde_json::to_value(&info).unwrap();
        let js = runtime_info(cli.clone());
        assert_eq!(
            js,
            json!({
                "appId": "demo", "appName": "Demo", "version": "1", "releaseType": "draft",
                "sessionId": 1, "status": "opened", "inStack": true, "isHome": true,
                "currentPage": null, "initialRoute": "pages/home/index", "pagesCount": 0,
                "pageEntries": [], "pageStack": [], "tabBar": null, "navigationBar": null,
                "lxappDir": "lxapp", "dataDir": "data", "cacheDir": "cache",
                "logicFeatures": {"context_with_underscores": ["feature_name"]}
            })
        );
        assert_eq!(serde_json::to_value(info).unwrap(), cli);
        assert_eq!(cli["app_name"], "Demo");
        assert!(cli.get("appName").is_none());
    }

    #[test]
    fn runtime_chrome_converts_each_nested_record() {
        let js = runtime_info(json!({
            "tab_bar": {"presentation": "standard", "visibility": "auto", "route_visible": true,
                "effective_visible": true, "selected_index": 0,
                "items": [{"index": 0, "text": "home_button", "icon_path": "my_icon.png", "badge": null, "red_dot": false}]},
            "navigation_bar": {"title": "Home", "home_button": "auto", "home_button_visible": true,
                "runtime_style": {"background_color": null, "foreground_color": "#fff", "divider_color": null}}
        }));
        assert_eq!(
            js,
            json!({
                "tabBar": {"presentation": "standard", "visibility": "auto", "routeVisible": true,
                    "effectiveVisible": true, "selectedIndex": 0,
                    "items": [{"index": 0, "text": "home_button", "iconPath": "my_icon.png", "badge": null, "redDot": false}]},
                "navigationBar": {"title": "Home", "homeButton": "auto", "homeButtonVisible": true,
                    "runtimeStyle": {"backgroundColor": null, "foregroundColor": "#fff", "dividerColor": null}}
            })
        );
    }

    #[test]
    fn query_converts_single_and_all_matches_without_changing_misses() {
        let wire = json!({"exists": true, "aria_label": "center_x", "text_truncated": false,
            "value_truncated": true, "rect": {"left": 1, "center_x": 2, "center_y": 3,
            "viewport_width": 100, "viewport_height": 200}});
        let js = json!({"exists": true, "ariaLabel": "center_x", "textTruncated": false,
            "valueTruncated": true, "rect": {"left": 1, "centerX": 2, "centerY": 3,
            "viewportWidth": 100, "viewportHeight": 200}});
        assert_eq!(page_query(wire.clone()), js);
        assert_eq!(
            page_query(json!({"count": 1, "items": [wire]})),
            json!({"count": 1, "items": [js]})
        );
        for miss in [
            json!({"exists": false, "count": 0, "index": 0, "visible": false}),
            json!({"count": 0, "items": []}),
        ] {
            assert_eq!(page_query(miss.clone()), miss);
        }
    }
}
