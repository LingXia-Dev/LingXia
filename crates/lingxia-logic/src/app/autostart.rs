use crate::authorization::{self, LogicRoute};
use crate::i18n::js_error_from_platform_error;
use lingxia_platform::traits::app_runtime::AppRuntime;
use rong::{HostError, JSContext, JSFunc, JSObject, JSResult};

/// `lx.host.autostart` — launch-at-startup control, when the host declared the
/// `autostart` capability; otherwise every call fails `E_NOT_SUPPORTED`.
pub(super) fn init(ctx: &JSContext, app: &JSObject) -> JSResult<()> {
    if !crate::capability::exposes(ctx, "host.autostart") {
        return super::unsupported_autostart(ctx, app);
    }
    let autostart = JSObject::new(ctx);
    autostart.set(
        "isEnabled",
        JSFunc::new(ctx, autostart_is_enabled)?.name("isEnabled")?,
    )?;
    autostart.set(
        "enable",
        JSFunc::new(ctx, |ctx: JSContext| autostart_set(ctx, true))?.name("enable")?,
    )?;
    autostart.set(
        "disable",
        JSFunc::new(ctx, |ctx: JSContext| autostart_set(ctx, false))?.name("disable")?,
    )?;
    app.set("autostart", autostart)?;
    Ok(())
}

/// Reads the live OS registration (login items / Run key), never a cached
/// preference — the user can flip it outside the app at any time.
async fn autostart_is_enabled(ctx: JSContext) -> JSResult<bool> {
    let invocation = authorization::require(&ctx, LogicRoute::AppAutostartIsEnabled)?;
    let lxapp = invocation.lxapp();
    let runtime = lxapp.runtime.clone();
    tokio::task::spawn_blocking(move || runtime.autostart_is_enabled())
        .await
        .map_err(|error| {
            HostError::new(
                rong::error::E_INTERNAL,
                format!("autostart query task failed: {error}"),
            )
        })?
        .map_err(|error| js_error_from_platform_error(&error))
}

async fn autostart_set(ctx: JSContext, enabled: bool) -> JSResult<()> {
    let route = if enabled {
        LogicRoute::AppAutostartEnable
    } else {
        LogicRoute::AppAutostartDisable
    };
    let invocation = authorization::require(&ctx, route)?;
    let lxapp = invocation.lxapp();
    let runtime = lxapp.runtime.clone();
    tokio::task::spawn_blocking(move || runtime.autostart_set_enabled(enabled))
        .await
        .map_err(|error| {
            HostError::new(
                rong::error::E_INTERNAL,
                format!("autostart update task failed: {error}"),
            )
        })?
        .map_err(|error| js_error_from_platform_error(&error))
}
