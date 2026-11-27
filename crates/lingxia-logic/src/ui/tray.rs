use crate::authorization::{self, LogicRoute};
use crate::i18n::{js_error, js_invalid_parameter_error};
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

/// Off the desktop a well-formed call fails here, after its arguments were checked.
fn require_tray(ctx: &JSContext) -> JSResult<()> {
    if crate::capability::exposes(ctx, "host.tray") {
        return Ok(());
    }
    Err(rong::HostError::new(
        rong::error::E_NOT_SUPPORTED,
        "this host has no tray item; check lx.supports('host.tray') first",
    )
    .into())
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
    let lxapp = authorization::require(&ctx, LogicRoute::TrayUpdate)?.lxapp();
    let title = match patch.title {
        None => None,
        Some(value) if value.is_null() || value.is_undefined() => Some(String::new()),
        Some(value) => Some(value.to_rust::<String>().map_err(|_| {
            crate::i18n::js_invalid_parameter_error("tray title must be a string or null")
        })?),
    };
    require_tray(&ctx)?;
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
    let lxapp = authorization::require(&ctx, LogicRoute::TrayOnClick)?.lxapp();
    let scope = ctx.clone();
    lxapp::lx::subscribe_gated(&scope, options.0, || require_tray(&scope), move || {
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
/// @throws {E_INVALID_ARG} An item without a non-empty `label`, or a field of the wrong type.
/// @throws {E_NOT_SUPPORTED} Off the desktop.
fn replace_menu(ctx: JSContext, items: Vec<JSObject>) -> JSResult<()> {
    let lxapp = authorization::require(&ctx, LogicRoute::TrayMenuReplace)?.lxapp();
    let appid = lxapp.appid.clone();

    // Validate the whole menu first: a refused item leaves the current menu in place.
    let mut specs: Vec<Value> = Vec::with_capacity(items.len());
    let mut handlers: Vec<(usize, JSFunc)> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let field =
            |name: &str, error: rong::RongJSError| error.at_field(name).at_index(i).at_argument(1);
        if item
            .get_opt::<_, bool>("separator")
            .map_err(|error| field("separator", error))?
            .unwrap_or(false)
        {
            specs.push(json!({ "separator": true }));
            continue;
        }
        let label = item
            .get_opt::<_, String>("label")
            .map_err(|error| field("label", error))?
            .filter(|label| !label.trim().is_empty())
            .ok_or_else(|| {
                js_invalid_parameter_error(format!(
                    "lx.host.tray.menu.replace item {i} needs a non-empty label"
                ))
            })?;
        // Natives take `enabled`.
        let disabled = item
            .get_opt::<_, bool>("disabled")
            .map_err(|error| field("disabled", error))?;
        let checked = item
            .get_opt::<_, bool>("checked")
            .map_err(|error| field("checked", error))?;
        if let Some(on_select) = item
            .get_opt::<_, JSFunc>("onSelect")
            .map_err(|error| field("onSelect", error))?
        {
            handlers.push((i, on_select));
        }
        specs.push(json!({
            "label": label,
            "enabled": !disabled.unwrap_or(false),
            "checked": checked.unwrap_or(false),
        }));
    }

    require_tray(&ctx)?;

    // Drop the previous menu's per-index click handlers.
    let previous = MENU_COUNTS.lock().ok().and_then(|m| m.get(&appid).copied());
    if let Some(prev) = previous {
        for i in 0..prev {
            unregister_app_handler(&ctx, &format!("lx.host.tray.menu:{i}"), None);
        }
    }
    for (i, on_select) in handlers {
        register_app_handler(&ctx, &format!("lx.host.tray.menu:{i}"), on_select)?;
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
