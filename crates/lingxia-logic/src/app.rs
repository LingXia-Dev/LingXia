use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_platform_error,
    js_service_unavailable_error,
};
use lingxia_app_context::{app_config, env_version, home_app_id};
use lingxia_platform::traits::app_runtime::AppRuntime;
use lxapp::LxApp;
use rong::{IntoJSObject, JSContext, JSObject, JSResult};

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod autostart;
mod screenshot;
mod update;

/// Host app base information.
#[derive(Debug, Clone, IntoJSObject)]
struct AppBaseInfo {
    language: String,
    #[js_name = "productName"]
    product_name: String,
    #[js_name = "version"]
    version: String,
    #[js_name = "SDKVersion"]
    sdk_version: String,
}

fn get_app_base_info(ctx: JSContext) -> JSResult<AppBaseInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let locale = lxapp.runtime.get_system_locale();
    let app_cfg =
        app_config().ok_or_else(|| js_service_unavailable_error("app config not available"))?;
    Ok(AppBaseInfo {
        language: locale.to_string(),
        product_name: app_cfg.product_name.clone(),
        version: app_cfg.product_version.clone(),
        sdk_version: lxapp::SDK_RUNTIME_VERSION.to_string(),
    })
}

fn exit_app(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .exit()
        .map_err(|e| js_error_from_platform_error(&e))
}

/// lx.app.setBadge(value) — the dock (macOS) / taskbar (Windows) badge. Null/empty clears it.
fn set_app_badge(ctx: JSContext, text: Option<String>) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .set_app_badge(text.as_deref().unwrap_or(""))
        .map_err(|e| js_error_from_platform_error(&e))
}

/// Guard for host-app-level APIs (`checkUpdate`, `screenshot`, `autostart`):
/// only the home lxapp may call them; others get a permission error.
pub(super) fn ensure_home_lxapp(lxapp: &LxApp, api_name: &str) -> JSResult<()> {
    let home_appid = home_app_id()
        .ok_or_else(|| js_service_unavailable_error("home lxapp is not configured"))?;
    if lxapp.appid == home_appid {
        return Ok(());
    }

    Err(js_error_from_business_code_with_detail(
        3000,
        format!("{api_name} is only available in the home lxapp"),
    ))
}

fn app_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("app") {
        Ok(obj) => Ok(obj),
        Err(_) => {
            let obj = JSObject::new(ctx);
            lx.set("app", obj.clone())?;
            Ok(obj)
        }
    }
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let app = app_namespace(ctx)?;
    register_app_property(ctx)?;
    register_app_api(ctx)?;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    autostart::init(ctx, &app)?;
    screenshot::init(ctx)?;
    update::init(ctx)?;

    Ok(())
}

rong::js_api! {
    fn register_app_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const app: "HostAppApi" = app_namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_app_api(ctx) {
        namespace HostAppApi = app_namespace(ctx)?;
        const envVersion: "HostAppEnvVersion" = env_version().as_str();
        fn getBaseInfo = get_app_base_info;
        fn exit = exit_app;
        fn setBadge(ts_params = "value: string | number | null") = set_app_badge;
    }
}
