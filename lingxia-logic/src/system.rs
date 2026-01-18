//! System information and OS-level operations.

use lingxia_platform::{AppRuntime, Location, Wifi};
use lxapp::host_api;
use lxapp::lx;
use lxapp::{LxApp, LxAppError};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, RongJSError};
use serde::Deserialize;
use std::sync::Arc;

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

fn get_system_locale(ctx: JSContext) -> JSResult<AppBaseInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let locale = lxapp.runtime.get_system_locale();
    Ok(AppBaseInfo {
        language: locale.to_string(),
    })
}

fn get_system_setting(ctx: JSContext) -> JSResult<SystemSettingInfo> {
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

fn open_url_impl(lxapp: &LxApp, options: &JSOpenURLOptions) -> Result<(), LxAppError> {
    if options.url.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "openURL requires url".to_string(),
        ));
    }

    // TODO: Add support for openIn option in the future
    // For now, always open in external browser (ignore openIn option)
    lxapp
        .runtime
        .launch_with_url(options.url.clone())
        .map_err(|e| LxAppError::Runtime(format!("openURL failed: {}", e)))?;
    Ok(())
}

fn open_url(ctx: JSContext, options: JSOpenURLOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    open_url_impl(&lxapp, &options).map_err(|e| RongJSError::Error(e.to_string()))
}

host_api!(
    OpenURL,
    JSOpenURLOptions,
    (),
    |lxapp: Arc<LxApp>, options: JSOpenURLOptions| -> Result<(), LxAppError> {
        open_url_impl(&lxapp, &options)
    }
);

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let get_app_base_info = JSFunc::new(ctx, get_system_locale)?;
    lx::register_js_api(ctx, "getAppBaseInfo", get_app_base_info)?;

    let get_system_setting_func = JSFunc::new(ctx, get_system_setting)?;
    lx::register_js_api(ctx, "getSystemSetting", get_system_setting_func)?;

    let open_url_func = JSFunc::new(ctx, open_url)?;
    lx::register_js_api(ctx, "openURL", open_url_func)?;

    lxapp::register_host("openURL", Arc::new(OpenURL));

    Ok(())
}
