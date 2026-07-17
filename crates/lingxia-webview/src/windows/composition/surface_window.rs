//! The per-webview surface child window hosting the DirectComposition
//! target. A composition-hosted WebView2 has no input HWND of its own, so
//! this window forwards mouse/wheel/pointer input, mirrors the engine
//! cursor, and bridges focus; once WebView2 gains focus its hidden input
//! window receives keyboard and IME directly from the OS.
//!
//! Re-entrancy contract: the state box in `GWLP_USERDATA` may only be
//! borrowed inside [`with_state`]'s closure, and that closure must not call
//! anything that can dispatch messages back into this wndproc (`SetFocus`,
//! `SetCapture`, `ReleaseCapture`, any COM call — an STA COM call pumps).
//! Such calls run after the closure returns, on COM handles cloned out of
//! the state, or two `&mut` borrows of the same box would alias.

use super::*;
use windows::Win32::Foundation::{LRESULT, POINT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, GA_ROOT, GWLP_USERDATA, HTCLIENT, IDC_ARROW, WNDCLASSW,
    WS_CHILD, WS_CLIPSIBLINGS, WS_EX_NOREDIRECTIONBITMAP,
};

const SURFACE_CLASS: PCWSTR = windows::core::w!("LingXiaWebViewSurface");

// Defined next to TrackMouseEvent's TME_LEAVE, not with the WM_MOUSE* set.
const WM_MOUSELEAVE: u32 = windows::Win32::UI::Controls::WM_MOUSELEAVE;

/// Grace period between hiding a surface window and suspending its
/// controller ("LXHS"). Quick hide→show cycles within it re-reveal a live
/// frame instead of flashing the card while presentation restarts.
const HIDE_SUSPEND_TIMER: usize = 0x4C58_4853;
const HIDE_SUSPEND_GRACE_MS: u32 = 1_000;

/// Starts the hide→suspend grace timer; called after `ShowWindow(SW_HIDE)`.
pub(crate) fn schedule_hide_suspend(hwnd: HWND) {
    unsafe {
        WindowsAndMessaging::SetTimer(Some(hwnd), HIDE_SUSPEND_TIMER, HIDE_SUSPEND_GRACE_MS, None);
    }
}

/// Cancels a pending hide→suspend; called before re-showing.
pub(crate) fn cancel_hide_suspend(hwnd: HWND) {
    unsafe {
        let _ = WindowsAndMessaging::KillTimer(Some(hwnd), HIDE_SUSPEND_TIMER);
    }
}

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

/// Borrows the state for the duration of `run` only. See the module-level
/// re-entrancy contract: `run` must not dispatch messages.
fn with_state<T>(hwnd: HWND, run: impl FnOnce(&mut SurfaceInputState) -> T) -> Option<T> {
    let ptr = unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA) }
        as *mut SurfaceInputState;
    unsafe { ptr.as_mut() }.map(run)
}

/// Tokens for the controller-level event subscriptions a surface window
/// owns; removed before a recreated surface re-subscribes, so handler
/// chains don't grow across host-teardown recoveries.
#[derive(Default, Clone, Copy)]
pub(crate) struct InputSubscriptions {
    cursor_changed: i64,
    move_focus_requested: i64,
}

fn ensure_surface_class() -> bool {
    static REGISTERED: OnceLock<bool> = OnceLock::new();
    *REGISTERED.get_or_init(|| unsafe {
        let class = WNDCLASSW {
            // CS_DBLCLKS: without it Windows never synthesizes the
            // WM_*BUTTONDBLCLK messages the forwarding below relies on.
            style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
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
) -> InputSubscriptions {
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
    let mut tokens = InputSubscriptions::default();

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
    unsafe {
        if let Err(err) = controller.add_CursorChanged(&cursor_handler, &mut tokens.cursor_changed)
        {
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
    unsafe {
        if let Err(err) =
            base.add_MoveFocusRequested(&focus_handler, &mut tokens.move_focus_requested)
        {
            log::warn!("add_MoveFocusRequested failed: {err}");
        }
    }
    tokens
}

/// Removes the subscriptions from a previous [`attach_input`] before a
/// recreated surface re-subscribes.
pub(crate) fn detach_input(
    controller: &ICoreWebView2CompositionController,
    base: &ICoreWebView2Controller,
    tokens: InputSubscriptions,
) {
    unsafe {
        if tokens.cursor_changed != 0 {
            let _ = controller.remove_CursorChanged(tokens.cursor_changed);
        }
        if tokens.move_focus_requested != 0 {
            let _ = base.remove_MoveFocusRequested(tokens.move_focus_requested);
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
        WM::WM_POINTERDOWN
        | WM::WM_POINTERUPDATE
        | WM::WM_POINTERUP
        | WM::WM_POINTERENTER
        | WM::WM_POINTERLEAVE
        | WM::WM_POINTERACTIVATE => {
            // Touch/pen only; a mouse pointer falls through so DefWindowProc
            // synthesizes the legacy mouse messages handled above.
            let handles = with_state(hwnd, |state| (state.env3.clone(), state.controller.clone()));
            if let Some((env3, controller)) = handles
                && let Some(result) =
                    super::pointer::forward_pointer_message(&env3, &controller, hwnd, msg, wparam)
            {
                // Same focus bridge as the mouse button-down path: WebView2
                // then moves real focus to its hidden input window, so a tap
                // into a text field can actually receive keyboard/IME input.
                if msg == WM::WM_POINTERDOWN {
                    unsafe {
                        let _ = SetFocus(Some(hwnd));
                    }
                }
                // Activation policy still belongs to the system.
                if msg == WM::WM_POINTERACTIVATE {
                    return unsafe { WM::DefWindowProcW(hwnd, msg, wparam, lparam) };
                }
                return result;
            }
            unsafe { WM::DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM::WM_SETCURSOR => {
            if (lparam.0 & 0xffff) as u32 == HTCLIENT
                && let Some(controller) = with_state(hwnd, |state| state.controller.clone())
                && let Some(cursor) = controller_cursor(&controller)
            {
                unsafe { WM::SetCursor(Some(cursor)) };
                return LRESULT(1);
            }
            unsafe { WM::DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM::WM_SETFOCUS => {
            if let Some(base) = with_state(hwnd, |state| state.base.clone()) {
                unsafe {
                    let _ = base.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
                }
            }
            LRESULT(0)
        }
        WM::WM_CAPTURECHANGED => {
            // Capture stolen mid-drag (menu, drag loop, another SetCapture):
            // tell the engine the pointer stream ended, or it keeps a stuck
            // drag/selection until the next unrelated click.
            let interrupted = with_state(hwnd, |state| {
                let had_buttons = state.buttons_down != 0;
                state.buttons_down = 0;
                state.tracking_leave = false;
                had_buttons.then(|| state.controller.clone())
            })
            .flatten();
            if let Some(controller) = interrupted {
                unsafe {
                    let _ = controller.SendMouseInput(
                        COREWEBVIEW2_MOUSE_EVENT_KIND(WM_MOUSELEAVE as i32),
                        COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS(0),
                        0,
                        POINT::default(),
                    );
                }
            }
            LRESULT(0)
        }
        WM::WM_TIMER if wparam.0 == HIDE_SUSPEND_TIMER => {
            cancel_hide_suspend(hwnd);
            // Still hidden after the grace period: stop rasterization.
            let base = with_state(hwnd, |state| state.base.clone());
            if let Some(base) = base
                && unsafe { !WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() }
            {
                unsafe {
                    let _ = base.SetIsVisible(false);
                }
            }
            LRESULT(0)
        }
        WM::WM_GETOBJECT => {
            use windows::Win32::UI::Accessibility::{
                IRawElementProviderSimple, UiaReturnRawElementProvider, UiaRootObjectId,
            };
            if lparam.0 as i32 == UiaRootObjectId
                && let Some(controller) = with_state(hwnd, |state| state.controller.clone())
                && let Ok(controller2) = controller.cast::<ICoreWebView2CompositionController2>()
                && let Ok(provider) = unsafe { controller2.AutomationProvider() }
                && let Ok(provider) = provider.cast::<IRawElementProviderSimple>()
            {
                return unsafe { UiaReturnRawElementProvider(hwnd, wparam, lparam, &provider) };
            }
            unsafe { WM::DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM::WM_DESTROY => {
            // Covers implicit teardown too (a foreign host window being
            // destroyed takes reparented surfaces with it): the OLE
            // drop-target registration must be revoked before the window is
            // gone or it leaks with a controller reference.
            super::dragdrop::revoke_drop_target(hwnd);
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

/// Deferred re-entrant work decided while the state was borrowed.
#[derive(Default)]
struct MouseActions {
    set_focus: bool,
    set_capture: bool,
    release_capture: bool,
}

fn forward_mouse_message(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    use WindowsAndMessaging as WM;
    let is_wheel = matches!(msg, WM::WM_MOUSEWHEEL | WM::WM_MOUSEHWHEEL);
    let (virtual_keys, mouse_data, point) = if msg == WM_MOUSELEAVE {
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

    // Bookkeeping under a scoped borrow; message-dispatching calls run after
    // it ends (module-level re-entrancy contract).
    let Some((controller, actions)) = with_state(hwnd, |state| {
        let mut actions = MouseActions::default();
        match msg {
            WM::WM_LBUTTONDOWN | WM::WM_RBUTTONDOWN | WM::WM_MBUTTONDOWN | WM::WM_XBUTTONDOWN => {
                actions.set_focus = true;
                actions.set_capture = state.buttons_down == 0;
                state.buttons_down |= button_bit(msg, wparam);
            }
            WM::WM_LBUTTONUP | WM::WM_RBUTTONUP | WM::WM_MBUTTONUP | WM::WM_XBUTTONUP => {
                state.buttons_down &= !button_bit(msg, wparam);
                actions.release_capture = state.buttons_down == 0;
            }
            WM::WM_MOUSEMOVE if !state.tracking_leave => {
                let mut track = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                // TrackMouseEvent registers a request; it dispatches nothing.
                state.tracking_leave = unsafe { TrackMouseEvent(&mut track).is_ok() };
            }
            WM_MOUSELEAVE => state.tracking_leave = false,
            _ => {}
        }
        (state.controller.clone(), actions)
    }) else {
        return unsafe { WM::DefWindowProcW(hwnd, msg, wparam, lparam) };
    };

    if actions.set_focus {
        // Activate/focus the surface first, WebView2 sample style: the
        // engine then takes real focus onto its hidden input window and
        // keyboard/IME flow natively.
        unsafe {
            let _ = SetFocus(Some(hwnd));
        }
    }
    if actions.set_capture {
        unsafe { SetCapture(hwnd) };
    }
    if actions.release_capture {
        unsafe {
            let _ = ReleaseCapture();
        }
    }

    let result = unsafe {
        controller.SendMouseInput(
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
