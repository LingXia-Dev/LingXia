//! The page's content window: Win32 input becomes host calls, CPU frames paint through GDI.
//! The window procedure finds its page through this thread's registry.
use crate::Page;
use lingxia_view::interaction::view::{Key, Pointer, PointerPhase, Wheel, WheelPhase};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::atomic::Ordering;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, CreateSolidBrush, DIB_RGB_COLORS,
    DeleteObject, EndPaint, FillRect, InvalidateRect, PAINTSTRUCT, ScreenToClient,
    SetDIBitsToDevice, UpdateWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::Ime::{
    GCS_COMPSTR, GCS_RESULTSTR, IME_COMPOSITION_STRING, ISC_SHOWUICOMPOSITIONWINDOW,
    ImmGetCompositionStringW, ImmGetContext, ImmReleaseContext,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
    VIRTUAL_KEY, VK_CONTROL, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_MENU, VK_NEXT,
    VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
};
use windows::Win32::UI::Input::Pointer::GetPointerType;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CreateWindowExW, DLGC_WANTALLKEYS, DefWindowProcW, GetClientRect, GetMessageTime,
    GetSystemMetrics, IDC_ARROW, IsWindowVisible, LoadCursorW, PT_PEN, PT_TOUCH, RegisterClassW,
    RegisterWindowMessageW, SM_CXDRAG, WINDOW_EX_STYLE, WM_CAPTURECHANGED, WM_CHAR, WM_DESTROY,
    WM_DPICHANGED_AFTERPARENT, WM_ERASEBKGND, WM_GETDLGCODE, WM_GETOBJECT, WM_IME_COMPOSITION,
    WM_IME_ENDCOMPOSITION, WM_IME_SETCONTEXT, WM_IME_STARTCOMPOSITION, WM_KEYDOWN, WM_KILLFOCUS,
    WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_PAINT, WM_POINTERCAPTURECHANGED, WM_POINTERDOWN, WM_POINTERUP, WM_POINTERUPDATE,
    WM_RBUTTONUP, WM_SETFOCUS, WM_SIZE, WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
    WS_VISIBLE,
};
use windows::core::w;

const WM_MOUSELEAVE: u32 = 0x02A3;
/// A character travels to the core's key path as this plus its code point.
const CHAR_BASE: i64 = 0x1_0000;

/// Per-window input state, on the window's thread.
#[derive(Default)]
pub(crate) struct InputState {
    /// The left button went down in the page and is held.
    pub(crate) pressed: bool,
    /// The press was the second of a double click.
    double: bool,
    /// `WM_MOUSELEAVE` is armed.
    tracking: bool,
    /// A `WM_KEYDOWN` the page took: its `WM_CHAR` is not a second key.
    swallow_char: bool,
    /// The finger (pointer id) the page follows; one at a time.
    finger: Option<u32>,
    velocity: crate::touch::Velocity,
    pub(crate) web: crate::web_input::WebInput,
}

/// The finger id synthetic touch uses; real pointer ids are small.
const SYNTHETIC_FINGER: u32 = u32::MAX;

/// The registered message automation posts to inject touch (the platform crate registers
/// the same name): `wparam` is the phase (0 down, 1 move, 2 up, 3 cancel), `lparam` the
/// client point packed as for mouse messages.
fn touch_message() -> u32 {
    static MESSAGE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    // SAFETY: a constant name.
    *MESSAGE.get_or_init(|| unsafe { RegisterWindowMessageW(w!("LingXiaViewTouch")) })
}

/// The high word of `wparam` on automation's posted mouse messages (`lingxia-webview`'s
/// `SYNTHETIC_MOUSE_WPARAM_MARKER`).
pub(crate) const SYNTHETIC_MOUSE: usize = 0x4c58_0000;

/// Automation's precision-touchpad scroll: a phase over the fingers' travel in window pixels,
/// which the content follows.
fn scroll_message() -> u32 {
    static MESSAGE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    // SAFETY: a constant name.
    *MESSAGE.get_or_init(|| unsafe { RegisterWindowMessageW(w!("LingXiaViewScroll")) })
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

pub(crate) fn page_for(hwnd: isize) -> Option<Rc<Page>> {
    PAGES.with(|pages| pages.borrow().get(&hwnd)?.upgrade())
}

fn page_of(hwnd: HWND) -> Option<Rc<Page>> {
    page_for(hwnd.0 as isize)
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

/// A message's client position in the page's pixels.
fn at(page: &Page, lparam: LPARAM) -> (f32, f32) {
    page.page_pixels(point(lparam))
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
    if msg == touch_message() && msg != 0 {
        let phase = match wparam.0 {
            0 => PointerPhase::Down,
            1 => PointerPhase::Move,
            2 => PointerPhase::Up,
            _ => PointerPhase::Cancel,
        };
        touch(page, hwnd, SYNTHETIC_FINGER, phase, at(page, lparam));
        return Some(LRESULT(0));
    }
    if msg == scroll_message() && msg != 0 {
        let host = page.host();
        if host.input_blocked() {
            return Some(LRESULT(0));
        }
        let pixels = page.scale() / page.shown_scale();
        let delta = |shift: usize| f32::from(((wparam.0 >> shift) & 0xffff) as u16 as i16) * pixels;
        let phase = match wparam.0 >> 32 {
            0 => WheelPhase::Began,
            1 => WheelPhase::Changed,
            2 => WheelPhase::Ended,
            _ => WheelPhase::Momentum,
        };
        let (x, y) = at(page, lparam);
        host.input_age(input_age());
        host.wheel(Wheel {
            x,
            y,
            dx: delta(0),
            dy: delta(16),
            precise: true,
            phase,
        });
        return Some(LRESULT(0));
    }
    if msg == crate::uia::CALL {
        return Some(crate::uia::call(lparam));
    }
    if !host.input_blocked() && crate::web_input::route(page, hwnd, msg, wparam, lparam) {
        return Some(LRESULT(0));
    }
    match msg {
        WM_GETOBJECT => crate::uia::get_object(hwnd, wparam, lparam),
        WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP | WM_POINTERCAPTURECHANGED => {
            let id = (wparam.0 & 0xffff) as u32;
            let mut kind = Default::default();
            // SAFETY: an out-parameter for this message's pointer.
            let finger = unsafe { GetPointerType(id, &mut kind) }.is_ok()
                && (kind == PT_TOUCH || kind == PT_PEN);
            if !finger {
                return None;
            }
            let phase = match msg {
                WM_POINTERDOWN => PointerPhase::Down,
                WM_POINTERUPDATE => PointerPhase::Move,
                WM_POINTERUP => PointerPhase::Up,
                _ => PointerPhase::Cancel,
            };
            let mut at = POINT {
                x: i32::from((lparam.0 & 0xffff) as u16 as i16),
                y: i32::from(((lparam.0 >> 16) & 0xffff) as u16 as i16),
            };
            // SAFETY: converts in place for this thread's window.
            let _ = unsafe { ScreenToClient(hwnd, &mut at) };
            touch(
                page,
                hwnd,
                id,
                phase,
                page.page_pixels((at.x as f32, at.y as f32)),
            );
            // Handled pointer messages are not promoted to mouse messages.
            Some(LRESULT(0))
        }
        WM_SIZE | WM_DPICHANGED_AFTERPARENT => {
            page.report();
            crate::recovery::layout(hwnd.0 as isize);
            None
        }
        // A host that destroys the window holding the page destroys the page's window too.
        WM_DESTROY => {
            crate::recovery::forget(hwnd.0 as isize);
            crate::composition::forget(hwnd.0 as isize);
            crate::uia::window_destroyed(hwnd);
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
            let (x, y) = at(page, lparam);
            // Automation's posted moves: tracking follows the physical cursor, which is
            // elsewhere, and would leave at once.
            let synthetic = wparam.0 & 0xffff_0000 == SYNTHETIC_MOUSE;
            let tracking =
                synthetic || std::mem::replace(&mut page.input.borrow_mut().tracking, true);
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
                    let (x, y) = at(page, lparam);
                    host.select_word_at([x, y]);
                }
            }
            // SAFETY: releases this thread's capture; WM_CAPTURECHANGED sees `pressed` false.
            let _ = unsafe { ReleaseCapture() };
            Some(LRESULT(0))
        }
        WM_CAPTURECHANGED => {
            crate::web_input::release(page);
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
            let (x, y) = at(page, lparam);
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
            let (x, y) = page.page_pixels((at.x as f32, at.y as f32));
            host.wheel(Wheel {
                x,
                y,
                dx,
                dy,
                precise: false,
                phase: WheelPhase::Discrete,
            })
            .then_some(LRESULT(0))
        }
        WM_KEYDOWN => {
            page.input.borrow_mut().swallow_char = false;
            match crate::ime::key(host, wparam.0 as u16, held(VK_SHIFT), held(VK_CONTROL)) {
                crate::ime::KeyUse::Taken => {
                    page.input.borrow_mut().swallow_char = true;
                    return Some(LRESULT(0));
                }
                crate::ime::KeyUse::Character => return Some(LRESULT(0)),
                crate::ime::KeyUse::Page => {}
            }
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
            if crate::ime::character(host, wparam.0 as u16) {
                return Some(LRESULT(0));
            }
            // A printable character reaches the View's pickers (time digits, type-ahead).
            let unit = wparam.0 as u32;
            let printable = char::from_u32(unit).filter(|c| !c.is_control());
            let handled =
                printable.is_some_and(|c| key(page, CHAR_BASE + i64::from(u32::from(c)), lparam));
            handled.then_some(LRESULT(0))
        }
        // The page draws the composition in the editor; the system draws only candidates.
        WM_IME_SETCONTEXT => {
            let lparam = LPARAM(lparam.0 & !(ISC_SHOWUICOMPOSITIONWINDOW as isize));
            // SAFETY: forwards this window's message.
            Some(unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) })
        }
        WM_IME_STARTCOMPOSITION => host.adapter().ime.connected().then_some(LRESULT(0)),
        WM_IME_COMPOSITION => {
            let flags = lparam.0 as u32;
            let read = |kind: IME_COMPOSITION_STRING| composition_text(hwnd, kind);
            let result = (flags & GCS_RESULTSTR.0 != 0)
                .then(|| read(GCS_RESULTSTR))
                .flatten();
            let composing =
                (flags & GCS_COMPSTR.0 != 0).then(|| read(GCS_COMPSTR).unwrap_or_default());
            crate::ime::composition(host, result, composing).then_some(LRESULT(0))
        }
        WM_IME_ENDCOMPOSITION => {
            crate::ime::end_composition(host);
            host.adapter().ime.connected().then_some(LRESULT(0))
        }
        // Focus that arrives by Tab walks into the page; a click only takes it, so the page
        // does not move a focus ring (and scroll) under the pointer.
        WM_SETFOCUS => {
            if held(VK_TAB) {
                host.focus_entered(held(VK_SHIFT));
            }
            None
        }
        WM_KILLFOCUS => {
            host.focus_left();
            None
        }
        _ => None,
    }
}

/// The input method's `kind` string for this window.
fn composition_text(hwnd: HWND, kind: IME_COMPOSITION_STRING) -> Option<String> {
    // SAFETY: the window's context, read into a buffer of the size it reports, then released.
    unsafe {
        let himc = ImmGetContext(hwnd);
        if himc.is_invalid() {
            return None;
        }
        let bytes = ImmGetCompositionStringW(himc, kind, None, 0);
        let text = (bytes >= 0).then(|| {
            let mut units = vec![0u16; bytes as usize / 2];
            ImmGetCompositionStringW(himc, kind, Some(units.as_mut_ptr().cast()), bytes as u32);
            String::from_utf16_lossy(&units)
        });
        let _ = ImmReleaseContext(hwnd, himc);
        text
    }
}

fn pointer(page: &Page, phase: PointerPhase, lparam: LPARAM) {
    let (x, y) = at(page, lparam);
    // SAFETY: no arguments.
    let slop =
        unsafe { GetSystemMetrics(SM_CXDRAG) }.max(1) as f32 * page.scale() / page.shown_scale();
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

/// A finger on the page: one at a time, as Android takes it; a second one cancels.
fn touch(page: &Page, hwnd: HWND, id: u32, phase: PointerPhase, (x, y): (f32, f32)) {
    let host = page.host();
    if host.input_blocked() {
        return;
    }
    // SAFETY: no arguments.
    let ms = f64::from(unsafe { GetMessageTime() } as u32);
    let scale = page.scale();
    let velocity = {
        let mut input = page.input.borrow_mut();
        match (phase, input.finger) {
            (PointerPhase::Down, None) => {
                input.finger = Some(id);
                input.velocity.reset();
            }
            (PointerPhase::Down, Some(_)) => {
                input.finger = None;
                drop(input);
                send_touch(page, PointerPhase::Cancel, x, y, [0.0, 0.0], scale);
                return;
            }
            (_, Some(current)) if current == id => {}
            _ => return,
        }
        input.velocity.add(ms, x, y);
        if matches!(phase, PointerPhase::Up | PointerPhase::Cancel) {
            input.finger = None;
        }
        if phase == PointerPhase::Up {
            let max = 8000.0 * scale;
            input.velocity.release(ms).map(|v| v.clamp(-max, max))
        } else {
            [0.0, 0.0]
        }
    };
    if phase == PointerPhase::Down {
        // SAFETY: this thread's window.
        let _ = unsafe { SetFocus(Some(hwnd)) };
    }
    send_touch(page, phase, x, y, velocity, scale);
}

fn send_touch(page: &Page, phase: PointerPhase, x: f32, y: f32, velocity: [f32; 2], scale: f32) {
    page.host().input_age(input_age());
    page.host().pointer(Pointer {
        phase,
        x,
        y,
        slop: 8.0 * scale,
        velocity,
        min_fling: 50.0 * scale,
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
        let drawn = if gpu {
            page.host().snapshot().is_some()
        } else {
            page.host()
                .adapter()
                .cpu_frame
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .is_some()
        };
        if !drawn {
            // Nothing presented yet: an unpainted window shows whatever lies behind it.
            let color = if page.dark() {
                0x0020_2020
            } else {
                0x00FF_FFFF
            };
            let brush = CreateSolidBrush(COLORREF(color));
            FillRect(dc, &paint.rcPaint, brush);
            let _ = DeleteObject(brush.into());
        }
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
