use crate::i18n::{js_internal_error, js_invalid_parameter_error, js_service_unavailable_error};
use lxapp::LxApp;
use lxapp::navbar::NavigationBarStyle;
use lxapp::page_chrome::{PageChromeColor, VisibilityPreference};
use rong::{JSContext, JSObject, JSResult, JSValue};

fn namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("navigationBar") {
        Ok(namespace) => Ok(namespace),
        Err(_) => {
            let namespace = JSObject::new(ctx);
            lx.set("navigationBar", namespace.clone())?;
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
    opaque: bool,
) -> JSResult<Option<Option<PageChromeColor>>> {
    let Some(value) = nullable_string(object, field, path)? else {
        return Ok(None);
    };
    let parsed = value
        .map(|value| {
            let color = PageChromeColor::parse(&value)
                .map_err(|detail| js_invalid_parameter_error(format!("{path}: {detail}")))?;
            if opaque && !color.is_opaque() {
                return Err(js_invalid_parameter_error(format!(
                    "{path}: expected opaque #RRGGBB"
                )));
            }
            Ok(color)
        })
        .transpose()?;
    Ok(Some(parsed))
}

async fn update(ctx: JSContext, patch: JSObject) -> JSResult<()> {
    reject_unknown(&patch, &["title", "homeButton", "style"], "navigationBar")?;
    let app = LxApp::from_ctx(&ctx)?;
    let page = app
        .current_page()
        .map_err(|_| js_service_unavailable_error("navigationBar: no active page"))?;
    let mut candidate = page
        .get_navbar_state()
        .ok_or_else(|| js_service_unavailable_error("navigationBar: active page is unavailable"))?;

    if let Some(title) = nullable_string(&patch, "title", "navigationBar.title")? {
        candidate.set_runtime_title(title);
    }
    if property(&patch, "homeButton").is_some() {
        let value = patch.get::<_, String>("homeButton").map_err(|_| {
            js_invalid_parameter_error("navigationBar.homeButton: expected auto or hidden")
        })?;
        candidate.set_home_button_preference(match value.as_str() {
            "auto" => VisibilityPreference::Auto,
            "hidden" => VisibilityPreference::Hidden,
            _ => {
                return Err(js_invalid_parameter_error(
                    "navigationBar.homeButton: expected auto or hidden",
                ));
            }
        });
    }
    if let Some(style_value) = property(&patch, "style") {
        if style_value.is_null() {
            candidate.clear_runtime_style();
        } else {
            let style = patch.get::<_, JSObject>("style").map_err(|_| {
                js_invalid_parameter_error("navigationBar.style: expected object or null")
            })?;
            reject_unknown(
                &style,
                &["backgroundColor", "foregroundColor", "dividerColor"],
                "navigationBar.style",
            )?;
            let mut next: NavigationBarStyle = candidate.runtime_style;
            if let Some(value) = nullable_color(
                &style,
                "backgroundColor",
                "navigationBar.style.backgroundColor",
                true,
            )? {
                next.background_color = value;
            }
            if let Some(value) = nullable_color(
                &style,
                "foregroundColor",
                "navigationBar.style.foregroundColor",
                true,
            )? {
                next.foreground_color = value;
            }
            if let Some(value) = nullable_color(
                &style,
                "dividerColor",
                "navigationBar.style.dividerColor",
                false,
            )? {
                next.divider_color = value;
            }
            candidate.runtime_style = next;
        }
    }

    app.commit_navigation_bar(page, candidate)
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
        const navigationBar: "NavigationBarApi" = namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_api(ctx) {
        namespace NavigationBarApi = namespace(ctx)?;
        fn update(ts_params = "patch: NavigationBarPatch") = update;
    }
}
