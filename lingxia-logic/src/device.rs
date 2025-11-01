use lingxia_lxapp::lx::fast_api;
use lingxia_lxapp::{LxApp, LxAppError, lx};
use lingxia_messaging::{CallbackResult};
use lingxia_platform::{Device, DeviceInfo, ScreenInfo};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, RongJSError};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

/// Device information
#[derive(Debug, Clone, IntoJSObj)]
pub struct DevInfoObj {
    brand: String,
    model: String,
    system: String, // Operating system version
}

/// Screen information
#[derive(Debug, Clone, IntoJSObj)]
pub struct ScreenInfoObj {
    width: f64,
    height: f64,
    scale: f64,
}

impl From<DeviceInfo> for DevInfoObj {
    fn from(device_info: DeviceInfo) -> Self {
        DevInfoObj {
            brand: device_info.brand,
            model: device_info.model,
            system: device_info.system,
        }
    }
}

impl From<ScreenInfo> for ScreenInfoObj {
    fn from(screen_info: ScreenInfo) -> Self {
        ScreenInfoObj {
            width: screen_info.width,
            height: screen_info.height,
            scale: screen_info.scale,
        }
    }
}

impl From<CallbackResult> for ScreenInfoObj {
    fn from(result: CallbackResult) -> Self {
        if !result.success {
            return ScreenInfoObj {
                width: 0.0,
                height: 0.0,
                scale: 1.0,
            };
        }

        match serde_json::from_str::<Value>(&result.data) {
            Ok(json) => ScreenInfoObj {
                width: json.get("width").and_then(Value::as_f64).unwrap_or(0.0),
                height: json.get("height").and_then(Value::as_f64).unwrap_or(0.0),
                scale: json.get("scale").and_then(Value::as_f64).unwrap_or(1.0),
            },
            Err(_) => ScreenInfoObj {
                width: 0.0,
                height: 0.0,
                scale: 1.0,
            },
        }
    }
}

// Parameter structs for fast_api object parameters
// Phone call parameters using FromJSObj for universal object handling
#[derive(FromJSObj, Deserialize)]
struct MakePhoneCallParams {
    #[serde(rename = "phoneNumber")]
    #[rename = "phoneNumber"]
    phone_number: String,
}

pub(crate) fn device_info(ctx: JSContext) -> JSResult<DevInfoObj> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    let device_info = lxapp.runtime.device_info();
    Ok(device_info.into())
}

fn screen_info(ctx: JSContext) -> JSResult<ScreenInfoObj> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    let info = lxapp.runtime.screen_info();
    Ok(info.into())
}

pub(crate) fn vibrate_short(ctx: JSContext) -> JSResult<bool> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    match lxapp.runtime.vibrate(false) {
        Ok(_) => Ok(true),
        Err(e) => Err(RongJSError::Error(format!(
            "Failed to vibrate short: {}",
            e
        ))),
    }
}

pub(crate) fn vibrate_long(ctx: JSContext) -> JSResult<bool> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    match lxapp.runtime.vibrate(true) {
        Ok(_) => Ok(true),
        Err(e) => Err(RongJSError::Error(format!("Failed to vibrate long: {}", e))),
    }
}

fn make_phone_call(ctx: JSContext, params: MakePhoneCallParams) -> JSResult<bool> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();

    match lxapp.runtime.make_phone_call(&params.phone_number) {
        Ok(()) => Ok(true),
        Err(e) => Err(RongJSError::Error(format!(
            "Failed to make phone call: {}",
            e
        ))),
    }
}

// Make phone call fast_api (with object parameter using FromJSObj)
fast_api!(
    MakePhoneCall,
    MakePhoneCallParams,
    bool,
    |lxapp: Arc<LxApp>, params: MakePhoneCallParams| -> Result<bool, LxAppError> {
        match lxapp.runtime.make_phone_call(&params.phone_number) {
            Ok(()) => Ok(true),
            Err(e) => Err(LxAppError::Runtime(format!(
                "Failed to make phone call: {}",
                e
            ))),
        }
    }
);

pub fn init(ctx: &JSContext) -> JSResult<()> {
    // Device info APIs
    let device_info_func = JSFunc::new(ctx, device_info)?;
    lx::register_js_api(ctx, "getDeviceInfo", device_info_func)?;

    let screen_info_func = JSFunc::new(ctx, screen_info)?;
    lx::register_js_api(ctx, "getScreenInfo", screen_info_func)?;

    // Device action APIs - split vibrate into two functions
    let vibrate_short_func = JSFunc::new(ctx, vibrate_short)?;
    lx::register_js_api(ctx, "vibrateShort", vibrate_short_func)?;

    let vibrate_long_func = JSFunc::new(ctx, vibrate_long)?;
    lx::register_js_api(ctx, "vibrateLong", vibrate_long_func)?;

    // Phone call API - both regular JS API and Fast API
    let make_phone_call_func = JSFunc::new(ctx, make_phone_call)?;
    lx::register_js_api(ctx, "makePhoneCall", make_phone_call_func)?;

    // makePhoneCall also uses Fast API for object parameter handling
    lx::register_fast_api("makePhoneCall", Arc::new(MakePhoneCall));

    Ok(())
}
