//! Windows API handler registries and UI-thread callback dispatch.

use super::*;

pub(crate) type CloseHandler = Arc<dyn Fn() + Send + Sync>;

pub(crate) type ChromeEventHandler = Arc<dyn Fn(WindowsChromeEvent) + Send + Sync>;

pub(crate) static WINDOW_CLOSE_HANDLERS: OnceLock<Mutex<HashMap<String, CloseHandler>>> =
    OnceLock::new();

pub(crate) static WINDOW_CHROME_HANDLERS: OnceLock<Mutex<HashMap<String, ChromeEventHandler>>> =
    OnceLock::new();

pub(crate) static WINDOW_NATIVE_PANEL_INPUT_HANDLERS: OnceLock<
    Mutex<HashMap<String, WindowsPanelInputHandler>>,
> = OnceLock::new();

pub(crate) static WEBVIEW_USER_DATA_DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

/// Window message carrying a boxed callback posted by
/// [`post_to_window_thread`]; handled in the window procedure.
pub(crate) const WM_LINGXIA_RUN_CALLBACK: u32 = WM_APP + 0x158;

/// Runs `callback` on the UI thread that owns `window` (a window handle
/// previously surfaced to a product layer, e.g. via `WindowsChromeState`).
///
/// Generic thread-marshalling mechanics for product layers that must touch
/// Win32 UI owned by a webview UI thread (e.g. creating child controls)
/// from a background thread. Returns `false` when the window is gone or
/// the post failed; the callback is dropped in that case.
pub fn post_to_window_thread(window: isize, callback: Box<dyn FnOnce() + Send>) -> bool {
    if !is_window_handle_valid(window) {
        return false;
    }
    let raw = Box::into_raw(Box::new(callback));
    let posted = unsafe {
        WindowsAndMessaging::PostMessageW(
            Some(hwnd_from_handle(window)),
            WM_LINGXIA_RUN_CALLBACK,
            WPARAM(raw as usize),
            LPARAM(0),
        )
        .is_ok()
    };
    if !posted {
        // Reclaim the leaked box so the callback (and its captures) drop.
        drop(unsafe { Box::from_raw(raw) });
    }
    posted
}

/// Executes a callback delivered through [`WM_LINGXIA_RUN_CALLBACK`].
pub(crate) fn run_posted_window_callback(wparam: WPARAM) {
    let raw = wparam.0 as *mut Box<dyn FnOnce() + Send>;
    if raw.is_null() {
        return;
    }
    let callback = unsafe { Box::from_raw(raw) };
    callback();
}

pub fn set_native_panel_input_handler(panel_id: &str, handler: WindowsPanelInputHandler) {
    let handlers = WINDOW_NATIVE_PANEL_INPUT_HANDLERS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handlers) = handlers.lock() {
        handlers.insert(panel_id.to_string(), handler);
    }
}

pub fn clear_native_panel_input_handler(panel_id: &str) {
    if let Some(handlers) = WINDOW_NATIVE_PANEL_INPUT_HANDLERS.get()
        && let Ok(mut handlers) = handlers.lock()
    {
        handlers.remove(panel_id);
    }
    if active_native_panel().as_deref() == Some(panel_id) {
        set_active_native_panel(None);
    }
}

pub fn set_webview_close_handler(webtag: &WebTag, handler: CloseHandler) {
    let handlers = WINDOW_CLOSE_HANDLERS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handlers) = handlers.lock() {
        handlers.insert(webtag.key().to_string(), handler);
    }
}

pub fn set_webview_chrome_event_handler(webtag: &WebTag, handler: ChromeEventHandler) {
    let handlers = WINDOW_CHROME_HANDLERS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handlers) = handlers.lock() {
        handlers.insert(webtag.key().to_string(), handler);
    }
}

pub fn set_webview_user_data_dir(path: impl Into<PathBuf>) {
    let dir = path.into();
    let state = WEBVIEW_USER_DATA_DIR.get_or_init(|| Mutex::new(None));
    if let Ok(mut state) = state.lock() {
        *state = Some(dir);
    }
}

pub(crate) fn configured_webview_user_data_dir() -> Option<PathBuf> {
    WEBVIEW_USER_DATA_DIR
        .get()
        .and_then(|state| state.lock().ok())
        .and_then(|state| state.clone())
}

pub(crate) fn invoke_close_handler(webtag_key: &str) -> bool {
    let Some(handlers) = WINDOW_CLOSE_HANDLERS.get() else {
        return false;
    };

    // Clone the handler so it stays registered for subsequent WM_CLOSE messages;
    // the entry is removed in the window-cleanup/Drop path (cleanup_window_state).
    let handler = handlers
        .lock()
        .ok()
        .and_then(|handlers| handlers.get(webtag_key).cloned());
    if let Some(handler) = handler {
        let _ = std::thread::Builder::new()
            .name(format!("lingxia-webview-close-{}", webtag_key))
            .spawn(move || handler());
        return true;
    }
    false
}

pub(crate) fn remove_close_handler(webtag_key: &str) {
    if let Some(handlers) = WINDOW_CLOSE_HANDLERS.get()
        && let Ok(mut handlers) = handlers.lock()
    {
        handlers.remove(webtag_key);
    }
}

pub(crate) fn remove_chrome_event_handler(webtag_key: &str) {
    if let Some(handlers) = WINDOW_CHROME_HANDLERS.get()
        && let Ok(mut handlers) = handlers.lock()
    {
        handlers.remove(webtag_key);
    }
}

pub(crate) fn invoke_chrome_event_handler(webtag_key: &str, event: WindowsChromeEvent) -> bool {
    let Some(handlers) = WINDOW_CHROME_HANDLERS.get() else {
        return false;
    };

    let handler = handlers
        .lock()
        .ok()
        .and_then(|handlers| handlers.get(webtag_key).cloned());
    if let Some(handler) = handler {
        let event_name = match &event {
            WindowsChromeEvent::TabBarClick { .. } => "tabbar",
            WindowsChromeEvent::PanelActivatorClick { .. } => "panel-activator",
            WindowsChromeEvent::NavigationBack => "nav-back",
            WindowsChromeEvent::NavigationHome => "nav-home",
            WindowsChromeEvent::BrowserNewTabClick => "browser-new-tab",
            WindowsChromeEvent::BrowserTabClick { .. } => "browser-tab",
            WindowsChromeEvent::BrowserTabCloseClick { .. } => "browser-tab-close",
            WindowsChromeEvent::NativePanelTabClick { .. } => "panel-tab",
            WindowsChromeEvent::NativePanelTabCloseClick { .. } => "panel-tab-close",
            WindowsChromeEvent::NativePanelNewTabClick { .. } => "panel-new-tab",
            WindowsChromeEvent::NativePanelMaximizeClick { .. } => "panel-maximize",
            WindowsChromeEvent::NativePanelTabRenameRequest { .. } => "panel-tab-rename",
            WindowsChromeEvent::NativePanelRightClick { .. } => "panel-right-click",
            WindowsChromeEvent::BrowserNavBackClick => "browser-nav-back",
            WindowsChromeEvent::BrowserNavForwardClick => "browser-nav-forward",
            WindowsChromeEvent::BrowserNavReloadClick => "browser-nav-reload",
            WindowsChromeEvent::BrowserAddressBarClick => "browser-address-bar",
            WindowsChromeEvent::SidebarToggleClick => "sidebar-toggle",
            WindowsChromeEvent::SidebarGroupToggleClick { .. } => "sidebar-group-toggle",
            WindowsChromeEvent::SidebarActionClick { .. } => "sidebar-action",
        };
        let thread_name = format!("lingxia-webview-chrome-{event_name}-{webtag_key}");
        let _ = std::thread::Builder::new()
            .name(thread_name)
            .spawn(move || handler(event));
        return true;
    }
    false
}
