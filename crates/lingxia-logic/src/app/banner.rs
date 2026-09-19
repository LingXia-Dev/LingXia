use crate::authorization::{self, LogicRoute};
use crate::capability::is_control_app;
use crate::i18n::{js_error_from_platform_error, js_invalid_parameter_error};
use lingxia_platform::traits::app_runtime::{
    AppRuntime, DesktopBannerAction, DesktopBannerActionStyle, DesktopBannerBackground,
    DesktopBannerOutcome, DesktopBannerShow,
};
use rong::{FromJSObject, HostError, JSContext, JSFunc, JSObject, JSResult, JSValue};

const MAX_ID_CHARS: usize = 64;
const MAX_ACTIONS: usize = 2;

#[derive(Debug, Clone, Default, FromJSObject)]
#[ts_skip]
struct JSAction {
    id: Option<String>,
    label: Option<String>,
    style: Option<String>,
}

#[derive(Debug, Clone, Default, FromJSObject)]
#[ts_skip]
struct JSShowOptions {
    id: Option<String>,
    title: Option<String>,
    body: Option<String>,
    actions: Option<Vec<JSAction>>,
    #[js_name = "timeoutMs"]
    timeout_ms: Option<f64>,
    background: Option<String>,
}

/// `lx.app.banner` — Control-app desktop overlay. Absent on guests and
/// off desktop, so presence and `lx.supports({ capability: 'banner' })` agree.
pub(super) fn init(ctx: &JSContext, app: &JSObject) -> JSResult<()> {
    if !is_control_app(ctx) || !lingxia_platform::banner_supported() {
        return Ok(());
    }
    let banner = JSObject::new(ctx);
    banner.set("show", JSFunc::new(ctx, show)?.name("show")?)?;
    banner.set("dismiss", JSFunc::new(ctx, dismiss)?.name("dismiss")?)?;
    app.set("banner", banner)?;
    Ok(())
}

async fn show(ctx: JSContext, options: JSValue) -> JSResult<JSObject> {
    let (invocation, request) =
        authorization::require_before_decode(&ctx, LogicRoute::AppBannerShow, || {
            decode_show(options)
        })?;
    let runtime = invocation.lxapp().runtime.clone();
    let outcome = spawn_blocking(move || runtime.banner_show(&request)).await?;
    encode_outcome(&ctx, outcome)
}

async fn dismiss(ctx: JSContext, id: JSValue) -> JSResult<()> {
    let (invocation, id) =
        authorization::require_before_decode(&ctx, LogicRoute::AppBannerDismiss, || {
            let id = id.to_rust::<String>()?;
            if id.is_empty() {
                return Err(js_invalid_parameter_error(
                    "lx.app.banner.dismiss id must not be empty",
                ));
            }
            Ok(id)
        })?;
    let runtime = invocation.lxapp().runtime.clone();
    spawn_blocking(move || runtime.banner_dismiss(&id)).await
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
                format!("banner task failed: {error}"),
            )
        })?
        .map_err(|error| js_error_from_platform_error(&error))
}

fn decode_show(options: JSValue) -> JSResult<DesktopBannerShow> {
    let parsed = options.to_rust::<JSShowOptions>()?;
    let title = parsed
        .title
        .filter(|title| !title.is_empty())
        .ok_or_else(|| js_invalid_parameter_error("lx.app.banner.show title is required"))?;
    let id = match parsed.id {
        Some(id) => validate_id(&id)?,
        None => uuid::Uuid::new_v4().to_string(),
    };
    let actions = decode_actions(parsed.actions)?;
    let timeout_ms = match parsed.timeout_ms {
        None => None,
        Some(ms) if ms >= 0.0 => Some(ms as u64),
        Some(_) => {
            return Err(js_invalid_parameter_error(
                "lx.app.banner.show timeoutMs must be >= 0",
            ));
        }
    };
    let background = match parsed.background {
        None => DesktopBannerBackground::System,
        Some(value) => {
            DesktopBannerBackground::parse(&value).map_err(js_invalid_parameter_error)?
        }
    };
    Ok(DesktopBannerShow {
        id,
        title,
        body: parsed.body.unwrap_or_default(),
        actions,
        timeout_ms,
        background,
    })
}

fn decode_actions(actions: Option<Vec<JSAction>>) -> JSResult<Vec<DesktopBannerAction>> {
    let Some(actions) = actions else {
        return Ok(Vec::new());
    };
    if actions.len() > MAX_ACTIONS {
        return Err(js_invalid_parameter_error(
            "lx.app.banner.show accepts at most two actions",
        ));
    }
    actions
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
            let style = match action.style.as_deref() {
                None | Some("default") => DesktopBannerActionStyle::Default,
                Some("primary") => DesktopBannerActionStyle::Primary,
                Some("destructive") => DesktopBannerActionStyle::Destructive,
                Some(other) => {
                    return Err(js_invalid_parameter_error(format!(
                        "banner action style must be default, primary, or destructive (got {other})"
                    )));
                }
            };
            Ok(DesktopBannerAction { id, label, style })
        })
        .collect()
}

fn validate_id(id: &str) -> JSResult<String> {
    if id.is_empty() || id.chars().count() > MAX_ID_CHARS {
        return Err(js_invalid_parameter_error(format!(
            "lx.app.banner.show id must be 1–{MAX_ID_CHARS} characters"
        )));
    }
    Ok(id.to_string())
}

fn encode_outcome(ctx: &JSContext, outcome: DesktopBannerOutcome) -> JSResult<JSObject> {
    let result = JSObject::new(ctx);
    result.set("id", outcome.id())?;
    match outcome {
        DesktopBannerOutcome::Action { action, .. } => {
            result.set("canceled", false)?;
            result.set("action", action)?;
        }
        other => {
            result.set("canceled", true)?;
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
