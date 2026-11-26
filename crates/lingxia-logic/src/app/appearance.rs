use crate::authorization::{self, LogicRoute};
use crate::i18n::{js_error, js_invalid_parameter_error};
use lxapp::page_chrome::AppearancePreference;
use lxapp::{
    APPEARANCE_CHANGE_EVENT, HOST_APPEARANCE_CHANGE_EVENT, HandlerToken, register_app_handler,
    unregister_app_handler_token,
};
use rong::{FromJSObject, JSContext, JSFunc, JSObject, JSResult, JSValue, function::Optional};
use std::cell::Cell;
use std::rc::Rc;

#[derive(FromJSObject)]
#[ts_skip]
struct ResolvedEvent {
    revision: u64,
    resolved: String,
}

#[derive(FromJSObject)]
#[ts_skip]
struct PreferenceEvent {
    revision: u64,
    preference: String,
}

/// `lx.app.appearance` — the light/dark scheme this lxapp renders in. An lxapp that pinned one in
/// its manifest reports that; the rest follow the product. Every context gets it.
pub(super) fn init_app(ctx: &JSContext, app: &JSObject) -> JSResult<()> {
    let namespace = JSObject::new(ctx);
    namespace.set("get", JSFunc::new(ctx, get)?.name("get")?)?;
    namespace.set("watch", JSFunc::new(ctx, watch)?.name("watch")?)?;
    app.set("appearance", namespace)?;
    Ok(())
}

/// `lx.host.appearancePreference` — the product's light/dark preference. Every context gets it; each call
/// authorizes, so only the Control app and control surfaces get past it.
pub(super) fn init_host(ctx: &JSContext, host: &JSObject) -> JSResult<()> {
    let namespace = JSObject::new(ctx);
    namespace.set("get", JSFunc::new(ctx, get_preference)?.name("get")?)?;
    namespace.set("set", JSFunc::new(ctx, set_preference)?.name("set")?)?;
    namespace.set("watch", JSFunc::new(ctx, watch_preference)?.name("watch")?)?;
    host.set("appearancePreference", namespace)?;
    Ok(())
}

fn get(ctx: JSContext) -> JSResult<String> {
    Ok(lxapp::LxApp::from_ctx(&ctx)?
        .appearance_state()
        .resolved
        .as_str()
        .to_string())
}

/// Follow the scheme this lxapp renders in, starting with the current value.
/// Returns an unsubscribe.
fn watch(ctx: JSContext, callback: JSFunc, options: Optional<JSValue>) -> JSResult<JSFunc> {
    let scope = ctx.clone();
    lxapp::lx::subscribe(&scope, options.0, move || {
        let last_revision = Rc::new(Cell::new(0));
        let delivered = Rc::new(Cell::new(false));
        let event_revision = last_revision.clone();
        let event_delivered = delivered.clone();
        let event_callback = callback.clone();
        let handler = JSFunc::new(&ctx, move |event: ResolvedEvent| {
            if event.revision <= event_revision.get() {
                return;
            }
            event_revision.set(event.revision);
            event_delivered.set(true);
            let _ = event_callback.call::<_, ()>(None, (event.resolved,));
        })?;
        // Register first, then snapshot: an event queued across this boundary
        // carries a revision and is discarded if the snapshot already covers it.
        let token = register_app_handler(&ctx, APPEARANCE_CHANGE_EVENT, handler)?;
        let state = lxapp::LxApp::from_ctx(&ctx)?.appearance_state();
        if !delivered.get() || state.revision > last_revision.get() {
            last_revision.set(state.revision);
            delivered.set(true);
            let _ = callback.call::<_, ()>(None, (state.resolved.as_str().to_string(),));
        }
        let off_ctx = ctx.clone();
        unsubscribe(&off_ctx, APPEARANCE_CHANGE_EVENT, token)
    })
}

fn get_preference(ctx: JSContext) -> JSResult<String> {
    authorization::require(&ctx, LogicRoute::AppAppearancePreferenceGet)?;
    Ok(lxapp::host_appearance_state()
        .preference
        .as_str()
        .to_string())
}

/// Pin the whole product to `'light'` or `'dark'`, or follow the system with
/// `'auto'`. An lxapp that pinned a scheme in its manifest keeps it.
async fn set_preference(ctx: JSContext, preference: JSValue) -> JSResult<()> {
    let (_, preference) =
        authorization::require_before_decode(&ctx, LogicRoute::AppAppearancePreferenceSet, || {
            preference.to_rust::<String>()
        })?;
    let preference = preference
        .parse::<AppearancePreference>()
        .map_err(js_invalid_parameter_error)?;
    let app = lxapp::LxApp::from_ctx(&ctx)?;
    lxapp::set_host_appearance_preference(preference).map_err(js_error)?;
    // Resolve once the caller's own appearance follows, so `get()` right after reads it.
    if app.appearance_state().preference == AppearancePreference::Auto {
        app.refresh_appearance().await.map_err(js_error)?;
    }
    Ok(())
}

/// Follow the choice, not what it resolves to: a system flip under `'auto'`
/// moves `lx.app.appearance.watch` and leaves this quiet.
fn watch_preference(
    ctx: JSContext,
    callback: JSValue,
    options: Optional<JSValue>,
) -> JSResult<JSFunc> {
    let scope = ctx.clone();
    lxapp::lx::subscribe(&scope, options.0, move || {
        let (_, callback) = authorization::require_before_decode(
            &ctx,
            LogicRoute::AppAppearancePreferenceWatch,
            || callback.to_rust::<JSFunc>(),
        )?;
        let last_revision = Rc::new(Cell::new(0));
        let last: Rc<std::cell::RefCell<Option<String>>> = Rc::default();
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
        let token = register_app_handler(&ctx, HOST_APPEARANCE_CHANGE_EVENT, handler)?;
        let preference = lxapp::host_appearance_state()
            .preference
            .as_str()
            .to_string();
        *last.borrow_mut() = Some(preference.clone());
        let _ = callback.call::<_, ()>(None, (preference,));
        let off_ctx = ctx.clone();
        unsubscribe(&off_ctx, HOST_APPEARANCE_CHANGE_EVENT, token)
    })
}

fn unsubscribe(ctx: &JSContext, event: &'static str, token: HandlerToken) -> JSResult<JSFunc> {
    let off_ctx = ctx.clone();
    let unsubscribed = Cell::new(false);
    JSFunc::new(ctx, move || {
        if unsubscribed.get() {
            return;
        }
        unregister_app_handler_token(&off_ctx, event, token);
        unsubscribed.set(true);
    })
}
