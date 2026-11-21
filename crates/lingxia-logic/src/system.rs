//! System information and OS-level operations.

use crate::i18n::host_error_from_platform_error;
use lingxia_platform::traits::location::Location;
use lingxia_platform::traits::wifi::Wifi;
use lxapp::LxApp;
use rong::{IntoJSObject, JSContext, JSResult};

/// System setting status
#[derive(Debug, Clone, IntoJSObject)]
pub struct SystemSettingInfo {
    /// Location services are on for the device.
    #[js_name = "locationEnabled"]
    location_enabled: bool,
    /// Wi-Fi is on.
    #[js_name = "wifiEnabled"]
    wifi_enabled: bool,
}

/// Read system switches the lxapp may branch on, such as location and WiFi.
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
        location_enabled,
        wifi_enabled,
    })
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace DeviceApi = crate::namespace::device(ctx)?;
        fn systemSettings(ts_return = "SystemSettingInfo") = get_system_setting;
    }
}
