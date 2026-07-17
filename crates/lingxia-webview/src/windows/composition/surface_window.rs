//! The per-webview surface child window hosting the DirectComposition
//! target. A composition-hosted WebView2 has no input HWND of its own, so
//! this window forwards mouse/wheel input, mirrors the engine cursor, and
//! bridges focus; once WebView2 gains focus its hidden input window receives
//! keyboard and IME directly from the OS.

use super::*;
use windows::Win32::Foundation::{LRESULT, POINT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, GA_ROOT, GWLP_USERDATA, HTCLIENT, IDC_ARROW, WNDCLASSW, WS_CHILD,
    WS_CLIPSIBLINGS, WS_EX_NOREDIRECTIONBITMAP,
};

const SURFACE_CLASS: PCWSTR = windows::core::w!("LingXiaWebViewSurface");

// Defined next to TrackMouseEvent's TME_LEAVE, not with the WM_MOUSE* set.
const WM_MOUSELEAVE: u32 = windows::Win32::UI::Controls::WM_MOUSELEAVE;

/// Engine cursor for the surface, `None` when the query fails.
fn controller_cursor(
    controller: &ICoreWebView2CompositionController,
) -> Option<WindowsAndMessaging::HCURSOR> {
    let mut cursor = WindowsAndMessaging::HCURSOR::default();
    unsafe { controller.Cursor(&mut cursor).ok()? };
    Some(cursor)
}

/// Input-forwarding state, boxed into `GWLP_USERDATA` once the composition
/// controller exists. The wndproc runs on the webview's UI thread — the
/// controller's creating thread — so calls need no marshalling.
struct SurfaceInputState {
    env3: ICoreWebView2Environment3,
    controller: ICoreWebView2CompositionController,
    base: ICoreWebView2Controller,
    /// MK_* mask of buttons currently down; capture is held while non-zero
    /// so out-of-bounds drags (text selection) keep streaming.
    buttons_down: u32,
    /// True while a `TME_LEAVE` request is armed.
    tracking_leave: bool,
}

fn ensure_surface_class() -> bool {
    static REGISTERED: OnceLock<bool> = OnceLock::new();
    *REGISTERED.get_or_init(|| unsafe {
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(surface_proc),
            hInstance: GetModuleHandleW(None).map(Into::into).unwrap_or_default(),
            hCursor: WindowsAndMessaging::LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: SURFACE_CLASS,
            ..Default::default()
        };
        WindowsAndMessaging::RegisterClassW(&class) != 0
    })
}

/// Creates the (initially hidden) surface child window on the calling
/// thread — the webview's UI thread, so its messages pump there.
/// `WS_EX_NOREDIRECTIONBITMAP`: the DComp tree is the window's only content,
/// so skip the GDI redirection surface entirely.
pub(crate) fn create_surface_window(parent: HWND, bounds: RECT) -> StdResult<HWND> {
    if !ensure_surface_class() {
        return Err(WebViewError::WebView(
            "RegisterClassW(LingXiaWebViewSurface) failed".to_string(),
        ));
    }
    unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP,
            SURFACE_CLASS,
            PCWSTR::null(),
            WS_CHILD | WS_CLIPSIBLINGS,
            bounds.left,
            bounds.top,
            (bounds.right - bounds.left).max(0),
            (bounds.bottom - bounds.top).max(0),
            Some(parent),
            None,
            GetModuleHandleW(None).map(Into::into).ok(),
            None,
        )
        .map_err(|err| WebViewError::WebView(format!("CreateWindowExW(surface) failed: {err}")))
    }
}

/// Arms input forwarding once the composition controller exists: stashes the
/// state box and subscribes cursor/focus events. Best-effort event wiring —
/// a failed subscription degrades cursor/tab-out polish, not input itself.
pub(crate) fn attach_input(
    hwnd: HWND,
    env3: &ICoreWebView2Environment3,
    controller: &ICoreWebView2CompositionController,
    base: &ICoreWebView2Controller,
) {
    let state = Box::new(SurfaceInputState {
        env3: env3.clone(),
        controller: controller.clone(),
        base: base.clone(),
        buttons_down: 0,
        tracking_leave: false,
    });
    unsafe {
        WindowsAndMessaging::SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
    }

    // Engine cursor changes while the pointer rests over the surface (no
    // WM_SETCURSOR arrives without mouse movement).
    let cursor_hwnd = hwnd;
    let cursor_handler = CursorChangedEventHandler::create(Box::new(move |sender, _args| {
        if let Some(controller) = sender
            && surface_is_hovered(cursor_hwnd)
            && let Some(cursor) = controller_cursor(&controller)
        {
            unsafe { WindowsAndMessaging::SetCursor(Some(cursor)) };
        }
        Ok(())
    }));
    let mut token = 0i64;
    unsafe {
        if let Err(err) = controller.add_CursorChanged(&cursor_handler, &mut token) {
            log::warn!("add_CursorChanged failed: {err}");
        }
    }

    // Tab-out: hand focus back to the top-level host window.
    let focus_hwnd = hwnd;
    let focus_handler = MoveFocusRequestedEventHandler::create(Box::new(move |_sender, args| {
        unsafe {
            let root = WindowsAndMessaging::GetAncestor(focus_hwnd, GA_ROOT);
            if !root.is_invalid() {
                let _ = SetFocus(Some(root));
            }
        }
        if let Some(args) = args {
            unsafe { args.SetHandled(true)? };
        }
        Ok(())
    }));
    let mut token = 0i64;
    unsafe {
        if let Err(err) = base.add_MoveFocusRequested(&focus_handler, &mut token) {
            log::warn!("add_MoveFocusRequested failed: {err}");
        }
    }
}

fn surface_is_hovered(hwnd: HWND) -> bool {
    unsafe {
        let mut point = POINT::default();
        if WindowsAndMessaging::GetCursorPos(&mut point).is_err() {
            return false;
        }
        WindowsAndMessaging::WindowFromPoint(point) == hwnd
            || windows::Win32::UI::Input::KeyboardAndMouse::GetCapture() == hwnd
    }
}

fn input_state<'a>(hwnd: HWND) -> Option<&'a mut SurfaceInputState> {
    let ptr = unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA) }
        as *mut SurfaceInputState;
    unsafe { ptr.as_mut() }
}

fn button_bit(msg: u32, wparam: WPARAM) -> u32 {
    use WindowsAndMessaging as WM;
    match msg {
        WM::WM_LBUTTONDOWN | WM::WM_LBUTTONUP | WM::WM_LBUTTONDBLCLK => 0x0001, // MK_LBUTTON
        WM::WM_RBUTTONDOWN | WM::WM_RBUTTONUP | WM::WM_RBUTTONDBLCLK => 0x0002, // MK_RBUTTON
        WM::WM_MBUTTONDOWN | WM::WM_MBUTTONUP | WM::WM_MBUTTONDBLCLK => 0x0010, // MK_MBUTTON
        WM::WM_XBUTTONDOWN | WM::WM_XBUTTONUP | WM::WM_XBUTTONDBLCLK => {
            if (wparam.0 >> 16) as u16 & 0x0002 != 0 {
                0x0040 // MK_XBUTTON2
            } else {
                0x0020 // MK_XBUTTON1
            }
        }
        _ => 0,
    }
}

unsafe extern "system" fn surface_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    use WindowsAndMessaging as WM;
    match msg {
        // No redirection surface to erase.
        WM::WM_ERASEBKGND => LRESULT(1),
        WM::WM_MOUSEMOVE
        | WM::WM_LBUTTONDOWN
        | WM::WM_LBUTTONUP
        | WM::WM_LBUTTONDBLCLK
        | WM::WM_RBUTTONDOWN
        | WM::WM_RBUTTONUP
        | WM::WM_RBUTTONDBLCLK
        | WM::WM_MBUTTONDOWN
        | WM::WM_MBUTTONUP
        | WM::WM_MBUTTONDBLCLK
        | WM::WM_XBUTTONDOWN
        | WM::WM_XBUTTONUP
        | WM::WM_XBUTTONDBLCLK
        | WM::WM_MOUSEWHEEL
        | WM::WM_MOUSEHWHEEL
        | WM_MOUSELEAVE => forward_mouse_message(hwnd, msg, wparam, lparam),
        WM::WM_POINTERDOWN | WM::WM_POINTERUPDATE | WM::WM_POINTERUP => {
            // Touch/pen only; a mouse pointer falls through so DefWindowProc
            // synthesizes the legacy mouse messages handled above.
            if let Some(state) = input_state(hwnd)
                && let Some(result) = super::pointer::forward_pointer_message(
                    &state.env3,
                    &state.controller,
                    hwnd,
                    msg,
                    wparam,
                )
            {
                return result;
            }
            unsafe { WM::DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM::WM_SETCURSOR => {
            if (lparam.0 & 0xffff) as u32 == HTCLIENT
                && let Some(state) = input_state(hwnd)
                && let Some(cursor) = controller_cursor(&state.controller)
            {
                unsafe { WM::SetCursor(Some(cursor)) };
                return LRESULT(1);
            }
            unsafe { WM::DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM::WM_SETFOCUS => {
            if let Some(state) = input_state(hwnd) {
                unsafe {
                    let _ = state
                        .base
                        .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
                }
            }
            LRESULT(0)
        }
        WM::WM_CAPTURECHANGED => {
            if let Some(state) = input_state(hwnd) {
                state.buttons_down = 0;
            }
            LRESULT(0)
        }
        WM::WM_GETOBJECT => {
            use windows::Win32::UI::Accessibility::{
                IRawElementProviderSimple, UiaReturnRawElementProvider, UiaRootObjectId,
            };
            if lparam.0 as i32 == UiaRootObjectId
                && let Some(state) = input_state(hwnd)
                && let Ok(controller2) = state
                    .controller
                    .cast::<ICoreWebView2CompositionController2>()
                && let Ok(provider) = unsafe { controller2.AutomationProvider() }
                && let Ok(provider) = provider.cast::<IRawElementProviderSimple>()
            {
                return unsafe { UiaReturnRawElementProvider(hwnd, wparam, lparam, &provider) };
            }
            unsafe { WM::DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM::WM_NCDESTROY => {
            let ptr =
                unsafe { WM::SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) } as *mut SurfaceInputState;
            if !ptr.is_null() {
                drop(unsafe { Box::from_raw(ptr) });
            }
            unsafe { WM::DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        _ => unsafe { WM::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn forward_mouse_message(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    use WindowsAndMessaging as WM;
    let Some(state) = input_state(hwnd) else {
        return unsafe { WM::DefWindowProcW(hwnd, msg, wparam, lparam) };
    };

    let is_wheel = matches!(msg, WM::WM_MOUSEWHEEL | WM::WM_MOUSEHWHEEL);
    let (virtual_keys, mouse_data, point) = if msg == WM_MOUSELEAVE {
        state.tracking_leave = false;
        (0u32, 0u32, POINT::default())
    } else {
        let mut point = POINT {
            x: (lparam.0 & 0xffff) as i16 as i32,
            y: ((lparam.0 >> 16) & 0xffff) as i16 as i32,
        };
        // Wheel messages carry screen coordinates.
        if is_wheel {
            unsafe {
                let _ = windows::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut point);
            }
        }
        let mouse_data = if is_wheel {
            ((wparam.0 >> 16) as u16 as i16 as i32) as u32
        } else if matches!(
            msg,
            WM::WM_XBUTTONDOWN | WM::WM_XBUTTONUP | WM::WM_XBUTTONDBLCLK
        ) {
            (wparam.0 >> 16) as u16 as u32
        } else {
            0
        };
        ((wparam.0 & 0xffff) as u32, mouse_data, point)
    };

    match msg {
        WM::WM_LBUTTONDOWN | WM::WM_RBUTTONDOWN | WM::WM_MBUTTONDOWN | WM::WM_XBUTTONDOWN => {
            // Activate/focus the surface first, WebView2 sample style: the
            // engine then takes real focus onto its hidden input window and
            // keyboard/IME flow natively.
            unsafe {
                let _ = SetFocus(Some(hwnd));
            }
            if state.buttons_down == 0 {
                unsafe { SetCapture(hwnd) };
            }
            state.buttons_down |= button_bit(msg, wparam);
        }
        WM::WM_LBUTTONUP | WM::WM_RBUTTONUP | WM::WM_MBUTTONUP | WM::WM_XBUTTONUP => {
            state.buttons_down &= !button_bit(msg, wparam);
            if state.buttons_down == 0 {
                unsafe {
                    let _ = ReleaseCapture();
                }
            }
        }
        WM::WM_MOUSEMOVE if !state.tracking_leave => {
            let mut track = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            state.tracking_leave = unsafe { TrackMouseEvent(&mut track).is_ok() };
        }
        _ => {}
    }

    let result = unsafe {
        state.controller.SendMouseInput(
            COREWEBVIEW2_MOUSE_EVENT_KIND(msg as i32),
            COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS(virtual_keys as i32),
            mouse_data,
            point,
        )
    };
    if let Err(err) = result {
        log::debug!("SendMouseInput({msg:#06x}) failed: {err}");
    }
    LRESULT(0)
}
