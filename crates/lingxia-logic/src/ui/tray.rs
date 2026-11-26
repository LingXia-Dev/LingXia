use crate::authorization::{self, LogicRoute};
use crate::i18n::js_error;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lxapp::{app_handler_unsub, register_app_handler, unregister_app_handler};
use rong::{FromJSObject, JSContext, JSFunc, JSObject, JSResult, JSValue, function::Optional};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

/// Number of tray menu items each app currently has registered, so a later
/// `menu.replace` can unregister the previous items' click handlers.
static MENU_COUNTS: LazyLock<Mutex<HashMap<String, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Live `lx.host.tray.onClick` subscriptions (the tray is host-global). While any
/// exist, the native tray intercepts left-clicks for JS instead of running the
/// configured surface action.
static CLICK_HANDLERS: AtomicUsize = AtomicUsize::new(0);

/// The desktop tray item this product publishes.
fn tray_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    crate::namespace::child(ctx, &crate::app::app_namespace(ctx)?, "tray")
}

fn require_tray(ctx: &JSContext, route: LogicRoute) -> JSResult<std::sync::Arc<lxapp::LxApp>> {
    let invocation = authorization::require(ctx, route)?;
    if !crate::capability::exposes(ctx, "host.tray") {
        return Err(rong::HostError::new(
            rong::error::E_NOT_SUPPORTED,
            "this host has no tray item; check lx.supports('host.tray') first",
        )
        .into());
    }
    Ok(invocation.lxapp())
}

/// `lx.host.tray.update` patch; an absent key leaves that part unchanged.
#[derive(FromJSObject)]
#[ts_skip]
struct JSTrayPatch {
    icon: Option<String>,
    title: Option<JSValue>,
    visible: Option<bool>,
}

/// Change the tray item: its icon (a bundled resource), the text beside it
/// (macOS; `null` or `''` clears it), and whether it shows.
///
/// @supports host.tray
/// @throws {E_NOT_SUPPORTED} Off the desktop.
fn update(ctx: JSContext, patch: JSTrayPatch) -> JSResult<()> {
    let lxapp = require_tray(&ctx, LogicRoute::TrayUpdate)?;
    let title = match patch.title {
        None => None,
        Some(value) if value.is_null() || value.is_undefined() => Some(String::new()),
        Some(value) => Some(value.to_rust::<String>().map_err(|_| {
            crate::i18n::js_invalid_parameter_error("tray title must be a string or null")
        })?),
    };
    if let Some(icon) = patch.icon {
        lxapp.runtime.set_tray_icon(&icon).map_err(js_error)?;
    }
    if let Some(title) = title {
        lxapp.runtime.set_tray_title(&title).map_err(js_error)?;
    }
    if let Some(visible) = patch.visible {
        lxapp.runtime.set_tray_visible(visible).map_err(js_error)?;
    }
    Ok(())
}

/// Run `callback` on a left-click of the tray icon. While any handler is
/// registered, the tray's declared action does not run. Returns an unsubscribe function.
///
/// @supports host.tray
/// @throws {E_NOT_SUPPORTED} Off the desktop.
fn on_click(ctx: JSContext, handler: JSFunc, options: Optional<JSValue>) -> JSResult<JSFunc> {
    let scope = ctx.clone();
    lxapp::lx::subscribe(&scope, options.0, move || {
        let lxapp = require_tray(&ctx, LogicRoute::TrayOnClick)?;
        let token = register_app_handler(&ctx, "lx.host.tray.click", handler)?;
        if CLICK_HANDLERS.fetch_add(1, Ordering::SeqCst) == 0 {
            let _ = lxapp.runtime.set_tray_click_intercept(true);
        }
        let off = app_handler_unsub(&ctx, "lx.host.tray.click", token);
        let runtime = lxapp.runtime.clone();
        // Guard against a double `off()`: a second call must not decrement the count
        // again (which would underflow and break future interception).
        let unsubscribed = std::sync::atomic::AtomicBool::new(false);
        JSFunc::new(&ctx, move || {
            if unsubscribed.swap(true, Ordering::SeqCst) {
                return;
            }
            off.unsubscribe();
            if CLICK_HANDLERS.fetch_sub(1, Ordering::SeqCst) == 1 {
                let _ = runtime.set_tray_click_intercept(false);
            }
        })
    })
}

/// Replace the tray's dropdown menu. A picked item runs its `onSelect`.
///
/// @supports host.tray
/// @throws {E_NOT_SUPPORTED} Off the desktop.
fn replace_menu(ctx: JSContext, items: Vec<JSObject>) -> JSResult<()> {
    let lxapp = require_tray(&ctx, LogicRoute::TrayMenuReplace)?;
    let appid = lxapp.appid.clone();

    // Drop the previous menu's per-index click handlers.
    let previous = MENU_COUNTS.lock().ok().and_then(|m| m.get(&appid).copied());
    if let Some(prev) = previous {
        for i in 0..prev {
            unregister_app_handler(&ctx, &format!("lx.host.tray.menu:{i}"), None);
        }
    }

    let mut specs: Vec<Value> = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        if item.get::<_, bool>("separator").unwrap_or(false) {
            specs.push(json!({ "separator": true }));
            continue;
        }
        let label = item.get::<_, String>("label").unwrap_or_default();
        // Natives take `enabled`.
        let enabled = !item.get::<_, bool>("disabled").unwrap_or(false);
        let checked = item.get::<_, bool>("checked").unwrap_or(false);
        specs.push(json!({ "label": label, "enabled": enabled, "checked": checked }));
        if let Ok(on_select) = item.get::<_, JSFunc>("onSelect") {
            register_app_handler(&ctx, &format!("lx.host.tray.menu:{i}"), on_select)?;
        }
    }

    if let Ok(mut counts) = MENU_COUNTS.lock() {
        counts.insert(appid, items.len());
    }

    let json = serde_json::to_string(&specs).unwrap_or_else(|_| "[]".to_string());
    lxapp.runtime.set_tray_menu(&json).map_err(js_error)
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_tray_property(ctx)?;
    register_tray_api(ctx)?;
    register_menu_api(ctx)
}

rong::js_api! {
    fn register_tray_property(ctx) {
        namespace HostAppApi = crate::app::app_namespace(ctx)?;
        const tray: "TrayApi" = tray_namespace(ctx)?;
    }
}

fn menu_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    crate::namespace::child(ctx, &tray_namespace(ctx)?, "menu")
}

rong::js_api! {
    fn register_tray_api(ctx) {
        namespace TrayApi = tray_namespace(ctx)?;
        const menu: "TrayMenuApi" = menu_namespace(ctx)?;
        fn update(ts_params = "patch: TrayPatch") = update;
        fn onClick(
            ts_params = "callback: () => void, options?: SignalOptions",
            ts_return = "() => void"
        ) = on_click;
    }
}

rong::js_api! {
    fn register_menu_api(ctx) {
        namespace TrayMenuApi = menu_namespace(ctx)?;
        fn replace(ts_params = "items: readonly (TrayMenuItem | TrayMenuSeparator)[]") = replace_menu;
    }
}
