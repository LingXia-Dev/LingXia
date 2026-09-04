//! `lx.shell.sidebarActions` — app-declared host-shell entries (Control app only).

use crate::app::ensure_control_caller;
use lingxia_shell::{
    ShellError, ShellSidebarAction, ShellSidebarActionUpdate, SidebarActionCollection,
    SidebarActionPlacement,
};
use lxapp::{LxApp, register_app_handler, unregister_app_handler};
use rong::{JSContext, JSContextService, JSFunc, JSObject, JSResult, JSValue};
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Default)]
struct SidebarActionHandlerRegistry {
    state: RefCell<SidebarActionHandlerGeneration>,
}

#[derive(Default)]
struct SidebarActionHandlerGeneration {
    generation: u64,
    handlers: HashMap<String, JSFunc>,
}

impl JSContextService for SidebarActionHandlerRegistry {
    fn on_shutdown(&self) {
        let state = self.state.borrow();
        if state.handlers.is_empty() && state.generation == 0 {
            return;
        }
        let Ok(manager) = lingxia_shell::manager() else {
            return;
        };
        let previous = manager.snapshot().sidebar_actions;
        if previous.generation() != state.generation {
            return;
        }
        let mut next = previous.clone();
        next.clear();
        if manager
            .commit_sidebar_actions(previous.generation(), next.clone())
            .is_ok()
            && lingxia_shell::apply_current_sidebar_actions().is_err()
        {
            let _ = manager.commit_sidebar_actions(next.generation(), previous);
            let _ = lingxia_shell::apply_current_sidebar_actions();
        }
    }
}

fn handler_registry(ctx: &JSContext) -> &SidebarActionHandlerRegistry {
    if ctx.get_service::<SidebarActionHandlerRegistry>().is_none() {
        ctx.set_service(SidebarActionHandlerRegistry::default());
    }
    ctx.get_service::<SidebarActionHandlerRegistry>()
        .expect("sidebar action handler registry was inserted above")
}

/// The host shell around the content — its sidebar shortcuts and chrome.
fn shell_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("shell") {
        Ok(obj) => Ok(obj),
        Err(_) => {
            let obj = JSObject::new(ctx);
            lx.set("shell", obj.clone())?;
            Ok(obj)
        }
    }
}

fn sidebar_actions_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let shell = shell_namespace(ctx)?;
    match shell.get::<_, JSObject>("sidebarActions") {
        Ok(obj) => Ok(obj),
        Err(_) => {
            let obj = JSObject::new(ctx);
            shell.set("sidebarActions", obj.clone())?;
            Ok(obj)
        }
    }
}

fn sidebar_action_event(generation: u64, id: &str) -> String {
    format!("lx.shell.sidebarActions:{generation}:{id}")
}

struct ParsedSidebarAction {
    item: ShellSidebarAction,
    handler: JSFunc,
}

fn has_property(item: &JSObject, field: &str) -> bool {
    item.get::<_, JSValue>(field)
        .ok()
        .is_some_and(|value| !value.is_undefined() && !value.is_null())
}

fn required_string(item: &JSObject, field: &'static str) -> JSResult<String> {
    let value = item.get::<_, String>(field).map_err(|_| {
        rong::HostError::new(
            rong::error::E_INVALID_ARG,
            format!("shell sidebar action {field} must be a string"),
        )
    })?;
    let value = value.trim();
    if value.is_empty() {
        return Err(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            format!("shell sidebar action {field} must not be empty"),
        )
        .into());
    }
    Ok(value.to_string())
}

fn reject_unknown_keys(item: &JSObject, allowed: &[&str]) -> JSResult<()> {
    for key in item.keys_as::<String>()? {
        if !allowed.contains(&key.as_str()) {
            return Err(rong::HostError::new(
                rong::error::E_INVALID_ARG,
                format!("unknown shell sidebar action field '{key}'"),
            )
            .into());
        }
    }
    Ok(())
}

fn optional_string(item: &JSObject, field: &'static str) -> JSResult<Option<String>> {
    has_property(item, field)
        .then(|| required_string(item, field))
        .transpose()
}

fn optional_bool(item: &JSObject, field: &'static str) -> JSResult<Option<bool>> {
    if !has_property(item, field) {
        return Ok(None);
    }
    item.get::<_, bool>(field).map(Some).map_err(|_| {
        rong::HostError::new(
            rong::error::E_INVALID_ARG,
            format!("shell sidebar action {field} must be a boolean"),
        )
        .into()
    })
}

fn parse_sidebar_action(item: &JSObject) -> JSResult<ParsedSidebarAction> {
    reject_unknown_keys(
        item,
        &["id", "placement", "label", "icon", "disabled", "onActivate"],
    )?;
    let id = required_string(item, "id")?;
    let placement = match required_string(item, "placement")?.as_str() {
        "header" => SidebarActionPlacement::Header,
        "footer" => SidebarActionPlacement::Footer,
        value => {
            return Err(rong::HostError::new(
                rong::error::E_INVALID_ARG,
                format!("unknown shell sidebar action placement '{value}'"),
            )
            .into());
        }
    };
    let label = required_string(item, "label")?;
    let icon = required_string(item, "icon")?;
    let disabled = optional_bool(item, "disabled")?.unwrap_or(false);
    let handler = item.get::<_, JSFunc>("onActivate").map_err(|_| {
        rong::HostError::new(
            rong::error::E_INVALID_ARG,
            "shell sidebar action onActivate must be a function",
        )
    })?;

    let item = ShellSidebarAction {
        id,
        placement,
        label,
        icon,
        disabled,
    }
    .validate()
    .map_err(js_error)?;
    Ok(ParsedSidebarAction { item, handler })
}

/// Atomically replaces the complete desktop sidebar action declaration. Only the
/// Control app may call this API. Ids must be non-empty and unique across both
/// placements; header accepts at most two entries. Icons must be bundled relative
/// paths or runtime-managed `lx://` paths accessible to the Control app.
///
/// Every entry is bound to its generation-scoped callback. The shell invokes that
/// callback but never infers navigation or selected state. Validation or host
/// projection failure leaves the previous generation active. `replace([])` clears
/// the chrome explicitly. Declarations are process-local, so call `replace` again
/// on every Logic launch.
fn sidebar_actions_replace(ctx: JSContext, items: Vec<JSObject>) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    ensure_control_caller(&lxapp, "lx.shell.sidebarActions.replace")?;
    let parsed = items
        .iter()
        .map(parse_sidebar_action)
        .collect::<JSResult<Vec<_>>>()?;
    let next_items = parsed.iter().map(|item| item.item.clone()).collect();
    let next_handlers = parsed
        .into_iter()
        .map(|item| (item.item.id, item.handler))
        .collect();
    commit_generation(&ctx, |next| next.replace(next_items), next_handlers)
}

/// Atomically updates the icon, label, and/or disabled state of one stable id.
/// Only the Control app may call this API. The patch must be non-empty; unknown
/// fields are rejected. The callback and placement stay unchanged. Throws
/// `E_NOT_FOUND` when `id` is not in the current declaration.
fn sidebar_actions_update(ctx: JSContext, id: String, patch: JSObject) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    ensure_control_caller(&lxapp, "lx.shell.sidebarActions.update")?;
    reject_unknown_keys(&patch, &["label", "icon", "disabled"])?;
    let patch = ShellSidebarActionUpdate {
        label: optional_string(&patch, "label")?,
        icon: optional_string(&patch, "icon")?,
        disabled: optional_bool(&patch, "disabled")?,
    };
    let handlers = retained_handlers(&ctx);
    commit_generation(&ctx, |next| next.update(&id, patch), handlers)
}

/// Atomically removes one stable id and its generation-scoped callback. Only the
/// Control app may call this API. Throws `E_NOT_FOUND` when `id` is not in the
/// current declaration.
fn sidebar_actions_remove(ctx: JSContext, id: String) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    ensure_control_caller(&lxapp, "lx.shell.sidebarActions.remove")?;
    let mut handlers = retained_handlers(&ctx);
    handlers.remove(id.trim());
    commit_generation(&ctx, |next| next.remove(&id), handlers)
}

/// Atomically clears every runtime sidebar action and callback. Only the home
/// lxapp may call this API. Equivalent to `replace([])` and safe when already
/// empty; the Control app must still redeclare actions after the next Logic launch.
fn sidebar_actions_clear(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    ensure_control_caller(&lxapp, "lx.shell.sidebarActions.clear")?;
    commit_generation(
        &ctx,
        |next| {
            next.clear();
            Ok(())
        },
        HashMap::new(),
    )
}

fn retained_handlers(ctx: &JSContext) -> HashMap<String, JSFunc> {
    handler_registry(ctx).state.borrow().handlers.clone()
}

fn commit_generation(
    ctx: &JSContext,
    mutate: impl FnOnce(&mut SidebarActionCollection) -> Result<(), ShellError>,
    mut next_handlers: HashMap<String, JSFunc>,
) -> JSResult<()> {
    let manager = lingxia_shell::manager().map_err(js_error)?;
    let previous = manager.snapshot().sidebar_actions;
    let mut next = previous.clone();
    mutate(&mut next).map_err(js_error)?;
    next_handlers.retain(|id, _| next.items().iter().any(|item| item.id == *id));

    let next_generation = next.generation();
    let mut registered: Vec<String> = Vec::new();
    for (id, handler) in &next_handlers {
        let event = sidebar_action_event(next_generation, id);
        if let Err(error) = register_app_handler(ctx, &event, handler.clone()) {
            for event in registered {
                unregister_app_handler(ctx, &event, None);
            }
            return Err(error);
        }
        registered.push(event);
    }

    if let Err(error) = manager.commit_sidebar_actions(previous.generation(), next.clone()) {
        for event in registered {
            unregister_app_handler(ctx, &event, None);
        }
        return Err(js_error(error).into());
    }
    if let Err(error) = lingxia_shell::apply_current_sidebar_actions() {
        let _ = manager.commit_sidebar_actions(next.generation(), previous.clone());
        let _ = lingxia_shell::apply_current_sidebar_actions();
        for event in registered {
            unregister_app_handler(ctx, &event, None);
        }
        return Err(js_error(error).into());
    }

    let registry = handler_registry(ctx);
    let mut state = registry.state.borrow_mut();
    for id in state.handlers.keys() {
        unregister_app_handler(ctx, &sidebar_action_event(state.generation, id), None);
    }
    state.generation = next_generation;
    state.handlers = next_handlers;
    Ok(())
}

fn js_error(error: ShellError) -> rong::HostError {
    let code = match &error {
        ShellError::SidebarActionNotFound { .. } => rong::error::E_NOT_FOUND,
        ShellError::Io(_)
        | ShellError::Host(_)
        | ShellError::NotInitialized
        | ShellError::ConcurrentMutation { .. }
        | ShellError::ConcurrentPinMutation => rong::error::E_INTERNAL,
        _ => rong::error::E_INVALID_ARG,
    };
    rong::HostError::new(code, error.to_string())
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let _ = handler_registry(ctx);
    register_shell_property(ctx)?;
    register_sidebar_actions_api(ctx)
}

rong::js_api! {
    fn register_shell_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const shell: "ShellApi" = shell_namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_sidebar_actions_api(ctx) {
        namespace ShellSidebarActionsApi = sidebar_actions_namespace(ctx)?;
        fn replace(ts_params = "items: ShellSidebarAction[]") = sidebar_actions_replace;
        fn update(ts_params = "id: string, patch: ShellSidebarActionUpdate") = sidebar_actions_update;
        fn remove(ts_params = "id: string") = sidebar_actions_remove;
        fn clear() = sidebar_actions_clear;
    }
}
