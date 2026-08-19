use super::page_chrome_patch::parse_patch;
use crate::i18n::{js_internal_error, js_invalid_parameter_error, js_service_unavailable_error};
use lxapp::LxApp;
use lxapp::navbar::NavigationBarPatch;
use rong::{JSContext, JSObject, JSResult};

/// The navigation bar above the page: title, colors, and loading state.
fn namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("navigationBar") {
        Ok(namespace) => Ok(namespace),
        Err(_) => {
            let namespace = JSObject::new(ctx);
            lx.set("navigationBar", namespace.clone())?;
            Ok(namespace)
        }
    }
}

/// Patch the navigation bar of the active page; unset fields stay as they are.
async fn update(ctx: JSContext, patch: JSObject) -> JSResult<()> {
    let patch = parse_patch::<NavigationBarPatch>(&patch, "navigationBar")?;
    let app = LxApp::from_ctx(&ctx)?;
    let page = app
        .current_page()
        .map_err(|_| js_service_unavailable_error("navigationBar: no active page"))?;
    app.commit_navigation_bar(page, patch)
        .await
        .map_err(|error| match error {
            lxapp::LxAppError::InvalidParameter(_) => js_invalid_parameter_error(error.to_string()),
            lxapp::LxAppError::ResourceNotFound(_) => {
                js_service_unavailable_error(error.to_string())
            }
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
        const navigationBar: "NavigationBarApi" = namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_api(ctx) {
        namespace NavigationBarApi = namespace(ctx)?;
        fn update(ts_params = "patch: NavigationBarPatch") = update;
    }
}
