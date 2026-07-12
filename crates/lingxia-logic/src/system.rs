//! System information and OS-level operations.

use crate::i18n::host_error_from_platform_error;
use lingxia_platform::traits::location::Location;
use lingxia_platform::traits::wifi::Wifi;
use lxapp::{LxApp, lx};
use rong::{IntoJSObject, JSContext, JSFunc, JSResult};

/// System setting status
#[derive(Debug, Clone, IntoJSObject)]
pub struct SystemSettingInfo {
    #[js_name = "bluetoothEnabled"]
    bluetooth_enabled: bool,
    #[js_name = "locationEnabled"]
    location_enabled: bool,
    #[js_name = "wifiEnabled"]
    wifi_enabled: bool,
}

fn get_system_setting(ctx: JSContext) -> JSResult<SystemSettingInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let location_enabled = lxapp
        .runtime
        .is_location_enabled()
        .map_err(|e| host_error_from_platform_error(&e))?;
    let wifi_enabled = lxapp
        .runtime
        .is_wifi_enabled()
        .map_err(|e| host_error_from_platform_error(&e))?;

    Ok(SystemSettingInfo {
        bluetooth_enabled: false,
        location_enabled,
        wifi_enabled,
    })
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let get_system_setting_func = JSFunc::new(ctx, get_system_setting)?;
    lx::register_js_api(ctx, "getSystemSetting", get_system_setting_func)?;

    Ok(())
}
