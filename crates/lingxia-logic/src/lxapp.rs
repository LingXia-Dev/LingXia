use ::lxapp::{AppSessionClass, LxApp};
use rong::{IntoJSObject, JSContext, JSResult};

#[derive(Debug, Clone, IntoJSObject)]
struct AppInfo {
    /// This lxapp's id.
    #[js_name = "appId"]
    app_id: String,
    /// Its display name.
    name: String,
    /// Its version, from `lxapp.json`.
    version: String,
    /// The channel it was installed from.
    #[js_name = "channel"]
    #[ts_type = "LxAppReleaseType"]
    release_type: String,
    /// What this lxapp may call. `control`: the product's own lxapp, admitted to
    /// Control-only members. `control-surface`: a host-bundled settings lxapp the
    /// Control app opens, admitted only to its own members. `guest`: any other.
    #[ts_type = "'control' | 'control-surface' | 'guest'"]
    role: &'static str,
}

/// Identify the running lxapp: its id, display name, version, channel, and role.
fn get_app_info(ctx: JSContext) -> JSResult<AppInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let info = lxapp.get_lxapp_info();
    Ok(AppInfo {
        app_id: lxapp.appid.clone(),
        name: info.app_name,
        version: info.version,
        release_type: info.release_type,
        role: match lxapp.app_session_class() {
            AppSessionClass::ControlApp => "control",
            AppSessionClass::ControlSurface => "control-surface",
            AppSessionClass::StandardApp => "guest",
        },
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
