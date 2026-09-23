//! `NavDriver` — page-stack navigation + runtime reads for one selected lxapp.
//! Action verbs (`to` / `redirect` / `switchTab` / `relaunch`)
//! take a configured page name (+ optional `query`); reads are `current` /
//! `stack`; `back` pops. Semantics come from the shared `lxapp::automation`
//! lower half (tab-bar guard included), matching `lxdev lxapp nav`.
//!
//! Actions resolve once the page stack changed (`waitUntil: 'commit'`, the
//! default). `waitUntil: 'ready'` (host automation runs only) also waits for
//! the landed page's `onReady`.

use crate::auto_err;
use crate::resolve::{js_object_to_json, upgrade_authorized};
use lxapp::{LxApp, NavigationType, automation as auto};
use rong::{
    FromJSObject, HostError, IntoJSObject, JSContext, JSObject, JSResult, function::Optional,
    js_class, js_method,
};
use std::sync::{Arc, Weak};
use std::time::Duration;

/// Cap for `timeoutMs` on a ready wait; larger values clamp, like the other
/// automation waits.
const MAX_READY_TIMEOUT_MS: f64 = 60_000.0;

#[js_class(clone)]
pub(crate) struct JSNavDriver {
    lxapp: Weak<LxApp>,
}

impl JSNavDriver {
    pub(crate) fn new(lxapp: Weak<LxApp>) -> Self {
        Self { lxapp }
    }
}

#[derive(FromJSObject)]
struct JSBackOptions {
    delta: Option<u32>,
    #[js_name = "waitUntil"]
    wait_until: Option<String>,
    #[js_name = "timeoutMs"]
    timeout_ms: Option<f64>,
}

#[derive(FromJSObject)]
struct JSNavOptions {
    page: String,
    query: Option<JSObject>,
    #[js_name = "waitUntil"]
    wait_until: Option<String>,
    #[js_name = "timeoutMs"]
    timeout_ms: Option<f64>,
}

/// Parse `waitUntil` / `timeoutMs`. `None` means resolve on commit (the stack
/// changed); `Some` bounds the wait for the landed page's `onReady`.
fn parse_wait_until(
    wait_until: Option<&str>,
    timeout_ms: Option<f64>,
) -> Result<Option<Duration>, String> {
    match wait_until {
        None | Some("commit") => {
            if timeout_ms.is_some() {
                return Err("timeoutMs requires waitUntil: 'ready'".to_string());
            }
            Ok(None)
        }
        Some("ready") => match timeout_ms {
            None => Ok(Some(auto::PAGE_READY_TIMEOUT)),
            Some(ms) if ms.is_finite() && ms > 0.0 => Ok(Some(Duration::from_millis(
                ms.min(MAX_READY_TIMEOUT_MS) as u64,
            ))),
            Some(ms) => Err(format!("timeoutMs must be a positive number, got {ms}")),
        },
        Some(other) => Err(format!(
            "unsupported waitUntil '{other}'; expected 'commit' or 'ready'"
        )),
    }
}

/// [`parse_wait_until`] plus the host-only guard for a ready wait.
fn ready_wait(
    ctx: &JSContext,
    wait_until: Option<&str>,
    timeout_ms: Option<f64>,
) -> JSResult<Option<Duration>> {
    let wait = parse_wait_until(wait_until, timeout_ms).map_err(auto_err)?;
    // onReady is signalled from the app's Logic thread; lx.automation() in
    // that same Logic must not await it (see `lxapp::automation::navigate`).
    if wait.is_some() && crate::host_automation_authority(ctx).is_none() {
        return Err(auto_err(
            "waitUntil: 'ready' is only available in host automation runs",
        ));
    }
    Ok(wait)
}

#[derive(FromJSObject, Default)]
struct JSPageRef {
    page: Option<String>,
}

#[derive(Debug, Clone, IntoJSObject)]
struct JSPageInfo {
    path: String,
    name: Option<String>,
    /// The page instance: a page replaced by navigation (even to the same
    /// path) gets a new id.
    #[js_name = "instanceId"]
    instance_id: Option<String>,
    current: bool,
    #[js_name = "inStack"]
    in_stack: bool,
    /// `onReady` has been dispatched.
    ready: bool,
    #[js_name = "webviewAttached"]
    webview_attached: bool,
}

impl JSPageInfo {
    fn of(status: auto::PageStatus, page: &lxapp::PageInstance) -> Self {
        Self {
            instance_id: Some(page.instance_id_string()),
            ..status.into()
        }
    }
}

impl From<auto::PageStatus> for JSPageInfo {
    fn from(status: auto::PageStatus) -> Self {
        Self {
            path: status.path,
            name: status.name,
            instance_id: None,
            current: status.current,
            in_stack: status.in_stack,
            ready: status.ready,
            webview_attached: status.webview_attached,
        }
    }
}

impl JSNavDriver {
    async fn navigate(
        &self,
        ctx: &JSContext,
        options: &JSNavOptions,
        kind: NavigationType,
    ) -> JSResult<JSPageInfo> {
        let app = upgrade_authorized(ctx, &self.lxapp)?;
        let wait = ready_wait(ctx, options.wait_until.as_deref(), options.timeout_ms)?;
        let query = options.query.as_ref().map(js_object_to_json).transpose()?;
        let (page, name) = auto::navigate(&app, &options.page, query.as_ref(), kind, false)
            .await
            .map_err(auto_err)?;
        landed(&app, page, name, wait).await
    }
}

async fn landed(
    app: &Arc<LxApp>,
    page: lxapp::PageInstance,
    name: Option<String>,
    wait: Option<Duration>,
) -> JSResult<JSPageInfo> {
    if let Some(timeout) = wait {
        auto::wait_page_runtime_ready(app, &page, timeout)
            .await
            .map_err(|message| {
                crate::error::instance_error(
                    app,
                    &page,
                    name.as_deref(),
                    crate::error::code_for(&message),
                    message,
                )
            })?;
    }
    Ok(JSPageInfo::of(
        auto::page_status(app, &page, name.as_deref()),
        &page,
    ))
}

#[js_class(rename = "NavDriver")]
impl JSNavDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(rong::error::E_ILLEGAL_CONSTRUCTOR, "Use lx.automation()").into())
    }

    #[js_method]
    async fn to(&self, ctx: JSContext, options: JSNavOptions) -> JSResult<JSPageInfo> {
        self.navigate(&ctx, &options, NavigationType::Forward).await
    }

    #[js_method]
    async fn redirect(&self, ctx: JSContext, options: JSNavOptions) -> JSResult<JSPageInfo> {
        self.navigate(&ctx, &options, NavigationType::Replace).await
    }

    #[js_method(rename = "switchTab")]
    async fn switch_tab(&self, ctx: JSContext, options: JSNavOptions) -> JSResult<JSPageInfo> {
        self.navigate(&ctx, &options, NavigationType::SwitchTab)
            .await
    }

    #[js_method]
    async fn relaunch(&self, ctx: JSContext, options: JSNavOptions) -> JSResult<JSPageInfo> {
        self.navigate(&ctx, &options, NavigationType::Launch).await
    }

    #[js_method]
    async fn back(&self, ctx: JSContext, options: Optional<JSBackOptions>) -> JSResult<JSPageInfo> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let options = options.0;
        let wait = ready_wait(
            &ctx,
            options.as_ref().and_then(|o| o.wait_until.as_deref()),
            options.as_ref().and_then(|o| o.timeout_ms),
        )?;
        let delta = options.and_then(|o| o.delta).unwrap_or(1);
        let (page, name) = auto::navigate_back(&app, delta, false)
            .await
            .map_err(auto_err)?;
        landed(&app, page, name, wait).await
    }

    #[js_method]
    async fn current(&self, ctx: JSContext) -> JSResult<JSPageInfo> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let (page, name) = auto::resolve_page(&app, None).map_err(auto_err)?;
        Ok(JSPageInfo::of(
            auto::page_status(&app, &page, name.as_deref()),
            &page,
        ))
    }

    /// Status of a configured page by name (`lxdev lxapp page info --page`);
    /// omit `page` for the current page.
    #[js_method]
    async fn info(&self, ctx: JSContext, options: Optional<JSPageRef>) -> JSResult<JSPageInfo> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let options = options.0.unwrap_or_default();
        let (page, name) =
            auto::resolve_page(&app, options.page.as_deref()).map_err(|message| {
                crate::error::page_error(
                    &app,
                    options.page.as_deref(),
                    crate::error::code_for(&message),
                    message,
                )
            })?;
        Ok(JSPageInfo::of(
            auto::page_status(&app, &page, name.as_deref()),
            &page,
        ))
    }

    #[js_method]
    async fn stack(&self, ctx: JSContext) -> JSResult<Vec<JSPageInfo>> {
        let app = upgrade_authorized(&ctx, &self.lxapp)?;
        let info = app.runtime_info();
        let current = info.current_page.clone();
        let stack = info
            .page_stack
            .iter()
            .map(|path| {
                let page = app.get_page(path);
                let state = page.as_ref().map(|p| p.automation_state());
                JSPageInfo {
                    name: auto::page_name_for_path(&app, path),
                    instance_id: page.as_ref().map(|p| p.instance_id_string()),
                    current: current
                        .as_deref()
                        .is_some_and(|c| auto::page_paths_match(c, path)),
                    in_stack: true,
                    ready: state.as_ref().is_some_and(|s| s.ready),
                    webview_attached: state.as_ref().is_some_and(|s| s.webview_attached),
                    path: path.clone(),
                }
            })
            .collect();
        Ok(stack)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_until_defaults_to_commit_and_bounds_ready() {
        assert_eq!(parse_wait_until(None, None), Ok(None));
        assert_eq!(parse_wait_until(Some("commit"), None), Ok(None));
        assert_eq!(
            parse_wait_until(Some("ready"), None),
            Ok(Some(auto::PAGE_READY_TIMEOUT))
        );
        assert_eq!(
            parse_wait_until(Some("ready"), Some(60_000.0)),
            Ok(Some(Duration::from_secs(60)))
        );
        assert_eq!(
            parse_wait_until(Some("ready"), Some(300_000.0)),
            Ok(Some(Duration::from_secs(60)))
        );
        assert!(parse_wait_until(Some("ready"), Some(0.0)).is_err());
        assert!(parse_wait_until(Some("ready"), Some(f64::NAN)).is_err());
        assert!(parse_wait_until(None, Some(1_000.0)).is_err());
        assert!(parse_wait_until(Some("commit"), Some(1_000.0)).is_err());
        assert!(parse_wait_until(Some("load"), None).is_err());
    }
}
