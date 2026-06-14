//! Windows host-window API owned by the Windows SDK layer.
//!
//! `lingxia-webview` provides WebView2 surface creation and lookup only. This
//! module owns LingXia Windows host concepts: chrome layout payloads, panels,
//! host callbacks, and presentation policy around WebView surfaces.

use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::sync::{Arc, Mutex, OnceLock};

pub use lingxia_webview::platform::windows::{
    WindowsWebViewHandler, WindowsWebViewNativeView, WindowsWebViewNativeViewHost,
    find_webview_handler, set_webview_devtools_enabled, set_webview_native_view_host,
    set_webview_user_data_dir,
};
use lingxia_webview::{WebTag, WebViewError};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, HDC, PAINTSTRUCT};
use windows::Win32::System::LibraryLoader;
use windows::Win32::UI::WindowsAndMessaging::{
    self, WINDOW_EX_STYLE, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};
use windows::core::{PCWSTR, w};

type StdResult<T, E = WebViewError> = std::result::Result<T, E>;

pub type HostWindowCreatedHandler = Arc<dyn Fn(isize) + Send + Sync>;
type CloseHandler = Arc<dyn Fn() + Send + Sync>;
type ChromeEventHandler = Arc<dyn Fn(WindowsChromeCommand) + Send + Sync>;
pub type WindowsHostPanelInputHandler = Arc<dyn Fn(WindowsHostPanelKeyEvent) -> bool + Send + Sync>;

static DEFAULT_WINDOW_SIZE: OnceLock<(i32, i32)> = OnceLock::new();
static CLOSE_HANDLERS: OnceLock<Mutex<HashMap<String, CloseHandler>>> = OnceLock::new();
static CHROME_HANDLERS: OnceLock<Mutex<HashMap<String, ChromeEventHandler>>> = OnceLock::new();
static HOST_WINDOW_CREATED_HANDLERS: OnceLock<Mutex<Vec<HostWindowCreatedHandler>>> =
    OnceLock::new();
static HOST_PANEL_INPUT_HANDLERS: OnceLock<Mutex<HashMap<String, WindowsHostPanelInputHandler>>> =
    OnceLock::new();
static VISIBLE_PANELS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static PANEL_TABS: OnceLock<Mutex<HashMap<String, Vec<WindowsHostPanelTab>>>> = OnceLock::new();
static ACTIVE_WEBTAG: OnceLock<Mutex<Option<WebTag>>> = OnceLock::new();
static WEBTAG_WINDOWS: OnceLock<Mutex<HashMap<String, isize>>> = OnceLock::new();
static WINDOW_LAYOUTS: OnceLock<Mutex<HashMap<String, WindowsWindowLayout>>> = OnceLock::new();
static WINDOWS_CHROME_RENDERER: OnceLock<Mutex<Option<Arc<dyn WindowsChromeRenderer>>>> =
    OnceLock::new();
static WINDOWS_CARD_DECORATOR: OnceLock<Mutex<Option<Arc<dyn WindowsCardDecorator>>>> =
    OnceLock::new();
const WM_LINGXIA_RUN_CALLBACK: u32 = WindowsAndMessaging::WM_APP + 0x158;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowsWebViewContentWindow {
    pub window: isize,
    pub content_left: i32,
    pub content_top: i32,
    pub content_width: i32,
    pub content_height: i32,
    pub scale: f64,
}

struct PlatformNativeViewHost;

impl WindowsWebViewNativeViewHost for PlatformNativeViewHost {
    fn create_webview_parent(&self, webtag: &WebTag) -> StdResult<WindowsWebViewNativeView> {
        create_webview_parent_window(webtag)
    }

    fn destroy_webview_parent(&self, webtag_key: &str, view: WindowsWebViewNativeView) {
        remove_window_handle(webtag_key);
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(view.window));
        }
    }

    fn webview_parent_bounds(&self, view: WindowsWebViewNativeView) -> StdResult<RECT> {
        let hwnd = hwnd_from_handle(view.window);
        let webtag_key = window_webtag_key(hwnd).ok_or_else(|| {
            WebViewError::WebView("Windows WebView parent has no webtag".to_string())
        })?;
        Ok(content_rect_for_window(hwnd, &webtag_key))
    }
}

pub fn install_native_view_host() {
    set_webview_native_view_host(Arc::new(PlatformNativeViewHost));
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

    fn card_corner_color(&self) -> Option<COLORREF> {
        None
    }

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

pub trait WindowsCardDecorator: Send + Sync {
    fn update(&self, parent: HWND, card: RECT, color: COLORREF, side: i32, square_bottom: bool);
    fn raise(&self, parent: HWND);
    fn destroy(&self, parent: HWND);
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
pub struct WindowsWebViewHostWindow {
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

pub fn set_windows_card_decorator(decorator: Arc<dyn WindowsCardDecorator>) {
    let slot = WINDOWS_CARD_DECORATOR.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(decorator);
    }
}

pub fn set_webview_close_handler(webtag: &WebTag, handler: CloseHandler) {
    let handlers = CLOSE_HANDLERS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handlers) = handlers.lock() {
        handlers.insert(webtag.key().to_string(), handler);
    }
}

pub fn set_webview_chrome_event_handler(webtag: &WebTag, handler: ChromeEventHandler) {
    let handlers = CHROME_HANDLERS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handlers) = handlers.lock() {
        handlers.insert(webtag.key().to_string(), handler);
    }
}

pub fn add_webview_host_window_created_handler(handler: HostWindowCreatedHandler) {
    let handlers = HOST_WINDOW_CREATED_HANDLERS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut handlers) = handlers.lock() {
        handlers.push(handler);
    }
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

pub fn set_webview_window_layout(webtag: &WebTag, layout: WindowsWindowLayout) -> StdResult<()> {
    let layouts = WINDOW_LAYOUTS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut layouts) = layouts.lock() {
        layouts.insert(webtag.key().to_string(), layout);
    }
    if let Some(hwnd) = window_handle_for_key(webtag.key()) {
        sync_window_layout(hwnd);
        unsafe {
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, false);
        }
    }
    Ok(())
}

pub fn clear_webview_group_override(_webtag: &WebTag) {}
pub fn set_webview_group_override(_webtag: &WebTag, _group_key: &str) {}
pub fn clear_webview_os_frame(_webtag: &WebTag) {}
pub fn set_webview_os_frame(_webtag: &WebTag) {}

pub fn show_webview_as_panel(webtag: &WebTag, title: &str, panel_id: &str) -> StdResult<()> {
    show_webview_window(webtag, title, true)?;
    mark_active(webtag);
    mark_panel_visible(panel_id, true);
    Ok(())
}

pub fn present_webview_in_active_group(webtag: &WebTag) -> StdResult<()> {
    let handler = find_webview_handler(webtag).ok_or_else(|| handler_not_ready(webtag))?;
    handler.set_content_visible(true)?;
    show_native_view(handler.native_view(), "", true)?;
    mark_active(webtag);
    Ok(())
}

pub fn present_webview_as_group_main(webtag: &WebTag, _group_key: String) -> StdResult<()> {
    present_webview_in_active_group(webtag)
}

pub fn present_webview_as_overlay(
    webtag: &WebTag,
    _width: f64,
    _height: f64,
    _width_ratio: f64,
    _height_ratio: f64,
    _position: u8,
) -> StdResult<()> {
    show_webview_window(webtag, "", true)?;
    mark_active(webtag);
    Ok(())
}

pub fn resize_webview_host_content(webtag: &WebTag, width: i32, height: i32) -> StdResult<()> {
    if width <= 0 || height <= 0 {
        return Err(WebViewError::WebView(format!(
            "invalid window content size {width}x{height}"
        )));
    }
    let snapshot = webview_window_snapshot(webtag)?;
    unsafe {
        WindowsAndMessaging::SetWindowPos(
            hwnd_from_handle(snapshot.window_id as isize),
            None,
            0,
            0,
            width,
            height,
            WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOZORDER
                | WindowsAndMessaging::SWP_NOACTIVATE,
        )
        .map_err(|err| WebViewError::WebView(format!("SetWindowPos failed: {err}")))?;
    }
    Ok(())
}

pub fn restore_presented_group_main() -> StdResult<()> {
    Ok(())
}

pub fn show_interactive_host_panel(
    panel_id: &str,
    _title: &str,
    _body: &str,
    _position: WindowsPanelPosition,
) -> StdResult<()> {
    mark_panel_visible(panel_id, true);
    Ok(())
}

pub fn hide_host_panel(panel_id: &str) -> StdResult<()> {
    mark_panel_visible(panel_id, false);
    Ok(())
}

pub fn update_host_panel_body(_panel_id: &str, _body: &str) -> StdResult<()> {
    Ok(())
}

pub fn set_host_panel_tabs(panel_id: &str, tabs: Vec<WindowsHostPanelTab>) -> bool {
    let state = PANEL_TABS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut state) = state.lock() {
        state.insert(panel_id.to_string(), tabs);
        return true;
    }
    false
}

pub fn set_host_panel_maximized(_panel_id: &str, _maximized: bool) -> bool {
    true
}

pub fn invalidate_host_panel(_panel_id: &str) -> bool {
    true
}

pub fn is_panel_visible(panel_id: &str) -> bool {
    VISIBLE_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .is_some_and(|panels| panels.contains(panel_id))
}

pub fn find_webview_host_window(webtag: &WebTag) -> StdResult<WindowsWebViewHostWindow> {
    let content = find_webview_content_window(webtag).ok_or_else(|| {
        WebViewError::WebView(format!("no window registered for {}", webtag.key()))
    })?;
    Ok(WindowsWebViewHostWindow {
        window: content.window,
    })
}

pub fn request_webview_host_window_layout(window: WindowsWebViewHostWindow) -> bool {
    if window.window == 0 {
        return false;
    }
    unsafe {
        windows::Win32::Graphics::Gdi::InvalidateRect(
            Some(hwnd_from_handle(window.window)),
            None,
            false,
        )
        .as_bool()
    }
}

pub fn active_content_screen_rect() -> Option<WindowsContentRect> {
    let webtag = ACTIVE_WEBTAG
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone())?;
    let content = find_webview_content_window(&webtag)?;
    let hwnd = hwnd_from_handle(content.window);
    let mut origin = windows::Win32::Foundation::POINT {
        x: content.content_left,
        y: content.content_top,
    };
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut origin);
    }
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) };
    Some(WindowsContentRect {
        host_window: content.window,
        left: origin.x,
        top: origin.y,
        width: content.content_width,
        height: content.content_height,
        dpi,
    })
}

fn windows_chrome_renderer() -> Option<Arc<dyn WindowsChromeRenderer>> {
    WINDOWS_CHROME_RENDERER
        .get()
        .and_then(|renderer| renderer.lock().ok())
        .and_then(|renderer| renderer.clone())
}

fn current_window_layout(webtag_key: &str) -> WindowsWindowLayout {
    WINDOW_LAYOUTS
        .get()
        .and_then(|layouts| layouts.lock().ok())
        .and_then(|layouts| layouts.get(webtag_key).cloned())
        .unwrap_or_default()
}

fn content_rect_for_window(hwnd: HWND, webtag_key: &str) -> RECT {
    let mut client = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetClientRect(hwnd, &mut client).is_err() {
            return RECT {
                left: 0,
                top: 0,
                right: 1024,
                bottom: 768,
            };
        }
    }
    let Some(renderer) = windows_chrome_renderer() else {
        return client;
    };
    normalize_rect(renderer.content_rect(client, &current_window_layout(webtag_key)))
}

fn sync_window_layout(hwnd: HWND) {
    let Some(webtag_key) = window_webtag_key(hwnd) else {
        return;
    };
    let rect = content_rect_for_window(hwnd, &webtag_key);
    let width = (rect.right - rect.left).max(0);
    let height = (rect.bottom - rect.top).max(0);
    let Some(webtag) = webtag_for_key(&webtag_key) else {
        return;
    };
    let Some(handler) = find_webview_handler(&webtag) else {
        return;
    };
    if let Err(err) = handler.set_content_bounds(rect.left, rect.top, width, height) {
        log::debug!("Failed to sync Windows WebView content bounds: {err}");
    }
    let _ = handler.notify_parent_position_changed();
}

fn webtag_for_key(webtag_key: &str) -> Option<WebTag> {
    lingxia_webview::runtime::list_webviews()
        .into_iter()
        .find(|webtag| webtag.key() == webtag_key)
}

fn paint_window_chrome(hwnd: HWND) {
    let Some(webtag_key) = window_webtag_key(hwnd) else {
        unsafe {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let _ = EndPaint(hwnd, &ps);
            let _ = hdc;
        }
        return;
    };
    let Some(renderer) = windows_chrome_renderer() else {
        return;
    };
    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    let state = WindowsChromeState {
        hwnd,
        client,
        layout: current_window_layout(&webtag_key),
        attached: None,
        frame_button_hover: None,
        frame_button_pressed: None,
    };
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        renderer.paint(hdc, &state);
        let _ = EndPaint(hwnd, &ps);
    }
}

fn normalize_rect(mut rect: RECT) -> RECT {
    if rect.right < rect.left {
        rect.right = rect.left;
    }
    if rect.bottom < rect.top {
        rect.bottom = rect.top;
    }
    rect
}

fn handler_not_ready(webtag: &WebTag) -> WebViewError {
    WebViewError::WebView(format!("WebView handler not found for {}", webtag.key()))
}

fn mark_active(webtag: &WebTag) {
    let slot = ACTIVE_WEBTAG.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(webtag.clone());
    }
}

fn mark_panel_visible(panel_id: &str, visible: bool) {
    let panels = VISIBLE_PANELS.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(mut panels) = panels.lock() {
        if visible {
            panels.insert(panel_id.to_string());
        } else {
            panels.remove(panel_id);
        }
    }
}

pub fn find_webview_content_window(webtag: &WebTag) -> Option<WindowsWebViewContentWindow> {
    let hwnd = window_handle_for_key(webtag.key())?;
    let client = content_rect_for_window(hwnd, webtag.key());
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) };
    Some(WindowsWebViewContentWindow {
        window: hwnd_handle(hwnd),
        content_left: client.left,
        content_top: client.top,
        content_width: (client.right - client.left).max(0),
        content_height: (client.bottom - client.top).max(0),
        scale: if dpi == 0 { 1.0 } else { dpi as f64 / 96.0 },
    })
}

pub fn webview_window_snapshot(webtag: &WebTag) -> StdResult<WindowsWebViewWindowSnapshot> {
    let hwnd = window_handle_for_key(webtag.key()).ok_or_else(|| {
        WebViewError::WebView(format!("no window registered for {}", webtag.key()))
    })?;
    let content = content_rect_for_window(hwnd, webtag.key());
    Ok(WindowsWebViewWindowSnapshot {
        window_id: hwnd_handle(hwnd) as usize,
        webtag_key: webtag.key().to_string(),
        visible: unsafe { WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() },
        content_left: content.left,
        content_top: content.top,
        content_width: (content.right - content.left).max(0) as u32,
        content_height: (content.bottom - content.top).max(0) as u32,
    })
}

pub fn show_webview_window(webtag: &WebTag, title: &str, activate: bool) -> StdResult<()> {
    let handler = find_webview_handler(webtag).ok_or_else(|| handler_not_ready(webtag))?;
    show_native_view(handler.native_view(), title, activate)?;
    handler.set_content_visible(true)?;
    Ok(())
}

pub fn hide_webview_window(webtag: &WebTag) -> StdResult<()> {
    let handler = find_webview_handler(webtag).ok_or_else(|| handler_not_ready(webtag))?;
    handler.set_content_visible(false)?;
    unsafe {
        WindowsAndMessaging::SetWindowPos(
            hwnd_from_handle(handler.native_view().window),
            None,
            0,
            0,
            0,
            0,
            WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOZORDER
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_HIDEWINDOW,
        )
        .map_err(|err| WebViewError::WebView(format!("SetWindowPos failed: {err}")))?;
    }
    Ok(())
}

fn show_native_view(view: WindowsWebViewNativeView, title: &str, activate: bool) -> StdResult<()> {
    let title = to_wide(title);
    let hwnd = hwnd_from_handle(view.window);
    unsafe {
        let _ = WindowsAndMessaging::SetWindowTextW(hwnd, PCWSTR(title.as_ptr()));
        let mut flags = WindowsAndMessaging::SWP_NOMOVE | WindowsAndMessaging::SWP_NOSIZE;
        if !activate {
            flags |= WindowsAndMessaging::SWP_NOACTIVATE;
        }
        WindowsAndMessaging::SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            flags | WindowsAndMessaging::SWP_SHOWWINDOW,
        )
        .map_err(|err| WebViewError::WebView(format!("SetWindowPos failed: {err}")))?;
        if activate {
            let _ = WindowsAndMessaging::BringWindowToTop(hwnd);
            let _ = WindowsAndMessaging::SetForegroundWindow(hwnd);
        }
    }
    sync_window_layout(hwnd);
    Ok(())
}

fn create_webview_parent_window(webtag: &WebTag) -> StdResult<WindowsWebViewNativeView> {
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WindowsAndMessaging::WM_NCCREATE => {
                let create = lparam.0 as *const WindowsAndMessaging::CREATESTRUCTW;
                if !create.is_null() {
                    let user_data = unsafe { (*create).lpCreateParams } as *mut String;
                    unsafe {
                        let _ = WindowsAndMessaging::SetWindowLongPtrW(
                            hwnd,
                            WindowsAndMessaging::GWLP_USERDATA,
                            user_data as isize,
                        );
                    }
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_CLOSE => {
                if let Some(webtag_key) = window_webtag_key(hwnd)
                    && invoke_close_handler(&webtag_key)
                {
                    return LRESULT(0);
                }
                unsafe {
                    let _ = WindowsAndMessaging::DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WindowsAndMessaging::WM_SIZE | WindowsAndMessaging::WM_WINDOWPOSCHANGED => {
                sync_window_layout(hwnd);
                if windows_chrome_renderer().is_some() {
                    unsafe {
                        let _ =
                            windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, false);
                    }
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_ERASEBKGND => {
                if windows_chrome_renderer().is_some() {
                    return LRESULT(1);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_PAINT => {
                if windows_chrome_renderer().is_some() {
                    paint_window_chrome(hwnd);
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_DESTROY => {
                unsafe {
                    WindowsAndMessaging::PostQuitMessage(0);
                }
                LRESULT(0)
            }
            WindowsAndMessaging::WM_NCDESTROY => {
                let raw = unsafe {
                    WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
                } as *mut String;
                if !raw.is_null() {
                    unsafe {
                        let _ = Box::from_raw(raw);
                        let _ = WindowsAndMessaging::SetWindowLongPtrW(
                            hwnd,
                            WindowsAndMessaging::GWLP_USERDATA,
                            0,
                        );
                    }
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_LINGXIA_RUN_CALLBACK => {
                run_posted_window_callback(wparam);
                LRESULT(0)
            }
            _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    let class = WNDCLASSW {
        style: WindowsAndMessaging::CS_HREDRAW | WindowsAndMessaging::CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hCursor: unsafe { WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_ARROW) }
            .unwrap_or_default(),
        lpszClassName: w!("LingXiaWebViewParent"),
        ..Default::default()
    };

    unsafe {
        WindowsAndMessaging::RegisterClassW(&class);
        let (width, height) = default_window_size();
        let user_data = Box::new(webtag.key().to_string());
        let user_data_ptr = Box::into_raw(user_data);
        let result = WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("LingXiaWebViewParent"),
            w!("LingXia WebView"),
            WS_OVERLAPPEDWINDOW,
            WindowsAndMessaging::CW_USEDEFAULT,
            WindowsAndMessaging::CW_USEDEFAULT,
            width,
            height,
            None,
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            Some(user_data_ptr.cast()),
        );
        match result {
            Ok(hwnd) => {
                register_window_handle(webtag.key(), hwnd);
                invoke_host_window_created_handler(hwnd);
                Ok(WindowsWebViewNativeView {
                    window: hwnd_handle(hwnd),
                })
            }
            Err(err) => {
                let _ = Box::from_raw(user_data_ptr);
                Err(WebViewError::WebView(format!(
                    "CreateWindowExW failed: {err}"
                )))
            }
        }
    }
}

pub fn post_to_window_thread(window: isize, callback: Box<dyn FnOnce() + Send>) -> bool {
    if window == 0 {
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
        drop(unsafe { Box::from_raw(raw) });
    }
    posted
}

fn run_posted_window_callback(wparam: WPARAM) {
    let raw = wparam.0 as *mut Box<dyn FnOnce() + Send>;
    if raw.is_null() {
        return;
    }
    let callback = unsafe { Box::from_raw(raw) };
    callback();
}

fn invoke_close_handler(webtag_key: &str) -> bool {
    let handler = CLOSE_HANDLERS
        .get()
        .and_then(|handlers| handlers.lock().ok())
        .and_then(|handlers| handlers.get(webtag_key).cloned());
    if let Some(handler) = handler {
        let _ = std::thread::Builder::new()
            .name(format!("lingxia-windows-close-{webtag_key}"))
            .spawn(move || handler());
        true
    } else {
        false
    }
}

fn invoke_host_window_created_handler(hwnd: HWND) {
    let handlers = HOST_WINDOW_CREATED_HANDLERS
        .get()
        .and_then(|state| state.lock().ok())
        .map(|state| state.clone())
        .unwrap_or_default();
    for handler in handlers {
        handler(hwnd_handle(hwnd));
    }
}

fn register_window_handle(webtag_key: &str, hwnd: HWND) {
    let handles = WEBTAG_WINDOWS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handles) = handles.lock() {
        handles.insert(webtag_key.to_string(), hwnd_handle(hwnd));
    }
}

fn remove_window_handle(webtag_key: &str) {
    if let Some(handles) = WEBTAG_WINDOWS.get()
        && let Ok(mut handles) = handles.lock()
    {
        handles.remove(webtag_key);
    }
}

fn window_handle_for_key(webtag_key: &str) -> Option<HWND> {
    WEBTAG_WINDOWS
        .get()
        .and_then(|handles| handles.lock().ok())
        .and_then(|handles| handles.get(webtag_key).copied())
        .filter(|handle| unsafe {
            WindowsAndMessaging::IsWindow(Some(hwnd_from_handle(*handle))).as_bool()
        })
        .map(hwnd_from_handle)
}

fn window_webtag_key(hwnd: HWND) -> Option<String> {
    let raw =
        unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA) }
            as *const String;
    if raw.is_null() {
        None
    } else {
        Some(unsafe { (*raw).clone() })
    }
}

fn hwnd_from_handle(handle: isize) -> HWND {
    HWND(handle as *mut c_void)
}

fn hwnd_handle(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
