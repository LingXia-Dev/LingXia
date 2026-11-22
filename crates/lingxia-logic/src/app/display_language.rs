use crate::authorization::{self, LogicRoute};
use crate::i18n::{js_error_from_lxapp_error, js_invalid_parameter_error};
use lxapp::{
    DISPLAY_LANGUAGE_CHANGE_EVENT, DISPLAY_LANGUAGE_STATE_CHANGE_EVENT, HandlerToken,
    register_app_handler, unregister_app_handler_token,
};
use rong::{FromJSObject, JSContext, JSFunc, JSObject, JSResult, JSValue, function::Optional};
use std::cell::{Cell, RefCell};
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

/// `lx.app.language` — the language this lxapp renders in: the product's, narrowed to the
/// `languages` its lxapp.json lists. Every context gets it.
pub(super) fn init_app(ctx: &JSContext, app: &JSObject) -> JSResult<()> {
    let namespace = JSObject::new(ctx);
    namespace.set("get", JSFunc::new(ctx, get)?.name("get")?)?;
    namespace.set("watch", JSFunc::new(ctx, watch)?.name("watch")?)?;
    app.set("language", namespace)?;
    Ok(())
}

/// `lx.host.displayLanguage` — the product's language preference. Every context gets it; each call
/// authorizes, so only the Control app gets past it.
pub(super) fn init_host(ctx: &JSContext, host: &JSObject) -> JSResult<()> {
    let namespace = JSObject::new(ctx);
    namespace.set("get", JSFunc::new(ctx, get_preference)?.name("get")?)?;
    namespace.set("set", JSFunc::new(ctx, set_preference)?.name("set")?)?;
    namespace.set("watch", JSFunc::new(ctx, watch_preference)?.name("watch")?)?;
    host.set("displayLanguage", namespace)?;
    Ok(())
}

fn get(ctx: JSContext) -> JSResult<String> {
    Ok(lxapp::LxApp::from_ctx(&ctx)?.content_language(&lxapp::display_language()))
}

/// Follow the language this lxapp renders in, starting with the current value; the callback runs
/// only when the narrowed language changes. Returns an unsubscribe.
///
/// Logic needs this for the strings it hands to native chrome — modal and action-sheet text —
/// which nothing re-renders on its behalf.
fn watch(ctx: JSContext, callback: JSFunc, options: Optional<JSValue>) -> JSResult<JSFunc> {
    let scope = ctx.clone();
    lxapp::lx::subscribe(&scope, options.0, move || {
        let app = lxapp::LxApp::from_ctx(&ctx)?;
        let last_revision = Rc::new(Cell::new(0));
        let last: Rc<RefCell<Option<String>>> = Rc::default();
        let deliver = {
            let last = last.clone();
            let callback = callback.clone();
            move |effective: &str| {
                let language = app.content_language(effective);
                if last.borrow().as_deref() == Some(language.as_str()) {
                    return;
                }
                *last.borrow_mut() = Some(language.clone());
                let _ = callback.call::<_, ()>(None, (language,));
            }
        };
        let event_revision = last_revision.clone();
        let event_deliver = deliver.clone();
        let handler = JSFunc::new(&ctx, move |event: EffectiveEvent| {
            if event.revision <= event_revision.get() {
                return;
            }
            event_revision.set(event.revision);
            event_deliver(&event.effective);
        })?;
        // Register first, then snapshot. Events queued across this boundary carry
        // revisions and are discarded when the snapshot already includes them.
        let token = register_app_handler(&ctx, DISPLAY_LANGUAGE_CHANGE_EVENT, handler)?;
        let snapshot = lxapp::display_language_state_update();
        if last.borrow().is_none() || snapshot.revision > last_revision.get() {
            last_revision.set(snapshot.revision.max(last_revision.get()));
            deliver(snapshot.state.effective.as_str());
        }
        unsubscribe(&ctx, DISPLAY_LANGUAGE_CHANGE_EVENT, token)
    })
}

fn get_preference(ctx: JSContext) -> JSResult<String> {
    authorization::require(&ctx, LogicRoute::AppDisplayLanguagePreferenceGet)?;
    Ok(lxapp::display_language_state().preference.to_string())
}

/// Persist `'auto'`, or any canonical BCP-47 tag.
async fn set_preference(ctx: JSContext, preference: JSValue) -> JSResult<()> {
    let (_, preference) = authorization::require_before_decode(
        &ctx,
        LogicRoute::AppDisplayLanguagePreferenceSet,
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
/// stays quiet where `lx.app.language.watch` fires.
fn watch_preference(
    ctx: JSContext,
    callback: JSValue,
    options: Optional<JSValue>,
) -> JSResult<JSFunc> {
    let scope = ctx.clone();
    lxapp::lx::subscribe(&scope, options.0, move || {
        let (_, callback) = authorization::require_before_decode(
            &ctx,
            LogicRoute::AppDisplayLanguagePreferenceWatch,
            || callback.to_rust::<JSFunc>(),
        )?;
        let last_revision = Rc::new(Cell::new(0));
        let last_preference: Rc<RefCell<Option<String>>> = Rc::default();
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
