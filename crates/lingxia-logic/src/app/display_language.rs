use crate::authorization::{self, LogicRoute};
use crate::i18n::{js_error_from_lxapp_error, js_invalid_parameter_error};
use lxapp::{
    DISPLAY_LANGUAGE_CHANGE_EVENT, DISPLAY_LANGUAGE_STATE_CHANGE_EVENT, HandlerToken,
    register_app_handler, unregister_app_handler_token,
};
use rong::{FromJSObject, JSContext, JSFunc, JSObject, JSResult, JSValue};
use std::cell::Cell;
use std::rc::Rc;

#[derive(FromJSObject)]
#[ts_skip]
struct EffectiveEvent {
    revision: u64,
    effective: String,
}

#[derive(FromJSObject)]
#[ts_skip]
struct IncomingState {
    preference: String,
}

#[derive(FromJSObject)]
#[ts_skip]
struct StateEvent {
    revision: u64,
    state: IncomingState,
}

/// `lx.app.displayLanguage` — the language this lxapp renders in. Every context
/// gets it, including the control surfaces that have no other `lx.app` members:
/// following the product is not a privilege.
pub(super) fn init_follower(ctx: &JSContext, app: &JSObject) -> JSResult<()> {
    let namespace = JSObject::new(ctx);
    namespace.set("get", JSFunc::new(ctx, get)?.name("get")?)?;
    namespace.set("watch", JSFunc::new(ctx, watch)?.name("watch")?)?;
    app.set("displayLanguage", namespace)?;
    Ok(())
}

/// `lx.app.control.displayLanguage` — the preference behind that language.
pub(super) fn init_control(ctx: &JSContext, control: &JSObject) -> JSResult<()> {
    let namespace = JSObject::new(ctx);
    namespace.set(
        "getPreference",
        JSFunc::new(ctx, get_preference)?.name("getPreference")?,
    )?;
    namespace.set(
        "setPreference",
        JSFunc::new(ctx, set_preference)?.name("setPreference")?,
    )?;
    namespace.set(
        "watchPreference",
        JSFunc::new(ctx, watch_preference)?.name("watchPreference")?,
    )?;
    control.set("displayLanguage", namespace)?;
    Ok(())
}

fn get(_ctx: JSContext) -> JSResult<String> {
    Ok(lxapp::display_language())
}

/// Follow the language this lxapp renders in.
///
/// Logic needs this because the strings it hands to native chrome — navigation
/// bar titles, tab bar labels, modal and action-sheet text — are the app's own,
/// and nothing re-renders them on its behalf. The callback is invoked once with
/// the current value, then on every change.
fn watch(ctx: JSContext, callback: JSFunc) -> JSResult<JSFunc> {
    let last_revision = Rc::new(Cell::new(0));
    let delivered = Rc::new(Cell::new(false));
    let event_revision = last_revision.clone();
    let event_delivered = delivered.clone();
    let event_callback = callback.clone();
    let handler = JSFunc::new(&ctx, move |event: EffectiveEvent| {
        if event.revision <= event_revision.get() {
            return;
        }
        event_revision.set(event.revision);
        event_delivered.set(true);
        let _ = event_callback.call::<_, ()>(None, (event.effective,));
    })?;
    // Register first, then snapshot. Events queued across this boundary carry
    // revisions and are discarded when the snapshot already includes them.
    let token = register_app_handler(&ctx, DISPLAY_LANGUAGE_CHANGE_EVENT, handler)?;
    let snapshot = lxapp::display_language_state_update();
    if !delivered.get() || snapshot.revision > last_revision.get() {
        last_revision.set(snapshot.revision);
        delivered.set(true);
        let _ = callback.call::<_, ()>(None, (snapshot.state.effective.to_string(),));
    }
    unsubscribe(&ctx, DISPLAY_LANGUAGE_CHANGE_EVENT, token)
}

fn get_preference(ctx: JSContext) -> JSResult<String> {
    authorization::require(&ctx, LogicRoute::AppGetDisplayLanguagePreference)?;
    Ok(lxapp::display_language_state().preference.to_string())
}

/// Persist `'auto'`, or any canonical BCP-47 tag.
async fn set_preference(ctx: JSContext, preference: JSValue) -> JSResult<()> {
    let (_, preference) = authorization::require_before_decode(
        &ctx,
        LogicRoute::AppSetDisplayLanguagePreference,
        || preference.to_rust::<String>(),
    )?;
    let preference = preference
        .parse::<lxapp::DisplayLanguagePreference>()
        .map_err(js_invalid_parameter_error)?;
    lxapp::set_display_language_preference(preference)
        .map_err(|error| js_error_from_lxapp_error(&error))
}

/// Follow what the user chose, not what it resolves to. A system locale change
/// under `'auto'` moves the language without moving the preference, so this
/// stays quiet where `watch` fires.
fn watch_preference(ctx: JSContext, callback: JSValue) -> JSResult<JSFunc> {
    let (_, callback) = authorization::require_before_decode(
        &ctx,
        LogicRoute::AppWatchDisplayLanguagePreference,
        || callback.to_rust::<JSFunc>(),
    )?;
    let last_revision = Rc::new(Cell::new(0));
    let last_preference: Rc<std::cell::RefCell<Option<String>>> = Rc::default();
    let event_revision = last_revision.clone();
    let event_preference = last_preference.clone();
    let event_callback = callback.clone();
    let handler = JSFunc::new(&ctx, move |event: StateEvent| {
        if event.revision <= event_revision.get() {
            return;
        }
        event_revision.set(event.revision);
        if event_preference.borrow().as_deref() == Some(event.state.preference.as_str()) {
            return;
        }
        *event_preference.borrow_mut() = Some(event.state.preference.clone());
        let _ = event_callback.call::<_, ()>(None, (event.state.preference,));
    })?;
    let token = register_app_handler(&ctx, DISPLAY_LANGUAGE_STATE_CHANGE_EVENT, handler)?;
    let snapshot = lxapp::display_language_state_update();
    if last_preference.borrow().is_none() || snapshot.revision > last_revision.get() {
        let preference = snapshot.state.preference.to_string();
        last_revision.set(snapshot.revision);
        let changed = last_preference.borrow().as_deref() != Some(preference.as_str());
        *last_preference.borrow_mut() = Some(preference.clone());
        if changed {
            let _ = callback.call::<_, ()>(None, (preference,));
        }
    }
    unsubscribe(&ctx, DISPLAY_LANGUAGE_STATE_CHANGE_EVENT, token)
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
