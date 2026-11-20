use ::lxapp::LxApp;
use rong::{IntoJSObject, JSContext, JSResult};

#[derive(Debug, Clone, IntoJSObject)]
struct AppInfo {
    #[js_name = "appId"]
    app_id: String,
    name: String,
    version: String,
    #[js_name = "channel"]
    #[ts_type = "LxAppReleaseType"]
    release_type: String,
}

/// Identify the running lxapp: its id, display name, version, and channel.
fn get_app_info(ctx: JSContext) -> JSResult<AppInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let info = lxapp.get_lxapp_info();
    Ok(AppInfo {
        app_id: lxapp.appid.clone(),
        name: info.app_name,
        version: info.version,
        release_type: info.release_type,
    })
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)?;
    register_app_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const app: "LxAppApi" = crate::namespace::app(ctx)?;
    }
}

rong::js_api! {
    fn register_app_api(ctx) {
        namespace LxAppApi = crate::namespace::app(ctx)?;
        const info: "Readonly<AppInfo>" = get_app_info(ctx.clone())?;
    }
}
