use crate::i18n::{js_error_from_platform_error, js_service_unavailable_error};
use lingxia_app_context::app_config;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lxapp::LxApp;
use rong::{IntoJSObj, JSContext, JSFunc, JSObject, JSResult};

/// Host app base information.
#[derive(Debug, Clone, IntoJSObj)]
struct AppBaseInfo {
    language: String,
    #[rename = "productName"]
    product_name: String,
    #[rename = "version"]
    version: String,
    #[rename = "SDKVersion"]
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
    app.set("getBaseInfo", JSFunc::new(ctx, get_app_base_info)?)?;
    app.set("exit", JSFunc::new(ctx, exit_app)?)?;

    Ok(())
}
