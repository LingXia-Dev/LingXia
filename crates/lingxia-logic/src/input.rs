use lxapp::lifecycle::key_events;
use lxapp::{LxApp, register_app_handler, unregister_app_handler};
use rong::{JSContext, JSFunc, JSResult};

const KEY_DOWN_EVENT: &str = "KeyDown";
const KEY_UP_EVENT: &str = "KeyUp";

/// Subscribes to key-down events and returns the unsubscribe fn.
fn on_key_down(ctx: JSContext, callback: JSFunc) -> JSResult<JSFunc> {
    register_app_handler(&ctx, KEY_DOWN_EVENT, callback.clone())?;
    let lxapp = LxApp::from_ctx(&ctx)?;
    key_events::inc_key_down(&lxapp.appid, lxapp.session_id());

    let off_ctx = ctx.clone();
    JSFunc::new(&ctx, move || {
        let remaining = unregister_app_handler(&off_ctx, KEY_DOWN_EVENT, Some(callback.clone()));
        if let Ok(lxapp) = LxApp::from_ctx(&off_ctx) {
            key_events::set_key_down(&lxapp.appid, lxapp.session_id(), remaining);
        }
    })
}

/// Subscribes to key-up events and returns the unsubscribe fn.
fn on_key_up(ctx: JSContext, callback: JSFunc) -> JSResult<JSFunc> {
    register_app_handler(&ctx, KEY_UP_EVENT, callback.clone())?;
    let lxapp = LxApp::from_ctx(&ctx)?;
    key_events::inc_key_up(&lxapp.appid, lxapp.session_id());

    let off_ctx = ctx.clone();
    JSFunc::new(&ctx, move || {
        let remaining = unregister_app_handler(&off_ctx, KEY_UP_EVENT, Some(callback.clone()));
        if let Ok(lxapp) = LxApp::from_ctx(&off_ctx) {
            key_events::set_key_up(&lxapp.appid, lxapp.session_id(), remaining);
        }
    })
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn onKeyDown(
            ts_params = "callback: KeyEventCallback",
            ts_return = "() => void"
        ) = on_key_down;
        fn onKeyUp(
            ts_params = "callback: KeyEventCallback",
            ts_return = "() => void"
        ) = on_key_up;
    }
}
