//! Windows host-window implementation owned by the Windows SDK layer.

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::sync::{Arc, Mutex, OnceLock};

use lingxia_webview::platform::windows::{
    WindowsWebViewHandler, WindowsWebViewNativeView, WindowsWebViewNativeViewHost,
    find_webview_handler, set_webview_native_view_host,
};
use lingxia_webview::runtime as webview_runtime;
use lingxia_webview::{WebTag, WebViewError};
pub use lingxia_windows_host::{
    WindowsCardDecorator, WindowsChromeAttachedLayout, WindowsChromeAttachedState,
    WindowsChromeCommand, WindowsChromeHit, WindowsChromePanel, WindowsChromePanelLayoutInput,
    WindowsChromeState, WindowsContentRect, WindowsFrameButton, WindowsHostBackend,
    WindowsHostPanelContent, WindowsHostPanelTab, WindowsHostWindow, WindowsPanelPosition,
    WindowsWebViewContentWindow, WindowsWebViewWindowSnapshot, WindowsWindowLayout,
    add_host_window_created_handler, cleanup_webview_state, current_window_layout,
    default_window_size, host_window_created_handlers, set_webview_window_layout,
    set_windows_card_decorator, set_windows_host_backend, webview_chrome_event_handler,
    webview_close_handler, webview_visibility_handler, windows_chrome_renderer,
};
use lingxia_windows_host::{WindowsHostPanelKeyEvent, host_panel_input_handler};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, CreateSolidBrush, DeleteObject, Ellipse, EndPaint, ExcludeClipRect,
    GetMonitorInfoW, HDC, HGDIOBJ, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    PAINTSTRUCT, PS_SOLID, RestoreDC, SaveDC, ScreenToClient, SelectObject,
};
use windows::Win32::System::LibraryLoader;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    self, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
    WS_OVERLAPPEDWINDOW, WS_POPUP, WS_SIZEBOX, WS_SYSMENU,
};
use windows::core::{PCWSTR, w};

type StdResult<T, E = WebViewError> = std::result::Result<T, E>;

static VISIBLE_PANELS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static PANEL_TABS: OnceLock<Mutex<HashMap<String, Vec<WindowsHostPanelTab>>>> = OnceLock::new();
static WEBVIEW_PANELS: OnceLock<Mutex<HashMap<String, WebViewPanelEntry>>> = OnceLock::new();
static HOST_PANELS: OnceLock<Mutex<HashMap<String, HostPanelEntry>>> = OnceLock::new();
static ACTIVE_WEBTAG: OnceLock<Mutex<Option<WebTag>>> = OnceLock::new();
static WEBTAG_WINDOWS: OnceLock<Mutex<HashMap<String, isize>>> = OnceLock::new();
static WEBTAG_VISIBILITY: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
static WEBTAG_CONTENT_BOUNDS: OnceLock<Mutex<HashMap<String, ContentBounds>>> = OnceLock::new();
static HOST_ACTIVE_WEBTAG: OnceLock<Mutex<HashMap<isize, String>>> = OnceLock::new();
static PRESENTED_GROUP_MAIN: OnceLock<Mutex<HashMap<isize, String>>> = OnceLock::new();
static FOCUSED_HOST_PANEL: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static HOST_CHROME_SNAPSHOTS: OnceLock<Mutex<HashMap<isize, HostChromeSnapshot>>> = OnceLock::new();
static CHROME_INTERACTIONS: OnceLock<Mutex<HashMap<isize, ChromeInteraction>>> = OnceLock::new();
static PULL_REFRESH_WEBTAGS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static PULL_REFRESH_TICKS: OnceLock<Mutex<HashMap<isize, u32>>> = OnceLock::new();
const WM_LINGXIA_RUN_CALLBACK: u32 = WindowsAndMessaging::WM_APP + 0x158;
const PULL_REFRESH_TIMER_ID: usize = 0x5A17;
const PULL_REFRESH_TIMER_MS: u32 = 120;
const PULL_REFRESH_SLOT_HEIGHT: i32 = 42;
const PULL_REFRESH_INDICATOR_WIDTH: i32 = 64;
const PULL_REFRESH_INDICATOR_HEIGHT: i32 = 32;
const OVERLAY_MARGIN: i32 = 24;
const OVERLAY_MIN_WIDTH: i32 = 280;
const OVERLAY_MIN_HEIGHT: i32 = 220;
const OVERLAY_DEFAULT_WIDTH: i32 = 460;
const OVERLAY_DEFAULT_HEIGHT: i32 = 560;
const OVERLAY_MAX_WIDTH: i32 = 560;
const OVERLAY_MAX_HEIGHT: i32 = 720;
const RESIZE_BORDER: i32 = 8;

#[derive(Debug, Clone, Copy, Default)]
struct ChromeInteraction {
    frame_button_hover: Option<WindowsFrameButton>,
    frame_button_pressed: Option<WindowsFrameButton>,
}

#[derive(Debug, Clone)]
struct WebViewPanelEntry {
    webtag_key: String,
    position: WindowsPanelPosition,
    requested_size: Option<i32>,
    docked: bool,
    maximized: bool,
}

#[derive(Debug, Clone)]
struct HostPanelEntry {
    title: String,
    body: String,
    position: WindowsPanelPosition,
    requested_size: Option<i32>,
    docked: bool,
    maximized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ContentBounds {
    hwnd: isize,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}

#[derive(Clone)]
struct HostChromeSnapshot {
    layout: WindowsWindowLayout,
    attached: Option<WindowsChromeAttachedLayout>,
}

struct PlatformNativeViewHost;

impl WindowsWebViewNativeViewHost for PlatformNativeViewHost {
    fn create_webview_parent(&self, webtag: &WebTag) -> StdResult<WindowsWebViewNativeView> {
        create_webview_parent_window(webtag)
    }

    fn destroy_webview_parent(&self, _webtag_key: &str, view: WindowsWebViewNativeView) {
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
    set_windows_host_backend(Arc::new(WindowsHostBackendImpl));
}

struct WindowsHostBackendImpl;

impl WindowsHostBackend for WindowsHostBackendImpl {
    fn show_webview_as_panel(&self, webtag: &WebTag, title: &str, panel_id: &str) -> StdResult<()> {
        show_webview_as_panel(webtag, title, panel_id)
    }

    fn present_webview_in_active_group(&self, webtag: &WebTag) -> StdResult<()> {
        present_webview_in_active_group(webtag)
    }

    fn present_webview_as_group_main(&self, webtag: &WebTag, group_key: String) -> StdResult<()> {
        present_webview_as_group_main(webtag, group_key)
    }

    fn present_webview_as_overlay(
        &self,
        webtag: &WebTag,
        width: f64,
        height: f64,
        width_ratio: f64,
        height_ratio: f64,
        position: u8,
    ) -> StdResult<()> {
        present_webview_as_overlay(webtag, width, height, width_ratio, height_ratio, position)
    }

    fn resize_host_window_content(
        &self,
        webtag: &WebTag,
        width: i32,
        height: i32,
    ) -> StdResult<()> {
        resize_host_window_content(webtag, width, height)
    }

    fn restore_presented_group_main(&self) -> StdResult<()> {
        restore_presented_group_main()
    }

    fn show_interactive_host_panel(
        &self,
        panel_id: &str,
        title: &str,
        body: &str,
        position: WindowsPanelPosition,
    ) -> StdResult<()> {
        show_interactive_host_panel(panel_id, title, body, position)
    }

    fn hide_host_panel(&self, panel_id: &str) -> StdResult<()> {
        hide_host_panel(panel_id)
    }

    fn update_host_panel_body(&self, panel_id: &str, body: &str) -> StdResult<()> {
        update_host_panel_body(panel_id, body)
    }

    fn set_host_panel_tabs(&self, panel_id: &str, tabs: Vec<WindowsHostPanelTab>) -> bool {
        set_host_panel_tabs(panel_id, tabs)
    }

    fn set_host_panel_maximized(&self, panel_id: &str, maximized: bool) -> bool {
        set_host_panel_maximized(panel_id, maximized)
    }

    fn invalidate_host_panel(&self, panel_id: &str) -> bool {
        invalidate_host_panel(panel_id)
    }

    fn is_panel_visible(&self, panel_id: &str) -> bool {
        is_panel_visible(panel_id)
    }

    fn find_webview_content_window(&self, webtag: &WebTag) -> Option<WindowsWebViewContentWindow> {
        find_webview_content_window(webtag)
    }

    fn webview_window_snapshot(&self, webtag: &WebTag) -> StdResult<WindowsWebViewWindowSnapshot> {
        webview_window_snapshot(webtag)
    }

    fn show_webview_window(&self, webtag: &WebTag, title: &str, activate: bool) -> StdResult<()> {
        show_webview_window(webtag, title, activate)
    }

    fn navigate_webview_window(
        &self,
        webtag: &WebTag,
        title: &str,
        activate: bool,
    ) -> StdResult<()> {
        navigate_webview_window(webtag, title, activate)
    }

    fn hide_webview_window(&self, webtag: &WebTag) -> StdResult<()> {
        hide_webview_window(webtag)
    }

    fn request_host_window_layout(&self, window: WindowsHostWindow) -> bool {
        request_host_window_layout(window)
    }

    fn active_content_screen_rect(&self) -> Option<WindowsContentRect> {
        active_content_screen_rect()
    }

    fn post_to_window_thread(&self, window: isize, callback: Box<dyn FnOnce() + Send>) -> bool {
        post_to_window_thread(window, callback)
    }

    fn sync_webview_window_layout(&self, webtag: &WebTag) {
        if let Some(hwnd) = window_handle_for_key(webtag.key()) {
            if !should_sync_webview_layout_now(hwnd) {
                return;
            }
            sync_window_layout(hwnd);
            invalidate_window_chrome(hwnd);
        }
    }
}

fn should_sync_webview_layout_now(hwnd: HWND) -> bool {
    if windows_chrome_renderer().is_none() {
        return true;
    }
    match active_host_window() {
        Some(active) => active == hwnd,
        None => true,
    }
}

pub fn show_webview_as_panel(webtag: &WebTag, title: &str, panel_id: &str) -> StdResult<()> {
    let handler = find_webview_handler(webtag).ok_or_else(|| handler_not_ready(webtag))?;
    let Some(host) = active_host_window() else {
        show_webview_window(webtag, title, true)?;
        mark_panel_visible(panel_id, true);
        return Ok(());
    };

    handler.set_content_visible(false)?;
    set_window_handle(webtag.key(), host);
    register_webview_panel(panel_id, webtag, panel_position_for_id(panel_id));
    mark_panel_visible(panel_id, true);
    sync_window_layout(host);
    handler.set_content_visible(true)?;
    notify_webtag_visibility(webtag.key(), true);
    invalidate_window(host);
    Ok(())
}

pub fn present_webview_in_active_group(webtag: &WebTag) -> StdResult<()> {
    let handler = find_webview_handler(webtag).ok_or_else(|| handler_not_ready(webtag))?;
    let Some(host) = active_host_window() else {
        handler.set_content_visible(true)?;
        show_native_view(handler.native_view(), "", true)?;
        let hwnd = hwnd_from_handle(handler.native_view().window);
        set_window_handle(webtag.key(), hwnd);
        set_host_active_webtag(hwnd, webtag.key());
        mark_active(webtag);
        notify_webtag_visibility(webtag.key(), true);
        return Ok(());
    };

    let previous = active_webtag_key_for_window(host);
    let already_active =
        previous.as_deref() == Some(webtag.key()) && webtag_is_visible(webtag.key());
    if already_active {
        sync_window_layout(host);
        invalidate_window_chrome(host);
        return Ok(());
    }

    if let Some(previous) = previous.as_ref()
        && previous != webtag.key()
    {
        let presented = PRESENTED_GROUP_MAIN.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(mut presented) = presented.lock() {
            presented
                .entry(hwnd_handle(host))
                .or_insert_with(|| previous.clone());
        }
    }

    if webtag_is_visible(webtag.key()) {
        handler.set_content_visible(false)?;
    }
    set_window_handle(webtag.key(), host);
    set_host_active_webtag(host, webtag.key());
    sync_window_layout(host);
    handler.set_content_visible(true)?;

    if let Some(previous) = previous
        && previous != webtag.key()
    {
        if let Some(previous_webtag) = webtag_for_key(&previous)
            && let Some(previous_handler) = find_webview_handler(&previous_webtag)
        {
            let _ = previous_handler.set_content_visible(false);
        }
        notify_webtag_visibility(&previous, false);
    }

    if !is_window_visible(host) {
        unsafe {
            let _ = WindowsAndMessaging::SetWindowPos(
                host,
                None,
                0,
                0,
                0,
                0,
                WindowsAndMessaging::SWP_NOMOVE
                    | WindowsAndMessaging::SWP_NOSIZE
                    | WindowsAndMessaging::SWP_SHOWWINDOW,
            );
            let _ = WindowsAndMessaging::BringWindowToTop(host);
            let _ = WindowsAndMessaging::SetForegroundWindow(host);
        }
    }
    mark_active(webtag);
    notify_webtag_visibility(webtag.key(), true);
    invalidate_window_chrome(host);
    Ok(())
}

pub fn present_webview_as_group_main(webtag: &WebTag, _group_key: String) -> StdResult<()> {
    present_webview_in_active_group(webtag)
}

pub fn present_webview_as_overlay(
    webtag: &WebTag,
    width: f64,
    height: f64,
    width_ratio: f64,
    height_ratio: f64,
    position: u8,
) -> StdResult<()> {
    let handler = find_webview_handler(webtag).ok_or_else(|| handler_not_ready(webtag))?;
    let hwnd = hwnd_from_handle(handler.native_view().window);
    let bounds = overlay_reference_rect(hwnd);
    let rect = overlay_rect(bounds, width, height, width_ratio, height_ratio, position);

    unsafe {
        WindowsAndMessaging::SetWindowPos(
            hwnd,
            None,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            WindowsAndMessaging::SWP_SHOWWINDOW,
        )
        .map_err(|err| WebViewError::WebView(format!("SetWindowPos failed: {err}")))?;
        let _ = WindowsAndMessaging::BringWindowToTop(hwnd);
        let _ = WindowsAndMessaging::SetForegroundWindow(hwnd);
    }
    handler.set_content_visible(true)?;
    set_window_handle(webtag.key(), hwnd);
    set_host_active_webtag(hwnd, webtag.key());
    sync_window_layout(hwnd);
    mark_active(webtag);
    notify_webtag_visibility(webtag.key(), true);
    Ok(())
}

fn overlay_reference_rect(hwnd: HWND) -> RECT {
    if let Some(content) = active_content_screen_rect()
        && content.width > OVERLAY_MIN_WIDTH
        && content.height > OVERLAY_MIN_HEIGHT
    {
        return RECT {
            left: content.left,
            top: content.top,
            right: content.left + content.width,
            bottom: content.top + content.height,
        };
    }

    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            return info.rcWork;
        }
    }

    RECT {
        left: 0,
        top: 0,
        right: 1024,
        bottom: 768,
    }
}

fn overlay_rect(
    bounds: RECT,
    width: f64,
    height: f64,
    width_ratio: f64,
    height_ratio: f64,
    position: u8,
) -> RECT {
    let bounds_width = (bounds.right - bounds.left).max(OVERLAY_MIN_WIDTH);
    let bounds_height = (bounds.bottom - bounds.top).max(OVERLAY_MIN_HEIGHT);
    let max_width = (bounds_width - OVERLAY_MARGIN * 2)
        .min(OVERLAY_MAX_WIDTH)
        .max(OVERLAY_MIN_WIDTH);
    let max_height = (bounds_height - OVERLAY_MARGIN * 2)
        .min(OVERLAY_MAX_HEIGHT)
        .max(OVERLAY_MIN_HEIGHT);
    let overlay_width = resolve_overlay_extent(
        width,
        width_ratio,
        bounds_width,
        OVERLAY_DEFAULT_WIDTH,
        OVERLAY_MIN_WIDTH,
        max_width,
    );
    let overlay_height = resolve_overlay_extent(
        height,
        height_ratio,
        bounds_height,
        OVERLAY_DEFAULT_HEIGHT,
        OVERLAY_MIN_HEIGHT,
        max_height,
    );

    let center_x = bounds.left + (bounds_width - overlay_width) / 2;
    let center_y = bounds.top + (bounds_height - overlay_height) / 2;
    let (x, y) = match position {
        1 => (center_x, bounds.bottom - overlay_height - OVERLAY_MARGIN),
        2 => (bounds.left + OVERLAY_MARGIN, center_y),
        3 => (bounds.right - overlay_width - OVERLAY_MARGIN, center_y),
        4 => (center_x, bounds.top + OVERLAY_MARGIN),
        _ => (center_x, center_y),
    };

    let left = x.clamp(bounds.left + OVERLAY_MARGIN, bounds.right - overlay_width);
    let top = y.clamp(bounds.top + OVERLAY_MARGIN, bounds.bottom - overlay_height);
    RECT {
        left,
        top,
        right: left + overlay_width,
        bottom: top + overlay_height,
    }
}

fn resolve_overlay_extent(
    absolute: f64,
    ratio: f64,
    reference: i32,
    fallback: i32,
    min: i32,
    max: i32,
) -> i32 {
    let value = if absolute.is_finite() && absolute > 0.0 {
        absolute.round() as i32
    } else if ratio.is_finite() && ratio > 0.0 {
        (reference as f64 * ratio.clamp(0.1, 1.0)).round() as i32
    } else {
        fallback
    };
    value.clamp(min.min(max), max)
}

pub fn resize_host_window_content(webtag: &WebTag, width: i32, height: i32) -> StdResult<()> {
    if width <= 0 || height <= 0 {
        return Err(WebViewError::WebView(format!(
            "invalid window content size {width}x{height}"
        )));
    }
    let snapshot = webview_window_snapshot(webtag)?;
    let hwnd = hwnd_from_handle(snapshot.window_id as isize);
    let mut window = RECT::default();
    unsafe {
        WindowsAndMessaging::GetWindowRect(hwnd, &mut window)
            .map_err(|err| WebViewError::WebView(format!("GetWindowRect failed: {err}")))?;
    }
    let current_content_width = snapshot.content_width as i32;
    let current_content_height = snapshot.content_height as i32;
    let outer_width = (window.right - window.left).max(1);
    let outer_height = (window.bottom - window.top).max(1);
    let target_outer_width = (outer_width + width - current_content_width).max(width);
    let target_outer_height = (outer_height + height - current_content_height).max(height);
    unsafe {
        WindowsAndMessaging::SetWindowPos(
            hwnd,
            None,
            0,
            0,
            target_outer_width,
            target_outer_height,
            WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOZORDER
                | WindowsAndMessaging::SWP_NOACTIVATE,
        )
        .map_err(|err| WebViewError::WebView(format!("SetWindowPos failed: {err}")))?;
    }
    Ok(())
}

pub fn restore_presented_group_main() -> StdResult<()> {
    let Some(host) = active_host_window() else {
        return Ok(());
    };
    let main_key = PRESENTED_GROUP_MAIN
        .get()
        .and_then(|presented| presented.lock().ok())
        .and_then(|mut presented| presented.remove(&hwnd_handle(host)));
    let Some(main_key) = main_key else {
        return Ok(());
    };

    let current = active_webtag_key_for_window(host);
    let Some(main_webtag) = webtag_for_key(&main_key) else {
        return Ok(());
    };
    let Some(main_handler) = find_webview_handler(&main_webtag) else {
        return Ok(());
    };

    if webtag_is_visible(main_webtag.key()) {
        main_handler.set_content_visible(false)?;
    }
    set_window_handle(main_webtag.key(), host);
    set_host_active_webtag(host, main_webtag.key());
    sync_window_layout(host);
    main_handler.set_content_visible(true)?;

    if let Some(current) = current
        && current != main_webtag.key()
    {
        if let Some(current_webtag) = webtag_for_key(&current)
            && let Some(current_handler) = find_webview_handler(&current_webtag)
        {
            let _ = current_handler.set_content_visible(false);
        }
        notify_webtag_visibility(&current, false);
    }
    mark_active(&main_webtag);
    notify_webtag_visibility(main_webtag.key(), true);
    invalidate_window_chrome(host);
    Ok(())
}

pub fn show_interactive_host_panel(
    panel_id: &str,
    title: &str,
    body: &str,
    position: WindowsPanelPosition,
) -> StdResult<()> {
    let panels = HOST_PANELS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut panels) = panels.lock() {
        panels.insert(
            panel_id.to_string(),
            HostPanelEntry {
                title: title.to_string(),
                body: body.to_string(),
                position,
                requested_size: None,
                docked: matches!(position, WindowsPanelPosition::Bottom),
                maximized: false,
            },
        );
    }
    mark_panel_visible(panel_id, true);
    focus_host_panel(panel_id);
    focus_active_host_window();
    sync_active_host_layout();
    Ok(())
}

pub fn hide_host_panel(panel_id: &str) -> StdResult<()> {
    mark_panel_visible(panel_id, false);
    clear_focused_host_panel(panel_id);
    sync_active_host_layout();
    Ok(())
}

pub fn update_host_panel_body(panel_id: &str, body: &str) -> StdResult<()> {
    if let Some(panels) = HOST_PANELS.get()
        && let Ok(mut panels) = panels.lock()
        && let Some(panel) = panels.get_mut(panel_id)
    {
        panel.body = body.to_string();
    }
    repaint_active_host();
    Ok(())
}

pub fn set_host_panel_tabs(panel_id: &str, tabs: Vec<WindowsHostPanelTab>) -> bool {
    let state = PANEL_TABS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut state) = state.lock() {
        state.insert(panel_id.to_string(), tabs);
        repaint_active_host();
        return true;
    }
    false
}

pub fn set_host_panel_maximized(panel_id: &str, maximized: bool) -> bool {
    let mut updated = false;
    if let Some(panels) = HOST_PANELS.get()
        && let Ok(mut panels) = panels.lock()
        && let Some(panel) = panels.get_mut(panel_id)
    {
        panel.maximized = maximized;
        updated = true;
    }
    if !updated
        && let Some(panels) = WEBVIEW_PANELS.get()
        && let Ok(mut panels) = panels.lock()
        && let Some(panel) = panels.get_mut(panel_id)
    {
        panel.maximized = maximized;
        updated = true;
    }
    if updated {
        sync_active_host_layout();
    }
    updated
}

pub fn invalidate_host_panel(panel_id: &str) -> bool {
    invalidate_active_host_panel(panel_id)
}

pub fn is_panel_visible(panel_id: &str) -> bool {
    VISIBLE_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .is_some_and(|panels| panels.contains(panel_id))
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

fn content_rect_for_window(hwnd: HWND, webtag_key: &str) -> RECT {
    refresh_adjusted_content_rect(webtag_key, base_content_rect_for_window(hwnd, webtag_key))
}

fn base_content_rect_for_window(hwnd: HWND, webtag_key: &str) -> RECT {
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
    if let Some(active_key) = active_webtag_key_for_window(hwnd)
        && let Some(attached) = attached_layout_for_window(hwnd, &active_key, client)
    {
        if webtag_key == active_key {
            return normalize_rect(attached.main);
        }
        if let Some(panel) = attached
            .panels
            .iter()
            .find(|panel| panel.webtag_key == webtag_key)
        {
            return normalize_rect(panel.rect);
        }
    }
    normalize_rect(renderer.content_rect(client, &current_window_layout(webtag_key)))
}

fn refresh_adjusted_content_rect(webtag_key: &str, mut rect: RECT) -> RECT {
    if is_pull_refreshing(webtag_key) && rect.bottom - rect.top > PULL_REFRESH_SLOT_HEIGHT + 80 {
        rect.top = (rect.top + PULL_REFRESH_SLOT_HEIGHT).min(rect.bottom);
    }
    normalize_rect(rect)
}

fn sync_window_layout(hwnd: HWND) {
    let Some(webtag_key) = active_webtag_key_for_window(hwnd) else {
        return;
    };
    sync_webtag_content_bounds(hwnd, &webtag_key);

    let mut visible_webtags = HashSet::from([webtag_key.clone()]);
    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    if let Some(attached) = attached_layout_for_window(hwnd, &webtag_key, client) {
        let mut laid_out_panels = HashSet::new();
        for panel in attached
            .panels
            .iter()
            .filter(|panel| !panel.webtag_key.is_empty())
        {
            laid_out_panels.insert(panel.webtag_key.clone());
            visible_webtags.insert(panel.webtag_key.clone());
            sync_webtag_content_bounds(hwnd, &panel.webtag_key);
        }
        if attached_has_maximized_native_panel(&attached) {
            collapse_obscured_webview_panels(hwnd, &laid_out_panels);
        }
    }
    reconcile_host_webview_visibility(hwnd, &visible_webtags);
}

fn notify_window_position_changed(hwnd: HWND) {
    let Some(webtag_key) = active_webtag_key_for_window(hwnd) else {
        return;
    };
    let Some(webtag) = webtag_for_key(&webtag_key) else {
        return;
    };
    let Some(handler) = find_webview_handler(&webtag) else {
        return;
    };
    let _ = handler.notify_parent_position_changed();
}

fn sync_webtag_content_bounds(hwnd: HWND, webtag_key: &str) {
    let rect = content_rect_for_window(hwnd, webtag_key);
    sync_webtag_content_bounds_to_rect(hwnd, webtag_key, rect);
}

fn sync_webtag_content_bounds_to_rect(hwnd: HWND, webtag_key: &str, rect: RECT) {
    let width = (rect.right - rect.left).max(0);
    let height = (rect.bottom - rect.top).max(0);
    let host_bounds = ContentBounds {
        hwnd: hwnd_handle(hwnd),
        left: rect.left,
        top: rect.top,
        width,
        height,
    };
    let Some(webtag) = webtag_for_key(webtag_key) else {
        return;
    };
    let Some(handler) = find_webview_handler(&webtag) else {
        return;
    };
    let surface = hwnd_from_handle(handler.native_view().window);
    if surface != hwnd
        && let Err(err) = handler.set_parent_window(hwnd_handle(hwnd))
    {
        log::debug!("Failed to set Windows WebView parent for {webtag_key}: {err}");
    }
    let controller_bounds = rect;
    if webtag_content_bounds_changed(webtag_key, host_bounds) {
        if let Err(err) = handler.set_content_bounds(
            controller_bounds.left,
            controller_bounds.top,
            controller_bounds.right - controller_bounds.left,
            controller_bounds.bottom - controller_bounds.top,
        ) {
            log::debug!("Failed to sync Windows WebView content bounds: {err}");
        }
    }
    let _ = handler.notify_parent_position_changed();
}

fn attached_has_maximized_native_panel(attached: &WindowsChromeAttachedLayout) -> bool {
    attached
        .panels
        .iter()
        .any(|panel| panel.webtag_key.is_empty() && host_panel_is_maximized(&panel.panel_id))
}

fn collapse_obscured_webview_panels(hwnd: HWND, laid_out: &HashSet<String>) {
    let Some(panels) = WEBVIEW_PANELS.get() else {
        return;
    };
    let visible = VISIBLE_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .map(|panels| panels.clone())
        .unwrap_or_default();
    let Ok(panels) = panels.lock() else {
        return;
    };
    for (panel_id, panel) in panels.iter() {
        if visible.contains(panel_id) && !laid_out.contains(&panel.webtag_key) {
            sync_webtag_content_bounds_to_rect(hwnd, &panel.webtag_key, RECT::default());
        }
    }
}

fn reconcile_host_webview_visibility(hwnd: HWND, visible_webtags: &HashSet<String>) {
    let host = hwnd_handle(hwnd);
    let host_visible = is_window_visible(hwnd) && !is_minimized(hwnd);
    let mut to_show = Vec::new();
    let mut to_hide = Vec::new();

    for webtag in lingxia_webview::runtime::list_webviews() {
        let key = webtag.key().to_string();
        let Some(window) = window_handle_for_key(&key) else {
            continue;
        };
        if hwnd_handle(window) != host {
            continue;
        }
        let Some(handler) = find_webview_handler(&webtag) else {
            continue;
        };
        let surface = hwnd_from_handle(handler.native_view().window);
        let should_show = host_visible && visible_webtags.contains(&key);
        match (should_show, webtag_is_visible(&key)) {
            (true, false) => to_show.push((key, handler, surface)),
            (false, true) => to_hide.push((key, handler, surface)),
            _ => {}
        }
    }

    let host_surface_incoming = to_show.iter().any(|(_, _, surface)| *surface == hwnd);
    if host_surface_incoming {
        for (key, handler, _) in to_hide {
            hide_reconciled_webview(&key, &handler);
        }
        for (key, handler, _) in to_show {
            show_reconciled_webview(&key, &handler);
        }
    } else {
        // Show the incoming child surface before hiding the outgoing one.
        // WebView2 controller visibility changes are not atomic across
        // controllers; hiding first exposes the host background for a frame
        // and reads as flicker.
        for (key, handler, _) in to_show {
            show_reconciled_webview(&key, &handler);
        }
        for (key, handler, _) in to_hide {
            hide_reconciled_webview(&key, &handler);
        }
    }
}

fn show_reconciled_webview(key: &str, handler: &WindowsWebViewHandler) {
    if let Err(err) = handler.set_content_visible(true) {
        log::debug!("failed to show Windows WebView during visibility reconcile for {key}: {err}");
        return;
    }
    let _ = handler.notify_parent_position_changed();
    notify_webtag_visibility(key, true);
}

fn hide_reconciled_webview(key: &str, handler: &WindowsWebViewHandler) {
    if let Err(err) = handler.set_content_visible(false) {
        log::debug!("failed to hide Windows WebView during visibility reconcile for {key}: {err}");
        return;
    }
    notify_webtag_visibility(key, false);
}

fn webtag_content_bounds_changed(webtag_key: &str, bounds: ContentBounds) -> bool {
    let state = WEBTAG_CONTENT_BOUNDS.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut state) = state.lock() else {
        return true;
    };
    match state.get(webtag_key) {
        Some(previous) if *previous == bounds => false,
        _ => {
            state.insert(webtag_key.to_string(), bounds);
            true
        }
    }
}

fn handle_window_position_changed(hwnd: HWND, lparam: LPARAM) {
    let pos = lparam.0 as *const WindowsAndMessaging::WINDOWPOS;
    if pos.is_null() {
        sync_window_layout(hwnd);
        return;
    }
    let flags = unsafe { (*pos).flags };
    let moved = flags.0 & WindowsAndMessaging::SWP_NOMOVE.0 == 0;
    let sized = flags.0 & WindowsAndMessaging::SWP_NOSIZE.0 == 0;
    if sized {
        sync_window_layout(hwnd);
        if windows_chrome_renderer().is_some() {
            invalidate_window(hwnd);
        }
    } else if moved {
        notify_window_position_changed(hwnd);
    }
}

fn webtag_for_key(webtag_key: &str) -> Option<WebTag> {
    lingxia_webview::runtime::list_webviews()
        .into_iter()
        .find(|webtag| webtag.key() == webtag_key)
}

fn attached_layout_for_window(
    hwnd: HWND,
    active_webtag_key: &str,
    client: RECT,
) -> Option<WindowsChromeAttachedLayout> {
    let renderer = windows_chrome_renderer()?;
    let layout = current_window_layout(active_webtag_key);
    let panels = visible_panel_layout_inputs();
    if panels.is_empty() {
        return None;
    }
    renderer
        .attached_layout(client, &layout, &panels)
        .map(|mut attached| {
            attached.main = normalize_rect(attached.main);
            for panel in &mut attached.panels {
                panel.rect = normalize_rect(panel.rect);
            }
            let _ = hwnd;
            attached
        })
}

fn attached_state_for_window(
    hwnd: HWND,
    active_webtag_key: &str,
    client: RECT,
) -> Option<WindowsChromeAttachedState> {
    let layout = attached_layout_for_window(hwnd, active_webtag_key, client)?;
    let panels = layout
        .panels
        .into_iter()
        .map(|panel| {
            let panel_id = panel.panel_id.clone();
            WindowsChromePanel {
                panel_id: panel_id.clone(),
                rect: panel.rect,
                host_content: host_panel_content(&panel_id),
                docked: panel_is_docked(&panel_id),
            }
        })
        .collect();
    Some(WindowsChromeAttachedState {
        main: layout.main,
        panels,
    })
}

fn visible_panel_layout_inputs() -> Vec<WindowsChromePanelLayoutInput> {
    let mut out = Vec::new();
    let visible = VISIBLE_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .map(|panels| panels.clone())
        .unwrap_or_default();

    if let Some(panels) = WEBVIEW_PANELS.get()
        && let Ok(panels) = panels.lock()
    {
        for (panel_id, panel) in panels.iter() {
            if visible.contains(panel_id) {
                out.push(WindowsChromePanelLayoutInput {
                    panel_id: panel_id.clone(),
                    webtag_key: panel.webtag_key.clone(),
                    position: panel.position,
                    requested_size: panel.requested_size,
                    docked: panel.docked,
                    maximized: panel.maximized,
                });
            }
        }
    }

    if let Some(panels) = HOST_PANELS.get()
        && let Ok(panels) = panels.lock()
    {
        for (panel_id, panel) in panels.iter() {
            if visible.contains(panel_id) {
                out.push(WindowsChromePanelLayoutInput {
                    panel_id: panel_id.clone(),
                    webtag_key: String::new(),
                    position: panel.position,
                    requested_size: panel.requested_size,
                    docked: panel.docked,
                    maximized: panel.maximized,
                });
            }
        }
    }

    out
}

fn host_panel_content(panel_id: &str) -> Option<WindowsHostPanelContent> {
    let entry = HOST_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .and_then(|panels| panels.get(panel_id).cloned())?;
    let tabs = PANEL_TABS
        .get()
        .and_then(|tabs| tabs.lock().ok())
        .and_then(|tabs| tabs.get(panel_id).cloned())
        .unwrap_or_default();
    Some(WindowsHostPanelContent {
        title: Some(entry.title),
        body: Some(entry.body),
        tabs,
        maximized: entry.maximized,
    })
}

fn paint_window_chrome(hwnd: HWND) {
    let Some(webtag_key) = active_webtag_key_for_window(hwnd) else {
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
        attached: attached_state_for_window(hwnd, &webtag_key, client),
        frame_button_hover: chrome_interaction(hwnd).frame_button_hover,
        frame_button_pressed: chrome_interaction(hwnd).frame_button_pressed,
    };
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let saved = SaveDC(hdc);
        exclude_host_webview_content_from_paint(hdc, hwnd, &webtag_key);
        renderer.paint_region(hdc, &state, ps.rcPaint);
        let _ = RestoreDC(hdc, saved);
        paint_pull_refresh_indicator(hdc, hwnd, &webtag_key);
        let _ = EndPaint(hwnd, &ps);
    }
}

fn exclude_host_webview_content_from_paint(hdc: HDC, hwnd: HWND, webtag_key: &str) {
    let Some(webtag) = webtag_for_key(webtag_key) else {
        return;
    };
    let Some(handler) = find_webview_handler(&webtag) else {
        return;
    };
    if hwnd_from_handle(handler.native_view().window) != hwnd {
        return;
    }
    let rect = content_rect_for_window(hwnd, webtag_key);
    if rect.right <= rect.left || rect.bottom <= rect.top {
        return;
    }
    unsafe {
        let _ = ExcludeClipRect(hdc, rect.left, rect.top, rect.right, rect.bottom);
    }
}

fn paint_pull_refresh_indicator(hdc: HDC, hwnd: HWND, webtag_key: &str) {
    if !is_pull_refreshing(webtag_key) {
        return;
    }
    let rect = pull_refresh_indicator_rect(hwnd, webtag_key);
    if rect.right <= rect.left || rect.bottom <= rect.top {
        return;
    }
    let tick = pull_refresh_tick(hwnd);
    let active = (tick / 2) % 3;
    let center_y = rect.top + (rect.bottom - rect.top) / 2;
    let center_x = rect.left + (rect.right - rect.left) / 2;
    let spacing = 14;
    for index in 0..3 {
        let radius = if index == active { 4 } else { 3 };
        let color = if index == active { 0x667085 } else { 0xA8AFBA };
        let x = center_x + (index as i32 - 1) * spacing;
        draw_refresh_dot(hdc, x, center_y, radius, color);
    }
}

fn pull_refresh_indicator_rect(hwnd: HWND, webtag_key: &str) -> RECT {
    let content = base_content_rect_for_window(hwnd, webtag_key);
    let slot_top = content.top;
    let slot_bottom = (slot_top + PULL_REFRESH_SLOT_HEIGHT).min(content.bottom);
    let center_x = content.left + (content.right - content.left) / 2;
    normalize_rect(RECT {
        left: center_x - PULL_REFRESH_INDICATOR_WIDTH / 2,
        top: slot_top + ((slot_bottom - slot_top) - PULL_REFRESH_INDICATOR_HEIGHT) / 2,
        right: center_x + PULL_REFRESH_INDICATOR_WIDTH / 2,
        bottom: slot_top
            + ((slot_bottom - slot_top) - PULL_REFRESH_INDICATOR_HEIGHT) / 2
            + PULL_REFRESH_INDICATOR_HEIGHT,
    })
}

fn draw_refresh_dot(hdc: HDC, x: i32, y: i32, radius: i32, rgb: u32) {
    let color = rgb_to_colorref(rgb);
    unsafe {
        let brush = CreateSolidBrush(color);
        let pen = CreatePen(PS_SOLID, 1, color);
        let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
        let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
        let _ = Ellipse(hdc, x - radius, y - radius, x + radius + 1, y + radius + 1);
        let _ = SelectObject(hdc, old_pen);
        let _ = SelectObject(hdc, old_brush);
        let _ = DeleteObject(HGDIOBJ(pen.0));
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

fn rgb_to_colorref(rgb: u32) -> COLORREF {
    COLORREF(((rgb & 0xff) << 16) | (rgb & 0x00ff00) | ((rgb >> 16) & 0xff))
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

fn active_host_window() -> Option<HWND> {
    ACTIVE_WEBTAG
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.as_ref().map(|webtag| webtag.key().to_string()))
        .and_then(|key| window_handle_for_key(&key))
}

fn sync_active_host_layout() {
    if let Some(hwnd) = active_host_window() {
        sync_window_layout(hwnd);
        invalidate_window_chrome(hwnd);
    }
}

fn repaint_active_host() {
    if let Some(hwnd) = active_host_window() {
        invalidate_window(hwnd);
    }
}

fn register_webview_panel(panel_id: &str, webtag: &WebTag, position: WindowsPanelPosition) {
    let panels = WEBVIEW_PANELS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut panels) = panels.lock() {
        panels.insert(
            panel_id.to_string(),
            WebViewPanelEntry {
                webtag_key: webtag.key().to_string(),
                position,
                requested_size: None,
                docked: matches!(position, WindowsPanelPosition::Bottom),
                maximized: false,
            },
        );
    }
}

fn panel_id_for_webtag(webtag_key: &str) -> Option<String> {
    WEBVIEW_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .and_then(|panels| {
            panels
                .iter()
                .find(|(_, panel)| panel.webtag_key == webtag_key)
                .map(|(panel_id, _)| panel_id.clone())
        })
}

fn panel_is_docked(panel_id: &str) -> bool {
    if let Some(docked) = WEBVIEW_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .and_then(|panels| panels.get(panel_id).map(|panel| panel.docked))
    {
        return docked;
    }
    HOST_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .and_then(|panels| panels.get(panel_id).map(|panel| panel.docked))
        .unwrap_or(false)
}

fn host_panel_is_maximized(panel_id: &str) -> bool {
    HOST_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .and_then(|panels| panels.get(panel_id).map(|panel| panel.maximized))
        .unwrap_or(false)
}

#[cfg(feature = "shell-runtime")]
fn panel_position_for_id(panel_id: &str) -> WindowsPanelPosition {
    lingxia_app_context::app_config()
        .and_then(|config| config.panels.as_ref().cloned())
        .and_then(|panels| panels.items.into_iter().find(|item| item.id == panel_id))
        .map(|item| match item.position {
            lingxia_app_context::PanelPosition::Left => WindowsPanelPosition::Left,
            lingxia_app_context::PanelPosition::Right => WindowsPanelPosition::Right,
            lingxia_app_context::PanelPosition::Bottom => WindowsPanelPosition::Bottom,
        })
        .unwrap_or(WindowsPanelPosition::Right)
}

#[cfg(not(feature = "shell-runtime"))]
fn panel_position_for_id(_panel_id: &str) -> WindowsPanelPosition {
    WindowsPanelPosition::Right
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
    if !visible {
        clear_focused_host_panel(panel_id);
    }
}

fn focus_host_panel(panel_id: &str) {
    let focused = FOCUSED_HOST_PANEL.get_or_init(|| Mutex::new(None));
    if let Ok(mut focused) = focused.lock() {
        *focused = Some(panel_id.to_string());
    }
}

fn focus_active_host_window() {
    if let Some(hwnd) = active_host_window() {
        focus_host_window(hwnd);
    }
}

fn focus_host_window(hwnd: HWND) {
    let window = hwnd_handle(hwnd);
    let _ = post_to_window_thread(
        window,
        Box::new(move || unsafe {
            let hwnd = hwnd_from_handle(window);
            let _ = WindowsAndMessaging::SetForegroundWindow(hwnd);
            let _ = SetFocus(Some(hwnd));
        }),
    );
}

fn clear_focused_host_panel(panel_id: &str) {
    if let Some(focused) = FOCUSED_HOST_PANEL.get()
        && let Ok(mut focused) = focused.lock()
        && focused.as_deref() == Some(panel_id)
    {
        *focused = None;
    }
}

fn chrome_interaction(hwnd: HWND) -> ChromeInteraction {
    CHROME_INTERACTIONS
        .get()
        .and_then(|state| state.lock().ok())
        .and_then(|state| state.get(&hwnd_handle(hwnd)).copied())
        .unwrap_or_default()
}

fn update_chrome_interaction(hwnd: HWND, update: impl FnOnce(&mut ChromeInteraction)) {
    let state = CHROME_INTERACTIONS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut state) = state.lock() {
        update(state.entry(hwnd_handle(hwnd)).or_default());
    }
}

fn clear_chrome_interaction(hwnd: HWND) {
    if let Some(state) = CHROME_INTERACTIONS.get()
        && let Ok(mut state) = state.lock()
    {
        state.remove(&hwnd_handle(hwnd));
    }
}

fn chrome_state_for_window(hwnd: HWND) -> Option<WindowsChromeState> {
    let webtag_key = active_webtag_key_for_window(hwnd)?;
    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    let interaction = chrome_interaction(hwnd);
    Some(WindowsChromeState {
        hwnd,
        client,
        layout: current_window_layout(&webtag_key),
        attached: attached_state_for_window(hwnd, &webtag_key, client),
        frame_button_hover: interaction.frame_button_hover,
        frame_button_pressed: interaction.frame_button_pressed,
    })
}

fn chrome_hit_for_window(hwnd: HWND, point: (i32, i32)) -> Option<WindowsChromeHit> {
    let renderer = windows_chrome_renderer()?;
    renderer.hit_test(&chrome_state_for_window(hwnd)?, point)
}

fn invalidate_window(hwnd: HWND) {
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, false);
    }
}

fn invalidate_window_chrome(hwnd: HWND) {
    let Some(renderer) = windows_chrome_renderer() else {
        invalidate_window(hwnd);
        return;
    };
    let Some(webtag_key) = active_webtag_key_for_window(hwnd) else {
        invalidate_window(hwnd);
        return;
    };

    let mut client = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetClientRect(hwnd, &mut client).is_err() {
            invalidate_window(hwnd);
            return;
        }
    }

    let layout = current_window_layout(&webtag_key);
    let attached = attached_layout_for_window(hwnd, &webtag_key, client);
    if invalidate_precise_shell_chrome(hwnd, client, &layout, attached.as_ref()) {
        return;
    }

    let content = attached
        .as_ref()
        .map(|attached| attached.main)
        .unwrap_or_else(|| renderer.content_rect(client, &layout));

    invalidate_rect_if_non_empty(
        hwnd,
        RECT {
            left: client.left,
            top: client.top,
            right: client.right,
            bottom: content.top,
        },
    );
    invalidate_rect_if_non_empty(
        hwnd,
        RECT {
            left: client.left,
            top: content.top,
            right: content.left,
            bottom: content.bottom,
        },
    );
    invalidate_rect_if_non_empty(
        hwnd,
        RECT {
            left: content.right,
            top: content.top,
            right: client.right,
            bottom: content.bottom,
        },
    );
    invalidate_rect_if_non_empty(
        hwnd,
        RECT {
            left: client.left,
            top: content.bottom,
            right: client.right,
            bottom: client.bottom,
        },
    );

    if let Some(attached) = attached {
        for panel in attached.panels {
            if host_panel_content(&panel.panel_id).is_some() {
                invalidate_rect_if_non_empty(hwnd, panel.rect);
            }
        }
    }
}

#[cfg(feature = "shell-runtime")]
fn invalidate_precise_shell_chrome(
    hwnd: HWND,
    client: RECT,
    layout: &WindowsWindowLayout,
    attached: Option<&WindowsChromeAttachedLayout>,
) -> bool {
    let hwnd_key = hwnd_handle(hwnd);
    let snapshots = HOST_CHROME_SNAPSHOTS.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut snapshots) = snapshots.lock() else {
        return false;
    };
    let snapshot = HostChromeSnapshot {
        layout: layout.clone(),
        attached: attached.cloned(),
    };
    let Some(previous) = snapshots.insert(hwnd_key, snapshot) else {
        return false;
    };
    let attached_dirty = attached_chrome_dirty_rects(previous.attached.as_ref(), attached);
    for rect in attached_dirty {
        invalidate_rect_if_non_empty(hwnd, rect);
    }
    let Some(dirty) = crate::shell::shell_chrome_dirty_rects(client, &previous.layout, layout)
    else {
        return false;
    };
    for rect in dirty {
        invalidate_rect_if_non_empty(hwnd, rect);
    }
    true
}

#[cfg(not(feature = "shell-runtime"))]
fn invalidate_precise_shell_chrome(
    _hwnd: HWND,
    _client: RECT,
    _layout: &WindowsWindowLayout,
    _attached: Option<&WindowsChromeAttachedLayout>,
) -> bool {
    false
}

fn attached_chrome_dirty_rects(
    previous: Option<&WindowsChromeAttachedLayout>,
    current: Option<&WindowsChromeAttachedLayout>,
) -> Vec<RECT> {
    if previous == current {
        return Vec::new();
    }

    let mut dirty = Vec::new();
    if let Some(previous) = previous {
        push_attached_layout_dirty_rects(&mut dirty, previous);
    }
    if let Some(current) = current {
        push_attached_layout_dirty_rects(&mut dirty, current);
    }
    dirty
}

fn push_attached_layout_dirty_rects(dirty: &mut Vec<RECT>, attached: &WindowsChromeAttachedLayout) {
    for panel in &attached.panels {
        push_unique_dirty_rect(dirty, panel.rect);
        if let Some(handle) = panel.resize_handle {
            push_unique_dirty_rect(dirty, handle);
        }
    }
}

fn push_unique_dirty_rect(dirty: &mut Vec<RECT>, rect: RECT) {
    let rect = normalize_rect(RECT {
        left: rect.left.saturating_sub(2),
        top: rect.top.saturating_sub(2),
        right: rect.right.saturating_add(2),
        bottom: rect.bottom.saturating_add(2),
    });
    if rect.right <= rect.left || rect.bottom <= rect.top {
        return;
    }
    if !dirty.iter().any(|candidate| *candidate == rect) {
        dirty.push(rect);
    }
}

fn invalidate_rect_if_non_empty(hwnd: HWND, rect: RECT) {
    let rect = normalize_rect(rect);
    if rect.right <= rect.left || rect.bottom <= rect.top {
        return;
    }
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), Some(&rect), false);
    }
}

fn invalidate_active_host_panel(panel_id: &str) -> bool {
    let Some(hwnd) = active_host_window() else {
        return false;
    };
    let Some(webtag_key) = active_webtag_key_for_window(hwnd) else {
        return false;
    };
    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    let rect = attached_layout_for_window(hwnd, &webtag_key, client).and_then(|attached| {
        attached
            .panels
            .into_iter()
            .find(|panel| panel.panel_id == panel_id)
            .map(|panel| panel.rect)
    });
    let Some(rect) = rect else {
        invalidate_window(hwnd);
        return true;
    };
    unsafe {
        windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), Some(&rect), false).as_bool()
    }
}

fn lparam_client_point(lparam: LPARAM) -> (i32, i32) {
    let value = lparam.0 as u32;
    let x = (value & 0xffff) as i16 as i32;
    let y = ((value >> 16) & 0xffff) as i16 as i32;
    (x, y)
}

fn lparam_screen_point(lparam: LPARAM) -> POINT {
    let value = lparam.0 as u32;
    POINT {
        x: (value & 0xffff) as i16 as i32,
        y: ((value >> 16) & 0xffff) as i16 as i32,
    }
}

fn hit_test_window(hwnd: HWND, lparam: LPARAM) -> LRESULT {
    let mut point = lparam_screen_point(lparam);
    unsafe {
        let _ = ScreenToClient(hwnd, &mut point);
    }
    let point_tuple = (point.x, point.y);
    if let Some(hit) = resize_hit_test(hwnd, point_tuple) {
        return hit;
    }

    match chrome_hit_for_window(hwnd, point_tuple) {
        Some(WindowsChromeHit::Caption) => LRESULT(WindowsAndMessaging::HTCAPTION as isize),
        Some(_) => LRESULT(WindowsAndMessaging::HTCLIENT as isize),
        None => LRESULT(WindowsAndMessaging::HTCLIENT as isize),
    }
}

fn resize_hit_test(hwnd: HWND, point: (i32, i32)) -> Option<LRESULT> {
    if unsafe { WindowsAndMessaging::IsZoomed(hwnd).as_bool() } {
        return None;
    }
    let mut client = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetClientRect(hwnd, &mut client).is_err() {
            return None;
        }
    }
    let left = point.0 < client.left + RESIZE_BORDER;
    let right = point.0 >= client.right - RESIZE_BORDER;
    let top = point.1 < client.top + RESIZE_BORDER;
    let bottom = point.1 >= client.bottom - RESIZE_BORDER;
    match (left, right, top, bottom) {
        (true, _, true, _) => Some(LRESULT(WindowsAndMessaging::HTTOPLEFT as isize)),
        (_, true, true, _) => Some(LRESULT(WindowsAndMessaging::HTTOPRIGHT as isize)),
        (true, _, _, true) => Some(LRESULT(WindowsAndMessaging::HTBOTTOMLEFT as isize)),
        (_, true, _, true) => Some(LRESULT(WindowsAndMessaging::HTBOTTOMRIGHT as isize)),
        (true, _, _, _) => Some(LRESULT(WindowsAndMessaging::HTLEFT as isize)),
        (_, true, _, _) => Some(LRESULT(WindowsAndMessaging::HTRIGHT as isize)),
        (_, _, true, _) => Some(LRESULT(WindowsAndMessaging::HTTOP as isize)),
        (_, _, _, true) => Some(LRESULT(WindowsAndMessaging::HTBOTTOM as isize)),
        _ => None,
    }
}

fn screen_point_for_client(hwnd: HWND, point: (i32, i32)) -> POINT {
    let mut screen = POINT {
        x: point.0,
        y: point.1,
    };
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut screen);
    }
    screen
}

fn invoke_chrome_command(
    webtag_key: &str,
    hwnd: HWND,
    point: (i32, i32),
    mut command: WindowsChromeCommand,
) {
    if command.include_screen_position {
        let screen = screen_point_for_client(hwnd, point);
        let mut payload = match command.payload {
            serde_json::Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        payload.insert("screen_x".to_string(), serde_json::json!(screen.x));
        payload.insert("screen_y".to_string(), serde_json::json!(screen.y));
        command.payload = serde_json::Value::Object(payload);
    }
    if let Some(handler) = webview_chrome_event_handler(webtag_key) {
        let _ = std::thread::Builder::new()
            .name(format!("lingxia-windows-chrome-{webtag_key}"))
            .spawn(move || handler(command));
    }
}

fn handle_frame_button(hwnd: HWND, button: WindowsFrameButton) {
    unsafe {
        match button {
            WindowsFrameButton::Minimize => {
                let _ = WindowsAndMessaging::ShowWindow(hwnd, WindowsAndMessaging::SW_MINIMIZE);
            }
            WindowsFrameButton::Maximize => {
                let command = if WindowsAndMessaging::IsZoomed(hwnd).as_bool() {
                    WindowsAndMessaging::SW_RESTORE
                } else {
                    WindowsAndMessaging::SW_MAXIMIZE
                };
                let _ = WindowsAndMessaging::ShowWindow(hwnd, command);
            }
            WindowsFrameButton::Close => {
                let _ = WindowsAndMessaging::PostMessageW(
                    Some(hwnd),
                    WindowsAndMessaging::WM_CLOSE,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
    }
}

fn handle_chrome_mouse_move(hwnd: HWND, point: (i32, i32)) -> bool {
    let hit = chrome_hit_for_window(hwnd, point);
    let hover = match hit {
        Some(WindowsChromeHit::FrameButton(button)) => Some(button),
        _ => None,
    };
    if chrome_interaction(hwnd).frame_button_hover != hover {
        update_chrome_interaction(hwnd, |state| state.frame_button_hover = hover);
        invalidate_window(hwnd);
    }
    matches!(
        hit,
        Some(
            WindowsChromeHit::Chrome
                | WindowsChromeHit::FrameButton(_)
                | WindowsChromeHit::Command(_)
                | WindowsChromeHit::Focusable { .. }
        )
    )
}

fn handle_chrome_left_down(hwnd: HWND, point: (i32, i32)) -> bool {
    let Some(hit) = chrome_hit_for_window(hwnd, point) else {
        return false;
    };
    match hit {
        WindowsChromeHit::Caption => unsafe {
            let _ = ReleaseCapture();
            let _ = WindowsAndMessaging::SendMessageW(
                hwnd,
                WindowsAndMessaging::WM_NCLBUTTONDOWN,
                Some(WPARAM(WindowsAndMessaging::HTCAPTION as usize)),
                Some(LPARAM(0)),
            );
            true
        },
        WindowsChromeHit::FrameButton(button) => unsafe {
            let _ = SetCapture(hwnd);
            update_chrome_interaction(hwnd, |state| {
                state.frame_button_hover = Some(button);
                state.frame_button_pressed = Some(button);
            });
            invalidate_window(hwnd);
            true
        },
        WindowsChromeHit::Focusable { id, .. } => {
            focus_host_panel(&id);
            focus_host_window(hwnd);
            true
        }
        WindowsChromeHit::Chrome | WindowsChromeHit::Command(_) => true,
    }
}

fn handle_chrome_left_up(hwnd: HWND, point: (i32, i32)) -> bool {
    let webtag_key = active_webtag_key_for_window(hwnd);
    let pressed = chrome_interaction(hwnd).frame_button_pressed;
    unsafe {
        let _ = ReleaseCapture();
    }
    update_chrome_interaction(hwnd, |state| state.frame_button_pressed = None);
    let hit = chrome_hit_for_window(hwnd, point);
    if let Some(button) = pressed {
        let activate =
            matches!(hit, Some(WindowsChromeHit::FrameButton(candidate)) if candidate == button);
        update_chrome_interaction(hwnd, |state| {
            state.frame_button_hover = if activate { Some(button) } else { None };
        });
        invalidate_window(hwnd);
        if activate {
            handle_frame_button(hwnd, button);
        }
        return true;
    }
    match hit {
        Some(WindowsChromeHit::Command(command)) => {
            if let Some(webtag_key) = webtag_key {
                invoke_chrome_command(&webtag_key, hwnd, point, command);
            }
            true
        }
        Some(WindowsChromeHit::Chrome | WindowsChromeHit::Focusable { .. }) => true,
        _ => false,
    }
}

fn handle_chrome_left_double_click(hwnd: HWND, point: (i32, i32)) -> bool {
    let Some(webtag_key) = active_webtag_key_for_window(hwnd) else {
        return false;
    };
    match chrome_hit_for_window(hwnd, point) {
        Some(WindowsChromeHit::Command(command)) => {
            let command = command.double_click.as_deref().cloned().unwrap_or(command);
            invoke_chrome_command(&webtag_key, hwnd, point, command);
            true
        }
        Some(WindowsChromeHit::Caption) => unsafe {
            let _ = WindowsAndMessaging::SendMessageW(
                hwnd,
                WindowsAndMessaging::WM_NCLBUTTONDBLCLK,
                Some(WPARAM(WindowsAndMessaging::HTCAPTION as usize)),
                Some(LPARAM(0)),
            );
            true
        },
        Some(
            WindowsChromeHit::Chrome
            | WindowsChromeHit::FrameButton(_)
            | WindowsChromeHit::Focusable { .. },
        ) => true,
        None => false,
    }
}

fn handle_chrome_right_up(hwnd: HWND, point: (i32, i32)) -> bool {
    let Some(webtag_key) = active_webtag_key_for_window(hwnd) else {
        return false;
    };
    if let Some(WindowsChromeHit::Focusable {
        context_menu: Some(command),
        ..
    }) = chrome_hit_for_window(hwnd, point)
    {
        invoke_chrome_command(&webtag_key, hwnd, point, command);
        return true;
    }
    false
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
    let mut window = RECT::default();
    unsafe {
        WindowsAndMessaging::GetWindowRect(hwnd, &mut window)
            .map_err(|err| WebViewError::WebView(format!("GetWindowRect failed: {err}")))?;
    }
    Ok(WindowsWebViewWindowSnapshot {
        window_id: hwnd_handle(hwnd) as usize,
        webtag_key: webtag.key().to_string(),
        visible: webtag_is_visible(webtag.key())
            && unsafe { WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() },
        window_left: window.left,
        window_top: window.top,
        window_width: (window.right - window.left).max(0),
        window_height: (window.bottom - window.top).max(0),
        content_left: content.left,
        content_top: content.top,
        content_width: (content.right - content.left).max(0) as u32,
        content_height: (content.bottom - content.top).max(0) as u32,
    })
}

pub fn set_webview_pull_down_refreshing(webtag: &WebTag, refreshing: bool) -> bool {
    let Some(hwnd) = window_handle_for_key(webtag.key()) else {
        return false;
    };
    let webtag_key = webtag.key().to_string();
    post_to_window_thread(
        hwnd_handle(hwnd),
        Box::new(move || set_webtag_pull_down_refreshing_on_window(&webtag_key, refreshing)),
    )
}

fn set_webtag_pull_down_refreshing_on_window(webtag_key: &str, refreshing: bool) {
    let Some(hwnd) = window_handle_for_key(webtag_key) else {
        return;
    };
    let slot = PULL_REFRESH_WEBTAGS.get_or_init(|| Mutex::new(HashSet::new()));
    let changed = match slot.lock() {
        Ok(mut refreshing_webtags) => {
            if refreshing {
                refreshing_webtags.insert(webtag_key.to_string())
            } else {
                refreshing_webtags.remove(webtag_key)
            }
        }
        Err(_) => false,
    };
    if refreshing {
        ensure_pull_refresh_timer(hwnd);
    } else {
        stop_pull_refresh_timer_if_idle(hwnd);
    }
    if changed {
        sync_window_layout(hwnd);
        invalidate_window(hwnd);
    }
}

pub fn show_webview_window(webtag: &WebTag, title: &str, activate: bool) -> StdResult<()> {
    let handler = find_webview_handler(webtag).ok_or_else(|| handler_not_ready(webtag))?;
    show_native_view(handler.native_view(), title, activate)?;
    handler.set_content_visible(true)?;
    let hwnd = hwnd_from_handle(handler.native_view().window);
    set_host_active_webtag(hwnd, webtag.key());
    set_window_handle(webtag.key(), hwnd);
    mark_active(webtag);
    notify_webtag_visibility(webtag.key(), true);
    Ok(())
}

fn show_webview_window_replacing(
    webtag: &WebTag,
    title: &str,
    activate: bool,
    hide_webtags: Vec<WebTag>,
) -> StdResult<()> {
    let handler = find_webview_handler(webtag).ok_or_else(|| handler_not_ready(webtag))?;
    let target = stable_host_for_replacement(webtag, &hide_webtags)
        .unwrap_or_else(|| hwnd_from_handle(handler.native_view().window));
    let title = to_wide(title);
    unsafe {
        let _ = WindowsAndMessaging::SetWindowTextW(target, PCWSTR(title.as_ptr()));
    }

    let already_active = active_webtag_key_for_window(target).as_deref() == Some(webtag.key())
        && webtag_is_visible(webtag.key());
    if already_active {
        sync_window_layout(target);
        return Ok(());
    }

    let inherited_layout = active_webtag_key_for_window(target)
        .map(|key| current_window_layout(&key))
        .filter(|layout| !layout.is_empty());
    let target_has_layout = !current_window_layout(webtag.key()).is_empty();

    if webtag_is_visible(webtag.key()) {
        handler.set_content_visible(false)?;
    }
    set_window_handle(webtag.key(), target);
    set_host_active_webtag(target, webtag.key());
    let inherited = if target_has_layout {
        false
    } else if let Some(layout) = inherited_layout {
        set_webview_window_layout(webtag, layout).is_ok()
    } else {
        false
    };
    if !inherited {
        sync_window_layout(target);
    }
    handler.set_content_visible(true)?;

    for hidden in &hide_webtags {
        if let Some(hidden_handler) = find_webview_handler(hidden) {
            let _ = hidden_handler.set_content_visible(false);
            let native = hwnd_from_handle(hidden_handler.native_view().window);
            if native != target && is_window_visible(native) {
                set_window_handle(hidden.key(), native);
                unsafe {
                    let _ = WindowsAndMessaging::SetWindowPos(
                        native,
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
        }
    }

    if !is_window_visible(target) || activate {
        unsafe {
            let mut flags = WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_SHOWWINDOW;
            if !activate {
                flags |= WindowsAndMessaging::SWP_NOACTIVATE;
            }
            WindowsAndMessaging::SetWindowPos(target, None, 0, 0, 0, 0, flags)
                .map_err(|err| WebViewError::WebView(format!("SetWindowPos failed: {err}")))?;
            if activate {
                let _ = WindowsAndMessaging::BringWindowToTop(target);
                let _ = WindowsAndMessaging::SetForegroundWindow(target);
            }
        }
    }

    mark_active(webtag);
    notify_webtag_visibility(webtag.key(), true);
    for hidden in &hide_webtags {
        notify_webtag_visibility(hidden.key(), false);
    }
    Ok(())
}

pub fn navigate_webview_window(webtag: &WebTag, title: &str, activate: bool) -> StdResult<()> {
    show_webview_window_replacing(webtag, title, activate, normal_group_webtags(webtag))
}

fn normal_group_webtags(active: &WebTag) -> Vec<WebTag> {
    let appid = active.extract_appid();
    let session_id = active.session_id();
    webview_runtime::list_webviews()
        .into_iter()
        .filter(|webtag| {
            webtag.key() != active.key()
                && webtag.extract_appid() == appid
                && webtag.session_id() == session_id
                && !is_page_instance_webtag(webtag)
        })
        .collect()
}

fn is_page_instance_webtag(webtag: &WebTag) -> bool {
    webtag
        .key()
        .split_once(':')
        .map(|(_, path)| path.matches('#').count() > 1)
        .unwrap_or(false)
}

fn stable_host_for_replacement(webtag: &WebTag, candidates: &[WebTag]) -> Option<HWND> {
    candidates
        .iter()
        .filter(|candidate| {
            candidate.extract_appid() == webtag.extract_appid()
                && candidate.session_id() == webtag.session_id()
        })
        .filter_map(|candidate| {
            webview_window_snapshot(candidate)
                .ok()
                .filter(|snapshot| snapshot.visible)
                .map(|snapshot| hwnd_from_handle(snapshot.window_id as isize))
        })
        .next()
}

pub fn hide_webview_window(webtag: &WebTag) -> StdResult<()> {
    let handler = find_webview_handler(webtag).ok_or_else(|| handler_not_ready(webtag))?;
    if let Some(panel_id) = panel_id_for_webtag(webtag.key()) {
        handler.set_content_visible(false)?;
        mark_panel_visible(&panel_id, false);
        set_window_handle(webtag.key(), hwnd_from_handle(handler.native_view().window));
        notify_webtag_visibility(webtag.key(), false);
        sync_active_host_layout();
        return Ok(());
    }
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
    notify_webtag_visibility(webtag.key(), false);
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
                if invoke_window_close_handler(hwnd) {
                    return LRESULT(0);
                }
                unsafe {
                    let _ = WindowsAndMessaging::DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WindowsAndMessaging::WM_SHOWWINDOW => {
                if let Some(webtag_key) = active_webtag_key_for_window(hwnd) {
                    notify_webtag_visibility(&webtag_key, wparam.0 != 0 && !is_minimized(hwnd));
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_ACTIVATEAPP => {
                if let Some(webtag_key) = active_webtag_key_for_window(hwnd) {
                    notify_webtag_visibility(
                        &webtag_key,
                        wparam.0 != 0 && is_window_visible(hwnd) && !is_minimized(hwnd),
                    );
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_SIZE => {
                if let Some(webtag_key) = active_webtag_key_for_window(hwnd) {
                    notify_webtag_visibility(
                        &webtag_key,
                        wparam.0 as u32 != WindowsAndMessaging::SIZE_MINIMIZED
                            && is_window_visible(hwnd),
                    );
                }
                sync_window_layout(hwnd);
                if windows_chrome_renderer().is_some() {
                    invalidate_window(hwnd);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_WINDOWPOSCHANGED => {
                if let Some(webtag_key) = active_webtag_key_for_window(hwnd) {
                    notify_webtag_visibility(
                        &webtag_key,
                        is_window_visible(hwnd) && !is_minimized(hwnd),
                    );
                }
                handle_window_position_changed(hwnd, lparam);
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
            WindowsAndMessaging::WM_TIMER if wparam.0 == PULL_REFRESH_TIMER_ID => {
                advance_pull_refresh_tick(hwnd);
                invalidate_window(hwnd);
                LRESULT(0)
            }
            WindowsAndMessaging::WM_NCHITTEST => {
                if windows_chrome_renderer().is_some() {
                    return hit_test_window(hwnd, lparam);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_MOUSEMOVE => {
                if windows_chrome_renderer().is_some()
                    && handle_chrome_mouse_move(hwnd, lparam_client_point(lparam))
                {
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_LBUTTONDOWN => {
                if windows_chrome_renderer().is_some()
                    && handle_chrome_left_down(hwnd, lparam_client_point(lparam))
                {
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_LBUTTONUP => {
                if windows_chrome_renderer().is_some()
                    && handle_chrome_left_up(hwnd, lparam_client_point(lparam))
                {
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_LBUTTONDBLCLK => {
                if windows_chrome_renderer().is_some()
                    && handle_chrome_left_double_click(hwnd, lparam_client_point(lparam))
                {
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_RBUTTONUP => {
                if windows_chrome_renderer().is_some()
                    && handle_chrome_right_up(hwnd, lparam_client_point(lparam))
                {
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_DESTROY => LRESULT(0),
            WindowsAndMessaging::WM_NCDESTROY => {
                let webtag_key = window_webtag_key(hwnd);
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
                if let Some(webtag_key) = webtag_key {
                    cleanup_window_state(&webtag_key);
                }
                if is_top_level_window(hwnd) && !has_live_top_level_host_window_except(hwnd) {
                    unsafe {
                        WindowsAndMessaging::PostQuitMessage(0);
                    }
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_CHAR => {
                if handle_host_panel_key_message(msg, wparam) {
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_KEYDOWN => {
                if handle_host_panel_key_message(msg, wparam) {
                    return LRESULT(0);
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
        let style = if windows_chrome_renderer().is_some() {
            WINDOW_STYLE(
                WS_POPUP.0 | WS_SIZEBOX.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0,
            )
        } else {
            WS_OVERLAPPEDWINDOW
        };
        let result = WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("LingXiaWebViewParent"),
            w!("LingXia WebView"),
            style,
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
    let handler = webview_close_handler(webtag_key);
    if let Some(handler) = handler {
        let _ = std::thread::Builder::new()
            .name(format!("lingxia-windows-close-{webtag_key}"))
            .spawn(move || handler());
        true
    } else {
        false
    }
}

fn invoke_window_close_handler(hwnd: HWND) -> bool {
    let mut candidates = Vec::new();
    if let Some(main_key) = PRESENTED_GROUP_MAIN
        .get()
        .and_then(|presented| presented.lock().ok())
        .and_then(|presented| presented.get(&hwnd_handle(hwnd)).cloned())
    {
        candidates.push(main_key);
    }
    if let Some(active_key) = active_webtag_key_for_window(hwnd) {
        candidates.push(active_key);
    }
    if let Some(owner_key) = window_webtag_key(hwnd) {
        candidates.push(owner_key);
    }
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|key| seen.insert(key.clone()))
        .any(|key| invoke_close_handler(&key))
}

fn handle_host_panel_key_message(msg: u32, wparam: WPARAM) -> bool {
    let Some(panel_id) = focused_input_host_panel() else {
        return false;
    };
    let Some(handler) = host_panel_input_handler(&panel_id) else {
        return false;
    };
    let character = if msg == WindowsAndMessaging::WM_CHAR {
        char::from_u32(wparam.0 as u32)
    } else {
        None
    };
    let event = WindowsHostPanelKeyEvent {
        vk: if msg == WindowsAndMessaging::WM_KEYDOWN {
            wparam.0 as u32
        } else {
            0
        },
        ctrl: key_is_down(VK_CONTROL.0 as i32),
        shift: key_is_down(VK_SHIFT.0 as i32),
        alt: key_is_down(VK_MENU.0 as i32),
        character,
    };
    handler(event)
}

fn key_is_down(vk: i32) -> bool {
    unsafe { GetKeyState(vk) < 0 }
}

fn focused_input_host_panel() -> Option<String> {
    if let Some(panel_id) = FOCUSED_HOST_PANEL
        .get()
        .and_then(|focused| focused.lock().ok())
        .and_then(|focused| focused.clone())
        && visible_host_panel_accepts_input(&panel_id)
    {
        return Some(panel_id);
    }
    visible_input_host_panels().into_iter().next()
}

fn visible_input_host_panels() -> Vec<String> {
    let visible = VISIBLE_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .map(|panels| panels.clone())
        .unwrap_or_default();
    let Some(panels) = HOST_PANELS.get().and_then(|panels| panels.lock().ok()) else {
        return Vec::new();
    };
    panels
        .keys()
        .filter(|panel_id| {
            visible.contains(*panel_id) && host_panel_input_handler(panel_id).is_some()
        })
        .cloned()
        .collect()
}

fn visible_host_panel_accepts_input(panel_id: &str) -> bool {
    VISIBLE_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .is_some_and(|panels| panels.contains(panel_id))
        && HOST_PANELS
            .get()
            .and_then(|panels| panels.lock().ok())
            .is_some_and(|panels| panels.contains_key(panel_id))
        && host_panel_input_handler(panel_id).is_some()
}

fn is_top_level_window(hwnd: HWND) -> bool {
    unsafe {
        WindowsAndMessaging::GetParent(hwnd)
            .map(|parent| parent.0.is_null())
            .unwrap_or(true)
    }
}

fn has_live_top_level_host_window_except(excluded: HWND) -> bool {
    let Some(handles) = WEBTAG_WINDOWS.get().and_then(|handles| handles.lock().ok()) else {
        return false;
    };
    let mut seen = HashSet::new();
    handles.values().copied().any(|handle| {
        if handle == hwnd_handle(excluded) || !seen.insert(handle) {
            return false;
        }
        let hwnd = hwnd_from_handle(handle);
        unsafe { WindowsAndMessaging::IsWindow(Some(hwnd)).as_bool() && is_top_level_window(hwnd) }
    })
}

fn invoke_host_window_created_handler(hwnd: HWND) {
    for handler in host_window_created_handlers() {
        handler(hwnd_handle(hwnd));
    }
}

fn register_window_handle(webtag_key: &str, hwnd: HWND) {
    set_window_handle(webtag_key, hwnd);
    let visibility = WEBTAG_VISIBILITY.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut visibility) = visibility.lock() {
        visibility.insert(webtag_key.to_string(), false);
    }
}

fn set_window_handle(webtag_key: &str, hwnd: HWND) {
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

fn cleanup_window_state(webtag_key: &str) {
    notify_webtag_visibility(webtag_key, false);
    clear_pull_refreshing(webtag_key);
    let removed_panel = cleanup_webview_panel(webtag_key);
    if let Some(hwnd) = window_handle_for_key(webtag_key) {
        clear_chrome_interaction(hwnd);
    }
    remove_window_handle(webtag_key);
    clear_host_active_webtag(webtag_key);
    if let Some(visibility) = WEBTAG_VISIBILITY.get()
        && let Ok(mut visibility) = visibility.lock()
    {
        visibility.remove(webtag_key);
    }
    if let Some(bounds) = WEBTAG_CONTENT_BOUNDS.get()
        && let Ok(mut bounds) = bounds.lock()
    {
        bounds.remove(webtag_key);
    }
    cleanup_webview_state(webtag_key);
    if let Some(active) = ACTIVE_WEBTAG.get()
        && let Ok(mut active) = active.lock()
        && active
            .as_ref()
            .is_some_and(|webtag| webtag.key() == webtag_key)
    {
        *active = None;
    }
    if removed_panel {
        sync_active_host_layout();
    }
}

fn cleanup_webview_panel(webtag_key: &str) -> bool {
    let panel_id = WEBVIEW_PANELS.get().and_then(|panels| {
        let mut panels = panels.lock().ok()?;
        let panel_id = panels
            .iter()
            .find(|(_, panel)| panel.webtag_key == webtag_key)
            .map(|(panel_id, _)| panel_id.clone())?;
        panels.remove(&panel_id);
        Some(panel_id)
    });
    let Some(panel_id) = panel_id else {
        return false;
    };
    mark_panel_visible(&panel_id, false);
    if let Some(tabs) = PANEL_TABS.get()
        && let Ok(mut tabs) = tabs.lock()
    {
        tabs.remove(&panel_id);
    }
    true
}

fn webtag_is_visible(webtag_key: &str) -> bool {
    WEBTAG_VISIBILITY
        .get()
        .and_then(|visibility| visibility.lock().ok())
        .and_then(|visibility| visibility.get(webtag_key).copied())
        .unwrap_or(false)
}

fn notify_webtag_visibility(webtag_key: &str, visible: bool) {
    let visibility = WEBTAG_VISIBILITY.get_or_init(|| Mutex::new(HashMap::new()));
    let changed = match visibility.lock() {
        Ok(mut visibility) => {
            if visibility.get(webtag_key).copied() == Some(visible) {
                false
            } else {
                visibility.insert(webtag_key.to_string(), visible);
                true
            }
        }
        Err(_) => false,
    };
    if !changed {
        return;
    }
    let Some(webtag) = webtag_for_key(webtag_key) else {
        return;
    };
    if let Some(handler) = webview_visibility_handler() {
        let _ = std::thread::Builder::new()
            .name(format!("lingxia-windows-visible-{webtag_key}"))
            .spawn(move || handler(&webtag, visible));
    }
}

fn is_pull_refreshing(webtag_key: &str) -> bool {
    PULL_REFRESH_WEBTAGS
        .get()
        .and_then(|slot| slot.lock().ok())
        .is_some_and(|refreshing| refreshing.contains(webtag_key))
}

fn clear_pull_refreshing(webtag_key: &str) {
    let hwnd = window_handle_for_key(webtag_key);
    if let Some(slot) = PULL_REFRESH_WEBTAGS.get()
        && let Ok(mut refreshing) = slot.lock()
    {
        refreshing.remove(webtag_key);
    }
    if let Some(hwnd) = hwnd {
        stop_pull_refresh_timer_if_idle(hwnd);
    }
}

fn ensure_pull_refresh_timer(hwnd: HWND) {
    unsafe {
        let _ = WindowsAndMessaging::SetTimer(
            Some(hwnd),
            PULL_REFRESH_TIMER_ID,
            PULL_REFRESH_TIMER_MS,
            None,
        );
    }
}

fn stop_pull_refresh_timer_if_idle(hwnd: HWND) {
    if window_has_pull_refreshing_webtag(hwnd) {
        return;
    }
    unsafe {
        let _ = WindowsAndMessaging::KillTimer(Some(hwnd), PULL_REFRESH_TIMER_ID);
    }
    if let Some(ticks) = PULL_REFRESH_TICKS.get()
        && let Ok(mut ticks) = ticks.lock()
    {
        ticks.remove(&hwnd_handle(hwnd));
    }
}

fn window_has_pull_refreshing_webtag(hwnd: HWND) -> bool {
    let Some(refreshing) = PULL_REFRESH_WEBTAGS.get().and_then(|slot| slot.lock().ok()) else {
        return false;
    };
    refreshing.iter().any(|webtag_key| {
        window_handle_for_key(webtag_key).is_some_and(|candidate| candidate == hwnd)
    })
}

fn advance_pull_refresh_tick(hwnd: HWND) {
    let ticks = PULL_REFRESH_TICKS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut ticks) = ticks.lock() {
        let tick = ticks.entry(hwnd_handle(hwnd)).or_insert(0);
        *tick = tick.wrapping_add(1);
    }
    if !window_has_pull_refreshing_webtag(hwnd) {
        stop_pull_refresh_timer_if_idle(hwnd);
    }
}

fn pull_refresh_tick(hwnd: HWND) -> u32 {
    PULL_REFRESH_TICKS
        .get()
        .and_then(|ticks| ticks.lock().ok())
        .and_then(|ticks| ticks.get(&hwnd_handle(hwnd)).copied())
        .unwrap_or(0)
}

fn set_host_active_webtag(hwnd: HWND, webtag_key: &str) {
    let hosts = HOST_ACTIVE_WEBTAG.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut hosts) = hosts.lock() {
        hosts.insert(hwnd_handle(hwnd), webtag_key.to_string());
    }
}

fn clear_host_active_webtag(webtag_key: &str) {
    if let Some(hosts) = HOST_ACTIVE_WEBTAG.get()
        && let Ok(mut hosts) = hosts.lock()
    {
        hosts.retain(|_, active_key| active_key != webtag_key);
    }
}

fn active_webtag_key_for_window(hwnd: HWND) -> Option<String> {
    HOST_ACTIVE_WEBTAG
        .get()
        .and_then(|hosts| hosts.lock().ok())
        .and_then(|hosts| hosts.get(&hwnd_handle(hwnd)).cloned())
        .or_else(|| window_webtag_key(hwnd))
}

fn is_window_visible(hwnd: HWND) -> bool {
    unsafe { WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() }
}

fn is_minimized(hwnd: HWND) -> bool {
    unsafe { WindowsAndMessaging::IsIconic(hwnd).as_bool() }
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
