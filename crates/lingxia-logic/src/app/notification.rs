use crate::authorization::{self, LogicRoute};
use crate::i18n::{js_error_from_platform_error, js_invalid_parameter_error};
use lingxia_platform::traits::app_runtime::{AppRuntime, LocalNotificationShow};
use rong::{FromJSObject, HostError, JSContext, JSFunc, JSObject, JSResult, JSValue};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_ID_CHARS: usize = 64;

#[derive(Debug, Clone, Default, FromJSObject)]
#[ts_skip]
struct JSSchedule {
    at: Option<f64>,
    #[js_name = "delayMs"]
    delay_ms: Option<f64>,
}

#[derive(Debug, Clone, Default, FromJSObject)]
#[ts_skip]
struct JSShowOptions {
    id: Option<String>,
    title: Option<String>,
    body: Option<String>,
    applink: Option<String>,
    schedule: Option<JSSchedule>,
    silent: Option<bool>,
}

/// `lx.app.notification` — local banners as a resume affordance. Absent unless
/// the host declared `capabilities.notifications`.
pub(super) fn init(ctx: &JSContext, app: &JSObject) -> JSResult<()> {
    if !lingxia_app_context::capability::notifications() {
        return Ok(());
    }
    let notification = JSObject::new(ctx);
    notification.set(
        "getPermission",
        JSFunc::new(ctx, get_permission)?.name("getPermission")?,
    )?;
    notification.set(
        "requestPermission",
        JSFunc::new(ctx, request_permission)?.name("requestPermission")?,
    )?;
    notification.set("show", JSFunc::new(ctx, show)?.name("show")?)?;
    notification.set("cancel", JSFunc::new(ctx, cancel)?.name("cancel")?)?;
    notification.set(
        "cancelAll",
        JSFunc::new(ctx, cancel_all)?.name("cancelAll")?,
    )?;
    app.set("notification", notification)?;
    Ok(())
}

async fn get_permission(ctx: JSContext) -> JSResult<String> {
    let invocation = authorization::require(&ctx, LogicRoute::AppNotificationGetPermission)?;
    let runtime = invocation.lxapp().runtime.clone();
    spawn_blocking(move || runtime.notification_permission()).await
}

async fn request_permission(ctx: JSContext) -> JSResult<String> {
    let invocation = authorization::require(&ctx, LogicRoute::AppNotificationRequestPermission)?;
    let runtime = invocation.lxapp().runtime.clone();
    spawn_blocking(move || runtime.notification_request_permission()).await
}

async fn show(ctx: JSContext, options: JSValue) -> JSResult<JSObject> {
    let (invocation, request) =
        authorization::require_before_decode(&ctx, LogicRoute::AppNotificationShow, || {
            decode_show(options)
        })?;
    let runtime = invocation.lxapp().runtime.clone();
    let id = request.id.clone();
    let status = spawn_blocking(move || runtime.notification_show(&request)).await?;
    let result = JSObject::new(&ctx);
    result.set("id", id)?;
    result.set("status", status.as_str())?;
    Ok(result)
}

async fn cancel(ctx: JSContext, id: JSValue) -> JSResult<()> {
    let (invocation, id) =
        authorization::require_before_decode(&ctx, LogicRoute::AppNotificationCancel, || {
            let id = id.to_rust::<String>()?;
            if id.is_empty() {
                return Err(js_invalid_parameter_error(
                    "lx.app.notification.cancel id must not be empty",
                ));
            }
            Ok(id)
        })?;
    let runtime = invocation.lxapp().runtime.clone();
    spawn_blocking(move || runtime.notification_cancel(&id)).await
}

async fn cancel_all(ctx: JSContext) -> JSResult<()> {
    let invocation = authorization::require(&ctx, LogicRoute::AppNotificationCancelAll)?;
    let runtime = invocation.lxapp().runtime.clone();
    spawn_blocking(move || runtime.notification_cancel_all()).await
}

async fn spawn_blocking<T, F>(work: F) -> JSResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, lingxia_platform::error::PlatformError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| {
            HostError::new(
                rong::error::E_INTERNAL,
                format!("notification task failed: {error}"),
            )
        })?
        .map_err(|error| js_error_from_platform_error(&error))
}

fn decode_show(options: JSValue) -> JSResult<LocalNotificationShow> {
    let parsed = options.to_rust::<JSShowOptions>()?;
    let title = parsed
        .title
        .filter(|title| !title.is_empty())
        .ok_or_else(|| js_invalid_parameter_error("lx.app.notification.show title is required"))?;
    let id = match parsed.id {
        Some(id) => validate_id(&id)?,
        None => uuid::Uuid::new_v4().to_string(),
    };
    if let Some(applink) = parsed.applink.as_deref() {
        validate_applink(applink)?;
    }
    Ok(LocalNotificationShow {
        id,
        title,
        body: parsed.body.unwrap_or_default(),
        applink: parsed.applink.filter(|link| !link.is_empty()),
        deliver_at_ms: schedule_at_ms(parsed.schedule)?,
        silent: parsed.silent.unwrap_or(false),
    })
}

fn validate_id(id: &str) -> JSResult<String> {
    if id.is_empty() || id.chars().count() > MAX_ID_CHARS {
        return Err(js_invalid_parameter_error(format!(
            "lx.app.notification.show id must be 1–{MAX_ID_CHARS} characters"
        )));
    }
    Ok(id.to_string())
}

fn validate_applink(url: &str) -> JSResult<()> {
    match lingxia_service::applink::parse(url) {
        Ok(Some(_)) => Ok(()),
        Ok(None) | Err(_) => Err(js_invalid_parameter_error(
            "lx.app.notification.show applink must be an https URL on a configured host",
        )),
    }
}

fn schedule_at_ms(schedule: Option<JSSchedule>) -> JSResult<Option<u64>> {
    let Some(schedule) = schedule else {
        return Ok(None);
    };
    match (schedule.at, schedule.delay_ms) {
        (Some(at), None) if at >= 0.0 => Ok(Some(at as u64)),
        (Some(_), None) => Err(js_invalid_parameter_error(
            "lx.app.notification.show schedule.at must be >= 0",
        )),
        (None, Some(delay)) if delay >= 0.0 => Ok(Some(unix_now_ms().saturating_add(delay as u64))),
        (None, Some(_)) => Err(js_invalid_parameter_error(
            "lx.app.notification.show schedule.delayMs must be >= 0",
        )),
        (Some(_), Some(_)) => Err(js_invalid_parameter_error(
            "lx.app.notification.show schedule takes at or delayMs, not both",
        )),
        (None, None) => Ok(None),
    }
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{JSSchedule, schedule_at_ms, validate_applink, validate_id};

    #[test]
    fn id_rejects_empty_and_overlong() {
        assert!(validate_id("").is_err());
        assert!(validate_id(&"x".repeat(65)).is_err());
        assert!(validate_id("ok").is_ok());
    }

    #[test]
    fn applink_rejects_non_https() {
        assert!(validate_applink("http://example.com/x").is_err());
        assert!(validate_applink("not-a-url").is_err());
    }

    #[test]
    fn schedule_rejects_negative_at() {
        assert!(
            schedule_at_ms(Some(JSSchedule {
                at: Some(-1.0),
                delay_ms: None,
            }))
            .is_err()
        );
    }
}
