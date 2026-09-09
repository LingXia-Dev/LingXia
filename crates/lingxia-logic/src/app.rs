use crate::authorization::{self, LogicRoute};
use crate::i18n::{js_error_from_platform_error, js_service_unavailable_error};
use lingxia_app_context::{app_config, env_version};
use lingxia_platform::traits::app_runtime::AppRuntime;
use rong::{IntoJSObject, JSContext, JSObject, JSResult, JSValue};

mod appearance;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod autostart;
mod cache;
mod display_language;
mod screenshot;
mod update;

/// Host app identity. Everything here is fixed for the life of the process;
/// the language the app renders in is not, and lives on
/// `lx.app.displayLanguage`.
#[derive(Debug, Clone, IntoJSObject)]
struct AppBaseInfo {
    /// Platform family: `"iOS"` / `"macOS"` / `"Android"` / `"Windows"` /
    /// `"Harmony"`. Matches the View-side `usePlatform().os` value.
    #[ts_type = "HostOs"]
    os: String,
    #[js_name = "productName"]
    product_name: String,
    #[js_name = "version"]
    version: String,
    #[js_name = "SDKVersion"]
    sdk_version: String,
}

/// Read the host app's identity: OS, product name, product version, and SDK
/// runtime version.
fn get_app_base_info(_ctx: JSContext) -> JSResult<AppBaseInfo> {
    let app_cfg =
        app_config().ok_or_else(|| js_service_unavailable_error("app config not available"))?;
    Ok(AppBaseInfo {
        os: lingxia_platform::os_label().to_string(),
        product_name: app_cfg.product_name.clone(),
        version: app_cfg.product_version.clone(),
        sdk_version: lxapp::SDK_RUNTIME_VERSION.to_string(),
    })
}

/// Exit the host app immediately without a confirmation dialog.
///
/// Control app only: quitting the product is not an lxapp's decision. Other
/// lxapps get a permission error.
///
/// If the user should confirm first, call `lx.showModal(...)` and invoke this
/// only after confirmation.
fn exit_app(ctx: JSContext) -> JSResult<()> {
    let invocation = authorization::require(&ctx, LogicRoute::AppExit)?;
    let lxapp = invocation.lxapp();
    lxapp::clear_active_display_language_session_override();
    lxapp
        .runtime
        .exit()
        .map_err(|e| js_error_from_platform_error(&e))
}

/// Set the app-icon badge, for example an unread count.
///
/// This targets the dock on macOS, taskbar on Windows, and home/launcher icon
/// on mobile — the product's own icon, not the calling lxapp's, so it is
/// Control app only and other lxapps get a permission error. Null or an empty
/// string clears it. Unsupported platforms treat the call as a no-op.
fn set_app_badge(ctx: JSContext, value: JSValue) -> JSResult<()> {
    let invocation = authorization::require(&ctx, LogicRoute::AppSetBadge)?;
    let lxapp = invocation.lxapp();
    let text = badge_text(value, "lx.app.setBadge")?;
    lxapp
        .runtime
        .set_app_badge(&text)
        .map_err(|e| js_error_from_platform_error(&e))
}

/// A badge is `string | number | null`. Coercing anything else would paint
/// `[object Object]` on the dock, so reject it at the boundary instead.
pub(crate) fn badge_text(value: JSValue, api: &str) -> JSResult<String> {
    if value.is_undefined() || value.is_null() {
        return Ok(String::new());
    }
    if value.is_string() || value.is_number() {
        return value.to_rust::<String>();
    }
    Err(rong::HostError::new(
        rong::error::E_INVALID_ARG,
        format!(
            "{api} value must be a string, a number, or null (received {})",
            value.type_of()
        ),
    )
    .into())
}

/// The native host app around this lxapp — its identity, updates, and window.
fn app_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("app") {
        Ok(obj) => Ok(obj),
        Err(_) => {
            let obj = JSObject::new(ctx);
            lx.set("app", obj.clone())?;
            Ok(obj)
        }
    }
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let app = app_namespace(ctx)?;
    init_base(ctx)?;
    register_app_controls(ctx)?;
    init_control_namespace(ctx, &app)?;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    autostart::init(ctx, &app)?;
    cache::init(ctx, &app)?;
    screenshot::init(ctx)?;
    update::init(ctx)?;

    Ok(())
}

/// Register read-only host identity for every lxapp context, including focused
/// system apps that intentionally do not receive the broader `lx.*` surface.
///
/// `lx.app.displayLanguage` belongs here: rendering in the product's language
/// is what every context does, control surfaces included.
pub(crate) fn init_base(ctx: &JSContext) -> JSResult<()> {
    register_app_property(ctx)?;
    register_app_base_api(ctx)?;
    let app = app_namespace(ctx)?;
    display_language::init_follower(ctx, &app)?;
    appearance::init_follower(ctx, &app)
}

/// `lx.app.control` — the members that edit product-wide settings, and the one
/// writer for each. Injected only into the ControlApp session, so
/// `lx.app.control?.…` and `lx.supports({ capability: 'control' })` always
/// agree. Every member behind it still authorizes on its own.
fn init_control_namespace(ctx: &JSContext, app: &JSObject) -> JSResult<()> {
    if !crate::capability::is_control_app(ctx) {
        return Ok(());
    }
    let control = JSObject::new(ctx);
    display_language::init_control(ctx, &control)?;
    appearance::init_control(ctx, &control)?;
    app.set("control", control)?;
    Ok(())
}

rong::js_api! {
    fn register_app_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const app: "HostAppApi" = app_namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_app_base_api(ctx) {
        namespace HostAppApi = app_namespace(ctx)?;
        const envVersion: "HostAppEnvVersion" = env_version().as_str();
        fn getBaseInfo = get_app_base_info;
    }
}

rong::js_api! {
    fn register_app_controls(ctx) {
        namespace HostAppApi = app_namespace(ctx)?;
        fn exit = exit_app;
        fn setBadge(ts_params = "value: string | number | null") = set_app_badge;
    }
}
