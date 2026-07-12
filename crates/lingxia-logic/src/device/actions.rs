use crate::i18n::js_error_from_platform_error;
use lingxia_platform::traits::device::Device;
use lxapp::LxApp;
use rong::{FromJSObject, JSContext, JSResult};
use serde::Deserialize;

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn vibrateShort = vibrate_short;
        fn vibrateLong = vibrate_long;
        fn makePhoneCall(ts_params = "options: MakePhoneCallOptions") = make_phone_call;
    }
}

fn vibrate_short(ctx: JSContext) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .vibrate(false)
        .map(|_| true)
        .map_err(|e| js_error_from_platform_error(&e))
}

fn vibrate_long(ctx: JSContext) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .vibrate(true)
        .map(|_| true)
        .map_err(|e| js_error_from_platform_error(&e))
}

#[derive(FromJSObject, Deserialize)]
struct MakePhoneCallParams {
    #[serde(rename = "phoneNumber")]
    #[js_name = "phoneNumber"]
    phone_number: String,
}

fn make_phone_call(ctx: JSContext, params: MakePhoneCallParams) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .make_phone_call(&params.phone_number)
        .map(|_| true)
        .map_err(|e| js_error_from_platform_error(&e))
}
