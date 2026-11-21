use super::page_chrome_patch::parse_patch;
use crate::i18n::{js_internal_error, js_invalid_parameter_error, js_service_unavailable_error};
use lxapp::LxApp;
use lxapp::tabbar::TabBarPatch;
use rong::{JSContext, JSObject, JSResult};

/// The tab bar declared by this lxapp: visibility, items, badges, and style.
fn namespace(ctx: &JSContext) -> JSResult<JSObject> {
    crate::namespace::child(ctx, &crate::namespace::app(ctx)?, "tabBar")
}

/// Patch this lxapp's tab bar: items, badges, red dots and visibility. Unset
/// fields stay; colors stay in `lxapp.json` `tabBar.style`.
///
/// @platform macOS, Windows: the bar is the sidebar, which follows the host theme; `backgroundColor` does not paint it, and a dark host ignores every style key.
/// @throws {E_INVALID_ARG} A `style` field, or an unknown one.
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
        namespace LxAppApi = crate::namespace::app(ctx)?;
        const tabBar: "TabBarApi" = namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_api(ctx) {
        namespace TabBarApi = namespace(ctx)?;
        fn update(ts_params = "patch: TabBarPatch") = update;
    }
}
