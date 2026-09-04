use crate::{error, info, warn};
use rong::{
    JSContext, JSContextService, JSFunc, JSObject, JSResult, RongJSError, error::HostError,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

pub const BROWSER_TAB_CLOSED_EVENT: &str = "__lingxiaBrowserTabClosed";
/// App-scoped event carrying the host's effective display language.
pub const DISPLAY_LANGUAGE_CHANGE_EVENT: &str = "DisplayLanguageChange";
/// App-scoped event carrying the complete host display-language state.
pub const DISPLAY_LANGUAGE_STATE_CHANGE_EVENT: &str = "DisplayLanguageStateChange";

/// Internal scope marker. The page scope carries the page INSTANCE id, so
/// one instance's teardown can never clear a same-path sibling's handlers.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Scope {
    App,
    PageInstance(String),
}

/// Envelope for a native -> JS event.
#[derive(Clone, Debug)]
pub(crate) struct AppBusEvent {
    pub scope: Scope,
    pub event_name: String,
    pub payload_json: Option<String>,
}

/// Handler registrations owned by one AppService context.
///
/// The map is `Rc` so an unsubscribe handle can reach it without cloning the
/// `JSContext`. A context captured inside a JS-held Rust closure is opaque to
/// the cycle collector — the pair keep each other alive. A weak handle also
/// makes a late `off()` after shutdown a no-op instead of touching a dead
/// context.
#[derive(Default)]
pub(crate) struct EventBusRegistry {
    handlers: Rc<RefCell<HashMap<Scope, Vec<HandlerEntry>>>>,
    next_token: Cell<u64>,
}

/// Unsubscribe handle that does not retain a `JSContext`.
#[derive(Clone)]
pub struct AppHandlerUnsub {
    handlers: Weak<RefCell<HashMap<Scope, Vec<HandlerEntry>>>>,
    event_name: String,
    token: HandlerToken,
}

impl AppHandlerUnsub {
    /// Remove this registration. Safe after context shutdown (no-op).
    pub fn unsubscribe(&self) -> usize {
        let Some(handlers) = self.handlers.upgrade() else {
            return 0;
        };
        unregister_app_token(&handlers, &self.event_name, self.token)
    }
}

/// Build an unsubscribe handle for `token` that does not capture `ctx`.
pub fn app_handler_unsub(
    ctx: &JSContext,
    event_name: impl Into<String>,
    token: HandlerToken,
) -> AppHandlerUnsub {
    AppHandlerUnsub {
        handlers: Rc::downgrade(&registry(ctx).handlers),
        event_name: event_name.into(),
        token,
    }
}

fn unregister_app_token(
    handlers: &RefCell<HashMap<Scope, Vec<HandlerEntry>>>,
    event_name: &str,
    token: HandlerToken,
) -> usize {
    if event_name.trim().is_empty() {
        return 0;
    }
    let mut remaining = 0usize;
    handlers.borrow_mut().retain(|scope, entries| {
        if !matches!(scope, Scope::App) {
            return true;
        }
        entries.retain(|handler| handler.event_name != event_name || handler.token != token);
        remaining += entries
            .iter()
            .filter(|handler| handler.event_name == event_name)
            .count();
        !entries.is_empty()
    });
    remaining
}

/// Identifies one registration. An unsubscribe handle carries its token so it
/// removes exactly the entry it created — registering the same function twice
/// yields two independent subscriptions, and a stale handle can never take out
/// a later one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandlerToken(u64);

#[derive(Clone)]
struct HandlerEntry {
    token: HandlerToken,
    event_name: String,
    callback: JSFunc,
}

impl JSContextService for EventBusRegistry {}

fn registry(ctx: &JSContext) -> &EventBusRegistry {
    if ctx.get_service::<EventBusRegistry>().is_none() {
        ctx.set_service(EventBusRegistry::default());
    }
    ctx.get_service::<EventBusRegistry>()
        .expect("event bus registry was inserted above")
}

/// Initialize the context registry (idempotent).
pub(crate) fn init(ctx: &JSContext) {
    registry(ctx);
}

/// Remove all handler registrations for a page instance (e.g., on unload).
pub(crate) fn clear_page(ctx: &JSContext, page_instance_id: &str) {
    let registry = registry(ctx);
    registry
        .handlers
        .borrow_mut()
        .retain(|scope, _| match scope {
            Scope::PageInstance(id) => id != page_instance_id,
            _ => true,
        });
}

/// Register an app-scoped handler.
pub fn register_app_handler(
    ctx: &JSContext,
    event_name: &str,
    callback: JSFunc,
) -> JSResult<HandlerToken> {
    if event_name.trim().is_empty() {
        return Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "event_name is required",
        )));
    }

    let registry = registry(ctx);
    let token = HandlerToken(registry.next_token.get());
    registry.next_token.set(token.0 + 1);
    let entry = HandlerEntry {
        token,
        event_name: event_name.to_string(),
        callback,
    };

    registry
        .handlers
        .borrow_mut()
        .entry(Scope::App)
        .or_default()
        .push(entry);
    Ok(token)
}

/// Remove exactly the app-scoped registration `token` created. Returns the
/// remaining handler count for that event, so a caller can tear down its
/// native listener when the last subscription goes.
pub fn unregister_app_handler_token(
    ctx: &JSContext,
    event_name: &str,
    token: HandlerToken,
) -> usize {
    unregister_app_token(&registry(ctx).handlers, event_name, token)
}

/// Unregister an app-scoped handler by event name.
/// When `callback` is `None`, removes all handlers for that event.
/// Returns the remaining handler count for the event.
pub fn unregister_app_handler(
    ctx: &JSContext,
    event_name: &str,
    callback: Option<JSFunc>,
) -> usize {
    if event_name.trim().is_empty() {
        return 0;
    }
    let registry = registry(ctx);
    let mut remaining = 0usize;
    registry.handlers.borrow_mut().retain(|scope, entries| {
        if !matches!(scope, Scope::App) {
            return true;
        }
        if let Some(ref cb) = callback {
            entries.retain(|h| h.event_name != event_name || h.callback != *cb);
        } else {
            entries.retain(|h| h.event_name != event_name);
        }
        remaining += entries
            .iter()
            .filter(|h| h.event_name == event_name)
            .count();
        !entries.is_empty()
    });
    remaining
}

/// Register a page-scoped handler (page_path required).
pub fn register_page_handler(
    ctx: &JSContext,
    page_instance_id: &str,
    event_name: &str,
    callback: JSFunc,
) -> JSResult<()> {
    if event_name.trim().is_empty() {
        return Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "event_name is required",
        )));
    }
    if page_instance_id.trim().is_empty() {
        return Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "page_instance_id is required",
        )));
    }

    let registry = registry(ctx);
    let token = HandlerToken(registry.next_token.get());
    registry.next_token.set(token.0 + 1);
    let entry = HandlerEntry {
        token,
        event_name: event_name.to_string(),
        callback,
    };

    registry
        .handlers
        .borrow_mut()
        .entry(Scope::PageInstance(page_instance_id.to_string()))
        .or_default()
        .push(entry);
    Ok(())
}

/// Unregister page-scoped handlers for a given page + event (removes all matching).
pub fn unregister_page_handler(ctx: &JSContext, page_instance_id: &str, event_name: &str) {
    if page_instance_id.trim().is_empty() || event_name.trim().is_empty() {
        return;
    }
    let instance_id = page_instance_id.to_string();
    let registry = registry(ctx);
    registry.handlers.borrow_mut().retain(|scope, entries| {
        if let Scope::PageInstance(id) = scope
            && id == &instance_id
        {
            entries.retain(|h| h.event_name != event_name);
            return !entries.is_empty();
        }
        true
    });
}

/// Dispatch an app bus event into the correct JS handlers on the JS thread.
pub(crate) async fn dispatch_app_bus_event(ctx: &JSContext, event: &AppBusEvent) -> JSResult<()> {
    match &event.scope {
        Scope::App => {
            emit_to_handlers(
                ctx,
                Scope::App,
                &event.event_name,
                event.payload_json.as_deref(),
            )
            .await
        }
        Scope::PageInstance(path) => {
            emit_to_handlers(
                ctx,
                Scope::PageInstance(path.clone()),
                &event.event_name,
                event.payload_json.as_deref(),
            )
            .await
        }
    }
}

async fn emit_to_handlers(
    ctx: &JSContext,
    scope: Scope,
    event_name: &str,
    payload_json: Option<&str>,
) -> JSResult<()> {
    let registry = registry(ctx);
    let handlers = {
        let map = registry.handlers.borrow();
        map.get(&scope).cloned().unwrap_or_default()
    };

    if handlers.is_empty() {
        return Ok(());
    }

    info!(
        "Dispatching {} scope={:?} handlers={}",
        event_name,
        scope,
        handlers.len()
    );

    let payload_base = if let Some(json) = payload_json {
        JSObject::from_json_string(ctx, json).unwrap_or_else(|_| JSObject::new(ctx))
    } else {
        JSObject::new(ctx)
    };

    for handler in handlers.into_iter().filter(|h| h.event_name == event_name) {
        let payload = payload_base.clone();
        let _ = handler.callback.call_async::<_, ()>(None, (payload,)).await;
    }

    Ok(())
}

/// Emit an app-scoped event into JS.
pub fn publish_app_event(appid: &str, event_name: &str, payload_json: Option<String>) -> bool {
    let Some(lxapp) = crate::try_get(appid) else {
        warn!("publish_app_event: unknown appid {}", appid);
        return false;
    };

    let event = AppBusEvent {
        scope: Scope::App,
        event_name: event_name.to_string(),
        payload_json,
    };

    if let Err(e) = lxapp.executor.dispatch_app_bus_event(lxapp.clone(), event) {
        error!("Failed to dispatch app event: {}", e).with_appid(appid.to_string());
        false
    } else {
        true
    }
}

/// Emit a page-scoped event into JS (page_path required).
pub fn publish_page_event(
    appid: &str,
    page_path: &str,
    event_name: &str,
    payload_json: Option<String>,
) -> bool {
    if page_path.trim().is_empty() {
        warn!("publish_page_event: missing page_path");
        return false;
    }

    let Some(lxapp) = crate::try_get(appid) else {
        warn!("publish_page_event: unknown appid {}", appid);
        return false;
    };

    let Some(instance_id) = lxapp
        .get_page(page_path)
        .map(|page| page.instance_id_string())
    else {
        warn!(
            "publish_page_event: no live page instance for {}",
            page_path
        );
        return false;
    };

    let event = AppBusEvent {
        scope: Scope::PageInstance(instance_id),
        event_name: event_name.to_string(),
        payload_json,
    };

    if let Err(e) = lxapp.executor.dispatch_app_bus_event(lxapp.clone(), event) {
        error!("Failed to dispatch page event: {}", e).with_appid(appid.to_string());
        false
    } else {
        true
    }
}

#[cfg(test)]
mod token_tests {
    use super::*;
    use rong::{JSEngine, RongJS};

    /// Two subscriptions on one function are independent, and a token removes
    /// exactly its own entry — the guarantee an unsubscribe handle makes.
    #[test]
    fn a_token_removes_only_its_own_registration() -> JSResult<()> {
        let runtime = RongJS::runtime();
        let ctx = runtime.context();
        let callback = JSFunc::new(&ctx, || {})?;
        let first = register_app_handler(&ctx, "evt", callback.clone())?;
        let second = register_app_handler(&ctx, "evt", callback.clone())?;
        let other = register_app_handler(&ctx, "other", callback)?;
        assert_ne!(first, second);

        assert_eq!(unregister_app_handler_token(&ctx, "other", first), 1);
        let evt_count = registry(&ctx)
            .handlers
            .borrow()
            .get(&Scope::App)
            .into_iter()
            .flatten()
            .filter(|entry| entry.event_name == "evt")
            .count();
        assert_eq!(evt_count, 2, "an event name mismatch must be inert");

        assert_eq!(
            unregister_app_handler_token(&ctx, "evt", first),
            1,
            "the sibling subscription must survive"
        );
        assert_eq!(unregister_app_handler_token(&ctx, "evt", first), 1);
        assert_eq!(unregister_app_handler_token(&ctx, "evt", second), 0);
        assert_eq!(unregister_app_handler_token(&ctx, "other", other), 0);
        Ok(())
    }

    /// The handle removes its registration without being handed a context.
    /// (That it cannot form the #246 cycle is a property of the type — it
    /// holds a `Weak` to the map, not a `JSContext` — not of this test.)
    #[test]
    fn an_unsub_handle_does_not_need_the_context() -> JSResult<()> {
        let runtime = RongJS::runtime();
        let ctx = runtime.context();
        let callback = JSFunc::new(&ctx, || {})?;
        let token = register_app_handler(&ctx, "evt", callback)?;
        let off = app_handler_unsub(&ctx, "evt", token);
        assert_eq!(off.unsubscribe(), 0);
        assert_eq!(off.unsubscribe(), 0, "a second call is inert");
        Ok(())
    }
}
