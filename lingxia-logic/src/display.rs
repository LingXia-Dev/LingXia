//! Display and screen orientation APIs.

use crate::i18n::{js_error_from_platform_error, js_invalid_parameter_error};
use lingxia_platform::traits::ui::UIUpdate;
use lxapp::lx;
use lxapp::{
    LxApp, OrientationConfig, publish_app_event, register_app_handler, unregister_app_handler,
};
use rong::function::Optional;
use rong::{JSContext, JSFunc, JSObject, JSResult};

const DEVICE_ORIENTATION_CHANGE_EVENT: &str = "DeviceOrientationChange";
const ORIENTATION_PORTRAIT: &str = "portrait";
const ORIENTATION_LANDSCAPE: &str = "landscape";

fn normalize_orientation_value(value: &str) -> Option<&'static str> {
    match value {
        "portrait" | "reverse-portrait" => Some(ORIENTATION_PORTRAIT),
        "landscape" | "reverse-landscape" => Some(ORIENTATION_LANDSCAPE),
        _ => None,
    }
}

fn emit_orientation_change_event(appid: &str, value: &str) {
    let payload = format!(r#"{{"value":"{}"}}"#, value);
    let _ = publish_app_event(appid, DEVICE_ORIENTATION_CHANGE_EVENT, Some(payload));
}

#[inline]
fn should_emit_orientation_event_after_set() -> bool {
    // iOS/Harmony may not deliver a host orientation callback immediately after
    // setDeviceOrientation, so we actively emit one to keep JS state in sync.
    // Android already emits orientation events from Activity callbacks; emitting
    // again here would create duplicate events.
    cfg!(target_os = "ios") || cfg!(target_os = "macos") || cfg!(target_env = "ohos")
}

fn set_device_orientation(ctx: JSContext, orientation: String) -> JSResult<bool> {
    if orientation != "portrait" && orientation != "landscape" {
        return Err(js_invalid_parameter_error(format!(
            "Invalid orientation value: {} (expected portrait or landscape)",
            orientation
        )));
    }

    let lxapp = LxApp::from_ctx(&ctx)?;
    let config = OrientationConfig::from_label(&orientation).ok_or_else(|| {
        js_invalid_parameter_error(format!("Invalid orientation value: {}", orientation))
    })?;
    lxapp.set_app_orientation(config);

    lxapp
        .runtime
        .update_orientation_ui(lxapp.appid.clone())
        .map_err(|e| js_error_from_platform_error(&e))?;

    if should_emit_orientation_event_after_set() {
        emit_orientation_change_event(&lxapp.appid, &orientation);
    }

    Ok(true)
}

fn on_device_orientation_change(ctx: JSContext, callback: JSFunc) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let current_path = lxapp.peek_current_page().unwrap_or_default();
    let current = if current_path.is_empty() {
        lxapp.get_app_orientation()
    } else {
        lxapp.get_page_orientation(&current_path)
    };

    let value = normalize_orientation_value(current.to_label())
        .ok_or_else(|| js_invalid_parameter_error("Current orientation unavailable".to_string()))?;

    let callback_for_initial = callback.clone();
    register_app_handler(&ctx, DEVICE_ORIENTATION_CHANGE_EVENT, callback)?;

    let payload = JSObject::new(&ctx);
    payload.set("value", value)?;
    let _ = callback_for_initial.call::<_, ()>(None, (payload,));

    Ok(())
}

fn off_device_orientation_change(ctx: JSContext, callback: Optional<JSFunc>) -> JSResult<()> {
    unregister_app_handler(&ctx, DEVICE_ORIENTATION_CHANGE_EVENT, callback.0);
    Ok(())
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let set_device_orientation_func = JSFunc::new(ctx, set_device_orientation)?;
    lx::register_js_api(ctx, "setDeviceOrientation", set_device_orientation_func)?;

    let on_device_orientation_change_func = JSFunc::new(ctx, on_device_orientation_change)?;
    lx::register_js_api(
        ctx,
        "onDeviceOrientationChange",
        on_device_orientation_change_func,
    )?;

    let off_device_orientation_change_func = JSFunc::new(ctx, off_device_orientation_change)?;
    lx::register_js_api(
        ctx,
        "offDeviceOrientationChange",
        off_device_orientation_change_func,
    )?;

    Ok(())
}
