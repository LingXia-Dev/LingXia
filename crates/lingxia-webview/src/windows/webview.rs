use crate::traits::{DownloadRequest, LoadDataRequest, NavigationPolicy, NewWindowPolicy};
use crate::webview::{
    EffectiveWebViewCreateOptions, WebTag, WebViewCreateSender, WebViewCreateStage, find_webview,
    find_webview_delegate, register_webview,
};
use crate::{
    LogLevel, WebResourceBody, WebResourceResponse, WebViewController, WebViewError,
    WebViewScriptError,
};
use async_trait::async_trait;
use http::{Request, StatusCode};
use std::collections::HashMap;
use std::ffi::c_void;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use windows::{
    Win32::{
        Foundation::{COLORREF, E_POINTER, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, ClientToScreen, CreateBitmap, CreateSolidBrush, DT_CENTER, DT_END_ELLIPSIS,
            DT_LEFT, DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawTextW, EndPaint, FillRect,
            GetStockObject, HDC, HGDIOBJ, InvalidateRect, NULL_PEN, PAINTSTRUCT, RoundRect,
            SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
        },
        System::{
            Com::{
                COINIT_APARTMENTTHREADED, IStream, STREAM_SEEK_SET,
                StructuredStorage::CreateStreamOnHGlobal,
            },
            LibraryLoader, Threading,
        },
        UI::{
            Shell::SHCreateMemStream,
            WindowsAndMessaging::{
                self, CREATESTRUCTW, GCLP_HICON, GCLP_HICONSM, HICON, ICON_BIG, ICON_SMALL,
                ICONINFO, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_NCCREATE, WM_SETICON,
                WNDCLASSW, WS_OVERLAPPEDWINDOW,
            },
        },
    },
    core::{BOOL, Interface, PCWSTR, PWSTR, Result as WinResult, w},
};

const WM_LINGXIA_COMMAND: u32 = WM_APP + 0x154;
const WM_LINGXIA_LAYOUT: u32 = WM_APP + 0x155;
const ARC_PANEL_PADDING: i32 = 6;
const ARC_PANEL_RADIUS: i32 = 14;
const ARC_WINDOW_BACKGROUND: u32 = 0xe7e8eb;
const ARC_PANEL_BACKGROUND: u32 = 0xffffff;
const ARC_SIDEBAR_BACKGROUND: u32 = 0xe7e8eb;
const ARC_SIDEBAR_WIDTH: i32 = 180;
const SIDEBAR_HEADER_HEIGHT: i32 = 66;
const SIDEBAR_ITEM_HEIGHT: i32 = 34;
const SIDEBAR_ITEM_GAP: i32 = 4;
const SIDEBAR_ITEM_INSET: i32 = 10;
const SIDEBAR_FOOTER_HEIGHT: i32 = 46;
const SIDEBAR_ICON_SIZE: i32 = 16;
const PANEL_ACTIVATOR_SIZE: i32 = 28;
const PANEL_ACTIVATOR_ICON_SIZE: i32 = 16;
const PANEL_ACTIVATOR_GAP: i32 = 4;
const PANEL_ACTIVATOR_MARGIN: i32 = 6;
const ATTACHED_PANEL_WIDTH: i32 = 380;
const ATTACHED_PANEL_BOTTOM_HEIGHT: i32 = 280;
type StdResult<T, E = WebViewError> = std::result::Result<T, E>;

#[derive(Debug, Clone, PartialEq, Eq)]
enum WindowsWindowRole {
    Main,
    Panel { panel_id: String },
}

enum UiCommand {
    LoadUrl {
        url: String,
        resp: Sender<StdResult<()>>,
    },
    LoadHtml {
        html: String,
        base_url: String,
        history_url: Option<String>,
        resp: Sender<StdResult<()>>,
    },
    ExecJs {
        js: String,
        resp: Sender<StdResult<()>>,
    },
    EvalJs {
        js: String,
        resp: Sender<std::result::Result<serde_json::Value, WebViewScriptError>>,
    },
    PostMessage {
        message: String,
        resp: Sender<StdResult<()>>,
    },
    SetUserAgent {
        ua: String,
        resp: Sender<StdResult<()>>,
    },
    ClearBrowsingData {
        resp: Sender<StdResult<()>>,
    },
    CurrentUrl {
        resp: Sender<StdResult<Option<String>>>,
    },
    Reload {
        resp: Sender<StdResult<()>>,
    },
    GoBack {
        resp: Sender<StdResult<()>>,
    },
    GoForward {
        resp: Sender<StdResult<()>>,
    },
    TakeScreenshot {
        resp: Sender<StdResult<Vec<u8>>>,
    },
    WindowSnapshot {
        resp: Sender<StdResult<WindowsWebViewWindowSnapshot>>,
    },
    ShowWindow {
        title: String,
        activate: bool,
        role: WindowsWindowRole,
        resp: Sender<StdResult<()>>,
    },
    HideWindow {
        resp: Sender<StdResult<()>>,
    },
    SetWindowLayout {
        layout: WindowsWindowLayout,
        resp: Sender<StdResult<()>>,
    },
    Shutdown,
}

struct UiState {
    controller: ICoreWebView2Controller,
    webview: ICoreWebView2,
    hwnd: HWND,
    webtag_key: String,
    window_visible: bool,
    memory_pages: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

pub struct WebViewInner {
    command_tx: Sender<UiCommand>,
    thread_id: u32,
    join_handle: Mutex<Option<JoinHandle<()>>>,
    pub(crate) webtag: WebTag,
}

type CloseHandler = Arc<dyn Fn() + Send + Sync>;
type ChromeEventHandler = Arc<dyn Fn(WindowsChromeEvent) + Send + Sync>;
type IconCacheKey = (PathBuf, u32);
type IconHandleCache = HashMap<IconCacheKey, Option<isize>>;

struct WindowUserData {
    webtag_key: String,
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

#[derive(Debug, Clone, Copy)]
struct AppIconHandles {
    small: isize,
    large: isize,
}

#[derive(Debug, Clone, Copy)]
struct WindowPlacement {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WindowAttachment {
    group_key: String,
    kind: WindowAttachmentKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WindowAttachmentKind {
    MainHost,
    MainChild,
    Panel {
        panel_id: String,
        position: WindowsPanelPosition,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GroupPanel {
    webtag_key: String,
    panel_id: String,
    position: WindowsPanelPosition,
}

static WINDOW_CLOSE_HANDLERS: OnceLock<Mutex<HashMap<String, CloseHandler>>> = OnceLock::new();
static WINDOW_CHROME_HANDLERS: OnceLock<Mutex<HashMap<String, ChromeEventHandler>>> =
    OnceLock::new();
static WINDOW_LAYOUTS: OnceLock<Mutex<HashMap<String, WindowsWindowLayout>>> = OnceLock::new();
static WINDOW_GROUP_LAYOUTS: OnceLock<Mutex<HashMap<String, WindowsWindowLayout>>> =
    OnceLock::new();
static WINDOW_GROUP_PLACEMENTS: OnceLock<Mutex<HashMap<String, WindowPlacement>>> = OnceLock::new();
static WINDOW_GROUP_HOSTS: OnceLock<Mutex<HashMap<String, isize>>> = OnceLock::new();
static WINDOW_HANDLES: OnceLock<Mutex<HashMap<String, isize>>> = OnceLock::new();
static WINDOW_ATTACHMENTS: OnceLock<Mutex<HashMap<String, WindowAttachment>>> = OnceLock::new();
static WINDOW_GROUP_ACTIVE_MAIN: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
static WINDOW_ACTIVE_GROUP: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static WINDOW_GROUP_PANELS: OnceLock<Mutex<HashMap<String, Vec<GroupPanel>>>> = OnceLock::new();
static APP_ICON_HANDLES: OnceLock<Mutex<Option<AppIconHandles>>> = OnceLock::new();
static PANEL_ICON_HANDLES: OnceLock<Mutex<IconHandleCache>> = OnceLock::new();

impl std::fmt::Debug for WebViewInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebViewInner")
            .field("thread_id", &self.thread_id)
            .field("webtag", &self.webtag)
            .finish()
    }
}

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

fn show_webview_window_with_activation(
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

pub fn set_app_icon_from_path(path: &Path) -> StdResult<()> {
    let handles = AppIconHandles {
        small: create_icon_from_png(path, 16)?,
        large: create_icon_from_png(path, 32)?,
    };
    let icon_state = APP_ICON_HANDLES.get_or_init(|| Mutex::new(None));
    let mut icon_state = icon_state
        .lock()
        .map_err(|_| WebViewError::WebView("Windows app icon state is poisoned".to_string()))?;
    *icon_state = Some(handles);
    Ok(())
}

fn current_app_icon_handles() -> Option<AppIconHandles> {
    APP_ICON_HANDLES
        .get()
        .and_then(|icons| icons.lock().ok().and_then(|icons| *icons))
}

fn create_icon_from_png(path: &Path, size: u32) -> StdResult<isize> {
    let image = image::open(path)
        .map_err(|err| {
            WebViewError::WebView(format!(
                "Failed to load Windows app icon {}: {}",
                path.display(),
                err
            ))
        })?
        .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
        .into_rgba8();

    let mut bgra = Vec::with_capacity(image.len());
    for pixel in image.pixels() {
        let [r, g, b, a] = pixel.0;
        bgra.extend_from_slice(&[b, g, r, a]);
    }

    unsafe {
        let width = size as i32;
        let height = size as i32;
        let color = CreateBitmap(width, height, 1, 32, Some(bgra.as_ptr().cast()));
        if color.is_invalid() {
            return Err(WebViewError::WebView(format!(
                "Failed to create Windows app icon color bitmap from {}",
                path.display()
            )));
        }

        let mask = CreateBitmap(width, height, 1, 1, None);
        if mask.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(color.0));
            return Err(WebViewError::WebView(format!(
                "Failed to create Windows app icon mask bitmap from {}",
                path.display()
            )));
        }

        let info = ICONINFO {
            fIcon: BOOL(1),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = WindowsAndMessaging::CreateIconIndirect(&info).map_err(|err| {
            WebViewError::WebView(format!(
                "Failed to create Windows app icon from {}: {}",
                path.display(),
                err
            ))
        })?;
        let _ = DeleteObject(HGDIOBJ(color.0));
        let _ = DeleteObject(HGDIOBJ(mask.0));
        Ok(icon.0 as isize)
    }
}

fn hicon(handle: isize) -> HICON {
    HICON(handle as *mut c_void)
}

fn apply_window_icons(hwnd: HWND, icons: AppIconHandles) {
    unsafe {
        let _ = WindowsAndMessaging::SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_SMALL as usize)),
            Some(LPARAM(icons.small)),
        );
        let _ = WindowsAndMessaging::SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_BIG as usize)),
            Some(LPARAM(icons.large)),
        );
        let _ = WindowsAndMessaging::SetClassLongPtrW(hwnd, GCLP_HICONSM, icons.small);
        let _ = WindowsAndMessaging::SetClassLongPtrW(hwnd, GCLP_HICON, icons.large);
    }
}

fn hide_titlebar_icon(hwnd: HWND) {
    unsafe {
        let _ = WindowsAndMessaging::SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_SMALL as usize)),
            Some(LPARAM(0)),
        );
        let _ = WindowsAndMessaging::SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_BIG as usize)),
            Some(LPARAM(0)),
        );
        let _ = WindowsAndMessaging::SetClassLongPtrW(hwnd, GCLP_HICONSM, 0);
        let _ = WindowsAndMessaging::SetClassLongPtrW(hwnd, GCLP_HICON, 0);
        let ex_style =
            WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWL_EXSTYLE) as u32;
        let _ = WindowsAndMessaging::SetWindowLongPtrW(
            hwnd,
            WindowsAndMessaging::GWL_EXSTYLE,
            (ex_style | WindowsAndMessaging::WS_EX_DLGMODALFRAME.0) as isize,
        );
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOZORDER
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_FRAMECHANGED,
        );
    }
}

fn invoke_close_handler(webtag_key: &str) -> bool {
    let Some(handlers) = WINDOW_CLOSE_HANDLERS.get() else {
        return false;
    };

    let handler = handlers
        .lock()
        .ok()
        .and_then(|mut handlers| handlers.remove(webtag_key));
    if let Some(handler) = handler {
        let _ = std::thread::Builder::new()
            .name(format!("lingxia-webview-close-{}", webtag_key))
            .spawn(move || handler());
        return true;
    }
    false
}

fn remove_close_handler(webtag_key: &str) {
    if let Some(handlers) = WINDOW_CLOSE_HANDLERS.get()
        && let Ok(mut handlers) = handlers.lock()
    {
        handlers.remove(webtag_key);
    }
}

fn current_window_layout(webtag_key: &str) -> WindowsWindowLayout {
    let exact = WINDOW_LAYOUTS
        .get()
        .and_then(|layouts| layouts.lock().ok())
        .and_then(|layouts| layouts.get(webtag_key).cloned());
    let group = layout_group_key_for_webtag(webtag_key);
    let group_layout = WINDOW_GROUP_LAYOUTS
        .get()
        .and_then(|layouts| layouts.lock().ok())
        .and_then(|layouts| layouts.get(&group).cloned());

    if window_attachment(webtag_key)
        .is_some_and(|attachment| matches!(attachment.kind, WindowAttachmentKind::MainHost))
    {
        return group_layout.or(exact).unwrap_or_default();
    }

    exact.or(group_layout).unwrap_or_default()
}

fn set_window_layout_for_key(webtag_key: &str, layout: WindowsWindowLayout) {
    let layouts = WINDOW_LAYOUTS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut layouts) = layouts.lock() {
        layouts.insert(webtag_key.to_string(), layout);
    }

    if !window_attachment(webtag_key)
        .is_some_and(|attachment| matches!(attachment.kind, WindowAttachmentKind::Panel { .. }))
    {
        let group_key = layout_group_key_for_webtag(webtag_key);
        let layouts = WINDOW_GROUP_LAYOUTS.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(mut layouts) = layouts.lock() {
            layouts.insert(
                group_key,
                current_exact_window_layout(webtag_key).unwrap_or_default(),
            );
        }
    }
}

fn remove_window_layout(webtag_key: &str) {
    if let Some(layouts) = WINDOW_LAYOUTS.get()
        && let Ok(mut layouts) = layouts.lock()
    {
        layouts.remove(webtag_key);
    }
}

fn remove_group_layout(group_key: &str) {
    if let Some(layouts) = WINDOW_GROUP_LAYOUTS.get()
        && let Ok(mut layouts) = layouts.lock()
    {
        layouts.remove(group_key);
    }
}

fn remove_chrome_event_handler(webtag_key: &str) {
    if let Some(handlers) = WINDOW_CHROME_HANDLERS.get()
        && let Ok(mut handlers) = handlers.lock()
    {
        handlers.remove(webtag_key);
    }
}

fn invoke_chrome_event_handler(webtag_key: &str, event: WindowsChromeEvent) -> bool {
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

fn webtag_group_key(webtag_key: &str) -> String {
    let Some((appid, path_with_session)) = webtag_key.split_once(':') else {
        return webtag_key.to_string();
    };
    let session = path_with_session
        .rsplit_once('#')
        .and_then(|(_, suffix)| suffix.parse::<u64>().ok())
        .map(|session| session.to_string())
        .unwrap_or_else(|| "0".to_string());
    format!("{appid}#{session}")
}

fn current_exact_window_layout(webtag_key: &str) -> Option<WindowsWindowLayout> {
    WINDOW_LAYOUTS
        .get()
        .and_then(|layouts| layouts.lock().ok())
        .and_then(|layouts| layouts.get(webtag_key).cloned())
}

fn window_attachment(webtag_key: &str) -> Option<WindowAttachment> {
    WINDOW_ATTACHMENTS
        .get()
        .and_then(|attachments| attachments.lock().ok())
        .and_then(|attachments| attachments.get(webtag_key).cloned())
}

fn layout_group_key_for_webtag(webtag_key: &str) -> String {
    window_attachment(webtag_key)
        .map(|attachment| attachment.group_key)
        .unwrap_or_else(|| webtag_group_key(webtag_key))
}

fn hwnd_handle(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

fn hwnd_from_handle(handle: isize) -> HWND {
    HWND(handle as *mut c_void)
}

fn is_window_handle_valid(handle: isize) -> bool {
    unsafe { WindowsAndMessaging::IsWindow(Some(hwnd_from_handle(handle))).as_bool() }
}

fn store_current_window_placement(state: &UiState) {
    if matches!(
        window_attachment(&state.webtag_key).map(|attachment| attachment.kind),
        Some(WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. })
    ) {
        return;
    }
    let mut rect = RECT::default();
    if !unsafe { WindowsAndMessaging::IsWindowVisible(state.hwnd).as_bool() }
        || unsafe { WindowsAndMessaging::GetWindowRect(state.hwnd, &mut rect) }.is_err()
    {
        return;
    }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return;
    }

    let placements = WINDOW_GROUP_PLACEMENTS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut placements) = placements.lock() {
        placements.insert(
            webtag_group_key(&state.webtag_key),
            WindowPlacement {
                left: rect.left,
                top: rect.top,
                width,
                height,
            },
        );
    }
}

fn register_window_handle(webtag_key: &str, hwnd: HWND) {
    let handles = WINDOW_HANDLES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handles) = handles.lock() {
        handles.insert(webtag_key.to_string(), hwnd_handle(hwnd));
    }
}

fn window_handle_for_key(webtag_key: &str) -> Option<HWND> {
    WINDOW_HANDLES
        .get()
        .and_then(|handles| handles.lock().ok())
        .and_then(|handles| handles.get(webtag_key).copied())
        .filter(|handle| is_window_handle_valid(*handle))
        .map(hwnd_from_handle)
}

fn host_handle_for_group(group_key: &str) -> Option<HWND> {
    WINDOW_GROUP_HOSTS
        .get()
        .and_then(|hosts| hosts.lock().ok())
        .and_then(|hosts| hosts.get(group_key).copied())
        .filter(|handle| is_window_handle_valid(*handle))
        .map(hwnd_from_handle)
}

fn set_window_attachment(webtag_key: &str, attachment: WindowAttachment) {
    let attachments = WINDOW_ATTACHMENTS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut attachments) = attachments.lock() {
        attachments.insert(webtag_key.to_string(), attachment);
    }
}

fn remove_window_attachment(webtag_key: &str) -> Option<WindowAttachment> {
    WINDOW_ATTACHMENTS
        .get()
        .and_then(|attachments| attachments.lock().ok())
        .and_then(|mut attachments| attachments.remove(webtag_key))
}

fn remove_window_handle(webtag_key: &str) {
    if let Some(handles) = WINDOW_HANDLES.get()
        && let Ok(mut handles) = handles.lock()
    {
        handles.remove(webtag_key);
    }
}

fn ensure_main_attachment(state: &UiState) -> (String, HWND, bool) {
    register_window_handle(&state.webtag_key, state.hwnd);
    let group_key = webtag_group_key(&state.webtag_key);
    let host_handle = {
        let hosts = WINDOW_GROUP_HOSTS.get_or_init(|| Mutex::new(HashMap::new()));
        let Ok(mut hosts) = hosts.lock() else {
            return (group_key, state.hwnd, true);
        };
        let existing = hosts
            .get(&group_key)
            .copied()
            .filter(|handle| is_window_handle_valid(*handle));
        let host_handle = existing.unwrap_or_else(|| hwnd_handle(state.hwnd));
        hosts.insert(group_key.clone(), host_handle);
        host_handle
    };
    let is_host = host_handle == hwnd_handle(state.hwnd);
    let kind = if is_host {
        WindowAttachmentKind::MainHost
    } else {
        WindowAttachmentKind::MainChild
    };
    set_window_attachment(
        &state.webtag_key,
        WindowAttachment {
            group_key: group_key.clone(),
            kind,
        },
    );

    let host = hwnd_from_handle(host_handle);
    if !is_host {
        attach_child_window_to_host(state.hwnd, host);
    }
    (group_key, host, is_host)
}

fn active_group_key() -> Option<String> {
    WINDOW_ACTIVE_GROUP
        .get()
        .and_then(|active| active.lock().ok())
        .and_then(|active| active.clone())
}

fn set_active_group(group_key: &str) {
    let active = WINDOW_ACTIVE_GROUP.get_or_init(|| Mutex::new(None));
    if let Ok(mut active) = active.lock() {
        *active = Some(group_key.to_string());
    }
}

fn group_active_main(group_key: &str) -> Option<String> {
    WINDOW_GROUP_ACTIVE_MAIN
        .get()
        .and_then(|active| active.lock().ok())
        .and_then(|active| active.get(group_key).cloned())
}

fn set_group_active_main(group_key: &str, webtag_key: &str) {
    let active = WINDOW_GROUP_ACTIVE_MAIN.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut active) = active.lock() {
        active.insert(group_key.to_string(), webtag_key.to_string());
    }
}

fn panel_position_for_group(group_key: &str, panel_id: &str) -> WindowsPanelPosition {
    WINDOW_GROUP_LAYOUTS
        .get()
        .and_then(|layouts| layouts.lock().ok())
        .and_then(|layouts| layouts.get(group_key).cloned())
        .and_then(|layout| {
            layout
                .panel_activators
                .into_iter()
                .find(|activator| activator.id == panel_id)
                .map(|activator| activator.position)
        })
        .unwrap_or_default()
}

fn register_group_panel(group_key: &str, panel: GroupPanel) {
    let panels = WINDOW_GROUP_PANELS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut panels) = panels.lock() {
        let group_panels = panels.entry(group_key.to_string()).or_default();
        group_panels.retain(|item| item.panel_id != panel.panel_id);
        group_panels.push(panel);
    }
}

fn remove_group_panel(group_key: &str, webtag_key: &str) {
    if let Some(panels) = WINDOW_GROUP_PANELS.get()
        && let Ok(mut panels) = panels.lock()
        && let Some(group_panels) = panels.get_mut(group_key)
    {
        group_panels.retain(|panel| panel.webtag_key != webtag_key);
    }
}

fn group_panels(group_key: &str) -> Vec<GroupPanel> {
    WINDOW_GROUP_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .and_then(|panels| panels.get(group_key).cloned())
        .unwrap_or_default()
}

fn attach_child_window_to_host(child: HWND, host: HWND) {
    unsafe {
        let _ = WindowsAndMessaging::SetParent(child, Some(host));
        let style =
            WindowsAndMessaging::GetWindowLongPtrW(child, WindowsAndMessaging::GWL_STYLE) as u32;
        let child_style = (style & !WS_OVERLAPPEDWINDOW.0 & !WindowsAndMessaging::WS_POPUP.0)
            | WindowsAndMessaging::WS_CHILD.0
            | WindowsAndMessaging::WS_CLIPCHILDREN.0
            | WindowsAndMessaging::WS_CLIPSIBLINGS.0;
        let _ = WindowsAndMessaging::SetWindowLongPtrW(
            child,
            WindowsAndMessaging::GWL_STYLE,
            child_style as isize,
        );
        let _ = WindowsAndMessaging::SetWindowPos(
            child,
            Some(WindowsAndMessaging::HWND_TOP),
            0,
            0,
            0,
            0,
            WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_NOOWNERZORDER
                | WindowsAndMessaging::SWP_FRAMECHANGED,
        );
    }
}

fn show_shell_host(group_key: &str, host: HWND, activate: bool) {
    if let Some(placement) = current_group_window_placement_for_group(group_key) {
        unsafe {
            let _ = WindowsAndMessaging::SetWindowPos(
                host,
                None,
                placement.left,
                placement.top,
                placement.width,
                placement.height,
                WindowsAndMessaging::SWP_NOZORDER | WindowsAndMessaging::SWP_NOACTIVATE,
            );
        }
    }

    let title = to_wide("");
    unsafe {
        hide_titlebar_icon(host);
        let _ = WindowsAndMessaging::SetWindowTextW(host, PCWSTR(title.as_ptr()));
        let mut flags = WindowsAndMessaging::SWP_NOMOVE | WindowsAndMessaging::SWP_NOSIZE;
        if !activate {
            flags |= WindowsAndMessaging::SWP_NOACTIVATE;
        }
        let _ = WindowsAndMessaging::SetWindowPos(
            host,
            None,
            0,
            0,
            0,
            0,
            flags | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
        if activate {
            let _ = WindowsAndMessaging::BringWindowToTop(host);
            let _ = WindowsAndMessaging::SetForegroundWindow(host);
        }
    }
}

fn current_group_window_placement_for_group(group_key: &str) -> Option<WindowPlacement> {
    WINDOW_GROUP_PLACEMENTS
        .get()
        .and_then(|placements| placements.lock().ok())
        .and_then(|placements| placements.get(group_key).copied())
}

#[derive(Debug, Clone)]
struct AttachedGroupRects {
    main: RECT,
    panels: HashMap<String, RECT>,
}

fn attached_group_rects(group_key: &str, host: HWND) -> Option<AttachedGroupRects> {
    let mut client = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetClientRect(host, &mut client).is_err() {
            return None;
        }
    }
    let layout = WINDOW_GROUP_LAYOUTS
        .get()
        .and_then(|layouts| layouts.lock().ok())
        .and_then(|layouts| layouts.get(group_key).cloned())
        .unwrap_or_default();
    let mut main = compute_content_rect(client, &layout);
    let mut panels = HashMap::new();

    for panel in group_panels(group_key) {
        let rect = match panel.position {
            WindowsPanelPosition::Left => {
                let width = attached_panel_width(main);
                let rect = RECT {
                    left: main.left,
                    top: main.top,
                    right: (main.left + width).min(main.right),
                    bottom: main.bottom,
                };
                main.left = (rect.right + ARC_PANEL_PADDING).min(main.right);
                rect
            }
            WindowsPanelPosition::Right => {
                let width = attached_panel_width(main);
                let rect = RECT {
                    left: (main.right - width).max(main.left),
                    top: main.top,
                    right: main.right,
                    bottom: main.bottom,
                };
                main.right = (rect.left - ARC_PANEL_PADDING).max(main.left);
                rect
            }
            WindowsPanelPosition::Bottom => {
                let height = attached_panel_bottom_height(main);
                let rect = RECT {
                    left: main.left,
                    top: (main.bottom - height).max(main.top),
                    right: main.right,
                    bottom: main.bottom,
                };
                main.bottom = (rect.top - ARC_PANEL_PADDING).max(main.top);
                rect
            }
        };
        panels.insert(panel.webtag_key, normalize_rect(rect));
    }

    Some(AttachedGroupRects {
        main: normalize_rect(main),
        panels,
    })
}

fn attached_panel_width(content: RECT) -> i32 {
    let available = rect_width(&content);
    if available <= 0 {
        return 0;
    }
    ATTACHED_PANEL_WIDTH
        .min((available / 2).max(260))
        .min(available)
}

fn attached_panel_bottom_height(content: RECT) -> i32 {
    let available = rect_height(&content);
    if available <= 0 {
        return 0;
    }
    ATTACHED_PANEL_BOTTOM_HEIGHT
        .min((available / 2).max(180))
        .min(available)
}

fn layout_group_windows(group_key: &str) {
    let Some(host) = host_handle_for_group(group_key) else {
        return;
    };
    let Some(rects) = attached_group_rects(group_key, host) else {
        return;
    };
    let active_main = group_active_main(group_key);
    let attachments = WINDOW_ATTACHMENTS
        .get()
        .and_then(|attachments| attachments.lock().ok())
        .map(|attachments| {
            attachments
                .iter()
                .filter(|(_, attachment)| attachment.group_key == group_key)
                .map(|(webtag_key, attachment)| (webtag_key.clone(), attachment.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    for (webtag_key, attachment) in attachments {
        let Some(hwnd) = window_handle_for_key(&webtag_key) else {
            continue;
        };
        match attachment.kind {
            WindowAttachmentKind::MainHost => {}
            WindowAttachmentKind::MainChild => {
                let visible = active_main.as_deref() == Some(webtag_key.as_str());
                set_attached_window_rect(hwnd, rects.main, visible);
            }
            WindowAttachmentKind::Panel { .. } => {
                let Some(rect) = rects.panels.get(&webtag_key).copied() else {
                    hide_attached_window(hwnd);
                    continue;
                };
                set_attached_window_rect(hwnd, rect, true);
            }
        }
    }

    unsafe {
        let _ = InvalidateRect(Some(host), None, false);
    }
}

fn layout_group_for_state(state: &UiState) {
    if !matches!(
        window_attachment(&state.webtag_key).map(|attachment| attachment.kind),
        Some(WindowAttachmentKind::MainHost)
    ) {
        return;
    }
    layout_group_windows(&layout_group_key_for_webtag(&state.webtag_key));
}

fn request_group_shell_refresh(group_key: &str) {
    let Some(host) = host_handle_for_group(group_key) else {
        return;
    };
    unsafe {
        let _ = WindowsAndMessaging::PostMessageW(
            Some(host),
            WM_LINGXIA_LAYOUT,
            WPARAM::default(),
            LPARAM::default(),
        );
        let _ = InvalidateRect(Some(host), None, false);
    }
}

fn set_attached_window_rect(hwnd: HWND, rect: RECT, visible: bool) {
    let width = rect_width(&rect);
    let height = rect_height(&rect);
    if width == 0 || height == 0 || !visible {
        hide_attached_window(hwnd);
        return;
    }
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd,
            Some(WindowsAndMessaging::HWND_TOP),
            rect.left,
            rect.top,
            width,
            height,
            WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_NOOWNERZORDER
                | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
    }
}

fn hide_attached_window(hwnd: HWND) {
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd,
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
        );
    }
}

impl WebViewInner {
    pub(crate) fn create(
        appid: &str,
        path: &str,
        session_id: Option<u64>,
        effective_options: EffectiveWebViewCreateOptions,
        sender: WebViewCreateSender,
    ) {
        let webtag = WebTag::new(appid, path, session_id);
        let webtag_for_thread = webtag.clone();
        let effective_options_for_thread = effective_options.clone();
        let (startup_tx, startup_rx) = mpsc::channel();

        let join_handle = thread::Builder::new()
            .name(format!("lingxia-webview-{}", webtag.as_str()))
            .spawn(move || {
                if let Err(err) =
                    run_ui_thread(webtag_for_thread, effective_options_for_thread, startup_tx)
                {
                    log::error!("Windows WebView UI thread failed: {}", err);
                }
            });

        let join_handle = match join_handle {
            Ok(handle) => handle,
            Err(err) => {
                sender.fail(
                    WebViewCreateStage::Requested,
                    WebViewError::WebView(format!(
                        "Failed to spawn Windows WebView thread: {}",
                        err
                    )),
                );
                return;
            }
        };

        match startup_rx.recv() {
            Ok(Ok((command_tx, thread_id))) => {
                let webview = Arc::new(crate::WebView::new(
                    WebViewInner {
                        command_tx,
                        thread_id,
                        join_handle: Mutex::new(Some(join_handle)),
                        webtag,
                    },
                    effective_options,
                ));
                register_webview(webview.clone());
                sender.succeed(webview);
            }
            Ok(Err(err)) => {
                sender.fail(WebViewCreateStage::Requested, err);
                let _ = join_handle.join();
            }
            Err(err) => {
                sender.fail(
                    WebViewCreateStage::Requested,
                    WebViewError::WebView(format!(
                        "Windows WebView startup channel failed: {}",
                        err
                    )),
                );
                let _ = join_handle.join();
            }
        }
    }

    fn dispatch_command(
        &self,
        command: impl FnOnce(Sender<StdResult<()>>) -> UiCommand,
    ) -> StdResult<()> {
        if unsafe { Threading::GetCurrentThreadId() } == self.thread_id {
            return Err(WebViewError::WebView(
                "Cannot run synchronous WebView command from WebView UI thread".to_string(),
            ));
        }

        let (resp_tx, resp_rx) = mpsc::channel();
        self.command_tx
            .send(command(resp_tx))
            .map_err(|_| WebViewError::WebView("WebView UI thread is unavailable".to_string()))?;

        unsafe {
            let _ = WindowsAndMessaging::PostThreadMessageW(
                self.thread_id,
                WM_LINGXIA_COMMAND,
                WPARAM::default(),
                LPARAM::default(),
            );
        }

        resp_rx
            .recv()
            .map_err(|_| WebViewError::WebView("WebView UI thread did not reply".to_string()))?
    }

    fn show_window(&self, title: String, activate: bool, role: WindowsWindowRole) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::ShowWindow {
            title,
            activate,
            role,
            resp,
        })
    }

    fn hide_window(&self) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::HideWindow { resp })
    }

    fn set_window_layout(&self, layout: WindowsWindowLayout) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::SetWindowLayout { layout, resp })
    }

    fn dispatch_screenshot_command(&self) -> StdResult<Vec<u8>> {
        if unsafe { Threading::GetCurrentThreadId() } == self.thread_id {
            return Err(WebViewError::WebView(
                "Cannot capture WebView screenshot from WebView UI thread".to_string(),
            ));
        }

        let (resp_tx, resp_rx) = mpsc::channel();
        self.command_tx
            .send(UiCommand::TakeScreenshot { resp: resp_tx })
            .map_err(|_| WebViewError::WebView("WebView UI thread is unavailable".to_string()))?;

        unsafe {
            let _ = WindowsAndMessaging::PostThreadMessageW(
                self.thread_id,
                WM_LINGXIA_COMMAND,
                WPARAM::default(),
                LPARAM::default(),
            );
        }

        resp_rx
            .recv()
            .map_err(|_| WebViewError::WebView("WebView UI thread did not reply".to_string()))?
    }

    fn window_snapshot(&self) -> StdResult<WindowsWebViewWindowSnapshot> {
        if unsafe { Threading::GetCurrentThreadId() } == self.thread_id {
            return Err(WebViewError::WebView(
                "Cannot inspect WebView window from WebView UI thread".to_string(),
            ));
        }

        let (resp_tx, resp_rx) = mpsc::channel();
        self.command_tx
            .send(UiCommand::WindowSnapshot { resp: resp_tx })
            .map_err(|_| WebViewError::WebView("WebView UI thread is unavailable".to_string()))?;

        unsafe {
            let _ = WindowsAndMessaging::PostThreadMessageW(
                self.thread_id,
                WM_LINGXIA_COMMAND,
                WPARAM::default(),
                LPARAM::default(),
            );
        }

        resp_rx
            .recv()
            .map_err(|_| WebViewError::WebView("WebView UI thread did not reply".to_string()))?
    }

    fn dispatch_eval_command(
        &self,
        js: String,
    ) -> std::result::Result<serde_json::Value, WebViewScriptError> {
        if unsafe { Threading::GetCurrentThreadId() } == self.thread_id {
            return Err(WebViewScriptError::Platform(
                "Cannot evaluate JavaScript from WebView UI thread".to_string(),
            ));
        }

        let (resp_tx, resp_rx) = mpsc::channel();
        self.command_tx
            .send(UiCommand::EvalJs { js, resp: resp_tx })
            .map_err(|_| WebViewScriptError::Destroyed)?;

        unsafe {
            let _ = WindowsAndMessaging::PostThreadMessageW(
                self.thread_id,
                WM_LINGXIA_COMMAND,
                WPARAM::default(),
                LPARAM::default(),
            );
        }

        resp_rx.recv().map_err(|_| WebViewScriptError::Destroyed)?
    }

    fn dispatch_current_url(&self) -> StdResult<Option<String>> {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.command_tx
            .send(UiCommand::CurrentUrl { resp: resp_tx })
            .map_err(|_| WebViewError::WebView("WebView UI thread is unavailable".to_string()))?;

        unsafe {
            let _ = WindowsAndMessaging::PostThreadMessageW(
                self.thread_id,
                WM_LINGXIA_COMMAND,
                WPARAM::default(),
                LPARAM::default(),
            );
        }

        resp_rx
            .recv()
            .map_err(|_| WebViewError::WebView("WebView UI thread did not reply".to_string()))?
    }
}

#[async_trait]
impl WebViewController for WebViewInner {
    fn load_url(&self, url: &str) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::LoadUrl {
            url: url.to_string(),
            resp,
        })
    }

    fn load_data(&self, request: LoadDataRequest<'_>) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::LoadHtml {
            html: request.data.to_string(),
            base_url: request.base_url.to_string(),
            history_url: request.history_url.map(str::to_string),
            resp,
        })
    }

    fn exec_js(&self, js: &str) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::ExecJs {
            js: js.to_string(),
            resp,
        })
    }

    async fn eval_js(
        &self,
        js: &str,
    ) -> std::result::Result<serde_json::Value, WebViewScriptError> {
        self.dispatch_eval_command(js.to_string())
    }

    fn post_message(&self, message: &str) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::PostMessage {
            message: message.to_string(),
            resp,
        })
    }

    fn clear_browsing_data(&self) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::ClearBrowsingData { resp })
    }

    fn set_user_agent(&self, ua: &str) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::SetUserAgent {
            ua: ua.to_string(),
            resp,
        })
    }

    async fn current_url(&self) -> StdResult<Option<String>> {
        self.dispatch_current_url()
    }

    fn reload(&self) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::Reload { resp })
    }

    fn go_back(&self) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::GoBack { resp })
    }

    fn go_forward(&self) -> StdResult<()> {
        self.dispatch_command(|resp| UiCommand::GoForward { resp })
    }

    async fn take_screenshot(&self) -> StdResult<Vec<u8>> {
        self.dispatch_screenshot_command()
    }
}

impl Drop for WebViewInner {
    fn drop(&mut self) {
        remove_close_handler(self.webtag.key());
        remove_chrome_event_handler(self.webtag.key());
        remove_window_layout(self.webtag.key());

        let _ = self.command_tx.send(UiCommand::Shutdown);
        unsafe {
            let _ = WindowsAndMessaging::PostThreadMessageW(
                self.thread_id,
                WM_LINGXIA_COMMAND,
                WPARAM::default(),
                LPARAM::default(),
            );
        }

        if let Ok(mut guard) = self.join_handle.lock()
            && let Some(handle) = guard.take()
        {
            let _ = handle.join();
        }
    }
}

fn run_ui_thread(
    webtag: WebTag,
    effective_options: EffectiveWebViewCreateOptions,
    startup_tx: Sender<StdResult<(Sender<UiCommand>, u32)>>,
) -> StdResult<()> {
    unsafe {
        windows::Win32::System::Com::CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|err| WebViewError::WebView(format!("CoInitializeEx failed: {err}")))?;
    }

    let result = run_ui_thread_inner(webtag, effective_options, startup_tx);

    unsafe {
        windows::Win32::System::Com::CoUninitialize();
    }

    result
}

fn run_ui_thread_inner(
    webtag: WebTag,
    effective_options: EffectiveWebViewCreateOptions,
    startup_tx: Sender<StdResult<(Sender<UiCommand>, u32)>>,
) -> StdResult<()> {
    ensure_message_queue();

    let hwnd = create_hidden_window(&webtag)?;
    let env = create_environment(&effective_options)?;
    let controller = create_controller(&env, hwnd)?;
    let webview = unsafe {
        controller
            .CoreWebView2()
            .map_err(|err| WebViewError::WebView(format!("CoreWebView2 failed: {err}")))?
    };

    configure_controller(&controller)?;
    configure_settings(&webview)?;
    install_document_scripts(&webview)?;
    let memory_pages = Arc::new(Mutex::new(HashMap::new()));
    let webtag_key = webtag.key().to_string();
    register_event_handlers(
        &env,
        &webview,
        webtag,
        &effective_options.registered_schemes,
        memory_pages.clone(),
    )?;

    let (command_tx, command_rx) = mpsc::channel();
    startup_tx
        .send(Ok((command_tx, unsafe { Threading::GetCurrentThreadId() })))
        .map_err(|_| WebViewError::WebView("Failed to publish WebView startup".to_string()))?;

    let mut state = UiState {
        controller,
        webview,
        hwnd,
        webtag_key,
        window_visible: false,
        memory_pages,
    };

    message_loop(&mut state, command_rx)
}

fn ensure_message_queue() {
    let mut msg = MSG::default();
    unsafe {
        let _ = WindowsAndMessaging::PeekMessageW(
            &mut msg,
            None,
            0,
            0,
            WindowsAndMessaging::PM_NOREMOVE,
        );
    }
}

fn create_hidden_window(webtag: &WebTag) -> StdResult<HWND> {
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_NCCREATE {
            let create = lparam.0 as *const CREATESTRUCTW;
            if !create.is_null() {
                let user_data = unsafe { (*create).lpCreateParams } as *mut WindowUserData;
                unsafe {
                    let _ = WindowsAndMessaging::SetWindowLongPtrW(
                        hwnd,
                        WindowsAndMessaging::GWLP_USERDATA,
                        user_data as isize,
                    );
                }
            }
        } else if msg == WindowsAndMessaging::WM_ERASEBKGND {
            return LRESULT(1);
        } else if msg == WindowsAndMessaging::WM_SIZE || msg == WindowsAndMessaging::WM_MOVE {
            unsafe {
                let _ = WindowsAndMessaging::PostMessageW(
                    Some(hwnd),
                    WM_LINGXIA_LAYOUT,
                    WPARAM::default(),
                    LPARAM::default(),
                );
            }
        } else if msg == WindowsAndMessaging::WM_PAINT {
            paint_window_chrome(hwnd);
            return LRESULT(0);
        } else if msg == WindowsAndMessaging::WM_LBUTTONUP {
            let raw = unsafe {
                WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
            } as *mut WindowUserData;
            if !raw.is_null()
                && handle_window_chrome_click(
                    hwnd,
                    unsafe { &(*raw).webtag_key },
                    lparam_to_point(lparam),
                )
            {
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_CLOSE {
            let raw = unsafe {
                WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
            } as *mut WindowUserData;
            if !raw.is_null() && invoke_close_handler(unsafe { &(*raw).webtag_key }) {
                return LRESULT(0);
            }
            unsafe {
                let _ = WindowsAndMessaging::DestroyWindow(hwnd);
            }
            return LRESULT(0);
        } else if msg == WindowsAndMessaging::WM_DESTROY {
            unsafe {
                WindowsAndMessaging::PostQuitMessage(0);
            }
            return LRESULT(0);
        } else if msg == WindowsAndMessaging::WM_NCDESTROY {
            let raw = unsafe {
                WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
            } as *mut WindowUserData;
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
        }
        unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    let app_icons = current_app_icon_handles();
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hIcon: app_icons
            .map(|icons| hicon(icons.large))
            .unwrap_or_default(),
        lpszClassName: w!("LingXiaHiddenWebViewHost"),
        ..Default::default()
    };

    unsafe {
        WindowsAndMessaging::RegisterClassW(&class);
        let user_data = Box::new(WindowUserData {
            webtag_key: webtag.key().to_string(),
        });
        let user_data_ptr = Box::into_raw(user_data);

        let result = WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("LingXiaHiddenWebViewHost"),
            w!("LingXiaHiddenWebViewHost"),
            WINDOW_STYLE(WS_OVERLAPPEDWINDOW.0),
            WindowsAndMessaging::CW_USEDEFAULT,
            WindowsAndMessaging::CW_USEDEFAULT,
            1024,
            768,
            None,
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            Some(user_data_ptr.cast()),
        );

        match result {
            Ok(hwnd) => {
                if let Some(icons) = app_icons {
                    apply_window_icons(hwnd, icons);
                }
                hide_titlebar_icon(hwnd);
                Ok(hwnd)
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

#[derive(Debug, Clone, Copy)]
struct ChromeRects {
    content: RECT,
    panel: RECT,
    navigation_bar: Option<RECT>,
    tab_bar: Option<RECT>,
}

fn compute_content_rect(client: RECT, layout: &WindowsWindowLayout) -> RECT {
    compute_chrome_rects(client, layout).content
}

fn compute_chrome_rects(client: RECT, layout: &WindowsWindowLayout) -> ChromeRects {
    let mut content = client;
    let tab_bar = layout
        .tab_bar
        .as_ref()
        .filter(|tabbar| tabbar.visible && !tabbar.items.is_empty() && tabbar.dimension > 0)
        .map(|tabbar| match tabbar.position {
            WindowsTabBarPosition::Left => {
                let width = tabbar.dimension.max(ARC_SIDEBAR_WIDTH);
                let right = (content.left + width).min(content.right);
                let rect = RECT {
                    left: content.left,
                    top: content.top,
                    right,
                    bottom: content.bottom,
                };
                content.left = right + ARC_PANEL_PADDING;
                content.top += ARC_PANEL_PADDING;
                content.right -= ARC_PANEL_PADDING;
                content.bottom -= ARC_PANEL_PADDING;
                rect
            }
            WindowsTabBarPosition::Right => {
                let width = tabbar.dimension.max(ARC_SIDEBAR_WIDTH);
                let left = (content.right - width).max(content.left);
                let rect = RECT {
                    left,
                    top: content.top,
                    right: content.right,
                    bottom: content.bottom,
                };
                content.right = left - ARC_PANEL_PADDING;
                content.top += ARC_PANEL_PADDING;
                content.left += ARC_PANEL_PADDING;
                content.bottom -= ARC_PANEL_PADDING;
                rect
            }
            WindowsTabBarPosition::Bottom => {
                let top = (content.bottom - tabbar.dimension).max(content.top);
                let rect = RECT {
                    left: content.left,
                    top,
                    right: content.right,
                    bottom: content.bottom,
                };
                content.bottom = top;
                rect
            }
        });

    content = normalize_rect(content);
    let panel = content;

    let navigation_bar = layout
        .navigation_bar
        .as_ref()
        .filter(|navbar| navbar.visible && navbar.height > 0)
        .map(|navbar| {
            let bottom = (content.top + navbar.height).min(content.bottom);
            content.top = bottom;
            RECT {
                left: content.left,
                top: panel.top,
                right: content.right,
                bottom,
            }
        });

    ChromeRects {
        content: normalize_rect(content),
        panel: normalize_rect(panel),
        navigation_bar: navigation_bar.map(normalize_rect),
        tab_bar: tab_bar.map(normalize_rect),
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

fn rect_width(rect: &RECT) -> i32 {
    (rect.right - rect.left).max(0)
}

fn rect_height(rect: &RECT) -> i32 {
    (rect.bottom - rect.top).max(0)
}

fn rect_contains(rect: &RECT, point: (i32, i32)) -> bool {
    point.0 >= rect.left && point.0 < rect.right && point.1 >= rect.top && point.1 < rect.bottom
}

fn lparam_to_point(lparam: LPARAM) -> (i32, i32) {
    let value = lparam.0 as u32;
    let x = (value & 0xffff) as i16 as i32;
    let y = ((value >> 16) & 0xffff) as i16 as i32;
    (x, y)
}

fn paint_window_chrome(hwnd: HWND) {
    let mut paint = PAINTSTRUCT::default();
    unsafe {
        let hdc = BeginPaint(hwnd, &mut paint);
        if !hdc.is_invalid() {
            draw_window_chrome(hwnd, hdc);
        }
        let _ = EndPaint(hwnd, &paint);
    }
}

fn draw_window_chrome(hwnd: HWND, hdc: HDC) {
    let Some(webtag_key) = window_webtag_key(hwnd) else {
        return;
    };
    if !window_draws_shell_chrome(&webtag_key) {
        return;
    }
    let layout = current_window_layout(&webtag_key);
    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    let rects = compute_chrome_rects(client, &layout);

    fill_rect(hdc, client, ARC_WINDOW_BACKGROUND);
    if rect_width(&rects.panel) > 0 && rect_height(&rects.panel) > 0 {
        fill_round_rect(hdc, rects.panel, ARC_PANEL_BACKGROUND, ARC_PANEL_RADIUS);
    }

    if let (Some(navbar), Some(navbar_rect)) = (&layout.navigation_bar, rects.navigation_bar) {
        draw_navigation_bar(hdc, navbar_rect, navbar);
    }
    if let (Some(tabbar), Some(tabbar_rect)) = (&layout.tab_bar, rects.tab_bar) {
        draw_tab_bar(hdc, tabbar_rect, tabbar);
    }
    draw_panel_activators(hdc, client, &rects, &layout);
}

fn window_draws_shell_chrome(webtag_key: &str) -> bool {
    !matches!(
        window_attachment(webtag_key).map(|attachment| attachment.kind),
        Some(WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. })
    )
}

fn window_webtag_key(hwnd: HWND) -> Option<String> {
    let raw =
        unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA) }
            as *mut WindowUserData;
    if raw.is_null() {
        None
    } else {
        Some(unsafe { (*raw).webtag_key.clone() })
    }
}

fn draw_navigation_bar(hdc: HDC, rect: RECT, navbar: &WindowsNavigationBarLayout) {
    fill_rect(hdc, rect, navbar.background_color);
    draw_bottom_border(hdc, rect, 0xe6e6e6);

    let text_color = navbar.text_color;
    let mut title_rect = RECT {
        left: rect.left + 96,
        top: rect.top,
        right: rect.right - 96,
        bottom: rect.bottom,
    };

    if navbar.show_back_button {
        let back_rect = nav_button_rect(rect, 0);
        draw_text(hdc, "<", back_rect, text_color, DT_CENTER);
        title_rect.left = title_rect.left.max(back_rect.right + 8);
    }
    if navbar.show_home_button {
        let home_rect = nav_button_rect(rect, if navbar.show_back_button { 1 } else { 0 });
        draw_text(hdc, "Home", home_rect, text_color, DT_CENTER);
        title_rect.left = title_rect.left.max(home_rect.right + 8);
    }

    if !navbar.title.trim().is_empty() {
        draw_text(hdc, &navbar.title, title_rect, text_color, DT_CENTER);
    }
}

fn nav_button_rect(navbar: RECT, index: i32) -> RECT {
    let width = 44;
    RECT {
        left: navbar.left + 8 + index * width,
        top: navbar.top,
        right: navbar.left + 8 + (index + 1) * width,
        bottom: navbar.bottom,
    }
}

fn draw_tab_bar(hdc: HDC, rect: RECT, tabbar: &WindowsTabBarLayout) {
    if matches!(
        tabbar.position,
        WindowsTabBarPosition::Left | WindowsTabBarPosition::Right
    ) {
        draw_sidebar_tab_bar(hdc, rect, tabbar);
        return;
    }

    fill_rect(hdc, rect, tabbar.background_color);
    draw_tabbar_border(hdc, rect, tabbar);

    let count = tabbar.items.len();
    if count == 0 {
        return;
    }

    for (index, item) in tabbar.items.iter().enumerate() {
        let item_rect = tab_item_rect(rect, tabbar.position, count, index);
        let selected = tabbar.selected_index == index as i32;
        if selected {
            fill_rect(hdc, inset_rect(item_rect, 4, 5), 0xf3f7ff);
        }

        let text_color = if selected {
            tabbar.selected_color
        } else {
            tabbar.color
        };
        let mut label_rect = inset_rect(item_rect, 6, 4);
        if matches!(tabbar.position, WindowsTabBarPosition::Bottom) {
            label_rect.top += 6;
        }
        draw_text(hdc, &item.text, label_rect, text_color, DT_CENTER);

        if let Some(badge) = item.badge.as_ref().filter(|badge| !badge.is_empty()) {
            draw_badge(hdc, item_rect, badge);
        } else if item.has_red_dot {
            draw_red_dot(hdc, item_rect);
        }
    }
}

fn draw_sidebar_tab_bar(hdc: HDC, rect: RECT, tabbar: &WindowsTabBarLayout) {
    fill_rect(hdc, rect, ARC_SIDEBAR_BACKGROUND);

    let title = if tabbar.app_name.trim().is_empty() {
        "LXAPP".to_string()
    } else {
        tabbar.app_name.to_ascii_uppercase()
    };
    let header_rect = RECT {
        left: rect.left + SIDEBAR_ITEM_INSET + 2,
        top: rect.top + 22,
        right: rect.right - SIDEBAR_ITEM_INSET,
        bottom: rect.top + SIDEBAR_HEADER_HEIGHT,
    };
    draw_text(hdc, &title, header_rect, 0x4f5661, DT_LEFT);

    for (index, item) in tabbar.items.iter().enumerate() {
        let item_rect = sidebar_item_rect(rect, index);
        let selected = tabbar.selected_index == index as i32;
        if selected {
            fill_round_rect(hdc, item_rect, 0xffffff, 8);
            fill_round_rect(
                hdc,
                RECT {
                    left: item_rect.left + 6,
                    top: item_rect.top + 9,
                    right: item_rect.left + 10,
                    bottom: item_rect.bottom - 9,
                },
                tabbar.selected_color,
                3,
            );
        }

        let label_rect = RECT {
            left: item_rect.left + 42,
            top: item_rect.top,
            right: item_rect.right - 8,
            bottom: item_rect.bottom,
        };
        let text_color = if selected { 0x111827 } else { 0x667085 };
        let icon_path = if selected && !item.selected_icon_path.trim().is_empty() {
            &item.selected_icon_path
        } else {
            &item.icon_path
        };
        if !icon_path.trim().is_empty() {
            let icon_rect = centered_icon_rect(
                RECT {
                    left: item_rect.left + 18,
                    top: item_rect.top,
                    right: item_rect.left + 18 + SIDEBAR_ICON_SIZE,
                    bottom: item_rect.bottom,
                },
                SIDEBAR_ICON_SIZE,
            );
            if !draw_icon_from_path(hdc, icon_path, icon_rect, SIDEBAR_ICON_SIZE as u32) {
                draw_text(hdc, "□", icon_rect, text_color, DT_CENTER);
            }
        }
        draw_text(hdc, &item.text, label_rect, text_color, DT_LEFT);

        if let Some(badge) = item.badge.as_ref().filter(|badge| !badge.is_empty()) {
            draw_badge(hdc, item_rect, badge);
        } else if item.has_red_dot {
            draw_red_dot(hdc, item_rect);
        }
    }

    let footer_top = rect.bottom - SIDEBAR_FOOTER_HEIGHT;
    draw_top_border(
        hdc,
        RECT {
            left: rect.left + SIDEBAR_ITEM_INSET,
            top: footer_top,
            right: rect.right - SIDEBAR_ITEM_INSET,
            bottom: rect.bottom,
        },
        0xd6d9de,
    );
}

fn draw_tabbar_border(hdc: HDC, rect: RECT, tabbar: &WindowsTabBarLayout) {
    match tabbar.position {
        WindowsTabBarPosition::Bottom => draw_top_border(hdc, rect, tabbar.border_color),
        WindowsTabBarPosition::Left => draw_right_border(hdc, rect, tabbar.border_color),
        WindowsTabBarPosition::Right => draw_left_border(hdc, rect, tabbar.border_color),
    }
}

fn tab_item_rect(rect: RECT, position: WindowsTabBarPosition, count: usize, index: usize) -> RECT {
    let count_i32 = count.max(1) as i32;
    let index_i32 = index as i32;
    match position {
        WindowsTabBarPosition::Bottom => {
            let width = (rect_width(&rect) / count_i32).max(1);
            let left = rect.left + width * index_i32;
            RECT {
                left,
                top: rect.top,
                right: if index + 1 == count {
                    rect.right
                } else {
                    left + width
                },
                bottom: rect.bottom,
            }
        }
        WindowsTabBarPosition::Left | WindowsTabBarPosition::Right => {
            let height = (rect_height(&rect) / count_i32).max(1);
            let top = rect.top + height * index_i32;
            RECT {
                left: rect.left,
                top,
                right: rect.right,
                bottom: if index + 1 == count {
                    rect.bottom
                } else {
                    top + height
                },
            }
        }
    }
}

fn sidebar_item_rect(rect: RECT, index: usize) -> RECT {
    let top =
        rect.top + SIDEBAR_HEADER_HEIGHT + index as i32 * (SIDEBAR_ITEM_HEIGHT + SIDEBAR_ITEM_GAP);
    normalize_rect(RECT {
        left: rect.left + SIDEBAR_ITEM_INSET,
        top,
        right: rect.right - SIDEBAR_ITEM_INSET,
        bottom: top + SIDEBAR_ITEM_HEIGHT,
    })
}

fn panel_activator_rects(
    client: RECT,
    rects: &ChromeRects,
    layout: &WindowsWindowLayout,
) -> Vec<(String, RECT)> {
    if layout.panel_activators.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(layout.panel_activators.len());

    if let (Some(tabbar), Some(tabbar_rect)) = (&layout.tab_bar, rects.tab_bar)
        && matches!(
            tabbar.position,
            WindowsTabBarPosition::Left | WindowsTabBarPosition::Right
        )
    {
        let footer_top = tabbar_rect.bottom - SIDEBAR_FOOTER_HEIGHT;
        let top = footer_top + (SIDEBAR_FOOTER_HEIGHT - PANEL_ACTIVATOR_SIZE) / 2;
        let mut right = tabbar_rect.right - PANEL_ACTIVATOR_MARGIN;
        for activator in &layout.panel_activators {
            let left = right - PANEL_ACTIVATOR_SIZE;
            if left < tabbar_rect.left + PANEL_ACTIVATOR_MARGIN {
                break;
            }
            out.push((
                activator.id.clone(),
                normalize_rect(RECT {
                    left,
                    top,
                    right,
                    bottom: top + PANEL_ACTIVATOR_SIZE,
                }),
            ));
            right = left - PANEL_ACTIVATOR_GAP;
        }
        return out;
    }

    let bottom_limit = rects
        .tab_bar
        .map(|tabbar| tabbar.top)
        .unwrap_or(client.bottom);
    let left = rects.panel.left + PANEL_ACTIVATOR_MARGIN;
    let mut bottom = bottom_limit - PANEL_ACTIVATOR_MARGIN;

    for activator in &layout.panel_activators {
        let top = bottom - PANEL_ACTIVATOR_SIZE;
        if top < client.top + PANEL_ACTIVATOR_MARGIN {
            break;
        }
        out.push((
            activator.id.clone(),
            normalize_rect(RECT {
                left,
                top,
                right: left + PANEL_ACTIVATOR_SIZE,
                bottom,
            }),
        ));
        bottom = top - PANEL_ACTIVATOR_GAP;
    }

    out
}

fn inset_rect(rect: RECT, dx: i32, dy: i32) -> RECT {
    normalize_rect(RECT {
        left: rect.left + dx,
        top: rect.top + dy,
        right: rect.right - dx,
        bottom: rect.bottom - dy,
    })
}

fn handle_window_chrome_click(hwnd: HWND, webtag_key: &str, point: (i32, i32)) -> bool {
    if !window_draws_shell_chrome(webtag_key) {
        return false;
    }
    let layout = current_window_layout(webtag_key);
    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    let rects = compute_chrome_rects(client, &layout);

    if let (Some(navbar), Some(navbar_rect)) = (&layout.navigation_bar, rects.navigation_bar)
        && rect_contains(&navbar_rect, point)
    {
        if navbar.show_back_button && rect_contains(&nav_button_rect(navbar_rect, 0), point) {
            return invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::NavigationBack);
        }
        let home_index = if navbar.show_back_button { 1 } else { 0 };
        if navbar.show_home_button
            && rect_contains(&nav_button_rect(navbar_rect, home_index), point)
        {
            return invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::NavigationHome);
        }
        return true;
    }

    for (panel_id, rect) in panel_activator_rects(client, &rects, &layout) {
        if rect_contains(&rect, point) {
            return invoke_chrome_event_handler(
                webtag_key,
                WindowsChromeEvent::PanelActivatorClick { panel_id },
            );
        }
    }

    if let (Some(tabbar), Some(tabbar_rect)) = (&layout.tab_bar, rects.tab_bar)
        && rect_contains(&tabbar_rect, point)
    {
        for index in 0..tabbar.items.len() {
            let item_rect = if matches!(
                tabbar.position,
                WindowsTabBarPosition::Left | WindowsTabBarPosition::Right
            ) {
                sidebar_item_rect(tabbar_rect, index)
            } else {
                tab_item_rect(tabbar_rect, tabbar.position, tabbar.items.len(), index)
            };
            if rect_contains(&item_rect, point) {
                return invoke_chrome_event_handler(
                    webtag_key,
                    WindowsChromeEvent::TabBarClick { index },
                );
            }
        }
        return true;
    }

    false
}

fn draw_panel_activators(
    hdc: HDC,
    client: RECT,
    rects: &ChromeRects,
    layout: &WindowsWindowLayout,
) {
    for (panel_id, rect) in panel_activator_rects(client, rects, layout) {
        let active = layout
            .panel_activators
            .iter()
            .find(|item| item.id == panel_id)
            .is_some_and(|item| item.active);
        let fill = if active { 0x111827 } else { 0xffffff };
        let activator = layout
            .panel_activators
            .iter()
            .find(|item| item.id == panel_id);
        let text = activator
            .map(|item| panel_activator_label(&item.label))
            .unwrap_or_else(|| panel_activator_label(&panel_id));
        let text_color = if active { 0xffffff } else { 0x667085 };

        if active {
            fill_round_rect(hdc, rect, fill, 6);
        }
        let icon_rect = centered_icon_rect(rect, PANEL_ACTIVATOR_ICON_SIZE);
        let icon_drawn = activator
            .filter(|item| !item.icon_path.trim().is_empty())
            .is_some_and(|item| {
                draw_icon_from_path(
                    hdc,
                    &item.icon_path,
                    icon_rect,
                    PANEL_ACTIVATOR_ICON_SIZE as u32,
                )
            });
        if !icon_drawn {
            draw_text(hdc, &text, rect, text_color, DT_CENTER);
        }
    }
}

fn panel_activator_label(label: &str) -> String {
    let mut out = String::new();
    for ch in label.chars().filter(|ch| ch.is_ascii_alphanumeric()) {
        out.push(ch.to_ascii_uppercase());
        if out.len() == 2 {
            break;
        }
    }
    if out.is_empty() { "?".to_string() } else { out }
}

fn draw_text(
    hdc: HDC,
    text: &str,
    rect: RECT,
    rgb: u32,
    horizontal: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    if text.is_empty() || rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }

    let mut wide: Vec<u16> = text.encode_utf16().collect();
    let mut rect = rect;
    unsafe {
        let _ = SetBkMode(hdc, TRANSPARENT);
        let _ = SetTextColor(hdc, rgb_to_colorref(rgb));
        let _ = DrawTextW(
            hdc,
            &mut wide,
            &mut rect,
            horizontal | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        );
    }
}

fn draw_badge(hdc: HDC, item_rect: RECT, badge: &str) {
    let badge_rect = RECT {
        left: item_rect.right - 30,
        top: item_rect.top + 7,
        right: item_rect.right - 8,
        bottom: item_rect.top + 25,
    };
    fill_rect(hdc, badge_rect, 0xff3b30);
    draw_text(hdc, badge, badge_rect, 0xffffff, DT_CENTER);
}

fn draw_red_dot(hdc: HDC, item_rect: RECT) {
    let dot_rect = RECT {
        left: item_rect.right - 18,
        top: item_rect.top + 9,
        right: item_rect.right - 10,
        bottom: item_rect.top + 17,
    };
    fill_rect(hdc, dot_rect, 0xff3b30);
}

fn draw_top_border(hdc: HDC, rect: RECT, rgb: u32) {
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.top + 1,
        },
        rgb,
    );
}

fn draw_bottom_border(hdc: HDC, rect: RECT, rgb: u32) {
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: rect.bottom - 1,
            right: rect.right,
            bottom: rect.bottom,
        },
        rgb,
    );
}

fn draw_left_border(hdc: HDC, rect: RECT, rgb: u32) {
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: rect.top,
            right: rect.left + 1,
            bottom: rect.bottom,
        },
        rgb,
    );
}

fn draw_right_border(hdc: HDC, rect: RECT, rgb: u32) {
    fill_rect(
        hdc,
        RECT {
            left: rect.right - 1,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        },
        rgb,
    );
}

fn fill_rect(hdc: HDC, rect: RECT, rgb: u32) {
    if rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }
    unsafe {
        let brush = CreateSolidBrush(rgb_to_colorref(rgb));
        if brush.is_invalid() {
            return;
        }
        let _ = FillRect(hdc, &rect, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

fn fill_round_rect(hdc: HDC, rect: RECT, rgb: u32, radius: i32) {
    if rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }
    unsafe {
        let brush = CreateSolidBrush(rgb_to_colorref(rgb));
        if brush.is_invalid() {
            return;
        }
        let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
        let pen = GetStockObject(NULL_PEN);
        let old_pen = SelectObject(hdc, pen);
        let _ = RoundRect(
            hdc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            radius,
            radius,
        );
        if !old_pen.is_invalid() {
            let _ = SelectObject(hdc, old_pen);
        }
        if !old_brush.is_invalid() {
            let _ = SelectObject(hdc, old_brush);
        }
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

fn centered_icon_rect(rect: RECT, size: i32) -> RECT {
    let left = rect.left + (rect_width(&rect) - size).max(0) / 2;
    let top = rect.top + (rect_height(&rect) - size).max(0) / 2;
    normalize_rect(RECT {
        left,
        top,
        right: left + size,
        bottom: top + size,
    })
}

fn draw_icon_from_path(hdc: HDC, path: &str, rect: RECT, size: u32) -> bool {
    let Some(handle) = cached_png_icon_handle(path, size) else {
        return false;
    };
    unsafe {
        WindowsAndMessaging::DrawIconEx(
            hdc,
            rect.left,
            rect.top,
            hicon(handle),
            rect_width(&rect),
            rect_height(&rect),
            0,
            None,
            WindowsAndMessaging::DI_NORMAL,
        )
        .is_ok()
    }
}

fn cached_png_icon_handle(path: &str, size: u32) -> Option<isize> {
    let path = PathBuf::from(path);
    let key = (path.clone(), size);
    let handles = PANEL_ICON_HANDLES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handles) = handles.lock() {
        if let Some(handle) = handles.get(&key) {
            return *handle;
        }
        let handle = create_icon_from_png(&path, size).ok();
        handles.insert(key, handle);
        return handle;
    }
    create_icon_from_png(&path, size).ok()
}

fn rgb_to_colorref(rgb: u32) -> COLORREF {
    let r = (rgb >> 16) & 0xff;
    let g = (rgb >> 8) & 0xff;
    let b = rgb & 0xff;
    COLORREF(r | (g << 8) | (b << 16))
}

fn create_environment(
    effective_options: &EffectiveWebViewCreateOptions,
) -> StdResult<ICoreWebView2Environment> {
    let options = CoreWebView2EnvironmentOptions::default();
    let custom_schemes = webview2_custom_schemes(&effective_options.registered_schemes);

    unsafe {
        let registrations = custom_schemes
            .into_iter()
            .map(|scheme| {
                let registration = CoreWebView2CustomSchemeRegistration::new(scheme);
                registration.set_has_authority_component(true);
                registration.set_treat_as_secure(true);
                Some(registration.into())
            })
            .collect();
        options.set_scheme_registrations(registrations);
    }
    let options_iface: ICoreWebView2EnvironmentOptions = options.into();

    let (tx, rx) = mpsc::channel();
    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            CreateCoreWebView2EnvironmentWithOptions(
                windows::core::PCWSTR::null(),
                windows::core::PCWSTR::null(),
                &options_iface,
                &handler,
            )
            .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |result, environment| {
            result?;
            tx.send(environment.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .map_err(|_| windows::core::Error::from(E_POINTER))?;
            Ok(())
        }),
    )
    .map_err(map_webview2_error)?;

    rx.recv()
        .map_err(|_| WebViewError::WebView("Environment callback channel failed".to_string()))?
        .map_err(|err| WebViewError::WebView(format!("Environment creation failed: {err}")))
}

fn registered_request_schemes(registered_schemes: &[String]) -> Vec<String> {
    let mut schemes = if registered_schemes.is_empty() {
        vec!["lx".to_string()]
    } else {
        registered_schemes.to_vec()
    };
    schemes.sort_unstable();
    schemes.dedup();
    schemes
}

fn webview2_custom_schemes(registered_schemes: &[String]) -> Vec<String> {
    registered_request_schemes(registered_schemes)
        .into_iter()
        .filter(|scheme| scheme != "http" && scheme != "https")
        .collect()
}

fn create_controller(
    env: &ICoreWebView2Environment,
    hwnd: HWND,
) -> StdResult<ICoreWebView2Controller> {
    let env = env.clone();
    let (tx, rx) = mpsc::channel();

    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            env.CreateCoreWebView2Controller(hwnd, &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |result, controller| {
            result?;
            tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .map_err(|_| windows::core::Error::from(E_POINTER))?;
            Ok(())
        }),
    )
    .map_err(map_webview2_error)?;

    rx.recv()
        .map_err(|_| WebViewError::WebView("Controller callback channel failed".to_string()))?
        .map_err(|err| WebViewError::WebView(format!("Controller creation failed: {err}")))
}

fn configure_controller(controller: &ICoreWebView2Controller) -> StdResult<()> {
    unsafe {
        controller
            .SetBounds(RECT {
                left: 0,
                top: 0,
                right: 1024,
                bottom: 768,
            })
            .map_err(|err| WebViewError::WebView(format!("SetBounds failed: {err}")))?;
        controller
            .SetIsVisible(true)
            .map_err(|err| WebViewError::WebView(format!("SetIsVisible failed: {err}")))?;
    }
    Ok(())
}

fn configure_settings(webview: &ICoreWebView2) -> StdResult<()> {
    unsafe {
        let settings = webview
            .Settings()
            .map_err(|err| WebViewError::WebView(format!("Settings failed: {err}")))?;
        settings
            .SetIsScriptEnabled(true)
            .map_err(|err| WebViewError::WebView(format!("SetIsScriptEnabled failed: {err}")))?;
        settings
            .SetAreDefaultScriptDialogsEnabled(false)
            .map_err(|err| {
                WebViewError::WebView(format!("SetAreDefaultScriptDialogsEnabled failed: {err}"))
            })?;
        settings.SetIsWebMessageEnabled(true).map_err(|err| {
            WebViewError::WebView(format!("SetIsWebMessageEnabled failed: {err}"))
        })?;
        settings
            .SetIsStatusBarEnabled(false)
            .map_err(|err| WebViewError::WebView(format!("SetIsStatusBarEnabled failed: {err}")))?;
    }
    Ok(())
}

fn install_document_scripts(webview: &ICoreWebView2) -> StdResult<()> {
    let script = r#"
        (function() {
            if (window.__LingXiaWindowsInjected) return;
            window.__LingXiaWindowsInjected = true;

            if (window.chrome && window.chrome.webview && !window.__LingXiaNativeMessageListener) {
                window.__LingXiaNativeMessageListener = true;
                window.chrome.webview.addEventListener('message', function(event) {
                    try {
                        var payload = typeof event.data === 'string' ? event.data : JSON.stringify(event.data);
                        if (typeof window.__LingXiaRecvMessage === 'function') {
                            window.__LingXiaRecvMessage(payload);
                        } else {
                            console.warn('[LingXia] __LingXiaRecvMessage not available');
                        }
                    } catch (e) {}
                });
            }

            window.LingXiaProxy = window.LingXiaProxy || {
                supportsMessagePort: function() { return false; },
                getPort: function() { return ''; },
                postMessage: function(message) {
                    window.chrome && window.chrome.webview && window.chrome.webview.postMessage(String(message));
                }
            };

            if (window.__LingXiaConsoleInjected) return;
            window.__LingXiaConsoleInjected = true;
            ['log', 'info', 'warn', 'error', 'debug'].forEach(function(level) {
                var original = console[level];
                console[level] = function() {
                    try {
                        var msg = Array.prototype.map.call(arguments, function(arg) {
                            return typeof arg === 'object' ? JSON.stringify(arg) : String(arg);
                        }).join(' ');
                        window.chrome && window.chrome.webview && window.chrome.webview.postMessage(JSON.stringify({
                            __lingxia_console__: true,
                            level: level,
                            message: msg
                        }));
                    } catch (e) {}
                    if (original) return original.apply(console, arguments);
                };
            });
        })();
    "#;

    let webview = webview.clone();
    let script = script.to_string();
    AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            let script = CoTaskMemPWSTR::from(script.as_str());
            webview
                .AddScriptToExecuteOnDocumentCreated(*script.as_ref().as_pcwstr(), &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(|result, _id| result),
    )
    .map_err(map_webview2_error)?;

    Ok(())
}

fn register_event_handlers(
    env: &ICoreWebView2Environment,
    webview: &ICoreWebView2,
    webtag: WebTag,
    registered_schemes: &[String],
    memory_pages: Arc<Mutex<HashMap<String, Vec<u8>>>>,
) -> StdResult<()> {
    let started_tag = webtag.clone();
    unsafe {
        let mut token = 0;
        webview
            .add_NavigationStarting(
                &NavigationStartingEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };

                    let mut uri = PWSTR::null();
                    args.Uri(&mut uri)?;
                    let uri = CoTaskMemPWSTR::from(uri).to_string();

                    if let Some(webview) = find_webview(&started_tag)
                        && matches!(webview.handle_navigation(&uri), NavigationPolicy::Cancel)
                    {
                        args.SetCancel(true)?;
                        return Ok(());
                    }

                    if let Some(delegate) = find_webview_delegate(&started_tag) {
                        delegate.on_page_started();
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_NavigationStarting failed: {err}"))
            })?;
    }

    let finished_tag = webtag.clone();
    unsafe {
        let mut token = 0;
        webview
            .add_NavigationCompleted(
                &NavigationCompletedEventHandler::create(Box::new(move |_sender, _args| {
                    if let Some(delegate) = find_webview_delegate(&finished_tag) {
                        delegate.on_page_finished();
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_NavigationCompleted failed: {err}"))
            })?;
    }

    let new_window_tag = webtag.clone();
    unsafe {
        let mut token = 0;
        webview
            .add_NewWindowRequested(
                &NewWindowRequestedEventHandler::create(Box::new(move |sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };

                    let uri = take_request_string(|slot| args.Uri(slot))?;
                    let Some(webview) = find_webview(&new_window_tag) else {
                        args.SetHandled(true)?;
                        return Ok(());
                    };

                    match webview.handle_new_window(&uri) {
                        NewWindowPolicy::LoadInSelf => {
                            if let Some(sender) = sender {
                                let uri = CoTaskMemPWSTR::from(uri.as_str());
                                sender.Navigate(*uri.as_ref().as_pcwstr())?;
                            }
                            args.SetHandled(true)?;
                        }
                        NewWindowPolicy::Cancel => {
                            args.SetHandled(true)?;
                        }
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_NewWindowRequested failed: {err}"))
            })?;
    }

    let download_tag = webtag.clone();
    unsafe {
        let webview4: ICoreWebView2_4 = webview.cast().map_err(|err| {
            WebViewError::WebView(format!("WebView2_4 cast failed for downloads: {err}"))
        })?;
        let mut token = 0;
        webview4
            .add_DownloadStarting(
                &DownloadStartingEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let Some(webview) = find_webview(&download_tag) else {
                        return Ok(());
                    };
                    if !webview.has_download_handler() {
                        return Ok(());
                    }

                    let operation = args.DownloadOperation()?;
                    let request = download_request_from_operation(&operation)?;
                    webview.handle_download(request);
                    args.SetCancel(true)?;
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| WebViewError::WebView(format!("add_DownloadStarting failed: {err}")))?;
    }

    let message_tag = webtag.clone();
    unsafe {
        let mut token = 0;
        webview
            .add_WebMessageReceived(
                &WebMessageReceivedEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };

                    let mut message = PWSTR::null();
                    args.TryGetWebMessageAsString(&mut message)?;
                    let payload = CoTaskMemPWSTR::from(message).to_string();

                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&payload)
                        && json
                            .get("__lingxia_console__")
                            .and_then(|value| value.as_bool())
                            .unwrap_or(false)
                    {
                        if let (Some(level), Some(message)) = (
                            json.get("level").and_then(|value| value.as_str()),
                            json.get("message").and_then(|value| value.as_str()),
                        ) && let Some(delegate) = find_webview_delegate(&message_tag)
                        {
                            let level = match level {
                                "error" => LogLevel::Error,
                                "warn" => LogLevel::Warn,
                                "debug" => LogLevel::Debug,
                                "info" => LogLevel::Info,
                                _ => LogLevel::Info,
                            };
                            delegate.log(level, message);
                        }
                        return Ok(());
                    }

                    if let Some(delegate) = find_webview_delegate(&message_tag) {
                        let _ = thread::Builder::new()
                            .name(format!("lingxia-web-message-{}", message_tag.key()))
                            .spawn(move || delegate.handle_post_message(payload));
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_WebMessageReceived failed: {err}"))
            })?;
    }

    for scheme in registered_request_schemes(registered_schemes) {
        let filter = format!("{scheme}://*");
        let filter = CoTaskMemPWSTR::from(filter.as_str());
        unsafe {
            webview
                .AddWebResourceRequestedFilter(
                    *filter.as_ref().as_pcwstr(),
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                )
                .map_err(|err| {
                    WebViewError::WebView(format!(
                        "AddWebResourceRequestedFilter failed for {scheme}: {err}"
                    ))
                })?;
        }
    }

    let request_tag = webtag;
    let env = env.clone();
    let memory_pages = memory_pages.clone();
    unsafe {
        let mut token = 0;
        webview
            .add_WebResourceRequested(
                &WebResourceRequestedEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };

                    let request = args.Request()?;
                    let uri = take_request_string(|slot| request.Uri(slot))?;
                    let method = take_request_string(|slot| request.Method(slot))?;
                    if let Some(html) = find_memory_page(&memory_pages, &uri) {
                        let native_response = build_memory_html_response(&env, html)?;
                        args.SetResponse(&native_response)?;
                        return Ok(());
                    }

                    let body = request
                        .Content()
                        .ok()
                        .and_then(|stream| read_stream_to_end(&stream).ok())
                        .unwrap_or_default();

                    let mut http_request = Request::builder()
                        .method(method.as_str())
                        .uri(uri.as_str())
                        .body(body)
                        .map_err(http_error_to_win)?;
                    populate_request_headers(&request, http_request.headers_mut())?;

                    let scheme = request_scheme(&uri);
                    let response = find_webview(&request_tag)
                        .and_then(|webview| webview.handle_scheme_request(scheme, http_request))
                        .unwrap_or_else(not_found_response);

                    let native_response = build_webview2_response(&env, response)?;
                    args.SetResponse(&native_response)?;
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_WebResourceRequested failed: {err}"))
            })?;
    }

    Ok(())
}

fn message_loop(state: &mut UiState, command_rx: Receiver<UiCommand>) -> StdResult<()> {
    let mut msg = MSG::default();

    loop {
        while let Ok(command) = command_rx.try_recv() {
            if handle_command(state, command)? {
                cleanup_state(state);
                return Ok(());
            }
        }

        let status = unsafe { WindowsAndMessaging::GetMessageW(&mut msg, None, 0, 0).0 };
        match status {
            -1 => {
                cleanup_state(state);
                return Err(WebViewError::WebView(
                    "GetMessageW failed in WebView loop".to_string(),
                ));
            }
            0 => {
                cleanup_state(state);
                return Ok(());
            }
            _ => {
                if msg.message == WM_LINGXIA_LAYOUT {
                    let _ = sync_controller_bounds(state);
                    layout_group_for_state(state);
                    store_current_window_placement(state);
                } else if msg.message != WM_LINGXIA_COMMAND {
                    if msg.message == WindowsAndMessaging::WM_SIZE {
                        let _ = sync_controller_bounds(state);
                        layout_group_for_state(state);
                        store_current_window_placement(state);
                    } else if msg.message == WindowsAndMessaging::WM_MOVE {
                        store_current_window_placement(state);
                    }
                    unsafe {
                        let _ = WindowsAndMessaging::TranslateMessage(&msg);
                        WindowsAndMessaging::DispatchMessageW(&msg);
                    }
                }
            }
        }
    }
}

fn handle_command(state: &mut UiState, command: UiCommand) -> StdResult<bool> {
    match command {
        UiCommand::LoadUrl { url, resp } => {
            clear_memory_pages(&state.memory_pages);
            let result = unsafe {
                let url = CoTaskMemPWSTR::from(url.as_str());
                state
                    .webview
                    .Navigate(*url.as_ref().as_pcwstr())
                    .map_err(|err| WebViewError::WebView(format!("Navigate failed: {err}")))
            };
            let _ = resp.send(result);
        }
        UiCommand::LoadHtml {
            html,
            base_url,
            history_url,
            resp,
        } => {
            let navigation_url = history_url.unwrap_or_else(|| base_url.clone());
            clear_memory_pages(&state.memory_pages);
            store_memory_page(
                &state.memory_pages,
                &navigation_url,
                prepare_navigation_html(&html, &base_url, &navigation_url),
            );
            if navigation_url != base_url {
                store_memory_page(&state.memory_pages, &base_url, html.into_bytes());
            }
            let result = unsafe {
                let url = CoTaskMemPWSTR::from(navigation_url.as_str());
                state
                    .webview
                    .Navigate(*url.as_ref().as_pcwstr())
                    .map_err(|err| WebViewError::WebView(format!("Navigate failed: {err}")))
            };
            let _ = resp.send(result);
        }
        UiCommand::ExecJs { js, resp } => {
            let result = execute_script_json(&state.webview, &js)
                .map(|_| ())
                .map_err(|err| WebViewError::WebView(format!("ExecuteScript failed: {err}")));
            let _ = resp.send(result);
        }
        UiCommand::EvalJs { js, resp } => {
            let result = execute_script_json(&state.webview, &js)
                .and_then(|json| decode_script_result(&json));
            let _ = resp.send(result);
        }
        UiCommand::PostMessage { message, resp } => {
            let result = unsafe {
                let message = CoTaskMemPWSTR::from(message.as_str());
                state
                    .webview
                    .PostWebMessageAsString(*message.as_ref().as_pcwstr())
                    .map_err(|err| {
                        WebViewError::WebView(format!("PostWebMessageAsString failed: {err}"))
                    })
            };
            let _ = resp.send(result);
        }
        UiCommand::SetUserAgent { ua, resp } => {
            let result = set_user_agent(&state.webview, &ua);
            let _ = resp.send(result);
        }
        UiCommand::ClearBrowsingData { resp } => {
            let result = clear_browsing_data(&state.webview);
            let _ = resp.send(result);
        }
        UiCommand::CurrentUrl { resp } => {
            let result = current_url(&state.webview);
            let _ = resp.send(result);
        }
        UiCommand::Reload { resp } => {
            let result = unsafe {
                state
                    .webview
                    .Reload()
                    .map_err(|err| WebViewError::WebView(format!("Reload failed: {err}")))
            };
            let _ = resp.send(result);
        }
        UiCommand::GoBack { resp } => {
            let result = go_history(&state.webview, HistoryDirection::Back);
            let _ = resp.send(result);
        }
        UiCommand::GoForward { resp } => {
            let result = go_history(&state.webview, HistoryDirection::Forward);
            let _ = resp.send(result);
        }
        UiCommand::TakeScreenshot { resp } => {
            let result = capture_preview_png(&state.webview);
            let _ = resp.send(result);
        }
        UiCommand::WindowSnapshot { resp } => {
            let result = window_snapshot(state);
            let _ = resp.send(result);
        }
        UiCommand::ShowWindow {
            title,
            activate,
            role,
            resp,
        } => {
            let result = show_native_window(state, &title, activate, role);
            let _ = resp.send(result);
        }
        UiCommand::HideWindow { resp } => {
            let result = hide_native_window(state);
            let _ = resp.send(result);
        }
        UiCommand::SetWindowLayout { layout, resp } => {
            let result = set_native_window_layout(state, layout);
            let _ = resp.send(result);
        }
        UiCommand::Shutdown => return Ok(true),
    }

    Ok(false)
}

fn cleanup_state(state: &mut UiState) {
    cleanup_window_state(state);
    unsafe {
        let _ = state.controller.Close();
        let _ = WindowsAndMessaging::DestroyWindow(state.hwnd);
    }
}

fn cleanup_window_state(state: &UiState) {
    let attachment = remove_window_attachment(&state.webtag_key);
    remove_window_handle(&state.webtag_key);
    remove_window_layout(&state.webtag_key);
    remove_close_handler(&state.webtag_key);
    remove_chrome_event_handler(&state.webtag_key);

    if let Some(attachment) = attachment {
        match attachment.kind {
            WindowAttachmentKind::MainHost => {
                if let Some(hosts) = WINDOW_GROUP_HOSTS.get()
                    && let Ok(mut hosts) = hosts.lock()
                    && hosts.get(&attachment.group_key).copied() == Some(hwnd_handle(state.hwnd))
                {
                    hosts.remove(&attachment.group_key);
                }
                if let Some(active) = WINDOW_GROUP_ACTIVE_MAIN.get()
                    && let Ok(mut active) = active.lock()
                {
                    active.remove(&attachment.group_key);
                }
                if let Some(active_group) = WINDOW_ACTIVE_GROUP.get()
                    && let Ok(mut active_group) = active_group.lock()
                    && active_group.as_deref() == Some(attachment.group_key.as_str())
                {
                    *active_group = None;
                }
                remove_group_layout(&attachment.group_key);
            }
            WindowAttachmentKind::MainChild => {
                if let Some(active) = WINDOW_GROUP_ACTIVE_MAIN.get()
                    && let Ok(mut active) = active.lock()
                    && active
                        .get(&attachment.group_key)
                        .is_some_and(|key| key == &state.webtag_key)
                {
                    active.remove(&attachment.group_key);
                }
            }
            WindowAttachmentKind::Panel { .. } => {
                remove_group_panel(&attachment.group_key, &state.webtag_key);
                layout_group_windows(&attachment.group_key);
                request_group_shell_refresh(&attachment.group_key);
            }
        }
    }
}

fn show_native_window(
    state: &mut UiState,
    _title: &str,
    activate: bool,
    role: WindowsWindowRole,
) -> StdResult<()> {
    match role {
        WindowsWindowRole::Main => show_native_main_window(state, activate),
        WindowsWindowRole::Panel { panel_id } => show_native_panel_window(state, &panel_id),
    }
}

fn show_native_main_window(state: &mut UiState, activate: bool) -> StdResult<()> {
    let (group_key, host, is_host) = ensure_main_attachment(state);
    set_active_group(&group_key);
    set_group_active_main(&group_key, &state.webtag_key);
    show_shell_host(&group_key, host, activate);

    if is_host {
        set_controller_visible(state, true)?;
        sync_controller_bounds(state)?;
    } else {
        attach_child_window_to_host(state.hwnd, host);
        set_controller_visible(state, true)?;
        layout_group_windows(&group_key);
    }

    request_group_shell_refresh(&group_key);
    state.window_visible = true;
    store_current_window_placement(state);
    Ok(())
}

fn show_native_panel_window(state: &mut UiState, panel_id: &str) -> StdResult<()> {
    register_window_handle(&state.webtag_key, state.hwnd);
    let group_key = active_group_key().unwrap_or_else(|| webtag_group_key(&state.webtag_key));
    let Some(host) = host_handle_for_group(&group_key) else {
        return show_native_main_window(state, true);
    };
    let position = panel_position_for_group(&group_key, panel_id);
    attach_child_window_to_host(state.hwnd, host);
    set_window_attachment(
        &state.webtag_key,
        WindowAttachment {
            group_key: group_key.clone(),
            kind: WindowAttachmentKind::Panel {
                panel_id: panel_id.to_string(),
                position,
            },
        },
    );
    register_group_panel(
        &group_key,
        GroupPanel {
            webtag_key: state.webtag_key.clone(),
            panel_id: panel_id.to_string(),
            position,
        },
    );
    set_controller_visible(state, true)?;
    layout_group_windows(&group_key);
    request_group_shell_refresh(&group_key);
    state.window_visible = true;
    Ok(())
}

fn hide_native_window(state: &mut UiState) -> StdResult<()> {
    store_current_window_placement(state);
    match window_attachment(&state.webtag_key).map(|attachment| attachment.kind) {
        Some(WindowAttachmentKind::MainHost) => hide_native_main_host_window(state),
        Some(WindowAttachmentKind::MainChild) => {
            set_controller_visible(state, false)?;
            hide_attached_window(state.hwnd);
            state.window_visible = false;
            Ok(())
        }
        Some(WindowAttachmentKind::Panel { .. }) => {
            let group_key = layout_group_key_for_webtag(&state.webtag_key);
            set_controller_visible(state, false)?;
            hide_attached_window(state.hwnd);
            remove_group_panel(&group_key, &state.webtag_key);
            layout_group_windows(&group_key);
            request_group_shell_refresh(&group_key);
            state.window_visible = false;
            Ok(())
        }
        None => hide_detached_window(state),
    }
}

fn hide_native_main_host_window(state: &mut UiState) -> StdResult<()> {
    let group_key = layout_group_key_for_webtag(&state.webtag_key);
    if group_active_main(&group_key).as_deref() != Some(state.webtag_key.as_str()) {
        set_controller_visible(state, false)?;
        state.window_visible = false;
        return Ok(());
    }
    hide_detached_window(state)
}

fn hide_detached_window(state: &mut UiState) -> StdResult<()> {
    set_controller_visible(state, false)?;
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            state.hwnd,
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
        );
    }
    state.window_visible = false;
    Ok(())
}

fn set_controller_visible(state: &UiState, visible: bool) -> StdResult<()> {
    unsafe {
        state
            .controller
            .SetIsVisible(visible)
            .map_err(|err| WebViewError::WebView(format!("SetIsVisible failed: {err}")))?;
    }
    Ok(())
}

fn set_native_window_layout(state: &UiState, layout: WindowsWindowLayout) -> StdResult<()> {
    set_window_layout_for_key(&state.webtag_key, layout);
    sync_controller_bounds(state)?;
    if let Some(attachment) = window_attachment(&state.webtag_key)
        && !matches!(attachment.kind, WindowAttachmentKind::Panel { .. })
    {
        layout_group_windows(&attachment.group_key);
        request_group_shell_refresh(&attachment.group_key);
    }
    unsafe {
        let _ = InvalidateRect(Some(state.hwnd), None, false);
    }
    Ok(())
}

fn sync_controller_bounds(state: &UiState) -> StdResult<()> {
    let mut rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(state.hwnd, &mut rect);
        if rect.right <= rect.left || rect.bottom <= rect.top {
            rect = RECT {
                left: 0,
                top: 0,
                right: 1024,
                bottom: 768,
            };
        }
        rect = controller_bounds_for_state(state, rect);
        state
            .controller
            .SetBounds(rect)
            .map_err(|err| WebViewError::WebView(format!("SetBounds failed: {err}")))?;
    }
    Ok(())
}

fn controller_bounds_for_state(state: &UiState, client: RECT) -> RECT {
    match window_attachment(&state.webtag_key) {
        Some(WindowAttachment {
            kind: WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. },
            ..
        }) => normalize_rect(client),
        Some(WindowAttachment {
            group_key,
            kind: WindowAttachmentKind::MainHost,
        }) => {
            let content = compute_content_rect(client, &current_window_layout(&state.webtag_key));
            attached_group_rects(&group_key, state.hwnd)
                .map(|rects| rects.main)
                .unwrap_or(content)
        }
        None => compute_content_rect(client, &current_window_layout(&state.webtag_key)),
    }
}

fn window_snapshot(state: &UiState) -> StdResult<WindowsWebViewWindowSnapshot> {
    let mut window_rect = RECT::default();
    let mut client_rect = RECT::default();
    let mut client_origin = POINT { x: 0, y: 0 };

    let window_id = if let Some(attachment) = window_attachment(&state.webtag_key) {
        if matches!(
            attachment.kind,
            WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. }
        ) {
            let host = host_handle_for_group(&attachment.group_key).unwrap_or(state.hwnd);
            unsafe {
                WindowsAndMessaging::GetWindowRect(host, &mut window_rect)
                    .map_err(|err| WebViewError::WebView(format!("GetWindowRect failed: {err}")))?;
            }
            hwnd_handle(host) as usize
        } else {
            unsafe {
                WindowsAndMessaging::GetWindowRect(state.hwnd, &mut window_rect)
                    .map_err(|err| WebViewError::WebView(format!("GetWindowRect failed: {err}")))?;
            }
            hwnd_handle(state.hwnd) as usize
        }
    } else {
        unsafe {
            WindowsAndMessaging::GetWindowRect(state.hwnd, &mut window_rect)
                .map_err(|err| WebViewError::WebView(format!("GetWindowRect failed: {err}")))?;
        }
        hwnd_handle(state.hwnd) as usize
    };

    unsafe {
        WindowsAndMessaging::GetClientRect(state.hwnd, &mut client_rect)
            .map_err(|err| WebViewError::WebView(format!("GetWindowRect failed: {err}")))?;
        if !ClientToScreen(state.hwnd, &mut client_origin).as_bool() {
            return Err(WebViewError::WebView("ClientToScreen failed".to_string()));
        }
    }

    let content = controller_bounds_for_state(state, client_rect);
    let content_left = client_origin.x - window_rect.left + content.left;
    let content_top = client_origin.y - window_rect.top + content.top;
    let content_width = rect_width(&content) as u32;
    let content_height = rect_height(&content) as u32;

    Ok(WindowsWebViewWindowSnapshot {
        window_id,
        webtag_key: state.webtag_key.clone(),
        visible: state.window_visible
            && unsafe { WindowsAndMessaging::IsWindowVisible(state.hwnd).as_bool() },
        content_left,
        content_top,
        content_width,
        content_height,
    })
}

fn set_user_agent(webview: &ICoreWebView2, ua: &str) -> StdResult<()> {
    unsafe {
        let settings = webview
            .Settings()
            .map_err(|err| WebViewError::WebView(format!("Settings failed: {err}")))?;
        let settings2: ICoreWebView2Settings2 = settings
            .cast()
            .map_err(|err| WebViewError::WebView(format!("Settings2 cast failed: {err}")))?;
        let ua = CoTaskMemPWSTR::from(ua);
        settings2
            .SetUserAgent(*ua.as_ref().as_pcwstr())
            .map_err(|err| WebViewError::WebView(format!("SetUserAgent failed: {err}")))?;
    }
    Ok(())
}

fn clear_browsing_data(webview: &ICoreWebView2) -> StdResult<()> {
    let webview13: ICoreWebView2_13 = webview
        .cast()
        .map_err(|err| WebViewError::WebView(format!("WebView profile cast failed: {err}")))?;
    let profile = unsafe {
        webview13
            .Profile()
            .map_err(|err| WebViewError::WebView(format!("Profile failed: {err}")))?
    };
    let profile2: ICoreWebView2Profile2 = profile
        .cast()
        .map_err(|err| WebViewError::WebView(format!("Profile2 cast failed: {err}")))?;

    let (tx, rx) = mpsc::channel();
    unsafe {
        profile2
            .ClearBrowsingDataAll(&ClearBrowsingDataCompletedHandler::create(Box::new(
                move |result| {
                    tx.send(result)
                        .map_err(|_| windows::core::Error::from(E_POINTER))?;
                    Ok(())
                },
            )))
            .map_err(|err| WebViewError::WebView(format!("ClearBrowsingDataAll failed: {err}")))?;
    }

    rx.recv()
        .map_err(|_| WebViewError::WebView("Clear browsing data callback failed".to_string()))?
        .map_err(|err| WebViewError::WebView(format!("Clear browsing data failed: {err}")))
}

fn current_url(webview: &ICoreWebView2) -> StdResult<Option<String>> {
    unsafe {
        let mut uri = PWSTR::null();
        webview
            .Source(&mut uri)
            .map_err(|err| WebViewError::WebView(format!("Source failed: {err}")))?;
        Ok(non_empty(CoTaskMemPWSTR::from(uri).to_string()))
    }
}

enum HistoryDirection {
    Back,
    Forward,
}

fn go_history(webview: &ICoreWebView2, direction: HistoryDirection) -> StdResult<()> {
    unsafe {
        let mut can_go = BOOL::default();
        match direction {
            HistoryDirection::Back => {
                webview
                    .CanGoBack(&mut can_go)
                    .map_err(|err| WebViewError::WebView(format!("CanGoBack failed: {err}")))?;
                if can_go.as_bool() {
                    webview
                        .GoBack()
                        .map_err(|err| WebViewError::WebView(format!("GoBack failed: {err}")))?;
                }
            }
            HistoryDirection::Forward => {
                webview
                    .CanGoForward(&mut can_go)
                    .map_err(|err| WebViewError::WebView(format!("CanGoForward failed: {err}")))?;
                if can_go.as_bool() {
                    webview
                        .GoForward()
                        .map_err(|err| WebViewError::WebView(format!("GoForward failed: {err}")))?;
                }
            }
        }
    }
    Ok(())
}

fn capture_preview_png(webview: &ICoreWebView2) -> StdResult<Vec<u8>> {
    let stream = unsafe { CreateStreamOnHGlobal(None, true) }
        .map_err(|err| WebViewError::WebView(format!("CreateStreamOnHGlobal failed: {err}")))?;
    let capture_stream = stream.clone();
    let webview = webview.clone();

    CapturePreviewCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            webview
                .CapturePreview(
                    COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                    &capture_stream,
                    &handler,
                )
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(|result| result),
    )
    .map_err(map_webview2_error)?;

    let bytes = read_stream_to_end(&stream)
        .map_err(|err| WebViewError::WebView(format!("read screenshot stream failed: {err}")))?;
    if bytes.is_empty() {
        return Err(WebViewError::WebView(
            "WebView2 screenshot stream was empty".to_string(),
        ));
    }
    Ok(bytes)
}

fn execute_script_json(
    webview: &ICoreWebView2,
    js: &str,
) -> std::result::Result<String, WebViewScriptError> {
    let webview = webview.clone();
    let js = js.to_string();
    let (tx, rx) = mpsc::channel();
    ExecuteScriptCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            let js = CoTaskMemPWSTR::from(js.as_str());
            webview
                .ExecuteScript(*js.as_ref().as_pcwstr(), &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |result, json| match result {
            Ok(()) => {
                let _ = tx.send(Ok(json));
                Ok(())
            }
            Err(err) => {
                let _ = tx.send(Err(WebViewScriptError::Platform(err.to_string())));
                Err(err)
            }
        }),
    )
    .map_err(|err| WebViewScriptError::Platform(err.to_string()))?;

    rx.recv().map_err(|_| WebViewScriptError::Destroyed)?
}

fn decode_script_result(raw: &str) -> std::result::Result<serde_json::Value, WebViewScriptError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(serde_json::Value::Null);
    }
    serde_json::from_str(trimmed).map_err(|err| {
        WebViewScriptError::Platform(format!(
            "WebView2 returned invalid JavaScript result JSON: {err}; raw={trimmed}"
        ))
    })
}

fn prepare_navigation_html(html: &str, base_url: &str, navigation_url: &str) -> Vec<u8> {
    if navigation_url == base_url {
        return html.as_bytes().to_vec();
    }

    inject_base_url(html, base_url).into_bytes()
}

fn store_memory_page(
    memory_pages: &Arc<Mutex<HashMap<String, Vec<u8>>>>,
    url: &str,
    html: Vec<u8>,
) {
    if let Ok(mut pages) = memory_pages.lock() {
        pages.insert(normalize_memory_page_url(url), html);
    }
}

fn clear_memory_pages(memory_pages: &Arc<Mutex<HashMap<String, Vec<u8>>>>) {
    if let Ok(mut pages) = memory_pages.lock() {
        pages.clear();
    }
}

fn find_memory_page(
    memory_pages: &Arc<Mutex<HashMap<String, Vec<u8>>>>,
    url: &str,
) -> Option<Vec<u8>> {
    memory_pages
        .lock()
        .ok()
        .and_then(|pages| pages.get(&normalize_memory_page_url(url)).cloned())
}

fn normalize_memory_page_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

fn request_scheme(url: &str) -> &str {
    url.split_once(':')
        .map(|(scheme, _)| scheme)
        .unwrap_or_default()
}

fn download_request_from_operation(
    operation: &ICoreWebView2DownloadOperation,
) -> WinResult<DownloadRequest> {
    let url = take_request_string(|slot| unsafe { operation.Uri(slot) })?;
    let content_disposition = non_empty(take_request_string(|slot| unsafe {
        operation.ContentDisposition(slot)
    })?);
    let mime_type = non_empty(take_request_string(|slot| unsafe {
        operation.MimeType(slot)
    })?);
    let result_file_path = non_empty(take_request_string(|slot| unsafe {
        operation.ResultFilePath(slot)
    })?);
    let content_length = unsafe {
        let mut total = 0i64;
        operation.TotalBytesToReceive(&mut total)?;
        u64::try_from(total).ok().filter(|value| *value > 0)
    };
    let suggested_filename = result_file_path
        .as_ref()
        .and_then(|path| std::path::Path::new(path).file_name())
        .map(|name| name.to_string_lossy().to_string());

    Ok(DownloadRequest {
        url,
        user_agent: None,
        content_disposition,
        mime_type,
        content_length,
        suggested_filename,
        source_page_url: None,
        cookie: None,
    })
}

fn non_empty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

fn build_memory_html_response(
    env: &ICoreWebView2Environment,
    html: Vec<u8>,
) -> WinResult<ICoreWebView2WebResourceResponse> {
    let response = http::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .header("access-control-allow-origin", "null")
        .body(html)
        .map_err(http_error_to_win)?;
    let (parts, body) = response.into_parts();
    build_webview2_response(env, (parts, body).into())
}

fn inject_base_url(html: &str, base_url: &str) -> String {
    let base_tag = format!(r#"<base href="{}">"#, html_escape(base_url));
    let lower = html.to_lowercase();

    if let Some(pos) = lower.find("</head>") {
        let (before, after) = html.split_at(pos);
        return format!("{before}{base_tag}{after}");
    }

    if let Some(pos) = lower.find("<body")
        && let Some(end) = html[pos..].find('>')
    {
        let insert = pos + end + 1;
        let (before, after) = html.split_at(insert);
        return format!("{before}{base_tag}{after}");
    }

    format!("{base_tag}{html}")
}

fn html_escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;")
}

fn build_webview2_response(
    env: &ICoreWebView2Environment,
    response: WebResourceResponse,
) -> WinResult<ICoreWebView2WebResourceResponse> {
    let (parts, body) = response.into_parts();
    let bytes = materialize_body(body);
    let stream = body_to_stream(&bytes)?;
    let reason = CoTaskMemPWSTR::from(canonical_reason(parts.status).as_str());
    let headers = CoTaskMemPWSTR::from(format_headers(&parts.headers).as_str());

    unsafe {
        env.CreateWebResourceResponse(
            Some(&stream),
            parts.status.as_u16() as i32,
            *reason.as_ref().as_pcwstr(),
            *headers.as_ref().as_pcwstr(),
        )
    }
}

fn materialize_body(body: WebResourceBody) -> Vec<u8> {
    match body {
        WebResourceBody::Bytes(bytes) => bytes,
        WebResourceBody::Path(path) => std::fs::read(path).unwrap_or_default(),
        WebResourceBody::Pipe(reader) => {
            let mut data = Vec::new();
            let mut file = pipe_reader_to_file(reader);
            let _ = file.as_mut().map(|file| file.read_to_end(&mut data));
            data
        }
    }
}

fn pipe_reader_to_file(reader: crate::SystemPipeReader) -> Option<std::fs::File> {
    #[cfg(unix)]
    {
        Some(reader.into_file())
    }
    #[cfg(windows)]
    {
        Some(reader.into_file())
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        let _ = reader;
        None
    }
}

fn body_to_stream(bytes: &[u8]) -> WinResult<IStream> {
    unsafe { SHCreateMemStream(Some(bytes)).ok_or_else(windows::core::Error::from_thread) }
}

fn format_headers(headers: &http::HeaderMap) -> String {
    let mut result = String::new();
    for (name, value) in headers {
        if let Ok(value) = value.to_str() {
            result.push_str(name.as_str());
            result.push_str(": ");
            result.push_str(value);
            result.push_str("\r\n");
        }
    }
    result
}

fn populate_request_headers(
    request: &ICoreWebView2WebResourceRequest,
    headers: &mut http::HeaderMap,
) -> WinResult<()> {
    let native_headers = unsafe { request.Headers()? };
    let iterator = unsafe { native_headers.GetIterator()? };
    let mut has_current = BOOL::default();
    unsafe {
        iterator.HasCurrentHeader(&mut has_current)?;
    }

    while has_current.as_bool() {
        let mut name = PWSTR::null();
        let mut value = PWSTR::null();
        unsafe {
            iterator.GetCurrentHeader(&mut name, &mut value)?;
        }

        let name = CoTaskMemPWSTR::from(name).to_string();
        let value = CoTaskMemPWSTR::from(value).to_string();
        if let (Ok(header_name), Ok(header_value)) = (
            name.parse::<http::header::HeaderName>(),
            value.parse::<http::header::HeaderValue>(),
        ) {
            headers.append(header_name, header_value);
        }

        let mut has_next = BOOL::default();
        unsafe {
            iterator.MoveNext(&mut has_next)?;
        }
        has_current = has_next;
    }

    Ok(())
}

fn read_stream_to_end(stream: &IStream) -> WinResult<Vec<u8>> {
    unsafe {
        let _ = stream.Seek(0, STREAM_SEEK_SET, None);
    }

    let mut result = Vec::new();
    let mut buffer = [0u8; 8192];

    loop {
        let mut bytes_read = 0u32;
        unsafe {
            stream
                .Read(
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    Some(&mut bytes_read),
                )
                .ok()?;
        }

        if bytes_read == 0 {
            break;
        }

        result.extend_from_slice(&buffer[..bytes_read as usize]);
    }

    Ok(result)
}

fn canonical_reason(status: StatusCode) -> String {
    status.canonical_reason().unwrap_or("OK").to_string()
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn not_found_response() -> WebResourceResponse {
    let response = http::Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("content-type", "text/plain; charset=utf-8")
        .body(b"Not Found".to_vec())
        .expect("failed to build fallback response");
    response.into_parts().into()
}

fn take_request_string(getter: impl FnOnce(*mut PWSTR) -> WinResult<()>) -> WinResult<String> {
    let mut value = PWSTR::null();
    getter(&mut value)?;
    Ok(CoTaskMemPWSTR::from(value).to_string())
}

fn http_error_to_win(err: http::Error) -> windows::core::Error {
    windows::core::Error::new(E_POINTER, format!("{err}"))
}

fn map_webview2_error(err: webview2_com::Error) -> WebViewError {
    WebViewError::WebView(format!("{err}"))
}
