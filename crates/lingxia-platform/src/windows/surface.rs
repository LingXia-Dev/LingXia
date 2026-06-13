use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use lingxia_webview::WebTag;
use lingxia_webview::platform::windows::{WindowsWebViewHandler, find_webview_handler};
use lingxia_webview::runtime as webview_runtime;

use super::request_windows_app_exit;
use crate::traits::app_runtime::LxAppOpenMode;

static WINDOWS_SHOW_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static WINDOWS_SHOW_REQUESTS: LazyLock<Mutex<HashMap<String, u64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowsCloseAction {
    ExitApp,
    HideWindow,
}

pub(super) fn show_webtag_window(
    webtag: WebTag,
    title: String,
    activate: bool,
    open_mode: LxAppOpenMode,
    panel_id: String,
) {
    let request_key = show_request_key(&webtag, open_mode, &panel_id);
    let request_id = remember_show_request(&request_key);
    if let Some(handler) = find_webview_handler(&webtag) {
        if show_request_is_current(&request_key, request_id) {
            install_close_handler(&webtag, close_action_for_mode(open_mode));
            show_webview_handler_for_mode(handler, &title, activate, open_mode, &panel_id);
        }
        return;
    }

    let _ = thread::Builder::new()
        .name(format!("lingxia-windows-show-{}", webtag.key()))
        .spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if !show_request_is_current(&request_key, request_id) {
                    return;
                }
                if let Some(handler) = find_webview_handler(&webtag) {
                    install_close_handler(&webtag, close_action_for_mode(open_mode));
                    show_webview_handler_for_mode(handler, &title, activate, open_mode, &panel_id);
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            log::error!("Timed out waiting for Windows WebView {}", webtag.key());
        });
}

pub(super) fn hide_lxapp_window(appid: &str, session_id: u64) {
    // Invalidate any pending show request first so the polling waiter thread
    // cannot re-show the window after this hide.
    invalidate_show_request(&format!("main:{appid}#{session_id}"));
    for webtag in webview_runtime::list_webviews() {
        if webtag.extract_appid() == appid && webtag.session_id() == Some(session_id) {
            if let Some(handler) = find_webview_handler(&webtag) {
                let _ = handler.hide();
            }
        }
    }
}

fn show_webview_handler_for_mode(
    handler: WindowsWebViewHandler,
    title: &str,
    activate: bool,
    open_mode: LxAppOpenMode,
    panel_id: &str,
) {
    let result = match open_mode {
        LxAppOpenMode::Panel => handler.show_panel(title, panel_id),
        LxAppOpenMode::Normal if activate => handler.show_window(title),
        LxAppOpenMode::Normal => handler.show_window_inactive(title),
    };
    if let Err(err) = result {
        log::warn!(
            "Failed to show Windows WebView window {}: {}",
            handler.webtag().key(),
            err
        );
    }
}

fn show_request_key(webtag: &WebTag, open_mode: LxAppOpenMode, panel_id: &str) -> String {
    match open_mode {
        LxAppOpenMode::Normal => {
            format!(
                "main:{}#{}",
                webtag.extract_appid(),
                webtag
                    .session_id()
                    .map(|session| session.to_string())
                    .unwrap_or_else(|| "0".to_string())
            )
        }
        LxAppOpenMode::Panel => format!("panel:{panel_id}"),
    }
}

fn remember_show_request(key: &str) -> u64 {
    let request_id = WINDOWS_SHOW_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut requests) = WINDOWS_SHOW_REQUESTS.lock() {
        requests.insert(key.to_string(), request_id);
    }
    request_id
}

fn show_request_is_current(key: &str, request_id: u64) -> bool {
    WINDOWS_SHOW_REQUESTS
        .lock()
        .ok()
        .and_then(|requests| requests.get(key).copied())
        == Some(request_id)
}

fn invalidate_show_request(key: &str) {
    if let Ok(mut requests) = WINDOWS_SHOW_REQUESTS.lock() {
        requests.remove(key);
    }
}

fn close_action_for_mode(open_mode: LxAppOpenMode) -> WindowsCloseAction {
    match open_mode {
        LxAppOpenMode::Normal => WindowsCloseAction::ExitApp,
        LxAppOpenMode::Panel => WindowsCloseAction::HideWindow,
    }
}

fn install_close_handler(webtag: &WebTag, action: WindowsCloseAction) {
    let webtag_for_close = webtag.clone();
    lingxia_webview::platform::windows::lingxia_host::set_webview_close_handler(
        webtag,
        Arc::new(move || match action {
            WindowsCloseAction::ExitApp => request_windows_app_exit(),
            WindowsCloseAction::HideWindow => {
                if let Some(handler) = find_webview_handler(&webtag_for_close) {
                    let _ = handler.hide();
                }
            }
        }),
    );
}
