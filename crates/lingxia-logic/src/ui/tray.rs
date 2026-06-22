use crate::i18n::js_error_from_platform_error;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lxapp::LxApp;
use rong::{JSContext, JSFunc, JSObject, JSResult};

fn tray_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("tray") {
        Ok(obj) => Ok(obj),
        Err(_) => {
            let obj = JSObject::new(ctx);
            lx.set("tray", obj.clone())?;
            Ok(obj)
        }
    }
}

/// lx.tray.setBadge(value) — the menu-bar / system-tray badge. Null/empty clears it.
fn set_badge(ctx: JSContext, text: Option<String>) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .set_tray_badge(text.as_deref().unwrap_or(""))
        .map_err(|e| js_error_from_platform_error(&e))
}

/// lx.tray.setIcon(icon) — replace the tray icon (a resource path).
fn set_icon(ctx: JSContext, icon: String) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .set_tray_icon(&icon)
        .map_err(|e| js_error_from_platform_error(&e))
}

/// lx.tray.setTitle(text) — text shown beside the icon (macOS). Empty clears it.
fn set_title(ctx: JSContext, text: Option<String>) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .set_tray_title(text.as_deref().unwrap_or(""))
        .map_err(|e| js_error_from_platform_error(&e))
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let tray = tray_namespace(ctx)?;
    tray.set("setBadge", JSFunc::new(ctx, set_badge)?)?;
    tray.set("setIcon", JSFunc::new(ctx, set_icon)?)?;
    tray.set("setTitle", JSFunc::new(ctx, set_title)?)?;
    Ok(())
}
