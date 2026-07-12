use crate::i18n::{js_error_from_platform_error, js_service_unavailable_error};
use lingxia_platform::traits::pull_to_refresh::PullToRefresh;
use lxapp::LxApp;
use rong::{JSContext, JSResult};

/// lx.startPullDownRefresh()
///
/// Programmatically start the pull-to-refresh animation.
/// This will show the refresh indicator and trigger the onPullDownRefresh lifecycle method.
fn start_pull_down_refresh(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let path = lxapp
        .peek_current_page()
        .ok_or_else(|| js_service_unavailable_error("No current page found"))?;

    lxapp
        .runtime
        .start_pull_down_refresh(&lxapp.appid, &path)
        .map_err(|e| {
            lxapp::error!("start_pull_down_refresh failed: {}", e);
            js_error_from_platform_error(&e)
        })?;

    Ok(())
}

/// lx.stopPullDownRefresh()
///
/// Stop the pull-to-refresh animation.
/// This should be called after the refresh operation is complete.
fn stop_pull_down_refresh(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let path = lxapp
        .peek_current_page()
        .ok_or_else(|| js_service_unavailable_error("No current page found"))?;

    lxapp
        .runtime
        .stop_pull_down_refresh(&lxapp.appid, &path)
        .map_err(|e| {
            lxapp::error!("stop_pull_down_refresh failed: {}", e);
            js_error_from_platform_error(&e)
        })?;

    Ok(())
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn startPullDownRefresh = start_pull_down_refresh;
        fn stopPullDownRefresh = stop_pull_down_refresh;
    }
}
