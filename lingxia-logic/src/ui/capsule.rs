use lingxia_messaging::get_callback;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lxapp::{LxApp, lx};
use rong::{IntoJSObj, JSContext, JSFunc, JSResult, RongJSError, error::HostError};
use serde::Deserialize;

/// Get capsule button bounding client rect
/// Returns object with format: {"width": 84.5, "height": 32, "top": 50, "right": 375, "bottom": 82, "left": 290.5}
#[derive(Debug, Clone, Default, Deserialize, IntoJSObj)]
struct JSCapsuleRect {
    width: Option<f64>,
    height: Option<f64>,
    top: Option<f64>,
    right: Option<f64>,
    bottom: Option<f64>,
    left: Option<f64>,
}

/// Get capsule button bounding client rect (async)
/// Returns Promise<{width, height, top, right, bottom, left}>
async fn get_capsule_rect(ctx: JSContext) -> JSResult<JSCapsuleRect> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    let (callback_id, receiver) = get_callback();

    match lxapp.runtime.get_capsule_rect(callback_id) {
        Ok(()) => match receiver.await {
            Ok(result) => match result.into_result() {
                Ok(json_str) => {
                    let rect: JSCapsuleRect = serde_json::from_str(&json_str).unwrap_or_default();
                    Ok(rect)
                }
                Err(code) => Err(RongJSError::from(HostError::new(
                    rong::error::E_INTERNAL,
                    format!("Failed to get capsule rect: error code {}", code),
                ))),
            },
            Err(_) => Err(RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                "getCapsuleRect callback timeout or cancelled",
            ))),
        },
        Err(e) => Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            format!("Failed to get capsule rect: {}", e),
        ))),
    }
}

/// Initialize capsule button functions
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    // Register getCapsuleRect function
    let func = JSFunc::new(ctx, get_capsule_rect)?;
    lx::register_js_api(ctx, "getCapsuleRect", func)?;
    Ok(())
}
