use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_lxapp_error, js_internal_error,
};
use crate::message_port;
use lxapp::lx;
use lxapp::{LxApp, LxAppError, NavigationType, startup};
use rong::{FromJSObj, JSContext, JSFunc, JSObject, JSResult};
use serde_json::Value;
use std::sync::Arc;

#[derive(FromJSObj)]
struct PageTargetOptions {
    page: Option<String>,
    path: Option<String>,
    query: Option<JSObject>,
}

#[derive(FromJSObj)]
struct NavigateBack {
    delta: u32,
}

fn current_page_path(lxapp: &LxApp) -> Result<String, LxAppError> {
    lxapp
        .peek_current_page()
        .ok_or_else(|| LxAppError::Runtime("No current page found".to_string()))
}

fn resolve_page_target(lxapp: &LxApp, options: &PageTargetOptions) -> Result<String, LxAppError> {
    let has_page = options
        .page
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    let has_path = options
        .path
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    match (has_page, has_path) {
        (true, true) => {
            return Err(LxAppError::InvalidParameter(
                "pass either page or path, not both".to_string(),
            ));
        }
        (false, false) => {
            return Err(LxAppError::InvalidParameter(
                "page or path is required".to_string(),
            ));
        }
        _ => {}
    }

    let path = if let Some(page) = options
        .page
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        lxapp
            .find_page_path_by_name(page)
            .ok_or_else(|| LxAppError::ResourceNotFound(format!("page name: {page}")))?
    } else {
        options
            .path
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_string()
    };

    append_query(path, options.query.as_ref())
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
        .list
        .iter()
        .any(|item| normalize_tabbar_path(&item.pagePath) == target)
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

/// Navigate to a new page (forward navigation)
async fn navigate_to(ctx: JSContext, options: PageTargetOptions) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let target_url =
        resolve_page_target(&lxapp, &options).map_err(|e| js_error_from_lxapp_error(&e))?;

    ensure_page_exists_js(&lxapp, &target_url)?;

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

/// Navigate back to previous page
fn navigate_back(ctx: JSContext, options: NavigateBack) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    navigate_back_impl(&lxapp, options.delta).map_err(|e| js_error_from_lxapp_error(&e))
}

/// Redirect to a new page (replace current page)
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

    let page_svc = lxapp
        .get_or_create_page_in_ctx(&ctx, &target_url)
        .await
        .map_err(|e| js_internal_error(format!("Failed to ensure target page svc: {}", e)))?;
    let _ = page_svc.clear_opener();

    navigate_with_url(lxapp.clone(), target_url, NavigationType::Replace, false)
        .await
        .map_err(|e| js_error_from_lxapp_error(&e))
}

/// Switch to a tab page
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

/// Relaunch to a new page (clear page stack)
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
    // Register navigation functions
    let navigate_to_func = JSFunc::new(ctx, navigate_to)?;
    lx::register_js_api(ctx, "navigateTo", navigate_to_func)?;

    let navigate_back_func = JSFunc::new(ctx, navigate_back)?;
    lx::register_js_api(ctx, "navigateBack", navigate_back_func)?;

    let redirect_to_func = JSFunc::new(ctx, redirect_to)?;
    lx::register_js_api(ctx, "redirectTo", redirect_to_func)?;

    let switch_tab_func = JSFunc::new(ctx, switch_tab)?;
    lx::register_js_api(ctx, "switchTab", switch_tab_func)?;

    let re_launch_func = JSFunc::new(ctx, re_launch)?;
    lx::register_js_api(ctx, "reLaunch", re_launch_func)?;

    Ok(())
}
