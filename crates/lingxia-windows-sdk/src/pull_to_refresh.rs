use std::sync::Arc;

use lxapp::{LxAppDelegate, LxAppUiEventType};

pub(crate) fn install() {
    lingxia_platform::set_windows_pull_to_refresh_handler(Arc::new(|appid, path, refreshing| {
        handle_pull_to_refresh(appid, path, refreshing)
    }));
}

/// Start a pull-down refresh for (appid, path) programmatically — e.g. from the
/// lxapp right-click "Refresh" entry. Fires `onPullDownRefresh` like the gesture.
/// The shell's lxapp context-menu provider calls this, so it is gated to
/// shell-chrome to stay dead-code-clean in a `runtime`-only (no-shell) build.
#[cfg(feature = "shell-chrome")]
pub(crate) fn request_refresh(appid: &str, path: &str) {
    let _ = handle_pull_to_refresh(appid.to_string(), path.to_string(), true);
}

fn handle_pull_to_refresh(appid: String, path: String, refreshing: bool) -> bool {
    let Some(app) = lxapp::try_get(&appid) else {
        log::warn!("pull-to-refresh ignored: lxapp is not active: {appid}");
        return !refreshing;
    };
    let target_path = if path.trim().is_empty() {
        app.peek_current_page().unwrap_or_default()
    } else {
        path
    };
    if target_path.is_empty() {
        log::warn!("pull-to-refresh ignored: no current page for {appid}");
        return !refreshing;
    }
    let Ok(page) = app.require_page(&target_path) else {
        log::warn!("pull-to-refresh ignored: page is not alive: {appid}:{target_path}");
        return !refreshing;
    };
    let Some(webview) = page.webview() else {
        log::warn!("pull-to-refresh ignored: WebView is not ready: {appid}:{target_path}");
        return !refreshing;
    };

    if !crate::window_host::set_webview_pull_down_refreshing(&webview.webtag(), refreshing) {
        return !refreshing;
    }
    if refreshing {
        app.on_lxapp_event(LxAppUiEventType::PullDownRefresh, target_path)
    } else {
        true
    }
}
