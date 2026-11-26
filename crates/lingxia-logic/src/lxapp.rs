use ::lxapp::{AppSessionClass, LxApp};
use rong::{IntoJSObject, JSContext, JSResult};

/// This lxapp's identity and the role it runs in.
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
    /// Control-only members. `control_surface`: a host-bundled settings lxapp the
    /// Control app opens, admitted only to its own members. `guest`: any other.
    #[ts_type = "'control' | 'control_surface' | 'guest'"]
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
            AppSessionClass::ControlSurface => "control_surface",
            AppSessionClass::StandardApp => "guest",
        },
    })
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)?;
    register_app_api(ctx)?;
    // Framework-internal, for `@lingxia/logic`'s `packageText`; not part of `lx`.
    ctx.global()
        .set("__lingxiaAppText", rong::JSFunc::new(ctx, app_text)?)?;
    Ok(())
}

/// The lxapp's override of a package word in `language`'s catalogue, or null.
fn app_text(
    ctx: JSContext,
    key: String,
    language: String,
    args: String,
) -> JSResult<Option<String>> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let args = serde_json::from_str(&args).map_err(|error| {
        crate::i18n::js_invalid_parameter_error(format!("packageText args: {error}"))
    })?;
    Ok(lxapp.message_in(&key, &language, &args))
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const app: "LxAppApi" = crate::namespace::app(ctx)?;
    }
}

/// The lxapp's message `key` from its `i18n/` catalogues in its language, with ICU `args`;
/// tags keep their text. Read it when the text is shown: a later language switch does not change it.
fn message(
    ctx: JSContext,
    key: String,
    args: rong::function::Optional<rong::JSObject>,
) -> JSResult<String> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let args = match args.0 {
        None => serde_json::Value::Null,
        Some(object) => {
            let json = object.to_json_string().map_err(|error| {
                crate::i18n::js_invalid_parameter_error(format!(
                    "lx.app.t args must be a plain object: {error}"
                ))
            })?;
            serde_json::from_str(&json).map_err(|error| {
                crate::i18n::js_invalid_parameter_error(format!("lx.app.t args: {error}"))
            })?
        }
    };
    lxapp
        .message(&key, &args)
        .map_err(|error| crate::i18n::host_error_of(error).into())
}

rong::js_api! {
    fn register_app_api(ctx) {
        namespace LxAppApi = crate::namespace::app(ctx)?;
        const info: "Readonly<AppInfo>" = get_app_info(ctx.clone())?;
        fn t(
            ts_params = "...message: LxAppMessageCall"
        ) = message;
    }
}
