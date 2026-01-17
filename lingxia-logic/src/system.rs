use lingxia_platform::{AppRuntime, Location, UIUpdate, Wifi};
use lxapp::{LxApp, OrientationConfig, lx};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, RongJSError};

/// AppBase information
#[derive(Debug, Clone, IntoJSObj)]
pub struct AppBaseInfo {
    language: String,
}

/// System setting status
#[derive(Debug, Clone, IntoJSObj)]
pub struct SystemSettingInfo {
    #[rename = "bluetoothEnabled"]
    bluetooth_enabled: bool,
    #[rename = "locationEnabled"]
    location_enabled: bool,
    #[rename = "wifiEnabled"]
    wifi_enabled: bool,
}

/// App orientation status
#[derive(Debug, Clone, IntoJSObj)]
pub struct AppOrientationInfo {
    orientation: String,
}

#[derive(FromJSObj)]
pub(crate) struct SetAppOrientationOptions {
    orientation: String,
}

impl From<OrientationConfig> for AppOrientationInfo {
    fn from(config: OrientationConfig) -> Self {
        Self {
            orientation: config.to_label().to_string(),
        }
    }
}

pub(crate) fn get_system_locale(ctx: JSContext) -> JSResult<AppBaseInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let locale = lxapp.runtime.get_system_locale();
    Ok(AppBaseInfo {
        language: locale.to_string(),
    })
}

pub(crate) fn get_system_setting(ctx: JSContext) -> JSResult<SystemSettingInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let location_enabled = lxapp
        .runtime
        .is_location_enabled()
        .map_err(|e| RongJSError::Error(format!("Failed to get location setting: {}", e)))?;
    let wifi_enabled = lxapp
        .runtime
        .is_wifi_enabled()
        .map_err(|e| RongJSError::Error(format!("Failed to get WiFi setting: {}", e)))?;

    Ok(SystemSettingInfo {
        bluetooth_enabled: false,
        location_enabled,
        wifi_enabled,
    })
}

pub(crate) fn get_app_orientation(ctx: JSContext) -> JSResult<AppOrientationInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    Ok(lxapp.get_app_orientation().into())
}

pub(crate) fn set_app_orientation(
    ctx: JSContext,
    options: SetAppOrientationOptions,
) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let config = OrientationConfig::from_label(&options.orientation).ok_or_else(|| {
        RongJSError::Error(format!(
            "Invalid orientation value: {}",
            options.orientation
        ))
    })?;
    lxapp
        .set_app_orientation(config)
        .map_err(|e| RongJSError::Error(format!("Failed to set app orientation: {}", e)))?;

    if let Err(e) = lxapp.runtime.update_orientation_ui(lxapp.appid.clone()) {
        eprintln!("Failed to update orientation UI: {}", e);
        return Ok(false);
    }

    Ok(true)
}

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let get_app_base_info = JSFunc::new(ctx, get_system_locale)?;
    lx::register_js_api(ctx, "getAppBaseInfo", get_app_base_info)?;

    let get_system_setting_func = JSFunc::new(ctx, get_system_setting)?;
    lx::register_js_api(ctx, "getSystemSetting", get_system_setting_func)?;

    let get_app_orientation_func = JSFunc::new(ctx, get_app_orientation)?;
    lx::register_js_api(ctx, "getAppOrientation", get_app_orientation_func)?;

    let set_app_orientation_func = JSFunc::new(ctx, set_app_orientation)?;
    lx::register_js_api(ctx, "setAppOrientation", set_app_orientation_func)?;

    Ok(())
}
