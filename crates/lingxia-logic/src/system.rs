//! System information and OS-level operations.

use crate::i18n::{
    host_error_from_platform_error, js_error_from_business_code_with_detail,
    js_error_from_platform_error,
};
use lingxia_platform::traits::app_runtime::{AppRuntime, OpenUrlRequest, OpenUrlTarget};
use lingxia_platform::traits::location::Location;
use lingxia_platform::traits::wifi::Wifi;
use lxapp::{LxApp, lx};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult};
use serde::Deserialize;

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

#[derive(FromJSObj, Deserialize)]
#[serde(deny_unknown_fields)]
struct JSOpenURLOptions {
    #[serde(rename = "url")]
    #[rename = "url"]
    url: String,
    /// URL open target.
    /// - "external": Open in system default browser
    /// - "self": Open inside current app on macOS; other platforms currently fall back to external
    #[serde(rename = "target")]
    #[rename = "target"]
    target: Option<String>,
}

fn open_url_impl(lxapp: &LxApp, options: &JSOpenURLOptions) -> JSResult<()> {
    if options.url.trim().is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "openURL requires url",
        ));
    }

    let target = OpenUrlTarget::parse(options.target.as_deref());
    lxapp
        .runtime
        .open_url(OpenUrlRequest {
            owner_appid: lxapp.appid.clone(),
            owner_session_id: lxapp.session_id(),
            url: options.url.clone(),
            target,
        })
        .map_err(|e| js_error_from_platform_error(&e))?;
    Ok(())
}

fn open_url(ctx: JSContext, options: JSOpenURLOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    open_url_impl(&lxapp, &options)
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let get_system_setting_func = JSFunc::new(ctx, get_system_setting)?;
    lx::register_js_api(ctx, "getSystemSetting", get_system_setting_func)?;

    let open_url_func = JSFunc::new(ctx, open_url)?;
    lx::register_js_api(ctx, "openURL", open_url_func)?;

    Ok(())
}
