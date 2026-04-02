use lxapp::lifecycle::key_events;
use lxapp::{LxApp, lx, register_app_handler, unregister_app_handler};
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

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let on_key_down_func = JSFunc::new(ctx, on_key_down)?;
    lx::register_js_api(ctx, "onKeyDown", on_key_down_func)?;

    let off_key_down_func = JSFunc::new(ctx, off_key_down)?;
    lx::register_js_api(ctx, "offKeyDown", off_key_down_func)?;

    let on_key_up_func = JSFunc::new(ctx, on_key_up)?;
    lx::register_js_api(ctx, "onKeyUp", on_key_up_func)?;

    let off_key_up_func = JSFunc::new(ctx, off_key_up)?;
    lx::register_js_api(ctx, "offKeyUp", off_key_up_func)?;

    Ok(())
}
