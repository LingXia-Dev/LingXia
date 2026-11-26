use crate::authorization::{self, LogicRoute};
use crate::i18n::{js_error, js_invalid_parameter_error};
use lxapp::{
    ContrastPreference, HOST_CONTRAST_CHANGE_EVENT, HandlerToken, register_app_handler,
    unregister_app_handler_token,
};
use rong::{FromJSObject, JSContext, JSFunc, JSObject, JSResult, JSValue, function::Optional};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(FromJSObject)]
#[ts_skip]
struct PreferenceEvent {
    revision: u64,
    preference: String,
}

/// `lx.host.contrastPreference` — the product's contrast preference. Every context gets it; each
/// call authorizes, so only the Control app and control surfaces get past it.
pub(super) fn init_host(ctx: &JSContext, host: &JSObject) -> JSResult<()> {
    let namespace = JSObject::new(ctx);
    namespace.set("get", JSFunc::new(ctx, get)?.name("get")?)?;
    namespace.set("set", JSFunc::new(ctx, set)?.name("set")?)?;
    namespace.set("watch", JSFunc::new(ctx, watch)?.name("watch")?)?;
    host.set("contrastPreference", namespace)?;
    Ok(())
}

fn get(ctx: JSContext) -> JSResult<String> {
    authorization::require(&ctx, LogicRoute::AppContrastPreferenceGet)?;
    Ok(lxapp::host_contrast_update().preference.as_str().to_string())
}

/// `'more'` puts every View page in high contrast; `'auto'` follows the system.
async fn set(ctx: JSContext, preference: JSValue) -> JSResult<()> {
    let (_, preference) =
        authorization::require_before_decode(&ctx, LogicRoute::AppContrastPreferenceSet, || {
            preference.to_rust::<String>()
        })?;
    let preference = preference
        .parse::<ContrastPreference>()
        .map_err(js_invalid_parameter_error)?;
    lxapp::set_host_contrast_preference(preference).map_err(js_error)?;
    Ok(())
}

/// Follow the choice, starting with the current one. Returns an unsubscribe.
fn watch(ctx: JSContext, callback: JSValue, options: Optional<JSValue>) -> JSResult<JSFunc> {
    let scope = ctx.clone();
    lxapp::lx::subscribe(&scope, options.0, move || {
        let (_, callback) = authorization::require_before_decode(
            &ctx,
            LogicRoute::AppContrastPreferenceWatch,
            || callback.to_rust::<JSFunc>(),
        )?;
        let last_revision = Rc::new(Cell::new(0));
        let last: Rc<RefCell<Option<String>>> = Rc::default();
        let event_revision = last_revision.clone();
        let event_last = last.clone();
        let event_callback = callback.clone();
        let handler = JSFunc::new(&ctx, move |event: PreferenceEvent| {
            if event.revision <= event_revision.get() {
                return;
            }
            event_revision.set(event.revision);
            if event_last.borrow().as_deref() == Some(event.preference.as_str()) {
                return;
            }
            *event_last.borrow_mut() = Some(event.preference.clone());
            let _ = event_callback.call::<_, ()>(None, (event.preference,));
        })?;
        // Register first, then snapshot: an event queued across this boundary carries a
        // revision the snapshot already covers.
        let token = register_app_handler(&ctx, HOST_CONTRAST_CHANGE_EVENT, handler)?;
        let update = lxapp::host_contrast_update();
        if update.revision > last_revision.get() {
            last_revision.set(update.revision);
            let preference = update.preference.as_str().to_string();
            *last.borrow_mut() = Some(preference.clone());
            let _ = callback.call::<_, ()>(None, (preference,));
        }
        unsubscribe(&ctx, token)
    })
}

fn unsubscribe(ctx: &JSContext, token: HandlerToken) -> JSResult<JSFunc> {
    let off_ctx = ctx.clone();
    let unsubscribed = Cell::new(false);
    JSFunc::new(ctx, move || {
        if unsubscribed.get() {
            return;
        }
        unregister_app_handler_token(&off_ctx, HOST_CONTRAST_CHANGE_EVENT, token);
        unsubscribed.set(true);
    })
}
