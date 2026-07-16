//! `lx.shell.activators` — app-declared host-shell entries (home lxapp only).

use crate::app::ensure_home_lxapp;
use lingxia_shell::{
    ActivatorCollection, NativeShellCapability, ShellActivator, ShellActivatorTarget,
    ShellActivatorUpdate, ShellError,
};
use lxapp::{LxApp, register_app_handler, unregister_app_handler};
use rong::{JSContext, JSFunc, JSObject, JSResult, JSValue};
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Default)]
struct ActionHandlerGeneration {
    generation: u64,
    handlers: HashMap<String, JSFunc>,
}

thread_local! {
    static ACTION_HANDLERS: RefCell<ActionHandlerGeneration> = RefCell::new(ActionHandlerGeneration::default());
}

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

fn activators_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let shell = shell_namespace(ctx)?;
    match shell.get::<_, JSObject>("activators") {
        Ok(obj) => Ok(obj),
        Err(_) => {
            let obj = JSObject::new(ctx);
            shell.set("activators", obj.clone())?;
            Ok(obj)
        }
    }
}

fn action_event(generation: u64, id: &str) -> String {
    format!("lx.shell.activators:{generation}:{id}")
}

struct ParsedActivator {
    item: ShellActivator,
    action_handler: Option<JSFunc>,
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
            format!("shell activator {field} must be a string"),
        )
    })?;
    let value = value.trim();
    if value.is_empty() {
        return Err(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            format!("shell activator {field} must not be empty"),
        )
        .into());
    }
    Ok(value.to_string())
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
            format!("shell activator {field} must be a boolean"),
        )
        .into()
    })
}

fn parse_item(item: &JSObject) -> JSResult<ParsedActivator> {
    let id = required_string(item, "id")?;
    let lxapp = optional_string(item, "lxapp")?;
    let native = optional_string(item, "native")?;
    let has_action = has_property(item, "onActivate");
    let target_count = [lxapp.is_some(), native.is_some(), has_action]
        .into_iter()
        .filter(|present| *present)
        .count();
    if target_count != 1 {
        return Err(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            "shell activator must set exactly one of lxapp, native, or onActivate",
        )
        .into());
    }

    let label = optional_string(item, "label")?;
    let icon = optional_string(item, "icon")?;
    let disabled = optional_bool(item, "disabled")?.unwrap_or(false);
    let (target, action_handler) = if let Some(key) = lxapp {
        (ShellActivatorTarget::Lxapp { key }, None)
    } else if let Some(native) = native {
        if native != "terminal" {
            return Err(rong::HostError::new(
                rong::error::E_INVALID_ARG,
                format!("unsupported shell native capability '{native}'"),
            )
            .into());
        }
        (
            ShellActivatorTarget::Native {
                key: NativeShellCapability::Terminal,
            },
            None,
        )
    } else {
        let handler = item.get::<_, JSFunc>("onActivate").map_err(|_| {
            rong::HostError::new(
                rong::error::E_INVALID_ARG,
                "shell activator onActivate must be a function",
            )
        })?;
        (ShellActivatorTarget::Action, Some(handler))
    };

    let item = ShellActivator {
        id,
        target,
        label,
        icon,
        disabled,
    }
    .validate()
    .map_err(js_error)?;
    Ok(ParsedActivator {
        item,
        action_handler,
    })
}

/// Atomically replaces the complete desktop activator declaration. Home lxapp
/// only. Relative icons resolve from the home app bundle; lxapp/native entries
/// persist across restarts, while action entries return after Logic registers
/// their callbacks. `replace([])` is an explicit persistent empty declaration.
fn activators_replace(ctx: JSContext, items: Vec<JSObject>) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    ensure_home_lxapp(&lxapp, "lx.shell.activators.replace")?;
    let parsed = items.iter().map(parse_item).collect::<JSResult<Vec<_>>>()?;
    let next_items = parsed.iter().map(|item| item.item.clone()).collect();
    let next_handlers = parsed
        .into_iter()
        .filter_map(|item| item.action_handler.map(|handler| (item.item.id, handler)))
        .collect();
    commit_generation(&ctx, |next| next.replace(next_items), next_handlers)
}

/// Updates presentation fields for one stable id. Home lxapp only.
fn activators_update(ctx: JSContext, id: String, patch: JSObject) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    ensure_home_lxapp(&lxapp, "lx.shell.activators.update")?;
    let patch = ShellActivatorUpdate {
        label: optional_string(&patch, "label")?,
        icon: optional_string(&patch, "icon")?,
        disabled: optional_bool(&patch, "disabled")?,
    };
    let handlers = retained_handlers();
    commit_generation(&ctx, |next| next.update(&id, patch), handlers)
}

/// Removes one stable id from the declaration. Home lxapp only.
fn activators_remove(ctx: JSContext, id: String) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    ensure_home_lxapp(&lxapp, "lx.shell.activators.remove")?;
    let mut handlers = retained_handlers();
    handlers.remove(id.trim());
    commit_generation(&ctx, |next| next.remove(&id), handlers)
}

/// Persists an explicit empty declaration. Home lxapp only.
fn activators_clear(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    ensure_home_lxapp(&lxapp, "lx.shell.activators.clear")?;
    commit_generation(
        &ctx,
        |next| {
            next.clear();
            Ok(())
        },
        HashMap::new(),
    )
}

fn retained_handlers() -> HashMap<String, JSFunc> {
    ACTION_HANDLERS.with(|state| state.borrow().handlers.clone())
}

fn commit_generation(
    ctx: &JSContext,
    mutate: impl FnOnce(&mut ActivatorCollection) -> Result<(), ShellError>,
    mut next_handlers: HashMap<String, JSFunc>,
) -> JSResult<()> {
    let manager = lingxia_shell::manager().map_err(js_error)?;
    let previous = manager.snapshot().activators;
    let mut next = previous.clone();
    mutate(&mut next).map_err(js_error)?;
    next_handlers.retain(|id, _| {
        next.items()
            .iter()
            .any(|item| item.id == *id && matches!(item.target, ShellActivatorTarget::Action))
    });

    let next_generation = next.generation();
    let mut registered: Vec<String> = Vec::new();
    for (id, handler) in &next_handlers {
        let event = action_event(next_generation, id);
        if let Err(error) = register_app_handler(ctx, &event, handler.clone()) {
            for event in registered {
                unregister_app_handler(ctx, &event, None);
            }
            return Err(error);
        }
        registered.push(event);
    }

    if let Err(error) = manager.commit_activators(previous.generation(), next.clone()) {
        for event in registered {
            unregister_app_handler(ctx, &event, None);
        }
        return Err(js_error(error).into());
    }
    if let Err(error) = lingxia_shell::apply_current_activators() {
        let _ = manager.commit_activators(next.generation(), previous.clone());
        let _ = lingxia_shell::apply_current_activators();
        for event in registered {
            unregister_app_handler(ctx, &event, None);
        }
        return Err(js_error(error).into());
    }

    ACTION_HANDLERS.with(|state| {
        let mut state = state.borrow_mut();
        for id in state.handlers.keys() {
            unregister_app_handler(ctx, &action_event(state.generation, id), None);
        }
        state.generation = next_generation;
        state.handlers = next_handlers;
    });
    Ok(())
}

fn js_error(error: ShellError) -> rong::HostError {
    let code = match &error {
        ShellError::ActivatorNotFound { .. } => rong::error::E_NOT_FOUND,
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
    register_shell_property(ctx)?;
    register_activators_api(ctx)
}

rong::js_api! {
    fn register_shell_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const shell: "ShellApi" = shell_namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_activators_api(ctx) {
        namespace ShellActivatorsApi = activators_namespace(ctx)?;
        fn replace(ts_params = "items: ShellActivator[]") = activators_replace;
        fn update(ts_params = "id: string, patch: ShellActivatorUpdate") = activators_update;
        fn remove(ts_params = "id: string") = activators_remove;
        fn clear() = activators_clear;
    }
}
