//! Windows host UI contract shared by the Rust Windows SDK pieces.
//!
//! This crate intentionally contains no Win32 window implementation. The
//! implementation belongs to `lingxia-windows-sdk`.
//!
//! The crate is Windows-only; off-Windows it compiles to nothing so a
//! `cargo *(--workspace)` on other hosts neither pulls the `windows` crate
//! nor lints Win32 contracts that can't exist there.
#![cfg(windows)]

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
static ASIDE_PANEL_TABS: OnceLock<Mutex<HashMap<String, Vec<WindowsAsidePanelTab>>>> =
    OnceLock::new();
static ASIDE_PANEL_EVENT_HANDLER: OnceLock<Mutex<Option<WindowsAsidePanelEventHandler>>> =
    OnceLock::new();

/// One tab of a docked aside browser panel (grouped web-URL asides).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsAsidePanelTab {
    pub surface_id: String,
    pub title: String,
    pub active: bool,
}

/// Chrome events from a docked aside browser panel, routed back to the
/// surface layer that owns the grouped web asides.
#[derive(Debug, Clone)]
pub enum WindowsAsidePanelEvent {
    TabClick { surface_id: String },
    TabClose { surface_id: String },
    CloseAll,
    NavBack,
    NavForward,
    NavReload,
}

pub type WindowsAsidePanelEventHandler = Arc<dyn Fn(WindowsAsidePanelEvent) + Send + Sync>;

/// Stable panel id of the shared aside browser panel (one per window).
pub const ASIDE_BROWSER_PANEL_ID: &str = "lx.aside-browser";

/// Publishes the tab strip of an aside browser panel; an empty list removes
/// it (the panel then falls back to non-tabbed chrome).
pub fn set_aside_panel_tabs(panel_id: &str, tabs: Vec<WindowsAsidePanelTab>) {
    let registry = ASIDE_PANEL_TABS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut registry) = registry.lock() {
        if tabs.is_empty() {
            registry.remove(panel_id);
        } else {
            registry.insert(panel_id.to_string(), tabs);
        }
    }
}

pub fn aside_panel_tabs(panel_id: &str) -> Vec<WindowsAsidePanelTab> {
    ASIDE_PANEL_TABS
        .get()
        .and_then(|registry| registry.lock().ok())
        .and_then(|registry| registry.get(panel_id).cloned())
        .unwrap_or_default()
}

pub fn set_windows_aside_panel_event_handler(handler: WindowsAsidePanelEventHandler) {
    let slot = ASIDE_PANEL_EVENT_HANDLER.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(handler);
    }
}

/// Routes a chrome event to the aside-panel handler; `false` when none is
/// installed.
pub fn dispatch_windows_aside_panel_event(event: WindowsAsidePanelEvent) -> bool {
    let handler = ASIDE_PANEL_EVENT_HANDLER
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone());
    let Some(handler) = handler else {
        return false;
    };
    handler(event);
    true
}

fn unsupported_operation<T>(operation: &str) -> StdResult<T> {
    Err(WebViewError::WebView(format!(
        "Windows host backend does not support {operation}"
    )))
}

/// Host callbacks implemented by the window owner.
///
/// Every hook has a conservative default so a custom host can opt into only the
/// capabilities it actually orchestrates. For example, a host that wants the
/// SDK-managed native view components usually starts with
/// `find_webview_content_window` and `post_to_window_thread`, then adds panel or
/// shell integration as needed.
pub trait WindowsHostBackend: Send + Sync {
    fn show_webview_as_panel(
        &self,
        _webtag: &WebTag,
        _title: &str,
        _panel_id: &str,
    ) -> StdResult<()> {
        unsupported_operation("show_webview_as_panel")
    }

    fn show_webview_as_adaptive_panel(
        &self,
        _webtag: &WebTag,
        _title: &str,
        _panel_id: &str,
        _position: WindowsPanelPosition,
        _preferred_size: Option<i32>,
    ) -> StdResult<()> {
        unsupported_operation("show_webview_as_adaptive_panel")
    }

    fn present_webview_in_active_group(&self, _webtag: &WebTag) -> StdResult<()> {
        unsupported_operation("present_webview_in_active_group")
    }

    fn present_webview_as_group_main(&self, _webtag: &WebTag, _group_key: String) -> StdResult<()> {
        unsupported_operation("present_webview_as_group_main")
    }

    fn present_webview_as_overlay(
        &self,
        _webtag: &WebTag,
        _width: f64,
        _height: f64,
        _width_ratio: f64,
        _height_ratio: f64,
        _position: u8,
    ) -> StdResult<()> {
        unsupported_operation("present_webview_as_overlay")
    }

    fn resize_host_window_content(
        &self,
        _webtag: &WebTag,
        _width: i32,
        _height: i32,
    ) -> StdResult<()> {
        unsupported_operation("resize_host_window_content")
    }

    fn restore_presented_group_main(&self) -> StdResult<()> {
        unsupported_operation("restore_presented_group_main")
    }

    fn show_interactive_host_panel(
        &self,
        _panel_id: &str,
        _title: &str,
        _body: &str,
        _position: WindowsPanelPosition,
    ) -> StdResult<()> {
        unsupported_operation("show_interactive_host_panel")
    }

    fn hide_host_panel(&self, _panel_id: &str) -> StdResult<()> {
        unsupported_operation("hide_host_panel")
    }

    fn update_host_panel_body(&self, _panel_id: &str, _body: &str) -> StdResult<()> {
        unsupported_operation("update_host_panel_body")
    }

    fn set_host_panel_tabs(&self, _panel_id: &str, _tabs: Vec<WindowsHostPanelTab>) -> bool {
        false
    }

    fn set_host_panel_maximized(&self, _panel_id: &str, _maximized: bool) -> bool {
        false
    }

    fn invalidate_host_panel(&self, _panel_id: &str) -> bool {
        false
    }

    fn is_panel_visible(&self, _panel_id: &str) -> bool {
        false
    }

    fn find_webview_content_window(&self, _webtag: &WebTag) -> Option<WindowsWebViewContentWindow> {
        None
    }

    fn webview_window_snapshot(&self, _webtag: &WebTag) -> StdResult<WindowsWebViewWindowSnapshot> {
        unsupported_operation("webview_window_snapshot")
    }

    fn show_webview_window(
        &self,
        _webtag: &WebTag,
        _title: &str,
        _activate: bool,
    ) -> StdResult<()> {
        unsupported_operation("show_webview_window")
    }

    fn show_webview_window_with_content_size(
        &self,
        _webtag: &WebTag,
        _title: &str,
        _activate: bool,
        _width: Option<i32>,
        _height: Option<i32>,
    ) -> StdResult<()> {
        unsupported_operation("show_webview_window_with_content_size")
    }

    fn navigate_webview_window(
        &self,
        _webtag: &WebTag,
        _title: &str,
        _activate: bool,
    ) -> StdResult<()> {
        unsupported_operation("navigate_webview_window")
    }

    fn hide_webview_window(&self, _webtag: &WebTag) -> StdResult<()> {
        unsupported_operation("hide_webview_window")
    }

    fn request_host_window_layout(&self, _window: WindowsHostWindow) -> bool {
        false
    }

    fn active_content_screen_rect(&self) -> Option<WindowsContentRect> {
        None
    }

    fn post_to_window_thread(&self, _window: isize, _callback: Box<dyn FnOnce() + Send>) -> bool {
        false
    }

    fn sync_webview_window_layout(&self, _webtag: &WebTag) {}

    /// Repaints an aside panel's chrome after a tab-strip change that leaves
    /// the attached layout untouched (e.g. an inactive tab closed).
    fn refresh_aside_panel(&self, _panel_id: &str) {}
}

pub fn refresh_aside_panel(panel_id: &str) {
    if let Ok(backend) = backend() {
        backend.refresh_aside_panel(panel_id);
    }
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
    Top,
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
    pub webtag_key: String,
    pub title: String,
    pub rect: RECT,
    /// Top-band slice (aligned with the main navbar baseline) where a browser
    /// aside paints its address bar; `None` for panels with no band header.
    pub header_rect: Option<RECT>,
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
    /// Top-band slice for a browser aside's address bar (see
    /// [`WindowsChromePanel::header_rect`]); `None` when the panel has none.
    pub header_rect: Option<RECT>,
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
    /// Client-space cursor position while over this window's chrome; drives
    /// hover feedback (frame buttons keep their dedicated state above).
    pub cursor: Option<(i32, i32)>,
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
        /// Optional command invoked on left-button-down in addition to
        /// focusing the surface (e.g. focusing the terminal pane under the
        /// cursor). Carries the click's screen position when requested.
        click_command: Option<WindowsChromeCommand>,
    },
    Command(WindowsChromeCommand),
    CommandWithContext {
        command: WindowsChromeCommand,
        context_menu: WindowsChromeCommand,
    },
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

    /// Bounding rect of the hover-highlightable element under `point`; the
    /// host invalidates the rects the cursor enters/leaves so hover feedback
    /// repaints exactly the affected element.
    fn hover_rect(&self, state: &WindowsChromeState, point: (i32, i32)) -> Option<RECT> {
        let _ = (state, point);
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

pub fn show_webview_as_adaptive_panel(
    webtag: &WebTag,
    title: &str,
    panel_id: &str,
    position: WindowsPanelPosition,
    preferred_size: Option<i32>,
) -> StdResult<()> {
    backend()?.show_webview_as_adaptive_panel(webtag, title, panel_id, position, preferred_size)
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

pub fn show_webview_window_with_content_size(
    webtag: &WebTag,
    title: &str,
    activate: bool,
    width: Option<i32>,
    height: Option<i32>,
) -> StdResult<()> {
    backend()?.show_webview_window_with_content_size(webtag, title, activate, width, height)
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
