use lingxia_lxapp::{LxApp, NavigationType, lx};
use rong::{FromJSObj, JSContext, JSFunc, JSObject, JSResult, RongJSError};

#[derive(FromJSObj)]
struct NavigateTo {
    url: String,
}

#[derive(FromJSObj)]
struct NavigateBack {
    delta: u32,
}

#[derive(FromJSObj)]
struct RedirectTo {
    url: String,
}

#[derive(FromJSObj)]
struct SwitchTab {
    url: String,
}

/// Navigate to a new page (forward navigation)
async fn navigate_to(ctx: JSContext, options: NavigateTo) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Get current page from the page stack
    let current_path = lxapp
        .peek_current_page()
        .ok_or_else(|| RongJSError::Error("No current page found".to_string()))?;

    // Ensure PageSvc for target page exists in this JSContext
    let page_svc = lxapp
        .get_or_create_page_in_ctx(&ctx, &options.url)
        .await
        .map_err(|e| RongJSError::Error(format!("Failed to ensure target page svc: {}", e)))?;

    // Get the destination native Page from PageSvc
    let dest_page = page_svc.get_page();

    if let Some(page) = lxapp.get_page(&current_path) {
        page.navigate_to(dest_page, NavigationType::Forward)
            .map_err(|e| RongJSError::Error(format!("Failed to navigate: {}", e)))?;
    } else {
        return Err(RongJSError::Error("Current page not found".to_string()));
    }

    let response = JSObject::new(&ctx);
    response.set("eventEmitter", page_svc.get_event_emitter())?;

    Ok(response)
}

/// Navigate back to previous page
fn navigate_back(ctx: JSContext, options: NavigateBack) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Get current page from the page stack
    let current_path = lxapp
        .peek_current_page()
        .ok_or_else(|| RongJSError::Error("No current page found".to_string()))?;

    if let Some(page) = lxapp.get_page(&current_path) {
        page.navigate_back(options.delta)
            .map_err(|e| RongJSError::Error(format!("Failed to navigate back: {}", e)))?;
    } else {
        return Err(RongJSError::Error("Current page not found".to_string()));
    }

    Ok(())
}

/// Redirect to a new page (replace current page)
async fn redirect_to(ctx: JSContext, options: RedirectTo) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Get current page from the page stack
    let current_path = lxapp
        .peek_current_page()
        .ok_or_else(|| RongJSError::Error("No current page found".to_string()))?;

    let page_svc = lxapp
        .get_or_create_page_in_ctx(&ctx, &options.url)
        .await
        .map_err(|e| RongJSError::Error(format!("Failed to ensure target page svc: {}", e)))?;
    let target_page = page_svc.get_page();

    if let Some(page) = lxapp.get_page(&current_path) {
        page.navigate_to(target_page, NavigationType::Replace)
            .map_err(|e| RongJSError::Error(format!("Failed to redirect: {}", e)))?;
    } else {
        return Err(RongJSError::Error("Current page not found".to_string()));
    }

    Ok(())
}

/// Switch to a tab page
async fn switch_tab(ctx: JSContext, options: SwitchTab) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Get current page from the page stack
    let current_path = lxapp
        .peek_current_page()
        .ok_or_else(|| RongJSError::Error("No current page found".to_string()))?;

    let page_svc = lxapp
        .get_or_create_page_in_ctx(&ctx, &options.url)
        .await
        .map_err(|e| RongJSError::Error(format!("Failed to ensure target page svc: {}", e)))?;
    let target_page = page_svc.get_page();

    if let Some(page) = lxapp.get_page(&current_path) {
        page.navigate_to(target_page, NavigationType::SwitchTab)
            .map_err(|e| RongJSError::Error(format!("Failed to switch tab: {}", e)))?;
    } else {
        return Err(RongJSError::Error("Current page not found".to_string()));
    }

    Ok(())
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

    Ok(())
}
