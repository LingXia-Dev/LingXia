//! Windows host UI contract shared by the Rust Windows SDK pieces.
//!
//! This crate intentionally contains no Win32 window implementation. The
//! implementation belongs to `lingxia-windows-sdk`.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use lingxia_webview::{WebTag, WebViewError};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::HDC;

type StdResult<T, E = WebViewError> = std::result::Result<T, E>;

pub type HostWindowCreatedHandler = Arc<dyn Fn(isize) + Send + Sync>;
pub type CloseHandler = Arc<dyn Fn() + Send + Sync>;
pub type ChromeEventHandler = Arc<dyn Fn(WindowsChromeCommand) + Send + Sync>;
pub type WebViewVisibilityHandler = Arc<dyn Fn(&WebTag, bool) + Send + Sync>;
pub type WindowsHostPanelInputHandler = Arc<dyn Fn(WindowsHostPanelKeyEvent) -> bool + Send + Sync>;

static DEFAULT_WINDOW_SIZE: OnceLock<(i32, i32)> = OnceLock::new();
static BACKEND: OnceLock<Arc<dyn WindowsHostBackend>> = OnceLock::new();
static CLOSE_HANDLERS: OnceLock<Mutex<HashMap<String, CloseHandler>>> = OnceLock::new();
static CHROME_HANDLERS: OnceLock<Mutex<HashMap<String, ChromeEventHandler>>> = OnceLock::new();
static VISIBILITY_HANDLER: OnceLock<Mutex<Option<WebViewVisibilityHandler>>> = OnceLock::new();
static HOST_WINDOW_CREATED_HANDLERS: OnceLock<Mutex<Vec<HostWindowCreatedHandler>>> =
    OnceLock::new();
static HOST_PANEL_INPUT_HANDLERS: OnceLock<Mutex<HashMap<String, WindowsHostPanelInputHandler>>> =
    OnceLock::new();
static WINDOW_LAYOUTS: OnceLock<Mutex<HashMap<String, WindowsWindowLayout>>> = OnceLock::new();
static WINDOWS_CHROME_RENDERER: OnceLock<Mutex<Option<Arc<dyn WindowsChromeRenderer>>>> =
    OnceLock::new();

pub trait WindowsHostBackend: Send + Sync {
    fn show_webview_as_panel(&self, webtag: &WebTag, title: &str, panel_id: &str) -> StdResult<()>;
    fn present_webview_in_active_group(&self, webtag: &WebTag) -> StdResult<()>;
    fn present_webview_as_group_main(&self, webtag: &WebTag, group_key: String) -> StdResult<()>;
    fn present_webview_as_overlay(
        &self,
        webtag: &WebTag,
        width: f64,
        height: f64,
        width_ratio: f64,
        height_ratio: f64,
        position: u8,
    ) -> StdResult<()>;
    fn resize_host_window_content(&self, webtag: &WebTag, width: i32, height: i32)
    -> StdResult<()>;
    fn restore_presented_group_main(&self) -> StdResult<()>;
    fn show_interactive_host_panel(
        &self,
        panel_id: &str,
        title: &str,
        body: &str,
        position: WindowsPanelPosition,
    ) -> StdResult<()>;
    fn hide_host_panel(&self, panel_id: &str) -> StdResult<()>;
    fn update_host_panel_body(&self, panel_id: &str, body: &str) -> StdResult<()>;
    fn set_host_panel_tabs(&self, panel_id: &str, tabs: Vec<WindowsHostPanelTab>) -> bool;
    fn set_host_panel_maximized(&self, panel_id: &str, maximized: bool) -> bool;
    fn invalidate_host_panel(&self, panel_id: &str) -> bool;
    fn is_panel_visible(&self, panel_id: &str) -> bool;
    fn find_webview_content_window(&self, webtag: &WebTag) -> Option<WindowsWebViewContentWindow>;
    fn webview_window_snapshot(&self, webtag: &WebTag) -> StdResult<WindowsWebViewWindowSnapshot>;
    fn show_webview_window(&self, webtag: &WebTag, title: &str, activate: bool) -> StdResult<()>;
    fn navigate_webview_window(
        &self,
        webtag: &WebTag,
        title: &str,
        activate: bool,
    ) -> StdResult<()>;
    fn hide_webview_window(&self, webtag: &WebTag) -> StdResult<()>;
    fn request_host_window_layout(&self, window: WindowsHostWindow) -> bool;
    fn active_content_screen_rect(&self) -> Option<WindowsContentRect>;
    fn post_to_window_thread(&self, window: isize, callback: Box<dyn FnOnce() + Send>) -> bool;
    fn sync_webview_window_layout(&self, webtag: &WebTag);
}

pub fn set_windows_host_backend(backend: Arc<dyn WindowsHostBackend>) {
    if BACKEND.set(backend).is_err() {
        log::warn!("Windows host backend is already installed; ignoring replacement");
    }
}

fn backend() -> StdResult<&'static Arc<dyn WindowsHostBackend>> {
    BACKEND
        .get()
        .ok_or_else(|| WebViewError::WebView("Windows host backend is not installed".to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsWebViewWindowSnapshot {
    pub window_id: usize,
    pub webtag_key: String,
    pub visible: bool,
    pub window_left: i32,
    pub window_top: i32,
    pub window_width: i32,
    pub window_height: i32,
    pub content_left: i32,
    pub content_top: i32,
    pub content_width: u32,
    pub content_height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowsWebViewContentWindow {
    pub window: isize,
    pub content_left: i32,
    pub content_top: i32,
    pub content_width: i32,
    pub content_height: i32,
    pub scale: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowsPanelPosition {
    Left,
    #[default]
    Right,
    Bottom,
}

#[derive(Clone, Default)]
pub struct WindowsWindowLayout {
    payload: Option<Arc<dyn Any + Send + Sync>>,
}

impl std::fmt::Debug for WindowsWindowLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsWindowLayout")
            .field("has_payload", &self.payload.is_some())
            .finish()
    }
}

impl WindowsWindowLayout {
    pub fn new<T>(payload: T) -> Self
    where
        T: Any + Send + Sync + 'static,
    {
        Self {
            payload: Some(Arc::new(payload)),
        }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.payload.is_none()
    }

    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: Any + 'static,
    {
        self.payload.as_deref()?.downcast_ref::<T>()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsHostPanelTab {
    pub id: u64,
    pub title: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsHostPanelContent {
    pub title: Option<String>,
    pub body: Option<String>,
    pub tabs: Vec<WindowsHostPanelTab>,
    pub maximized: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowsChromePanel {
    pub panel_id: String,
    pub rect: RECT,
    pub host_content: Option<WindowsHostPanelContent>,
    pub docked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsChromePanelLayoutInput {
    pub panel_id: String,
    pub webtag_key: String,
    pub position: WindowsPanelPosition,
    pub requested_size: Option<i32>,
    pub docked: bool,
    pub maximized: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowsChromePanelLayout {
    pub panel_id: String,
    pub webtag_key: String,
    pub rect: RECT,
    pub resize_handle: Option<RECT>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowsChromeAttachedLayout {
    pub main: RECT,
    pub panels: Vec<WindowsChromePanelLayout>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowsChromeAttachedState {
    pub main: RECT,
    pub panels: Vec<WindowsChromePanel>,
}

#[derive(Debug, Clone)]
pub struct WindowsChromeState {
    pub hwnd: HWND,
    pub client: RECT,
    pub layout: WindowsWindowLayout,
    pub attached: Option<WindowsChromeAttachedState>,
    pub frame_button_hover: Option<WindowsFrameButton>,
    pub frame_button_pressed: Option<WindowsFrameButton>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsFrameButton {
    Minimize,
    Maximize,
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowsChromeCommand {
    pub id: String,
    pub payload: serde_json::Value,
    pub focus: Option<String>,
    pub double_click: Option<Box<WindowsChromeCommand>>,
    pub include_screen_position: bool,
}

impl WindowsChromeCommand {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            payload: serde_json::Value::Null,
            focus: None,
            double_click: None,
            include_screen_position: false,
        }
    }

    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }

    pub fn with_focus(mut self, surface_id: impl Into<String>) -> Self {
        self.focus = Some(surface_id.into());
        self
    }

    pub fn with_double_click(mut self, command: WindowsChromeCommand) -> Self {
        self.double_click = Some(Box::new(command));
        self
    }

    pub fn with_screen_position(mut self) -> Self {
        self.include_screen_position = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowsChromeHit {
    Caption,
    FrameButton(WindowsFrameButton),
    Focusable {
        id: String,
        context_menu: Option<WindowsChromeCommand>,
    },
    Command(WindowsChromeCommand),
    Chrome,
}

pub trait WindowsChromeRenderer: Send + Sync {
    fn content_rect(&self, client: RECT, layout: &WindowsWindowLayout) -> RECT;
    fn panel_corner_radius(&self) -> i32;

    fn attached_layout(
        &self,
        client: RECT,
        layout: &WindowsWindowLayout,
        panels: &[WindowsChromePanelLayoutInput],
    ) -> Option<WindowsChromeAttachedLayout> {
        let _ = (client, layout, panels);
        None
    }

    fn paint(&self, hdc: HDC, state: &WindowsChromeState);

    fn paint_region(&self, hdc: HDC, state: &WindowsChromeState, invalid: RECT) {
        let _ = invalid;
        self.paint(hdc, state);
    }

    fn hit_test(&self, state: &WindowsChromeState, point: (i32, i32)) -> Option<WindowsChromeHit>;

    fn frame_button_rect(
        &self,
        state: &WindowsChromeState,
        button: WindowsFrameButton,
    ) -> Option<RECT> {
        let _ = (state, button);
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsHostPanelKeyEvent {
    pub vk: u32,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub character: Option<char>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsHostWindow {
    pub window: isize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsContentRect {
    pub host_window: isize,
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
    pub dpi: u32,
}

pub fn set_default_window_size(width: i32, height: i32) {
    if width > 0 && height > 0 {
        let _ = DEFAULT_WINDOW_SIZE.set((width, height));
    }
}

pub fn default_window_size() -> (i32, i32) {
    DEFAULT_WINDOW_SIZE.get().copied().unwrap_or((1024, 768))
}

pub fn set_windows_chrome_renderer(renderer: Arc<dyn WindowsChromeRenderer>) {
    let slot = WINDOWS_CHROME_RENDERER.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(renderer);
    }
}

pub fn windows_chrome_renderer() -> Option<Arc<dyn WindowsChromeRenderer>> {
    WINDOWS_CHROME_RENDERER
        .get()
        .and_then(|renderer| renderer.lock().ok())
        .and_then(|renderer| renderer.clone())
}

pub fn set_webview_close_handler(webtag: &WebTag, handler: CloseHandler) {
    let handlers = CLOSE_HANDLERS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handlers) = handlers.lock() {
        handlers.insert(webtag.key().to_string(), handler);
    }
}

pub fn webview_close_handler(webtag_key: &str) -> Option<CloseHandler> {
    CLOSE_HANDLERS
        .get()
        .and_then(|handlers| handlers.lock().ok())
        .and_then(|handlers| handlers.get(webtag_key).cloned())
}

pub fn set_webview_visibility_handler(handler: WebViewVisibilityHandler) {
    let slot = VISIBILITY_HANDLER.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(handler);
    }
}

pub fn webview_visibility_handler() -> Option<WebViewVisibilityHandler> {
    VISIBILITY_HANDLER
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone())
}

pub fn set_webview_chrome_event_handler(webtag: &WebTag, handler: ChromeEventHandler) {
    let handlers = CHROME_HANDLERS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handlers) = handlers.lock() {
        handlers.insert(webtag.key().to_string(), handler);
    }
}

pub fn webview_chrome_event_handler(webtag_key: &str) -> Option<ChromeEventHandler> {
    CHROME_HANDLERS
        .get()
        .and_then(|handlers| handlers.lock().ok())
        .and_then(|handlers| handlers.get(webtag_key).cloned())
}

pub fn add_host_window_created_handler(handler: HostWindowCreatedHandler) {
    let handlers = HOST_WINDOW_CREATED_HANDLERS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut handlers) = handlers.lock() {
        handlers.push(handler);
    }
}

pub fn host_window_created_handlers() -> Vec<HostWindowCreatedHandler> {
    HOST_WINDOW_CREATED_HANDLERS
        .get()
        .and_then(|state| state.lock().ok())
        .map(|state| state.clone())
        .unwrap_or_default()
}

pub fn set_host_panel_input_handler(panel_id: &str, handler: WindowsHostPanelInputHandler) {
    let handlers = HOST_PANEL_INPUT_HANDLERS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handlers) = handlers.lock() {
        handlers.insert(panel_id.to_string(), handler);
    }
}

pub fn clear_host_panel_input_handler(panel_id: &str) {
    if let Some(handlers) = HOST_PANEL_INPUT_HANDLERS.get()
        && let Ok(mut handlers) = handlers.lock()
    {
        handlers.remove(panel_id);
    }
}

pub fn host_panel_input_handler(panel_id: &str) -> Option<WindowsHostPanelInputHandler> {
    HOST_PANEL_INPUT_HANDLERS
        .get()
        .and_then(|handlers| handlers.lock().ok())
        .and_then(|handlers| handlers.get(panel_id).cloned())
}

pub fn set_webview_window_layout(webtag: &WebTag, layout: WindowsWindowLayout) -> StdResult<()> {
    let layouts = WINDOW_LAYOUTS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut layouts) = layouts.lock() {
        layouts.insert(webtag.key().to_string(), layout);
    }
    if let Ok(backend) = backend() {
        backend.sync_webview_window_layout(webtag);
    }
    Ok(())
}

pub fn current_window_layout(webtag_key: &str) -> WindowsWindowLayout {
    WINDOW_LAYOUTS
        .get()
        .and_then(|layouts| layouts.lock().ok())
        .and_then(|layouts| layouts.get(webtag_key).cloned())
        .unwrap_or_default()
}

pub fn cleanup_webview_state(webtag_key: &str) {
    if let Some(handlers) = CLOSE_HANDLERS.get()
        && let Ok(mut handlers) = handlers.lock()
    {
        handlers.remove(webtag_key);
    }
    if let Some(handlers) = CHROME_HANDLERS.get()
        && let Ok(mut handlers) = handlers.lock()
    {
        handlers.remove(webtag_key);
    }
    if let Some(layouts) = WINDOW_LAYOUTS.get()
        && let Ok(mut layouts) = layouts.lock()
    {
        layouts.remove(webtag_key);
    }
}

pub fn show_webview_as_panel(webtag: &WebTag, title: &str, panel_id: &str) -> StdResult<()> {
    backend()?.show_webview_as_panel(webtag, title, panel_id)
}

pub fn present_webview_in_active_group(webtag: &WebTag) -> StdResult<()> {
    backend()?.present_webview_in_active_group(webtag)
}

pub fn present_webview_as_group_main(webtag: &WebTag, group_key: String) -> StdResult<()> {
    backend()?.present_webview_as_group_main(webtag, group_key)
}

pub fn present_webview_as_overlay(
    webtag: &WebTag,
    width: f64,
    height: f64,
    width_ratio: f64,
    height_ratio: f64,
    position: u8,
) -> StdResult<()> {
    backend()?.present_webview_as_overlay(
        webtag,
        width,
        height,
        width_ratio,
        height_ratio,
        position,
    )
}

pub fn resize_host_window_content(webtag: &WebTag, width: i32, height: i32) -> StdResult<()> {
    backend()?.resize_host_window_content(webtag, width, height)
}

pub fn restore_presented_group_main() -> StdResult<()> {
    backend()?.restore_presented_group_main()
}

pub fn show_interactive_host_panel(
    panel_id: &str,
    title: &str,
    body: &str,
    position: WindowsPanelPosition,
) -> StdResult<()> {
    backend()?.show_interactive_host_panel(panel_id, title, body, position)
}

pub fn hide_host_panel(panel_id: &str) -> StdResult<()> {
    backend()?.hide_host_panel(panel_id)
}

pub fn update_host_panel_body(panel_id: &str, body: &str) -> StdResult<()> {
    backend()?.update_host_panel_body(panel_id, body)
}

pub fn set_host_panel_tabs(panel_id: &str, tabs: Vec<WindowsHostPanelTab>) -> bool {
    backend()
        .map(|backend| backend.set_host_panel_tabs(panel_id, tabs))
        .unwrap_or(false)
}

pub fn set_host_panel_maximized(panel_id: &str, maximized: bool) -> bool {
    backend()
        .map(|backend| backend.set_host_panel_maximized(panel_id, maximized))
        .unwrap_or(false)
}

pub fn invalidate_host_panel(panel_id: &str) -> bool {
    backend()
        .map(|backend| backend.invalidate_host_panel(panel_id))
        .unwrap_or(false)
}

pub fn is_panel_visible(panel_id: &str) -> bool {
    backend()
        .map(|backend| backend.is_panel_visible(panel_id))
        .unwrap_or(false)
}

pub fn find_host_window_for_webview(webtag: &WebTag) -> StdResult<WindowsHostWindow> {
    let content = find_webview_content_window(webtag).ok_or_else(|| {
        WebViewError::WebView(format!("no window registered for {}", webtag.key()))
    })?;
    Ok(WindowsHostWindow {
        window: content.window,
    })
}

pub fn request_host_window_layout(window: WindowsHostWindow) -> bool {
    backend()
        .map(|backend| backend.request_host_window_layout(window))
        .unwrap_or(false)
}

pub fn active_content_screen_rect() -> Option<WindowsContentRect> {
    backend()
        .ok()
        .and_then(|backend| backend.active_content_screen_rect())
}

pub fn find_webview_content_window(webtag: &WebTag) -> Option<WindowsWebViewContentWindow> {
    backend()
        .ok()
        .and_then(|backend| backend.find_webview_content_window(webtag))
}

pub fn webview_window_snapshot(webtag: &WebTag) -> StdResult<WindowsWebViewWindowSnapshot> {
    backend()?.webview_window_snapshot(webtag)
}

pub fn show_webview_window(webtag: &WebTag, title: &str, activate: bool) -> StdResult<()> {
    backend()?.show_webview_window(webtag, title, activate)
}

pub fn navigate_webview_window(webtag: &WebTag, title: &str, activate: bool) -> StdResult<()> {
    backend()?.navigate_webview_window(webtag, title, activate)
}

pub fn hide_webview_window(webtag: &WebTag) -> StdResult<()> {
    backend()?.hide_webview_window(webtag)
}

pub fn post_to_window_thread(window: isize, callback: Box<dyn FnOnce() + Send>) -> bool {
    backend()
        .map(|backend| backend.post_to_window_thread(window, callback))
        .unwrap_or(false)
}
