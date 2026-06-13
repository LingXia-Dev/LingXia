//! HWND, RECT, and DWM geometry helpers for host windows.

use super::*;

pub(crate) fn hwnd_handle(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

pub(crate) fn hwnd_from_handle(handle: isize) -> HWND {
    HWND(handle as *mut c_void)
}

pub(crate) fn is_window_handle_valid(handle: isize) -> bool {
    unsafe { WindowsAndMessaging::IsWindow(Some(hwnd_from_handle(handle))).as_bool() }
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

pub(crate) fn set_attached_window_rect(hwnd: HWND, rect: RECT, visible: bool) {
    let width = rect_width(&rect);
    let height = rect_height(&rect);
    if width == 0 || height == 0 || !visible {
        hide_attached_window(hwnd);
        return;
    }
    unsafe {
        // SWP_NOCOPYBITS: during live resizes the old surface contents must
        // not be blitted into the new position (stale-content ghosting);
        // the webview repaints the full card anyway.
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd,
            Some(WindowsAndMessaging::HWND_TOP),
            rect.left,
            rect.top,
            width,
            height,
            WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_NOOWNERZORDER
                | WindowsAndMessaging::SWP_NOCOPYBITS
                | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
    }
    update_corner_caps(
        hwnd,
        RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        },
    );
}

pub(crate) fn hide_attached_window(hwnd: HWND) {
    destroy_corner_caps(hwnd);
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
/// disables DWM corner rounding); attached child surfaces, where DWM
/// rounding cannot apply, are rounded visually by the corner-cap overlays
/// instead (see [`update_corner_caps`]).
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

pub(crate) fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
