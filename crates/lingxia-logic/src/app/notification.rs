use crate::authorization::{self, LogicRoute};
use crate::i18n::{js_error_from_platform_error, js_invalid_parameter_error};
use lingxia_platform::traits::app_runtime::{AppRuntime, LocalNotificationShow};
use lingxia_service::navigation::{self, NavigationError, NavigationTarget, intent};
use rong::{FromJSObject, HostError, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError};
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
    target: Option<JSObject>,
    /// Removed in favour of `target`. Decoded only so the old call reports a
    /// parameter error instead of degrading into a bare activate.
    applink: Option<String>,
    schedule: Option<JSSchedule>,
    silent: Option<bool>,
}

/// `lx.host.notification` — local banners as a resume affordance. Absent unless
/// the host declared `capabilities.notifications`.
pub(super) fn init(ctx: &JSContext, app: &JSObject) -> JSResult<()> {
    if !crate::capability::exposes(ctx, "app.notification") {
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
    // Persist the target before the OS is told anything, so a tap can never
    // arrive ahead of the record that explains it.
    let staged = intent::stage(&request.id, &request.target).map_err(navigation_error)?;
    let platform_request = LocalNotificationShow {
        id: request.id,
        title: request.title,
        body: request.body,
        activation_token: staged.token.clone(),
        deliver_at_ms: request.deliver_at_ms,
        silent: request.silent,
    };
    let status = spawn_blocking(move || runtime.notification_show(&platform_request)).await;
    match status {
        Ok(status) => {
            if status == lingxia_platform::traits::app_runtime::LocalNotificationStatus::Suppressed
            {
                // Nothing was posted, so nothing may resolve this token later.
                intent::rollback(&staged);
            } else {
                intent::commit(&staged);
            }
            let result = JSObject::new(&ctx);
            result.set("id", id)?;
            result.set("status", status.as_str())?;
            Ok(result)
        }
        Err(error) => {
            intent::rollback(&staged);
            Err(error)
        }
    }
}

async fn cancel(ctx: JSContext, id: JSValue) -> JSResult<()> {
    let (invocation, id) =
        authorization::require_before_decode(&ctx, LogicRoute::AppNotificationCancel, || {
            let id = id.to_rust::<String>()?;
            if id.is_empty() {
                return Err(js_invalid_parameter_error(
                    "lx.host.notification.cancel id must not be empty",
                ));
            }
            Ok(id)
        })?;
    let runtime = invocation.lxapp().runtime.clone();
    // Retire the target first: a banner the OS has not removed yet must not
    // still navigate.
    intent::invalidate(&id);
    spawn_blocking(move || runtime.notification_cancel(&id)).await
}

async fn cancel_all(ctx: JSContext) -> JSResult<()> {
    let invocation = authorization::require(&ctx, LogicRoute::AppNotificationCancelAll)?;
    let runtime = invocation.lxapp().runtime.clone();
    intent::invalidate_all();
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

/// What `show` asked for, once the request itself is known to be valid.
struct ShowRequest {
    id: String,
    title: String,
    body: String,
    target: NavigationTarget,
    deliver_at_ms: Option<u64>,
    silent: bool,
}

fn decode_show(options: JSValue) -> JSResult<ShowRequest> {
    let parsed = options.to_rust::<JSShowOptions>()?;
    if parsed.applink.is_some() {
        return Err(js_invalid_parameter_error(
            "lx.host.notification.show no longer takes applink; pass target: { kind: 'page', page } or { kind: 'appLink', url }",
        ));
    }
    let title = parsed
        .title
        .filter(|title| !title.is_empty())
        .ok_or_else(|| js_invalid_parameter_error("lx.host.notification.show title is required"))?;
    let id = match parsed.id {
        Some(id) => validate_id(&id)?,
        None => uuid::Uuid::new_v4().to_string(),
    };
    let target = decode_target(parsed.target)?;
    // The request is checked before the frontmost state is consulted, so an
    // invalid target cannot be quietly accepted as a suppressed show.
    navigation::validate(&target).map_err(navigation_error)?;
    Ok(ShowRequest {
        id,
        title,
        body: parsed.body.unwrap_or_default(),
        target,
        deliver_at_ms: schedule_at_ms(parsed.schedule)?,
        silent: parsed.silent.unwrap_or(false),
    })
}

/// An omitted target is `activate`: bring the product forward, nothing else.
fn decode_target(target: Option<JSObject>) -> JSResult<NavigationTarget> {
    let Some(object) = target else {
        return Ok(NavigationTarget::Activate);
    };
    let json = object.to_json_string().map_err(|error| {
        js_invalid_parameter_error(format!(
            "lx.host.notification.show target must be a plain object: {error}"
        ))
    })?;
    let value = serde_json::from_str::<serde_json::Value>(&json).map_err(|error| {
        js_invalid_parameter_error(format!("lx.host.notification.show target: {error}"))
    })?;
    decode_target_json(&value)
}

fn decode_target_json(value: &serde_json::Value) -> JSResult<NavigationTarget> {
    if value.is_null() {
        return Ok(NavigationTarget::Activate);
    }
    NavigationTarget::from_json(value).map_err(navigation_error)
}

fn navigation_error(error: NavigationError) -> RongJSError {
    match error {
        NavigationError::InvalidTarget(message) => {
            js_invalid_parameter_error(format!("lx.host.notification.show {message}"))
        }
        NavigationError::Unavailable(message) | NavigationError::Internal(message) => {
            HostError::new(rong::error::E_INTERNAL, message).into()
        }
    }
}

fn validate_id(id: &str) -> JSResult<String> {
    if id.is_empty() || id.chars().count() > MAX_ID_CHARS {
        return Err(js_invalid_parameter_error(format!(
            "lx.host.notification.show id must be 1–{MAX_ID_CHARS} characters"
        )));
    }
    Ok(id.to_string())
}

fn schedule_at_ms(schedule: Option<JSSchedule>) -> JSResult<Option<u64>> {
    let Some(schedule) = schedule else {
        return Ok(None);
    };
    match (schedule.at, schedule.delay_ms) {
        (Some(at), None) if at >= 0.0 => Ok(Some(at as u64)),
        (Some(_), None) => Err(js_invalid_parameter_error(
            "lx.host.notification.show schedule.at must be >= 0",
        )),
        (None, Some(delay)) if delay >= 0.0 => Ok(Some(unix_now_ms().saturating_add(delay as u64))),
        (None, Some(_)) => Err(js_invalid_parameter_error(
            "lx.host.notification.show schedule.delayMs must be >= 0",
        )),
        (Some(_), Some(_)) => Err(js_invalid_parameter_error(
            "lx.host.notification.show schedule takes at or delayMs, not both",
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
    use super::{JSSchedule, decode_target_json, schedule_at_ms, validate_id};
    use lingxia_service::navigation::NavigationTarget;

    fn target(text: &str) -> Result<NavigationTarget, ()> {
        decode_target_json(&serde_json::from_str(text).unwrap()).map_err(|_| ())
    }

    #[test]
    fn id_rejects_empty_and_overlong() {
        assert!(validate_id("").is_err());
        assert!(validate_id(&"x".repeat(65)).is_err());
        assert!(validate_id("ok").is_ok());
    }

    #[test]
    fn an_omitted_target_activates() {
        assert_eq!(
            decode_target_json(&serde_json::Value::Null).unwrap(),
            NavigationTarget::Activate
        );
    }

    #[test]
    fn target_branches_do_not_mix() {
        assert!(target(r#"{"kind":"page","page":"order"}"#).is_ok());
        assert!(target(r#"{"kind":"route","name":"a"}"#).is_ok());
        assert!(target(r#"{"kind":"page","page":"/pages/order/index"}"#).is_err());
        assert!(target(r#"{"kind":"route","name":"a","url":"https://x.test/"}"#).is_err());
        assert!(target(r#"{"kind":"nope"}"#).is_err());
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
