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

/// F5 or Ctrl+R on a page that declared `enablePullDownRefresh` refreshes the
/// page instead of reloading the WebView. Returns whether the key was taken;
/// other pages and keys are left to WebView2.
pub(crate) fn handle_refresh_key(webtag: &WebTag, virtual_key: u32) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};
    let down = |key: u16| unsafe { GetKeyState(i32::from(key)) } < 0;
    let (control, shift, alt) = (down(VK_CONTROL.0), down(VK_SHIFT.0), down(VK_MENU.0));
    if !is_refresh_key(virtual_key, control, shift, alt) {
        return false;
    }
    let enabled = lxapp::find_page_by_webtag(&webtag.extract_appid(), webtag.key())
        .is_some_and(|page| page.is_pull_down_refresh_enabled());
    if !enabled {
        return false;
    }
    // A held key repeats; one refresh at a time. Keys arrive on the window
    // thread, where the start runs before this returns, so a repeat sees it.
    if !crate::window_host::is_pull_down_refreshing(webtag) {
        let _ = set_page_refreshing(webtag.key(), true);
    }
    true
}

/// Plain F5, or Ctrl+R: the browser refresh keys, minus their hard-reload
/// variants.
fn is_refresh_key(virtual_key: u32, control: bool, shift: bool, alt: bool) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_F5;
    if shift || alt {
        return false;
    }
    match virtual_key {
        key if key == u32::from(VK_F5.0) => !control,
        0x52 => control,
        _ => false,
    }
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

#[cfg(test)]
mod tests {
    use super::is_refresh_key;

    #[test]
    fn refresh_keys_are_f5_and_ctrl_r_only() {
        const F5: u32 = 0x74;
        const R: u32 = 0x52;
        assert!(is_refresh_key(F5, false, false, false));
        assert!(is_refresh_key(R, true, false, false));
        assert!(
            !is_refresh_key(R, false, false, false),
            "a bare R is typing"
        );
        assert!(
            !is_refresh_key(F5, true, false, false),
            "Ctrl+F5 is a hard reload"
        );
        assert!(
            !is_refresh_key(R, true, true, false),
            "Ctrl+Shift+R is a hard reload"
        );
        assert!(!is_refresh_key(F5, false, false, true));
        assert!(!is_refresh_key(0x74 + 1, false, false, false));
    }
}
