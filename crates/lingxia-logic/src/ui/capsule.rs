use crate::i18n::{js_error_from_platform_error, js_internal_error};
use lingxia_platform::traits::app_runtime::AppRuntime;
use lxapp::LxApp;
use rong::{IntoJSObject, JSContext, JSResult};
use serde::Deserialize;

/// A visible capsule button's complete bounding client rect.
#[derive(Debug, Clone, Deserialize, IntoJSObject)]
#[ts_skip]
struct JSCapsuleRect {
    width: f64,
    height: f64,
    top: f64,
    right: f64,
    bottom: f64,
    left: f64,
}

/// Get the visible capsule button's bounding rect. Returns `null` for the home
/// lxapp, an inactive lxapp, or a host that does not expose a capsule; rejection
/// indicates an actual platform failure rather than hidden chrome.
async fn get_capsule_rect(ctx: JSContext) -> JSResult<Option<JSCapsuleRect>> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    let json_str = lxapp
        .runtime
        .get_capsule_rect(&lxapp.appid)
        .await
        .map_err(|e| js_error_from_platform_error(&e))?;

    parse_capsule_rect(&json_str)
}

fn parse_capsule_rect(json: &str) -> JSResult<Option<JSCapsuleRect>> {
    serde_json::from_str(json)
        .map_err(|e| js_internal_error(format!("getCapsuleRect invalid payload: {}", e)))
}

/// Initialize capsule button functions
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn getCapsuleRect(ts_return = "Promise<CapsuleRect | null>") = get_capsule_rect;
    }
}

#[cfg(test)]
mod tests {
    use super::parse_capsule_rect;

    #[test]
    fn parses_visible_capsule_rect() {
        let rect = parse_capsule_rect(
            r#"{"width":84.5,"height":32,"top":50,"right":375,"bottom":82,"left":290.5}"#,
        )
        .expect("valid capsule rect")
        .expect("visible capsule");

        assert_eq!(rect.width, 84.5);
        assert_eq!(rect.height, 32.0);
        assert_eq!(rect.top, 50.0);
        assert_eq!(rect.right, 375.0);
        assert_eq!(rect.bottom, 82.0);
        assert_eq!(rect.left, 290.5);
    }

    #[test]
    fn parses_hidden_capsule_as_null() {
        assert!(parse_capsule_rect("null").expect("valid null").is_none());
    }

    #[test]
    fn rejects_incomplete_visible_rect() {
        assert!(parse_capsule_rect(r#"{"width":84.5}"#).is_err());
    }
}
