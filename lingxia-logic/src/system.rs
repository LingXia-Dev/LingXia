use lingxia_platform::{AppRuntime, Location, UIUpdate, Wifi};
use lxapp::{LxApp, lx};
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

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let get_app_base_info = JSFunc::new(ctx, get_system_locale)?;
    lx::register_js_api(ctx, "getAppBaseInfo", get_app_base_info)?;

    let get_system_setting_func = JSFunc::new(ctx, get_system_setting)?;
    lx::register_js_api(ctx, "getSystemSetting", get_system_setting_func)?;

    Ok(())
}
