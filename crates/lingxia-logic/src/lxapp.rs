use ::lxapp::LxApp;
use rong::{IntoJSObject, JSContext, JSResult};

#[derive(Debug, Clone, IntoJSObject)]
struct LxAppInfo {
    #[js_name = "appId"]
    app_id: String,
    #[js_name = "appName"]
    app_name: String,
    version: String,
    #[js_name = "releaseType"]
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
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn getLxAppInfo(ts_return = "PublicLxAppInfo") = get_lxapp_info;
    }
}
