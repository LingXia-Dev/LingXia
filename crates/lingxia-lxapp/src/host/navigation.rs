use super::{HostCancel, await_or_cancel};
use crate::LxApp;
use crate::{LxAppError, NavigationType};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Deserialize)]
struct PageTargetOptions {
    page: Option<String>,
    path: Option<String>,
    query: Option<Value>,
}

#[derive(Deserialize)]
struct NavigateBackOptions {
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

fn append_query(path: String, query: Option<&Value>) -> Result<String, LxAppError> {
    let Some(query) = query else {
        return Ok(path);
    };
    crate::append_page_query(path, query).map_err(LxAppError::InvalidParameter)
}

fn normalize_tabbar_path(url: &str) -> String {
    let (path, _) = crate::startup::split_path_query(url);
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
    cancel: &mut HostCancel,
) -> Result<(), LxAppError> {
    lxapp.ensure_page_exists(&target_url)?;

    let current_path = current_page_path(&lxapp)?;
    let target_page = lxapp.get_or_create_page(&target_url);

    let Some(page) = lxapp.get_page(&current_path) else {
        return Err(LxAppError::Runtime("Current page not found".to_string()));
    };

    let target_page = page.navigate_to(target_page, nav_type)?;

    if wait_ready {
        await_or_cancel(cancel, async {
            target_page
                .wait_webview_ready()
                .await
                .map_err(LxAppError::WebView)
        })
        .await?;
    }

    Ok(())
}

host_api_async!(
    NavigateTo,
    PageTargetOptions,
    (),
    |lxapp, options, cancel| async {
        let url = resolve_page_target(&lxapp, &options)?;
        navigate_with_url(lxapp, url, NavigationType::Forward, true, &mut cancel).await?;
        Ok(())
    }
);

host_api_async!(
    NavigateBack,
    NavigateBackOptions,
    (),
    |lxapp, options, cancel| async {
        let current_path = current_page_path(&lxapp)?;
        let Some(page) = lxapp.get_page(&current_path) else {
            return Err(LxAppError::Runtime("Current page not found".to_string()));
        };
        page.navigate_back(options.delta)?;

        // Best-effort wait for the destination page's WebView to be ready, so view callers can
        // await navigation completion and reliably receive errors.
        if let Some(dest_path) = lxapp.peek_current_page()
            && let Some(dest_page) = lxapp.get_page(&dest_path)
        {
            await_or_cancel(&mut cancel, async {
                dest_page
                    .wait_webview_ready()
                    .await
                    .map_err(LxAppError::WebView)
            })
            .await?;
        }
        Ok(())
    }
);

host_api_async!(
    RedirectTo,
    PageTargetOptions,
    (),
    |lxapp, options, cancel| async {
        let url = resolve_page_target(&lxapp, &options)?;
        if is_tabbar_page_url(&lxapp, &url) {
            return Err(LxAppError::UnsupportedOperation(
                "redirectTo cannot navigate to a tabBar page".to_string(),
            ));
        }
        navigate_with_url(lxapp, url, NavigationType::Replace, true, &mut cancel).await?;
        Ok(())
    }
);

host_api_async!(
    SwitchTab,
    PageTargetOptions,
    (),
    |lxapp, options, cancel| async {
        let url = resolve_page_target(&lxapp, &options)?;
        navigate_with_url(lxapp, url, NavigationType::SwitchTab, true, &mut cancel).await?;
        Ok(())
    }
);

host_api_async!(
    ReLaunch,
    PageTargetOptions,
    (),
    |lxapp, options, cancel| async {
        let url = resolve_page_target(&lxapp, &options)?;
        navigate_with_url(lxapp, url, NavigationType::Launch, true, &mut cancel).await?;
        Ok(())
    }
);

pub(crate) fn register_all() {
    register_host_module!("navigation", {
        "navigateTo" => Arc::new(NavigateTo),
        "navigateBack" => Arc::new(NavigateBack),
        "redirectTo" => Arc::new(RedirectTo),
        "switchTab" => Arc::new(SwitchTab),
        "reLaunch" => Arc::new(ReLaunch)
    });
}
