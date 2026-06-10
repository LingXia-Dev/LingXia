//! Native window management: creation, window proc, hit testing,
//! show/hide flows, bounds syncing, and placement persistence.

use super::*;

pub(crate) struct WindowUserData {
    webtag_key: String,
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
    if matches!(
        window_attachment(&state.webtag_key).map(|attachment| attachment.kind),
        Some(WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. })
    ) {
        return;
    }
    if unsafe { WindowsAndMessaging::IsZoomed(state.hwnd).as_bool() } {
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
            if !raw.is_null()
                && handle_window_chrome_mouse_down(
                    hwnd,
                    unsafe { &(*raw).webtag_key },
                    lparam_to_point(lparam),
                )
            {
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_MOUSEMOVE {
            if handle_window_chrome_mouse_move(hwnd, lparam_to_point(lparam)) {
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_LBUTTONUP {
            if handle_window_chrome_mouse_up(hwnd) {
                return LRESULT(0);
            }
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

        // With a registered chrome renderer the window is borderless and the
        // renderer paints the whole frame; otherwise keep the standard OS
        // frame so windows stay usable without any product chrome.
        let window_style = if windows_chrome_renderer().is_some() {
            WINDOW_STYLE(
                WindowsAndMessaging::WS_POPUP.0
                    | WindowsAndMessaging::WS_THICKFRAME.0
                    | WindowsAndMessaging::WS_SYSMENU.0
                    | WindowsAndMessaging::WS_MINIMIZEBOX.0
                    | WindowsAndMessaging::WS_MAXIMIZEBOX.0,
            )
        } else {
            WS_OVERLAPPEDWINDOW
        };
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
        _ => WindowsAndMessaging::HTCLIENT,
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
        if window_attachment(&state.webtag_key).is_some() {
            apply_round_region_to_webview_children(state.hwnd, rect, renderer_panel_radius());
        }
    }
    Ok(())
}

pub(crate) fn controller_bounds_for_state(state: &UiState, client: RECT) -> RECT {
    match window_attachment(&state.webtag_key) {
        Some(WindowAttachment {
            kind: WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. },
            ..
        }) => normalize_rect(client),
        Some(WindowAttachment {
            group_key,
            kind: WindowAttachmentKind::MainHost,
        }) => {
            let content = renderer_content_rect(client, &current_window_layout(&state.webtag_key));
            attached_group_rects(&group_key, state.hwnd)
                .map(|rects| rects.main)
                .unwrap_or(content)
        }
        None => renderer_content_rect(client, &current_window_layout(&state.webtag_key)),
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

pub(crate) fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
