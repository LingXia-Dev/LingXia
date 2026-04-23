use crate::{error, info, warn};
use rong::{
    JSContext, JSFunc, JSObject, JSResult, JSRuntimeService, RongJSError, error::HostError,
};
use std::cell::RefCell;
use std::collections::HashMap;

/// Internal scope marker.
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

/// Registry stored on the JSRuntime for handler registrations.
pub(crate) struct EventBusRegistry {
    handlers: RefCell<HashMap<Scope, Vec<HandlerEntry>>>,
}

#[derive(Clone)]
struct HandlerEntry {
    event_name: String,
    callback: JSFunc,
}

impl JSRuntimeService for EventBusRegistry {}

impl Default for EventBusRegistry {
    fn default() -> Self {
        Self {
            handlers: RefCell::new(HashMap::new()),
        }
    }
}

/// Initialize the runtime registry (idempotent).
pub(crate) fn init(ctx: &JSContext) {
    ctx.runtime().get_or_init_service::<EventBusRegistry>();
}

/// Remove all handler registrations for a page (e.g., on unload).
pub(crate) fn clear_page(ctx: &JSContext, page_path: &str) {
    let registry = ctx.runtime().get_or_init_service::<EventBusRegistry>();
    registry
        .handlers
        .borrow_mut()
        .retain(|scope, _| match scope {
            Scope::PageInstance(path) => path != page_path,
            _ => true,
        });
}

/// Register an app-scoped handler.
pub fn register_app_handler(ctx: &JSContext, event_name: &str, callback: JSFunc) -> JSResult<()> {
    if event_name.trim().is_empty() {
        return Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "event_name is required",
        )));
    }

    let entry = HandlerEntry {
        event_name: event_name.to_string(),
        callback,
    };

    let registry = ctx.runtime().get_or_init_service::<EventBusRegistry>();
    registry
        .handlers
        .borrow_mut()
        .entry(Scope::App)
        .or_default()
        .push(entry);
    Ok(())
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
    let registry = ctx.runtime().get_or_init_service::<EventBusRegistry>();
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
    page_path: &str,
    event_name: &str,
    callback: JSFunc,
) -> JSResult<()> {
    if event_name.trim().is_empty() {
        return Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "event_name is required",
        )));
    }
    if page_path.trim().is_empty() {
        return Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "page_path is required",
        )));
    }

    let entry = HandlerEntry {
        event_name: event_name.to_string(),
        callback,
    };

    let registry = ctx.runtime().get_or_init_service::<EventBusRegistry>();
    registry
        .handlers
        .borrow_mut()
        .entry(Scope::PageInstance(page_path.to_string()))
        .or_default()
        .push(entry);
    Ok(())
}

/// Unregister page-scoped handlers for a given page + event (removes all matching).
pub fn unregister_page_handler(ctx: &JSContext, page_path: &str, event_name: &str) {
    if page_path.trim().is_empty() || event_name.trim().is_empty() {
        return;
    }
    let registry = ctx.runtime().get_or_init_service::<EventBusRegistry>();
    registry.handlers.borrow_mut().retain(|scope, entries| {
        if let Scope::PageInstance(path) = scope {
            if path == page_path {
                entries.retain(|h| h.event_name != event_name);
                return !entries.is_empty();
            }
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
    let registry = ctx.runtime().get_or_init_service::<EventBusRegistry>();
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

    let event = AppBusEvent {
        scope: Scope::PageInstance(page_path.to_string()),
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
