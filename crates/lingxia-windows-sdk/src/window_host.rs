//! Windows host-window implementation owned by the Windows SDK layer.

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use lingxia_webview::platform::windows::{
    WindowsWebViewHandler, WindowsWebViewNativeView, WindowsWebViewNativeViewHost,
    find_webview_handler, set_webview_native_view_host,
};
use lingxia_webview::runtime as webview_runtime;
use lingxia_webview::{WebTag, WebViewError};
// Contract types/handlers are consumed straight from the contract crate. This
// module is the host-window *implementation*; it does not re-export the
// contract (so other SDK modules import contract symbols from
// `lingxia_windows_contract` directly, not via `crate::window_host`).
use lingxia_windows_contract::{
    WindowsChromeAttachedLayout, WindowsChromeAttachedState, WindowsChromeCommand,
    WindowsChromeHit, WindowsChromePanel, WindowsChromePanelLayoutInput, WindowsChromeState,
    WindowsContentRect, WindowsFrameButton, WindowsHostBackend, WindowsHostPanelContent,
    WindowsHostPanelKeyEvent, WindowsHostPanelTab, WindowsHostWindow, WindowsPanelPosition,
    WindowsWebViewContentWindow, WindowsWebViewWindowSnapshot, WindowsWindowLayout,
    cleanup_webview_state, current_window_layout, default_window_size, host_panel_input_handler,
    host_window_created_handlers, set_webview_window_layout, set_windows_host_backend,
    webview_chrome_event_handler, webview_close_handler, webview_visibility_handler,
    windows_chrome_renderer,
};
#[cfg(feature = "shell-chrome")]
use windows::Win32::Foundation::SIZE;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
#[cfg(feature = "shell-chrome")]
use windows::Win32::Graphics::Gdi::{AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, BitBlt, CreateCompatibleDC, CreateDIBSection,
    CreatePen, CreateSolidBrush, DIB_RGB_COLORS, DeleteDC, DeleteObject, Ellipse, EndPaint,
    ExcludeClipRect, GetDC, GetMonitorInfoW, HDC, HGDIOBJ, IntersectClipRect,
    MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, PAINTSTRUCT, PS_SOLID, ReleaseDC,
    RestoreDC, SRCCOPY, SaveDC, ScreenToClient, SelectObject,
};
use windows::Win32::System::LibraryLoader;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
// Only the shell-chrome transparent-tabbar overlay cleanup needs the PID.
#[cfg(feature = "shell-chrome")]
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
    VK_CONTROL, VK_MENU, VK_SHIFT,
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
static PRIMARY_HOST_WINDOW: OnceLock<Mutex<Option<isize>>> = OnceLock::new();
static FOCUSED_HOST_PANEL: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static NATIVE_FRAMED_WINDOWS: OnceLock<Mutex<HashSet<isize>>> = OnceLock::new();
#[cfg(feature = "shell-chrome")]
static HOST_CHROME_SNAPSHOTS: OnceLock<Mutex<HashMap<isize, HostChromeSnapshot>>> = OnceLock::new();
static CHROME_INTERACTIONS: OnceLock<Mutex<HashMap<isize, ChromeInteraction>>> = OnceLock::new();
static CHROME_BACK_BUFFERS: OnceLock<Mutex<HashMap<isize, ChromeBackBuffer>>> = OnceLock::new();
static ATTACHED_PANEL_RESIZE_DRAG: OnceLock<Mutex<Option<AttachedPanelResizeDrag>>> =
    OnceLock::new();
static PULL_REFRESH_WEBTAGS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static PULL_REFRESH_TICKS: OnceLock<Mutex<HashMap<isize, u32>>> = OnceLock::new();
#[cfg(feature = "shell-chrome")]
static TRANSPARENT_TABBAR_OVERLAYS: OnceLock<Mutex<HashMap<isize, TransparentTabbarOverlay>>> =
    OnceLock::new();
#[cfg(feature = "shell-chrome")]
static SIDEBAR_TABBAR_POPUPS: OnceLock<Mutex<HashMap<isize, SidebarTabbarPopup>>> = OnceLock::new();
const WM_LINGXIA_RUN_CALLBACK: u32 = WindowsAndMessaging::WM_APP + 0x158;
const PULL_REFRESH_TIMER_ID: usize = 0x5A17;
const PULL_REFRESH_TIMER_MS: u32 = 120;
const PULL_REFRESH_SLOT_HEIGHT: i32 = 42;
const PULL_REFRESH_INDICATOR_WIDTH: i32 = 64;
const PULL_REFRESH_INDICATOR_HEIGHT: i32 = 32;
const OVERLAY_MARGIN: i32 = 24;
#[cfg(feature = "shell-chrome")]
const SIDEBAR_TABBAR_POPUP_TIMER_ID: usize = 0x5A18;
#[cfg(feature = "shell-chrome")]
const SIDEBAR_TABBAR_POPUP_TIMER_MS: u32 = 80;

/// `WM_DWMCOLORIZATIONCOLORCHANGED` (dwmapi.h) - sent on a system accent change.
/// Not surfaced by the `windows` crate's message constants, so define it here.
const WM_DWMCOLORIZATIONCOLORCHANGED: u32 = 0x0320;
/// `WM_MOUSELEAVE` (winuser.h) - not surfaced by the pinned `windows` rev.
const WM_MOUSELEAVE: u32 = 0x02a3;
const OVERLAY_MIN_WIDTH: i32 = 280;
const OVERLAY_MIN_HEIGHT: i32 = 220;
const OVERLAY_DEFAULT_WIDTH: i32 = 460;
const OVERLAY_DEFAULT_HEIGHT: i32 = 560;
const RESIZE_BORDER: i32 = 8;

#[derive(Debug, Clone, Copy, Default)]
struct ChromeInteraction {
    frame_button_hover: Option<WindowsFrameButton>,
    frame_button_pressed: Option<WindowsFrameButton>,
    /// Client-space cursor position while it is over this window's chrome.
    cursor: Option<(i32, i32)>,
    /// Rect of the chrome element currently under the cursor (from the
    /// renderer's `hover_rect`); the previous/next rects are invalidated on
    /// change so hover feedback repaints exactly the affected element.
    hover_rect: Option<RECT>,
}

/// Per-window offscreen surface chrome renders into; WM_PAINT blits the
/// update region to the screen in one step so DWM never composes a
/// mid-paint state (the direct fill-then-draw sequence read as flicker).
struct ChromeBackBuffer {
    dc: isize,
    bitmap: isize,
    width: i32,
    height: i32,
}

impl ChromeBackBuffer {
    fn release(&self) {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(self.bitmap as *mut c_void));
            let _ = DeleteDC(HDC(self.dc as *mut c_void));
        }
    }
}

/// The chrome back buffer for `hwnd`, recreated on size change. `None` when
/// the surface cannot be created (callers fall back to direct painting).
fn chrome_back_buffer_dc(hwnd: HWND, width: i32, height: i32) -> Option<HDC> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let buffers = CHROME_BACK_BUFFERS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut buffers = buffers.lock().ok()?;
    let key = hwnd_handle(hwnd);
    if let Some(existing) = buffers.get(&key) {
        if existing.width == width && existing.height == height {
            return Some(HDC(existing.dc as *mut c_void));
        }
        existing.release();
        buffers.remove(&key);
    }

    unsafe {
        let window_dc = GetDC(Some(hwnd));
        if window_dc.is_invalid() {
            return None;
        }
        let dc = CreateCompatibleDC(Some(window_dc));
        let _ = ReleaseDC(Some(hwnd), window_dc);
        if dc.is_invalid() {
            return None;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) else {
            let _ = DeleteDC(dc);
            return None;
        };
        if bitmap.is_invalid() {
            let _ = DeleteDC(dc);
            return None;
        }
        let previous = SelectObject(dc, HGDIOBJ(bitmap.0));
        if previous.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            return None;
        }
        buffers.insert(
            key,
            ChromeBackBuffer {
                dc: dc.0 as isize,
                bitmap: bitmap.0 as isize,
                width,
                height,
            },
        );
        Some(dc)
    }
}

fn release_chrome_back_buffer(hwnd: HWND) {
    let Some(buffers) = CHROME_BACK_BUFFERS.get() else {
        return;
    };
    let Ok(mut buffers) = buffers.lock() else {
        return;
    };
    if let Some(buffer) = buffers.remove(&hwnd_handle(hwnd)) {
        buffer.release();
    }
}

#[cfg(feature = "shell-chrome")]
#[derive(Debug, Clone, Copy)]
struct TransparentTabbarOverlay {
    window: isize,
    rect: RECT,
}

#[cfg(feature = "shell-chrome")]
#[derive(Debug, Clone)]
struct SidebarTabbarPopup {
    window: isize,
    owner: isize,
    anchor: RECT,
    rect: RECT,
    tabbar: crate::shell::WindowsShellTabBarLayout,
}

#[derive(Debug, Clone)]
struct WebViewPanelEntry {
    webtag_key: String,
    title: String,
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

#[derive(Debug, Clone)]
struct AttachedPanelResizeDrag {
    panel_id: String,
    position: WindowsPanelPosition,
    origin: (i32, i32),
    origin_size: i32,
}

#[cfg(feature = "shell-chrome")]
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

pub fn install_default_webview_native_view_host() {
    set_webview_native_view_host(Arc::new(PlatformNativeViewHost));
}

pub fn install_default_windows_backend() {
    install_default_webview_native_view_host();
    set_windows_host_backend(Arc::new(WindowsHostBackendImpl));
}

struct WindowsHostBackendImpl;

impl WindowsHostBackend for WindowsHostBackendImpl {
    fn show_webview_as_panel(&self, webtag: &WebTag, title: &str, panel_id: &str) -> StdResult<()> {
        show_webview_as_panel(webtag, title, panel_id)
    }

    fn show_webview_as_adaptive_panel(
        &self,
        webtag: &WebTag,
        title: &str,
        panel_id: &str,
        position: WindowsPanelPosition,
        preferred_size: Option<i32>,
    ) -> StdResult<()> {
        show_webview_as_adaptive_panel(webtag, title, panel_id, position, preferred_size)
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

    fn show_webview_window_with_content_size(
        &self,
        webtag: &WebTag,
        title: &str,
        activate: bool,
        width: Option<i32>,
        height: Option<i32>,
    ) -> StdResult<()> {
        show_webview_window_with_content_size(webtag, title, activate, width, height)
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

    fn refresh_aside_panel(&self, panel_id: &str) {
        refresh_aside_panel_chrome(panel_id);
    }
}

/// Repaints an aside panel's chrome: the top-band header strip plus the
/// panel rect.
fn refresh_aside_panel_chrome(panel_id: &str) {
    let Some(hwnd) = active_host_window() else {
        return;
    };
    let mut client = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetClientRect(hwnd, &mut client).is_err() {
            return;
        }
    }
    #[cfg(feature = "shell-chrome")]
    invalidate_rect_if_non_empty(
        hwnd,
        RECT {
            left: client.left,
            top: client.top,
            right: client.right,
            bottom: (client.top + crate::shell::shell_top_bar_height()).min(client.bottom),
        },
    );
    #[cfg(not(feature = "shell-chrome"))]
    let _ = client;
    let _ = invalidate_active_host_panel(panel_id);
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
    show_webview_as_adaptive_panel(
        webtag,
        title,
        panel_id,
        panel_position_for_id(panel_id),
        None,
    )
}

pub fn show_webview_as_adaptive_panel(
    webtag: &WebTag,
    title: &str,
    panel_id: &str,
    position: WindowsPanelPosition,
    preferred_size: Option<i32>,
) -> StdResult<()> {
    let handler = find_webview_handler(webtag).ok_or_else(|| handler_not_ready(webtag))?;
    // The caller's webtag may be the canonical (instance-less) form; every
    // registry below must use the live webview's own key, or the layout
    // pass's visibility reconcile never matches the panel and blinks it on
    // each sync.
    let webtag = &handler.webtag();
    let excluded = hwnd_from_handle(handler.native_view().window);
    let Some(host) = active_host_window_except(Some(excluded)) else {
        show_webview_window(webtag, title, true)?;
        mark_panel_visible(panel_id, true);
        return Ok(());
    };

    // Re-presenting the panel already visible in this host with the same
    // registration (a layout-plan recommit, e.g. on a page switch) must not
    // blink the webview: skip the hide/show cycle and refresh the layout.
    let already_docked = webtag_is_visible(webtag.key())
        && is_panel_visible(panel_id)
        && window_handle_for_key(webtag.key()).is_some_and(|window| window == host)
        && WEBVIEW_PANELS
            .get()
            .and_then(|panels| panels.lock().ok())
            .and_then(|panels| {
                panels.get(panel_id).map(|panel| {
                    panel.webtag_key == webtag.key()
                        && panel.position == position
                        && panel.requested_size == preferred_size
                })
            })
            .unwrap_or(false);
    if already_docked {
        register_webview_panel(panel_id, webtag, title, position, preferred_size);
        sync_window_layout(host);
        invalidate_window_chrome(host);
        return Ok(());
    }

    handler.set_content_visible(false)?;
    set_window_handle(webtag.key(), host);
    register_webview_panel(panel_id, webtag, title, position, preferred_size);
    mark_panel_visible(panel_id, true);
    sync_window_layout(host);
    if excluded != host && is_window_visible(excluded) {
        unsafe {
            let _ = WindowsAndMessaging::SetWindowPos(
                excluded,
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
    handler.set_content_visible(true)?;
    notify_webtag_visibility(webtag.key(), true);
    invalidate_window_chrome(host);
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
        set_primary_host_window(hwnd);
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
    set_primary_host_window(host);
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
    // Own the overlay by the active host window so it tracks the host on
    // minimize/restore and stays above it in z-order — contained within the host
    // (e.g. the device frame) instead of floating as an independent top-level
    // window that lingers when the host is minimized. Applies to every overlay
    // surface (page or URL), mirroring the macOS surface sheet.
    if let Some(owner) = active_host_window_except(Some(hwnd)) {
        unsafe {
            let _ = WindowsAndMessaging::SetWindowLongPtrW(
                hwnd,
                WindowsAndMessaging::GWLP_HWNDPARENT,
                owner.0 as isize,
            );
        }
    }
    // A managed overlay is not user-resizable. The shared webview-parent window
    // carries WS_SIZEBOX, whose non-client frame insets the WebView2 client a few
    // px from the window rect — and we size the *window* to the content rect, so
    // the *client* (what paints) ends up narrower, leaving a strip of the webview
    // beneath showing at the surface edges. Strip the sizing border so the client
    // fills the window and the overlay covers the content exactly.
    unsafe {
        let style = WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWL_STYLE);
        let stripped = style & !(WS_SIZEBOX.0 as isize);
        if stripped != style {
            WindowsAndMessaging::SetWindowLongPtrW(hwnd, WindowsAndMessaging::GWL_STYLE, stripped);
        }
    }
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
            WindowsAndMessaging::SWP_SHOWWINDOW | WindowsAndMessaging::SWP_FRAMECHANGED,
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
    // Size/place the overlay against the HOST (the device-frame content window),
    // never the active webtag: once an overlay is presented it *becomes* the
    // active webtag, so referencing the active content here would measure the
    // overlay against itself and let its size drift on every hide/show. Excluding
    // the overlay (`hwnd`) pins it to the device-frame content instead.
    if let Some(host) = active_host_window_except(Some(hwnd))
        && let Some(host_key) = active_webtag_key_for_window(host)
    {
        let client = content_rect_for_window(host, &host_key);
        let width = client.right - client.left;
        let height = client.bottom - client.top;
        if width > OVERLAY_MIN_WIDTH && height > OVERLAY_MIN_HEIGHT {
            let mut origin = windows::Win32::Foundation::POINT {
                x: client.left,
                y: client.top,
            };
            unsafe {
                let _ = windows::Win32::Graphics::Gdi::ClientToScreen(host, &mut origin);
            }
            return RECT {
                left: origin.x,
                top: origin.y,
                right: origin.x + width,
                bottom: origin.y + height,
            };
        }
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
    let bounds_width = (bounds.right - bounds.left).max(1);
    let bounds_height = (bounds.bottom - bounds.top).max(1);
    // Honor the caller's extent — an absolute size, else a fraction of the host
    // content (`*_ratio`), else the floating-card default — always clamped to the
    // host content so the surface fits inside it (e.g. the device frame). A ratio
    // is a fraction of the content, so `width_ratio = 1.0` is a full-width sheet;
    // this matches the cross-platform surface contract honored on macOS/iOS.
    let overlay_width = resolve_overlay_extent(
        width,
        width_ratio,
        bounds_width,
        OVERLAY_DEFAULT_WIDTH,
        OVERLAY_MIN_WIDTH,
    );
    let overlay_height = resolve_overlay_extent(
        height,
        height_ratio,
        bounds_height,
        OVERLAY_DEFAULT_HEIGHT,
        OVERLAY_MIN_HEIGHT,
    );

    // A surface that fills the cross axis is a sheet: flush to its anchored edge
    // and the cross edges (no margin). A smaller one is a floating card: inset by
    // a margin and centered on the free axis.
    let full_w = overlay_width >= bounds_width;
    let full_h = overlay_height >= bounds_height;
    let center_x = bounds.left + (bounds_width - overlay_width) / 2;
    let center_y = bounds.top + (bounds_height - overlay_height) / 2;
    let (x, y) = match position {
        1 => (
            if full_w { bounds.left } else { center_x },
            bounds.bottom - overlay_height - if full_w { 0 } else { OVERLAY_MARGIN },
        ),
        2 => (
            bounds.left + if full_h { 0 } else { OVERLAY_MARGIN },
            if full_h { bounds.top } else { center_y },
        ),
        3 => (
            bounds.right - overlay_width - if full_h { 0 } else { OVERLAY_MARGIN },
            if full_h { bounds.top } else { center_y },
        ),
        4 => (
            if full_w { bounds.left } else { center_x },
            bounds.top + if full_w { 0 } else { OVERLAY_MARGIN },
        ),
        _ => (center_x, center_y),
    };

    let left = x.clamp(bounds.left, (bounds.right - overlay_width).max(bounds.left));
    let top = y.clamp(bounds.top, (bounds.bottom - overlay_height).max(bounds.top));
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
) -> i32 {
    let value = if absolute.is_finite() && absolute > 0.0 {
        absolute.round() as i32
    } else if ratio.is_finite() && ratio > 0.0 {
        (reference as f64 * ratio.clamp(0.0, 1.0)).round() as i32
    } else {
        fallback
    };
    // Never exceed the host content; floor at the min capped to the content so a
    // frame narrower than the min can still host a (full-bleed) surface.
    value.clamp(min.min(reference), reference)
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
                docked: panel_position_is_flush_docked(position),
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
    request_host_layout_sync(hwnd_from_handle(window.window))
}

#[cfg(feature = "device-frame")]
pub(crate) fn request_host_window_layout_forced(window: WindowsHostWindow) -> bool {
    if window.window == 0 {
        return false;
    }
    request_host_layout_sync_forced(hwnd_from_handle(window.window))
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
            // The browser aside's toolbar occupies the panel's top row; the
            // webview fills the rest.
            let mut rect = panel.rect;
            if let Some(header) = panel.header_rect {
                rect.top = header.bottom.clamp(rect.top, rect.bottom);
            }
            return normalize_rect(rect);
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
    // The layout pass creates windows (overlays, panels) and moves them in the
    // z-order. Both must happen on the window's own thread: a helper window
    // created on a non-pumping thread (e.g. a tokio worker) permanently wedges
    // every later cross-thread SetWindowPos over the owner group.
    let owner_thread = unsafe { WindowsAndMessaging::GetWindowThreadProcessId(hwnd, None) };
    if owner_thread != 0 && owner_thread != unsafe { GetCurrentThreadId() } {
        let handle = hwnd_handle(hwnd);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let posted = post_to_window_thread(
            handle,
            Box::new(move || {
                sync_window_layout(hwnd_from_handle(handle));
                let _ = done_tx.send(());
            }),
        );
        // Present flows sequence a content show after this call, so the
        // marshaled pass must complete before returning — a page shown with
        // its stale (full-client) bounds flashes over the docked panels.
        // The window thread pumps while we wait; time out defensively.
        if posted
            && done_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .is_err()
        {
            log::warn!("marshaled window layout pass timed out");
        }
        return;
    }
    let Some(webtag_key) = active_webtag_key_for_window(hwnd) else {
        #[cfg(feature = "shell-chrome")]
        sync_transparent_tabbar_overlay(hwnd, None);
        #[cfg(feature = "device-frame")]
        crate::device_frame::set_device_frame_overlays_visible(hwnd_handle(hwnd), false);
        return;
    };
    sync_webtag_content_bounds(hwnd, &webtag_key);

    let mut visible_webtags = HashSet::from([webtag_key.clone()]);
    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    let mut native_panel_takes_focus = focused_input_host_panel().is_some();
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
            native_panel_takes_focus = true;
            collapse_obscured_webview_panels(hwnd, &laid_out_panels);
        }
    }
    reconcile_host_webview_visibility(hwnd, &visible_webtags);
    #[cfg(feature = "device-frame")]
    crate::device_frame::set_device_frame_overlays_visible(
        hwnd_handle(hwnd),
        visible_webtags.contains(&webtag_key) && !native_panel_takes_focus,
    );
    #[cfg(not(feature = "device-frame"))]
    let _ = native_panel_takes_focus;
    #[cfg(feature = "shell-chrome")]
    sync_transparent_tabbar_overlay(hwnd, Some(&webtag_key));
}

#[cfg(feature = "shell-chrome")]
fn sync_transparent_tabbar_overlay(hwnd: HWND, webtag_key: Option<&str>) {
    let handle = hwnd_handle(hwnd);
    let overlay_layout = webtag_key.and_then(|webtag_key| {
        if !is_window_visible(hwnd) || is_minimized(hwnd) {
            return None;
        }
        let mut client = RECT::default();
        unsafe {
            if WindowsAndMessaging::GetClientRect(hwnd, &mut client).is_err() {
                return None;
            }
        }
        let layout = current_window_layout(webtag_key);
        crate::shell::transparent_tabbar_overlay_rect(client, &layout).map(|rect| (layout, rect))
    });
    let Some((layout, rect)) =
        overlay_layout.filter(|(_, rect)| rect.right > rect.left && rect.bottom > rect.top)
    else {
        destroy_transparent_tabbar_overlay(hwnd);
        return;
    };

    let overlay = ensure_transparent_tabbar_overlay(hwnd, rect);
    if overlay == 0 {
        return;
    }
    let mut origin = POINT {
        x: rect.left,
        y: rect.top,
    };
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut origin);
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd_from_handle(overlay),
            Some(WindowsAndMessaging::HWND_TOP),
            origin.x,
            origin.y,
            rect.right - rect.left,
            rect.bottom - rect.top,
            WindowsAndMessaging::SWP_NOACTIVATE | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
    }
    upload_transparent_tabbar_overlay(hwnd_from_handle(overlay), &layout);

    if let Ok(mut overlays) = TRANSPARENT_TABBAR_OVERLAYS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        overlays.insert(
            handle,
            TransparentTabbarOverlay {
                window: overlay,
                rect,
            },
        );
    }
    cleanup_transparent_tabbar_overlays(hwnd, Some(overlay));
}

#[cfg(feature = "shell-chrome")]
fn transparent_tabbar_overlay_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let cursor =
            unsafe { WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_ARROW) }
                .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(transparent_tabbar_overlay_proc),
            hInstance: module,
            hCursor: cursor,
            lpszClassName: w!("LingXiaTransparentTabbarOverlay"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "transparent tabbar overlay class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaTransparentTabbarOverlay")
}

#[cfg(feature = "shell-chrome")]
unsafe extern "system" fn transparent_tabbar_overlay_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_PAINT => {
            paint_transparent_tabbar_overlay(hwnd);
            LRESULT(0)
        }
        WindowsAndMessaging::WM_ERASEBKGND => LRESULT(1),
        WindowsAndMessaging::WM_NCHITTEST => LRESULT(WindowsAndMessaging::HTCLIENT as isize),
        WindowsAndMessaging::WM_MOUSEMOVE => {
            if let Some((host, point)) = transparent_tabbar_overlay_host_point(hwnd, lparam) {
                let _ = handle_chrome_mouse_move(host, point, false);
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_LBUTTONDOWN => {
            if let Some((host, point)) = transparent_tabbar_overlay_host_point(hwnd, lparam) {
                unsafe {
                    let _ = SetFocus(Some(host));
                }
                let _ = handle_chrome_left_down(host, point);
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_LBUTTONUP => {
            if let Some((host, point)) = transparent_tabbar_overlay_host_point(hwnd, lparam) {
                let _ = handle_chrome_left_up(host, point);
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_LBUTTONDBLCLK => {
            if let Some((host, point)) = transparent_tabbar_overlay_host_point(hwnd, lparam) {
                let _ = handle_chrome_left_double_click(host, point);
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_RBUTTONUP => {
            if let Some((host, point)) = transparent_tabbar_overlay_host_point(hwnd, lparam) {
                let _ = handle_chrome_right_up(host, point);
            }
            LRESULT(0)
        }
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(feature = "shell-chrome")]
fn transparent_tabbar_overlay_host_point(hwnd: HWND, lparam: LPARAM) -> Option<(HWND, (i32, i32))> {
    let (host, rect) = transparent_tabbar_overlay_entry(hwnd)?;
    let local = lparam_client_point(lparam);
    Some((host, (rect.left + local.0, rect.top + local.1)))
}

#[cfg(feature = "shell-chrome")]
fn transparent_tabbar_overlay_entry(hwnd: HWND) -> Option<(HWND, RECT)> {
    let overlay_handle = hwnd_handle(hwnd);
    TRANSPARENT_TABBAR_OVERLAYS
        .get()
        .and_then(|overlays| overlays.lock().ok())
        .and_then(|overlays| {
            overlays
                .iter()
                .find(|(_, overlay)| overlay.window == overlay_handle)
                .map(|(host, overlay)| (hwnd_from_handle(*host), overlay.rect))
        })
}

#[cfg(feature = "shell-chrome")]
fn paint_transparent_tabbar_overlay(hwnd: HWND) {
    let Some((host, _)) = transparent_tabbar_overlay_entry(hwnd) else {
        validate_empty_paint(hwnd);
        return;
    };
    let layout =
        active_webtag_key_for_window(host).map(|webtag_key| current_window_layout(&webtag_key));
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let _ = BeginPaint(hwnd, &mut ps);
        let _ = EndPaint(hwnd, &ps);
    }
    if let Some(layout) = layout.as_ref() {
        upload_transparent_tabbar_overlay(hwnd, layout);
    }
}

#[cfg(feature = "shell-chrome")]
fn upload_transparent_tabbar_overlay(hwnd: HWND, layout: &WindowsWindowLayout) {
    let mut client = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetClientRect(hwnd, &mut client).is_err() {
            return;
        }
    }
    let width = (client.right - client.left).max(1);
    let height = (client.bottom - client.top).max(1);
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return;
        }
        let dc = CreateCompatibleDC(Some(screen));
        if dc.is_invalid() {
            let _ = ReleaseDC(None, screen);
            return;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(dc);
            let _ = ReleaseDC(None, screen);
            return;
        };
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            let _ = ReleaseDC(None, screen);
            return;
        }
        let old_bitmap = SelectObject(dc, HGDIOBJ(bitmap.0));
        let pixel_count = (width * height) as usize;
        std::ptr::write_bytes(
            bits.cast::<u8>(),
            0,
            pixel_count * std::mem::size_of::<u32>(),
        );
        crate::shell::paint_transparent_tabbar_overlay(dc, layout, width, height);
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u32>(), pixel_count);
        for pixel in pixels {
            if (*pixel >> 24) == 0 && (*pixel & 0x00ff_ffff) != 0 {
                let r = (*pixel >> 16) & 0xff;
                let g = (*pixel >> 8) & 0xff;
                let b = *pixel & 0xff;
                let max = r.max(g).max(b);
                let min = r.min(g).min(b);
                if max <= 180 && max.saturating_sub(min) <= 8 {
                    let alpha = ((max * 255 + 76) / 153).clamp(1, 255);
                    let gray = 153 * alpha / 255;
                    *pixel = (alpha << 24) | (gray << 16) | (gray << 8) | gray;
                } else {
                    *pixel |= max << 24;
                }
            } else if *pixel == 0 {
                // Per-pixel-alpha layered windows let fully transparent pixels
                // fall through to the WebView. Keep the visual transparent but
                // give the whole tabbar strip a 1/255 alpha hit surface so
                // clicks anywhere in a tab cell reach chrome hit-testing.
                *pixel = 0x0100_0000;
            }
        }
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let origin = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let _ = WindowsAndMessaging::UpdateLayeredWindow(
            hwnd,
            Some(screen),
            None,
            Some(&size),
            Some(dc),
            Some(&origin),
            COLORREF(0),
            Some(&blend),
            WindowsAndMessaging::ULW_ALPHA,
        );
        if !old_bitmap.is_invalid() {
            let _ = SelectObject(dc, old_bitmap);
        }
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
        let _ = ReleaseDC(None, screen);
    }
}

#[cfg(feature = "shell-chrome")]
fn validate_empty_paint(hwnd: HWND) {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let _ = BeginPaint(hwnd, &mut ps);
        let _ = EndPaint(hwnd, &ps);
    }
}

#[cfg(feature = "shell-chrome")]
fn ensure_transparent_tabbar_overlay(hwnd: HWND, rect: RECT) -> isize {
    let handle = hwnd_handle(hwnd);
    if let Some(existing) = TRANSPARENT_TABBAR_OVERLAYS
        .get()
        .and_then(|overlays| overlays.lock().ok())
        .and_then(|overlays| overlays.get(&handle).copied())
        && is_window_handle_valid(existing.window)
    {
        return existing.window;
    }

    let class = transparent_tabbar_overlay_class();
    let instance = unsafe { LibraryLoader::GetModuleHandleW(None) }
        .ok()
        .map(|module| HINSTANCE(module.0));
    let overlay = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_NOACTIVATE,
            class,
            PCWSTR::null(),
            WindowsAndMessaging::WS_POPUP,
            0,
            0,
            rect.right - rect.left,
            rect.bottom - rect.top,
            Some(hwnd),
            None,
            instance,
            None,
        )
    };
    let Ok(overlay) = overlay else {
        return 0;
    };
    hwnd_handle(overlay)
}

#[cfg(feature = "shell-chrome")]
fn destroy_transparent_tabbar_overlay(hwnd: HWND) {
    let overlay = TRANSPARENT_TABBAR_OVERLAYS
        .get()
        .and_then(|overlays| overlays.lock().ok())
        .and_then(|mut overlays| overlays.remove(&hwnd_handle(hwnd)));
    if let Some(overlay) = overlay
        && is_window_handle_valid(overlay.window)
    {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(overlay.window));
        }
    }
    cleanup_transparent_tabbar_overlays(hwnd, None);
}

#[cfg(feature = "shell-chrome")]
fn cleanup_transparent_tabbar_overlays(owner: HWND, keep: Option<isize>) {
    struct CleanupState {
        owner: isize,
        keep: Option<isize>,
        pid: u32,
        destroy: Vec<isize>,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
        unsafe {
            let state = &mut *(lparam.0 as *mut CleanupState);
            let handle = hwnd_handle(hwnd);
            if state.keep == Some(handle) {
                return windows::core::BOOL(1);
            }

            let mut pid = 0u32;
            let _ = WindowsAndMessaging::GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid != state.pid || window_class_name(hwnd) != "LingXiaTransparentTabbarOverlay" {
                return windows::core::BOOL(1);
            }

            let overlay_owner = WindowsAndMessaging::GetWindow(hwnd, WindowsAndMessaging::GW_OWNER)
                .unwrap_or_default();
            let owner_handle = hwnd_handle(overlay_owner);
            let owner_invalid = owner_handle == 0 || !is_window_handle_valid(owner_handle);
            if owner_handle == state.owner || owner_invalid {
                state.destroy.push(handle);
            }
            windows::core::BOOL(1)
        }
    }

    let mut state = CleanupState {
        owner: hwnd_handle(owner),
        keep,
        pid: unsafe { GetCurrentProcessId() },
        destroy: Vec::new(),
    };
    let _ = unsafe {
        WindowsAndMessaging::EnumWindows(
            Some(enum_proc),
            LPARAM(&mut state as *mut CleanupState as isize),
        )
    };
    for handle in state.destroy {
        if is_window_handle_valid(handle) {
            unsafe {
                let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(handle));
            }
        }
    }
}

#[cfg(feature = "shell-chrome")]
fn sync_sidebar_tabbar_popup(hwnd: HWND, point: (i32, i32)) {
    let Some(webtag_key) = active_webtag_key_for_window(hwnd) else {
        destroy_sidebar_tabbar_popup(hwnd);
        return;
    };
    let mut client = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetClientRect(hwnd, &mut client).is_err() {
            destroy_sidebar_tabbar_popup(hwnd);
            return;
        }
    }
    let layout = current_window_layout(&webtag_key);
    let Some(popup) = crate::shell::collapsed_sidebar_tabbar_popup(client, &layout, point) else {
        maybe_destroy_sidebar_tabbar_popup(hwnd);
        return;
    };

    let mut origin = POINT {
        x: popup.popup.left,
        y: popup.popup.top,
    };
    let mut anchor_left_top = POINT {
        x: popup.anchor.left,
        y: popup.anchor.top,
    };
    let mut anchor_right_bottom = POINT {
        x: popup.anchor.right,
        y: popup.anchor.bottom,
    };
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut origin);
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut anchor_left_top);
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut anchor_right_bottom);
    }
    let width = popup.popup.right - popup.popup.left;
    let height = popup.popup.bottom - popup.popup.top;
    let popup_rect = RECT {
        left: origin.x,
        top: origin.y,
        right: origin.x + width,
        bottom: origin.y + height,
    };
    let anchor = RECT {
        left: anchor_left_top.x,
        top: anchor_left_top.y,
        right: anchor_right_bottom.x,
        bottom: anchor_right_bottom.y,
    };
    // Called per mouse move over the rail anchor; skip when the popup already
    // shows this content at this place, else it would repaint continuously.
    if let Some(existing) = sidebar_tabbar_popup_for_owner(hwnd)
        && is_window_handle_valid(existing.window)
        && existing.rect == popup_rect
        && existing.anchor == anchor
        && existing.tabbar == popup.tabbar
    {
        return;
    }
    let window = ensure_sidebar_tabbar_popup(hwnd);
    if window == 0 {
        return;
    }
    if let Ok(mut popups) = SIDEBAR_TABBAR_POPUPS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        popups.insert(
            hwnd_handle(hwnd),
            SidebarTabbarPopup {
                window,
                owner: hwnd_handle(hwnd),
                anchor,
                rect: popup_rect,
                tabbar: popup.tabbar.clone(),
            },
        );
    }
    let popup_window = hwnd_from_handle(window);
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            popup_window,
            Some(WindowsAndMessaging::HWND_TOP),
            popup_rect.left,
            popup_rect.top,
            width,
            height,
            WindowsAndMessaging::SWP_NOACTIVATE | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
        let _ = WindowsAndMessaging::SetTimer(
            Some(popup_window),
            SIDEBAR_TABBAR_POPUP_TIMER_ID,
            SIDEBAR_TABBAR_POPUP_TIMER_MS,
            None,
        );
    }
    upload_sidebar_tabbar_popup(popup_window, &popup.tabbar, width, height);
}

/// Uploads the collapsed-rail tabbar popup as a per-pixel-alpha layered
/// window. The rounded shape comes from an anti-aliased alpha mask -
/// `SetWindowRgn`'s aliased clip leaves stair-stepped corner edges.
#[cfg(feature = "shell-chrome")]
fn upload_sidebar_tabbar_popup(
    hwnd: HWND,
    tabbar: &crate::shell::WindowsShellTabBarLayout,
    width: i32,
    height: i32,
) {
    if width <= 0 || height <= 0 {
        return;
    }
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return;
        }
        let dc = CreateCompatibleDC(Some(screen));
        if dc.is_invalid() {
            let _ = ReleaseDC(None, screen);
            return;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(dc);
            let _ = ReleaseDC(None, screen);
            return;
        };
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            let _ = ReleaseDC(None, screen);
            return;
        }
        let old_bitmap = SelectObject(dc, HGDIOBJ(bitmap.0));
        crate::shell::paint_collapsed_sidebar_tabbar_popup(dc, tabbar, width, height);
        let pixel_count = (width * height) as usize;
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u32>(), pixel_count);
        apply_rounded_alpha_mask(
            pixels,
            width,
            height,
            crate::shell::SIDEBAR_TABBAR_POPUP_RADIUS,
        );
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let origin = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let _ = WindowsAndMessaging::UpdateLayeredWindow(
            hwnd,
            Some(screen),
            None,
            Some(&size),
            Some(dc),
            Some(&origin),
            COLORREF(0),
            Some(&blend),
            WindowsAndMessaging::ULW_ALPHA,
        );
        if !old_bitmap.is_invalid() {
            let _ = SelectObject(dc, old_bitmap);
        }
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
        let _ = ReleaseDC(None, screen);
    }
}

/// Sets each pixel's alpha to its rounded-rect coverage (premultiplied, as
/// `UpdateLayeredWindow` expects), with a 1px anti-aliased corner falloff.
/// GDI-drawn content carries garbage alpha, so the mask is geometric.
#[cfg(feature = "shell-chrome")]
fn apply_rounded_alpha_mask(pixels: &mut [u32], width: i32, height: i32, radius: i32) {
    let coverage_at = |x: i32, y: i32| -> u32 {
        let corner_x = if x < radius {
            radius
        } else if x >= width - radius {
            width - radius
        } else {
            return 255;
        };
        let corner_y = if y < radius {
            radius
        } else if y >= height - radius {
            height - radius
        } else {
            return 255;
        };
        let dx = x as f32 + 0.5 - corner_x as f32;
        let dy = y as f32 + 0.5 - corner_y as f32;
        let distance = (dx * dx + dy * dy).sqrt();
        let coverage = (radius as f32 - distance + 0.5).clamp(0.0, 1.0);
        (coverage * 255.0) as u32
    };
    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) as usize;
            let alpha = coverage_at(x, y);
            let pixel = pixels[index];
            pixels[index] = match alpha {
                255 => 0xff00_0000 | (pixel & 0x00ff_ffff),
                0 => 0,
                alpha => {
                    let premultiply = |channel: u32| (channel * alpha + 127) / 255;
                    (alpha << 24)
                        | (premultiply((pixel >> 16) & 0xff) << 16)
                        | (premultiply((pixel >> 8) & 0xff) << 8)
                        | premultiply(pixel & 0xff)
                }
            };
        }
    }
}

#[cfg(feature = "shell-chrome")]
fn maybe_destroy_sidebar_tabbar_popup(owner: HWND) {
    let Some(popup) = sidebar_tabbar_popup_for_owner(owner) else {
        return;
    };
    let mut cursor = POINT::default();
    unsafe {
        if WindowsAndMessaging::GetCursorPos(&mut cursor).is_err() {
            destroy_sidebar_tabbar_popup(owner);
            return;
        }
    }
    if !point_in_screen_rect(cursor, popup.anchor) && !point_in_screen_rect(cursor, popup.rect) {
        destroy_sidebar_tabbar_popup(owner);
    }
}

#[cfg(feature = "shell-chrome")]
fn point_in_screen_rect(point: POINT, rect: RECT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

#[cfg(feature = "shell-chrome")]
fn ensure_sidebar_tabbar_popup(owner: HWND) -> isize {
    if let Some(existing) = sidebar_tabbar_popup_for_owner(owner)
        && is_window_handle_valid(existing.window)
    {
        return existing.window;
    }
    let result = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_NOACTIVATE,
            sidebar_tabbar_popup_class(),
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            1,
            1,
            Some(owner),
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    };
    match result {
        Ok(window) => hwnd_handle(window),
        Err(err) => {
            log::warn!("collapsed sidebar tabbar popup creation failed: {err}");
            0
        }
    }
}

#[cfg(feature = "shell-chrome")]
fn sidebar_tabbar_popup_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let cursor =
            unsafe { WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_ARROW) }
                .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(sidebar_tabbar_popup_proc),
            hInstance: module,
            hCursor: cursor,
            lpszClassName: w!("LingXiaSidebarTabbarPopup"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "sidebar tabbar popup class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaSidebarTabbarPopup")
}

#[cfg(feature = "shell-chrome")]
unsafe extern "system" fn sidebar_tabbar_popup_proc(
    hwnd: HWND,
    msg: u32,
    _wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_MOUSEACTIVATE => {
            return LRESULT(WindowsAndMessaging::MA_NOACTIVATE as isize);
        }
        WindowsAndMessaging::WM_PAINT => {
            paint_sidebar_tabbar_popup(hwnd);
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_ERASEBKGND => {
            return LRESULT(1);
        }
        WindowsAndMessaging::WM_LBUTTONUP => {
            if let Some(popup) = sidebar_tabbar_popup_for_window(hwnd) {
                let point = lparam_client_point(lparam);
                if let Some(index) =
                    crate::shell::collapsed_sidebar_tabbar_popup_hit(&popup.tabbar, point)
                    && let Some(webtag_key) =
                        active_webtag_key_for_window(hwnd_from_handle(popup.owner))
                {
                    let command = crate::shell::collapsed_sidebar_tabbar_click_command(index);
                    invoke_chrome_command(
                        &webtag_key,
                        hwnd_from_handle(popup.owner),
                        (0, 0),
                        command,
                    );
                }
                destroy_sidebar_tabbar_popup(hwnd_from_handle(popup.owner));
            }
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_TIMER if _wparam.0 == SIDEBAR_TABBAR_POPUP_TIMER_ID => {
            if let Some(popup) = sidebar_tabbar_popup_for_window(hwnd) {
                maybe_destroy_sidebar_tabbar_popup(hwnd_from_handle(popup.owner));
            }
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_DESTROY | WindowsAndMessaging::WM_NCDESTROY => {
            remove_sidebar_tabbar_popup_window(hwnd);
        }
        _ => {}
    }
    unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, _wparam, lparam) }
}

#[cfg(feature = "shell-chrome")]
fn paint_sidebar_tabbar_popup(hwnd: HWND) {
    let Some(popup) = sidebar_tabbar_popup_for_window(hwnd) else {
        return;
    };
    // A per-pixel-alpha layered window paints via UpdateLayeredWindow, not
    // WM_PAINT; validate the region and refresh the uploaded surface.
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let _ = BeginPaint(hwnd, &mut ps);
        let _ = EndPaint(hwnd, &ps);
    }
    let width = popup.rect.right - popup.rect.left;
    let height = popup.rect.bottom - popup.rect.top;
    upload_sidebar_tabbar_popup(hwnd, &popup.tabbar, width, height);
}

#[cfg(feature = "shell-chrome")]
fn sidebar_tabbar_popup_for_owner(owner: HWND) -> Option<SidebarTabbarPopup> {
    SIDEBAR_TABBAR_POPUPS
        .get()
        .and_then(|popups| popups.lock().ok())
        .and_then(|popups| popups.get(&hwnd_handle(owner)).cloned())
}

#[cfg(feature = "shell-chrome")]
fn sidebar_tabbar_popup_for_window(window: HWND) -> Option<SidebarTabbarPopup> {
    let handle = hwnd_handle(window);
    SIDEBAR_TABBAR_POPUPS
        .get()
        .and_then(|popups| popups.lock().ok())
        .and_then(|popups| {
            popups
                .values()
                .find(|popup| popup.window == handle)
                .cloned()
        })
}

#[cfg(feature = "shell-chrome")]
fn destroy_sidebar_tabbar_popup(owner: HWND) {
    let removed = SIDEBAR_TABBAR_POPUPS
        .get()
        .and_then(|popups| popups.lock().ok())
        .and_then(|mut popups| popups.remove(&hwnd_handle(owner)));
    if let Some(popup) = removed
        && is_window_handle_valid(popup.window)
    {
        unsafe {
            let _ = WindowsAndMessaging::KillTimer(
                Some(hwnd_from_handle(popup.window)),
                SIDEBAR_TABBAR_POPUP_TIMER_ID,
            );
            let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(popup.window));
        }
    }
}

#[cfg(feature = "shell-chrome")]
fn remove_sidebar_tabbar_popup_window(window: HWND) {
    let handle = hwnd_handle(window);
    if let Some(popups) = SIDEBAR_TABBAR_POPUPS.get()
        && let Ok(mut popups) = popups.lock()
        && let Some(owner) = popups
            .iter()
            .find(|(_, popup)| popup.window == handle)
            .map(|(owner, _)| *owner)
    {
        popups.remove(&owner);
    }
}

#[cfg(feature = "shell-chrome")]
fn window_class_name(hwnd: HWND) -> String {
    let mut buffer = [0u16; 96];
    let copied = unsafe { WindowsAndMessaging::GetClassNameW(hwnd, &mut buffer) };
    if copied <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buffer[..copied as usize])
    }
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
    if webtag_content_bounds_changed(webtag_key, host_bounds)
        && let Err(err) = handler.set_content_bounds(
            controller_bounds.left,
            controller_bounds.top,
            controller_bounds.right - controller_bounds.left,
            controller_bounds.bottom - controller_bounds.top,
        )
    {
        log::debug!("Failed to sync Windows WebView content bounds: {err}");
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
    // A transiently invisible host (frame-style churn during a present, or a
    // minimize) must not strip its webviews: nothing is on screen anyway, and
    // hiding the docked panel here blinks it once the host is back. The next
    // layout pass on a visible host reconciles.
    if !is_window_visible(hwnd) || is_minimized(hwnd) {
        return;
    }
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
        let should_show = visible_webtags.contains(&key);
        match (should_show, webtag_is_visible(&key)) {
            (true, false) => to_show.push((key, handler)),
            (false, true) => to_hide.push((key, handler)),
            _ => {}
        }
    }

    // WebView2 controller visibility changes are not atomic across
    // controllers. Showing incoming surfaces first avoids exposing the host
    // background for a frame during host/child surface transitions.
    for (key, handler) in to_show {
        show_reconciled_webview(&key, &handler);
    }
    for (key, handler) in to_hide {
        hide_reconciled_webview(&key, &handler);
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
                webtag_key: panel.webtag_key.clone(),
                title: webview_panel_title(&panel_id),
                rect: panel.rect,
                header_rect: panel.header_rect,
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

fn webview_panel_title(panel_id: &str) -> String {
    WEBVIEW_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .and_then(|panels| panels.get(panel_id).map(|panel| panel.title.clone()))
        .unwrap_or_default()
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
    let interaction = chrome_interaction(hwnd);
    let state = WindowsChromeState {
        hwnd,
        client,
        layout: current_window_layout(&webtag_key),
        attached: attached_state_for_window(hwnd, &webtag_key, client),
        frame_button_hover: interaction.frame_button_hover,
        frame_button_pressed: interaction.frame_button_pressed,
        cursor: interaction.cursor,
    };
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let invalid = ps.rcPaint;
        if invalid.right > invalid.left && invalid.bottom > invalid.top {
            let width = client.right - client.left;
            let height = client.bottom - client.top;
            match chrome_back_buffer_dc(hwnd, width, height) {
                Some(buffer) => {
                    let saved = SaveDC(buffer);
                    let _ = IntersectClipRect(
                        buffer,
                        invalid.left,
                        invalid.top,
                        invalid.right,
                        invalid.bottom,
                    );
                    paint_chrome_into(
                        buffer,
                        hwnd,
                        &webtag_key,
                        renderer.as_ref(),
                        &state,
                        invalid,
                    );
                    let _ = RestoreDC(buffer, saved);
                    // No WS_CLIPCHILDREN: the blit needs the same webview
                    // exclusions as the render or it would stamp over live
                    // WebView2 children.
                    let saved = SaveDC(hdc);
                    exclude_host_webview_content_from_paint(hdc, hwnd, &webtag_key);
                    let _ = BitBlt(
                        hdc,
                        invalid.left,
                        invalid.top,
                        invalid.right - invalid.left,
                        invalid.bottom - invalid.top,
                        Some(buffer),
                        invalid.left,
                        invalid.top,
                        SRCCOPY,
                    );
                    let _ = RestoreDC(hdc, saved);
                }
                None => {
                    paint_chrome_into(hdc, hwnd, &webtag_key, renderer.as_ref(), &state, invalid)
                }
            }
        }
        let _ = EndPaint(hwnd, &ps);
    }
}

fn paint_chrome_into(
    hdc: HDC,
    hwnd: HWND,
    webtag_key: &str,
    renderer: &dyn lingxia_windows_contract::WindowsChromeRenderer,
    state: &WindowsChromeState,
    invalid: RECT,
) {
    unsafe {
        let saved = SaveDC(hdc);
        exclude_host_webview_content_from_paint(hdc, hwnd, webtag_key);
        renderer.paint_region(hdc, state, invalid);
        let _ = RestoreDC(hdc, saved);
    }
    paint_pull_refresh_indicator(hdc, hwnd, webtag_key);
}

/// Clips every visible webview surface hosted by `hwnd` (main content and
/// attached webview panels) out of `hdc`; without WS_CLIPCHILDREN, painting
/// over a live WebView2 child flashes it until the child repaints.
fn exclude_host_webview_content_from_paint(hdc: HDC, hwnd: HWND, webtag_key: &str) {
    if let Some(webtag) = webtag_for_key(webtag_key)
        && let Some(handler) = find_webview_handler(&webtag)
        && hwnd_from_handle(handler.native_view().window) == hwnd
    {
        exclude_clip_rect_if_non_empty(hdc, content_rect_for_window(hwnd, webtag_key));
    }

    let mut client = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetClientRect(hwnd, &mut client).is_err() {
            return;
        }
    }
    let Some(attached) = attached_layout_for_window(hwnd, webtag_key, client) else {
        return;
    };
    for panel in &attached.panels {
        // Native (chrome-drawn) panels have no webview; a not-yet-visible
        // webview panel still wants its placeholder card painted.
        if !panel.webtag_key.is_empty() && webtag_is_visible(&panel.webtag_key) {
            // The browser aside's toolbar row is chrome; only the webview
            // area below it leaves the paint clip.
            let mut rect = panel.rect;
            if let Some(header) = panel.header_rect {
                rect.top = header.bottom.clamp(rect.top, rect.bottom);
            }
            exclude_clip_rect_if_non_empty(hdc, rect);
        }
    }
}

fn exclude_clip_rect_if_non_empty(hdc: HDC, rect: RECT) {
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
    active_host_window_except(None)
}

fn active_host_window_except(excluded: Option<HWND>) -> Option<HWND> {
    ACTIVE_WEBTAG
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.as_ref().map(|webtag| webtag.key().to_string()))
        .and_then(|key| window_handle_for_key(&key))
        .filter(|hwnd| Some(*hwnd) != excluded)
        .or_else(|| primary_host_window_except(excluded))
        .or_else(|| focused_registered_host_window_except(excluded))
        .or_else(|| first_visible_registered_host_window_except(excluded))
}

fn set_primary_host_window(hwnd: HWND) {
    let slot = PRIMARY_HOST_WINDOW.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(hwnd_handle(hwnd));
    }
}

fn primary_host_window_except(excluded: Option<HWND>) -> Option<HWND> {
    let hwnd = PRIMARY_HOST_WINDOW
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.map(hwnd_from_handle))?;
    if Some(hwnd) == excluded || !is_valid_host_window(hwnd) {
        return None;
    }
    Some(hwnd)
}

fn registered_host_windows() -> Vec<HWND> {
    let Some(handles) = WEBTAG_WINDOWS.get().and_then(|handles| handles.lock().ok()) else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    handles
        .values()
        .copied()
        .filter(|handle| seen.insert(*handle))
        .map(hwnd_from_handle)
        .filter(|hwnd| unsafe { WindowsAndMessaging::IsWindow(Some(*hwnd)).as_bool() })
        .filter(|hwnd| is_top_level_window(*hwnd))
        .collect()
}

fn is_valid_host_window(hwnd: HWND) -> bool {
    (unsafe { WindowsAndMessaging::IsWindow(Some(hwnd)).as_bool() })
        && is_top_level_window(hwnd)
        && !is_minimized(hwnd)
}

fn focused_registered_host_window_except(excluded: Option<HWND>) -> Option<HWND> {
    let focused = unsafe { WindowsAndMessaging::GetForegroundWindow() };
    if focused.0.is_null() {
        return None;
    }
    if Some(focused) == excluded {
        return None;
    }
    registered_host_windows()
        .into_iter()
        .find(|candidate| *candidate == focused)
}

fn first_visible_registered_host_window_except(excluded: Option<HWND>) -> Option<HWND> {
    registered_host_windows()
        .into_iter()
        .find(|hwnd| Some(*hwnd) != excluded && is_window_visible(*hwnd) && !is_minimized(*hwnd))
}

fn sync_active_host_layout() {
    if let Some(hwnd) = active_host_window() {
        request_host_layout_sync(hwnd);
    }
}

fn request_host_layout_sync(hwnd: HWND) -> bool {
    request_host_layout_sync_inner(hwnd, false)
}

#[cfg(feature = "device-frame")]
fn request_host_layout_sync_forced(hwnd: HWND) -> bool {
    request_host_layout_sync_inner(hwnd, true)
}

fn request_host_layout_sync_inner(hwnd: HWND, force_bounds: bool) -> bool {
    let window = hwnd_handle(hwnd);
    if post_to_window_thread(
        window,
        Box::new(move || {
            let hwnd = hwnd_from_handle(window);
            if force_bounds {
                clear_webtag_content_bounds_for_window(window);
            }
            sync_window_layout(hwnd);
            invalidate_window_chrome(hwnd);
        }),
    ) {
        return true;
    }
    if force_bounds {
        clear_webtag_content_bounds_for_window(window);
    }
    sync_window_layout(hwnd);
    invalidate_window_chrome(hwnd);
    true
}

fn clear_webtag_content_bounds_for_window(window: isize) {
    if let Some(bounds) = WEBTAG_CONTENT_BOUNDS.get()
        && let Ok(mut bounds) = bounds.lock()
    {
        bounds.retain(|_, cached| cached.hwnd != window);
    }
}

fn repaint_active_host() {
    if let Some(hwnd) = active_host_window() {
        invalidate_window(hwnd);
    }
}

fn attached_panel_resize_hit(hwnd: HWND, point: (i32, i32)) -> Option<AttachedPanelResizeDrag> {
    let webtag_key = active_webtag_key_for_window(hwnd)?;
    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    let attached = attached_layout_for_window(hwnd, &webtag_key, client)?;
    for panel in attached.panels {
        let Some(handle) = panel.resize_handle else {
            continue;
        };
        if !point_in_rect(&handle, point) {
            continue;
        }
        let position = panel_position_for_registered_panel(&panel.panel_id)?;
        let origin_size = panel_size_for_position(panel.rect, position);
        return Some(AttachedPanelResizeDrag {
            panel_id: panel.panel_id,
            position,
            origin: point,
            origin_size,
        });
    }
    None
}

fn begin_attached_panel_resize_drag(hwnd: HWND, point: (i32, i32)) -> Option<bool> {
    let drag = attached_panel_resize_hit(hwnd, point)?;
    let vertical = panel_resize_is_vertical(drag.position);
    let slot = ATTACHED_PANEL_RESIZE_DRAG.get_or_init(|| Mutex::new(None));
    if let Ok(mut active) = slot.lock() {
        *active = Some(drag);
    }
    Some(vertical)
}

fn update_attached_panel_resize_drag(hwnd: HWND, point: (i32, i32)) -> bool {
    let Some(drag) = ATTACHED_PANEL_RESIZE_DRAG
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|active| active.clone())
    else {
        return false;
    };
    let requested = resized_panel_size(&drag, point);
    set_registered_panel_requested_size(&drag.panel_id, requested);
    sync_window_layout(hwnd);
    invalidate_window_chrome(hwnd);
    true
}

fn end_attached_panel_resize_drag() -> bool {
    ATTACHED_PANEL_RESIZE_DRAG
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|mut active| active.take())
        .is_some()
}

fn attached_panel_resize_drag_vertical() -> Option<bool> {
    ATTACHED_PANEL_RESIZE_DRAG
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|active| {
            active
                .as_ref()
                .map(|drag| panel_resize_is_vertical(drag.position))
        })
}

fn resized_panel_size(drag: &AttachedPanelResizeDrag, point: (i32, i32)) -> i32 {
    let delta = match drag.position {
        WindowsPanelPosition::Left => point.0 - drag.origin.0,
        WindowsPanelPosition::Right => drag.origin.0 - point.0,
        WindowsPanelPosition::Top => point.1 - drag.origin.1,
        WindowsPanelPosition::Bottom => drag.origin.1 - point.1,
    };
    (drag.origin_size + delta).max(1)
}

fn set_registered_panel_requested_size(panel_id: &str, requested_size: i32) {
    let requested_size = Some(requested_size);
    if let Some(panels) = WEBVIEW_PANELS.get()
        && let Ok(mut panels) = panels.lock()
        && let Some(panel) = panels.get_mut(panel_id)
    {
        panel.requested_size = requested_size;
        return;
    }
    if let Some(panels) = HOST_PANELS.get()
        && let Ok(mut panels) = panels.lock()
        && let Some(panel) = panels.get_mut(panel_id)
    {
        panel.requested_size = requested_size;
    }
}

fn panel_position_for_registered_panel(panel_id: &str) -> Option<WindowsPanelPosition> {
    if let Some(position) = WEBVIEW_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .and_then(|panels| panels.get(panel_id).map(|panel| panel.position))
    {
        return Some(position);
    }
    HOST_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .and_then(|panels| panels.get(panel_id).map(|panel| panel.position))
}

fn panel_size_for_position(rect: RECT, position: WindowsPanelPosition) -> i32 {
    match position {
        WindowsPanelPosition::Left | WindowsPanelPosition::Right => (rect.right - rect.left).max(0),
        WindowsPanelPosition::Top | WindowsPanelPosition::Bottom => (rect.bottom - rect.top).max(0),
    }
}

fn panel_resize_is_vertical(position: WindowsPanelPosition) -> bool {
    matches!(
        position,
        WindowsPanelPosition::Left | WindowsPanelPosition::Right
    )
}

fn point_in_rect(rect: &RECT, point: (i32, i32)) -> bool {
    point.0 >= rect.left && point.0 < rect.right && point.1 >= rect.top && point.1 < rect.bottom
}

fn register_webview_panel(
    panel_id: &str,
    webtag: &WebTag,
    title: &str,
    position: WindowsPanelPosition,
    requested_size: Option<i32>,
) {
    let panels = WEBVIEW_PANELS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut panels) = panels.lock() {
        panels.insert(
            panel_id.to_string(),
            WebViewPanelEntry {
                webtag_key: webtag.key().to_string(),
                title: title.to_string(),
                position,
                requested_size,
                docked: panel_position_is_flush_docked(position),
                maximized: false,
            },
        );
    }
}

#[cfg(feature = "shell-chrome")]
pub(crate) fn close_webview_panel(panel_id: &str) {
    let Some(webtag_key) = WEBVIEW_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .and_then(|panels| panels.get(panel_id).map(|panel| panel.webtag_key.clone()))
    else {
        return;
    };
    if !invoke_close_handler(&webtag_key) {
        cleanup_webview_panel(&webtag_key);
    }
    if let Some(hwnd) = active_host_window() {
        sync_window_layout(hwnd);
        invalidate_window_chrome(hwnd);
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

#[cfg(feature = "shell-chrome")]
fn panel_position_for_id(panel_id: &str) -> WindowsPanelPosition {
    lingxia_app_context::app_config()
        .and_then(|config| config.panels.as_ref().cloned())
        .and_then(|panels| panels.items.into_iter().find(|item| item.id == panel_id))
        .map(|item| match item.position {
            lingxia_app_context::PanelPosition::Left => WindowsPanelPosition::Left,
            lingxia_app_context::PanelPosition::Right => WindowsPanelPosition::Right,
            lingxia_app_context::PanelPosition::Top => WindowsPanelPosition::Top,
            lingxia_app_context::PanelPosition::Bottom => WindowsPanelPosition::Bottom,
        })
        .unwrap_or(WindowsPanelPosition::Right)
}

#[cfg(not(feature = "shell-chrome"))]
fn panel_position_for_id(_panel_id: &str) -> WindowsPanelPosition {
    WindowsPanelPosition::Right
}

fn panel_position_is_flush_docked(position: WindowsPanelPosition) -> bool {
    matches!(
        position,
        WindowsPanelPosition::Top | WindowsPanelPosition::Bottom
    )
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

pub fn restore_and_focus_host_window(window: isize) -> bool {
    post_to_window_thread(
        window,
        Box::new(move || unsafe {
            let hwnd = hwnd_from_handle(window);
            let show_command = if WindowsAndMessaging::IsIconic(hwnd).as_bool() {
                WindowsAndMessaging::SW_RESTORE
            } else {
                WindowsAndMessaging::SW_SHOW
            };
            let _ = WindowsAndMessaging::ShowWindow(hwnd, show_command);
            let _ = WindowsAndMessaging::BringWindowToTop(hwnd);
            let _ = WindowsAndMessaging::SetForegroundWindow(hwnd);
            let _ = SetFocus(Some(hwnd));
        }),
    )
}

pub fn hide_host_window(window: isize) -> bool {
    post_to_window_thread(
        window,
        Box::new(move || unsafe {
            let hwnd = hwnd_from_handle(window);
            let _ = WindowsAndMessaging::ShowWindow(hwnd, WindowsAndMessaging::SW_HIDE);
        }),
    )
}

pub fn host_window_is_visible(window: isize) -> bool {
    let hwnd = hwnd_from_handle(window);
    unsafe {
        WindowsAndMessaging::IsWindowVisible(hwnd).as_bool()
            && !WindowsAndMessaging::IsIconic(hwnd).as_bool()
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

fn set_native_framed_window(hwnd: HWND, native: bool) {
    let windows = NATIVE_FRAMED_WINDOWS.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(mut windows) = windows.lock() {
        if native {
            windows.insert(hwnd_handle(hwnd));
        } else {
            windows.remove(&hwnd_handle(hwnd));
        }
    }
}

fn is_native_framed_window(hwnd: HWND) -> bool {
    NATIVE_FRAMED_WINDOWS
        .get()
        .and_then(|windows| windows.lock().ok())
        .is_some_and(|windows| windows.contains(&hwnd_handle(hwnd)))
}

fn clear_native_framed_window(hwnd: HWND) {
    if let Some(windows) = NATIVE_FRAMED_WINDOWS.get()
        && let Ok(mut windows) = windows.lock()
    {
        windows.remove(&hwnd_handle(hwnd));
    }
}

fn apply_native_window_frame(hwnd: HWND) -> StdResult<()> {
    apply_window_style(hwnd, WS_OVERLAPPEDWINDOW)
}

/// True when `hwnd` presents a fixed-size simulated device frame. Such a window
/// must never get a resize border or maximize box — dragging it would break the
/// device's fixed bezel/screen/tab-bar layout (the screen scales but the bezel
/// and corner overlays do not).
fn window_is_device_framed(hwnd: HWND) -> bool {
    #[cfg(feature = "device-frame")]
    {
        crate::device_frame::window_has_device_frame(hwnd_handle(hwnd))
    }
    #[cfg(not(feature = "device-frame"))]
    {
        let _ = hwnd;
        false
    }
}

fn apply_shell_window_frame(hwnd: HWND) -> StdResult<()> {
    if windows_chrome_renderer().is_none() {
        return Ok(());
    }
    // A device frame owns a fixed-size borderless silhouette: drop WS_SIZEBOX
    // and WS_MAXIMIZEBOX so it cannot be drag-resized or maximized. Otherwise a
    // normal shell window stays resizable.
    let style = if window_is_device_framed(hwnd) {
        WS_POPUP.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0
    } else {
        WS_POPUP.0 | WS_SIZEBOX.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0
    };
    apply_window_style(hwnd, WINDOW_STYLE(style))
}

fn apply_window_style(hwnd: HWND, style: WINDOW_STYLE) -> StdResult<()> {
    unsafe {
        let _ = WindowsAndMessaging::SetWindowLongPtrW(
            hwnd,
            WindowsAndMessaging::GWL_STYLE,
            style.0 as isize,
        );
        WindowsAndMessaging::SetWindowPos(
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
        )
        .map_err(|err| WebViewError::WebView(format!("SetWindowPos failed: {err}")))?;
    }
    Ok(())
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
        cursor: interaction.cursor,
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

/// Clears element hover state (cursor + hover rect) and repaints the rect
/// that was lit, e.g. when the cursor moves onto a resize handle.
fn clear_chrome_hover(hwnd: HWND) {
    let interaction = chrome_interaction(hwnd);
    if interaction.cursor.is_none() && interaction.hover_rect.is_none() {
        return;
    }
    update_chrome_interaction(hwnd, |state| {
        state.cursor = None;
        state.hover_rect = None;
    });
    if let Some(rect) = interaction.hover_rect {
        invalidate_rect_if_non_empty(hwnd, rect);
    }
}

/// Invalidates only the caption-button rects; frame-button hover/press
/// feedback must not repaint the rest of the chrome.
fn invalidate_frame_buttons(hwnd: HWND) {
    let rects = windows_chrome_renderer()
        .and_then(|renderer| {
            let state = chrome_state_for_window(hwnd)?;
            Some(
                [
                    WindowsFrameButton::Minimize,
                    WindowsFrameButton::Maximize,
                    WindowsFrameButton::Close,
                ]
                .into_iter()
                .filter_map(|button| renderer.frame_button_rect(&state, button))
                .collect::<Vec<_>>(),
            )
        })
        .unwrap_or_default();
    if rects.is_empty() {
        invalidate_window(hwnd);
        return;
    }
    for rect in rects {
        invalidate_rect_if_non_empty(hwnd, rect);
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

#[cfg(feature = "shell-chrome")]
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
    // The lxapp navbar is clipped to the main column and browser asides paint
    // their address bar in the shared top band - both track the main column's
    // width, which only changes when a panel opens/closes/resizes.
    // `shell_chrome_dirty_rects` diffs only the lxapp layout (unchanged on those
    // events), so repaint the top strip here when the *main rect* changes.
    // Gating on the main rect (not on full attached equality) is deliberate: a
    // live aside's frequent re-syncs leave the main width unchanged, so the top
    // strip is not repainted on every tick - which would flicker the navbar,
    // sidebar header, and address bar.
    let previous_main = previous.attached.as_ref().map(|attached| attached.main);
    let current_main = attached.map(|attached| attached.main);
    if previous_main != current_main {
        invalidate_rect_if_non_empty(
            hwnd,
            RECT {
                left: client.left,
                top: client.top,
                right: client.right,
                bottom: (client.top + crate::shell::shell_top_bar_height()).min(client.bottom),
            },
        );
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

#[cfg(not(feature = "shell-chrome"))]
fn invalidate_precise_shell_chrome(
    _hwnd: HWND,
    _client: RECT,
    _layout: &WindowsWindowLayout,
    _attached: Option<&WindowsChromeAttachedLayout>,
) -> bool {
    false
}

#[cfg(feature = "shell-chrome")]
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

#[cfg(feature = "shell-chrome")]
fn push_attached_layout_dirty_rects(dirty: &mut Vec<RECT>, attached: &WindowsChromeAttachedLayout) {
    for panel in &attached.panels {
        push_unique_dirty_rect(dirty, panel.rect);
        // A tab switch changes only the toolbar row; redraw it even when the
        // panel rect itself is unchanged.
        if let Some(header) = panel.header_rect {
            push_unique_dirty_rect(dirty, header);
        }
        if let Some(handle) = panel.resize_handle {
            push_unique_dirty_rect(dirty, handle);
        }
    }
}

#[cfg(feature = "shell-chrome")]
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
    if !dirty.contains(&rect) {
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

/// True while a terminal pane divider is being dragged (a capture loop owned
/// by the shell window proc). `DIVIDER_DRAG_VERTICAL` records its orientation
/// When set (tray-exclusive), host windows are created with
/// WS_EX_TOOLWINDOW so they have no taskbar button and are skipped in Alt-Tab —
/// the app lives only in the system tray. Mirrors macOS LSUIElement / accessory.
static HIDE_FROM_TASKBAR: AtomicBool = AtomicBool::new(false);

/// Set whether host windows should be hidden from the taskbar (tray-only app).
/// Read from `ui.json` `launch.hideDockIcon` at init.
pub fn set_hide_from_taskbar(hide: bool) {
    HIDE_FROM_TASKBAR.store(hide, Ordering::Relaxed);
}

/// for the resize cursor.
#[cfg(feature = "shell-chrome")]
static DIVIDER_DRAG: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "shell-chrome")]
static DIVIDER_DRAG_VERTICAL: AtomicBool = AtomicBool::new(false);

/// Sets the east-west / north-south resize cursor for a divider.
fn set_divider_cursor(vertical: bool) {
    let id = if vertical {
        WindowsAndMessaging::IDC_SIZEWE
    } else {
        WindowsAndMessaging::IDC_SIZENS
    };
    unsafe {
        if let Ok(cursor) = WindowsAndMessaging::LoadCursorW(None, id) {
            let _ = WindowsAndMessaging::SetCursor(Some(cursor));
        }
    }
}

/// `from_host` is false for moves forwarded from an overlay window; arming
/// WM_MOUSELEAVE tracking then would fire immediately (the cursor is over
/// the overlay, not the host) and fight the forwarded moves.
fn handle_chrome_mouse_move(hwnd: HWND, point: (i32, i32), from_host: bool) -> bool {
    let renderer = windows_chrome_renderer();
    let state = chrome_state_for_window(hwnd);
    let hit = match (renderer.as_ref(), state.as_ref()) {
        (Some(renderer), Some(state)) => renderer.hit_test(state, point),
        _ => None,
    };
    if update_attached_panel_resize_drag(hwnd, point) {
        if let Some(vertical) = attached_panel_resize_drag_vertical() {
            set_divider_cursor(vertical);
        }
        return true;
    }
    if let Some(drag) = attached_panel_resize_hit(hwnd, point) {
        clear_chrome_hover(hwnd);
        set_divider_cursor(panel_resize_is_vertical(drag.position));
        return true;
    }
    #[cfg(feature = "shell-chrome")]
    {
        if DIVIDER_DRAG.load(Ordering::Acquire) {
            crate::shell::update_divider_drag(point.0, point.1);
            set_divider_cursor(DIVIDER_DRAG_VERTICAL.load(Ordering::Acquire));
            return true;
        }
        if let Some(WindowsChromeHit::Focusable { id, .. }) = &hit
            && let Some(vertical) = crate::shell::divider_orientation_at(id, point.0, point.1)
        {
            clear_chrome_hover(hwnd);
            set_divider_cursor(vertical);
            return true;
        }
    }
    #[cfg(feature = "shell-chrome")]
    sync_sidebar_tabbar_popup(hwnd, point);
    let hover = match hit {
        Some(WindowsChromeHit::FrameButton(button)) => Some(button),
        _ => None,
    };
    if chrome_interaction(hwnd).frame_button_hover != hover {
        update_chrome_interaction(hwnd, |state| state.frame_button_hover = hover);
        invalidate_frame_buttons(hwnd);
    }

    // Hover feedback: repaint only the rects the cursor left/entered, so
    // movement within one element does not repaint.
    let hover_rect = match (renderer.as_ref(), state.as_ref()) {
        (Some(renderer), Some(state)) => renderer.hover_rect(state, point),
        _ => None,
    };
    let previous = chrome_interaction(hwnd);
    update_chrome_interaction(hwnd, |state| {
        state.cursor = Some(point);
        state.hover_rect = hover_rect;
    });
    if previous.hover_rect != hover_rect {
        for rect in [previous.hover_rect, hover_rect].into_iter().flatten() {
            invalidate_rect_if_non_empty(hwnd, rect);
        }
    }

    if from_host && (hover.is_some() || hover_rect.is_some()) {
        // WM_MOUSELEAVE clears highlights when the cursor exits the window
        // or moves onto the webview child.
        let mut track = TRACKMOUSEEVENT {
            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: hwnd,
            dwHoverTime: 0,
        };
        unsafe {
            let _ = TrackMouseEvent(&mut track);
        }
    }
    matches!(
        hit,
        Some(
            WindowsChromeHit::Chrome
                | WindowsChromeHit::FrameButton(_)
                | WindowsChromeHit::Command(_)
                | WindowsChromeHit::CommandWithContext { .. }
                | WindowsChromeHit::Focusable { .. }
        )
    )
}

fn handle_chrome_left_down(hwnd: HWND, point: (i32, i32)) -> bool {
    if let Some(vertical) = begin_attached_panel_resize_drag(hwnd, point) {
        unsafe {
            let _ = SetCapture(hwnd);
        }
        set_divider_cursor(vertical);
        return true;
    }
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
            invalidate_frame_buttons(hwnd);
            true
        },
        WindowsChromeHit::Focusable {
            id, click_command, ..
        } => {
            // A press on a pane divider starts a resize drag instead of
            // focusing/clicking the pane.
            #[cfg(feature = "shell-chrome")]
            if let Some(vertical) = crate::shell::begin_divider_drag(&id, point.0, point.1) {
                DIVIDER_DRAG.store(true, Ordering::Release);
                DIVIDER_DRAG_VERTICAL.store(vertical, Ordering::Release);
                unsafe {
                    let _ = SetCapture(hwnd);
                }
                set_divider_cursor(vertical);
                return true;
            }
            focus_host_panel(&id);
            focus_host_window(hwnd);
            if let (Some(command), Some(webtag_key)) =
                (click_command, active_webtag_key_for_window(hwnd))
            {
                invoke_chrome_command(&webtag_key, hwnd, point, command);
            }
            true
        }
        WindowsChromeHit::Chrome
        | WindowsChromeHit::Command(_)
        | WindowsChromeHit::CommandWithContext { .. } => true,
    }
}

fn handle_chrome_left_up(hwnd: HWND, point: (i32, i32)) -> bool {
    if attached_panel_resize_drag_vertical().is_some() {
        update_attached_panel_resize_drag(hwnd, point);
        end_attached_panel_resize_drag();
        unsafe {
            let _ = ReleaseCapture();
        }
        return true;
    }
    #[cfg(feature = "shell-chrome")]
    if DIVIDER_DRAG.swap(false, Ordering::AcqRel) {
        unsafe {
            let _ = ReleaseCapture();
        }
        crate::shell::end_divider_drag();
        return true;
    }
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
        invalidate_frame_buttons(hwnd);
        if activate {
            handle_frame_button(hwnd, button);
        }
        return true;
    }
    match hit {
        Some(WindowsChromeHit::Command(command))
        | Some(WindowsChromeHit::CommandWithContext {
            command,
            context_menu: _,
        }) => {
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
        Some(WindowsChromeHit::Command(command))
        | Some(WindowsChromeHit::CommandWithContext {
            command,
            context_menu: _,
        }) => {
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
    if let Some(WindowsChromeHit::CommandWithContext {
        command: _,
        context_menu,
    }) = chrome_hit_for_window(hwnd, point)
    {
        invoke_chrome_command(&webtag_key, hwnd, point, context_menu);
        return true;
    }
    false
}

/// The host window's client width in logical (DIP) units - the value the
/// adaptive surface graph's size class expects (thresholds are DIP: Compact
/// `<600`, Medium `600..=840`, Expanded `>840`).
#[cfg(feature = "shell-chrome")]
fn window_logical_client_width(hwnd: HWND) -> f64 {
    let mut client = RECT::default();
    if unsafe { WindowsAndMessaging::GetClientRect(hwnd, &mut client) }.is_err() {
        return 0.0;
    }
    let physical = (client.right - client.left).max(0) as f64;
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) };
    let scale = if dpi == 0 { 1.0 } else { dpi as f64 / 96.0 };
    physical / scale
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
    let hwnd = hwnd_from_handle(handler.native_view().window);
    set_native_framed_window(hwnd, false);
    apply_shell_window_frame(hwnd)?;
    show_native_view(handler.native_view(), title, activate)?;
    handler.set_content_visible(true)?;
    set_host_active_webtag(hwnd, webtag.key());
    set_window_handle(webtag.key(), hwnd);
    set_primary_host_window(hwnd);
    mark_active(webtag);
    notify_webtag_visibility(webtag.key(), true);
    Ok(())
}

pub fn show_webview_window_with_content_size(
    webtag: &WebTag,
    title: &str,
    activate: bool,
    width: Option<i32>,
    height: Option<i32>,
) -> StdResult<()> {
    let handler = find_webview_handler(webtag).ok_or_else(|| handler_not_ready(webtag))?;
    let hwnd = hwnd_from_handle(handler.native_view().window);
    set_native_framed_window(hwnd, true);
    apply_native_window_frame(hwnd)?;
    show_native_view(handler.native_view(), title, activate)?;
    handler.set_content_visible(true)?;
    set_host_active_webtag(hwnd, webtag.key());
    set_window_handle(webtag.key(), hwnd);
    set_primary_host_window(hwnd);
    mark_active(webtag);
    notify_webtag_visibility(webtag.key(), true);
    if width.is_some() || height.is_some() {
        let snapshot = webview_window_snapshot(webtag)?;
        let target_width = width.unwrap_or(snapshot.content_width as i32);
        let target_height = height.unwrap_or(snapshot.content_height as i32);
        resize_host_window_content(webtag, target_width, target_height)?;
        sync_window_layout(hwnd);
    }
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
    set_native_framed_window(target, false);
    apply_shell_window_frame(target)?;
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
    set_primary_host_window(target);
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
                #[cfg(feature = "browser-shell")]
                if should_hide_window_on_close(hwnd) {
                    unsafe {
                        let _ = WindowsAndMessaging::ShowWindow(hwnd, WindowsAndMessaging::SW_HIDE);
                    }
                    return LRESULT(0);
                }
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
                #[cfg(feature = "shell-chrome")]
                sync_transparent_tabbar_overlay(
                    hwnd,
                    active_webtag_key_for_window(hwnd).as_deref(),
                );
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
                // Keep the adaptive surface graph's size class tracking the
                // real window width (see `update_surface_width`).
                #[cfg(feature = "shell-chrome")]
                crate::shell::update_surface_width(window_logical_client_width(hwnd));
                sync_window_layout(hwnd);
                if windows_chrome_renderer().is_some() && !is_native_framed_window(hwnd) {
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
                // First time a main window becomes visible after a post-update
                // relaunch, (re-)center it and force it to the foreground, then
                // end the promote. The app is started by a detached helper (no
                // granted focus); it is shown via SetWindowPos/SWP_SHOWWINDOW
                // which never sends WM_SHOWWINDOW, so this is the reliable hook.
                if relaunch_promote_active()
                    && is_window_visible(hwnd)
                    && !is_minimized(hwnd)
                    && is_top_level_window(hwnd)
                {
                    center_and_foreground_window(hwnd);
                    deactivate_relaunch_promote();
                }
                #[cfg(feature = "shell-chrome")]
                sync_transparent_tabbar_overlay(
                    hwnd,
                    active_webtag_key_for_window(hwnd).as_deref(),
                );
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_ERASEBKGND => {
                if windows_chrome_renderer().is_some() && !is_native_framed_window(hwnd) {
                    return LRESULT(1);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            // A Win11 light/dark toggle broadcasts WM_SETTINGCHANGE (lParam
            // "ImmersiveColorSet"); an accent change sends
            // WM_DWMCOLORIZATIONCOLORCHANGED. Re-read the system theme and, only
            // when it actually changed, repaint the whole shell in the new palette.
            WindowsAndMessaging::WM_SETTINGCHANGE | WM_DWMCOLORIZATIONCOLORCHANGED => {
                #[cfg(feature = "shell-chrome")]
                if crate::shell::refresh_system_theme()
                    && windows_chrome_renderer().is_some()
                    && !is_native_framed_window(hwnd)
                {
                    invalidate_window(hwnd);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_PAINT => {
                if windows_chrome_renderer().is_some() && !is_native_framed_window(hwnd) {
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
                if windows_chrome_renderer().is_some() && !is_native_framed_window(hwnd) {
                    return hit_test_window(hwnd, lparam);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_MOUSEMOVE => {
                if windows_chrome_renderer().is_some()
                    && !is_native_framed_window(hwnd)
                    && handle_chrome_mouse_move(hwnd, lparam_client_point(lparam), true)
                {
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_MOUSELEAVE => {
                clear_chrome_hover(hwnd);
                if chrome_interaction(hwnd).frame_button_hover.is_some() {
                    update_chrome_interaction(hwnd, |state| state.frame_button_hover = None);
                    invalidate_frame_buttons(hwnd);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_LBUTTONDOWN => {
                if windows_chrome_renderer().is_some()
                    && !is_native_framed_window(hwnd)
                    && handle_chrome_left_down(hwnd, lparam_client_point(lparam))
                {
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_LBUTTONUP => {
                if windows_chrome_renderer().is_some()
                    && !is_native_framed_window(hwnd)
                    && handle_chrome_left_up(hwnd, lparam_client_point(lparam))
                {
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_LBUTTONDBLCLK => {
                if windows_chrome_renderer().is_some()
                    && !is_native_framed_window(hwnd)
                    && handle_chrome_left_double_click(hwnd, lparam_client_point(lparam))
                {
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_RBUTTONUP => {
                if windows_chrome_renderer().is_some()
                    && !is_native_framed_window(hwnd)
                    && handle_chrome_right_up(hwnd, lparam_client_point(lparam))
                {
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_DESTROY => {
                #[cfg(feature = "shell-chrome")]
                destroy_transparent_tabbar_overlay(hwnd);
                #[cfg(feature = "shell-chrome")]
                destroy_sidebar_tabbar_popup(hwnd);
                LRESULT(0)
            }
            WindowsAndMessaging::WM_NCDESTROY => {
                #[cfg(feature = "shell-chrome")]
                destroy_transparent_tabbar_overlay(hwnd);
                #[cfg(feature = "shell-chrome")]
                destroy_sidebar_tabbar_popup(hwnd);
                release_chrome_back_buffer(hwnd);
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
                clear_native_framed_window(hwnd);
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

    // No CS_HREDRAW/CS_VREDRAW: they invalidate the whole window on every
    // resize step. WM_SIZE already invalidates chrome windows explicitly, and
    // the buffered WM_PAINT keeps that repaint tear-free.
    let class = WNDCLASSW {
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
        // Create centered on the primary work area - the WS_POPUP default is
        // (0, 0)/top-left, which looks broken. This applies to every launch (so
        // a normal start and a post-update relaunch are consistent); the
        // relaunch additionally force-foregrounds, since a normal launch already
        // gets focus from the shell.
        let (origin_x, origin_y) = primary_centered_origin(width, height).unwrap_or((
            WindowsAndMessaging::CW_USEDEFAULT,
            WindowsAndMessaging::CW_USEDEFAULT,
        ));
        let ex_style = if HIDE_FROM_TASKBAR.load(Ordering::Relaxed) {
            WindowsAndMessaging::WS_EX_TOOLWINDOW
        } else {
            WINDOW_EX_STYLE::default()
        };
        let result = WindowsAndMessaging::CreateWindowExW(
            ex_style,
            w!("LingXiaWebViewParent"),
            w!("LingXia WebView"),
            style,
            origin_x,
            origin_y,
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

#[cfg(feature = "browser-shell")]
fn should_hide_window_on_close(_hwnd: HWND) -> bool {
    crate::tray_icon::is_installed()
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

static RELAUNCH_PROMOTE: OnceLock<AtomicBool> = OnceLock::new();

/// True for the duration of a post-update relaunch's startup. The update helper
/// drops a `.lx-update-relaunch` marker next to the exe; we read and delete it
/// once (so normal launches are unaffected) and keep the flag set until the
/// first main window has been centered + foregrounded. While set, every
/// preloaded host window is created already centered, so whichever one becomes
/// visible lands centered regardless of the show order - no window race.
fn relaunch_promote_active() -> bool {
    RELAUNCH_PROMOTE
        .get_or_init(|| AtomicBool::new(consume_update_relaunch_marker()))
        .load(Ordering::SeqCst)
}

fn deactivate_relaunch_promote() {
    if let Some(flag) = RELAUNCH_PROMOTE.get() {
        flag.store(false, Ordering::SeqCst);
    }
}

fn consume_update_relaunch_marker() -> bool {
    let Some(marker) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(".lx-update-relaunch")))
    else {
        return false;
    };
    if marker.exists() {
        let _ = std::fs::remove_file(&marker);
        true
    } else {
        false
    }
}

/// Top-left origin that centers a `width`x`height` window on the primary
/// monitor's work area. Used at window-creation time so the window is born
/// centered instead of at the WS_POPUP default of (0, 0).
fn primary_centered_origin(width: i32, height: i32) -> Option<(i32, i32)> {
    let mut work = RECT::default();
    let ok = unsafe {
        WindowsAndMessaging::SystemParametersInfoW(
            WindowsAndMessaging::SPI_GETWORKAREA,
            0,
            Some(&mut work as *mut _ as *mut c_void),
            WindowsAndMessaging::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_ok()
    };
    if !ok {
        return None;
    }
    let x = work.left + (((work.right - work.left) - width) / 2).max(0);
    let y = work.top + (((work.bottom - work.top) - height) / 2).max(0);
    Some((x, y))
}

/// Center `hwnd` on its monitor's work area and pull it to the foreground. Uses
/// the `AttachThreadInput` trick - attach to the current foreground thread's
/// input queue so `SetForegroundWindow` is honored even though this process was
/// launched in the background by the update helper. Runs on the window's own UI
/// thread (from `WM_SHOWWINDOW`), so it doesn't race the app's own layout.
fn center_and_foreground_window(hwnd: HWND) {
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let mut rc = RECT::default();
        if GetMonitorInfoW(monitor, &mut mi).as_bool()
            && WindowsAndMessaging::GetWindowRect(hwnd, &mut rc).is_ok()
        {
            let work = mi.rcWork;
            let w = rc.right - rc.left;
            let h = rc.bottom - rc.top;
            let x = work.left + (((work.right - work.left) - w) / 2).max(0);
            let y = work.top + (((work.bottom - work.top) - h) / 2).max(0);
            let _ = WindowsAndMessaging::SetWindowPos(
                hwnd,
                None,
                x,
                y,
                0,
                0,
                WindowsAndMessaging::SWP_NOSIZE | WindowsAndMessaging::SWP_NOZORDER,
            );
        }

        let foreground = WindowsAndMessaging::GetForegroundWindow();
        let fg_thread = WindowsAndMessaging::GetWindowThreadProcessId(foreground, None);
        let this_thread = GetCurrentThreadId();
        let attached = fg_thread != 0
            && fg_thread != this_thread
            && AttachThreadInput(this_thread, fg_thread, true).as_bool();
        let _ = WindowsAndMessaging::BringWindowToTop(hwnd);
        let _ = WindowsAndMessaging::SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));
        if attached {
            let _ = AttachThreadInput(this_thread, fg_thread, false);
        }
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

pub(crate) fn webtag_is_visible(webtag_key: &str) -> bool {
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

#[cfg(feature = "shell-chrome")]
fn is_window_handle_valid(handle: isize) -> bool {
    unsafe { WindowsAndMessaging::IsWindow(Some(hwnd_from_handle(handle))).as_bool() }
}

fn hwnd_handle(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
