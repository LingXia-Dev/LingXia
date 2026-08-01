use crate::i18n::{js_internal_error, js_invalid_parameter_error, js_service_unavailable_error};
use lxapp::LxApp;
use lxapp::page_chrome::{PageChromeColor, VisibilityPreference};
use rong::{JSContext, JSObject, JSResult, JSValue};
use std::collections::HashSet;

fn namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("tabBar") {
        Ok(namespace) => Ok(namespace),
        Err(_) => {
            let namespace = JSObject::new(ctx);
            lx.set("tabBar", namespace.clone())?;
            Ok(namespace)
        }
    }
}

fn property(object: &JSObject, field: &str) -> Option<JSValue> {
    object
        .get::<_, JSValue>(field)
        .ok()
        .filter(|value| !value.is_undefined())
}

fn reject_unknown(object: &JSObject, allowed: &[&str], path: &str) -> JSResult<()> {
    for key in object.keys_as::<String>()? {
        if !allowed.contains(&key.as_str()) {
            return Err(js_invalid_parameter_error(format!(
                "{path}.{key}: unknown field"
            )));
        }
    }
    Ok(())
}

fn nullable_string(object: &JSObject, field: &str, path: &str) -> JSResult<Option<Option<String>>> {
    let Some(value) = property(object, field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(Some(None));
    }
    object
        .get::<_, String>(field)
        .map(|value| Some(Some(value)))
        .map_err(|_| js_invalid_parameter_error(format!("{path}: expected string or null")))
}

fn nullable_color(
    object: &JSObject,
    field: &str,
    path: &str,
) -> JSResult<Option<Option<PageChromeColor>>> {
    nullable_string(object, field, path)?
        .map(|value| {
            value
                .map(|value| {
                    let color = PageChromeColor::parse(&value).map_err(|detail| {
                        js_invalid_parameter_error(format!("{path}: {detail}"))
                    })?;
                    if !color.is_opaque() {
                        return Err(js_invalid_parameter_error(format!(
                            "{path}: expected opaque #RRGGBB"
                        )));
                    }
                    Ok(color)
                })
                .transpose()
        })
        .transpose()
}

fn resolve_icon(
    app: &LxApp,
    value: Option<Option<String>>,
    path: &str,
) -> JSResult<Option<Option<String>>> {
    value
        .map(|value| {
            value
                .map(|value| {
                    app.resolve_accessible_path(&value)
                        .map(|path| path.to_string_lossy().into_owned())
                        .map_err(|_| {
                            js_invalid_parameter_error(format!(
                                "{path}: path must stay within the lxapp package"
                            ))
                        })
                })
                .transpose()
        })
        .transpose()
}

async fn update(ctx: JSContext, patch: JSObject) -> JSResult<()> {
    reject_unknown(&patch, &["visibility", "style", "items"], "tabBar")?;
    let app = LxApp::from_ctx(&ctx)?;
    let mut candidate = app
        .get_tabbar()
        .ok_or_else(|| js_service_unavailable_error("tabBar: lxapp has no declared tabbar"))?;

    if property(&patch, "visibility").is_some() {
        let value = patch.get::<_, String>("visibility").map_err(|_| {
            js_invalid_parameter_error("tabBar.visibility: expected auto or hidden")
        })?;
        candidate.set_visibility(match value.as_str() {
            "auto" => VisibilityPreference::Auto,
            "hidden" => VisibilityPreference::Hidden,
            _ => {
                return Err(js_invalid_parameter_error(
                    "tabBar.visibility: expected auto or hidden",
                ));
            }
        });
    }

    if let Some(value) = property(&patch, "style") {
        if value.is_null() {
            candidate.runtime_style = Default::default();
        } else {
            let style = patch
                .get::<_, JSObject>("style")
                .map_err(|_| js_invalid_parameter_error("tabBar.style: expected object or null"))?;
            reject_unknown(
                &style,
                &["foregroundColor", "selectedForegroundColor"],
                "tabBar.style",
            )?;
            if let Some(value) =
                nullable_color(&style, "foregroundColor", "tabBar.style.foregroundColor")?
            {
                candidate.runtime_style.foreground_color = value;
            }
            if let Some(value) = nullable_color(
                &style,
                "selectedForegroundColor",
                "tabBar.style.selectedForegroundColor",
            )? {
                candidate.runtime_style.selected_foreground_color = value;
            }
        }
    }

    if let Some(value) = property(&patch, "items") {
        if value.is_null() {
            return Err(js_invalid_parameter_error(
                "tabBar.items: expected an array",
            ));
        }
        let items = patch
            .get::<_, Vec<JSObject>>("items")
            .map_err(|_| js_invalid_parameter_error("tabBar.items: expected an array"))?;
        let mut indexes = HashSet::new();
        for (patch_index, item) in items.iter().enumerate() {
            let path = format!("tabBar.items[{patch_index}]");
            reject_unknown(
                item,
                &[
                    "index",
                    "text",
                    "iconPath",
                    "selectedIconPath",
                    "badge",
                    "redDot",
                ],
                &path,
            )?;
            let index = item.get::<_, i32>("index").map_err(|_| {
                js_invalid_parameter_error(format!("{path}.index: expected an integer"))
            })?;
            if candidate.get_item(index).is_none() {
                return Err(js_invalid_parameter_error(format!(
                    "{path}.index: index {index} is out of range"
                )));
            }
            if !indexes.insert(index) {
                return Err(js_invalid_parameter_error(format!(
                    "{path}.index: duplicate index {index}"
                )));
            }
            let text = nullable_string(item, "text", &format!("{path}.text"))?;
            let icon = resolve_icon(
                &app,
                nullable_string(item, "iconPath", &format!("{path}.iconPath"))?,
                &format!("{path}.iconPath"),
            )?;
            let selected_icon = resolve_icon(
                &app,
                nullable_string(
                    item,
                    "selectedIconPath",
                    &format!("{path}.selectedIconPath"),
                )?,
                &format!("{path}.selectedIconPath"),
            )?;
            let badge = nullable_string(item, "badge", &format!("{path}.badge"))?;
            let red_dot = if property(item, "redDot").is_some() {
                Some(item.get::<_, bool>("redDot").map_err(|_| {
                    js_invalid_parameter_error(format!("{path}.redDot: expected boolean"))
                })?)
            } else {
                None
            };
            if badge.as_ref().is_some_and(|value| value.is_some()) && red_dot == Some(true) {
                return Err(js_invalid_parameter_error(format!(
                    "{path}: badge and redDot true are mutually exclusive"
                )));
            }
            if let Some(value) = text {
                candidate.set_item_text(index, value);
            }
            if let Some(value) = icon {
                candidate.set_item_icon(index, value);
            }
            if let Some(value) = selected_icon {
                candidate.set_item_selected_icon(index, value);
            }
            if let Some(value) = badge {
                candidate.set_badge(index, value);
            }
            if let Some(value) = red_dot {
                candidate.set_red_dot(index, value);
            }
        }
    }

    app.commit_tabbar(candidate)
        .await
        .map_err(|error| match error {
            lxapp::LxAppError::ResourceNotFound(_) => {
                js_service_unavailable_error(error.to_string())
            }
            _ => js_internal_error(error.to_string()),
        })
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_property(ctx)?;
    register_api(ctx)
}

rong::js_api! {
    fn register_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const tabBar: "TabBarApi" = namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_api(ctx) {
        namespace TabBarApi = namespace(ctx)?;
        fn update(ts_params = "patch: TabBarPatch") = update;
    }
}
