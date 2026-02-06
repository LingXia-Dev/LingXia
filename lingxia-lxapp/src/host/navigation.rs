use super::{HostCancel, await_or_cancel, register_host};
use crate::LxApp;
use crate::{LxAppError, NavigationType};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct NavigateToOptions {
    url: String,
}

#[derive(Deserialize)]
struct NavigateBackOptions {
    delta: u32,
}

#[derive(Deserialize)]
struct RedirectToOptions {
    url: String,
}

#[derive(Deserialize)]
struct SwitchTabOptions {
    url: String,
}

#[derive(Deserialize)]
struct ReLaunchOptions {
    url: String,
}

fn current_page_path(lxapp: &LxApp) -> Result<String, LxAppError> {
    lxapp
        .peek_current_page()
        .ok_or_else(|| LxAppError::Runtime("No current page found".to_string()))
}

fn normalize_tabbar_path(url: &str) -> String {
    let (path, _) = crate::startup::split_path_query(url);
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
    cancel: &mut HostCancel,
) -> Result<(), LxAppError> {
    lxapp.ensure_page_exists(&target_url)?;

    let current_path = current_page_path(&lxapp)?;
    let target_page = lxapp.get_or_create_page(&target_url);

    if wait_ready && nav_type != NavigationType::Launch {
        await_or_cancel(cancel, async {
            target_page
                .wait_webview_ready()
                .await
                .map_err(LxAppError::WebView)
        })
        .await?;
    }

    let Some(page) = lxapp.get_page(&current_path) else {
        return Err(LxAppError::Runtime("Current page not found".to_string()));
    };

    let target_page = page.navigate_to(target_page, nav_type)?;

    if wait_ready && nav_type == NavigationType::Launch {
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
    NavigateToOptions,
    (),
    |lxapp, options, cancel| async {
        navigate_with_url(
            lxapp,
            options.url,
            NavigationType::Forward,
            true,
            &mut cancel,
        )
        .await?;
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
        if let Some(dest_path) = lxapp.peek_current_page() {
            if let Some(dest_page) = lxapp.get_page(&dest_path) {
                await_or_cancel(&mut cancel, async {
                    dest_page
                        .wait_webview_ready()
                        .await
                        .map_err(LxAppError::WebView)
                })
                .await?;
            }
        }
        Ok(())
    }
);

host_api_async!(
    RedirectTo,
    RedirectToOptions,
    (),
    |lxapp, options, cancel| async {
        if is_tabbar_page_url(&lxapp, &options.url) {
            return Err(LxAppError::UnsupportedOperation(
                "redirectTo cannot navigate to a tabBar page".to_string(),
            ));
        }
        navigate_with_url(
            lxapp,
            options.url,
            NavigationType::Replace,
            true,
            &mut cancel,
        )
        .await?;
        Ok(())
    }
);

host_api_async!(
    SwitchTab,
    SwitchTabOptions,
    (),
    |lxapp, options, cancel| async {
        navigate_with_url(
            lxapp,
            options.url,
            NavigationType::SwitchTab,
            true,
            &mut cancel,
        )
        .await?;
        Ok(())
    }
);

host_api_async!(
    ReLaunch,
    ReLaunchOptions,
    (),
    |lxapp, options, cancel| async {
        navigate_with_url(
            lxapp,
            options.url,
            NavigationType::Launch,
            true,
            &mut cancel,
        )
        .await?;
        Ok(())
    }
);

pub(crate) fn register_all() {
    register_host("navigateTo", Arc::new(NavigateTo));
    register_host("navigateBack", Arc::new(NavigateBack));
    register_host("redirectTo", Arc::new(RedirectTo));
    register_host("switchTab", Arc::new(SwitchTab));
    register_host("reLaunch", Arc::new(ReLaunch));
}
