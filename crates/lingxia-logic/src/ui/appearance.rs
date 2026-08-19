use crate::i18n::{js_internal_error, js_invalid_parameter_error, js_service_unavailable_error};
use lxapp::LxApp;
use lxapp::page_chrome::AppearancePreference;
use rong::{IntoJSObject, JSContext, JSObject, JSResult};

#[derive(Debug, Clone, IntoJSObject)]
struct AppearanceState {
    #[ts_type = "AppearancePreference"]
    preference: String,
    #[ts_type = "ResolvedAppearance"]
    resolved: String,
}

/// Light/dark appearance: what is in effect now, and changes to it.
fn namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("appearance") {
        Ok(namespace) => Ok(namespace),
        Err(_) => {
            let namespace = JSObject::new(ctx);
            lx.set("appearance", namespace.clone())?;
            Ok(namespace)
        }
    }
}

/// Read the appearance preference and the light/dark value it resolves to.
fn get(ctx: JSContext) -> JSResult<AppearanceState> {
    let state = LxApp::from_ctx(&ctx)?.appearance_state();
    Ok(AppearanceState {
        preference: state.preference.as_str().to_string(),
        resolved: state.resolved.as_str().to_string(),
    })
}

/// Set the appearance preference to `auto`, `light`, or `dark`.
async fn set(ctx: JSContext, preference: String) -> JSResult<()> {
    let preference = preference
        .parse::<AppearancePreference>()
        .map_err(js_invalid_parameter_error)?;
    LxApp::from_ctx(&ctx)?
        .set_appearance_preference(preference)
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
        const appearance: "AppearanceApi" = namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_api(ctx) {
        namespace AppearanceApi = namespace(ctx)?;
        fn get(ts_return = "AppearanceState") = get;
        fn set(ts_params = "preference: AppearancePreference") = set;
    }
}
