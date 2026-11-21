use crate::authorization::{self, LogicRoute};
use crate::i18n::js_error_from_platform_error;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lxapp::{app_handler_unsub, register_app_handler, unregister_app_handler};
use rong::{JSContext, JSFunc, JSObject, JSResult, JSValue, function::Optional};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

/// Number of tray menu items each app currently has registered, so a later
/// `setMenu` can unregister the previous items' click handlers.
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

/// Replace the tray icon with a bundled resource.
fn set_icon(ctx: JSContext, icon: String) -> JSResult<()> {
    let invocation = authorization::require(&ctx, LogicRoute::TraySetIcon)?;
    let lxapp = invocation.lxapp();
    lxapp
        .runtime
        .set_tray_icon(&icon)
        .map_err(|e| js_error_from_platform_error(&e))
}

/// Set the text beside the icon (macOS). Empty clears it.
fn set_title(ctx: JSContext, text: Option<String>) -> JSResult<()> {
    let invocation = authorization::require(&ctx, LogicRoute::TraySetTitle)?;
    let lxapp = invocation.lxapp();
    lxapp
        .runtime
        .set_tray_title(text.as_deref().unwrap_or(""))
        .map_err(|e| js_error_from_platform_error(&e))
}

/// Show the tray item.
fn show(ctx: JSContext) -> JSResult<()> {
    let invocation = authorization::require(&ctx, LogicRoute::TrayShow)?;
    let lxapp = invocation.lxapp();
    lxapp
        .runtime
        .set_tray_visible(true)
        .map_err(|e| js_error_from_platform_error(&e))
}

/// Hide the tray item.
fn hide(ctx: JSContext) -> JSResult<()> {
    let invocation = authorization::require(&ctx, LogicRoute::TrayHide)?;
    let lxapp = invocation.lxapp();
    lxapp
        .runtime
        .set_tray_visible(false)
        .map_err(|e| js_error_from_platform_error(&e))
}

/// Run `handler` on a left-click of the tray icon. While any handler is
/// registered, the tray's declared action does not run. Returns an unsubscribe function.
fn on_click(ctx: JSContext, handler: JSFunc, options: Optional<JSValue>) -> JSResult<JSFunc> {
    let scope = ctx.clone();
    lxapp::lx::subscribe(&scope, options.0, move || {
        let invocation = authorization::require(&ctx, LogicRoute::TrayOnClick)?;
        let lxapp = invocation.lxapp();
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

/// Replace the tray's dropdown menu. A picked item runs its `onClick`.
fn set_menu(ctx: JSContext, items: Vec<JSObject>) -> JSResult<()> {
    let invocation = authorization::require(&ctx, LogicRoute::TraySetMenu)?;
    let lxapp = invocation.lxapp();
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
        let enabled = item.get::<_, bool>("enabled").unwrap_or(true);
        let checked = item.get::<_, bool>("checked").unwrap_or(false);
        specs.push(json!({ "label": label, "enabled": enabled, "checked": checked }));
        if let Ok(on_click) = item.get::<_, JSFunc>("onClick") {
            register_app_handler(&ctx, &format!("lx.host.tray.menu:{i}"), on_click)?;
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
        namespace HostAppApi = crate::app::app_namespace(ctx)?;
        const tray: "TrayApi" = tray_namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_tray_api(ctx) {
        namespace TrayApi = tray_namespace(ctx)?;
        fn setIcon(ts_params = "icon: string") = set_icon;
        fn setTitle(ts_params = "text: string | null") = set_title;
        fn setMenu(ts_params = "items: Array<TrayMenuItem | TrayMenuSeparator>") = set_menu;
        fn onClick(
            ts_params = "handler: () => void, options?: SignalOptions",
            ts_return = "() => void"
        ) = on_click;
        fn show = show;
        fn hide = hide;
    }
}
