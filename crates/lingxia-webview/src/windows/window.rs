//! Native window management: creation, window proc, hit testing,
//! show/hide flows, bounds syncing, and placement persistence.

use super::*;

pub(crate) struct WindowUserData {
    webtag_key: String,
    /// Frame button currently under the cursor (client or non-client
    /// space). Only touched on the window's UI thread, hence `Cell`.
    hovered_frame_button: Cell<Option<WindowsFrameButton>>,
    /// Frame button with an in-progress left click.
    pressed_frame_button: Cell<Option<WindowsFrameButton>>,
    /// Whether `TrackMouseEvent(TME_LEAVE)` is armed for the client area.
    tracking_client_mouse: Cell<bool>,
    /// Whether `TrackMouseEvent(TME_LEAVE | TME_NONCLIENT)` is armed.
    tracking_nc_mouse: Cell<bool>,
}

impl WindowUserData {
    fn new(webtag_key: String) -> Self {
        Self {
            webtag_key,
            hovered_frame_button: Cell::new(None),
            pressed_frame_button: Cell::new(None),
            tracking_client_mouse: Cell::new(false),
            tracking_nc_mouse: Cell::new(false),
        }
    }
}

fn with_window_user_data<R>(hwnd: HWND, f: impl FnOnce(&WindowUserData) -> R) -> Option<R> {
    let raw =
        unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA) }
            as *mut WindowUserData;
    if raw.is_null() {
        None
    } else {
        Some(f(unsafe { &*raw }))
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct WindowPlacement {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}

pub(crate) static WINDOW_GROUP_PLACEMENTS: OnceLock<Mutex<HashMap<String, WindowPlacement>>> =
    OnceLock::new();

/// Per-UI-thread handle to the window's WebView2 controller so the window
/// procedure can drive layout itself. Interactive move/size runs inside
/// `DefWindowProcW`'s modal loop, which starves both the command channel
/// and posted thread messages, so anything that must track a drag live has
/// to run on the window-message path, never the command queue.
struct LiveLayoutContext {
    hwnd: HWND,
    webtag_key: String,
    controller: ICoreWebView2Controller,
}

thread_local! {
    static LIVE_LAYOUT_CONTEXT: RefCell<Option<LiveLayoutContext>> = const { RefCell::new(None) };
}

pub(crate) fn register_live_layout_context(state: &UiState) {
    LIVE_LAYOUT_CONTEXT.with(|slot| {
        *slot.borrow_mut() = Some(LiveLayoutContext {
            hwnd: state.hwnd,
            webtag_key: state.webtag_key.clone(),
            controller: state.controller.clone(),
        });
    });
}

pub(crate) fn clear_live_layout_context() {
    LIVE_LAYOUT_CONTEXT.with(|slot| {
        slot.borrow_mut().take();
    });
}

/// Lays out the window owned by this UI thread directly from the window
/// procedure: syncs the WebView2 controller bounds, notifies the controller
/// of the new window position, re-arranges attached group windows, and
/// stores the placement. Coalesced per message (no timers); cheap enough to
/// run on every `WM_WINDOWPOSCHANGED` step of an interactive drag.
pub(crate) fn handle_window_geometry_change(hwnd: HWND) {
    let context = LIVE_LAYOUT_CONTEXT.with(|slot| {
        slot.borrow()
            .as_ref()
            .filter(|context| context.hwnd == hwnd)
            .map(|context| (context.webtag_key.clone(), context.controller.clone()))
    });
    let Some((webtag_key, controller)) = context else {
        return;
    };
    let _ = sync_controller_bounds_for(hwnd, &webtag_key, &controller);
    unsafe {
        let _ = controller.NotifyParentWindowPositionChanged();
    }
    layout_group_for_main_host(&webtag_key);
    store_window_placement(hwnd, &webtag_key);
}

pub(crate) fn hwnd_handle(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

pub(crate) fn hwnd_from_handle(handle: isize) -> HWND {
    HWND(handle as *mut c_void)
}

pub(crate) fn is_window_handle_valid(handle: isize) -> bool {
    unsafe { WindowsAndMessaging::IsWindow(Some(hwnd_from_handle(handle))).as_bool() }
}

pub(crate) fn store_current_window_placement(state: &UiState) {
    store_window_placement(state.hwnd, &state.webtag_key);
}

pub(crate) fn store_window_placement(hwnd: HWND, webtag_key: &str) {
    if matches!(
        window_attachment(webtag_key).map(|attachment| attachment.kind),
        Some(WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. })
    ) {
        return;
    }
    if unsafe { WindowsAndMessaging::IsZoomed(hwnd).as_bool() } {
        return;
    }
    let mut rect = RECT::default();
    if !unsafe { WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() }
        || unsafe { WindowsAndMessaging::GetWindowRect(hwnd, &mut rect) }.is_err()
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
            webtag_group_key(webtag_key),
            WindowPlacement {
                left: rect.left,
                top: rect.top,
                width,
                height,
            },
        );
    }
}

pub(crate) fn attach_child_window_to_host(child: HWND, host: HWND) {
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

pub(crate) fn show_shell_host(group_key: &str, host: HWND, title: &str, activate: bool) {
    let host_visible = unsafe { WindowsAndMessaging::IsWindowVisible(host).as_bool() };
    let host_zoomed = unsafe { WindowsAndMessaging::IsZoomed(host).as_bool() };
    if !host_visible
        && !host_zoomed
        && let Some(placement) = current_group_window_placement_for_group(group_key)
    {
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

    fit_window_to_work_area(host);

    // With custom chrome the renderer paints the title area itself; plain
    // OS-frame windows keep the real title and title-bar icon.
    let custom_chrome = windows_chrome_renderer().is_some();
    let title = to_wide(if custom_chrome { "" } else { title });
    unsafe {
        if custom_chrome {
            hide_titlebar_icon(host);
        }
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

pub(crate) fn monitor_info_for_window(hwnd: HWND) -> Option<MONITORINFO> {
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        rcMonitor: RECT::default(),
        rcWork: RECT::default(),
        dwFlags: 0,
    };
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            Some(info)
        } else {
            None
        }
    }
}

pub(crate) fn apply_window_maximized_bounds(hwnd: HWND, lparam: LPARAM) {
    if lparam.0 == 0 {
        return;
    }
    let Some(info) = monitor_info_for_window(hwnd) else {
        return;
    };
    let work = info.rcWork;
    let monitor = info.rcMonitor;
    unsafe {
        let minmax = &mut *(lparam.0 as *mut MINMAXINFO);
        minmax.ptMaxPosition.x = work.left - monitor.left;
        minmax.ptMaxPosition.y = work.top - monitor.top;
        minmax.ptMaxSize.x = rect_width(&work);
        minmax.ptMaxSize.y = rect_height(&work);
    }
}

pub(crate) fn fit_window_to_work_area(hwnd: HWND) {
    unsafe {
        if WindowsAndMessaging::IsZoomed(hwnd).as_bool() {
            return;
        }
    }
    let Some(info) = monitor_info_for_window(hwnd) else {
        return;
    };
    let mut rect = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetWindowRect(hwnd, &mut rect).is_err() {
            return;
        }
    }

    let work = info.rcWork;
    let work_width = rect_width(&work);
    let work_height = rect_height(&work);
    if work_width <= 0 || work_height <= 0 {
        return;
    }

    let min_width = 320.min(work_width);
    let min_height = 240.min(work_height);
    let width = rect_width(&rect).clamp(min_width, work_width);
    let height = rect_height(&rect).clamp(min_height, work_height);
    let max_left = work.right - width;
    let max_top = work.bottom - height;
    let left = rect.left.clamp(work.left, max_left.max(work.left));
    let top = rect.top.clamp(work.top, max_top.max(work.top));

    if left == rect.left
        && top == rect.top
        && width == rect_width(&rect)
        && height == rect_height(&rect)
    {
        return;
    }

    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd,
            None,
            left,
            top,
            width,
            height,
            WindowsAndMessaging::SWP_NOZORDER | WindowsAndMessaging::SWP_NOACTIVATE,
        );
    }
}

pub(crate) fn current_group_window_placement_for_group(group_key: &str) -> Option<WindowPlacement> {
    WINDOW_GROUP_PLACEMENTS
        .get()
        .and_then(|placements| placements.lock().ok())
        .and_then(|placements| placements.get(group_key).copied())
}

pub(crate) fn set_attached_window_rect(hwnd: HWND, rect: RECT, visible: bool) {
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
    let radius = renderer_panel_radius();
    apply_round_region_to_window(hwnd, width, height, radius);
    apply_round_region_to_webview_children(
        hwnd,
        RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        },
        radius,
    );
}

pub(crate) fn hide_attached_window(hwnd: HWND) {
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

pub(crate) struct ChildRegionState {
    target: RECT,
    radius: i32,
}

pub(crate) unsafe extern "system" fn apply_child_region_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let state = unsafe { &mut *(lparam.0 as *mut ChildRegionState) };
    let mut rect = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetWindowRect(hwnd, &mut rect).is_err() {
            return BOOL(1);
        }
    }
    if rects_match_with_tolerance(&rect, &state.target, 2) {
        apply_round_region_to_window(hwnd, rect_width(&rect), rect_height(&rect), state.radius);
    }
    BOOL(1)
}

pub(crate) fn apply_round_region_to_webview_children(parent: HWND, client_rect: RECT, radius: i32) {
    let width = rect_width(&client_rect);
    let height = rect_height(&client_rect);
    if width <= 0 || height <= 0 {
        return;
    }

    let mut top_left = POINT {
        x: client_rect.left,
        y: client_rect.top,
    };
    let mut bottom_right = POINT {
        x: client_rect.right,
        y: client_rect.bottom,
    };
    unsafe {
        let _ = ClientToScreen(parent, &mut top_left);
        let _ = ClientToScreen(parent, &mut bottom_right);
    }
    let mut state = ChildRegionState {
        target: RECT {
            left: top_left.x,
            top: top_left.y,
            right: bottom_right.x,
            bottom: bottom_right.y,
        },
        radius,
    };
    unsafe {
        let _ = WindowsAndMessaging::EnumChildWindows(
            Some(parent),
            Some(apply_child_region_proc),
            LPARAM((&mut state as *mut ChildRegionState) as isize),
        );
    }
}

pub(crate) fn rects_match_with_tolerance(a: &RECT, b: &RECT, tolerance: i32) -> bool {
    (a.left - b.left).abs() <= tolerance
        && (a.top - b.top).abs() <= tolerance
        && (a.right - b.right).abs() <= tolerance
        && (a.bottom - b.bottom).abs() <= tolerance
}

/// Clips an attached (child) window to a rounded rect. Only ever applied to
/// child surfaces (attached cards/panels and their WebView2 children): on a
/// top-level window a region would disable DWM corner rounding and the drop
/// shadow — top-level windows use [`apply_round_corner_preference`] instead.
pub(crate) fn apply_round_region_to_window(hwnd: HWND, width: i32, height: i32, radius: i32) {
    if width <= 0 || height <= 0 || radius <= 0 {
        return;
    }
    unsafe {
        let diameter = (radius * 2).max(1);
        let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, diameter, diameter);
        if region.0.is_null() {
            return;
        }
        if SetWindowRgn(hwnd, Some(region), true) == 0 {
            let _ = DeleteObject(HGDIOBJ(region.0));
        }
    }
}

pub(crate) fn create_hidden_window(webtag: &WebTag) -> StdResult<HWND> {
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
        } else if msg == WindowsAndMessaging::WM_GETMINMAXINFO {
            // Custom-chrome (borderless) windows compute maximized bounds
            // themselves; plain OS-frame windows use default handling.
            if windows_chrome_renderer().is_some() {
                apply_window_maximized_bounds(hwnd, lparam);
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_NCCALCSIZE {
            if windows_chrome_renderer().is_some() {
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_NCHITTEST {
            if windows_chrome_renderer().is_some() {
                let raw = unsafe {
                    WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
                } as *mut WindowUserData;
                if !raw.is_null() {
                    let result =
                        handle_window_frame_hit_test(hwnd, unsafe { &(*raw).webtag_key }, lparam);
                    return LRESULT(result as isize);
                }
            }
        } else if msg == WindowsAndMessaging::WM_ERASEBKGND {
            if windows_chrome_renderer().is_some() {
                return LRESULT(1);
            }
        } else if msg == WindowsAndMessaging::WM_WINDOWPOSCHANGED {
            // Interactive move/size runs inside DefWindowProc's modal loop
            // where the command queue and posted thread messages are starved,
            // so layout must track the drag live from this message path.
            let pos = lparam.0 as *const WindowsAndMessaging::WINDOWPOS;
            if !pos.is_null() {
                let flags = unsafe { (*pos).flags };
                let sized = !flags.contains(WindowsAndMessaging::SWP_NOSIZE)
                    || flags.contains(WindowsAndMessaging::SWP_FRAMECHANGED);
                let moved = !flags.contains(WindowsAndMessaging::SWP_NOMOVE);
                if sized || moved {
                    handle_window_geometry_change(hwnd);
                }
                if sized && windows_chrome_renderer().is_some() {
                    // Chrome elements are anchored to the client edges, so a
                    // size change must repaint the whole window, not just the
                    // newly exposed strip.
                    unsafe {
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
            }
            // Fall through so DefWindowProc still produces WM_SIZE/WM_MOVE.
        } else if msg == WM_LINGXIA_LAYOUT {
            handle_window_geometry_change(hwnd);
            return LRESULT(0);
        } else if msg == WindowsAndMessaging::WM_PAINT {
            if windows_chrome_renderer().is_some() {
                paint_window_chrome(hwnd);
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_CHAR {
            if handle_native_panel_char(wparam) {
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_KEYDOWN {
            if handle_native_panel_keydown(wparam) {
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_LBUTTONDOWN {
            let raw = unsafe {
                WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
            } as *mut WindowUserData;
            if !raw.is_null() {
                let webtag_key = unsafe { &(*raw).webtag_key };
                let point = lparam_to_point(lparam);
                if handle_window_chrome_mouse_down(hwnd, webtag_key, point)
                    || handle_frame_button_mouse_down(hwnd, webtag_key, point)
                {
                    return LRESULT(0);
                }
            }
        } else if msg == WindowsAndMessaging::WM_MOUSEMOVE {
            handle_frame_button_client_mouse_move(hwnd, lparam_to_point(lparam));
            if handle_window_chrome_mouse_move(hwnd, lparam_to_point(lparam)) {
                return LRESULT(0);
            }
        } else if msg == WM_MOUSELEAVE {
            handle_frame_button_client_mouse_leave(hwnd);
        } else if msg == WindowsAndMessaging::WM_NCMOUSEMOVE {
            handle_frame_button_nc_mouse_move(hwnd, wparam.0 as u32);
        } else if msg == WindowsAndMessaging::WM_NCMOUSELEAVE {
            handle_frame_button_nc_mouse_leave(hwnd);
        } else if msg == WindowsAndMessaging::WM_NCLBUTTONDOWN
            || msg == WindowsAndMessaging::WM_NCLBUTTONDBLCLK
            || msg == WindowsAndMessaging::WM_NCLBUTTONUP
        {
            if handle_frame_button_nc_button(hwnd, msg, wparam.0 as u32) {
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_LBUTTONUP {
            if handle_window_chrome_mouse_up(hwnd) {
                return LRESULT(0);
            }
            let raw = unsafe {
                WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
            } as *mut WindowUserData;
            if !raw.is_null() {
                let webtag_key = unsafe { &(*raw).webtag_key };
                let point = lparam_to_point(lparam);
                if handle_frame_button_mouse_up(hwnd, webtag_key, point)
                    || handle_window_chrome_click(hwnd, webtag_key, point)
                {
                    return LRESULT(0);
                }
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
        let user_data = Box::new(WindowUserData::new(webtag.key().to_string()));
        let user_data_ptr = Box::into_raw(user_data);

        // Both modes keep the WS_OVERLAPPEDWINDOW styles. With a registered
        // chrome renderer the renderer paints the whole frame: the standard
        // styles (WS_THICKFRAME | WS_CAPTION) stay so DWM keeps drawing the
        // drop shadow and Win11 snap keeps working, while the visible frame
        // is removed in WM_NCCALCSIZE (client covers the window) and DWM is
        // extended 1px into the client area after creation. Without a
        // renderer the standard OS frame is left untouched.
        let window_style = WS_OVERLAPPEDWINDOW;
        let result = WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("LingXiaHiddenWebViewHost"),
            w!("LingXiaHiddenWebViewHost"),
            window_style,
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
                if windows_chrome_renderer().is_some() {
                    hide_titlebar_icon(hwnd);
                    extend_frame_into_client_area(hwnd);
                    apply_round_corner_preference(hwnd);
                }
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

/// Standard custom-frame setup: WM_NCCALCSIZE already makes the client area
/// cover the whole window, so extend the DWM frame 1px into the top of the
/// client area to keep the DWM drop shadow (and Win11 rounded corners) on a
/// window without a visible non-client frame, then force WM_NCCALCSIZE so
/// the borderless client area applies immediately.
pub(crate) fn extend_frame_into_client_area(hwnd: HWND) {
    let margins = MARGINS {
        cxLeftWidth: 0,
        cxRightWidth: 0,
        cyTopHeight: 1,
        cyBottomHeight: 0,
    };
    unsafe {
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
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

/// Opts a top-level window into DWM-rounded corners (Win11): unlike a GDI
/// window region, DWM rounding is anti-aliased and keeps the drop shadow.
/// Top-level windows must therefore never get `SetWindowRgn` (a region
/// disables DWM corner rounding); rounded regions are reserved for attached
/// child surfaces where DWM rounding cannot apply.
pub(crate) fn apply_round_corner_preference(hwnd: HWND) {
    let preference = DWMWCP_ROUND;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&preference as *const _) as *const c_void,
            std::mem::size_of_val(&preference) as u32,
        );
    }
}

pub(crate) fn normalize_rect(mut rect: RECT) -> RECT {
    if rect.right < rect.left {
        rect.right = rect.left;
    }
    if rect.bottom < rect.top {
        rect.bottom = rect.top;
    }
    rect
}

pub(crate) fn rect_width(rect: &RECT) -> i32 {
    (rect.right - rect.left).max(0)
}

pub(crate) fn rect_height(rect: &RECT) -> i32 {
    (rect.bottom - rect.top).max(0)
}

pub(crate) fn rect_contains(rect: &RECT, point: (i32, i32)) -> bool {
    point.0 >= rect.left && point.0 < rect.right && point.1 >= rect.top && point.1 < rect.bottom
}

pub(crate) fn lparam_to_point(lparam: LPARAM) -> (i32, i32) {
    let value = lparam.0 as u32;
    let x = (value & 0xffff) as i16 as i32;
    let y = ((value >> 16) & 0xffff) as i16 as i32;
    (x, y)
}

pub(crate) fn lparam_screen_to_client(hwnd: HWND, lparam: LPARAM) -> (i32, i32) {
    let (x, y) = lparam_to_point(lparam);
    let mut point = POINT { x, y };
    unsafe {
        let _ = ScreenToClient(hwnd, &mut point);
    }
    (point.x, point.y)
}

pub(crate) fn handle_window_frame_hit_test(hwnd: HWND, webtag_key: &str, lparam: LPARAM) -> u32 {
    if !window_draws_shell_chrome(webtag_key) {
        return WindowsAndMessaging::HTCLIENT;
    }

    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    let point = lparam_screen_to_client(hwnd, lparam);
    if let Some(hit) = window_resize_hit_test(hwnd, client, point) {
        return hit;
    }

    let Some(renderer) = windows_chrome_renderer() else {
        return WindowsAndMessaging::HTCLIENT;
    };
    let state = chrome_state_for_window(hwnd, webtag_key);
    match renderer.hit_test(&state, point) {
        Some(WindowsChromeHit::Caption) => WindowsAndMessaging::HTCAPTION,
        // Win11 Snap Layouts: the flyout only appears when WM_NCHITTEST
        // reports HTMAXBUTTON over the maximize button. DefWindowProc does
        // not click client-drawn snap buttons, so the click itself is
        // performed in WM_NCLBUTTONDOWN/WM_NCLBUTTONUP. Minimize and close
        // stay HTCLIENT and keep their client-message click handling.
        Some(WindowsChromeHit::FrameButton(WindowsFrameButton::Maximize)) => {
            WindowsAndMessaging::HTMAXBUTTON
        }
        _ => WindowsAndMessaging::HTCLIENT,
    }
}

/// Hover/pressed frame-button state of a window, surfaced to the chrome
/// renderer through [`WindowsChromeState`].
pub(crate) fn frame_button_visual_state(
    hwnd: HWND,
) -> (Option<WindowsFrameButton>, Option<WindowsFrameButton>) {
    with_window_user_data(hwnd, |data| {
        (
            data.hovered_frame_button.get(),
            data.pressed_frame_button.get(),
        )
    })
    .unwrap_or((None, None))
}

/// Invalidates just the rect of one frame button (no full-window flicker on
/// hover changes). Falls back to a full invalidation when the renderer does
/// not expose button rects.
fn invalidate_frame_button(hwnd: HWND, button: WindowsFrameButton) {
    let Some(renderer) = windows_chrome_renderer() else {
        return;
    };
    let Some(webtag_key) = window_webtag_key(hwnd) else {
        return;
    };
    let state = chrome_state_for_window(hwnd, &webtag_key);
    match renderer.frame_button_rect(&state, button) {
        Some(rect) => unsafe {
            let _ = InvalidateRect(Some(hwnd), Some(&rect), false);
        },
        None => unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        },
    }
}

fn set_frame_button_hover(hwnd: HWND, hovered: Option<WindowsFrameButton>) {
    let previous = with_window_user_data(hwnd, |data| data.hovered_frame_button.replace(hovered));
    let Some(previous) = previous else {
        return;
    };
    if previous == hovered {
        return;
    }
    if let Some(button) = previous {
        invalidate_frame_button(hwnd, button);
    }
    if let Some(button) = hovered {
        invalidate_frame_button(hwnd, button);
    }
}

fn set_frame_button_pressed(hwnd: HWND, pressed: Option<WindowsFrameButton>) {
    let previous = with_window_user_data(hwnd, |data| data.pressed_frame_button.replace(pressed));
    let Some(previous) = previous else {
        return;
    };
    if previous == pressed {
        return;
    }
    if let Some(button) = previous {
        invalidate_frame_button(hwnd, button);
    }
    if let Some(button) = pressed {
        invalidate_frame_button(hwnd, button);
    }
}

/// Arms `TrackMouseEvent` so the window receives WM_MOUSELEAVE (client) or
/// WM_NCMOUSELEAVE (non-client) once, deduplicated per window via the
/// tracking flags in [`WindowUserData`].
fn begin_mouse_tracking(hwnd: HWND, nonclient: bool) {
    let already = with_window_user_data(hwnd, |data| {
        if nonclient {
            data.tracking_nc_mouse.replace(true)
        } else {
            data.tracking_client_mouse.replace(true)
        }
    })
    .unwrap_or(true);
    if already {
        return;
    }
    let mut track = TRACKMOUSEEVENT {
        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: if nonclient {
            TME_LEAVE | TME_NONCLIENT
        } else {
            TME_LEAVE
        },
        hwndTrack: hwnd,
        dwHoverTime: 0,
    };
    unsafe {
        let _ = TrackMouseEvent(&mut track);
    }
}

/// Frame-button element under a client-space point, or `None`.
fn frame_button_at_point(
    hwnd: HWND,
    webtag_key: &str,
    point: (i32, i32),
) -> Option<WindowsFrameButton> {
    if !window_draws_shell_chrome(webtag_key) {
        return None;
    }
    let renderer = windows_chrome_renderer()?;
    let state = chrome_state_for_window(hwnd, webtag_key);
    match renderer.hit_test(&state, point) {
        Some(WindowsChromeHit::FrameButton(button)) => Some(button),
        _ => None,
    }
}

/// WM_MOUSEMOVE path: tracks hover for the client-handled frame buttons
/// (minimize/close; the maximize button lives in non-client space).
pub(crate) fn handle_frame_button_client_mouse_move(hwnd: HWND, point: (i32, i32)) {
    if windows_chrome_renderer().is_none() {
        return;
    }
    let Some(webtag_key) = window_webtag_key(hwnd) else {
        return;
    };
    let hovered = frame_button_at_point(hwnd, &webtag_key, point);
    set_frame_button_hover(hwnd, hovered);
    if hovered.is_some() {
        begin_mouse_tracking(hwnd, false);
    }
}

/// WM_NCMOUSEMOVE path: the maximize button reports HTMAXBUTTON from
/// WM_NCHITTEST, so its hover updates arrive as non-client mouse moves.
pub(crate) fn handle_frame_button_nc_mouse_move(hwnd: HWND, hit_code: u32) {
    if windows_chrome_renderer().is_none() {
        return;
    }
    if hit_code == WindowsAndMessaging::HTMAXBUTTON {
        set_frame_button_hover(hwnd, Some(WindowsFrameButton::Maximize));
        begin_mouse_tracking(hwnd, true);
    } else if frame_button_visual_state(hwnd).0 == Some(WindowsFrameButton::Maximize) {
        set_frame_button_hover(hwnd, None);
    }
}

/// WM_MOUSELEAVE: clears hover for client-tracked buttons only; the maximize
/// button is cleared by WM_NCMOUSELEAVE (the cursor moving from a client
/// button onto the maximize button produces WM_MOUSELEAVE after the
/// non-client move already set the new hover).
pub(crate) fn handle_frame_button_client_mouse_leave(hwnd: HWND) {
    with_window_user_data(hwnd, |data| data.tracking_client_mouse.set(false));
    if frame_button_visual_state(hwnd).0 != Some(WindowsFrameButton::Maximize) {
        set_frame_button_hover(hwnd, None);
    }
}

/// WM_NCMOUSELEAVE: clears maximize-button hover/pressed state.
pub(crate) fn handle_frame_button_nc_mouse_leave(hwnd: HWND) {
    with_window_user_data(hwnd, |data| data.tracking_nc_mouse.set(false));
    let (hovered, pressed) = frame_button_visual_state(hwnd);
    if hovered == Some(WindowsFrameButton::Maximize) {
        set_frame_button_hover(hwnd, None);
    }
    if pressed == Some(WindowsFrameButton::Maximize) {
        set_frame_button_pressed(hwnd, None);
    }
}

/// WM_LBUTTONDOWN on a client-handled frame button: records the pressed
/// state for painting and captures the mouse so the release is seen even
/// when it happens outside the button.
pub(crate) fn handle_frame_button_mouse_down(
    hwnd: HWND,
    webtag_key: &str,
    point: (i32, i32),
) -> bool {
    let Some(button) = frame_button_at_point(hwnd, webtag_key, point) else {
        return false;
    };
    set_frame_button_pressed(hwnd, Some(button));
    unsafe {
        let _ = SetCapture(hwnd);
    }
    true
}

/// WM_LBUTTONUP with a pressed frame button: executes the button only when
/// the release still lands on it (standard button-cancel semantics).
pub(crate) fn handle_frame_button_mouse_up(
    hwnd: HWND,
    webtag_key: &str,
    point: (i32, i32),
) -> bool {
    let (_, pressed) = frame_button_visual_state(hwnd);
    let Some(button) = pressed else {
        return false;
    };
    set_frame_button_pressed(hwnd, None);
    unsafe {
        let _ = ReleaseCapture();
    }
    if frame_button_at_point(hwnd, webtag_key, point) == Some(button) {
        handle_window_frame_button(hwnd, button);
    }
    true
}

/// WM_NCLBUTTONDOWN/WM_NCLBUTTONUP for HTMAXBUTTON: DefWindowProc does not
/// click client-drawn snap buttons, so the maximize/restore click is
/// performed here. Returns `true` when the message was consumed.
pub(crate) fn handle_frame_button_nc_button(hwnd: HWND, msg: u32, hit_code: u32) -> bool {
    if windows_chrome_renderer().is_none() {
        return false;
    }
    match msg {
        WindowsAndMessaging::WM_NCLBUTTONDOWN | WindowsAndMessaging::WM_NCLBUTTONDBLCLK => {
            if hit_code != WindowsAndMessaging::HTMAXBUTTON {
                return false;
            }
            set_frame_button_pressed(hwnd, Some(WindowsFrameButton::Maximize));
            true
        }
        WindowsAndMessaging::WM_NCLBUTTONUP => {
            if frame_button_visual_state(hwnd).1 != Some(WindowsFrameButton::Maximize) {
                return false;
            }
            set_frame_button_pressed(hwnd, None);
            if hit_code == WindowsAndMessaging::HTMAXBUTTON {
                handle_window_frame_button(hwnd, WindowsFrameButton::Maximize);
            }
            true
        }
        _ => false,
    }
}

pub(crate) fn window_resize_hit_test(hwnd: HWND, client: RECT, point: (i32, i32)) -> Option<u32> {
    if unsafe { WindowsAndMessaging::IsZoomed(hwnd).as_bool() } {
        return None;
    }
    let border = resize_border_thickness();
    let left = point.0 >= client.left && point.0 < client.left + border;
    let right = point.0 < client.right && point.0 >= client.right - border;
    let top = point.1 >= client.top && point.1 < client.top + border;
    let bottom = point.1 < client.bottom && point.1 >= client.bottom - border;

    match (left, right, top, bottom) {
        (true, _, true, _) => Some(WindowsAndMessaging::HTTOPLEFT),
        (_, true, true, _) => Some(WindowsAndMessaging::HTTOPRIGHT),
        (true, _, _, true) => Some(WindowsAndMessaging::HTBOTTOMLEFT),
        (_, true, _, true) => Some(WindowsAndMessaging::HTBOTTOMRIGHT),
        (_, _, true, _) => Some(WindowsAndMessaging::HTTOP),
        (_, _, _, true) => Some(WindowsAndMessaging::HTBOTTOM),
        (true, _, _, _) => Some(WindowsAndMessaging::HTLEFT),
        (_, true, _, _) => Some(WindowsAndMessaging::HTRIGHT),
        _ => None,
    }
}

pub(crate) fn resize_border_thickness() -> i32 {
    unsafe {
        let frame = WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CXFRAME);
        let padded = WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CXPADDEDBORDER);
        (frame + padded).max(6)
    }
}

pub(crate) fn window_draws_shell_chrome(webtag_key: &str) -> bool {
    !matches!(
        window_attachment(webtag_key).map(|attachment| attachment.kind),
        Some(WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. })
    )
}

pub(crate) fn window_webtag_key(hwnd: HWND) -> Option<String> {
    let raw =
        unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA) }
            as *mut WindowUserData;
    if raw.is_null() {
        None
    } else {
        Some(unsafe { (*raw).webtag_key.clone() })
    }
}

pub(crate) fn handle_window_frame_button(hwnd: HWND, button: WindowsFrameButton) {
    unsafe {
        match button {
            WindowsFrameButton::Minimize => {
                let _ = WindowsAndMessaging::ShowWindow(hwnd, WindowsAndMessaging::SW_MINIMIZE);
            }
            WindowsFrameButton::Maximize => {
                let cmd = if WindowsAndMessaging::IsZoomed(hwnd).as_bool() {
                    WindowsAndMessaging::SW_RESTORE
                } else {
                    WindowsAndMessaging::SW_MAXIMIZE
                };
                let _ = WindowsAndMessaging::ShowWindow(hwnd, cmd);
            }
            WindowsFrameButton::Close => {
                let _ = WindowsAndMessaging::SendMessageW(
                    hwnd,
                    WindowsAndMessaging::WM_CLOSE,
                    None,
                    None,
                );
            }
        }
    }
}

pub(crate) fn handle_window_chrome_click(hwnd: HWND, webtag_key: &str, point: (i32, i32)) -> bool {
    if !window_draws_shell_chrome(webtag_key) {
        return false;
    }
    let Some(renderer) = windows_chrome_renderer() else {
        return false;
    };
    let state = chrome_state_for_window(hwnd, webtag_key);

    match renderer.hit_test(&state, point) {
        Some(WindowsChromeHit::FrameButton(button)) => {
            handle_window_frame_button(hwnd, button);
            true
        }
        Some(WindowsChromeHit::NativePanel { panel_id }) => {
            set_active_native_panel(Some(panel_id));
            unsafe {
                let _ = SetFocus(Some(hwnd));
            }
            true
        }
        Some(WindowsChromeHit::NavigationBack) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::NavigationBack)
        }
        Some(WindowsChromeHit::NavigationHome) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::NavigationHome)
        }
        Some(WindowsChromeHit::PanelActivator { panel_id }) => invoke_chrome_event_handler(
            webtag_key,
            WindowsChromeEvent::PanelActivatorClick { panel_id },
        ),
        Some(WindowsChromeHit::TabBarItem { index }) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::TabBarClick { index })
        }
        Some(WindowsChromeHit::Chrome) => true,
        // Caption points never arrive as client clicks (WM_NCHITTEST maps
        // them to HTCAPTION first); treat defensively as unhandled.
        Some(WindowsChromeHit::Caption) | None => false,
    }
}

pub(crate) fn show_native_window(
    state: &mut UiState,
    title: &str,
    activate: bool,
    role: WindowsWindowRole,
) -> StdResult<()> {
    match role {
        WindowsWindowRole::Main => show_native_main_window(state, title, activate),
        WindowsWindowRole::Panel { panel_id } => show_native_panel_window(state, &panel_id),
    }
}

pub(crate) fn show_native_main_window(
    state: &mut UiState,
    title: &str,
    activate: bool,
) -> StdResult<()> {
    let (group_key, host, is_host) = ensure_main_attachment(state);
    set_active_group(&group_key);
    set_group_active_main(&group_key, &state.webtag_key);

    if is_host {
        show_shell_host(&group_key, host, title, activate);
        sync_controller_bounds(state)?;
        layout_group_windows(&group_key);
        set_controller_visible(state, true)?;
    } else {
        attach_child_window_to_host(state.hwnd, host);
        show_shell_host(&group_key, host, title, activate);
        layout_group_windows(&group_key);
        sync_controller_bounds(state)?;
        set_controller_visible(state, true)?;
    }

    request_group_shell_refresh(&group_key);
    state.window_visible = true;
    store_current_window_placement(state);
    Ok(())
}

pub(crate) fn show_native_panel_window(state: &mut UiState, panel_id: &str) -> StdResult<()> {
    register_window_handle(&state.webtag_key, state.hwnd);
    let group_key = active_group_key().unwrap_or_else(|| webtag_group_key(&state.webtag_key));
    let Some(host) = host_handle_for_group(&group_key) else {
        return show_native_main_window(state, "", true);
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
            native_kind: NativePanelKind::Text,
            native_title: None,
            native_body: None,
        },
    );
    set_controller_visible(state, true)?;
    layout_group_windows(&group_key);
    request_group_shell_refresh(&group_key);
    state.window_visible = true;
    Ok(())
}

pub(crate) fn hide_native_window(state: &mut UiState) -> StdResult<()> {
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

pub(crate) fn hide_native_main_host_window(state: &mut UiState) -> StdResult<()> {
    let group_key = layout_group_key_for_webtag(&state.webtag_key);
    if group_active_main(&group_key).as_deref() != Some(state.webtag_key.as_str()) {
        set_controller_visible(state, false)?;
        state.window_visible = false;
        return Ok(());
    }
    hide_detached_window(state)
}

pub(crate) fn hide_detached_window(state: &mut UiState) -> StdResult<()> {
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

pub(crate) fn set_controller_visible(state: &UiState, visible: bool) -> StdResult<()> {
    unsafe {
        state
            .controller
            .SetIsVisible(visible)
            .map_err(|err| WebViewError::WebView(format!("SetIsVisible failed: {err}")))?;
    }
    Ok(())
}

pub(crate) fn set_native_window_layout(
    state: &UiState,
    layout: WindowsWindowLayout,
) -> StdResult<()> {
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

pub(crate) fn sync_controller_bounds(state: &UiState) -> StdResult<()> {
    sync_controller_bounds_for(state.hwnd, &state.webtag_key, &state.controller)
}

pub(crate) fn sync_controller_bounds_for(
    hwnd: HWND,
    webtag_key: &str,
    controller: &ICoreWebView2Controller,
) -> StdResult<()> {
    let mut rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
        if rect.right <= rect.left || rect.bottom <= rect.top {
            rect = RECT {
                left: 0,
                top: 0,
                right: 1024,
                bottom: 768,
            };
        }
        rect = controller_bounds_for_window(hwnd, webtag_key, rect);
        controller
            .SetBounds(rect)
            .map_err(|err| WebViewError::WebView(format!("SetBounds failed: {err}")))?;
        if window_attachment(webtag_key).is_some() {
            apply_round_region_to_webview_children(hwnd, rect, renderer_panel_radius());
        }
    }
    Ok(())
}

pub(crate) fn controller_bounds_for_window(hwnd: HWND, webtag_key: &str, client: RECT) -> RECT {
    match window_attachment(webtag_key) {
        Some(WindowAttachment {
            kind: WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. },
            ..
        }) => normalize_rect(client),
        Some(WindowAttachment {
            group_key,
            kind: WindowAttachmentKind::MainHost,
        }) => {
            let content = renderer_content_rect(client, &current_window_layout(webtag_key));
            attached_group_rects(&group_key, hwnd)
                .map(|rects| rects.main)
                .unwrap_or(content)
        }
        None => renderer_content_rect(client, &current_window_layout(webtag_key)),
    }
}

pub(crate) fn window_snapshot(state: &UiState) -> StdResult<WindowsWebViewWindowSnapshot> {
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
            .map_err(|err| WebViewError::WebView(format!("GetClientRect failed: {err}")))?;
        if !ClientToScreen(state.hwnd, &mut client_origin).as_bool() {
            return Err(WebViewError::WebView("ClientToScreen failed".to_string()));
        }
    }

    let content = controller_bounds_for_window(state.hwnd, &state.webtag_key, client_rect);
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

pub(crate) fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
