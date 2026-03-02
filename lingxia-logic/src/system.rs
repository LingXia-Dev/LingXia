//! System information and OS-level operations.

use crate::i18n::{
    host_error_from_platform_error, js_error_from_business_code_with_detail,
    js_error_from_platform_error, js_service_unavailable_error,
};
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_platform::traits::location::Location;
use lingxia_platform::traits::wifi::Wifi;
use lxapp::LxApp;
use lxapp::lx;
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult};
use serde::Deserialize;

/// AppBase information
#[derive(Debug, Clone, IntoJSObj)]
pub struct AppBaseInfo {
    language: String,
    #[rename = "productName"]
    product_name: String,
    #[rename = "version"]
    version: String,
    #[rename = "SDKVersion"]
    sdk_version: String,
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

fn get_system_locale(ctx: JSContext) -> JSResult<AppBaseInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let locale = lxapp.runtime.get_system_locale();
    let app_cfg = lxapp::app_config()
        .ok_or_else(|| js_service_unavailable_error("app config not available"))?;
    Ok(AppBaseInfo {
        language: locale.to_string(),
        product_name: app_cfg.product_name.clone(),
        version: app_cfg.product_version.clone(),
        sdk_version: lxapp::SDK_RUNTIME_VERSION.to_string(),
    })
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
struct JSOpenURLOptions {
    #[serde(rename = "url")]
    #[rename = "url"]
    url: String,
    /// Opens URL in external browser (default) or internal webview
    /// - "external": Open in system browser (current behavior)
    /// - "internal": Open in internal webview (future support)
    #[serde(rename = "openIn")]
    #[rename = "openIn"]
    _open_in: Option<String>,
}

fn open_url_impl(lxapp: &LxApp, options: &JSOpenURLOptions) -> JSResult<()> {
    if options.url.is_empty() {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "openURL requires url",
        ));
    }

    // TODO: Add support for openIn option in the future
    // For now, always open in external browser (ignore openIn option)
    lxapp
        .runtime
        .launch_with_url(options.url.clone())
        .map_err(|e| js_error_from_platform_error(&e))?;
    Ok(())
}

fn open_url(ctx: JSContext, options: JSOpenURLOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    open_url_impl(&lxapp, &options)
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let get_app_base_info = JSFunc::new(ctx, get_system_locale)?;
    lx::register_js_api(ctx, "getAppBaseInfo", get_app_base_info)?;

    let get_system_setting_func = JSFunc::new(ctx, get_system_setting)?;
    lx::register_js_api(ctx, "getSystemSetting", get_system_setting_func)?;

    let open_url_func = JSFunc::new(ctx, open_url)?;
    lx::register_js_api(ctx, "openURL", open_url_func)?;

    Ok(())
}
