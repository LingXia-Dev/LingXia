//! Display and screen orientation APIs.

use crate::i18n::{js_error_from_platform_error, js_invalid_parameter_error};
use lingxia_platform::traits::ui::UIUpdate;
use lxapp::{
    LxApp, OrientationConfig, publish_app_event, register_app_handler, unregister_app_handler,
};
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

/// Subscribes to orientation changes and returns the unsubscribe fn.
fn on_device_orientation_change(ctx: JSContext, callback: JSFunc) -> JSResult<JSFunc> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let current_path = lxapp.peek_current_page_path().unwrap_or_default();
    let current = if current_path.is_empty() {
        lxapp.get_app_orientation()
    } else {
        lxapp.get_page_orientation(&current_path)
    };

    let value = normalize_orientation_value(current.to_label())
        .ok_or_else(|| js_invalid_parameter_error("Current orientation unavailable"))?;

    register_app_handler(&ctx, DEVICE_ORIENTATION_CHANGE_EVENT, callback.clone())?;

    let payload = JSObject::new(&ctx);
    payload.set("value", value)?;
    let _ = callback.call::<_, ()>(None, (payload,));

    let off_ctx = ctx.clone();
    JSFunc::new(&ctx, move || {
        unregister_app_handler(
            &off_ctx,
            DEVICE_ORIENTATION_CHANGE_EVENT,
            Some(callback.clone()),
        );
    })
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn setDeviceOrientation(ts_params = "orientation: DeviceOrientation") = set_device_orientation;
        fn onDeviceOrientationChange(
            ts_params = "callback: (event: DeviceOrientationChangeEvent) => void",
            ts_return = "() => void"
        ) = on_device_orientation_change;
    }
}
