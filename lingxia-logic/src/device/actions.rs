use lingxia_platform::traits::device::Device;
use lxapp::{LxApp, lx};
use rong::{FromJSObj, JSContext, JSFunc, JSResult, RongJSError, error::HostError};
use serde::Deserialize;

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let vibrate_short_func = JSFunc::new(ctx, vibrate_short)?;
    lx::register_js_api(ctx, "vibrateShort", vibrate_short_func)?;

    let vibrate_long_func = JSFunc::new(ctx, vibrate_long)?;
    lx::register_js_api(ctx, "vibrateLong", vibrate_long_func)?;

    let make_phone_call_func = JSFunc::new(ctx, make_phone_call)?;
    lx::register_js_api(ctx, "makePhoneCall", make_phone_call_func)?;

    Ok(())
}

fn vibrate_short(ctx: JSContext) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp.runtime.vibrate(false).map(|_| true).map_err(|e| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            format!("Failed to vibrate short: {}", e),
        ))
    })
}

fn vibrate_long(ctx: JSContext) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp.runtime.vibrate(true).map(|_| true).map_err(|e| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            format!("Failed to vibrate long: {}", e),
        ))
    })
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
        .map_err(|e| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                format!("Failed to make phone call: {}", e),
            ))
        })
}
