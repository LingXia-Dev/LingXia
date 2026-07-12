use crate::i18n::js_error_from_platform_error;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lxapp::{LxApp, register_app_handler, unregister_app_handler};
use rong::{JSContext, JSFunc, JSObject, JSResult};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

/// Number of tray menu items each app currently has registered, so a later
/// `setMenu` can unregister the previous items' click handlers.
static MENU_COUNTS: LazyLock<Mutex<HashMap<String, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Live `lx.tray.onClick` subscriptions (the tray is host-global). While any
/// exist, the native tray intercepts left-clicks for JS instead of running the
/// configured surface action.
static CLICK_HANDLERS: AtomicUsize = AtomicUsize::new(0);

fn tray_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("tray") {
        Ok(obj) => Ok(obj),
        Err(_) => {
            let obj = JSObject::new(ctx);
            lx.set("tray", obj.clone())?;
            Ok(obj)
        }
    }
}

/// lx.tray.setBadge(value) — the menu-bar / system-tray badge. Null/empty clears it.
fn set_badge(ctx: JSContext, text: Option<String>) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .set_tray_badge(text.as_deref().unwrap_or(""))
        .map_err(|e| js_error_from_platform_error(&e))
}

/// lx.tray.setIcon(icon) — replace the tray icon (a resource path).
fn set_icon(ctx: JSContext, icon: String) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .set_tray_icon(&icon)
        .map_err(|e| js_error_from_platform_error(&e))
}

/// lx.tray.setTitle(text) — text shown beside the icon (macOS). Empty clears it.
fn set_title(ctx: JSContext, text: Option<String>) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .set_tray_title(text.as_deref().unwrap_or(""))
        .map_err(|e| js_error_from_platform_error(&e))
}

/// lx.tray.show() — show the tray status item.
fn show(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .set_tray_visible(true)
        .map_err(|e| js_error_from_platform_error(&e))
}

/// lx.tray.hide() — hide the tray status item.
fn hide(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .runtime
        .set_tray_visible(false)
        .map_err(|e| js_error_from_platform_error(&e))
}

/// lx.tray.onClick(handler) — left-click on the tray icon. While at least one
/// handler is registered, the left-click runs only the handler(s); the tray's
/// configured surface action is suppressed. Returns an unsubscribe function.
fn on_click(ctx: JSContext, handler: JSFunc) -> JSResult<JSFunc> {
    register_app_handler(&ctx, "lx.tray.click", handler.clone())?;
    if CLICK_HANDLERS.fetch_add(1, Ordering::SeqCst) == 0 {
        let lxapp = LxApp::from_ctx(&ctx)?;
        let _ = lxapp.runtime.set_tray_click_intercept(true);
    }
    let off_ctx = ctx.clone();
    let off_handler = handler;
    // Guard against a double `off()`: a second call must not decrement the count
    // again (which would underflow and break future interception).
    let unsubscribed = std::sync::atomic::AtomicBool::new(false);
    JSFunc::new(&ctx, move || {
        if unsubscribed.swap(true, Ordering::SeqCst) {
            return;
        }
        unregister_app_handler(&off_ctx, "lx.tray.click", Some(off_handler.clone()));
        if CLICK_HANDLERS.fetch_sub(1, Ordering::SeqCst) == 1
            && let Ok(lxapp) = LxApp::from_ctx(&off_ctx)
        {
            let _ = lxapp.runtime.set_tray_click_intercept(false);
        }
    })
}

/// lx.tray.setMenu(items) — replace the tray dropdown menu. Each item is
/// `{ label, onClick?, enabled?, checked? }` or `{ separator: true }`. The native
/// menu is built from labels; clicks are routed back to each item's `onClick` by
/// index over the app event bus.
fn set_menu(ctx: JSContext, items: Vec<JSObject>) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let appid = lxapp.appid.clone();

    // Drop the previous menu's per-index click handlers.
    let previous = MENU_COUNTS.lock().ok().and_then(|m| m.get(&appid).copied());
    if let Some(prev) = previous {
        for i in 0..prev {
            unregister_app_handler(&ctx, &format!("lx.tray.menu:{i}"), None);
        }
    }

    let mut specs: Vec<Value> = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        if item.get::<_, bool>("separator").unwrap_or(false) {
            specs.push(json!({ "separator": true }));
            continue;
        }
        let label = item.get::<_, String>("label").unwrap_or_default();
        let enabled = item.get::<_, bool>("enabled").unwrap_or(true);
        let checked = item.get::<_, bool>("checked").unwrap_or(false);
        specs.push(json!({ "label": label, "enabled": enabled, "checked": checked }));
        if let Ok(on_click) = item.get::<_, JSFunc>("onClick") {
            register_app_handler(&ctx, &format!("lx.tray.menu:{i}"), on_click)?;
        }
    }

    if let Ok(mut counts) = MENU_COUNTS.lock() {
        counts.insert(appid, items.len());
    }

    let json = serde_json::to_string(&specs).unwrap_or_else(|_| "[]".to_string());
    lxapp
        .runtime
        .set_tray_menu(&json)
        .map_err(|e| js_error_from_platform_error(&e))
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_tray_property(ctx)?;
    register_tray_api(ctx)
}

rong::js_api! {
    fn register_tray_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const tray: "TrayApi" = tray_namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_tray_api(ctx) {
        namespace TrayApi = tray_namespace(ctx)?;
        fn setBadge(ts_params = "value: string | number | null") = set_badge;
        fn setIcon(ts_params = "icon: string") = set_icon;
        fn setTitle(ts_params = "text: string | null") = set_title;
        fn setMenu(ts_params = "items: Array<TrayMenuItem | TrayMenuSeparator>") = set_menu;
        fn onClick(
            ts_params = "handler: () => void",
            ts_return = "() => void"
        ) = on_click;
        fn show = show;
        fn hide = hide;
    }
}
