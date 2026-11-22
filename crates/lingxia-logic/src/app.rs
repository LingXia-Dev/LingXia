use crate::authorization::{self, LogicRoute};
use crate::i18n::{js_error_from_platform_error, js_service_unavailable_error};
use lingxia_app_context::{app_config, env};
use lingxia_platform::traits::app_runtime::AppRuntime;
use rong::{IntoJSObject, JSContext, JSFunc, JSObject, JSResult, JSValue, function::Optional};

mod appearance;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod autostart;
mod banner;
mod cache;
mod display_language;
mod notification;
pub(crate) use notification::{notification_permission, request_notification_permission};
mod screenshot;
mod service_env;
mod update;

/// Host app identity. Everything here is fixed for the life of the process;
/// the language the app renders in is not, and lives on `lx.app.language`.
#[derive(Debug, Clone, IntoJSObject)]
struct HostInfo {
    /// Platform family, lower case.
    #[ts_type = "HostOs"]
    os: String,
    /// The product's name.
    #[js_name = "productName"]
    product_name: String,
    /// The product's version.
    #[js_name = "version"]
    version: String,
    /// The LingXia runtime version.
    #[js_name = "sdkVersion"]
    sdk_version: String,
}

/// The host app's identity: OS, product name, product version, and SDK runtime
/// version.
fn host_info() -> JSResult<HostInfo> {
    let app_cfg =
        app_config().ok_or_else(|| js_service_unavailable_error("app config not available"))?;
    Ok(HostInfo {
        os: lingxia_platform::os_id().to_string(),
        product_name: lingxia_app_context::product_name()
            .unwrap_or(app_cfg.product_name.as_str())
            .to_string(),
        version: app_cfg.product_version.clone(),
        sdk_version: lxapp::SDK_RUNTIME_VERSION.to_string(),
    })
}

/// Exit the host app immediately without a confirmation dialog.
///
/// Control app only: quitting the product is not an lxapp's decision. Other
/// lxapps get a permission error.
///
/// If the user should confirm first, call `lx.ui.confirm(...)` and invoke this
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

/// `lx.host.badge`: a mark on the product's own system chrome, such as an unread count.
fn init_badge(ctx: &JSContext, app: &JSObject) -> JSResult<()> {
    let badge = JSObject::new(ctx);
    badge.set("set", JSFunc::new(ctx, set_badge)?.name("set")?)?;
    badge.set("clear", JSFunc::new(ctx, clear_badge)?.name("clear")?)?;
    app.set("badge", badge)?;
    Ok(())
}

async fn set_badge(
    ctx: JSContext,
    value: JSValue,
    options: Optional<JSObject>,
) -> JSResult<JSObject> {
    let invocation = authorization::require(&ctx, LogicRoute::AppBadgeSet)?;
    let text = badge_text(value)?;
    if text.is_empty() {
        return Err(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            "lx.host.badge.set needs a count or text; remove a badge with lx.host.badge.clear()",
        )
        .into());
    }
    if lingxia_platform::badge_surfaces().numeric_only && text.parse::<i64>().is_err() {
        return Err(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            format!(
                "lx.host.badge.set on this platform paints a count, so {text:?} is not a badge it can draw; pass a number"
            ),
        )
        .into());
    }
    paint_badge(&ctx, invocation.lxapp(), text, options.0).await
}

async fn clear_badge(ctx: JSContext, options: Optional<JSObject>) -> JSResult<JSObject> {
    let invocation = authorization::require(&ctx, LogicRoute::AppBadgeClear)?;
    paint_badge(&ctx, invocation.lxapp(), String::new(), options.0).await
}

/// Paints `text` (empty clears) on the surfaces `options` names.
async fn paint_badge(
    ctx: &JSContext,
    lxapp: std::sync::Arc<lxapp::LxApp>,
    text: String,
    options: Option<JSObject>,
) -> JSResult<JSObject> {
    let surface = badge_surface(options)?;
    let available = lingxia_platform::badge_surfaces();
    // A surface that exists but declined, such as a hidden macOS status item,
    // resolves `painted: false`; a surface the platform lacks rejects.
    let mut painted = false;
    let mut reachable = false;
    // Off the JS thread: a platform may block, and HarmonyOS answers through a
    // callback it can only wait for inside the async runtime.
    if surface != BadgeSurface::Tray && available.app_icon {
        reachable = true;
        let runtime = lxapp.runtime.clone();
        let text = text.clone();
        painted |= blocking(move || runtime.set_app_badge(&text)).await?;
    }
    if surface != BadgeSurface::AppIcon && available.tray {
        reachable = true;
        let runtime = lxapp.runtime.clone();
        let text = text.clone();
        painted |= blocking(move || runtime.set_tray_badge(&text)).await?;
    }
    if !reachable {
        return Err(rong::HostError::new(
            rong::error::E_NOT_SUPPORTED,
            "this platform has no badge surface of that kind; check lx.supports('host.badge') first",
        )
        .into());
    }
    let result = JSObject::new(ctx);
    result.set("painted", painted)?;
    Ok(result)
}

async fn blocking<F>(work: F) -> JSResult<bool>
where
    F: FnOnce() -> Result<bool, lingxia_platform::error::PlatformError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| {
            rong::HostError::new(
                rong::error::E_INTERNAL,
                format!("lx.host.badge task failed: {error}"),
            )
        })?
        .map_err(|error| js_error_from_platform_error(&error))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BadgeSurface {
    Auto,
    AppIcon,
    Tray,
}

fn badge_surface(options: Option<JSObject>) -> JSResult<BadgeSurface> {
    let Some(options) = options else {
        return Ok(BadgeSurface::Auto);
    };
    let Ok(surface) = options.get::<_, String>("surface") else {
        return Ok(BadgeSurface::Auto);
    };
    match surface.as_str() {
        "auto" => Ok(BadgeSurface::Auto),
        "app_icon" => Ok(BadgeSurface::AppIcon),
        "tray" => Ok(BadgeSurface::Tray),
        other => Err(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            format!("lx.host.badge surface must be auto, app_icon, or tray (received {other:?})"),
        )
        .into()),
    }
}

/// A badge is a string or a number. Coercing anything else would paint
/// `[object Object]` on the dock, so reject it at the boundary instead.
fn badge_text(value: JSValue) -> JSResult<String> {
    if value.is_string() || value.is_number() {
        return value.to_rust::<String>();
    }
    Err(rong::HostError::new(
        rong::error::E_INVALID_ARG,
        format!(
            "lx.host.badge.set value must be a string or a number (received {})",
            value.type_of()
        ),
    )
    .into())
}

/// The native host app around this lxapp — its identity, updates, and window.
pub(crate) fn app_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("host") {
        Ok(obj) => Ok(obj),
        Err(_) => {
            let obj = JSObject::new(ctx);
            lx.set("host", obj.clone())?;
            Ok(obj)
        }
    }
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let app = app_namespace(ctx)?;
    init_base(ctx)?;
    register_app_controls(ctx)?;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    autostart::init(ctx, &app)?;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    unsupported_autostart(ctx, &app)?;
    init_badge(ctx, &app)?;
    banner::init(ctx, &app)?;
    notification::init(ctx, &app)?;
    cache::init(ctx, &app)?;
    screenshot::init(ctx)?;
    update::init(ctx)?;
    service_env::init(ctx)?;

    Ok(())
}

/// Register read-only host identity for every lxapp context, including focused
/// system apps that intentionally do not receive the broader `lx.*` surface.
///
/// `lx.app.language` and `lx.app.appearance` belong here: rendering in the
/// product's language and scheme is what every context does, control surfaces
/// included. The preferences beside them on `lx.host` authorize on each call,
/// so only the Control app gets past them.
pub(crate) fn init_base(ctx: &JSContext) -> JSResult<()> {
    register_app_property(ctx)?;
    register_app_base_api(ctx)?;
    let lxapp_namespace = crate::namespace::app(ctx)?;
    display_language::init_app(ctx, &lxapp_namespace)?;
    appearance::init_app(ctx, &lxapp_namespace)?;
    let host = app_namespace(ctx)?;
    display_language::init_host(ctx, &host)?;
    appearance::init_host(ctx, &host)
}

rong::js_api! {
    fn register_app_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const host: "HostAppApi" = app_namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_app_base_api(ctx) {
        namespace HostAppApi = app_namespace(ctx)?;
        const env: "HostAppEnv" = env().as_str();
        const info: "Readonly<HostInfo>" = host_info()?;
    }
}

rong::js_api! {
    fn register_app_controls(ctx) {
        namespace HostAppApi = app_namespace(ctx)?;
        fn exit = exit_app;
    }
}

fn unsupported_autostart(ctx: &JSContext, app: &JSObject) -> JSResult<()> {
    crate::capability::unsupported_namespace(ctx, app, "lx.host.autostart", "host.autostart")?;
    Ok(())
}

/// `lx.host.terminal` where the host has no terminal: present, every call
/// fails `E_NOT_SUPPORTED`.
pub(crate) fn unsupported_terminal(ctx: &JSContext) -> JSResult<()> {
    let terminal = JSObject::new(ctx);
    for path in [
        "lx.host.terminal.settings",
        "lx.host.terminal.colorSchemes",
        "lx.host.terminal.fonts",
        "lx.host.terminal.windows",
    ] {
        crate::capability::unsupported_namespace(ctx, &terminal, path, "host.terminal")?;
    }
    app_namespace(ctx)?.set("terminal", terminal)?;
    Ok(())
}
