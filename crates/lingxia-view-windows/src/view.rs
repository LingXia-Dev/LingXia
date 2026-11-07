//! The page's content window: Win32 input becomes host calls, CPU frames paint through GDI.
//! The window procedure finds its page through this thread's registry.
use crate::Page;
use lingxia_view::interaction::view::{Key, Pointer, PointerPhase, Wheel, WheelPhase};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::atomic::Ordering;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, DIB_RGB_COLORS, EndPaint, InvalidateRect,
    PAINTSTRUCT, ScreenToClient, SetDIBitsToDevice, UpdateWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
    VIRTUAL_KEY, VK_CONTROL, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_MENU, VK_NEXT,
    VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CreateWindowExW, DLGC_WANTALLKEYS, DefWindowProcW, GetClientRect, GetMessageTime,
    GetSystemMetrics, IDC_ARROW, IsWindowVisible, LoadCursorW, RegisterClassW, SM_CXDRAG,
    WINDOW_EX_STYLE, WM_CAPTURECHANGED, WM_CHAR, WM_DESTROY, WM_DPICHANGED_AFTERPARENT,
    WM_ERASEBKGND, WM_GETDLGCODE, WM_KEYDOWN, WM_KILLFOCUS, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_RBUTTONUP, WM_SETFOCUS,
    WM_SIZE, WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_VISIBLE,
};
use windows::core::w;

const WM_MOUSELEAVE: u32 = 0x02A3;
/// A character travels to the core's key path as this plus its code point.
const CHAR_BASE: i64 = 0x1_0000;

/// Per-window input state, on the window's thread.
#[derive(Default)]
pub(crate) struct InputState {
    /// The left button went down in the page and is held.
    pressed: bool,
    /// The press was the second of a double click.
    double: bool,
    /// `WM_MOUSELEAVE` is armed.
    tracking: bool,
    /// A `WM_KEYDOWN` the page took: its `WM_CHAR` is not a second key.
    swallow_char: bool,
}

thread_local! {
    static PAGES: RefCell<HashMap<isize, Weak<Page>>> = RefCell::new(HashMap::new());
}

pub(crate) fn register(hwnd: isize, page: &Rc<Page>) {
    PAGES.with(|pages| pages.borrow_mut().insert(hwnd, Rc::downgrade(page)));
}

pub(crate) fn unregister(hwnd: isize) {
    PAGES.with(|pages| pages.borrow_mut().remove(&hwnd));
}

fn page_of(hwnd: HWND) -> Option<Rc<Page>> {
    PAGES.with(|pages| pages.borrow().get(&(hwnd.0 as isize))?.upgrade())
}

/// A visible child of `parent` on this thread.
pub(crate) fn create(parent: isize) -> Result<isize, String> {
    let class = w!("LingXiaGpuPage");
    // SAFETY: plain Win32 calls; a repeat class registration fails harmlessly.
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        RegisterClassW(&WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: class,
            ..Default::default()
        });
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            None,
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
            0,
            0,
            0,
            0,
            Some(HWND(parent as *mut _)),
            None,
            Some(instance.into()),
            None,
        )
        .map(|hwnd| hwnd.0 as isize)
        .map_err(|e| format!("GPU page window: {e}"))
    }
}

/// Paint the newest CPU frame now; false when the window cannot show it.
pub(crate) fn repaint(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    let hwnd = HWND(hwnd as *mut _);
    // SAFETY: a window of this thread.
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return false;
        }
        let _ = InvalidateRect(Some(hwnd), None, false);
        UpdateWindow(hwnd).as_bool()
    }
}

/// Client size in device pixels.
pub(crate) fn client_size(hwnd: isize) -> (u32, u32) {
    let mut rect = RECT::default();
    // SAFETY: an out-parameter on the stack.
    let _ = unsafe { GetClientRect(HWND(hwnd as *mut _), &mut rect) };
    (
        (rect.right - rect.left).max(0) as u32,
        (rect.bottom - rect.top).max(0) as u32,
    )
}

fn point(lparam: LPARAM) -> (f32, f32) {
    let x = (lparam.0 & 0xffff) as u16 as i16;
    let y = ((lparam.0 >> 16) & 0xffff) as u16 as i16;
    (f32::from(x), f32::from(y))
}

fn held(key: VIRTUAL_KEY) -> bool {
    // SAFETY: reads this thread's key state.
    unsafe { GetKeyState(i32::from(key.0)) < 0 }
}

/// How long ago the message being handled was posted, in microseconds.
fn input_age() -> u64 {
    // SAFETY: both read the tick count of this thread's queue.
    let (now, sent) = unsafe {
        (
            windows::Win32::System::SystemInformation::GetTickCount(),
            GetMessageTime() as u32,
        )
    };
    u64::from(now.wrapping_sub(sent).min(10_000)) * 1000
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let Some(page) = page_of(hwnd) else {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    };
    match handle(&page, hwnd, msg, wparam, lparam) {
        Some(result) => result,
        None => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn handle(
    page: &Rc<Page>,
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    let host = page.host();
    match msg {
        WM_SIZE | WM_DPICHANGED_AFTERPARENT => {
            page.report();
            None
        }
        // A host that destroys the window holding the page destroys the page's window too.
        WM_DESTROY => {
            page.window_destroyed();
            None
        }
        WM_ERASEBKGND => Some(LRESULT(1)),
        WM_PAINT => {
            paint(page, hwnd);
            Some(LRESULT(0))
        }
        WM_GETDLGCODE => Some(LRESULT(DLGC_WANTALLKEYS as isize)),
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            if host.input_blocked() {
                return None;
            }
            // SAFETY: windows of this thread.
            unsafe {
                let _ = SetFocus(Some(hwnd));
                SetCapture(hwnd);
            }
            {
                let mut input = page.input.borrow_mut();
                input.pressed = true;
                input.double = msg == WM_LBUTTONDBLCLK;
            }
            pointer(page, PointerPhase::Down, lparam);
            Some(LRESULT(0))
        }
        WM_MOUSEMOVE => {
            let (x, y) = point(lparam);
            let tracking = std::mem::replace(&mut page.input.borrow_mut().tracking, true);
            if !tracking {
                let mut track = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                // SAFETY: a sized struct for a window of this thread.
                let _ = unsafe { TrackMouseEvent(&mut track) };
            }
            host.hover(Some([x, y]));
            if page.input.borrow().pressed {
                pointer(page, PointerPhase::Move, lparam);
            }
            Some(LRESULT(0))
        }
        WM_LBUTTONUP => {
            let (pressed, double) = {
                let mut input = page.input.borrow_mut();
                (
                    std::mem::take(&mut input.pressed),
                    std::mem::take(&mut input.double),
                )
            };
            if pressed {
                pointer(page, PointerPhase::Up, lparam);
                if double {
                    let (x, y) = point(lparam);
                    host.select_word_at([x, y]);
                }
            }
            // SAFETY: releases this thread's capture; WM_CAPTURECHANGED sees `pressed` false.
            let _ = unsafe { ReleaseCapture() };
            Some(LRESULT(0))
        }
        WM_CAPTURECHANGED => {
            if std::mem::take(&mut page.input.borrow_mut().pressed) {
                pointer(page, PointerPhase::Cancel, LPARAM(0));
            }
            None
        }
        WM_MOUSELEAVE => {
            page.input.borrow_mut().tracking = false;
            host.hover(None);
            Some(LRESULT(0))
        }
        WM_RBUTTONUP => {
            let (x, y) = point(lparam);
            (!host.input_blocked() && host.context_menu([x, y])).then_some(LRESULT(0))
        }
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            if host.input_blocked() {
                return None;
            }
            let notches = f32::from(((wparam.0 >> 16) & 0xffff) as u16 as i16) / 120.0;
            let (x, y) = point(lparam);
            let mut at = POINT {
                x: x as i32,
                y: y as i32,
            };
            // SAFETY: an in-out point for a window of this thread.
            let _ = unsafe { ScreenToClient(hwnd, &mut at) };
            // Windows counts a vertical notch away from the user as positive; the core counts
            // toward the end of the axis. A horizontal notch to the right is already that.
            let (dx, dy) = if msg == WM_MOUSEWHEEL {
                (0.0, -notches)
            } else {
                (notches, 0.0)
            };
            host.input_age(input_age());
            host.wheel(Wheel {
                x: at.x as f32,
                y: at.y as f32,
                dx,
                dy,
                precise: false,
                phase: WheelPhase::Discrete,
            })
            .then_some(LRESULT(0))
        }
        WM_KEYDOWN => {
            page.input.borrow_mut().swallow_char = false;
            let code = match VIRTUAL_KEY(wparam.0 as u16) {
                VK_TAB => 0,
                VK_UP => 1,
                VK_DOWN => 2,
                VK_LEFT => 3,
                VK_RIGHT => 4,
                VK_HOME => 5,
                VK_END => 6,
                VK_RETURN | VK_SPACE => 7,
                VK_ESCAPE => 8,
                VK_PRIOR => 9,
                VK_NEXT => 10,
                _ => return None,
            };
            let handled = key(page, code, lparam);
            page.input.borrow_mut().swallow_char = handled;
            handled.then_some(LRESULT(0))
        }
        WM_CHAR => {
            if std::mem::take(&mut page.input.borrow_mut().swallow_char) {
                return Some(LRESULT(0));
            }
            // A printable character reaches the View's pickers (time digits, type-ahead).
            let unit = wparam.0 as u32;
            let printable = char::from_u32(unit).filter(|c| !c.is_control());
            let handled =
                printable.is_some_and(|c| key(page, CHAR_BASE + i64::from(u32::from(c)), lparam));
            handled.then_some(LRESULT(0))
        }
        WM_SETFOCUS => {
            host.focus_entered(held(VK_TAB) && held(VK_SHIFT));
            None
        }
        WM_KILLFOCUS => {
            host.focus_left();
            None
        }
        _ => None,
    }
}

fn pointer(page: &Page, phase: PointerPhase, lparam: LPARAM) {
    let (x, y) = point(lparam);
    // SAFETY: no arguments.
    let slop = unsafe { GetSystemMetrics(SM_CXDRAG) }.max(1) as f32;
    page.host().input_age(input_age());
    page.host().pointer(Pointer {
        phase,
        x,
        y,
        slop,
        velocity: [0.0, 0.0],
        min_fling: f32::MAX,
    });
}

fn key(page: &Page, code: i64, lparam: LPARAM) -> bool {
    let Some(key) = Key::from_code(code) else {
        return false;
    };
    if held(VK_CONTROL) || held(VK_MENU) || page.host().input_blocked() {
        return false;
    }
    let repeating = lparam.0 & (1 << 30) != 0;
    page.host().key(key, held(VK_SHIFT), repeating)
}

fn paint(page: &Page, hwnd: HWND) {
    let mut paint = PAINTSTRUCT::default();
    // SAFETY: Begin/EndPaint pair on this thread's window; the bitmap rows outlive the call.
    unsafe {
        let dc = BeginPaint(hwnd, &mut paint);
        let gpu = page.host().adapter().gpu.load(Ordering::Acquire) != 0;
        if !gpu
            && let Some(frame) = page
                .host()
                .adapter()
                .cpu_frame
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_ref()
        {
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: frame.width as i32,
                    // Top-down rows.
                    biHeight: -(frame.height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            SetDIBitsToDevice(
                dc,
                0,
                0,
                frame.width,
                frame.height,
                0,
                0,
                0,
                frame.height,
                frame.bgra.as_ptr().cast(),
                &info,
                DIB_RGB_COLORS,
            );
        }
        let _ = EndPaint(hwnd, &paint);
    }
}
