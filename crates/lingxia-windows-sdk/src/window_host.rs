//! Windows host-window implementation owned by the Windows SDK layer.

use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
#[cfg(feature = "components")]
use std::time::Instant;

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
    WindowsHostPanelKeyEvent, WindowsHostPanelTab, WindowsHostWindow, WindowsNavAnimation,
    WindowsPanelPosition, WindowsWebViewContentWindow, WindowsWebViewWindowSnapshot,
    WindowsWindowLayout, cleanup_webview_state, current_window_layout, default_window_size,
    host_panel_input_handler, host_window_created_handlers, set_webview_window_layout,
    set_windows_host_backend, webview_chrome_event_handler, webview_close_handler,
    webview_visibility_handler, windows_chrome_renderer,
};
use windows::Win32::Foundation::SIZE;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
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
#[cfg(all(feature = "shell-chrome", feature = "terminal-runtime"))]
use windows::Win32::UI::Input::Ime::{
    CANDIDATEFORM, CFS_EXCLUDE, CFS_POINT, COMPOSITIONFORM, ImmGetContext, ImmReleaseContext,
    ImmSetCandidateWindow, ImmSetCompositionWindow,
};
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
#[cfg(feature = "shell-chrome")]
static PANEL_POSITION_OVERRIDES: OnceLock<Mutex<HashMap<String, WindowsPanelPosition>>> =
    OnceLock::new();
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
static WINDOW_RESIZE_DRAGS: OnceLock<Mutex<HashMap<isize, WindowResizeDrag>>> = OnceLock::new();
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
#[cfg(feature = "shell-chrome")]
static SHELL_NOTICE_POPUPS: OnceLock<Mutex<HashMap<isize, ShellNoticePopup>>> = OnceLock::new();
#[cfg(feature = "shell-chrome")]
static TERMINAL_SELECTION_DRAGS: OnceLock<Mutex<HashMap<isize, String>>> = OnceLock::new();
#[cfg(feature = "components")]
static NAV_SNAPSHOT_SLIDES: OnceLock<Mutex<HashMap<isize, NavSnapshotSlide>>> = OnceLock::new();
const WM_LINGXIA_RUN_CALLBACK: u32 = WindowsAndMessaging::WM_APP + 0x158;
const PULL_REFRESH_TIMER_ID: usize = 0x5A17;
/// Navigation slide duration, matching the iOS/Android 300ms page transition.
#[cfg(feature = "components")]
const NAV_SLIDE_DURATION_MS: f64 = 300.0;
const PULL_REFRESH_TIMER_MS: u32 = 120;
const PULL_REFRESH_SLOT_HEIGHT: i32 = 42;
const PULL_REFRESH_INDICATOR_WIDTH: i32 = 64;
const PULL_REFRESH_INDICATOR_HEIGHT: i32 = 32;
const OVERLAY_MARGIN: i32 = 24;
#[cfg(feature = "shell-chrome")]
const SIDEBAR_TABBAR_POPUP_TIMER_ID: usize = 0x5A18;
#[cfg(feature = "shell-chrome")]
const SIDEBAR_TABBAR_POPUP_TIMER_MS: u32 = 80;
#[cfg(feature = "shell-chrome")]
const SHELL_NOTICE_TIMER_ID: usize = 0x5A19;
#[cfg(feature = "shell-chrome")]
const SHELL_NOTICE_TIMER_MS: u32 = 3_000;
#[cfg(feature = "runtime")]
const SYSTEM_MENU_ABOUT_COMMAND: usize = 0x7100;
#[cfg(feature = "runtime")]
const SYSTEM_MENU_COMMAND_MASK: usize = 0xfff0;

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

#[derive(Debug, Clone, Copy)]
enum WindowResizeEdge {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Copy)]
struct WindowResizeDrag {
    edge: WindowResizeEdge,
    cursor: POINT,
    window: RECT,
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
    /// Composition clip corners `[tl, tr, br, bl]`; part of the dedupe key so
    /// a radii-only layout change (aside open/close, dock/float, frame
    /// enter/exit) still reaches the webview.
    corner_radii: [i32; 4],
    /// Wedge backdrop color; part of the dedupe key so theme flips repaint
    /// the corner wedges.
    corner_color: u32,
    /// Device-frame fit factor ×1000; part of the dedupe key so a device
    /// switch re-applies the webview's rasterization scale.
    fit_scale_milli: u32,
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

    fn active_host_window_is_device_framed(&self) -> bool {
        active_host_window().is_some_and(window_is_device_framed)
    }

    fn active_host_window_webtag_key(&self) -> Option<String> {
        active_host_window().and_then(active_webtag_key_for_window)
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
        animation: WindowsNavAnimation,
    ) -> StdResult<()> {
        navigate_webview_window(webtag, title, activate, animation)
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

    // A phone-width host has no room for a docked panel: a page aside drills
    // in full-screen instead, mirroring iOS and the macOS runner's phone
    // presentation. URL asides never reach here compact — the shared logic
    // degrades them to browser tabs.
    {
        let mut client = RECT::default();
        unsafe {
            let _ = WindowsAndMessaging::GetClientRect(host, &mut client);
        }
        if client.right - client.left > 0 && client.right - client.left < PHONE_DRILL_MAX_WIDTH {
            return present_webview_fullscreen_drill(webtag, &handler, excluded, host);
        }
    }

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
    let Some(host) = prefer_visible_workspace(active_host_window()) else {
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
        // The visibility registry tracks the *window* (WM_SHOWWINDOW /
        // WM_WINDOWPOSCHANGED mark the active webtag visible), not the
        // WebView2 controller. At launch the host window is shown before the
        // controller ever became visible, so this branch must still show the
        // controller or the screen stays black until another present.
        handler.set_content_visible(true)?;
        sync_window_layout(host);
        invalidate_window_chrome(host);
        // A re-present of the active page is still a present: surface the
        // host if something hid it and converge stray duplicates.
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
        hide_other_workspace_windows(host);
        return Ok(());
    }

    // A newly-created browser/surface WebView has no shell layout yet. Keep
    // the outgoing main surface's geometry until its owner installs the final
    // layout; otherwise this intermediate pass treats the target as a raw
    // full-client WebView, drops every attached panel from reconciliation,
    // and makes the asides disappear/reappear during a sidebar switch.
    let inherited_layout = previous
        .as_deref()
        .filter(|previous| !webtag_is_registered_panel(previous))
        .map(current_window_layout)
        .filter(|layout| !layout.is_empty());
    let target_has_layout = !current_window_layout(webtag.key()).is_empty();

    // Remember the lxapp page a browser tab covers so closing the last tab
    // restores it. Track the LATEST lxapp page (the user may have navigated
    // since the group first presented) but never a browser tab itself —
    // switching between tabs must not overwrite the restore target.
    if let Some(previous) = previous.as_ref()
        && previous != webtag.key()
        && !webtag_is_registered_panel(previous)
        && group_main_restore_candidate(previous)
    {
        let presented = PRESENTED_GROUP_MAIN.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(mut presented) = presented.lock() {
            presented.insert(hwnd_handle(host), previous.clone());
        }
    }

    if webtag_is_visible(webtag.key()) {
        handler.set_content_visible(false)?;
    }
    set_window_handle(webtag.key(), host);
    set_host_active_webtag(host, webtag.key());
    set_primary_host_window(host);
    let inherited = if target_has_layout {
        false
    } else if let Some(layout) = inherited_layout {
        set_webview_window_layout(webtag, layout).is_ok()
    } else {
        false
    };
    if !inherited {
        sync_window_layout(host);
    }
    // Paint the incoming webtag's chrome while the outgoing controller still
    // covers the content. Without this synchronous pass, switching from a
    // browser row to an already-selected lxapp tab can show the new page for
    // one frame under the old browser address bar.
    repaint_window_now(host);
    // Show the host before the content so the controller's SetIsVisible(true)
    // lands on a visible parent chain and composition starts immediately.
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
    handler.set_content_visible(true)?;

    if let Some(previous) = previous
        && previous != webtag.key()
        && !webtag_is_registered_panel(&previous)
    {
        if let Some(previous_webtag) = webtag_for_key(&previous)
            && let Some(previous_handler) = find_webview_handler(&previous_webtag)
        {
            let _ = previous_handler.set_content_visible(false);
        }
        notify_webtag_visibility(&previous, false);
    }

    mark_active(webtag);
    notify_webtag_visibility(webtag.key(), true);
    repaint_window_now(host);
    hide_other_workspace_windows(host);
    Ok(())
}

pub fn present_webview_as_group_main(webtag: &WebTag, _group_key: String) -> StdResult<()> {
    present_webview_in_active_group(webtag)
}

/// Covers a first-time group-main swap with the outgoing WebView snapshot
/// until WebView2 has had a short window to submit the incoming controller's
/// first composed frame. DOM readiness alone is insufficient: an invisible
/// controller can be fully interactive while its first visible frame is still
/// blank white.
pub(crate) fn present_webview_in_active_group_with_snapshot_guard(
    webtag: &WebTag,
    hold_ms: u64,
) -> StdResult<()> {
    #[cfg(feature = "components")]
    {
        let host = active_host_window();
        let guard = host.and_then(|host| {
            active_webtag_key_for_window(host).and_then(|previous| {
                (!webtag_is_registered_panel(&previous)
                    && webtag_is_visible(&previous)
                    && prepare_nav_snapshot_slide(host, &previous, true))
                .then(|| nav_snapshot_overlay(host).map(|overlay| (host, overlay)))
                .flatten()
            })
        });
        let result = present_webview_in_active_group(webtag);
        if let Some((host, overlay)) = guard {
            if result.is_ok() {
                schedule_nav_snapshot_guard_release(host, overlay, hold_ms);
            } else {
                finish_nav_snapshot_slide(host);
            }
        }
        result
    }
    #[cfg(not(feature = "components"))]
    {
        let _ = hold_ms;
        present_webview_in_active_group(webtag)
    }
}

#[cfg(feature = "components")]
fn schedule_nav_snapshot_guard_release(host: HWND, overlay: isize, hold_ms: u64) {
    let host_handle = hwnd_handle(host);
    let _ = std::thread::Builder::new()
        .name("lingxia-first-frame-guard".to_string())
        .spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(hold_ms));
            if !post_to_window_thread(
                host_handle,
                Box::new(move || {
                    finish_nav_snapshot_slide_if_overlay(hwnd_from_handle(host_handle), overlay);
                }),
            ) {
                finish_nav_snapshot_slide_if_overlay(hwnd_from_handle(host_handle), overlay);
            }
        });
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
        register_floating_overlay(owner, webtag.key());
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
    apply_floating_card_dressing(hwnd);
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

/// A borderless overlay popup reads as a flat patch without window dressing:
/// round its corners and re-enable the DWM drop shadow (extending the frame a
/// single invisible pixel restores the shadow WS_POPUP loses) so the surface
/// visibly floats over the host content.
fn apply_floating_card_dressing(hwnd: HWND) {
    use windows::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmExtendFrameIntoClientArea,
        DwmSetWindowAttribute,
    };
    use windows::Win32::UI::Controls::MARGINS;
    unsafe {
        let preference = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&preference) as u32,
        );
        let margins = MARGINS {
            cxLeftWidth: 0,
            cxRightWidth: 0,
            cyTopHeight: 0,
            cyBottomHeight: 1,
        };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
    }
}

/// Floating overlay surfaces per host window (`owner -> surface webtags`), so
/// host chrome popups raised on every layout pass (the transparent tab bar)
/// can be kept underneath them.
static FLOATING_OVERLAYS: OnceLock<Mutex<HashMap<isize, Vec<String>>>> = OnceLock::new();

fn register_floating_overlay(owner: HWND, webtag_key: &str) {
    if let Ok(mut floats) = FLOATING_OVERLAYS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        let keys = floats.entry(hwnd_handle(owner)).or_default();
        if !keys.iter().any(|key| key == webtag_key) {
            keys.push(webtag_key.to_string());
        }
    }
}

fn unregister_floating_overlay(webtag_key: &str) {
    if let Some(mut floats) = FLOATING_OVERLAYS.get().and_then(|f| f.lock().ok()) {
        for keys in floats.values_mut() {
            keys.retain(|key| key != webtag_key);
        }
        floats.retain(|_, keys| !keys.is_empty());
    }
}

fn floating_overlay_owner(webtag_key: &str) -> Option<HWND> {
    FLOATING_OVERLAYS
        .get()
        .and_then(|floats| floats.lock().ok())
        .and_then(|floats| {
            floats
                .iter()
                .find(|(_, keys)| keys.iter().any(|key| key == webtag_key))
                .map(|(owner, _)| hwnd_from_handle(*owner))
        })
        .filter(|owner| is_valid_host_window(*owner))
}

/// Re-raises the owner's visible floating surfaces above its chrome popups —
/// the tab-bar overlay repositions with HWND_TOP on every layout pass, which
/// would otherwise stack it over an open float.
#[cfg(feature = "shell-chrome")]
fn raise_floating_overlays(owner: HWND) {
    let keys: Vec<String> = FLOATING_OVERLAYS
        .get()
        .and_then(|floats| floats.lock().ok())
        .and_then(|floats| floats.get(&hwnd_handle(owner)).cloned())
        .unwrap_or_default();
    for key in keys {
        if !webtag_is_visible(&key) {
            continue;
        }
        if let Some(float) = window_handle_for_key(&key)
            && is_window_handle_valid(hwnd_handle(float))
        {
            unsafe {
                let _ = WindowsAndMessaging::SetWindowPos(
                    float,
                    Some(WindowsAndMessaging::HWND_TOP),
                    0,
                    0,
                    0,
                    0,
                    WindowsAndMessaging::SWP_NOMOVE
                        | WindowsAndMessaging::SWP_NOSIZE
                        | WindowsAndMessaging::SWP_NOACTIVATE,
                );
            }
        }
    }
}

/// Compact-width breakpoint below which page surfaces drill in full-screen.
const PHONE_DRILL_MAX_WIDTH: i32 = 600;

/// Full-screen drill surfaces per host window (`owner -> surface webtag`),
/// so the transparent tab bar hides underneath them.
static FULLSCREEN_DRILLS: OnceLock<Mutex<HashMap<isize, String>>> = OnceLock::new();

#[cfg(feature = "shell-chrome")]
pub(crate) fn fullscreen_drill_visible(owner: HWND) -> bool {
    FULLSCREEN_DRILLS
        .get()
        .and_then(|drills| drills.lock().ok())
        .and_then(|drills| drills.get(&hwnd_handle(owner)).cloned())
        .is_some_and(|webtag_key| webtag_is_visible(&webtag_key))
}

/// Presents a page surface full-screen over the phone-sized host, mirroring
/// the macOS runner's drill-in: the surface covers the whole device screen
/// (tab bar included) with a floating back affordance to return.
fn present_webview_fullscreen_drill(
    webtag: &WebTag,
    handler: &WindowsWebViewHandler,
    hwnd: HWND,
    owner: HWND,
) -> StdResult<()> {
    unsafe {
        let _ = WindowsAndMessaging::SetWindowLongPtrW(
            hwnd,
            WindowsAndMessaging::GWLP_HWNDPARENT,
            owner.0 as isize,
        );
        let style = WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWL_STYLE);
        let borderless = (style
            & !(WS_SIZEBOX.0 as isize
                | WindowsAndMessaging::WS_CAPTION.0 as isize
                | WS_MAXIMIZEBOX.0 as isize
                | WS_MINIMIZEBOX.0 as isize))
            | WS_POPUP.0 as isize;
        if borderless != style {
            WindowsAndMessaging::SetWindowLongPtrW(
                hwnd,
                WindowsAndMessaging::GWL_STYLE,
                borderless,
            );
        }
    }
    let mut client = RECT::default();
    let mut origin = POINT { x: 0, y: 0 };
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(owner, &mut client);
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(owner, &mut origin);
        WindowsAndMessaging::SetWindowPos(
            hwnd,
            Some(WindowsAndMessaging::HWND_TOP),
            origin.x,
            origin.y,
            client.right - client.left,
            client.bottom - client.top,
            WindowsAndMessaging::SWP_SHOWWINDOW | WindowsAndMessaging::SWP_FRAMECHANGED,
        )
        .map_err(|err| WebViewError::WebView(format!("SetWindowPos failed: {err}")))?;
        let _ = WindowsAndMessaging::BringWindowToTop(hwnd);
    }
    handler.set_content_visible(true)?;
    set_window_handle(webtag.key(), hwnd);
    set_host_active_webtag(hwnd, webtag.key());
    sync_window_layout(hwnd);
    mark_active(webtag);
    notify_webtag_visibility(webtag.key(), true);
    if let Ok(mut drills) = FULLSCREEN_DRILLS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        drills.insert(hwnd_handle(owner), webtag.key().to_string());
    }
    // Hiding the tab bar under the drill; the next owner layout pass with
    // the drill gone restores it.
    sync_window_layout(owner);
    let drill = hwnd_handle(hwnd);
    let webtag_key = webtag.key().to_string();
    post_to_window_thread(
        drill,
        Box::new(move || show_drill_back_button(hwnd_from_handle(drill), webtag_key)),
    );
    Ok(())
}

const DRILL_BACK_SIZE: i32 = 28;
const DRILL_BACK_MARGIN: i32 = 12;

/// Floating drill-in back affordance pinned to the surface's top-left, the
/// phone gesture to dismiss a full-screen surface (macOS runner parity).
fn show_drill_back_button(drill: HWND, webtag_key: String) {
    let class = drill_back_class();
    let user_data = Box::into_raw(Box::new(webtag_key));
    let Ok(button) = (unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_NOACTIVATE,
            class,
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            DRILL_BACK_SIZE,
            DRILL_BACK_SIZE,
            Some(drill),
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            Some(user_data.cast()),
        )
    }) else {
        let _ = unsafe { Box::from_raw(user_data) };
        return;
    };
    let mut rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetWindowRect(drill, &mut rect);
        let _ = WindowsAndMessaging::SetWindowPos(
            button,
            Some(WindowsAndMessaging::HWND_TOP),
            rect.left + DRILL_BACK_MARGIN,
            rect.top + DRILL_BACK_MARGIN,
            DRILL_BACK_SIZE,
            DRILL_BACK_SIZE,
            WindowsAndMessaging::SWP_NOACTIVATE | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
    }
    upload_drill_back_button(button);
}

fn drill_back_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let cursor =
            unsafe { WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_ARROW) }
                .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(drill_back_proc),
            hInstance: module,
            hCursor: cursor,
            cbWndExtra: 0,
            lpszClassName: w!("LingXiaDrillBack"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "drill back class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaDrillBack")
}

unsafe extern "system" fn drill_back_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_NCCREATE => {
            let create = lparam.0 as *const WindowsAndMessaging::CREATESTRUCTW;
            if !create.is_null() {
                unsafe {
                    WindowsAndMessaging::SetWindowLongPtrW(
                        hwnd,
                        WindowsAndMessaging::GWLP_USERDATA,
                        (*create).lpCreateParams as isize,
                    );
                }
            }
            LRESULT(1)
        }
        WindowsAndMessaging::WM_MOUSEACTIVATE => {
            LRESULT(WindowsAndMessaging::MA_NOACTIVATE as isize)
        }
        WindowsAndMessaging::WM_LBUTTONUP => {
            let raw = unsafe {
                WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
            } as *const String;
            if !raw.is_null() {
                let webtag_key = unsafe { (*raw).clone() };
                if let Some(close) = webview_close_handler(&webtag_key) {
                    close();
                }
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_NCDESTROY => {
            let raw = unsafe {
                WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
            } as *mut String;
            if !raw.is_null() {
                let _ = unsafe { Box::from_raw(raw) };
            }
            unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Uploads the back button: a 35%-black circle with a white chevron.
fn upload_drill_back_button(hwnd: HWND) {
    let size = DRILL_BACK_SIZE;
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
                biWidth: size,
                biHeight: -size,
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
        // Black disc backdrop; the chevron in white 2px strokes.
        let brush = CreateSolidBrush(COLORREF(0));
        let pen = CreatePen(PS_SOLID, 0, COLORREF(0));
        let old_brush = SelectObject(dc, HGDIOBJ(brush.0));
        let old_pen = SelectObject(dc, HGDIOBJ(pen.0));
        let _ = windows::Win32::Graphics::Gdi::Rectangle(dc, 0, 0, size, size);
        let white_pen = CreatePen(PS_SOLID, 2, COLORREF(0x00ffffff));
        let _ = SelectObject(dc, HGDIOBJ(white_pen.0));
        let _ = windows::Win32::Graphics::Gdi::MoveToEx(dc, 17, 8, None);
        let _ = windows::Win32::Graphics::Gdi::LineTo(dc, 11, 14);
        let _ = windows::Win32::Graphics::Gdi::LineTo(dc, 17, 20);
        let _ = SelectObject(dc, old_pen);
        let _ = SelectObject(dc, old_brush);
        let _ = DeleteObject(HGDIOBJ(white_pen.0));
        let _ = DeleteObject(HGDIOBJ(pen.0));
        let _ = DeleteObject(HGDIOBJ(brush.0));

        let pixel_count = (size * size) as usize;
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u32>(), pixel_count);
        apply_drill_back_alpha(pixels, size);
        let dib_size = SIZE { cx: size, cy: size };
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
            Some(&dib_size),
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

/// Circular alpha: the disc is a 35% black wash, the white chevron strokes
/// stay opaque; outside the circle is fully transparent.
fn apply_drill_back_alpha(pixels: &mut [u32], size: i32) {
    const DIM: u32 = 0x59;
    let radius = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let index = (y * size + x) as usize;
            let pixel = pixels[index] & 0x00ff_ffff;
            let dx = x as f32 + 0.5 - radius;
            let dy = y as f32 + 0.5 - radius;
            let coverage = (radius - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0);
            let base = if pixel > 0x00f0_f0f0 { 255 } else { DIM };
            let alpha = (base as f32 * coverage) as u32;
            let premultiply = |channel: u32| (channel * alpha + 127) / 255;
            pixels[index] = (alpha << 24)
                | (premultiply((pixel >> 16) & 0xff) << 16)
                | (premultiply((pixel >> 8) & 0xff) << 8)
                | premultiply(pixel & 0xff);
        }
    }
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
        // The floating-card default never fills the host: keep a margin on a
        // phone-narrow host so the card still reads as floating.
        fallback.min(reference - 2 * OVERLAY_MARGIN)
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

/// Whether `webtag_key` can be a group-main restore target: any lxapp page
/// qualifies; a browser tab does not (closing the last tab must fall back to
/// the covered lxapp page, not another tab).
fn group_main_restore_candidate(webtag_key: &str) -> bool {
    #[cfg(feature = "browser-runtime")]
    {
        !webtag_key.starts_with(lingxia_browser::BUILTIN_BROWSER_APPID)
    }
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = webtag_key;
        true
    }
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
        && !webtag_is_registered_panel(&current)
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
    repaint_window_now(host);
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

#[cfg(feature = "shell-chrome")]
pub(crate) fn set_panel_position_override(panel_id: &str, position: Option<WindowsPanelPosition>) {
    let overrides = PANEL_POSITION_OVERRIDES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut overrides) = overrides.lock() {
        match position {
            Some(position) => {
                overrides.insert(panel_id.to_string(), position);
            }
            None => {
                overrides.remove(panel_id);
            }
        }
    }

    update_registered_panel_position(panel_id, panel_position_for_id(panel_id));
    sync_active_host_layout();
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

/// Per-corner composition rounding for a surface — clip radii plus the
/// backdrop color its corner wedges paint — derived from the same layout
/// pass that produced `rect`. Square whenever the surface is not part of a
/// rounded silhouette (fullscreen drill, no shell chrome, collapsed rect).
#[cfg(feature = "shell-chrome")]
fn surface_clip_style(hwnd: HWND, webtag_key: &str, rect: RECT) -> ([i32; 4], u32) {
    // A framed simulator screen rounds to the device silhouette, not the
    // shell workspace: bezel-colored wedges replace the frame's old
    // SetWindowRgn cut + corner-mask overlay on the composition path.
    #[cfg(feature = "device-frame")]
    if let Some((radius, bezel)) =
        crate::device_frame::device_frame_screen_clip_style(hwnd_handle(hwnd))
    {
        let mut client = RECT::default();
        if unsafe { WindowsAndMessaging::GetClientRect(hwnd, &mut client) }.is_err() {
            return ([0; 4], 0);
        }
        return (
            crate::shell::workspace_corner_radii(rect, client, radius),
            0xff00_0000 | bezel,
        );
    }
    if windows_chrome_renderer().is_none() || webtag_is_fullscreen_drill(webtag_key) {
        return ([0; 4], 0);
    }
    let backdrop = 0xff00_0000 | crate::shell::shell_window_background();
    // Floating panels are free-standing cards outside the workspace union.
    if let Some(panel_id) = panel_id_for_webtag(webtag_key)
        && !panel_is_docked(&panel_id)
    {
        return ([crate::shell::shell_panel_radius(); 4], backdrop);
    }
    let mut client = RECT::default();
    if unsafe { WindowsAndMessaging::GetClientRect(hwnd, &mut client) }.is_err() {
        return ([0; 4], 0);
    }
    let Some(active_key) = active_webtag_key_for_window(hwnd) else {
        return ([0; 4], 0);
    };
    let Some(silhouette) =
        crate::shell::workspace_silhouette_rect(client, &current_window_layout(&active_key))
    else {
        return ([0; 4], 0);
    };
    (
        crate::shell::workspace_corner_radii(
            rect,
            silhouette,
            crate::shell::shell_content_radius(),
        ),
        backdrop,
    )
}

#[cfg(not(feature = "shell-chrome"))]
fn surface_clip_style(_hwnd: HWND, _webtag_key: &str, _rect: RECT) -> ([i32; 4], u32) {
    ([0; 4], 0)
}

#[cfg(feature = "shell-chrome")]
fn webtag_is_fullscreen_drill(webtag_key: &str) -> bool {
    FULLSCREEN_DRILLS
        .get()
        .and_then(|drills| drills.lock().ok())
        .is_some_and(|drills| drills.values().any(|key| key == webtag_key))
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
        // A full-screen drill surface covers the whole device screen, tab
        // bar included.
        if fullscreen_drill_visible(hwnd) {
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
    raise_floating_overlays(hwnd);
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

/// One non-activating, short-lived notice per shell window. It is an owned
/// layered popup so it remains visible above windowed WebView2 children while
/// keeping keyboard focus in the page or context menu that triggered it.
#[cfg(feature = "shell-chrome")]
#[derive(Debug, Clone, Copy)]
struct ShellNoticePopup {
    window: isize,
    owner: isize,
}

#[cfg(feature = "shell-chrome")]
pub fn show_shell_notice(owner: isize, title: String, message: String) {
    if title.trim().is_empty() && message.trim().is_empty() {
        return;
    }
    post_to_window_thread(
        owner,
        Box::new(move || show_shell_notice_on_thread(owner, title, message)),
    );
}

#[cfg(feature = "shell-chrome")]
fn show_shell_notice_on_thread(owner: isize, title: String, message: String) {
    const IDEAL_WIDTH: i32 = 380;
    const HEIGHT: i32 = 88;
    const WINDOW_MARGIN: i32 = 24;

    let owner_hwnd = hwnd_from_handle(owner);
    destroy_shell_notice(owner_hwnd);

    let mut client = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetClientRect(owner_hwnd, &mut client).is_err() {
            return;
        }
    }
    let client_width = client.right - client.left;
    let client_height = client.bottom - client.top;
    let width = IDEAL_WIDTH.min(client_width - WINDOW_MARGIN * 2);
    if width < 220 || client_height < HEIGHT + WINDOW_MARGIN * 2 {
        return;
    }

    let mut origin = POINT::default();
    unsafe {
        if !windows::Win32::Graphics::Gdi::ClientToScreen(owner_hwnd, &mut origin).as_bool() {
            return;
        }
    }
    let left = origin.x + (client_width - width) / 2;
    let top_offset = (crate::shell::shell_top_bar_height() + 12)
        .clamp(WINDOW_MARGIN, client_height - HEIGHT - WINDOW_MARGIN);
    let top = origin.y + top_offset;

    let result = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_NOACTIVATE,
            shell_notice_class(),
            PCWSTR::null(),
            WS_POPUP,
            left,
            top,
            width,
            HEIGHT,
            Some(owner_hwnd),
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    };
    let Ok(window) = result else {
        log::warn!(
            "shell notice creation failed: {}",
            windows::core::Error::from_thread()
        );
        return;
    };

    if let Ok(mut notices) = SHELL_NOTICE_POPUPS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        notices.insert(
            owner,
            ShellNoticePopup {
                window: hwnd_handle(window),
                owner,
            },
        );
    }
    upload_shell_notice(window, &title, &message, width, HEIGHT);
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            window,
            Some(WindowsAndMessaging::HWND_TOP),
            left,
            top,
            width,
            HEIGHT,
            WindowsAndMessaging::SWP_NOACTIVATE | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
        let _ = WindowsAndMessaging::SetTimer(
            Some(window),
            SHELL_NOTICE_TIMER_ID,
            SHELL_NOTICE_TIMER_MS,
            None,
        );
    }
}

#[cfg(feature = "shell-chrome")]
fn destroy_shell_notice(owner: HWND) {
    let notice = SHELL_NOTICE_POPUPS
        .get()
        .and_then(|notices| notices.lock().ok())
        .and_then(|mut notices| notices.remove(&hwnd_handle(owner)));
    if let Some(notice) = notice
        && is_window_handle_valid(notice.window)
    {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(notice.window));
        }
    }
}

#[cfg(feature = "shell-chrome")]
fn shell_notice_owner_for_window(window: HWND) -> Option<HWND> {
    SHELL_NOTICE_POPUPS
        .get()
        .and_then(|notices| notices.lock().ok())
        .and_then(|notices| {
            notices
                .values()
                .find(|notice| notice.window == hwnd_handle(window))
                .map(|notice| hwnd_from_handle(notice.owner))
        })
}

#[cfg(feature = "shell-chrome")]
fn remove_shell_notice_window(window: HWND) {
    if let Some(notices) = SHELL_NOTICE_POPUPS.get()
        && let Ok(mut notices) = notices.lock()
    {
        notices.retain(|_, notice| notice.window != hwnd_handle(window));
    }
}

#[cfg(feature = "shell-chrome")]
fn shell_notice_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let cursor =
            unsafe { WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_ARROW) }
                .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(shell_notice_proc),
            hInstance: module,
            hCursor: cursor,
            lpszClassName: w!("LingXiaShellNotice"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "shell notice class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaShellNotice")
}

#[cfg(feature = "shell-chrome")]
unsafe extern "system" fn shell_notice_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_MOUSEACTIVATE => {
            return LRESULT(WindowsAndMessaging::MA_NOACTIVATE as isize);
        }
        WindowsAndMessaging::WM_TIMER if wparam.0 == SHELL_NOTICE_TIMER_ID => {
            if let Some(owner) = shell_notice_owner_for_window(hwnd) {
                destroy_shell_notice(owner);
            }
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_LBUTTONUP => {
            if let Some(owner) = shell_notice_owner_for_window(hwnd) {
                destroy_shell_notice(owner);
            }
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_ERASEBKGND => return LRESULT(1),
        WindowsAndMessaging::WM_NCDESTROY => remove_shell_notice_window(hwnd),
        _ => {}
    }
    unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[cfg(feature = "shell-chrome")]
fn upload_shell_notice(hwnd: HWND, title: &str, message: &str, width: i32, height: i32) {
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
        crate::shell::paint_shell_notice(dc, title, message, width, height);
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u32>(), (width * height) as usize);
        apply_rounded_alpha_mask(pixels, width, height, 12);
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let origin = POINT::default();
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

/// One phone tab-switcher sheet per host window (the macOS runner's
/// in-frame bottom sheet), shown from the phone bar's tabs button.
#[cfg(feature = "shell-chrome")]
struct PhoneTabSwitcher {
    window: isize,
    owner: isize,
    layout: crate::shell::PhoneTabSwitcherLayout,
}

#[cfg(feature = "shell-chrome")]
static PHONE_TAB_SWITCHERS: OnceLock<Mutex<HashMap<isize, PhoneTabSwitcher>>> = OnceLock::new();

#[cfg(feature = "shell-chrome")]
fn phone_tab_switcher_for_owner(owner: HWND) -> Option<isize> {
    PHONE_TAB_SWITCHERS
        .get()
        .and_then(|switchers| switchers.lock().ok())
        .and_then(|switchers| {
            switchers
                .get(&hwnd_handle(owner))
                .map(|switcher| switcher.window)
        })
}

/// Toggles the tab-switcher sheet over `owner`'s client area. `tabs` are
/// `(tab_id, title, active)` in display order. Marshaled onto the window's
/// thread: the popup window must be owned by a pumping thread.
#[cfg(feature = "shell-chrome")]
pub fn toggle_phone_tab_switcher(owner: isize, tabs: Vec<(String, String, bool)>) {
    post_to_window_thread(
        owner,
        Box::new(move || toggle_phone_tab_switcher_on_thread(owner, tabs)),
    );
}

#[cfg(feature = "shell-chrome")]
fn toggle_phone_tab_switcher_on_thread(owner: isize, tabs: Vec<(String, String, bool)>) {
    let owner_hwnd = hwnd_from_handle(owner);
    if phone_tab_switcher_for_owner(owner_hwnd).is_some() {
        destroy_phone_tab_switcher(owner_hwnd);
        return;
    }
    if tabs.is_empty() {
        return;
    }
    let mut client = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetClientRect(owner_hwnd, &mut client).is_err() {
            return;
        }
    }
    let width = client.right - client.left;
    let height = client.bottom - client.top;
    let layout = crate::shell::phone_tab_switcher_layout(width, height, &tabs);

    let class = phone_tab_switcher_class();
    let Ok(window) = (unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_NOACTIVATE,
            class,
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            width,
            height,
            Some(owner_hwnd),
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    }) else {
        log::warn!(
            "phone tab switcher creation failed: {}",
            windows::core::Error::from_thread()
        );
        return;
    };
    let mut origin = POINT { x: 0, y: 0 };
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(owner_hwnd, &mut origin);
        let _ = WindowsAndMessaging::SetWindowPos(
            window,
            Some(WindowsAndMessaging::HWND_TOP),
            origin.x,
            origin.y,
            width,
            height,
            WindowsAndMessaging::SWP_NOACTIVATE | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
    }
    upload_phone_tab_switcher(window, &layout);
    if let Ok(mut switchers) = PHONE_TAB_SWITCHERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        switchers.insert(
            hwnd_handle(owner_hwnd),
            PhoneTabSwitcher {
                window: hwnd_handle(window),
                owner: hwnd_handle(owner_hwnd),
                layout,
            },
        );
    }
}

#[cfg(feature = "shell-chrome")]
fn destroy_phone_tab_switcher(owner: HWND) {
    let switcher = PHONE_TAB_SWITCHERS
        .get()
        .and_then(|switchers| switchers.lock().ok())
        .and_then(|mut switchers| switchers.remove(&hwnd_handle(owner)));
    if let Some(switcher) = switcher
        && is_window_handle_valid(switcher.window)
    {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(switcher.window));
        }
    }
}

#[cfg(feature = "shell-chrome")]
fn phone_tab_switcher_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let cursor =
            unsafe { WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_ARROW) }
                .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(phone_tab_switcher_proc),
            hInstance: module,
            hCursor: cursor,
            lpszClassName: w!("LingXiaPhoneTabSwitcher"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "phone tab switcher class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaPhoneTabSwitcher")
}

#[cfg(feature = "shell-chrome")]
unsafe extern "system" fn phone_tab_switcher_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_MOUSEACTIVATE => {
            LRESULT(WindowsAndMessaging::MA_NOACTIVATE as isize)
        }
        WindowsAndMessaging::WM_ERASEBKGND => LRESULT(1),
        WindowsAndMessaging::WM_LBUTTONUP => {
            let point = lparam_client_point(lparam);
            let entry = PHONE_TAB_SWITCHERS
                .get()
                .and_then(|switchers| switchers.lock().ok())
                .and_then(|switchers| {
                    switchers
                        .values()
                        .find(|switcher| switcher.window == hwnd_handle(hwnd))
                        .map(|switcher| {
                            (
                                switcher.owner,
                                crate::shell::phone_tab_switcher_hit(&switcher.layout, point),
                            )
                        })
                });
            if let Some((owner, hit)) = entry {
                let owner_hwnd = hwnd_from_handle(owner);
                match hit {
                    crate::shell::PhoneTabSwitcherHit::Row(tab_id) => {
                        destroy_phone_tab_switcher(owner_hwnd);
                        dispatch_phone_switcher_command(
                            owner_hwnd,
                            crate::shell::phone_tab_click_command(&tab_id),
                        );
                    }
                    crate::shell::PhoneTabSwitcherHit::Close(tab_id) => {
                        destroy_phone_tab_switcher(owner_hwnd);
                        dispatch_phone_switcher_command(
                            owner_hwnd,
                            crate::shell::phone_tab_close_command(&tab_id),
                        );
                    }
                    crate::shell::PhoneTabSwitcherHit::Sheet => {}
                    crate::shell::PhoneTabSwitcherHit::Dismiss => {
                        destroy_phone_tab_switcher(owner_hwnd);
                    }
                }
            }
            LRESULT(0)
        }
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(feature = "shell-chrome")]
fn dispatch_phone_switcher_command(owner: HWND, command: WindowsChromeCommand) {
    if let Some(webtag_key) = active_webtag_key_for_window(owner) {
        invoke_chrome_command(&webtag_key, owner, (0, 0), command);
    }
}

/// Uploads the switcher as a per-pixel-alpha layered window: the sheet is
/// opaque with anti-aliased top corners; everything above it is the dim.
#[cfg(feature = "shell-chrome")]
fn upload_phone_tab_switcher(hwnd: HWND, layout: &crate::shell::PhoneTabSwitcherLayout) {
    let width = layout.width;
    let height = layout.height;
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
        crate::shell::paint_phone_tab_switcher(dc, layout);
        let pixel_count = (width * height) as usize;
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u32>(), pixel_count);
        apply_phone_switcher_alpha(pixels, width, height, layout.sheet.top);
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

/// Alpha for the switcher surface: a 35% dim above the sheet, opaque sheet
/// below with an anti-aliased falloff into the dim at its top corners.
#[cfg(feature = "shell-chrome")]
fn apply_phone_switcher_alpha(pixels: &mut [u32], width: i32, height: i32, sheet_top: i32) {
    const DIM: u32 = 0x59;
    let radius = crate::shell::PHONE_SWITCHER_SHEET_RADIUS;
    let coverage_at = |x: i32, y: i32| -> f32 {
        if y >= sheet_top + radius {
            return 1.0;
        }
        let corner_x = if x < radius {
            radius
        } else if x >= width - radius {
            width - radius
        } else {
            return 1.0;
        };
        let dx = x as f32 + 0.5 - corner_x as f32;
        let dy = y as f32 + 0.5 - (sheet_top + radius) as f32;
        let distance = (dx * dx + dy * dy).sqrt();
        (radius as f32 - distance + 0.5).clamp(0.0, 1.0)
    };
    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) as usize;
            let pixel = pixels[index];
            let alpha = if y < sheet_top {
                DIM
            } else {
                let coverage = coverage_at(x, y);
                DIM + ((255 - DIM) as f32 * coverage) as u32
            };
            pixels[index] = match alpha {
                255 => 0xff00_0000 | (pixel & 0x00ff_ffff),
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

/// Scales premultiplied pixels by per-corner rounded coverage `[tl, tr, br,
/// bl]`, so a snapshot of a composition-clipped surface keeps the corners the
/// live surface had (the nav slide otherwise ghosts square corners over the
/// rounded workspace).
#[cfg(feature = "components")]
fn apply_corner_alpha_mask(pixels: &mut [u32], width: i32, height: i32, radii: [i32; 4]) {
    if radii == [0; 4] {
        return;
    }
    let [tl, tr, br, bl] = radii;
    let coverage_at = |x: i32, y: i32| -> u32 {
        let (radius, corner_x, corner_y) = if x < tl && y < tl {
            (tl, tl, tl)
        } else if x >= width - tr && y < tr {
            (tr, width - tr, tr)
        } else if x >= width - br && y >= height - br {
            (br, width - br, height - br)
        } else if x < bl && y >= height - bl {
            (bl, bl, height - bl)
        } else {
            return 255;
        };
        let dx = x as f32 + 0.5 - corner_x as f32;
        let dy = y as f32 + 0.5 - corner_y as f32;
        let distance = (dx * dx + dy * dy).sqrt();
        ((radius as f32 - distance + 0.5).clamp(0.0, 1.0) * 255.0) as u32
    };
    for y in 0..height {
        for x in 0..width {
            let coverage = coverage_at(x, y);
            if coverage == 255 {
                continue;
            }
            let index = (y * width + x) as usize;
            if coverage == 0 {
                pixels[index] = 0;
                continue;
            }
            let pixel = pixels[index];
            let scale = |channel: u32| (channel * coverage + 127) / 255;
            pixels[index] = (scale(pixel >> 24) << 24)
                | (scale((pixel >> 16) & 0xff) << 16)
                | (scale((pixel >> 8) & 0xff) << 8)
                | scale(pixel & 0xff);
        }
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
    // The popup floats a few pixels off the rail, so the path from the
    // anchor into the popup crosses a dead gap; a mouse move sampled inside
    // that gap must not dismiss, or reaching the popup takes a lucky flick.
    // The bridge spans only the gap column (not the rail below the anchor,
    // so hovering other rail items still dismisses).
    let bridge = if popup.rect.left >= popup.anchor.right {
        RECT {
            left: popup.anchor.right,
            top: popup.anchor.top.min(popup.rect.top),
            right: popup.rect.left,
            bottom: popup.anchor.bottom.max(popup.rect.bottom),
        }
    } else {
        RECT {
            left: popup.rect.right,
            top: popup.anchor.top.min(popup.rect.top),
            right: popup.anchor.left,
            bottom: popup.anchor.bottom.max(popup.rect.bottom),
        }
    };
    if !point_in_screen_rect(cursor, popup.anchor)
        && !point_in_screen_rect(cursor, popup.rect)
        && !point_in_screen_rect(cursor, bridge)
    {
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
                    let command = crate::shell::collapsed_sidebar_tabbar_click_command(
                        &popup.tabbar.group_id,
                        index,
                    );
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
    let (corner_radii, corner_color) = surface_clip_style(hwnd, webtag_key, rect);
    let fit_scale = window_fit_scale(hwnd);
    let host_bounds = ContentBounds {
        hwnd: hwnd_handle(hwnd),
        left: rect.left,
        top: rect.top,
        width,
        height,
        corner_radii,
        corner_color,
        fit_scale_milli: (fit_scale * 1000.0).round() as u32,
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
    let bounds_changed = webtag_content_bounds_changed(webtag_key, host_bounds);
    if bounds_changed {
        // A fit-scaled device frame renders the page at `fit` physical px
        // per CSS px, keeping the simulated device's logical viewport.
        if let Err(err) = handler.set_rasterization_scale(fit_scale) {
            log::debug!("Failed to set WebView rasterization scale: {err}");
        }
        #[cfg(feature = "runtime")]
        report_surface_viewport(hwnd, &webtag, width, height);
    }
    if bounds_changed
        && let Err(err) = handler.set_content_geometry(
            controller_bounds.left,
            controller_bounds.top,
            controller_bounds.right - controller_bounds.left,
            controller_bounds.bottom - controller_bounds.top,
            corner_radii,
            corner_color,
        )
    {
        log::debug!("Failed to sync Windows WebView content bounds: {err}");
    }
    if bounds_changed {
        // WebView2 does not invalidate the parent pixels it just uncovered.
        // Without this repaint, a smaller incoming surface leaves the old
        // page's white right/bottom strips in the shell gutter until another
        // unrelated desktop activation happens.
        repaint_window_now(hwnd);
    }
    let _ = handler.notify_parent_position_changed();
}

/// Device-frame fit factor for a host window; 1.0 when unframed.
fn window_fit_scale(hwnd: HWND) -> f64 {
    #[cfg(feature = "device-frame")]
    if let Some(fit) = crate::device_frame::device_frame_fit_scale(hwnd_handle(hwnd)) {
        return fit;
    }
    let _ = hwnd;
    1.0
}

#[cfg(feature = "runtime")]
fn report_surface_viewport(hwnd: HWND, webtag: &WebTag, width: i32, height: i32) {
    if width <= 0 || height <= 0 {
        return;
    }
    let appid = webtag.extract_appid();
    if appid.is_empty() {
        return;
    }
    // A fit-scaled device frame overrides the DPI mapping: physical px are
    // `fit` per CSS px there, regardless of monitor scale.
    let fit = window_fit_scale(hwnd);
    let scale = if fit < 1.0 {
        fit
    } else {
        let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) };
        if dpi == 0 { 1.0 } else { dpi as f64 / 96.0 }
    };
    lingxia::windows::set_surface_viewport(&appid, width as f64 / scale, height as f64 / scale);
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
            attached.main_region = normalize_rect(attached.main_region);
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
                resize_handle: panel.resize_handle,
                host_content: host_panel_content(&panel_id),
                docked: panel_is_docked(&panel_id),
            }
        })
        .collect();
    Some(WindowsChromeAttachedState {
        main_region: layout.main_region,
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
        // A focused float/adaptive-aside overlay is a child presentation, not
        // the shell host that a new main tab should replace. Resolve it back
        // to its owner before considering the overlay's own HWND.
        .and_then(|key| floating_overlay_owner(&key).or_else(|| window_handle_for_key(&key)))
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
    // The first WM_SIZE can run while the home lxapp is still opening, before
    // its surface graph can accept the width. Seed again once this HWND has
    // become the real shell host so the graph never stays at its Medium
    // default until the user manually resizes the window.
    #[cfg(feature = "shell-chrome")]
    report_shell_surface_width(hwnd);
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

/// The shell owns exactly ONE workspace window. Whenever a visible shell
/// host exists, a hidden candidate (typically a stale per-webtag
/// registration pointing at the webview's own parked parent window) must
/// lose to it — showing the candidate would surface a duplicate shell
/// window beside the workspace. With no visible host (startup presents
/// into a still-hidden window) the candidate passes through untouched.
fn prefer_visible_workspace(candidate: Option<HWND>) -> Option<HWND> {
    match candidate {
        Some(hwnd) if is_window_visible(hwnd) => Some(hwnd),
        other => first_visible_registered_host_window_except(None)
            .filter(|hwnd| !is_separate_shell_window(*hwnd))
            .or(other),
    }
}

/// True for windows that are legitimately their own top-level presentation
/// (native-framed windows, device-framed simulators) rather than the shared
/// shell workspace.
fn is_separate_shell_window(hwnd: HWND) -> bool {
    if is_native_framed_window(hwnd) {
        return true;
    }
    #[cfg(feature = "device-frame")]
    if crate::device_frame::window_has_device_frame(hwnd_handle(hwnd)) {
        return true;
    }
    false
}

/// Convergence half of the single-workspace invariant: after presenting
/// into `host`, any other visible shell host window is a duplicate and is
/// hidden. Real separate windows (native-framed, device-framed) stay.
fn hide_other_workspace_windows(host: HWND) {
    for hwnd in registered_host_windows() {
        if hwnd == host || !is_window_visible(hwnd) || is_separate_shell_window(hwnd) {
            continue;
        }
        log::info!("hiding duplicate shell workspace window {:?}", hwnd.0);
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
fn update_registered_panel_position(panel_id: &str, position: WindowsPanelPosition) {
    if let Some(panels) = WEBVIEW_PANELS.get()
        && let Ok(mut panels) = panels.lock()
        && let Some(panel) = panels.get_mut(panel_id)
    {
        panel.position = position;
        panel.docked = panel_position_is_flush_docked(position);
    }
    if let Some(panels) = HOST_PANELS.get()
        && let Ok(mut panels) = panels.lock()
        && let Some(panel) = panels.get_mut(panel_id)
    {
        panel.position = position;
        panel.docked = panel_position_is_flush_docked(position);
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

fn webtag_is_registered_panel(webtag_key: &str) -> bool {
    WEBVIEW_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .is_some_and(|panels| panels.values().any(|panel| panel.webtag_key == webtag_key))
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
    if let Some(position) = panel_position_override(panel_id) {
        return position;
    }
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

#[cfg(feature = "shell-chrome")]
fn panel_position_override(panel_id: &str) -> Option<WindowsPanelPosition> {
    PANEL_POSITION_OVERRIDES
        .get()
        .and_then(|overrides| overrides.lock().ok())
        .and_then(|overrides| overrides.get(panel_id).copied())
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
        // WS_SIZEBOX reserves an 8px DWM-owned strip inside every edge even
        // after WM_NCCALCSIZE makes the client full-window. Edge resize and
        // snap are handled by the custom hit-test/drag path instead.
        WS_POPUP.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0
    };
    apply_window_style(hwnd, WINDOW_STYLE(style))?;
    if !window_is_device_framed(hwnd) {
        apply_shell_window_dressing(hwnd);
    }
    Ok(())
}

/// Last main shell host selected by a presentation. Unlike a page-derived
/// lookup this remains valid after WM_CLOSE hides the window, so tray activate
/// can restore the exact HWND the user closed.
#[cfg(feature = "browser-shell")]
pub(crate) fn primary_host_window_handle() -> Option<isize> {
    primary_host_window_except(None).map(hwnd_handle)
}

/// Ask DWM to round the outer window without extending its non-client frame
/// into the client. Extending even one pixel recreates the unpaintable system
/// strip this custom shell deliberately removes.
fn apply_shell_window_dressing(hwnd: HWND) {
    use windows::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
        DwmSetWindowAttribute,
    };

    unsafe {
        let preference = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const c_void,
            std::mem::size_of_val(&preference) as u32,
        );
        // Win11 otherwise paints a light/dark activation-dependent stroke
        // around WS_SIZEBOX windows. With custom client chrome that becomes
        // the unwanted white/gray band around all four app edges.
        let border_color = DWMWA_COLOR_NONE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &border_color as *const _ as *const c_void,
            std::mem::size_of_val(&border_color) as u32,
        );
    }
}

fn apply_window_style(hwnd: HWND, style: WINDOW_STYLE) -> StdResult<()> {
    unsafe {
        // Idempotence matters: SWP_FRAMECHANGED makes DWM rebuild the
        // window's composition tree, which drops the child surfaces'
        // DirectComposition content for a frame — a re-present with an
        // unchanged style (clicking the already-active tab) would flash
        // every webview to garbage for that frame. Runtime state bits stay
        // out of the rewrite: clearing WS_MAXIMIZE/WS_MINIMIZE would corrupt
        // maximize/restore and defeat this check on a maximized window.
        let current = WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWL_STYLE);
        let preserved = current
            & (WindowsAndMessaging::WS_VISIBLE.0
                | WindowsAndMessaging::WS_MAXIMIZE.0
                | WindowsAndMessaging::WS_MINIMIZE.0) as isize;
        let wanted = style.0 as isize | preserved;
        if current == wanted {
            return Ok(());
        }
        let _ =
            WindowsAndMessaging::SetWindowLongPtrW(hwnd, WindowsAndMessaging::GWL_STYLE, wanted);
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

/// Extend custom shell chrome through the whole top-level window. The custom
/// hit test and drag path own every resize edge, so no native frame is needed.
fn custom_shell_nc_calc_size(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    if windows_chrome_renderer().is_none() || is_native_framed_window(hwnd) {
        return None;
    }
    if lparam.0 == 0 {
        return Some(LRESULT(0));
    }

    // A maximized WS_SIZEBOX window extends beyond the monitor bounds by its
    // resize frame. Once the non-client frame is removed, clamp the client to
    // the work area so the custom caption and edge pixels remain reachable.
    if unsafe { WindowsAndMessaging::IsZoomed(hwnd).as_bool() } {
        let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(monitor, &mut info).as_bool() } {
            if wparam.0 != 0 {
                let params = lparam.0 as *mut WindowsAndMessaging::NCCALCSIZE_PARAMS;
                if !params.is_null() {
                    unsafe { (*params).rgrc[0] = info.rcWork };
                }
            } else {
                let rect = lparam.0 as *mut RECT;
                if !rect.is_null() {
                    unsafe { *rect = info.rcWork };
                }
            }
        }
    }
    Some(LRESULT(0))
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

/// Repaints exposed host pixels synchronously. WebView2 composition can keep
/// presenting an outgoing controller until `SetIsVisible(false)` completes;
/// a queued invalidation alone then leaves its last white frame in the newly
/// uncovered shell gutter until some unrelated desktop damage occurs.
fn repaint_window_now(hwnd: HWND) {
    invalidate_window(hwnd);
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::UpdateWindow(hwnd);
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
    // The lxapp navbar belongs to the main region and therefore tracks both
    // axes when a panel opens, closes, or resizes.
    // `shell_chrome_dirty_rects` diffs only the lxapp layout (unchanged on those
    // events), so repaint the top strip here when the *main rect* changes.
    // Gating on the main rect (not on full attached equality) is deliberate: a
    // live aside's frequent re-syncs leave the main width unchanged, so the top
    // strip is not repainted on every tick - which would flicker the navbar,
    // sidebar header, and address bar.
    let previous_main = previous
        .attached
        .as_ref()
        .map(|attached| attached.main_region);
    let current_main = attached.map(|attached| attached.main_region);
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
    push_unique_dirty_rect(dirty, attached.main_region);
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
        // Windows 11 discovers custom-titlebar snap layouts through
        // HTMAXBUTTON. The other caption buttons stay client-owned because
        // the shell paints and invokes them itself.
        Some(WindowsChromeHit::FrameButton(button)) => frame_button_non_client_hit(button)
            .map(LRESULT)
            .unwrap_or(LRESULT(WindowsAndMessaging::HTCLIENT as isize)),
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

fn window_resize_edge(hit: usize) -> Option<WindowResizeEdge> {
    match hit as u32 {
        WindowsAndMessaging::HTLEFT => Some(WindowResizeEdge::Left),
        WindowsAndMessaging::HTRIGHT => Some(WindowResizeEdge::Right),
        WindowsAndMessaging::HTTOP => Some(WindowResizeEdge::Top),
        WindowsAndMessaging::HTBOTTOM => Some(WindowResizeEdge::Bottom),
        WindowsAndMessaging::HTTOPLEFT => Some(WindowResizeEdge::TopLeft),
        WindowsAndMessaging::HTTOPRIGHT => Some(WindowResizeEdge::TopRight),
        WindowsAndMessaging::HTBOTTOMLEFT => Some(WindowResizeEdge::BottomLeft),
        WindowsAndMessaging::HTBOTTOMRIGHT => Some(WindowResizeEdge::BottomRight),
        _ => None,
    }
}

/// `WS_SIZEBOX` gives Windows native resize/snap behavior but also reserves an
/// unpaintable, system-colored frame inside every edge of this custom client
/// window. Keep the HWND frame-free and run the same edge gesture from our
/// existing `WM_NCHITTEST` results.
fn begin_window_resize_drag(hwnd: HWND, hit: usize) -> bool {
    let Some(edge) = window_resize_edge(hit) else {
        return false;
    };
    if unsafe { WindowsAndMessaging::IsZoomed(hwnd).as_bool() } {
        return false;
    }
    let mut cursor = POINT::default();
    let mut window = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetCursorPos(&mut cursor).is_err()
            || WindowsAndMessaging::GetWindowRect(hwnd, &mut window).is_err()
        {
            return false;
        }
        let _ = SetCapture(hwnd);
    }
    if let Ok(mut drags) = WINDOW_RESIZE_DRAGS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        drags.insert(
            hwnd_handle(hwnd),
            WindowResizeDrag {
                edge,
                cursor,
                window,
            },
        );
        true
    } else {
        unsafe {
            let _ = ReleaseCapture();
        }
        false
    }
}

fn window_resize_drag(hwnd: HWND) -> Option<WindowResizeDrag> {
    WINDOW_RESIZE_DRAGS
        .get()
        .and_then(|drags| drags.lock().ok())
        .and_then(|drags| drags.get(&hwnd_handle(hwnd)).copied())
}

fn update_window_resize_drag(hwnd: HWND) -> bool {
    let Some(drag) = window_resize_drag(hwnd) else {
        return false;
    };
    let mut cursor = POINT::default();
    if unsafe { WindowsAndMessaging::GetCursorPos(&mut cursor).is_err() } {
        return true;
    }
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) }.max(96) as i32;
    let rect = resized_window_rect(drag, cursor, dpi);
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd,
            None,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            WindowsAndMessaging::SWP_NOACTIVATE | WindowsAndMessaging::SWP_NOZORDER,
        );
    }
    true
}

fn resized_window_rect(drag: WindowResizeDrag, cursor: POINT, dpi: i32) -> RECT {
    let dx = cursor.x - drag.cursor.x;
    let dy = cursor.y - drag.cursor.y;
    let mut rect = drag.window;
    let moves_left = matches!(
        drag.edge,
        WindowResizeEdge::Left | WindowResizeEdge::TopLeft | WindowResizeEdge::BottomLeft
    );
    let moves_right = matches!(
        drag.edge,
        WindowResizeEdge::Right | WindowResizeEdge::TopRight | WindowResizeEdge::BottomRight
    );
    let moves_top = matches!(
        drag.edge,
        WindowResizeEdge::Top | WindowResizeEdge::TopLeft | WindowResizeEdge::TopRight
    );
    let moves_bottom = matches!(
        drag.edge,
        WindowResizeEdge::Bottom | WindowResizeEdge::BottomLeft | WindowResizeEdge::BottomRight
    );
    if moves_left {
        rect.left += dx;
    }
    if moves_right {
        rect.right += dx;
    }
    if moves_top {
        rect.top += dy;
    }
    if moves_bottom {
        rect.bottom += dy;
    }

    let dpi = dpi.max(96);
    let min_width = 640 * dpi / 96;
    let min_height = 480 * dpi / 96;
    if rect.right - rect.left < min_width {
        if moves_left {
            rect.left = rect.right - min_width;
        } else {
            rect.right = rect.left + min_width;
        }
    }
    if rect.bottom - rect.top < min_height {
        if moves_top {
            rect.top = rect.bottom - min_height;
        } else {
            rect.bottom = rect.top + min_height;
        }
    }

    rect
}

fn end_window_resize_drag(hwnd: HWND, release_capture: bool) -> bool {
    let removed = WINDOW_RESIZE_DRAGS
        .get()
        .and_then(|drags| drags.lock().ok())
        .and_then(|mut drags| drags.remove(&hwnd_handle(hwnd)))
        .is_some();
    if removed && release_capture {
        unsafe {
            let _ = ReleaseCapture();
        }
    }
    removed
}

fn snap_window_after_caption_drag(hwnd: HWND) {
    if unsafe { WindowsAndMessaging::IsZoomed(hwnd).as_bool() } {
        return;
    }
    let mut cursor = POINT::default();
    if unsafe { WindowsAndMessaging::GetCursorPos(&mut cursor).is_err() } {
        return;
    }
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info).as_bool() } {
        return;
    }
    let work = info.rcWork;
    let threshold = 2;
    unsafe {
        if cursor.y <= work.top + threshold {
            let _ = WindowsAndMessaging::ShowWindow(hwnd, WindowsAndMessaging::SW_MAXIMIZE);
        } else if cursor.x <= work.left + threshold {
            let _ = WindowsAndMessaging::SetWindowPos(
                hwnd,
                None,
                work.left,
                work.top,
                (work.right - work.left) / 2,
                work.bottom - work.top,
                WindowsAndMessaging::SWP_NOACTIVATE | WindowsAndMessaging::SWP_NOZORDER,
            );
        } else if cursor.x >= work.right - threshold {
            let width = (work.right - work.left) / 2;
            let _ = WindowsAndMessaging::SetWindowPos(
                hwnd,
                None,
                work.right - width,
                work.top,
                width,
                work.bottom - work.top,
                WindowsAndMessaging::SWP_NOACTIVATE | WindowsAndMessaging::SWP_NOZORDER,
            );
        }
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
    #[cfg(feature = "shell-chrome")]
    if let Some(panel_id) = TERMINAL_SELECTION_DRAGS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .and_then(|drags| drags.get(&hwnd_handle(hwnd)).cloned())
    {
        crate::shell::update_terminal_selection(&panel_id, point.0, point.1);
        return true;
    }
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

fn frame_button_non_client_hit(button: WindowsFrameButton) -> Option<isize> {
    (button == WindowsFrameButton::Maximize).then_some(WindowsAndMessaging::HTMAXBUTTON as isize)
}

fn handle_chrome_mouse_wheel(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> bool {
    let mut point = lparam_screen_point(lparam);
    if unsafe { !ScreenToClient(hwnd, &mut point).as_bool() } {
        return false;
    }
    let client_point = (point.x, point.y);

    let wheel_delta = ((wparam.0 >> 16) & 0xffff) as u16 as i16;
    #[cfg(feature = "shell-chrome")]
    if let Some(WindowsChromeHit::Focusable { id, .. }) = chrome_hit_for_window(hwnd, client_point)
        && id == "terminal"
    {
        if wheel_delta != 0 {
            let _ = crate::shell::scroll_pane_at(
                &id,
                client_point.0,
                client_point.1,
                i32::from(wheel_delta),
            );
        }
        return true;
    }

    let Some(renderer) = windows_chrome_renderer() else {
        return false;
    };
    let Some(state) = chrome_state_for_window(hwnd) else {
        return false;
    };
    let Some(command) = renderer.mouse_wheel(&state, client_point, wheel_delta) else {
        return false;
    };
    let Some(webtag_key) = active_webtag_key_for_window(hwnd) else {
        return false;
    };
    invoke_chrome_command(&webtag_key, hwnd, client_point, command);
    true
}

#[cfg(feature = "shell-chrome")]
fn finish_terminal_selection_drag(hwnd: HWND, point: Option<(i32, i32)>) -> bool {
    let panel_id = TERMINAL_SELECTION_DRAGS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&hwnd_handle(hwnd));
    let Some(panel_id) = panel_id else {
        return false;
    };
    if let Some(point) = point {
        crate::shell::update_terminal_selection(&panel_id, point.0, point.1);
    }
    crate::shell::end_terminal_selection(&panel_id);
    true
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
            #[cfg(feature = "shell-chrome")]
            if id == "terminal" && crate::shell::begin_terminal_selection(&id, point.0, point.1) {
                TERMINAL_SELECTION_DRAGS
                    .get_or_init(|| Mutex::new(HashMap::new()))
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(hwnd_handle(hwnd), id.clone());
                unsafe {
                    let _ = SetCapture(hwnd);
                }
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
    #[cfg(feature = "shell-chrome")]
    if finish_terminal_selection_drag(hwnd, Some(point)) {
        unsafe {
            let _ = ReleaseCapture();
        }
        return true;
    }
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

/// Reports the adaptive container width only from the primary shell host.
/// Every WebView2 controller starts with its own 1024px top-level parent; an
/// aside or a new aside-browser tab must not overwrite the real workspace
/// width while that temporary parent is created.
#[cfg(feature = "shell-chrome")]
fn report_shell_surface_width(hwnd: HWND) {
    let primary = PRIMARY_HOST_WINDOW
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| *slot)
        .is_some_and(|window| window == hwnd_handle(hwnd));
    if !primary || is_native_framed_window(hwnd) || active_webtag_key_for_window(hwnd).is_none() {
        return;
    }
    crate::shell::update_surface_width(window_logical_client_width(hwnd));
}

pub fn find_webview_content_window(webtag: &WebTag) -> Option<WindowsWebViewContentWindow> {
    let hwnd = window_handle_for_key(webtag.key())?;
    let client = content_rect_for_window(hwnd, webtag.key());
    Some(WindowsWebViewContentWindow {
        window: hwnd_handle(hwnd),
        content_left: client.left,
        content_top: client.top,
        content_width: (client.right - client.left).max(0),
        content_height: (client.bottom - client.top).max(0),
        // The WebView2 controller is pinned to raw pixels (RasterizationScale
        // 1.0 in `configure_controller`), so CSS px == physical px regardless
        // of the monitor DPI. Native component overlays must map document
        // rects with the same factor — the monitor scale would blow them up
        // past the elements they cover (a 1.5x video on a 150% laptop). A
        // fit-scaled device frame is the exception: there CSS px map to
        // `fit` physical px.
        scale: window_fit_scale(hwnd),
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
    let content_corner_radii = find_webview_handler(webtag)
        .filter(|handler| handler.is_composition_hosted())
        .map(|_| surface_clip_style(hwnd, webtag.key(), content).0)
        .unwrap_or([0; 4]);
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
        content_corner_radii,
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
    // Route through the replacing variant's host pick (visible sibling →
    // registered window → primary host → own parent window): the shell owns
    // exactly one workspace window, and presenting straight onto the
    // webview's own parent here was how activating an lxapp whose open
    // region was already Main popped a duplicate shell window.
    show_webview_window_replacing(webtag, title, activate, Vec::new())
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
    set_window_handle(webtag.key(), hwnd);
    set_host_active_webtag(hwnd, webtag.key());
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
    // Host pick order: a visible sibling's window, the window this page is
    // already registered on, then the primary host. Only with none of those
    // does the webview's own parent window become the host. The sibling scan
    // keys off the visibility REGISTRY, which app deactivation clears for the
    // active page (WM_ACTIVATEAPP) — so a navigation while the app is not
    // foreground (e.g. re-navigating to the current page from a background
    // window) used to find no candidate and jump the app to the webview's raw
    // parent window, hiding every page in the host the user was looking at
    // (a stuck white shell) while a duplicate shell window appeared.
    let target = prefer_visible_workspace(
        stable_host_for_replacement(webtag, &hide_webtags)
            .or_else(|| {
                window_handle_for_key(webtag.key()).filter(|hwnd| is_valid_host_window(*hwnd))
            })
            .or_else(|| primary_host_window_except(None)),
    )
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
        // The deleted direct-show path unconditionally surfaced and (with
        // activate) foregrounded the window; a re-open of the already-active
        // page must still do both, and still converge stray duplicates.
        if !is_window_visible(target) || activate {
            unsafe {
                let mut flags = WindowsAndMessaging::SWP_NOMOVE
                    | WindowsAndMessaging::SWP_NOSIZE
                    | WindowsAndMessaging::SWP_SHOWWINDOW;
                if !activate {
                    flags |= WindowsAndMessaging::SWP_NOACTIVATE;
                }
                let _ = WindowsAndMessaging::SetWindowPos(target, None, 0, 0, 0, 0, flags);
                if activate {
                    let _ = WindowsAndMessaging::BringWindowToTop(target);
                    let _ = WindowsAndMessaging::SetForegroundWindow(target);
                }
            }
        }
        hide_other_workspace_windows(target);
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
    // The incoming layout may have the same content bounds as the outgoing
    // one (browser address bar and lxapp navbar are both one row tall), so the
    // bounds cache does not trigger a repaint. Paint the final active chrome
    // before revealing the incoming controller; otherwise one desktop frame
    // combines new page content with the old tab/address chrome.
    repaint_window_now(target);
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
    hide_other_workspace_windows(target);
    Ok(())
}

pub fn navigate_webview_window(
    webtag: &WebTag,
    title: &str,
    activate: bool,
    animation: WindowsNavAnimation,
) -> StdResult<()> {
    // A device-framed host presents its pages inside one fixed simulator
    // silhouette (`present_webview_in_active_group`) rather than as separate
    // top-level windows. Route navigation through that group present so it keeps
    // the frame, its corner overlays, and the group-main restore target — the
    // replace path below reparents the page into its own native window and
    // reasserts a plain shell frame, which escapes the device frame entirely
    // (the show path already branches this way; navigation must match).
    if let Some(host) = active_host_window() {
        if window_is_device_framed(host) {
            return navigate_with_snapshot_slide(host, animation, || {
                present_webview_in_active_group(webtag)
            });
        }
        // Desktop (non-framed) shell: same slide over the replace-path swap —
        // the content area animates while the sidebar/top-bar chrome stays put.
        return navigate_with_snapshot_slide(host, animation, || {
            show_webview_window_replacing(webtag, title, activate, normal_group_webtags(webtag))
        });
    }
    let _ = animation;
    show_webview_window_replacing(webtag, title, activate, normal_group_webtags(webtag))
}

/// Runs a navigation present under the page-transition animation when
/// `animation` asks for one. Forward/backward play the 300ms horizontal slide
/// iOS/Android have; everything else (redirect, switchTab, reLaunch) — and any
/// state the slide can't serve (host hidden, no outgoing page, capture failure)
/// — is the plain instant present.
///
/// WebView2 cannot be animated directly: a controller whose bounds move
/// off-screen stops compositing and shows blank white for the whole slide. So
/// the transition works iOS-snapshot style, inverted for that constraint: the
/// *outgoing* page is captured (`CapturePreview` — sees only the webview's own
/// composition, unoccluded), mounted as a layered overlay above the live
/// content, the destination is presented at rest underneath (where it paints
/// correctly), and the overlay slides off to reveal it. Because the overlay is
/// an image, this also covers the same-WebView case (re-navigating to the page
/// you are already on — pages are keyed by path, so the one live WebView is
/// reused; iOS handles it with `performSameWebViewAnimation`).
fn navigate_with_snapshot_slide(
    host: HWND,
    animation: WindowsNavAnimation,
    present: impl FnOnce() -> StdResult<()>,
) -> StdResult<()> {
    #[cfg(feature = "components")]
    {
        let slide = matches!(
            animation,
            WindowsNavAnimation::Forward | WindowsNavAnimation::Backward
        ) && is_window_visible(host)
            && !is_minimized(host);
        if slide
            && let Some(prev_key) = active_webtag_key_for_window(host)
            && !webtag_is_registered_panel(&prev_key)
            && webtag_is_visible(&prev_key)
            && prepare_nav_snapshot_slide(
                host,
                &prev_key,
                matches!(animation, WindowsNavAnimation::Forward),
            )
        {
            // The snapshot overlay now covers the content region, so the swap
            // underneath is invisible; the slide reveals its result.
            let result = present();
            if result.is_ok() {
                start_nav_snapshot_slide(host);
            } else {
                finish_nav_snapshot_slide(host);
            }
            return result;
        }
    }
    let _ = animation;
    present()
}

/// A navigation snapshot overlay mid-slide. The layered window stays FIXED
/// over the content rect — each frame shifts the image *inside* it, leaving
/// the vacated strip transparent — so the outgoing page stays clipped to the
/// device frame's screen area (moving the window itself would slide the image
/// out across the bezel and the desktop).
///
/// For smooth frames the source is a persistent double-width DIB (one half the
/// snapshot, the other fully transparent, ordered by direction); each tick just
/// re-presents the window from a shifted source offset — no per-frame pixel
/// copies or GDI object churn.
#[cfg(feature = "components")]
struct NavSnapshotSlide {
    overlay: isize,
    origin: POINT,
    width: i32,
    height: i32,
    /// Memory DC holding the double-width source bitmap, host-thread only.
    source_dc: isize,
    source_bitmap: isize,
    source_old_bitmap: isize,
    forward: bool,
    started: Option<Instant>,
}

/// Captures the outgoing page and mounts it as a layered overlay covering its
/// on-screen rect. Returns `false` (no overlay, caller falls back to the
/// instant swap) when the capture or the overlay can't be produced.
#[cfg(feature = "components")]
fn prepare_nav_snapshot_slide(host: HWND, prev_key: &str, forward: bool) -> bool {
    // A slide still in flight (fast repeated taps): settle it first so its
    // overlay never lingers.
    finish_nav_snapshot_slide(host);

    let Some(handler) = webtag_for_key(prev_key).and_then(|tag| find_webview_handler(&tag)) else {
        return false;
    };
    let png = match handler.capture_png() {
        Ok(png) => png,
        Err(err) => {
            log::debug!("nav slide: CapturePreview failed, navigating without animation: {err}");
            return false;
        }
    };
    let Ok(decoded) = image::load_from_memory(&png) else {
        return false;
    };
    let rgba = decoded.into_rgba8();
    let (width, height) = (rgba.width() as i32, rgba.height() as i32);
    if width <= 0 || height <= 0 {
        return false;
    }
    // Premultiplied BGRA, as UpdateLayeredWindow's AC_SRC_ALPHA expects.
    let mut pixels: Vec<u32> = rgba
        .pixels()
        .map(|p| {
            let [r, g, b, a] = p.0;
            let (a, r, g, b) = (a as u32, r as u32, g as u32, b as u32);
            let pm = |c: u32| c * a / 255;
            (a << 24) | (pm(r) << 16) | (pm(g) << 8) | pm(b)
        })
        .collect();

    let rect = content_rect_for_window(host, prev_key);
    // CapturePreview sees content pre-clip; carry the live surface's rounded
    // corners into the snapshot so the slide doesn't ghost square ones.
    if handler.is_composition_hosted() {
        apply_corner_alpha_mask(
            &mut pixels,
            width,
            height,
            surface_clip_style(host, prev_key, rect).0,
        );
    }
    let mut origin = POINT {
        x: rect.left,
        y: rect.top,
    };
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(host, &mut origin);
    }

    // Window creation must happen on the host's own thread — a helper window
    // created on a non-pumping thread wedges every later cross-thread
    // SetWindowPos over the owner group.
    let host_handle = hwnd_handle(host);
    let owner_thread = unsafe { WindowsAndMessaging::GetWindowThreadProcessId(host, None) };
    if owner_thread != 0 && owner_thread != unsafe { GetCurrentThreadId() } {
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let posted = post_to_window_thread(
            host_handle,
            Box::new(move || {
                let mounted = mount_nav_snapshot_overlay(
                    hwnd_from_handle(host_handle),
                    origin,
                    width,
                    height,
                    pixels,
                    forward,
                );
                let _ = done_tx.send(mounted);
            }),
        );
        return posted
            && done_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap_or(false);
    }
    mount_nav_snapshot_overlay(host, origin, width, height, pixels, forward)
}

/// Creates the layered overlay window on the host thread, uploads the snapshot
/// pixels, and registers the pending slide. The overlay sits directly above the
/// host in z-order — below the status-bar/tab-bar overlays, so persistent
/// chrome stays fixed while the page image slides beneath it.
#[cfg(feature = "components")]
fn mount_nav_snapshot_overlay(
    host: HWND,
    origin: POINT,
    width: i32,
    height: i32,
    pixels: Vec<u32>,
    forward: bool,
) -> bool {
    static NAV_OVERLAY_CLASS: OnceLock<()> = OnceLock::new();
    NAV_OVERLAY_CLASS.get_or_init(|| {
        let class = WNDCLASSW {
            lpfnWndProc: Some(nav_overlay_proc),
            lpszClassName: w!("LingXiaNavSlideOverlay"),
            ..Default::default()
        };
        unsafe {
            WindowsAndMessaging::RegisterClassW(&class);
        }
    });

    let overlay = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TRANSPARENT
                | WindowsAndMessaging::WS_EX_NOACTIVATE
                | WindowsAndMessaging::WS_EX_TOOLWINDOW,
            w!("LingXiaNavSlideOverlay"),
            PCWSTR::null(),
            WS_POPUP,
            origin.x,
            origin.y,
            width,
            height,
            Some(host),
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    };
    let Ok(overlay) = overlay else {
        return false;
    };

    // Build the persistent double-width source: [snapshot | transparent] for a
    // forward slide (image exits left), [transparent | snapshot] for backward.
    // Frames then only move the UpdateLayeredWindow source offset within it.
    let Some((source_dc, source_bitmap, source_old_bitmap)) =
        create_nav_snapshot_source(width, height, &pixels, forward)
    else {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(overlay);
        }
        return false;
    };
    let start_src_x = if forward { 0 } else { width };
    let presented = present_nav_snapshot_frame(
        overlay,
        origin,
        width,
        height,
        hdc_from_handle(source_dc),
        start_src_x,
    );
    if !presented {
        release_nav_snapshot_source(source_dc, source_bitmap, source_old_bitmap);
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(overlay);
        }
        return false;
    }
    unsafe {
        // Show at the bottom of the owned group: inserting after the owner puts
        // the overlay directly above the host but below the other owned
        // overlays (status bar, tab bar), which must stay fixed on top.
        let _ = WindowsAndMessaging::SetWindowPos(
            overlay,
            Some(host),
            0,
            0,
            0,
            0,
            WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
    }

    let slides = NAV_SNAPSHOT_SLIDES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut slides) = slides.lock() {
        slides.insert(
            hwnd_handle(host),
            NavSnapshotSlide {
                overlay: hwnd_handle(overlay),
                origin,
                width,
                height,
                source_dc,
                source_bitmap,
                source_old_bitmap,
                forward,
                started: None,
            },
        );
    }
    true
}

/// Builds the double-width (2w × h) source DIB + memory DC for a slide.
/// Returns `(dc, bitmap, old_bitmap)` as handles owned by the slide state.
#[cfg(feature = "components")]
fn create_nav_snapshot_source(
    width: i32,
    height: i32,
    pixels: &[u32],
    forward: bool,
) -> Option<(isize, isize, isize)> {
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return None;
        }
        let dc = CreateCompatibleDC(Some(screen));
        if dc.is_invalid() {
            let _ = ReleaseDC(None, screen);
            return None;
        }
        let source_width = width * 2;
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: source_width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bitmap = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0);
        let _ = ReleaseDC(None, screen);
        let Ok(bitmap) = bitmap else {
            let _ = DeleteDC(dc);
            return None;
        };
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            return None;
        }
        let old_bitmap = SelectObject(dc, HGDIOBJ(bitmap.0));

        let (w, sw) = (width as usize, source_width as usize);
        let dst = std::slice::from_raw_parts_mut(bits.cast::<u32>(), sw * height as usize);
        dst.fill(0);
        let image_col = if forward { 0 } else { w };
        for y in 0..height as usize {
            let src_row = y * w;
            if src_row + w <= pixels.len() {
                dst[y * sw + image_col..y * sw + image_col + w]
                    .copy_from_slice(&pixels[src_row..src_row + w]);
            }
        }
        Some((dc.0 as isize, bitmap.0 as isize, old_bitmap.0 as isize))
    }
}

#[cfg(feature = "components")]
fn hdc_from_handle(handle: isize) -> HDC {
    HDC(handle as *mut c_void)
}

/// Presents one slide frame: re-blends the fixed overlay window from the
/// persistent source DC at horizontal offset `src_x`. No pixel copies.
#[cfg(feature = "components")]
fn present_nav_snapshot_frame(
    overlay: HWND,
    origin: POINT,
    width: i32,
    height: i32,
    source_dc: HDC,
    src_x: i32,
) -> bool {
    let size = SIZE {
        cx: width,
        cy: height,
    };
    let src_origin = POINT { x: src_x, y: 0 };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    unsafe {
        WindowsAndMessaging::UpdateLayeredWindow(
            overlay,
            None,
            Some(&origin),
            Some(&size),
            Some(source_dc),
            Some(&src_origin),
            COLORREF(0),
            Some(&blend),
            WindowsAndMessaging::ULW_ALPHA,
        )
        .is_ok()
    }
}

/// Frees the slide's persistent source DC + DIB.
#[cfg(feature = "components")]
fn release_nav_snapshot_source(dc: isize, bitmap: isize, old_bitmap: isize) {
    unsafe {
        let dc = hdc_from_handle(dc);
        if old_bitmap != 0 {
            let _ = SelectObject(dc, HGDIOBJ(old_bitmap as *mut c_void));
        }
        let _ = DeleteObject(HGDIOBJ(bitmap as *mut c_void));
        let _ = DeleteDC(dc);
    }
}

#[cfg(feature = "components")]
unsafe extern "system" fn nav_overlay_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Starts the slide clock and its frame pacer.
///
/// Frames are paced by a short-lived thread that blocks on `DwmFlush` — the
/// compositor's vblank — and posts each frame to the host thread. `WM_TIMER`
/// is the wrong tool here: it coalesces to ~15.6ms and is the lowest-priority
/// message, so ticks are skipped whenever the host thread has any other work,
/// which reads as judder. Vblank pacing renders exactly one frame per display
/// refresh. Without DWM (rare) the pacer degrades to an 8ms sleep.
#[cfg(feature = "components")]
fn start_nav_snapshot_slide(host: HWND) {
    let slides = NAV_SNAPSHOT_SLIDES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut slides) = slides.lock() {
        let Some(slide) = slides.get_mut(&hwnd_handle(host)) else {
            return;
        };
        slide.started = Some(Instant::now());
    }
    let host_handle = hwnd_handle(host);
    let _ = std::thread::Builder::new()
        .name("lingxia-nav-slide".to_string())
        .spawn(move || {
            use windows::Win32::Graphics::Dwm::DwmFlush;
            let deadline = Instant::now()
                + std::time::Duration::from_millis(NAV_SLIDE_DURATION_MS as u64 + 250);
            while nav_snapshot_slide_active(host_handle) {
                if unsafe { DwmFlush() }.is_err() {
                    std::thread::sleep(std::time::Duration::from_millis(8));
                }
                if !post_to_window_thread(
                    host_handle,
                    Box::new(move || {
                        advance_nav_snapshot_slide(hwnd_from_handle(host_handle));
                    }),
                ) {
                    break;
                }
                if Instant::now() > deadline {
                    // Host thread never advanced the slide to completion (e.g.
                    // stalled); tear it down so the overlay can't linger.
                    finish_nav_snapshot_slide(hwnd_from_handle(host_handle));
                    break;
                }
            }
        });
}

#[cfg(feature = "components")]
fn nav_snapshot_slide_active(host: isize) -> bool {
    NAV_SNAPSHOT_SLIDES
        .get()
        .and_then(|slides| slides.lock().ok())
        .is_some_and(|slides| slides.contains_key(&host))
}

#[cfg(feature = "components")]
fn nav_snapshot_overlay(host: HWND) -> Option<isize> {
    NAV_SNAPSHOT_SLIDES
        .get()
        .and_then(|slides| slides.lock().ok())
        .and_then(|slides| slides.get(&hwnd_handle(host)).map(|slide| slide.overlay))
}

#[cfg(feature = "components")]
fn finish_nav_snapshot_slide_if_overlay(host: HWND, overlay: isize) {
    if nav_snapshot_overlay(host) == Some(overlay) {
        finish_nav_snapshot_slide(host);
    }
}

/// Advances one slide frame; runs on the host's `WM_TIMER`. Shifts the
/// snapshot image inside the fixed overlay (forward reveals leftward, backward
/// rightward) and tears the overlay down when the duration elapses.
#[cfg(feature = "components")]
fn advance_nav_snapshot_slide(host: HWND) {
    let slides = NAV_SNAPSHOT_SLIDES.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(slides_guard) = slides.lock() else {
        return;
    };
    let Some(state) = slides_guard.get(&hwnd_handle(host)) else {
        return;
    };
    let Some(started) = state.started else {
        return;
    };

    let elapsed = started.elapsed().as_micros() as f64 / 1000.0;
    let t = (elapsed / NAV_SLIDE_DURATION_MS).clamp(0.0, 1.0);
    let eased = t * t * (3.0 - 2.0 * t);
    // Forward: the old page slides off to the left; backward: to the right.
    // The double-width source is [image|transparent] (forward) or
    // [transparent|image] (backward), so both map to a plain source offset.
    let dx = (state.width as f64 * eased).round() as i32;
    let src_x = if state.forward { dx } else { state.width - dx };
    present_nav_snapshot_frame(
        hwnd_from_handle(state.overlay),
        state.origin,
        state.width,
        state.height,
        hdc_from_handle(state.source_dc),
        src_x,
    );
    drop(slides_guard);
    if t >= 1.0 {
        finish_nav_snapshot_slide(host);
    }
}

/// Tears down a pending or in-flight slide: stops the timer, destroys the
/// snapshot overlay, and frees its source DIB. The overlay and its DC live on
/// the host thread, so teardown marshals there when called from elsewhere
/// (`DestroyWindow` only works on the window's owning thread).
#[cfg(feature = "components")]
fn finish_nav_snapshot_slide(host: HWND) {
    let slides = NAV_SNAPSHOT_SLIDES.get_or_init(|| Mutex::new(HashMap::new()));
    let state = match slides.lock() {
        Ok(mut slides) => slides.remove(&hwnd_handle(host)),
        Err(_) => None,
    };
    let Some(state) = state else {
        return;
    };
    let teardown = move |_host: HWND| {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(state.overlay));
        }
        release_nav_snapshot_source(
            state.source_dc,
            state.source_bitmap,
            state.source_old_bitmap,
        );
    };
    let host_handle = hwnd_handle(host);
    let owner_thread = unsafe { WindowsAndMessaging::GetWindowThreadProcessId(host, None) };
    if owner_thread != 0 && owner_thread != unsafe { GetCurrentThreadId() } {
        let _ = post_to_window_thread(
            host_handle,
            Box::new(move || teardown(hwnd_from_handle(host_handle))),
        );
        return;
    }
    teardown(host);
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
                && !webtag_is_registered_panel(webtag.key())
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
    let current_host = window_handle_for_key(webtag.key());
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
    release_fullscreen_drill(webtag.key());
    unregister_floating_overlay(webtag.key());
    #[cfg(feature = "components")]
    restore_previous_lxapp_after_hide(current_host, webtag);
    #[cfg(not(feature = "components"))]
    let _ = current_host;
    Ok(())
}

/// Needs the lxapp runtime (`components`) to know which app is current; a
/// bare host-api build has no lxapp stack to restore from.
#[cfg(feature = "components")]
fn restore_previous_lxapp_after_hide(host: Option<HWND>, hidden: &WebTag) {
    let Some(host) = host else {
        return;
    };
    if active_webtag_key_for_window(host).as_deref() != Some(hidden.key()) {
        return;
    }
    let (appid, path, session_id) = lxapp::get_current_lxapp();
    if appid.is_empty() || path.is_empty() {
        return;
    }
    if appid == hidden.extract_appid() && session_id == hidden.session_id().unwrap_or_default() {
        return;
    }
    let restore = WebTag::new(&appid, &path, Some(session_id));
    let Some(restore_handler) = find_webview_handler(&restore) else {
        return;
    };
    if webtag_is_visible(restore.key()) {
        let _ = restore_handler.set_content_visible(false);
    }
    set_window_handle(restore.key(), host);
    set_host_active_webtag(host, restore.key());
    set_primary_host_window(host);
    sync_window_layout(host);
    if restore_handler.set_content_visible(true).is_ok() {
        mark_active(&restore);
        notify_webtag_visibility(restore.key(), true);
        invalidate_window_chrome(host);
    }
}

/// A hidden or destroyed full-screen drill hands the screen back: the owner
/// relayouts so its tab bar (hidden underneath the drill) returns.
fn release_fullscreen_drill(webtag_key: &str) {
    let owners: Vec<isize> = FULLSCREEN_DRILLS
        .get()
        .and_then(|drills| drills.lock().ok())
        .map(|mut drills| {
            let owners = drills
                .iter()
                .filter(|(_, key)| key.as_str() == webtag_key)
                .map(|(owner, _)| *owner)
                .collect::<Vec<_>>();
            for owner in &owners {
                drills.remove(owner);
            }
            owners
        })
        .unwrap_or_default();
    for owner in owners {
        if is_window_handle_valid(owner) {
            sync_window_layout(hwnd_from_handle(owner));
        }
    }
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

#[cfg(feature = "runtime")]
fn install_lingxia_system_menu(hwnd: HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{
        DrawMenuBar, GetMenuItemCount, GetSystemMenu, InsertMenuW, MF_BYPOSITION, MF_SEPARATOR,
        MF_STRING,
    };

    unsafe {
        let menu = GetSystemMenu(hwnd, false);
        if menu.is_invalid() {
            return;
        }
        if GetMenuItemCount(Some(menu)) <= 0 {
            return;
        }

        let about = to_wide(&localized_system_menu_label("common.about", "About"));
        let _ = InsertMenuW(
            menu,
            0,
            MF_BYPOSITION | MF_STRING,
            SYSTEM_MENU_ABOUT_COMMAND,
            PCWSTR(about.as_ptr()),
        );
        let _ = InsertMenuW(menu, 1, MF_BYPOSITION | MF_SEPARATOR, 0, PCWSTR::null());
        let _ = DrawMenuBar(hwnd);
    }
}

#[cfg(not(feature = "runtime"))]
fn install_lingxia_system_menu(_hwnd: HWND) {}

#[cfg(feature = "runtime")]
fn handle_lingxia_system_menu_command(hwnd: HWND, command: usize) -> bool {
    match command & SYSTEM_MENU_COMMAND_MASK {
        SYSTEM_MENU_ABOUT_COMMAND => {
            show_lingxia_system_about(hwnd);
            true
        }
        _ => false,
    }
}

#[cfg(not(feature = "runtime"))]
fn handle_lingxia_system_menu_command(_hwnd: HWND, _command: usize) -> bool {
    false
}

#[cfg(feature = "runtime")]
fn show_lingxia_system_about(hwnd: HWND) {
    let title = localized_system_menu_label("common.about", "About");
    let app_name = lingxia::app::product_name()
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "LingXia".to_string());
    let version = lingxia::app::product_version()
        .map(str::to_string)
        .unwrap_or_default();
    let version_label = localized_system_menu_label("common.version", "Version");
    let body = if version.trim().is_empty() {
        app_name.clone()
    } else {
        format!("{app_name}\n{version_label} {version}")
    };
    if show_lingxia_system_about_task_dialog(hwnd, &title, &app_name, &body) {
        return;
    }
    show_lingxia_system_about_message_box(hwnd, &title, &body);
}

#[cfg(feature = "runtime")]
fn show_lingxia_system_about_task_dialog(
    hwnd: HWND,
    title: &str,
    app_name: &str,
    body: &str,
) -> bool {
    use windows::Win32::UI::Controls::{
        TASKDIALOGCONFIG, TASKDIALOGCONFIG_0, TDCBF_OK_BUTTON, TDF_ALLOW_DIALOG_CANCELLATION,
        TDF_POSITION_RELATIVE_TO_WINDOW, TDF_USE_HICON_MAIN, TaskDialogIndirect,
    };
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, HICON};

    let from_path = crate::app_icon::current_app_icon_path()
        .and_then(|path| crate::app_icon::create_icon_handle_from_path(&path, 64));
    let (icon_handle, owns_icon) = match from_path {
        Some(handle) => (handle, true),
        None => (
            crate::app_icon::current_large_icon_handle().unwrap_or(0),
            false,
        ),
    };

    let title = to_wide(title);
    let instruction = to_wide(app_name);
    let body = to_wide(body);
    let mut flags = TDF_ALLOW_DIALOG_CANCELLATION | TDF_POSITION_RELATIVE_TO_WINDOW;
    let main_icon = if icon_handle != 0 {
        flags |= TDF_USE_HICON_MAIN;
        TASKDIALOGCONFIG_0 {
            hMainIcon: HICON(icon_handle as *mut core::ffi::c_void),
        }
    } else {
        TASKDIALOGCONFIG_0::default()
    };
    let config = TASKDIALOGCONFIG {
        cbSize: std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
        hwndParent: hwnd,
        dwFlags: flags,
        dwCommonButtons: TDCBF_OK_BUTTON,
        pszWindowTitle: PCWSTR(title.as_ptr()),
        pszMainInstruction: PCWSTR(instruction.as_ptr()),
        pszContent: PCWSTR(body.as_ptr()),
        Anonymous1: main_icon,
        ..Default::default()
    };

    let shown = unsafe { TaskDialogIndirect(&config, None, None, None) }.is_ok();
    if owns_icon && icon_handle != 0 {
        unsafe {
            let _ = DestroyIcon(HICON(icon_handle as *mut core::ffi::c_void));
        }
    }
    shown
}

#[cfg(feature = "runtime")]
fn show_lingxia_system_about_message_box(hwnd: HWND, title: &str, body: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_OK, MessageBoxW};

    let body = to_wide(body);
    let title = to_wide(title);
    unsafe {
        let _ = MessageBoxW(
            Some(hwnd),
            PCWSTR(body.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK,
        );
    }
}

#[cfg(feature = "runtime")]
fn localized_system_menu_label(key: &str, fallback: &str) -> String {
    lingxia_platform::i18n::text(key, fallback)
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
            WindowsAndMessaging::WM_CREATE => {
                install_lingxia_system_menu(hwnd);
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_SYSCOMMAND => {
                if handle_lingxia_system_menu_command(hwnd, wparam.0) {
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_CLOSE => {
                #[cfg(feature = "browser-shell")]
                if should_hide_window_on_close(hwnd) {
                    set_primary_host_window(hwnd);
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
                    // Activation is an app lifecycle signal, not controller or
                    // HWND visibility. Mutating WEBTAG_VISIBILITY here made a
                    // visible inactive WebView look hidden to reconciliation,
                    // so switching apps could expose stale white host pixels.
                    dispatch_webtag_lifecycle_visibility(&webtag_key, wparam.0 != 0);
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
                report_shell_surface_width(hwnd);
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
                // refresh_system_theme() reports the change exactly once per
                // process, and the broadcast may reach a hidden parked parent
                // window first — so the winner refreshes EVERY registered
                // host, not just itself: repaint plus a forced geometry
                // resync (the webview corner wedges carry the
                // theme-dependent shell background and the bounds-dedupe
                // cache would otherwise swallow the change).
                #[cfg(feature = "shell-chrome")]
                if crate::shell::refresh_system_theme() && windows_chrome_renderer().is_some() {
                    for host in registered_host_windows() {
                        if is_native_framed_window(host) {
                            continue;
                        }
                        invalidate_window(host);
                        let _ = request_host_layout_sync_inner(host, true);
                    }
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_PAINT => {
                if windows_chrome_renderer().is_some() && !is_native_framed_window(hwnd) {
                    paint_window_chrome(hwnd);
                    #[cfg(all(feature = "shell-chrome", feature = "terminal-runtime"))]
                    let _ = sync_terminal_ime_position(hwnd);
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_NCCALCSIZE => {
                if let Some(result) = custom_shell_nc_calc_size(hwnd, wparam, lparam) {
                    return result;
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
            WindowsAndMessaging::WM_NCLBUTTONDOWN => {
                if windows_chrome_renderer().is_some()
                    && !is_native_framed_window(hwnd)
                    && begin_window_resize_drag(hwnd, wparam.0)
                {
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_NCLBUTTONUP => {
                if end_window_resize_drag(hwnd, true) {
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_CAPTURECHANGED | WindowsAndMessaging::WM_CANCELMODE => {
                let _ = end_window_resize_drag(hwnd, false);
                #[cfg(feature = "shell-chrome")]
                let _ = finish_terminal_selection_drag(hwnd, None);
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_EXITSIZEMOVE => {
                snap_window_after_caption_drag(hwnd);
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_MOUSEMOVE => {
                if update_window_resize_drag(hwnd) {
                    return LRESULT(0);
                }
                if windows_chrome_renderer().is_some()
                    && !is_native_framed_window(hwnd)
                    && handle_chrome_mouse_move(hwnd, lparam_client_point(lparam), true)
                {
                    return LRESULT(0);
                }
                unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WindowsAndMessaging::WM_MOUSEWHEEL => {
                if windows_chrome_renderer().is_some()
                    && !is_native_framed_window(hwnd)
                    && handle_chrome_mouse_wheel(hwnd, wparam, lparam)
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
                if end_window_resize_drag(hwnd, true) {
                    return LRESULT(0);
                }
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
                #[cfg(feature = "shell-chrome")]
                destroy_phone_tab_switcher(hwnd);
                LRESULT(0)
            }
            WindowsAndMessaging::WM_NCDESTROY => {
                let _ = end_window_resize_drag(hwnd, false);
                #[cfg(feature = "shell-chrome")]
                destroy_transparent_tabbar_overlay(hwnd);
                #[cfg(feature = "shell-chrome")]
                destroy_sidebar_tabbar_popup(hwnd);
                #[cfg(feature = "shell-chrome")]
                let _ = finish_terminal_selection_drag(hwnd, None);
                release_chrome_back_buffer(hwnd);
                let webtag_key = window_webtag_key(hwnd);
                if let Some(key) = webtag_key.as_deref() {
                    release_fullscreen_drill(key);
                    unregister_floating_overlay(key);
                }
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
            WindowsAndMessaging::WM_IME_STARTCOMPOSITION
            | WindowsAndMessaging::WM_IME_COMPOSITION => {
                #[cfg(all(feature = "shell-chrome", feature = "terminal-runtime"))]
                let _ = sync_terminal_ime_position(hwnd);
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
        // Title the window with the app's product name so taskbar tooltips and
        // Alt+Tab distinguish apps sharing this exe (e.g. two dev runners).
        // Show paths that pass an explicit title re-title later; this covers
        // paths that never do (the device-framed group present).
        let title = to_wide(&default_window_title());
        let result = WindowsAndMessaging::CreateWindowExW(
            ex_style,
            w!("LingXiaWebViewParent"),
            PCWSTR(title.as_ptr()),
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

/// Initial title for webview host windows: the app's product name when the
/// runtime is up (per-app in the dev runner), else the generic fallback.
fn default_window_title() -> String {
    #[cfg(feature = "runtime")]
    if let Some(name) = lingxia::app::product_name()
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
    {
        return name;
    }
    "LingXia WebView".to_string()
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

/// Positions Windows IME UI at the focused terminal's painted cursor. The
/// terminal is a custom-drawn host panel, so Windows has no native edit caret
/// from which it could infer these coordinates.
#[cfg(all(feature = "shell-chrome", feature = "terminal-runtime"))]
fn sync_terminal_ime_position(hwnd: HWND) -> bool {
    let Some(panel_id) = focused_input_host_panel().filter(|panel_id| panel_id == "terminal")
    else {
        return false;
    };
    let Some(cursor) = crate::shell::terminal_grid::focused_cursor_rect(&panel_id) else {
        return false;
    };

    unsafe {
        let context = ImmGetContext(hwnd);
        if context.is_invalid() {
            return false;
        }
        let point = POINT {
            x: cursor.left,
            y: cursor.top,
        };
        let composition = COMPOSITIONFORM {
            dwStyle: CFS_POINT,
            ptCurrentPos: point,
            rcArea: cursor,
        };
        let candidate = CANDIDATEFORM {
            dwIndex: 0,
            dwStyle: CFS_EXCLUDE,
            ptCurrentPos: point,
            rcArea: cursor,
        };
        let composition_set = ImmSetCompositionWindow(context, &composition).as_bool();
        let candidate_set = ImmSetCandidateWindow(context, &candidate).as_bool();
        let _ = ImmReleaseContext(hwnd, context);
        composition_set || candidate_set
    }
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
    dispatch_webtag_lifecycle_visibility(webtag_key, visible);
}

fn dispatch_webtag_lifecycle_visibility(webtag_key: &str, visible: bool) {
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
    sync_active_webtag_host_ui(webtag_key);
}

fn sync_active_webtag_host_ui(webtag_key: &str) {
    let Some(webtag) = webtag_for_key(webtag_key) else {
        return;
    };
    let appid = webtag.extract_appid();
    if appid.is_empty() {
        return;
    }
    lingxia_platform::sync_windows_ui(&appid);
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

fn is_window_handle_valid(handle: isize) -> bool {
    unsafe { WindowsAndMessaging::IsWindow(Some(hwnd_from_handle(handle))).as_bool() }
}

fn hwnd_handle(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        WindowResizeDrag, WindowResizeEdge, WindowsFrameButton, frame_button_non_client_hit,
        resized_window_rect,
    };
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::UI::WindowsAndMessaging;

    #[test]
    fn maximize_exposes_the_native_snap_hit_target() {
        assert_eq!(
            frame_button_non_client_hit(WindowsFrameButton::Maximize),
            Some(WindowsAndMessaging::HTMAXBUTTON as isize)
        );
        assert_eq!(
            frame_button_non_client_hit(WindowsFrameButton::Minimize),
            None
        );
        assert_eq!(frame_button_non_client_hit(WindowsFrameButton::Close), None);
    }

    #[test]
    fn frame_free_resize_tracks_edges_and_dpi_minimums() {
        let base = RECT {
            left: 100,
            top: 100,
            right: 1124,
            bottom: 868,
        };
        let grown = resized_window_rect(
            WindowResizeDrag {
                edge: WindowResizeEdge::Right,
                cursor: POINT { x: 1124, y: 400 },
                window: base,
            },
            POINT { x: 1244, y: 400 },
            144,
        );
        assert_eq!(grown.right, 1244);
        assert_eq!(grown.left, base.left);

        let clamped = resized_window_rect(
            WindowResizeDrag {
                edge: WindowResizeEdge::TopLeft,
                cursor: POINT { x: 100, y: 100 },
                window: base,
            },
            POINT { x: 500, y: 500 },
            144,
        );
        assert_eq!(clamped.right - clamped.left, 960);
        assert_eq!(clamped.bottom - clamped.top, 720);
        assert_eq!(clamped.right, base.right);
        assert_eq!(clamped.bottom, base.bottom);
    }
}
