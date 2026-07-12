//! System information and OS-level operations.

use crate::i18n::host_error_from_platform_error;
use lingxia_platform::traits::location::Location;
use lingxia_platform::traits::wifi::Wifi;
use lxapp::LxApp;
use rong::{IntoJSObject, JSContext, JSResult};

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
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn getSystemSetting(ts_return = "PublicSystemSettingInfo") = get_system_setting;
    }
}
