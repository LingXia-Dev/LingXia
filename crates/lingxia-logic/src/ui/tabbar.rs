use super::page_chrome_patch::parse_patch;
use crate::i18n::{js_internal_error, js_invalid_parameter_error, js_service_unavailable_error};
use lxapp::LxApp;
use lxapp::tabbar::TabBarPatch;
use rong::{JSContext, JSObject, JSResult};

/// The tab bar declared by this lxapp: visibility, items, badges, and style.
fn namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("tabBar") {
        Ok(namespace) => Ok(namespace),
        Err(_) => {
            let namespace = JSObject::new(ctx);
            lx.set("tabBar", namespace.clone())?;
            Ok(namespace)
        }
    }
}

/// Patch this lxapp's tab bar; unset fields stay as they are.
async fn update(ctx: JSContext, patch: JSObject) -> JSResult<()> {
    let patch = parse_patch::<TabBarPatch>(&patch, "tabBar")?;
    let app = LxApp::from_ctx(&ctx)?;
    app.commit_tabbar(patch).await.map_err(|error| match error {
        lxapp::LxAppError::InvalidParameter(_) => js_invalid_parameter_error(error.to_string()),
        lxapp::LxAppError::ResourceNotFound(_) => js_service_unavailable_error(error.to_string()),
        _ => js_internal_error(error.to_string()),
    })
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_property(ctx)?;
    register_api(ctx)
}

rong::js_api! {
    fn register_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const tabBar: "TabBarApi" = namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_api(ctx) {
        namespace TabBarApi = namespace(ctx)?;
        fn update(ts_params = "patch: TabBarPatch") = update;
    }
}
