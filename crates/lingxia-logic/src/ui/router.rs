use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_lxapp_error, js_internal_error,
};
use crate::message_port;
use crate::navigator::{self, NavigateToAppOptions};
use futures::channel::oneshot;
use lxapp::{LxApp, LxAppError, NavigationType, PageSvc, startup};
use rong::{
    FromJSObject, JSContext, JSFunc, JSObject, JSResult, JSValue, Promise, function::Optional,
};
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

#[derive(FromJSObject)]
#[ts_skip]
struct NavTargetInput {
    page: Option<String>,
    tab: Option<String>,
    app: Option<String>,
    url: Option<String>,
    params: Option<JSObject>,
    channel: Option<String>,
    version: Option<String>,
}

const TARGET_KEYS: &[&str] = &["page", "tab", "app", "url", "params", "channel", "version"];

#[derive(FromJSObject)]
#[ts_skip]
struct GoOptions {
    replace: Option<bool>,
}

enum NavTarget {
    Page {
        name: String,
        params: Option<JSObject>,
    },
    Tab(String),
    App(NavigateToAppOptions),
    Url(String),
}

fn invalid(message: impl Into<String>) -> LxAppError {
    LxAppError::InvalidParameter(message.into())
}

fn js_invalid(message: impl Into<String>) -> rong::RongJSError {
    js_error_from_lxapp_error(&invalid(message))
}

fn nav_target(value: &JSValue) -> Result<NavTarget, LxAppError> {
    let Some(object) = value.clone().into_object() else {
        return Err(invalid("a target is { page }, { tab }, { app } or { url }"));
    };
    for key in object.keys_as::<String>().map_err(|e| invalid(e.to_string()))? {
        if !TARGET_KEYS.contains(&key.as_str()) {
            return Err(invalid(format!("unknown target key '{key}'")));
        }
    }
    let input = value
        .clone()
        .to_rust::<NavTargetInput>()
        .map_err(|e| invalid(e.to_string()))?;
    // With `app`, `page` names a page of that app.
    let page = if input.app.is_some() {
        &None
    } else {
        &input.page
    };
    let named = [page, &input.tab, &input.app, &input.url]
        .iter()
        .filter(|key| key.is_some())
        .count();
    if named != 1 {
        return Err(invalid(
            "a target names exactly one of page, tab, app or url",
        ));
    }
    let non_empty = |key: &str, value: String| {
        let value = value.trim().to_string();
        if value.is_empty() {
            Err(invalid(format!("{key} must not be empty")))
        } else {
            Ok(value)
        }
    };
    if input.app.is_none() && (input.channel.is_some() || input.version.is_some()) {
        return Err(invalid("channel and version apply to an { app } target"));
    }
    if let Some(app) = input.app {
        return Ok(NavTarget::App(NavigateToAppOptions {
            appid: non_empty("app", app)?,
            path: None,
            page: input.page.map(|page| non_empty("page", page)).transpose()?,
            query: input.params,
            channel: input.channel,
            target_version: input.version,
        }));
    }
    if let Some(name) = input.page {
        return Ok(NavTarget::Page {
            name: non_empty("page", name)?,
            params: input.params,
        });
    }
    if input.params.is_some() {
        return Err(invalid("params apply to a { page } or { app } target"));
    }
    match (input.tab, input.url) {
        (Some(name), _) => Ok(NavTarget::Tab(non_empty("tab", name)?)),
        (_, Some(url)) => Ok(NavTarget::Url(non_empty("url", url)?)),
        _ => unreachable!("exactly one key is named"),
    }
}

fn page_url(lxapp: &LxApp, name: &str, params: Option<&JSObject>) -> Result<String, LxAppError> {
    let path = lxapp
        .find_page_path_by_name(name)
        .ok_or_else(|| LxAppError::ResourceNotFound(format!("page name: {name}")))?;
    append_query(path, params)
}

fn append_query(path: String, query: Option<&JSObject>) -> Result<String, LxAppError> {
    let Some(query) = query else {
        return Ok(path);
    };
    let query_json = query.to_json_string().map_err(LxAppError::from)?;
    let query: Value = serde_json::from_str(&query_json)?;
    lxapp::append_page_query(path, &query).map_err(LxAppError::InvalidParameter)
}

fn ensure_page_exists_js(lxapp: &LxApp, url: &str) -> JSResult<()> {
    lxapp
        .ensure_page_exists(url)
        .map_err(|e| js_error_from_lxapp_error(&e))
}

fn normalize_tabbar_path(url: &str) -> String {
    let (path, _) = startup::split_path_query(url);
    let mut trimmed = path.trim_start_matches('/').to_string();
    if let Some(dot_pos) = trimmed.rfind('.')
        && trimmed.rfind('/').is_none_or(|slash| dot_pos > slash)
    {
        trimmed.truncate(dot_pos);
    }
    trimmed
}

fn is_tabbar_page_url(lxapp: &LxApp, url: &str) -> bool {
    let Some(tabbar) = lxapp.get_tabbar() else {
        return false;
    };
    let target = normalize_tabbar_path(url);
    tabbar
        .items
        .iter()
        .any(|item| normalize_tabbar_path(&item.page_path) == target)
}

fn current_page_path(lxapp: &LxApp) -> Result<String, LxAppError> {
    lxapp
        .peek_current_page_path()
        .ok_or_else(|| LxAppError::Runtime("No current page found".to_string()))
}

async fn navigate_with_url(
    lxapp: Arc<LxApp>,
    target_url: String,
    nav_type: NavigationType,
    wait_ready: bool,
) -> Result<(), LxAppError> {
    let current_path = current_page_path(&lxapp)?;

    if let Some(page) = lxapp.get_page(&current_path) {
        // Resolution belongs to navigate_to_url: pre-resolving here would
        // overwrite the query of a same-route instance already on the stack
        // and wait on the wrong instance's readiness.
        let target_page = page.navigate_to_url(&target_url, nav_type)?;
        if wait_ready {
            target_page
                .wait_webview_ready()
                .await
                .map_err(LxAppError::WebView)?;
        }
        Ok(())
    } else {
        Err(LxAppError::Runtime("Current page not found".to_string()))
    }
}

fn configured_page_url(lxapp: &LxApp, name: &str, params: Option<&JSObject>) -> JSResult<String> {
    let url = page_url(lxapp, name, params).map_err(|e| js_error_from_lxapp_error(&e))?;
    ensure_page_exists_js(lxapp, &url)?;
    Ok(url)
}

fn tab_url(lxapp: &LxApp, name: &str) -> JSResult<String> {
    let url = page_url(lxapp, name, None).map_err(|e| js_error_from_lxapp_error(&e))?;
    ensure_page_exists_js(lxapp, &url)?;
    if !is_tabbar_page_url(lxapp, &url) {
        return Err(js_invalid(format!(
            "'{name}' is not a tab; go to it with {{ page: '{name}' }}"
        )));
    }
    Ok(url)
}

/// Push a page and return its handle at once. `opened` settles when the page
/// is on screen and `result` when it leaves; a failed open rejects both. A tab
/// root pushes its one instance, refused while that instance is on the stack.
fn push(ctx: &JSContext, name: String, params: Option<JSObject>) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(ctx)?;
    let target_url = configured_page_url(&lxapp, &name, params.as_ref())?;
    // Refuse before resolving the target: a refused push must not touch the
    // query or opener of a page already on the stack.
    lxapp
        .validate_navigation_entry(&target_url, NavigationType::Forward)
        .map_err(|e| js_error_from_lxapp_error(&e))?;

    let (handle, page_port) = message_port::pair(ctx)?;
    let pushed: Rc<RefCell<Option<PageSvc>>> = Rc::default();
    let (result_tx, result_rx) = oneshot::channel::<JSResult<JSObject>>();

    let open_ctx = ctx.clone();
    let open_slot = pushed.clone();
    let opened = Promise::from_future(ctx, None, async move {
        let opened = async {
            let page_svc = lxapp
                .create_page_for_entry_in_ctx(&open_ctx, &target_url)
                .await
                .map_err(|e| js_internal_error(format!("Failed to create the page: {e}")))?;
            page_svc.bind_opener(page_port)?;
            let result = page_svc.bind_result()?;
            *open_slot.borrow_mut() = Some(page_svc);
            navigate_with_url(lxapp.clone(), target_url, NavigationType::Forward, true)
                .await
                .map_err(|e| js_error_from_lxapp_error(&e))?;
            Ok::<_, rong::RongJSError>(result)
        }
        .await;
        match opened {
            Ok(result) => {
                let _ = result_tx.send(Ok(result));
                Ok(())
            }
            Err(error) => {
                let _ = result_tx.send(Err(error.clone()));
                Err(error)
            }
        }
    })?;
    let result = Promise::from_future(ctx, None, async move {
        match result_rx.await {
            Ok(result) => result,
            Err(_) => Err(js_internal_error("the page was never opened")),
        }
    })?;

    let close = JSFunc::new(ctx, move || -> JSResult<()> {
        let page_svc = pushed.borrow().clone().ok_or_else(|| {
            js_error_from_lxapp_error(&LxAppError::UnsupportedOperation(
                "the page has not opened yet; await handle.opened first".to_string(),
            ))
        })?;
        page_svc
            .leave_page()
            .map_err(|e| js_error_from_lxapp_error(&e))
    })?
    .name("close")?;

    handle.set("opened", opened)?;
    handle.set("result", result)?;
    handle.set("close", close)?;
    Ok(handle)
}

/// Replace the current page. The new page takes over the opener and the
/// result it is owed; replacing a page with its own route reloads it.
async fn replace(ctx: &JSContext, name: String, params: Option<JSObject>) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(ctx)?;
    let target_url = configured_page_url(&lxapp, &name, params.as_ref())?;
    if is_tabbar_page_url(&lxapp, &target_url) {
        return Err(js_error_from_business_code_with_detail(
            1002,
            &format!("a tab cannot replace a page; go to it with {{ tab: '{name}' }}"),
        ));
    }
    let current = lxapp.current_page_svc_in_ctx(ctx).await?;
    let to_self = current_page_path(&lxapp)
        .ok()
        .is_some_and(|path| lxapp.resolve_entry_path(&target_url) == path);
    if !to_self {
        let page_svc = lxapp
            .create_page_for_entry_in_ctx(ctx, &target_url)
            .await
            .map_err(|e| js_internal_error(format!("Failed to create the page: {e}")))?;
        current.hand_over_opener(&page_svc)?;
    }
    navigate_with_url(lxapp.clone(), target_url, NavigationType::Replace, false)
        .await
        .map_err(|e| js_error_from_lxapp_error(&e))
}

async fn switch_tab(ctx: &JSContext, name: String) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(ctx)?;
    let target_url = tab_url(&lxapp, &name)?;
    let page_svc = lxapp.get_or_create_page_in_ctx(ctx, &target_url).await?;
    let _ = page_svc.clear_opener();
    navigate_with_url(lxapp.clone(), target_url, NavigationType::SwitchTab, false)
        .await
        .map_err(|e| js_error_from_lxapp_error(&e))
}

/// `lx.nav.go`: pushing a `{ page }` returns its handle at once; every other
/// move returns a promise.
fn go(ctx: JSContext, target: JSValue, options: Optional<GoOptions>) -> JSResult<JSObject> {
    let target = nav_target(&target).map_err(|e| js_error_from_lxapp_error(&e))?;
    let replace_current = options.0.and_then(|options| options.replace) == Some(true);
    if replace_current && !matches!(target, NavTarget::Page { .. }) {
        return Err(js_invalid("replace applies to a { page } target"));
    }
    let task_ctx = ctx.clone();
    let promise = match target {
        NavTarget::Page { name, params } if !replace_current => return push(&ctx, name, params),
        NavTarget::Page { name, params } => {
            Promise::from_future(
                &ctx,
                None,
                async move { replace(&task_ctx, name, params).await },
            )?
        }
        NavTarget::Tab(name) => {
            Promise::from_future(&ctx, None, async move { switch_tab(&task_ctx, name).await })?
        }
        NavTarget::App(options) => Promise::from_future(&ctx, None, async move {
            navigator::navigate_to_app(task_ctx, options).await
        })?,
        NavTarget::Url(url) => Promise::from_future(&ctx, None, async move {
            crate::surface::open_url(task_ctx, url, Optional(None))
                .await
                .map(|_| ())
        })?,
    };
    Ok(promise.into_object())
}

/// Pop `n` pages (default one), stopping at the root. Resolves once the
/// revealed page is ready.
async fn back(ctx: JSContext, n: Optional<f64>) -> JSResult<()> {
    let n = n.0.unwrap_or(1.0);
    if !(n.is_finite() && n >= 1.0 && n.fract() == 0.0) {
        return Err(js_invalid(
            "back(n) takes a whole number of pages, at least 1",
        ));
    }
    let lxapp = LxApp::from_ctx(&ctx)?;
    let depth = lxapp.get_page_stack().len().saturating_sub(1);
    let delta = (n as usize).min(depth);
    if delta == 0 {
        return Ok(());
    }
    let current_path = current_page_path(&lxapp).map_err(|e| js_error_from_lxapp_error(&e))?;
    let page = lxapp.get_page(&current_path).ok_or_else(|| {
        js_error_from_lxapp_error(&LxAppError::Runtime("Current page not found".into()))
    })?;
    page.navigate_back(delta as u32)
        .map_err(|e| js_error_from_lxapp_error(&e))?;
    if let Some(path) = lxapp.peek_current_page()
        && let Some(page) = lxapp.get_page(&path)
    {
        page.wait_webview_ready()
            .await
            .map_err(|error| js_error_from_lxapp_error(&LxAppError::WebView(error)))?;
    }
    Ok(())
}

/// Clear the stack and start again at a page or tab. Every discarded page's
/// result settles canceled.
async fn reset(ctx: JSContext, target: JSValue) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let target_url = match nav_target(&target).map_err(|e| js_error_from_lxapp_error(&e))? {
        NavTarget::Page { name, params } => configured_page_url(&lxapp, &name, params.as_ref())?,
        NavTarget::Tab(name) => tab_url(&lxapp, &name)?,
        NavTarget::App(_) | NavTarget::Url(_) => {
            return Err(js_invalid("reset takes a { page } or { tab } target"));
        }
    };
    lxapp
        .relaunch(&target_url)
        .map(|_| ())
        .map_err(|e| js_error_from_lxapp_error(&e))
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_nav_property(ctx)
}

// Typed by hand in the prelude: `go`'s return depends on its target, which
// js_api cannot express.
fn nav_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let namespace = JSObject::new(ctx);
    namespace.set("go", JSFunc::new(ctx, go)?.name("go")?)?;
    namespace.set("back", JSFunc::new(ctx, back)?.name("back")?)?;
    namespace.set("reset", JSFunc::new(ctx, reset)?.name("reset")?)?;
    namespace.set(
        "leaveApp",
        JSFunc::new(ctx, navigator::navigate_back_app)?.name("leaveApp")?,
    )?;
    Ok(namespace)
}

rong::js_api! {
    fn register_nav_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const nav: "NavApi" = nav_namespace(ctx)?;
    }
}
