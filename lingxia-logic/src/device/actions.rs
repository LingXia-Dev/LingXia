use lingxia_platform::Device;
use lxapp::lx::{self, fast_api};
use lxapp::{LxApp, LxAppError};
use rong::{FromJSObj, JSContext, JSFunc, JSResult, RongJSError};
use serde::Deserialize;
use std::sync::Arc;

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let vibrate_short_func = JSFunc::new(ctx, vibrate_short)?;
    lx::register_js_api(ctx, "vibrateShort", vibrate_short_func)?;

    let vibrate_long_func = JSFunc::new(ctx, vibrate_long)?;
    lx::register_js_api(ctx, "vibrateLong", vibrate_long_func)?;

    let make_phone_call_func = JSFunc::new(ctx, make_phone_call)?;
    lx::register_js_api(ctx, "makePhoneCall", make_phone_call_func)?;

    lx::register_fast_api("makePhoneCall", Arc::new(MakePhoneCall));
    Ok(())
}

fn vibrate_short(ctx: JSContext) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .vibrate(false)
        .map(|_| true)
        .map_err(|e| RongJSError::Error(format!("Failed to vibrate short: {}", e)))
}

fn vibrate_long(ctx: JSContext) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .vibrate(true)
        .map(|_| true)
        .map_err(|e| RongJSError::Error(format!("Failed to vibrate long: {}", e)))
}

#[derive(FromJSObj, Deserialize)]
struct MakePhoneCallParams {
    #[serde(rename = "phoneNumber")]
    #[rename = "phoneNumber"]
    phone_number: String,
}

fn make_phone_call(ctx: JSContext, params: MakePhoneCallParams) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .make_phone_call(&params.phone_number)
        .map(|_| true)
        .map_err(|e| RongJSError::Error(format!("Failed to make phone call: {}", e)))
}

fast_api!(
    MakePhoneCall,
    MakePhoneCallParams,
    bool,
    |lxapp: Arc<LxApp>, params: MakePhoneCallParams| -> Result<bool, LxAppError> {
        lxapp
            .runtime
            .make_phone_call(&params.phone_number)
            .map(|_| true)
            .map_err(|e| LxAppError::Runtime(format!("Failed to make phone call: {}", e)))
    }
);
