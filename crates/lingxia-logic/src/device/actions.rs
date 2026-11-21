use crate::i18n::{js_error_from_platform_error, js_invalid_parameter_error};
use lingxia_platform::traits::device::Device;
use lxapp::LxApp;
use rong::{JSContext, JSResult, function::Optional};

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace DeviceApi = crate::namespace::device(ctx)?;
        fn haptics(ts_params = "kind?: 'short' | 'long'", ts_return = "void") = haptics;
    }
}

/// A vibration pulse, where the device has a vibrator: `short` (default) or
/// `long`. Rejects `E_NOT_SUPPORTED` where it has none.
fn haptics(ctx: JSContext, kind: Optional<String>) -> JSResult<()> {
    let long = match kind.0.as_deref() {
        None | Some("short") => false,
        Some("long") => true,
        Some(other) => {
            return Err(js_invalid_parameter_error(format!(
                "haptics takes 'short' or 'long', not '{other}'"
            )));
        }
    };
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .vibrate(long)
        .map_err(|e| js_error_from_platform_error(&e))
}
