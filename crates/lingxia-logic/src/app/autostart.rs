use crate::i18n::js_error_from_platform_error;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lxapp::LxApp;
use rong::{JSContext, JSFunc, JSObject, JSResult};

/// `lx.app.autostart` — launch-at-startup control. The member is absent unless
/// the host declared the `autostart` capability (and this module is compiled
/// only for macOS/Windows), so JS gates on presence: `lx.app.autostart?.…`.
pub(super) fn init(ctx: &JSContext, app: &JSObject) -> JSResult<()> {
    if !lingxia_app_context::autostart_enabled() || !lingxia_platform::autostart_supported() {
        return Ok(());
    }
    let autostart = JSObject::new(ctx);
    autostart.set(
        "isEnabled",
        JSFunc::new(ctx, autostart_is_enabled)?.name("isEnabled")?,
    )?;
    autostart.set(
        "setEnabled",
        JSFunc::new(ctx, autostart_set_enabled)?.name("setEnabled")?,
    )?;
    app.set("autostart", autostart)?;
    Ok(())
}

/// Reads the live OS registration (login items / Run key), never a cached
/// preference — the user can flip it outside the app at any time.
async fn autostart_is_enabled(ctx: JSContext) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    super::ensure_home_lxapp(&lxapp, "lx.app.autostart")?;
    lxapp
        .runtime
        .autostart_is_enabled()
        .map_err(|e| js_error_from_platform_error(&e))
}

async fn autostart_set_enabled(ctx: JSContext, enabled: bool) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    super::ensure_home_lxapp(&lxapp, "lx.app.autostart")?;
    lxapp
        .runtime
        .autostart_set_enabled(enabled)
        .map_err(|e| js_error_from_platform_error(&e))
}
