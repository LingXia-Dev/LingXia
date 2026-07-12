use lxapp::lifecycle::key_events;
use lxapp::{LxApp, register_app_handler, unregister_app_handler};
use rong::function::Optional;
use rong::{JSContext, JSFunc, JSResult};

const KEY_DOWN_EVENT: &str = "KeyDown";
const KEY_UP_EVENT: &str = "KeyUp";

fn on_key_down(ctx: JSContext, callback: JSFunc) -> JSResult<()> {
    register_app_handler(&ctx, KEY_DOWN_EVENT, callback)?;
    let lxapp = LxApp::from_ctx(&ctx)?;
    key_events::inc_key_down(&lxapp.appid, lxapp.session_id());
    Ok(())
}

fn off_key_down(ctx: JSContext, callback: Optional<JSFunc>) -> JSResult<()> {
    let remaining = unregister_app_handler(&ctx, KEY_DOWN_EVENT, callback.0);
    let lxapp = LxApp::from_ctx(&ctx)?;
    key_events::set_key_down(&lxapp.appid, lxapp.session_id(), remaining);
    Ok(())
}

fn on_key_up(ctx: JSContext, callback: JSFunc) -> JSResult<()> {
    register_app_handler(&ctx, KEY_UP_EVENT, callback)?;
    let lxapp = LxApp::from_ctx(&ctx)?;
    key_events::inc_key_up(&lxapp.appid, lxapp.session_id());
    Ok(())
}

fn off_key_up(ctx: JSContext, callback: Optional<JSFunc>) -> JSResult<()> {
    let remaining = unregister_app_handler(&ctx, KEY_UP_EVENT, callback.0);
    let lxapp = LxApp::from_ctx(&ctx)?;
    key_events::set_key_up(&lxapp.appid, lxapp.session_id(), remaining);
    Ok(())
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn onKeyDown(ts_params = "callback: KeyEventCallback") = on_key_down;
        fn offKeyDown(ts_params = "callback?: KeyEventCallback") = off_key_down;
        fn onKeyUp(ts_params = "callback: KeyEventCallback") = on_key_up;
        fn offKeyUp(ts_params = "callback?: KeyEventCallback") = off_key_up;
    }
}
