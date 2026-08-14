use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_lxapp_error, js_internal_error,
};
use crate::message_port;
use lxapp::{LxApp, LxAppError, NavigationType, startup};
use rong::{FromJSObject, JSContext, JSObject, JSResult, function::Optional};
use serde_json::Value;
use std::sync::Arc;

#[derive(FromJSObject)]
#[ts_skip]
struct PageTargetOptions {
    page: Option<String>,
    // Kept only so legacy route input is rejected instead of ignored by the
    // object decoder. Public JS types expose `page` only.
    path: Option<String>,
    query: Option<JSObject>,
}

#[derive(FromJSObject)]
#[ts_skip]
struct NavigateBack {
    delta: Option<u32>,
}

fn navigate_back_delta(options: Option<NavigateBack>) -> u32 {
    options.and_then(|options| options.delta).unwrap_or(1)
}

fn current_page_path(lxapp: &LxApp) -> Result<String, LxAppError> {
    lxapp
        .peek_current_page_path()
        .ok_or_else(|| LxAppError::Runtime("No current page found".to_string()))
}

fn resolve_page_target(lxapp: &LxApp, options: &PageTargetOptions) -> Result<String, LxAppError> {
    let page = configured_page_name(options)?;
    let path = lxapp
        .find_page_path_by_name(page)
        .ok_or_else(|| LxAppError::ResourceNotFound(format!("page name: {page}")))?;

    append_query(path, options.query.as_ref())
}

fn configured_page_name(options: &PageTargetOptions) -> Result<&str, LxAppError> {
    if options.path.is_some() {
        return Err(LxAppError::InvalidParameter(
            "path is not supported; pass the configured page name in page".to_string(),
        ));
    }
    options
        .page
        .as_deref()
        .map(str::trim)
        .filter(|page| !page.is_empty())
        .ok_or_else(|| {
            LxAppError::InvalidParameter(
                "page must be a non-empty configured page name".to_string(),
            )
        })
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

async fn navigate_with_url(
    lxapp: Arc<LxApp>,
    target_url: String,
    nav_type: NavigationType,
    wait_ready: bool,
) -> Result<(), LxAppError> {
    let current_path = current_page_path(&lxapp)?;
    let target_page = lxapp.get_or_create_page(&target_url);

    if wait_ready && nav_type != NavigationType::Launch {
        target_page
            .wait_webview_ready()
            .await
            .map_err(LxAppError::WebView)?;
    }

    if let Some(page) = lxapp.get_page(&current_path) {
        let target_page = page.navigate_to(target_page, nav_type)?;
        if wait_ready && nav_type == NavigationType::Launch {
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

fn navigate_back_impl(lxapp: &LxApp, delta: u32) -> Result<(), LxAppError> {
    let current_path = current_page_path(lxapp)?;

    if let Some(page) = lxapp.get_page(&current_path) {
        page.navigate_back(delta)?;
        Ok(())
    } else {
        Err(LxAppError::Runtime("Current page not found".to_string()))
    }
}

/// Push a configured page onto the stack.
///
/// A route can appear on the stack only once. The promise rejects with
/// `data.reason === "duplicate_route"` when the target is already present,
/// or `data.reason === "stack_full"` when the ten-page limit is reached.
async fn navigate_to(ctx: JSContext, options: PageTargetOptions) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let target_url =
        resolve_page_target(&lxapp, &options).map_err(|e| js_error_from_lxapp_error(&e))?;

    ensure_page_exists_js(&lxapp, &target_url)?;
    // Reject before resolving the target: a rejected navigateTo must not
    // touch the query or opener of the page already on the stack.
    lxapp
        .validate_navigation_entry(&target_url, NavigationType::Forward)
        .map_err(|e| js_error_from_lxapp_error(&e))?;

    let page_svc = lxapp
        .get_or_create_page_in_ctx(&ctx, &target_url)
        .await
        .map_err(|e| js_internal_error(format!("Failed to ensure target page svc: {}", e)))?;
    let (opener_port, page_port) = message_port::pair(&ctx)?;
    page_svc
        .bind_opener(page_port)
        .map_err(|e| js_internal_error(format!("Failed to bind page opener: {}", e)))?;

    navigate_with_url(lxapp.clone(), target_url, NavigationType::Forward, false)
        .await
        .map_err(|e| js_error_from_lxapp_error(&e))?;

    Ok(opener_port)
}

/// Pop one or more pages and reveal the destination page.
///
/// `options` and `options.delta` are optional; both default to one page. The
/// promise resolves once the destination WebView is ready, so callers can
/// safely continue with work that targets the revealed page.
async fn navigate_back(ctx: JSContext, options: Optional<NavigateBack>) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let delta = navigate_back_delta(options.0);

    navigate_back_impl(&lxapp, delta).map_err(|e| js_error_from_lxapp_error(&e))?;
    if let Some(path) = lxapp.peek_current_page()
        && let Some(page) = lxapp.get_page(&path)
    {
        page.wait_webview_ready()
            .await
            .map_err(|error| js_error_from_lxapp_error(&LxAppError::WebView(error)))?;
    }
    Ok(())
}

/// Replace the current stack entry with a configured page.
///
/// Redirecting to the current route keeps its page instance and runs `onLoad`
/// again with the new query. Redirecting to a route lower in the stack rejects
/// with `data.reason === "duplicate_route"`.
async fn redirect_to(ctx: JSContext, options: PageTargetOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let target_url =
        resolve_page_target(&lxapp, &options).map_err(|e| js_error_from_lxapp_error(&e))?;

    ensure_page_exists_js(&lxapp, &target_url)?;
    if is_tabbar_page_url(&lxapp, &target_url) {
        return Err(js_error_from_business_code_with_detail(
            1002,
            "redirectTo cannot navigate to a tabBar page",
        ));
    }

    lxapp
        .validate_navigation_entry(&target_url, NavigationType::Replace)
        .map_err(|e| js_error_from_lxapp_error(&e))?;
    let page_svc = lxapp
        .get_or_create_page_in_ctx(&ctx, &target_url)
        .await
        .map_err(|e| js_internal_error(format!("Failed to ensure target page svc: {}", e)))?;
    let _ = page_svc.clear_opener();

    navigate_with_url(lxapp.clone(), target_url, NavigationType::Replace, false)
        .await
        .map_err(|e| js_error_from_lxapp_error(&e))
}

/// Switch to a configured tab page.
///
/// The tab page being left is hidden and retained. Non-tab pages pushed above
/// a tab leave the stack and receive `onUnload`.
async fn switch_tab(ctx: JSContext, options: PageTargetOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let target_url =
        resolve_page_target(&lxapp, &options).map_err(|e| js_error_from_lxapp_error(&e))?;

    ensure_page_exists_js(&lxapp, &target_url)?;

    let page_svc = lxapp
        .get_or_create_page_in_ctx(&ctx, &target_url)
        .await
        .map_err(|e| js_internal_error(format!("Failed to ensure target page svc: {}", e)))?;
    let _ = page_svc.clear_opener();

    navigate_with_url(lxapp.clone(), target_url, NavigationType::SwitchTab, false)
        .await
        .map_err(|e| js_error_from_lxapp_error(&e))
}

/// Clear the page stack and launch a configured page as the new root.
async fn re_launch(ctx: JSContext, options: PageTargetOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let target_url =
        resolve_page_target(&lxapp, &options).map_err(|e| js_error_from_lxapp_error(&e))?;

    ensure_page_exists_js(&lxapp, &target_url)?;

    let page_svc = lxapp
        .get_or_create_page_in_ctx(&ctx, &target_url)
        .await
        .map_err(|e| js_internal_error(format!("Failed to ensure target page svc: {}", e)))?;
    let _ = page_svc.clear_opener();

    navigate_with_url(lxapp.clone(), target_url, NavigationType::Launch, false)
        .await
        .map_err(|e| js_error_from_lxapp_error(&e))
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn navigateTo(
            ts_params = "options: NavigateToOptions",
            ts_return = "Promise<PageMessagePort>"
        ) = navigate_to;
        fn navigateBack(
            ts_params = "options?: NavigateBackOptions",
            ts_return = "Promise<void>"
        ) = navigate_back;
        fn redirectTo(ts_params = "options: RedirectToOptions") = redirect_to;
        fn switchTab(ts_params = "options: SwitchTabOptions") = switch_tab;
        fn reLaunch(ts_params = "options: ReLaunchOptions") = re_launch;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_navigation_rejects_route_paths() {
        let options = PageTargetOptions {
            page: None,
            path: Some("/pages/home/index".to_string()),
            query: None,
        };

        let error = configured_page_name(&options).unwrap_err().to_string();
        assert!(error.contains("path is not supported"));
    }

    #[test]
    fn page_navigation_requires_a_non_empty_page_name() {
        let options = PageTargetOptions {
            page: Some("  ".to_string()),
            path: None,
            query: None,
        };

        let error = configured_page_name(&options).unwrap_err().to_string();
        assert!(error.contains("page must be a non-empty configured page name"));
    }

    #[test]
    fn navigate_back_defaults_to_one_page() {
        assert_eq!(navigate_back_delta(None), 1);
        assert_eq!(navigate_back_delta(Some(NavigateBack { delta: None })), 1);
        assert_eq!(
            navigate_back_delta(Some(NavigateBack { delta: Some(3) })),
            3
        );
    }
}
