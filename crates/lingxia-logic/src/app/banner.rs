use crate::authorization::{self, AttentionScope, LogicRoute};
use crate::i18n::{js_error, js_invalid_parameter_error};
use lingxia_error::Error;
use lingxia_platform::traits::app_runtime::{
    AppRuntime, DesktopBannerAction, DesktopBannerActionStyle, DesktopBannerBackground,
    DesktopBannerOutcome, DesktopBannerShow, DesktopBannerSource,
};
use lingxia_service::navigation::{
    self, NavigationError, NavigationRequest, NavigationSource, NavigationTarget,
};
use lxapp::LxApp;
use rong::{FromJSObject, HostError, JSContext, JSFunc, JSObject, JSResult, JSValue};
use std::sync::Arc;

const MAX_ID_CHARS: usize = 64;
const MAX_ACTIONS: usize = 2;
/// A notification banner has a button, so it would otherwise wait for one.
const NOTIFICATION_TIMEOUT_MS: u64 = 8_000;
const NOTIFICATION_OPEN: &str = "open";

#[derive(Debug, Clone, Default, FromJSObject)]
#[ts_skip]
struct JSAction {
    id: Option<String>,
    label: Option<String>,
    tone: Option<String>,
    target: Option<JSObject>,
}

#[derive(Debug, Clone, Default, FromJSObject)]
#[ts_skip]
struct JSShowOptions {
    id: Option<String>,
    title: Option<String>,
    #[js_name = "message"]
    body: Option<String>,
    actions: Option<Vec<JSAction>>,
    #[js_name = "durationMs"]
    timeout_ms: Option<f64>,
    background: Option<String>,
}

/// `lx.host.banner` — product-drawn desktop overlay; off desktop every call
/// fails `E_NOT_SUPPORTED`. A guest's banners are
/// scoped to it and carry its name and icon.
pub(super) fn init(ctx: &JSContext, app: &JSObject) -> JSResult<()> {
    if !crate::capability::exposes(ctx, "host.banner") {
        crate::capability::unsupported_namespace(ctx, app, "lx.host.banner", "host.banner")?;
        return Ok(());
    }
    let banner = JSObject::new(ctx);
    banner.set("show", JSFunc::new(ctx, show)?.name("show")?)?;
    banner.set("dismiss", JSFunc::new(ctx, dismiss)?.name("dismiss")?)?;
    app.set("banner", banner)?;
    Ok(())
}

async fn show(ctx: JSContext, options: JSValue) -> JSResult<JSObject> {
    let (invocation, (request, raw_targets)) =
        authorization::require_before_decode(&ctx, LogicRoute::AppBannerShow, || {
            decode_show(options)
        })?;
    let app = invocation.lxapp();
    let runtime = app.runtime.clone();
    let scope = AttentionScope::of(&invocation);
    // Scoped and checked before anything is shown, like a notification target.
    let mut targets = Vec::with_capacity(raw_targets.len());
    for (action, target) in raw_targets {
        let target = scope
            .host_target("lx.host.banner.show", Some(target), |name| {
                crate::ui::router::is_tab_name(&app, name)
            })
            .map_err(|refusal| refusal.into_js())?;
        navigation::validate(&target).map_err(navigation_error)?;
        targets.push((action, target));
    }
    let caller_id = request.id.clone();
    let mut request = request;
    request.id = scope.platform_id(&caller_id);
    let source = source_of(&scope, &app);
    let turn = call_order::take();
    let outcome = spawn_blocking(move || {
        let pending = call_order::in_turn(turn, || match &source {
            Some(source) => runtime.banner_enqueue_from(&request, source),
            None => runtime.banner_enqueue(&request),
        })?;
        pending.wait()
    })
    .await?;
    if let DesktopBannerOutcome::Action { action, .. } = &outcome
        && let Some((_, target)) = targets.into_iter().find(|(id, _)| id == action)
    {
        go(target);
    }
    encode_outcome(&ctx, &caller_id, outcome)
}

fn go(target: NavigationTarget) {
    navigation::activate();
    if let Err(error) =
        navigation::dispatch(NavigationRequest::new(target, NavigationSource::Banner))
    {
        log::warn!("banner action target failed: {}", error.message());
    }
}

fn source_of(scope: &AttentionScope, app: &Arc<LxApp>) -> Option<DesktopBannerSource> {
    scope.owner().map(|app_id| DesktopBannerSource {
        app_id: app_id.to_string(),
        name: Some(app.listing_name())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| app_id.to_string()),
        icon_path: lxapp::lxapp_display_icon(app_id).file().map(str::to_string),
    })
}

/// The banner id a notification shown while the product is frontmost uses,
/// kept apart from `lx.host.banner` ids.
pub(super) fn notification_banner_id(platform_id: &str) -> String {
    format!("notification:{platform_id}")
}

/// Show a notification as a banner because the product is frontmost. Returns
/// once it is queued; an Open button, when there is somewhere to go, follows
/// `target` like a tap on the notification would.
pub(super) async fn present_notification(
    app: Arc<LxApp>,
    scope: AttentionScope,
    platform_id: &str,
    title: String,
    body: String,
    target: NavigationTarget,
) -> JSResult<()> {
    let actions = if target == NavigationTarget::Activate {
        Vec::new()
    } else {
        vec![DesktopBannerAction {
            id: NOTIFICATION_OPEN.to_string(),
            label: crate::i18n::t(crate::I18nKey::CommonOpen),
            style: DesktopBannerActionStyle::Primary,
        }]
    };
    let request = DesktopBannerShow {
        id: notification_banner_id(platform_id),
        title,
        body,
        actions,
        timeout_ms: Some(NOTIFICATION_TIMEOUT_MS),
        background: DesktopBannerBackground::System,
    };
    let source = source_of(&scope, &app);
    let runtime = app.runtime.clone();
    let turn = call_order::take();
    let pending = spawn_blocking(move || {
        call_order::in_turn(turn, || match &source {
            Some(source) => runtime.banner_enqueue_from(&request, source),
            None => runtime.banner_enqueue(&request),
        })
    })
    .await?;
    let live = platform_id.to_string();
    live_notifications().insert(live.clone(), scope.owner().map(str::to_string));
    std::thread::spawn(move || {
        let outcome = pending.wait();
        live_notifications().remove(&live);
        if let Ok(DesktopBannerOutcome::Action { .. }) = outcome {
            go(target);
        }
    });
    Ok(())
}

/// Platform id → the guest that posted it.
type LiveNotifications = std::collections::HashMap<String, Option<String>>;

fn live_notifications() -> std::sync::MutexGuard<'static, LiveNotifications> {
    static LIVE: std::sync::OnceLock<std::sync::Mutex<LiveNotifications>> =
        std::sync::OnceLock::new();
    LIVE.get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

/// Close the frontmost-notification banners that `matches` (platform id,
/// posting guest), as cancelling those notifications must.
pub(super) fn dismiss_notifications(
    runtime: &lingxia_platform::Platform,
    matches: impl Fn(&str, Option<&str>) -> bool,
) {
    let ids: Vec<String> = live_notifications()
        .iter()
        .filter(|(id, owner)| matches(id, owner.as_deref()))
        .map(|(id, _)| id.clone())
        .collect();
    for id in ids {
        if let Err(error) = runtime.banner_dismiss(&notification_banner_id(&id)) {
            log::warn!("dismissing notification banner failed: {error}");
        }
    }
}

async fn dismiss(ctx: JSContext, id: JSValue) -> JSResult<()> {
    let (invocation, id) =
        authorization::require_before_decode(&ctx, LogicRoute::AppBannerDismiss, || {
            let id = id.to_rust::<String>()?;
            if id.is_empty() {
                return Err(js_invalid_parameter_error(
                    "lx.host.banner.dismiss id must not be empty",
                ));
            }
            Ok(id)
        })?;
    let runtime = invocation.lxapp().runtime.clone();
    let id = AttentionScope::of(&invocation).platform_id(&id);
    let turn = call_order::take();
    spawn_blocking(move || call_order::in_turn(turn, || runtime.banner_dismiss(&id))).await
}

/// Banner calls reach the platform in the order the script made them.
///
/// Each call runs on its own blocking thread, so without this a
/// `show(prompt)` followed by `dismiss(prompt)` can let the dismiss run first,
/// find nothing, and leave the prompt waiting forever. A call takes its turn
/// synchronously, when the script makes it, and the platform step (queueing
/// a banner, or dismissing one) runs only when every earlier call has had its
/// step. Waiting for a banner's outcome happens after the turn is released.
mod call_order {
    use std::sync::{Condvar, Mutex};
    use std::time::Duration;

    /// A turn whose owner never shows up (its task was dropped before it
    /// ran) must not stall every later call.
    const MAX_WAIT: Duration = Duration::from_secs(2);

    struct Order {
        next: u64,
        serving: u64,
    }

    static ORDER: Mutex<Order> = Mutex::new(Order {
        next: 0,
        serving: 0,
    });
    static TURN: Condvar = Condvar::new();

    fn lock() -> std::sync::MutexGuard<'static, Order> {
        ORDER.lock().unwrap_or_else(|error| error.into_inner())
    }

    pub(super) fn take() -> u64 {
        let mut order = lock();
        let turn = order.next;
        order.next += 1;
        turn
    }

    pub(super) fn in_turn<R>(turn: u64, step: impl FnOnce() -> R) -> R {
        {
            let order = lock();
            let _ = TURN
                .wait_timeout_while(order, MAX_WAIT, |order| order.serving < turn)
                .unwrap_or_else(|error| error.into_inner());
        }
        struct Release(u64);
        impl Drop for Release {
            fn drop(&mut self) {
                let mut order = lock();
                order.serving = order.serving.max(self.0 + 1);
                TURN.notify_all();
            }
        }
        let _release = Release(turn);
        step()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::{Arc, Mutex as StdMutex};

        #[test]
        fn later_calls_wait_for_earlier_steps() {
            let first = take();
            let second = take();
            let seen = Arc::new(StdMutex::new(Vec::new()));
            let later = {
                let seen = Arc::clone(&seen);
                std::thread::spawn(move || in_turn(second, || seen.lock().unwrap().push(second)))
            };
            std::thread::sleep(Duration::from_millis(50));
            in_turn(first, || seen.lock().unwrap().push(first));
            later.join().unwrap();
            assert_eq!(*seen.lock().unwrap(), vec![first, second]);
        }
    }
}

async fn spawn_blocking<T, F>(work: F) -> JSResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, Error> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| {
            HostError::new(
                rong::error::E_INTERNAL,
                format!("banner task failed: {error}"),
            )
        })?
        .map_err(js_error)
}

type ActionTargets = Vec<(String, serde_json::Value)>;

fn decode_show(options: JSValue) -> JSResult<(DesktopBannerShow, ActionTargets)> {
    let parsed = options.to_rust::<JSShowOptions>()?;
    let title = parsed
        .title
        .filter(|title| !title.is_empty())
        .ok_or_else(|| js_invalid_parameter_error("lx.host.banner.show title is required"))?;
    let id = match parsed.id {
        Some(id) => validate_id(&id)?,
        None => uuid::Uuid::new_v4().to_string(),
    };
    let (actions, targets) = decode_actions(parsed.actions)?;
    let timeout_ms = match parsed.timeout_ms {
        None => None,
        Some(ms) if ms >= 0.0 => Some(ms as u64),
        Some(_) => {
            return Err(js_invalid_parameter_error(
                "lx.host.banner.show durationMs must be >= 0",
            ));
        }
    };
    let background = match parsed.background {
        None => DesktopBannerBackground::System,
        Some(value) => {
            DesktopBannerBackground::parse(&value).map_err(js_invalid_parameter_error)?
        }
    };
    Ok((
        DesktopBannerShow {
            id,
            title,
            body: parsed.body.unwrap_or_default(),
            actions,
            timeout_ms,
            background,
        },
        targets,
    ))
}

fn decode_actions(
    actions: Option<Vec<JSAction>>,
) -> JSResult<(Vec<DesktopBannerAction>, ActionTargets)> {
    let Some(actions) = actions else {
        return Ok((Vec::new(), Vec::new()));
    };
    if actions.len() > MAX_ACTIONS {
        return Err(js_invalid_parameter_error(
            "lx.host.banner.show accepts at most two actions",
        ));
    }
    let mut targets = Vec::new();
    let actions = actions
        .into_iter()
        .map(|action| {
            let id = action
                .id
                .filter(|id| !id.is_empty())
                .ok_or_else(|| js_invalid_parameter_error("banner action id is required"))?;
            let label = action
                .label
                .filter(|label| !label.is_empty())
                .ok_or_else(|| js_invalid_parameter_error("banner action label is required"))?;
            let style = match action.tone.as_deref() {
                None | Some("default") => DesktopBannerActionStyle::Default,
                Some("primary") => DesktopBannerActionStyle::Primary,
                Some("danger") => DesktopBannerActionStyle::Destructive,
                Some(other) => {
                    return Err(js_invalid_parameter_error(format!(
                        "banner action tone must be default, primary, or danger (got {other})"
                    )));
                }
            };
            if let Some(target) = action.target {
                let json = target.to_json_string().map_err(|error| {
                    js_invalid_parameter_error(format!(
                        "banner action target must be a plain object: {error}"
                    ))
                })?;
                let value = serde_json::from_str(&json).map_err(|error| {
                    js_invalid_parameter_error(format!("banner action target: {error}"))
                })?;
                targets.push((id.clone(), value));
            }
            Ok(DesktopBannerAction { id, label, style })
        })
        .collect::<JSResult<Vec<_>>>()?;
    Ok((actions, targets))
}

fn navigation_error(error: NavigationError) -> rong::RongJSError {
    match error {
        NavigationError::InvalidTarget(message) => {
            js_invalid_parameter_error(format!("lx.host.banner.show {message}"))
        }
        NavigationError::Unavailable(message) | NavigationError::Internal(message) => {
            HostError::new(rong::error::E_INTERNAL, message).into()
        }
    }
}

fn validate_id(id: &str) -> JSResult<String> {
    if id.is_empty() || id.chars().count() > MAX_ID_CHARS {
        return Err(js_invalid_parameter_error(format!(
            "lx.host.banner.show id must be 1–{MAX_ID_CHARS} characters"
        )));
    }
    Ok(id.to_string())
}

/// `id` is the caller's own id, not the namespaced one the platform held.
fn encode_outcome(ctx: &JSContext, id: &str, outcome: DesktopBannerOutcome) -> JSResult<JSObject> {
    let result = JSObject::new(ctx);
    result.set("id", id)?;
    match outcome {
        DesktopBannerOutcome::Action { action, .. } => {
            result.set("status", "ok")?;
            result.set("action", action)?;
        }
        other => {
            result.set("status", "canceled")?;
            result.set("reason", other.reason().unwrap_or("dismissed"))?;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::validate_id;

    #[test]
    fn id_rejects_empty_and_overlong() {
        assert!(validate_id("").is_err());
        assert!(validate_id(&"x".repeat(65)).is_err());
        assert!(validate_id("ok").is_ok());
    }
}
