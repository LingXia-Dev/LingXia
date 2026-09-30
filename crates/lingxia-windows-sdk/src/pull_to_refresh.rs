use std::sync::Arc;

use lingxia_webview::WebTag;
use lxapp::{LxAppDelegate, LxAppUiEventType};

pub(crate) fn install() {
    lingxia_platform::set_windows_pull_to_refresh_handler(Arc::new(set_page_refreshing));
}

/// Start a pull-down refresh on the page a webtag names — e.g. from the lxapp
/// right-click "Refresh" entry. Fires `onPullDownRefresh` like the gesture.
/// The shell's lxapp context-menu provider calls this, so it is gated to
/// shell-chrome to stay dead-code-clean in a `runtime`-only (no-shell) build.
#[cfg(feature = "shell-chrome")]
pub(crate) fn request_refresh(webtag: &str) {
    let _ = set_page_refreshing(webtag, true);
}

fn set_page_refreshing(webtag: &str, refreshing: bool) -> bool {
    let tag = WebTag::from(webtag);
    if !refreshing {
        // The indicator is the WebView's, so it is put away whether or not
        // the page behind it is still alive.
        crate::window_host::set_webview_pull_down_refreshing(&tag, false);
        return true;
    }

    let appid = tag.extract_appid();
    let Some(app) = lxapp::try_get(&appid) else {
        log::warn!("pull-to-refresh ignored: lxapp is not active: {webtag}");
        return false;
    };
    if app
        .get_page_by_webtag(webtag)
        .and_then(|page| page.webview())
        .is_none()
    {
        log::warn!("pull-to-refresh ignored: page is not ready: {webtag}");
        return false;
    }
    if !crate::window_host::set_webview_pull_down_refreshing(&tag, true) {
        return false;
    }
    app.on_lxapp_event(LxAppUiEventType::PullDownRefresh, webtag.to_string())
}
