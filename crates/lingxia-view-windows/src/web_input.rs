//! Mouse input over an embedded `<webview>`. Its surface window is a disabled child of the page's
//! window, so the page takes every message and hands on the ones the web view owns: a View
//! control painted over it keeps its own.

use crate::Page;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{MapWindowPoints, ScreenToClient};
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, GetWindowRect, IsWindowVisible, PostMessageW, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_XBUTTONDBLCLK,
    WM_XBUTTONDOWN, WM_XBUTTONUP,
};
use windows::core::w;

const WM_MOUSELEAVE: u32 = 0x02A3;
/// `MK_LBUTTON | MK_RBUTTON | MK_MBUTTON | MK_XBUTTON1 | MK_XBUTTON2`.
const BUTTONS: usize = 0x0001 | 0x0002 | 0x0010 | 0x0020 | 0x0040;

/// Where the page's mouse goes while over or pressed on a web view.
#[derive(Default)]
pub(crate) struct WebInput {
    /// The surface a press went to: it keeps the mouse until its last button lifts.
    held: Option<isize>,
    /// The surface under the cursor, which gets `WM_MOUSELEAVE` once the cursor goes.
    hover: Option<isize>,
}

/// Hands `msg` to the web view that owns it; `true` when one took it.
pub(crate) fn route(page: &Page, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> bool {
    let down = matches!(
        msg,
        WM_LBUTTONDOWN
            | WM_LBUTTONDBLCLK
            | WM_RBUTTONDOWN
            | WM_RBUTTONDBLCLK
            | WM_MBUTTONDOWN
            | WM_MBUTTONDBLCLK
            | WM_XBUTTONDOWN
            | WM_XBUTTONDBLCLK
    );
    let up = matches!(
        msg,
        WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP | WM_XBUTTONUP
    );
    let wheel = matches!(msg, WM_MOUSEWHEEL | WM_MOUSEHWHEEL);
    if !(down || up || wheel || msg == WM_MOUSEMOVE) {
        return false;
    }
    let mut at = POINT {
        x: i32::from((lparam.0 & 0xffff) as u16 as i16),
        y: i32::from(((lparam.0 >> 16) & 0xffff) as u16 as i16),
    };
    if wheel {
        // SAFETY: an in-out point for a window of this thread.
        let _ = unsafe { ScreenToClient(hwnd, &mut at) };
    }
    let (held, hover, pressed) = {
        let input = page.input.borrow();
        (input.web.held, input.web.hover, input.pressed)
    };
    let target = match held {
        Some(surface) => Some(surface),
        // A press the page took stays the page's.
        None if pressed => None,
        None => {
            let (x, y) = page.page_pixels((at.x as f32, at.y as f32));
            page.host()
                .webview_at([x, y])
                .and_then(|_| surface_at(hwnd, at))
        }
    };
    if hover != target {
        if let Some(left) = hover {
            post(left, WM_MOUSELEAVE, WPARAM(0), LPARAM(0));
        }
        if hover.is_none() {
            page.host().hover(None);
        }
        page.input.borrow_mut().web.hover = target;
    }
    let Some(surface) = target else {
        return false;
    };
    let (wparam, lparam) = if wheel {
        (wparam, lparam)
    } else {
        let mut local = at;
        // SAFETY: maps between two live windows.
        unsafe {
            MapWindowPoints(
                Some(hwnd),
                Some(HWND(surface as *mut _)),
                std::slice::from_mut(&mut local),
            )
        };
        let packed = (local.x as u16 as usize) | ((local.y as u16 as usize) << 16);
        // The marker keeps the surface from tracking a cursor that is over the page.
        (
            WPARAM(wparam.0 & 0xffff | crate::view::SYNTHETIC_MOUSE),
            LPARAM(packed as isize),
        )
    };
    post(surface, msg, wparam, lparam);
    if down {
        page.input.borrow_mut().web.held = Some(surface);
        // SAFETY: this thread's window, so moves outside it still reach the web view.
        unsafe { SetCapture(hwnd) };
    } else if up && wparam.0 & BUTTONS == 0 {
        page.input.borrow_mut().web.held = None;
        // SAFETY: releases this thread's capture.
        let _ = unsafe { ReleaseCapture() };
    }
    true
}

/// The page lost the mouse: a held press ends where it is.
pub(crate) fn release(page: &Page) {
    page.input.borrow_mut().web.held = None;
}

fn post(surface: isize, msg: u32, wparam: WPARAM, lparam: LPARAM) {
    // SAFETY: posting to a window that may since have gone fails harmlessly.
    let _ = unsafe { PostMessageW(Some(HWND(surface as *mut _)), msg, wparam, lparam) };
}

/// The visible web view surface window under client point `at`.
fn surface_at(page: HWND, at: POINT) -> Option<isize> {
    let mut screen = at;
    // SAFETY: maps a point of a live window.
    unsafe { MapWindowPoints(Some(page), None, std::slice::from_mut(&mut screen)) };
    let mut child = None;
    loop {
        // SAFETY: walks this window's children by class.
        let next = unsafe { FindWindowExW(Some(page), child, w!("LingXiaWebViewSurface"), None) }
            .ok()
            .filter(|hwnd| !hwnd.0.is_null())?;
        child = Some(next);
        let mut rect = RECT::default();
        // SAFETY: reads a live window's state.
        let hit =
            unsafe { IsWindowVisible(next).as_bool() && GetWindowRect(next, &mut rect).is_ok() }
                && (rect.left..rect.right).contains(&screen.x)
                && (rect.top..rect.bottom).contains(&screen.y);
        if hit {
            return Some(next.0 as isize);
        }
    }
}

/// Takes `page`'s web view surface windows out of hit testing: the page routes their mouse.
pub fn adopt_web_surfaces(page: isize) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_STYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_DISABLED,
    };
    let page = HWND(page as *mut _);
    let mut child = None;
    // SAFETY: walks the page's children by class and sets a style bit; a window that went
    // meanwhile fails harmlessly.
    unsafe {
        while let Some(next) = FindWindowExW(Some(page), child, w!("LingXiaWebViewSurface"), None)
            .ok()
            .filter(|hwnd| !hwnd.0.is_null())
        {
            child = Some(next);
            let style = GetWindowLongPtrW(next, GWL_STYLE);
            if style & WS_DISABLED.0 as isize == 0 {
                SetWindowLongPtrW(next, GWL_STYLE, style | WS_DISABLED.0 as isize);
            }
        }
    }
}
