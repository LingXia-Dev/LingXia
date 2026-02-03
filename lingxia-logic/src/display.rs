//! Display and screen orientation APIs.

use lingxia_platform::traits::ui::UIUpdate;
use lxapp::lx;
use lxapp::{LxApp, OrientationConfig};
use rong::{FromJSObj, HostError, IntoJSObj, JSContext, JSFunc, JSResult};

/// App orientation status
#[derive(Debug, Clone, IntoJSObj)]
pub struct AppOrientationInfo {
    orientation: String,
}

#[derive(FromJSObj)]
struct SetAppOrientationOptions {
    orientation: String,
}

impl From<OrientationConfig> for AppOrientationInfo {
    fn from(config: OrientationConfig) -> Self {
        Self {
            orientation: config.to_label().to_string(),
        }
    }
}

fn get_app_orientation(ctx: JSContext) -> JSResult<AppOrientationInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    Ok(lxapp.get_app_orientation().into())
}

fn set_app_orientation(ctx: JSContext, options: SetAppOrientationOptions) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let config = OrientationConfig::from_label(&options.orientation).ok_or_else(|| {
        HostError::new(
            rong::error::E_INTERNAL,
            format!("Invalid orientation value: {}", options.orientation),
        )
    })?;
    lxapp.set_app_orientation(config);

    if let Err(e) = lxapp.runtime.update_orientation_ui(lxapp.appid.clone()) {
        eprintln!("Failed to update orientation UI: {}", e);
        return Ok(false);
    }

    Ok(true)
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let get_app_orientation_func = JSFunc::new(ctx, get_app_orientation)?;
    lx::register_js_api(ctx, "getAppOrientation", get_app_orientation_func)?;

    let set_app_orientation_func = JSFunc::new(ctx, set_app_orientation)?;
    lx::register_js_api(ctx, "setAppOrientation", set_app_orientation_func)?;

    Ok(())
}
