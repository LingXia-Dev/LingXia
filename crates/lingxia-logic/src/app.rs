use crate::authorization::{self, LogicRoute};
use crate::i18n::{js_error_from_platform_error, js_service_unavailable_error};
use lingxia_app_context::{app_config, env};
use lingxia_platform::traits::app_runtime::AppRuntime;
use rong::{IntoJSObject, JSContext, JSObject, JSResult, JSValue, function::Optional};

mod appearance;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod autostart;
mod banner;
mod cache;
mod display_language;
mod notification;
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

/// Mark the product in system chrome, for example with an unread count.
///
/// One call, because "where the count goes" is the platform's answer, not the
/// caller's: `auto` paints every product-owned surface this platform has — the
/// dock and the menu-bar item on macOS, the taskbar and the notification-area
/// item on Windows, the home-screen icon on iOS and HarmonyOS. Name a
/// `surface` only when one of them is the point.
///
/// It is the product's chrome, not the calling lxapp's, so it is Control app
/// only. Null or an empty string clears it.
///
/// Returns whether anything was actually painted. A platform with no such
/// chrome is a no-op that returns `false` rather than an error — portable code
/// can call this unconditionally — and
/// `lx.supports({ capability: 'badge' })` answers the same question up front.
async fn set_app_badge(
    ctx: JSContext,
    value: JSValue,
    options: Optional<JSObject>,
) -> JSResult<bool> {
    let invocation = authorization::require(&ctx, LogicRoute::AppSetBadge)?;
    let lxapp = invocation.lxapp();
    let surface = badge_surface(options.0)?;
    let available = lingxia_platform::badge_surfaces();
    let text = badge_text(value, "lx.app.setBadge")?;
    if available.numeric_only && !text.is_empty() && text.parse::<i64>().is_err() {
        return Err(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            format!(
                "lx.app.setBadge on this platform paints a count, so {text:?} is not a badge it can draw; pass a number or null"
            ),
        )
        .into());
    }

    // `auto` is best-effort decoration across whatever chrome exists: a
    // product with no status item must not have its dock badge fail on the
    // tray's account. A named surface is a specific request, so its failure is
    // the caller's to see.
    let strict = surface != BadgeSurface::Auto;
    let mut painted = false;
    // Off the JS thread: a platform may block, and HarmonyOS answers through a
    // callback it can only wait for inside the async runtime.
    if surface != BadgeSurface::Tray && available.app_icon {
        let runtime = lxapp.runtime.clone();
        let text = text.clone();
        let outcome = blocking(move || runtime.set_app_badge(&text)).await?;
        painted |= record(outcome, "appIcon", strict)?;
    }
    if surface != BadgeSurface::AppIcon && available.tray {
        let runtime = lxapp.runtime.clone();
        let text = text.clone();
        let outcome = blocking(move || runtime.set_tray_badge(&text)).await?;
        painted |= record(outcome, "tray", strict)?;
    }
    Ok(painted)
}

/// Runs a platform call off the JS thread and hands back whatever it returned,
/// so the caller decides whether a failure is worth reporting.
async fn blocking<F>(work: F) -> JSResult<Result<(), lingxia_platform::error::PlatformError>>
where
    F: FnOnce() -> Result<(), lingxia_platform::error::PlatformError> + Send + 'static,
{
    tokio::task::spawn_blocking(work).await.map_err(|error| {
        rong::HostError::new(
            rong::error::E_INTERNAL,
            format!("lx.app.setBadge task failed: {error}"),
        )
        .into()
    })
}

fn record(
    outcome: Result<(), lingxia_platform::error::PlatformError>,
    surface: &str,
    strict: bool,
) -> JSResult<bool> {
    match outcome {
        Ok(()) => Ok(true),
        Err(error) if strict => Err(js_error_from_platform_error(&error)),
        Err(error) => {
            log::debug!("lx.app.setBadge skipped the {surface} surface: {error}");
            Ok(false)
        }
    }
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
        "appIcon" => Ok(BadgeSurface::AppIcon),
        "tray" => Ok(BadgeSurface::Tray),
        other => Err(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            format!("lx.app.setBadge surface must be auto, appIcon, or tray (received {other:?})"),
        )
        .into()),
    }
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
    banner::init(ctx, &app)?;
    notification::init(ctx, &app)?;
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
        const env: "HostAppEnv" = env().as_str();
        fn getBaseInfo = get_app_base_info;
    }
}

rong::js_api! {
    fn register_app_controls(ctx) {
        namespace HostAppApi = app_namespace(ctx)?;
        fn exit = exit_app;
        fn setBadge(
            ts_params = "value: string | number | null, options?: SetBadgeOptions",
            ts_return = "Promise<boolean>"
        ) = set_app_badge;
    }
}
