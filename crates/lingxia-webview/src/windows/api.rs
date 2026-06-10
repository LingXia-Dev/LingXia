//! Public Windows API surface: window/panel entry points,
//! handler registries, and the exported layout/event types.

use super::*;

pub(crate) type CloseHandler = Arc<dyn Fn() + Send + Sync>;

pub(crate) type ChromeEventHandler = Arc<dyn Fn(WindowsChromeEvent) + Send + Sync>;

/// Handler for structured key input targeted at a native panel.
///
/// Returns `true` when the event was consumed (the window message is then
/// swallowed); `false` lets default window handling proceed.
pub type WindowsPanelInputHandler = Arc<dyn Fn(WindowsPanelKeyEvent) -> bool + Send + Sync>;

/// Structured key event forwarded to a native panel input handler.
///
/// `lingxia-webview` does not interpret keys (e.g. terminal escape
/// sequences); it only reports what the window received. `character` is set
/// for translated character input (`WM_CHAR`); for raw key-down input
/// (`WM_KEYDOWN`) it is `None` and `vk` carries the virtual-key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowsPanelKeyEvent {
    /// Virtual-key code for key-down events; `0` for character events.
    pub vk: u32,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Translated character for character events.
    pub character: Option<char>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowsChromeEvent {
    TabBarClick { index: usize },
    PanelActivatorClick { panel_id: String },
    NavigationBack,
    NavigationHome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowsTabBarPosition {
    #[default]
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsNavigationBarLayout {
    pub visible: bool,
    pub title: String,
    pub background_color: u32,
    pub text_color: u32,
    pub show_back_button: bool,
    pub show_home_button: bool,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsTabBarItemLayout {
    pub page_path: String,
    pub text: String,
    pub icon_path: String,
    pub selected_icon_path: String,
    pub badge: Option<String>,
    pub has_red_dot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsTabBarLayout {
    pub visible: bool,
    pub position: WindowsTabBarPosition,
    pub dimension: i32,
    pub app_name: String,
    pub color: u32,
    pub selected_color: u32,
    pub background_color: u32,
    pub border_color: u32,
    pub selected_index: i32,
    pub items: Vec<WindowsTabBarItemLayout>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowsPanelPosition {
    Left,
    #[default]
    Right,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsPanelActivatorLayout {
    pub id: String,
    pub label: String,
    pub icon_path: String,
    pub position: WindowsPanelPosition,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WindowsWindowLayout {
    pub navigation_bar: Option<WindowsNavigationBarLayout>,
    pub tab_bar: Option<WindowsTabBarLayout>,
    pub panel_activators: Vec<WindowsPanelActivatorLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsWebViewWindowSnapshot {
    pub window_id: usize,
    pub webtag_key: String,
    pub visible: bool,
    pub content_left: i32,
    pub content_top: i32,
    pub content_width: u32,
    pub content_height: u32,
}

pub(crate) static WINDOW_CLOSE_HANDLERS: OnceLock<Mutex<HashMap<String, CloseHandler>>> =
    OnceLock::new();

pub(crate) static WINDOW_CHROME_HANDLERS: OnceLock<Mutex<HashMap<String, ChromeEventHandler>>> =
    OnceLock::new();

pub(crate) static WINDOW_NATIVE_PANEL_INPUT_HANDLERS: OnceLock<
    Mutex<HashMap<String, WindowsPanelInputHandler>>,
> = OnceLock::new();

pub(crate) static WEBVIEW_USER_DATA_DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

pub fn show_webview_window(webtag: &WebTag, title: &str) -> StdResult<()> {
    show_webview_window_with_activation(webtag, title, true)
}

pub fn show_webview_window_inactive(webtag: &WebTag, title: &str) -> StdResult<()> {
    show_webview_window_with_activation(webtag, title, false)
}

pub fn show_webview_panel(webtag: &WebTag, title: &str, panel_id: &str) -> StdResult<()> {
    let webview = find_webview(webtag)
        .ok_or_else(|| WebViewError::WebView(format!("WebView not found for {}", webtag.key())))?;
    webview.inner.show_window(
        title.to_string(),
        true,
        WindowsWindowRole::Panel {
            panel_id: panel_id.to_string(),
        },
    )
}

pub(crate) fn show_webview_window_with_activation(
    webtag: &WebTag,
    title: &str,
    activate: bool,
) -> StdResult<()> {
    let webview = find_webview(webtag)
        .ok_or_else(|| WebViewError::WebView(format!("WebView not found for {}", webtag.key())))?;
    webview
        .inner
        .show_window(title.to_string(), activate, WindowsWindowRole::Main)
}

pub fn hide_webview_window(webtag: &WebTag) -> StdResult<()> {
    let webview = find_webview(webtag)
        .ok_or_else(|| WebViewError::WebView(format!("WebView not found for {}", webtag.key())))?;
    webview.inner.hide_window()
}

pub fn set_webview_window_layout(webtag: &WebTag, layout: WindowsWindowLayout) -> StdResult<()> {
    let webview = find_webview(webtag)
        .ok_or_else(|| WebViewError::WebView(format!("WebView not found for {}", webtag.key())))?;
    webview.inner.set_window_layout(layout)
}

pub fn webview_window_snapshot(webtag: &WebTag) -> StdResult<WindowsWebViewWindowSnapshot> {
    let webview = find_webview(webtag)
        .ok_or_else(|| WebViewError::WebView(format!("WebView not found for {}", webtag.key())))?;
    webview.inner.window_snapshot()
}

pub fn is_panel_visible(panel_id: &str) -> bool {
    active_group_key()
        .map(|group_key| {
            group_panels(&group_key)
                .into_iter()
                .any(|panel| panel.panel_id == panel_id)
        })
        .unwrap_or(false)
}

pub fn show_native_panel(
    panel_id: &str,
    title: &str,
    body: &str,
    position: WindowsPanelPosition,
) -> StdResult<()> {
    show_native_group_panel(panel_id, title, body, position, NativePanelKind::Text)
}

pub fn show_native_terminal_panel(
    panel_id: &str,
    title: &str,
    body: &str,
    position: WindowsPanelPosition,
) -> StdResult<()> {
    show_native_group_panel(panel_id, title, body, position, NativePanelKind::Terminal)
}

pub(crate) fn show_native_group_panel(
    panel_id: &str,
    title: &str,
    body: &str,
    position: WindowsPanelPosition,
    native_kind: NativePanelKind,
) -> StdResult<()> {
    let group_key = active_group_key()
        .ok_or_else(|| WebViewError::WebView("no active Windows shell group".to_string()))?;
    let Some(_host) = host_handle_for_group(&group_key) else {
        return Err(WebViewError::WebView(format!(
            "active Windows shell group has no host: {group_key}"
        )));
    };

    register_group_panel(
        &group_key,
        GroupPanel {
            webtag_key: native_panel_key(panel_id),
            panel_id: panel_id.to_string(),
            position,
            native_kind,
            native_title: Some(title.to_string()),
            native_body: Some(body.to_string()),
        },
    );
    if native_kind == NativePanelKind::Terminal {
        set_active_native_panel(Some(panel_id.to_string()));
    }
    layout_group_windows(&group_key);
    request_group_shell_refresh(&group_key);
    Ok(())
}

pub fn update_native_panel_body(panel_id: &str, body: &str) -> StdResult<()> {
    let Some(group_key) = update_group_panel_body(panel_id, body.to_string()) else {
        return Ok(());
    };
    request_group_shell_refresh(&group_key);
    Ok(())
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

#[inline]
pub fn hide_native_panel(panel_id: &str) -> StdResult<()> {
    hide_panel(panel_id)
}

pub fn hide_panel(panel_id: &str) -> StdResult<()> {
    let group_key = active_group_key()
        .ok_or_else(|| WebViewError::WebView("no active Windows shell group".to_string()))?;
    remove_group_panel_by_panel_id(&group_key, panel_id);
    layout_group_windows(&group_key);
    request_group_shell_refresh(&group_key);
    Ok(())
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
        };
        let thread_name = format!("lingxia-webview-chrome-{event_name}-{webtag_key}");
        let _ = std::thread::Builder::new()
            .name(thread_name)
            .spawn(move || handler(event));
        return true;
    }
    false
}
