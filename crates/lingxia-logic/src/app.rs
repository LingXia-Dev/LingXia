use lxapp::LxApp;
use lxapp::lx;
use rong::{IntoJSObj, JSContext, JSFunc, JSResult};

#[derive(Debug, Clone, IntoJSObj)]
struct LxAppInfo {
    #[rename = "appId"]
    app_id: String,
    #[rename = "appName"]
    app_name: String,
    version: String,
    #[rename = "releaseType"]
    release_type: String,
}

fn get_lxapp_info(ctx: JSContext) -> JSResult<LxAppInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let info = lxapp.get_lxapp_info();
    Ok(LxAppInfo {
        app_id: lxapp.appid.clone(),
        app_name: info.app_name,
        version: info.version,
        release_type: info.release_type,
    })
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let get_lxapp_info_func = JSFunc::new(ctx, get_lxapp_info)?;
    lx::register_js_api(ctx, "getLxAppInfo", get_lxapp_info_func)?;
    Ok(())
}
