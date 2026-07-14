//! `lx.shell` — the shell chrome writer API (home lxapp only).
//!
//! The activator is the shell's single persistent-entry mechanism: entries are
//! declared at runtime with an idempotent full-list `set` (never in YAML), so
//! call timing is a non-issue — any moment converges to the same state.

use crate::app::ensure_home_lxapp;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lxapp::{LxApp, register_app_handler, unregister_app_handler};
use rong::{JSContext, JSFunc, JSObject, JSResult, JSValue};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

/// Serialized activator items last pushed to the shell, so `update` can merge
/// a patch without the caller re-sending the full list.
static ACTIVATOR_ITEMS: LazyLock<Mutex<Vec<Value>>> = LazyLock::new(|| Mutex::new(Vec::new()));
/// Action-item ids whose click handlers are currently registered.
static ACTION_HANDLER_IDS: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

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

fn activator_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let shell = shell_namespace(ctx)?;
    match shell.get::<_, JSObject>("activator") {
        Ok(obj) => Ok(obj),
        Err(_) => {
            let obj = JSObject::new(ctx);
            shell.set("activator", obj.clone())?;
            Ok(obj)
        }
    }
}

/// The bus event carrying an action item's click, suffixed by its id.
fn action_event(id: &str) -> String {
    format!("lx.shell.activator:{id}")
}

/// Activator items persist in the host data dir (Rust-owned, like pins) so
/// every platform skin restores the same entries before the home logic
/// boots. Action items are stored too but skins skip them on restore —
/// their handlers only exist once the writer re-declares them.
fn persist_activator_items(lxapp: &LxApp, payload: &str) {
    let path = lxapp.app_data_dir().join("shell-activator.json");
    if let Err(err) = std::fs::write(&path, payload) {
        log::warn!("failed to persist activator items: {err}");
    }
}

/// Serialized activator items from the previous run ("[]" when none).
pub(crate) fn persisted_activator_items() -> String {
    lxapp::get_platform()
        .map(|platform| {
            use lingxia_platform::traits::app_runtime::AppRuntime;
            platform.app_data_dir().join("shell-activator.json")
        })
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_else(|| "[]".to_string())
}

struct ParsedActivatorItem {
    value: Value,
    action_handler: Option<(String, JSFunc)>,
}

fn has_item_property(item: &JSObject, field: &str) -> bool {
    item.get::<_, JSValue>(field)
        .ok()
        .is_some_and(|value| !value.is_undefined() && !value.is_null())
}

fn optional_item_string(item: &JSObject, field: &str) -> JSResult<Option<String>> {
    if !has_item_property(item, field) {
        return Ok(None);
    }
    let value = item.get::<_, String>(field).map_err(|_| {
        rong::HostError::new(
            rong::error::E_INVALID_ARG,
            format!("activator item {field} must be a string"),
        )
    })?;
    let value = value.trim();
    if value.is_empty() {
        return Err(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            format!("activator item {field} must not be empty"),
        )
        .into());
    }
    Ok(Some(value.to_string()))
}

/// Validate and serialize one incoming item without mutating handler state.
/// Surface items carry exactly one content key (`lxapp` / `native`); action
/// items carry `id` + `handler`.
fn parse_item(item: &JSObject) -> JSResult<ParsedActivatorItem> {
    let app_id = optional_item_string(item, "lxapp")?;
    let capability = optional_item_string(item, "native")?;
    let id = optional_item_string(item, "id")?;
    let key_count = [app_id.is_some(), capability.is_some(), id.is_some()]
        .into_iter()
        .filter(|present| *present)
        .count();
    if key_count != 1 {
        return Err(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            "activator item must set exactly one of lxapp, native, or id",
        )
        .into());
    }

    let name = optional_item_string(item, "name")?;
    let icon = optional_item_string(item, "icon")?;
    let color = optional_item_string(item, "color")?;
    if let Some(app_id) = app_id {
        if has_item_property(item, "handler") {
            return Err(rong::HostError::new(
                rong::error::E_INVALID_ARG,
                "an lxapp activator item must not set handler",
            )
            .into());
        }
        return Ok(ParsedActivatorItem {
            value: json!({ "kind": "lxapp", "key": app_id, "name": name, "icon": icon, "color": color }),
            action_handler: None,
        });
    }
    if let Some(capability) = capability {
        if has_item_property(item, "handler") {
            return Err(rong::HostError::new(
                rong::error::E_INVALID_ARG,
                "a native activator item must not set handler",
            )
            .into());
        }
        return Ok(ParsedActivatorItem {
            value: json!({ "kind": "native", "key": capability, "name": name, "icon": icon, "color": color }),
            action_handler: None,
        });
    }

    let id = id.expect("exactly one key was validated");
    let handler = item.get::<_, JSFunc>("handler").map_err(|_| {
        rong::HostError::new(
            rong::error::E_INVALID_ARG,
            "an action activator item requires handler",
        )
    })?;
    let name = name.ok_or_else(|| {
        rong::HostError::new(
            rong::error::E_INVALID_ARG,
            "an action activator item requires name",
        )
    })?;
    let icon = icon.ok_or_else(|| {
        rong::HostError::new(
            rong::error::E_INVALID_ARG,
            "an action activator item requires icon",
        )
    })?;
    Ok(ParsedActivatorItem {
        value: json!({ "kind": "action", "key": id, "name": name, "icon": icon, "color": color }),
        action_handler: Some((id, handler)),
    })
}

/// `lx.shell.activator.set(items)` — idempotent full-list declaration. The
/// shell diffs against the previous state; repeat calls converge.
fn activator_set(ctx: JSContext, items: Vec<JSObject>) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    ensure_home_lxapp(&lxapp, "lx.shell.activator.set")?;

    // Validate the complete generation before changing handlers, persistence,
    // or native chrome. A bad later item must leave the previous set intact.
    let mut parsed = Vec::with_capacity(items.len());
    let mut keys = HashSet::new();
    for item in &items {
        let item = parse_item(item)?;
        let key = item
            .value
            .get("key")
            .and_then(Value::as_str)
            .expect("validated activator item has a key");
        if !keys.insert(key.to_string()) {
            return Err(rong::HostError::new(
                rong::error::E_INVALID_ARG,
                format!("duplicate activator key '{key}'"),
            )
            .into());
        }
        parsed.push(item);
    }
    let specs: Vec<Value> = parsed.iter().map(|item| item.value.clone()).collect();
    let payload = serde_json::to_string(&specs).unwrap_or_else(|_| "[]".to_string());

    // Native chrome accepts the complete list before the handler generation is
    // swapped, so a platform failure leaves the old state fully operational.
    lxapp
        .runtime
        .set_activator_items(&payload)
        .map_err(|e| crate::i18n::js_error_from_platform_error(&e))?;

    let mut handler_ids = ACTION_HANDLER_IDS.lock().map_err(|_| {
        rong::HostError::new(rong::error::E_INTERNAL, "activator handler state poisoned")
    })?;
    for id in handler_ids.drain() {
        unregister_app_handler(&ctx, &action_event(&id), None);
    }
    for item in parsed {
        if let Some((id, handler)) = item.action_handler {
            register_app_handler(&ctx, &action_event(&id), handler)?;
            handler_ids.insert(id);
        }
    }
    if let Ok(mut state) = ACTIVATOR_ITEMS.lock() {
        *state = specs;
    }
    persist_activator_items(&lxapp, &payload);
    Ok(())
}

/// `lx.shell.activator.update(key, patch)` — patch one item's icon / name / color
/// without re-sending the list. `key` is a content key value or an action id.
fn activator_update(ctx: JSContext, key: String, patch: JSObject) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    ensure_home_lxapp(&lxapp, "lx.shell.activator.update")?;

    let key = key.trim();
    if key.is_empty() {
        return Err(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            "activator update key must not be empty",
        )
        .into());
    }
    let name = optional_item_string(&patch, "name")?;
    let icon = optional_item_string(&patch, "icon")?;
    let color = optional_item_string(&patch, "color")?;
    if name.is_none() && icon.is_none() && color.is_none() {
        return Err(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            "activator update patch must set name, icon, or color",
        )
        .into());
    }
    let (next, payload) = {
        let state = ACTIVATOR_ITEMS.lock().map_err(|_| {
            rong::HostError::new(rong::error::E_INTERNAL, "activator state poisoned")
        })?;
        let mut next = state.clone();
        let Some(entry) = next
            .iter_mut()
            .find(|entry| entry.get("key").and_then(Value::as_str) == Some(key))
        else {
            return Err(rong::HostError::new(
                rong::error::E_NOT_FOUND,
                format!("no activator item with key '{key}'"),
            )
            .into());
        };
        if let (Some(name), Some(obj)) = (name, entry.as_object_mut()) {
            obj.insert("name".into(), json!(name));
        }
        if let (Some(icon), Some(obj)) = (icon, entry.as_object_mut()) {
            obj.insert("icon".into(), json!(icon));
        }
        if let (Some(color), Some(obj)) = (color, entry.as_object_mut()) {
            obj.insert("color".into(), json!(color));
        }
        let payload = serde_json::to_string(&next).unwrap_or_else(|_| "[]".to_string());
        (next, payload)
    };
    lxapp
        .runtime
        .set_activator_items(&payload)
        .map_err(|e| crate::i18n::js_error_from_platform_error(&e))?;
    *ACTIVATOR_ITEMS
        .lock()
        .map_err(|_| rong::HostError::new(rong::error::E_INTERNAL, "activator state poisoned"))? =
        next;
    persist_activator_items(&lxapp, &payload);
    Ok(())
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_shell_property(ctx)?;
    register_activator_api(ctx)
}

rong::js_api! {
    fn register_shell_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const shell: "ShellApi" = shell_namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_activator_api(ctx) {
        namespace ActivatorApi = activator_namespace(ctx)?;
        fn set(ts_params = "items: ShellActivatorItem[]") = activator_set;
        fn update(
            ts_params = "key: string, patch: { icon?: string; name?: string; color?: string }"
        ) = activator_update;
    }
}
