use crate::i18n::{js_error_from_platform_error, js_internal_error};
use lingxia_messaging::{CallbackResult, register_handler, remove_callback};
use lingxia_platform::traits::appearance::{Appearance, AppearanceState};
use lxapp::LxApp;
use lxapp::{register_app_handler, unregister_app_handler};
use rong::function::Optional;
use rong::{IntoJSObject, JSContext, JSFunc, JSResult};

const APPEARANCE_CHANGE_EVENT: &str = "appearanceChange";

#[derive(Clone, Copy, Default)]
struct AppearanceCallbackId(Option<u64>);

fn set_appearance_callback_id(ctx: &JSContext, id: Option<u64>) {
    ctx.set_state(AppearanceCallbackId(id));
}

fn appearance_callback_id(ctx: &JSContext) -> Option<u64> {
    ctx.get_state::<AppearanceCallbackId>()
        .and_then(|state| state.0)
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
pub(crate) struct JSAppearanceState {
    preference: String,
    effective: String,
}

impl From<lingxia_platform::traits::appearance::AppearanceState> for JSAppearanceState {
    fn from(state: lingxia_platform::traits::appearance::AppearanceState) -> Self {
        Self {
            preference: state.preference.as_str().to_string(),
            effective: state.effective().to_string(),
        }
    }
}

fn get_appearance(_ctx: JSContext) -> JSResult<JSAppearanceState> {
    Ok(lxapp::get_appearance_state().into())
}

fn parse_native_appearance(payload: &str) -> JSResult<AppearanceState> {
    let value: serde_json::Value = serde_json::from_str(payload)
        .map_err(|error| js_internal_error(format!("Invalid appearance payload: {error}")))?;
    let preference = value
        .get("preference")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| js_internal_error("Appearance payload has no preference"))?
        .parse()
        .map_err(js_internal_error)?;
    let effective_dark = match value.get("effective").and_then(serde_json::Value::as_str) {
        Some("dark") => true,
        Some("light") => false,
        _ => {
            return Err(js_internal_error(
                "Appearance payload has invalid effective value",
            ));
        }
    };
    Ok(AppearanceState {
        preference,
        effective_dark,
    })
}

fn ensure_native_listener(ctx: &JSContext) -> JSResult<()> {
    if appearance_callback_id(ctx).is_some() {
        return Ok(());
    }

    let lxapp = LxApp::from_ctx(ctx)?;
    let callback_id = register_handler(|result| {
        if let CallbackResult::Success(payload) = result {
            match parse_native_appearance(&payload) {
                Ok(state) => lxapp::set_appearance_state(state),
                Err(error) => {
                    lxapp::warn!("Ignoring invalid native appearance event: {error}");
                }
            }
        }
    });
    if let Err(error) = lxapp.runtime.add_appearance_change_listener(callback_id) {
        remove_callback(callback_id);
        return Err(js_error_from_platform_error(&error));
    }
    set_appearance_callback_id(ctx, Some(callback_id));
    Ok(())
}

fn clear_native_listener(ctx: &JSContext) -> JSResult<()> {
    let Some(callback_id) = appearance_callback_id(ctx) else {
        return Ok(());
    };
    let lxapp = LxApp::from_ctx(ctx)?;
    lxapp
        .runtime
        .remove_appearance_change_listener(callback_id)
        .map_err(|error| js_error_from_platform_error(&error))?;
    remove_callback(callback_id);
    set_appearance_callback_id(ctx, None);
    Ok(())
}

fn on_appearance_change(ctx: JSContext, callback: JSFunc) -> JSResult<()> {
    register_app_handler(&ctx, APPEARANCE_CHANGE_EVENT, callback.clone())?;
    if let Err(error) = ensure_native_listener(&ctx) {
        let _ = unregister_app_handler(&ctx, APPEARANCE_CHANGE_EVENT, Some(callback));
        return Err(error);
    }
    Ok(())
}

fn off_appearance_change(ctx: JSContext, callback: Optional<JSFunc>) -> JSResult<()> {
    let remaining = unregister_app_handler(&ctx, APPEARANCE_CHANGE_EVENT, callback.0);
    if remaining == 0 {
        clear_native_listener(&ctx)?;
    }
    Ok(())
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn getAppearance(ts_return = "AppearanceState") = get_appearance;
        fn onAppearanceChange(ts_params = "callback: AppearanceChangeCallback") = on_appearance_change;
        fn offAppearanceChange(ts_params = "callback?: AppearanceChangeCallback") = off_appearance_change;
    }
}
