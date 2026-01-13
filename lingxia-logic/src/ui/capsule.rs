use lingxia_platform::AppRuntime;
use lxapp::{LxApp, lx};
use rong::{IntoJSObj, JSContext, JSFunc, JSResult};
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

fn get_capsule_rect(ctx: JSContext) -> JSResult<JSCapsuleRect> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    let json_str = lxapp
        .runtime
        .get_capsule_rect()
        .unwrap_or_else(|_| "{}".to_string());

    let rect: JSCapsuleRect = serde_json::from_str(&json_str).unwrap_or_default();
    Ok(rect)
}

/// Initialize capsule button functions
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    // Register getCapsuleRect function
    let func = JSFunc::new(ctx, get_capsule_rect)?;
    lx::register_js_api(ctx, "getCapsuleRect", func)?;
    Ok(())
}
