use lxapp::host_api;
use lxapp::lx;
use lxapp::{LxApp, LxAppError, NavigationType, startup};
use rong::{FromJSObj, JSContext, JSFunc, JSObject, JSResult, RongJSError, error::HostError};
use serde::Deserialize;
use std::sync::Arc;

#[derive(FromJSObj, Deserialize)]
struct NavigateTo {
    url: String,
}

#[derive(FromJSObj, Deserialize)]
struct NavigateBack {
    delta: u32,
}

#[derive(FromJSObj, Deserialize)]
struct RedirectTo {
    url: String,
}

#[derive(FromJSObj, Deserialize)]
struct SwitchTab {
    url: String,
}

#[derive(FromJSObj, Deserialize)]
struct ReLaunch {
    url: String,
}

fn current_page_path(lxapp: &LxApp) -> Result<String, LxAppError> {
    lxapp
        .peek_current_page()
        .ok_or_else(|| LxAppError::Runtime("No current page found".to_string()))
}

fn ensure_page_exists_js(lxapp: &LxApp, url: &str) -> JSResult<()> {
    lxapp.ensure_page_exists(url).map_err(|e| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            format!("Invalid page url: {}", e),
        ))
    })
}

fn normalize_tabbar_path(url: &str) -> String {
    let (path, _) = startup::split_path_query(url);
    let mut trimmed = path.trim_start_matches('/').to_string();
    if let Some(dot_pos) = trimmed.rfind('.') {
        if trimmed.rfind('/').map_or(true, |slash| dot_pos > slash) {
            trimmed.truncate(dot_pos);
        }
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
async fn navigate_to(ctx: JSContext, options: NavigateTo) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    ensure_page_exists_js(&lxapp, &options.url)?;

    // Ensure PageSvc for target page exists in this JSContext
    let page_svc = lxapp
        .get_or_create_page_in_ctx(&ctx, &options.url)
        .await
        .map_err(|e| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                format!("Failed to ensure target page svc: {}", e),
            ))
        })?;

    navigate_with_url(lxapp.clone(), options.url, NavigationType::Forward, false)
        .await
        .map_err(|e| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                format!("Failed to navigate: {}", e),
            ))
        })?;

    let response = JSObject::new(&ctx);
    response.set("eventEmitter", page_svc.get_event_emitter())?;

    Ok(response)
}

/// Navigate back to previous page
fn navigate_back(ctx: JSContext, options: NavigateBack) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    navigate_back_impl(&lxapp, options.delta).map_err(|e| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            format!("Failed to navigate back: {}", e),
        ))
    })
}

/// Redirect to a new page (replace current page)
async fn redirect_to(ctx: JSContext, options: RedirectTo) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    ensure_page_exists_js(&lxapp, &options.url)?;
    if is_tabbar_page_url(&lxapp, &options.url) {
        return Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "redirectTo cannot navigate to a tabBar page",
        )));
    }

    navigate_with_url(lxapp.clone(), options.url, NavigationType::Replace, false)
        .await
        .map_err(|e| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                format!("Failed to redirect: {}", e),
            ))
        })
}

/// Switch to a tab page
async fn switch_tab(ctx: JSContext, options: SwitchTab) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    ensure_page_exists_js(&lxapp, &options.url)?;

    let _page_svc = lxapp
        .get_or_create_page_in_ctx(&ctx, &options.url)
        .await
        .map_err(|e| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                format!("Failed to ensure target page svc: {}", e),
            ))
        })?;

    navigate_with_url(lxapp.clone(), options.url, NavigationType::SwitchTab, false)
        .await
        .map_err(|e| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                format!("Failed to switch tab: {}", e),
            ))
        })
}

/// Relaunch to a new page (clear page stack)
async fn re_launch(ctx: JSContext, options: ReLaunch) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    ensure_page_exists_js(&lxapp, &options.url)?;

    lxapp
        .get_or_create_page_in_ctx(&ctx, &options.url)
        .await
        .map_err(|e| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                format!("Failed to ensure target page svc: {}", e),
            ))
        })?;

    navigate_with_url(lxapp.clone(), options.url, NavigationType::Launch, false)
        .await
        .map_err(|e| {
            RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                format!("Failed to relaunch: {}", e),
            ))
        })
}

host_api!(NavigateToHost, NavigateTo, (), |lxapp: Arc<LxApp>,
                                           options: NavigateTo|
 -> Result<
    (),
    LxAppError,
> {
    lxapp.ensure_page_exists(&options.url)?;
    let url = options.url.clone();
    let lxapp_clone = lxapp.clone();

    let _ = rong::bg::spawn(async move {
        if let Err(err) =
            navigate_with_url(lxapp_clone, options.url, NavigationType::Forward, true).await
        {
            lxapp::warn!("navigateTo failed url={} err={}", url, err);
        }
    });

    Ok(())
});

host_api!(RedirectToHost, RedirectTo, (), |lxapp: Arc<LxApp>,
                                           options: RedirectTo|
 -> Result<
    (),
    LxAppError,
> {
    lxapp.ensure_page_exists(&options.url)?;
    if is_tabbar_page_url(&lxapp, &options.url) {
        return Err(LxAppError::UnsupportedOperation(
            "redirectTo cannot navigate to a tabBar page".to_string(),
        ));
    }
    let url = options.url.clone();
    let lxapp_clone = lxapp.clone();

    let _ = rong::bg::spawn(async move {
        if let Err(err) =
            navigate_with_url(lxapp_clone, options.url, NavigationType::Replace, true).await
        {
            lxapp::warn!("redirectTo failed url={} err={}", url, err);
        }
    });

    Ok(())
});

host_api!(SwitchTabHost, SwitchTab, (), |lxapp: Arc<LxApp>,
                                         options: SwitchTab|
 -> Result<(), LxAppError> {
    lxapp.ensure_page_exists(&options.url)?;
    let url = options.url.clone();
    let lxapp_clone = lxapp.clone();

    let _ = rong::bg::spawn(async move {
        if let Err(err) =
            navigate_with_url(lxapp_clone, options.url, NavigationType::SwitchTab, true).await
        {
            lxapp::warn!("switchTab failed url={} err={}", url, err);
        }
    });

    Ok(())
});

host_api!(ReLaunchHost, ReLaunch, (), |lxapp: Arc<LxApp>,
                                       options: ReLaunch|
 -> Result<(), LxAppError> {
    lxapp.ensure_page_exists(&options.url)?;
    let url = options.url.clone();
    let lxapp_clone = lxapp.clone();

    let _ = rong::bg::spawn(async move {
        if let Err(err) =
            navigate_with_url(lxapp_clone, options.url, NavigationType::Launch, true).await
        {
            lxapp::warn!("reLaunch failed url={} err={}", url, err);
        }
    });

    Ok(())
});

host_api!(
    NavigateBackHost,
    NavigateBack,
    (),
    |lxapp: Arc<LxApp>, options: NavigateBack| -> Result<(), LxAppError> {
        navigate_back_impl(&lxapp, options.delta)
    }
);

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

    lxapp::register_host("navigateTo", Arc::new(NavigateToHost));
    lxapp::register_host("navigateBack", Arc::new(NavigateBackHost));
    lxapp::register_host("redirectTo", Arc::new(RedirectToHost));
    lxapp::register_host("switchTab", Arc::new(SwitchTabHost));
    lxapp::register_host("reLaunch", Arc::new(ReLaunchHost));

    Ok(())
}
