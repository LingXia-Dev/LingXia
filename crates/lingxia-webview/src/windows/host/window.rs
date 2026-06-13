//! Native window management: creation, window proc, hit testing,
//! show/hide flows, bounds syncing, and placement persistence.

use super::*;

mod chrome;
mod controller_bounds;
mod corner_caps;
mod geometry;
mod placement;

mod visibility;
pub(crate) use chrome::*;
pub(crate) use controller_bounds::*;
pub(crate) use geometry::*;
pub(crate) use placement::*;

pub use corner_caps::{WindowsCardDecorator, set_windows_card_decorator};
pub(crate) use corner_caps::{destroy_corner_caps, raise_corner_caps, update_corner_caps};
pub(crate) use visibility::*;

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
            if windows_chrome_renderer().is_some() && !window_uses_os_frame(hwnd) {
                apply_window_maximized_bounds(hwnd, lparam);
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_NCCALCSIZE {
            if windows_chrome_renderer().is_some() && !window_uses_os_frame(hwnd) {
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_NCHITTEST {
            if windows_chrome_renderer().is_some() && !window_uses_os_frame(hwnd) {
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
            if windows_chrome_renderer().is_some() && !window_uses_os_frame(hwnd) {
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
        } else if msg == WindowsAndMessaging::WM_ENTERSIZEMOVE {
            // WM_WINDOWPOSCHANGED is coalesced inside DefWindowProc's modal
            // move/size loop, so a real mouse drag can outrun the layout;
            // timers still fire inside that loop and keep layout pumping.
            unsafe {
                let _ = WindowsAndMessaging::SetTimer(
                    Some(hwnd),
                    SIZEMOVE_TIMER_ID,
                    SIZEMOVE_TIMER_INTERVAL_MS,
                    None,
                );
            }
        } else if msg == WindowsAndMessaging::WM_EXITSIZEMOVE {
            unsafe {
                let _ = WindowsAndMessaging::KillTimer(Some(hwnd), SIZEMOVE_TIMER_ID);
            }
            handle_window_geometry_change(hwnd);
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
        } else if msg == WindowsAndMessaging::WM_TIMER {
            if wparam.0 == SIZEMOVE_TIMER_ID {
                handle_live_sizemove_tick(hwnd);
                return LRESULT(0);
            }
        } else if msg == WM_LINGXIA_LAYOUT {
            handle_window_geometry_change(hwnd);
            return LRESULT(0);
        } else if msg == WM_LINGXIA_RUN_CALLBACK {
            // Closure marshalled from another thread via
            // `post_to_window_thread` for host integrations that must live
            // on this UI thread.
            run_posted_window_callback(wparam);
            return LRESULT(0);
        } else if msg == WindowsAndMessaging::WM_PAINT {
            if windows_chrome_renderer().is_some() && !window_uses_os_frame(hwnd) {
                paint_window_chrome(hwnd);
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_CHAR {
            if handle_host_panel_char(wparam) {
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_KEYDOWN {
            if handle_host_panel_keydown(wparam) {
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_LBUTTONDOWN
            || msg == WindowsAndMessaging::WM_LBUTTONDBLCLK
        {
            // CS_DBLCLKS turns the second press of a double-click into
            // WM_LBUTTONDBLCLK; renderer-defined hits can opt into a
            // separate double-click command, while everything else keeps
            // plain button-down handling.
            let raw = unsafe {
                WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
            } as *mut WindowUserData;
            if !raw.is_null() {
                let webtag_key = unsafe { &(*raw).webtag_key };
                let point = lparam_to_point(lparam);
                if msg == WindowsAndMessaging::WM_LBUTTONDBLCLK
                    && handle_window_chrome_double_click(hwnd, webtag_key, point)
                {
                    return LRESULT(0);
                }
                if handle_window_chrome_mouse_down(hwnd, webtag_key, point)
                    || handle_frame_button_mouse_down(hwnd, webtag_key, point)
                {
                    return LRESULT(0);
                }
            }
        } else if msg == WindowsAndMessaging::WM_RBUTTONDOWN {
            let raw = unsafe {
                WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
            } as *mut WindowUserData;
            if !raw.is_null() {
                let webtag_key = unsafe { &(*raw).webtag_key };
                if handle_window_chrome_right_click(hwnd, webtag_key, lparam_to_point(lparam)) {
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

    let class = WNDCLASSW {
        // CS_HREDRAW | CS_VREDRAW: a resize invalidates the whole window,
        // not just the exposed strip; the chrome is anchored to all client
        // edges, and stale strips would otherwise linger during live drags.
        // CS_DBLCLKS: renderer-defined hits may emit double-click commands.
        style: WindowsAndMessaging::CS_HREDRAW
            | WindowsAndMessaging::CS_VREDRAW
            | WindowsAndMessaging::CS_DBLCLKS,
        lpfnWndProc: Some(window_proc),
        // Without an explicit class cursor the window never resets the pointer,
        // so the launch-time "AppStarting" busy spinner lingers over it. Use the
        // standard arrow.
        hCursor: unsafe { WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_ARROW) }
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
        let (default_width, default_height) = default_window_size();
        let result = WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("LingXiaHiddenWebViewHost"),
            w!("LingXiaHiddenWebViewHost"),
            window_style,
            WindowsAndMessaging::CW_USEDEFAULT,
            WindowsAndMessaging::CW_USEDEFAULT,
            default_width,
            default_height,
            None,
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            Some(user_data_ptr.cast()),
        );

        match result {
            Ok(hwnd) => {
                invoke_host_window_created_handler(hwnd);
                // OS-frame webviews (standalone window surfaces) keep the plain
                // WS_OVERLAPPEDWINDOW frame so the native title bar and
                // minimize/maximize/close buttons render; only custom-chrome
                // windows get the borderless DWM extension.
                if windows_chrome_renderer().is_some() && !webtag_uses_os_frame(webtag.key()) {
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
