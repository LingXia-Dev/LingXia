//! Display and screen orientation APIs.

use crate::i18n::{js_error_from_platform_error, js_invalid_parameter_error};
use lingxia_platform::traits::ui::UIUpdate;
use lxapp::lx;
use lxapp::{LxApp, OrientationConfig, register_app_handler, unregister_app_handler};
use rong::function::Optional;
use rong::{JSContext, JSFunc, JSResult};

const DEVICE_ORIENTATION_CHANGE_EVENT: &str = "DeviceOrientationChange";

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

    Ok(true)
}

fn on_device_orientation_change(ctx: JSContext, callback: JSFunc) -> JSResult<()> {
    register_app_handler(&ctx, DEVICE_ORIENTATION_CHANGE_EVENT, callback)?;
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
