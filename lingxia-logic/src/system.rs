use lingxia_platform::AppRuntime;
use lxapp::{LxApp, lx};
use rong::{IntoJSObj, JSContext, JSFunc, JSResult};

/// AppBase information
#[derive(Debug, Clone, IntoJSObj)]
pub struct AppBaseInfo {
    language: String,
}

pub(crate) fn get_system_locale(ctx: JSContext) -> JSResult<AppBaseInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let locale = lxapp.runtime.get_system_locale();
    Ok(AppBaseInfo {
        language: locale.to_string(),
    })
}

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let get_app_base_info = JSFunc::new(ctx, get_system_locale)?;
    lx::register_js_api(ctx, "getAppBaseInfo", get_app_base_info)?;

    Ok(())
}
