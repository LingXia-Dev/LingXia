use crate::authorization::{self, AttentionScope, LogicRoute};
use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_platform_error,
    js_invalid_parameter_error,
};
use lingxia_platform::traits::app_runtime::{AppRuntime, LocalNotificationShow};
use lingxia_service::navigation::{self, NavigationError, intent};
use lxapp::LxApp;
use rong::{FromJSObject, HostError, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
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
/// the host declared `capabilities.notifications`. Every lxapp gets it; a
/// guest's calls are scoped to itself by [`AttentionScope`].
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

/// The permission is the product's, so a guest's prompt names the product.
/// A guest may therefore cause at most one prompt per launch, and only while
/// it is open on screen, where the person can see who is asking. Once the
/// OS has an answer every caller just reads it.
async fn request_permission(ctx: JSContext) -> JSResult<String> {
    let invocation = authorization::require(&ctx, LogicRoute::AppNotificationRequestPermission)?;
    let app = invocation.lxapp();
    let runtime = app.runtime.clone();
    if AttentionScope::of(&invocation).owner().is_some() {
        let probe = runtime.clone();
        let current = spawn_blocking(move || probe.notification_permission()).await?;
        if current != "default" {
            return Ok(current);
        }
        guest_prompt::claim(&app, "lx.host.notification.requestPermission")?;
    }
    spawn_blocking(move || runtime.notification_request_permission()).await
}

/// One OS prompt per guest session, and only while that guest is open.
mod guest_prompt {
    use super::*;

    fn prompted() -> &'static Mutex<HashSet<(String, u64)>> {
        static PROMPTED: std::sync::OnceLock<Mutex<HashSet<(String, u64)>>> =
            std::sync::OnceLock::new();
        PROMPTED.get_or_init(|| Mutex::new(HashSet::new()))
    }

    pub(super) fn claim(app: &Arc<LxApp>, route: &str) -> JSResult<()> {
        if lxapp::open_region(&app.appid).is_none() {
            return Err(refused(format!(
                "{route}: a guest lxapp may ask for notification permission only while it is open"
            )));
        }
        let first = prompted()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert((app.appid.clone(), app.session_id()));
        if !first {
            return Err(refused(format!(
                "{route}: this lxapp already asked for notification permission in this launch"
            )));
        }
        Ok(())
    }

    fn refused(detail: String) -> RongJSError {
        js_error_from_business_code_with_detail(3006, detail)
    }
}

async fn show(ctx: JSContext, options: JSValue) -> JSResult<JSObject> {
    let (invocation, request) =
        authorization::require_before_decode(&ctx, LogicRoute::AppNotificationShow, || {
            decode_show(options)
        })?;
    let scope = AttentionScope::of(&invocation);
    let target = scope
        .notification_target("lx.host.notification.show", request.target)
        .map_err(|refusal| refusal.into_js())?;
    // The request is checked before the frontmost state is consulted, so an
    // invalid target cannot be quietly accepted as a suppressed show.
    navigation::validate(&target).map_err(navigation_error)?;
    let app = invocation.lxapp();
    let runtime = app.runtime.clone();
    if scope.owner().is_some() {
        // A post the OS has no answer for yet prompts on some platforms; for
        // a guest that is its one prompt, under the same rule.
        let probe = runtime.clone();
        if spawn_blocking(move || probe.notification_permission()).await? == "default" {
            guest_prompt::claim(&app, "lx.host.notification.show")?;
        }
    }
    let id = request.id.clone();
    let platform_id = scope.platform_id(&request.id);
    // Persist the target before the OS is told anything, so a tap can never
    // arrive ahead of the record that explains it.
    let staged =
        intent::stage_owned(&platform_id, &target, scope.owner()).map_err(navigation_error)?;
    let platform_request = LocalNotificationShow {
        id: platform_id,
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
    let id = AttentionScope::of(&invocation).platform_id(&id);
    // Retire the target first: a banner the OS has not removed yet must not
    // still navigate.
    intent::invalidate(&id);
    spawn_blocking(move || runtime.notification_cancel(&id)).await
}

/// The product's `cancelAll` clears everything, guests' included. A guest's
/// clears only what it posted.
async fn cancel_all(ctx: JSContext) -> JSResult<()> {
    let invocation = authorization::require(&ctx, LogicRoute::AppNotificationCancelAll)?;
    let runtime = invocation.lxapp().runtime.clone();
    let Some(owner) = AttentionScope::of(&invocation).owner().map(str::to_string) else {
        intent::invalidate_all();
        return spawn_blocking(move || runtime.notification_cancel_all()).await;
    };
    let ids = intent::ids_owned_by(&owner);
    for id in &ids {
        intent::invalidate(id);
    }
    spawn_blocking(move || {
        for id in &ids {
            runtime.notification_cancel(id)?;
        }
        Ok(())
    })
    .await
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

/// What `show` asked for. The target is resolved against the caller's scope
/// once the caller is known.
struct ShowRequest {
    id: String,
    title: String,
    body: String,
    target: Option<serde_json::Value>,
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
    Ok(ShowRequest {
        id,
        title,
        body: parsed.body.unwrap_or_default(),
        target,
        deliver_at_ms: schedule_at_ms(parsed.schedule)?,
        silent: parsed.silent.unwrap_or(false),
    })
}

/// The raw target object; [`AttentionScope::notification_target`] decides
/// what an omitted one means for this caller.
fn decode_target(target: Option<JSObject>) -> JSResult<Option<serde_json::Value>> {
    let Some(object) = target else {
        return Ok(None);
    };
    let json = object.to_json_string().map_err(|error| {
        js_invalid_parameter_error(format!(
            "lx.host.notification.show target must be a plain object: {error}"
        ))
    })?;
    serde_json::from_str::<serde_json::Value>(&json)
        .map(Some)
        .map_err(|error| {
            js_invalid_parameter_error(format!("lx.host.notification.show target: {error}"))
        })
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
    use super::{JSSchedule, schedule_at_ms, validate_id};
    use crate::authorization::AttentionScope;
    use lingxia_service::navigation::NavigationTarget;

    fn decode_target_json(value: &serde_json::Value) -> Result<NavigationTarget, ()> {
        AttentionScope::Product
            .notification_target("lx.host.notification.show", Some(value.clone()))
            .map_err(|_| ())
    }

    fn target(text: &str) -> Result<NavigationTarget, ()> {
        decode_target_json(&serde_json::from_str(text).unwrap())
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
