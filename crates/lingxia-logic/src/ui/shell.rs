//! `lx.shell` — the shell chrome writer API (home lxapp only).
//!
//! The activator is the shell's single persistent-entry mechanism: entries are
//! declared at runtime with an idempotent full-list `set` (never in YAML), so
//! call timing is a non-issue — any moment converges to the same state.

use crate::app::ensure_home_lxapp;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lxapp::{LxApp, register_app_handler, unregister_app_handler};
use rong::{JSContext, JSFunc, JSObject, JSResult};
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

/// Serialize one incoming item, registering an action item's click handler.
/// Surface items carry exactly one content key (`lxapp` / `native`); action
/// items carry `id` + `handler`.
fn parse_item(ctx: &JSContext, item: &JSObject) -> JSResult<Value> {
    let name = item.get::<_, String>("name").ok().filter(|s| !s.is_empty());
    let icon = item.get::<_, String>("icon").ok().filter(|s| !s.is_empty());
    let color = item
        .get::<_, String>("color")
        .ok()
        .filter(|s| !s.is_empty());
    if let Ok(app_id) = item.get::<_, String>("lxapp") {
        return Ok(
            json!({ "kind": "lxapp", "key": app_id, "name": name, "icon": icon, "color": color }),
        );
    }
    if let Ok(capability) = item.get::<_, String>("native") {
        return Ok(
            json!({ "kind": "native", "key": capability, "name": name, "icon": icon, "color": color }),
        );
    }
    let id = item.get::<_, String>("id").map_err(|_| {
        rong::HostError::new(
            rong::error::E_INVALID_ARG,
            "activator item must set lxapp, native, or an action id",
        )
    })?;
    if let Ok(handler) = item.get::<_, JSFunc>("handler") {
        register_app_handler(ctx, &action_event(&id), handler)?;
        if let Ok(mut ids) = ACTION_HANDLER_IDS.lock() {
            ids.insert(id.clone());
        }
    }
    // Action items need explicit presentation: they reference no content that
    // could supply metadata.
    Ok(json!({ "kind": "action", "key": id, "name": name, "icon": icon, "color": color }))
}

/// `lx.shell.activator.set(items)` — idempotent full-list declaration. The
/// shell diffs against the previous state; repeat calls converge.
fn activator_set(ctx: JSContext, items: Vec<JSObject>) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    ensure_home_lxapp(&lxapp, "lx.shell.activator.set")?;

    // Drop the previous generation's action handlers before re-registering.
    if let Ok(mut ids) = ACTION_HANDLER_IDS.lock() {
        for id in ids.drain() {
            unregister_app_handler(&ctx, &action_event(&id), None);
        }
    }

    let mut specs: Vec<Value> = Vec::with_capacity(items.len());
    for item in &items {
        specs.push(parse_item(&ctx, item)?);
    }
    let payload = serde_json::to_string(&specs).unwrap_or_else(|_| "[]".to_string());
    if let Ok(mut state) = ACTIVATOR_ITEMS.lock() {
        *state = specs;
    }
    lxapp
        .runtime
        .set_activator_items(&payload)
        .map_err(|e| crate::i18n::js_error_from_platform_error(&e))
}

/// `lx.shell.activator.update(key, patch)` — patch one item's icon / name
/// without re-sending the list. `key` is a content key value or an action id.
fn activator_update(ctx: JSContext, key: String, patch: JSObject) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    ensure_home_lxapp(&lxapp, "lx.shell.activator.update")?;

    let name = patch.get::<_, String>("name").ok();
    let icon = patch.get::<_, String>("icon").ok();
    let payload = {
        let mut state = ACTIVATOR_ITEMS.lock().map_err(|_| {
            rong::HostError::new(rong::error::E_INTERNAL, "activator state poisoned")
        })?;
        let Some(entry) = state
            .iter_mut()
            .find(|entry| entry.get("key").and_then(Value::as_str) == Some(key.as_str()))
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
        serde_json::to_string(&*state).unwrap_or_else(|_| "[]".to_string())
    };
    lxapp
        .runtime
        .set_activator_items(&payload)
        .map_err(|e| crate::i18n::js_error_from_platform_error(&e))
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
            ts_params = "key: string, patch: { icon?: string; name?: string }"
        ) = activator_update;
    }
}
