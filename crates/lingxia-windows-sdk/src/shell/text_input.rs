//! Reusable inline text-input helper for shell chrome.
//!
//! [`begin_inline_edit`] places a borderless Win32 `EDIT` child over a
//! chrome rect so chrome-drawn text becomes editable in place. It is used
//! for terminal tab renames today and is intentionally generic so the
//! address bar can reuse it.
//!
//! Lifecycle: the control commits on Enter or focus loss, cancels on Esc,
//! and always destroys itself afterwards (no caller-side teardown). The
//! commit callback receives the edited text exactly once.
//!
//! Threading: `begin_inline_edit` MUST run on the UI thread that owns
//! `host_hwnd` (a child window pumps messages on its creator's thread).
//! Callers on other threads marshal via
//! `lingxia_windows_host::post_to_window_thread`; the commit
//! callback then also runs on that UI thread.
//!
//! Painting: the shell host windows do not use `WS_CLIPCHILDREN`, so a
//! full chrome repaint would draw over the control. The chrome painter
//! calls [`exclude_active_inline_edit`] to clip the control's rect out of
//! its repaints while an edit is active.
//!
//! The inline editor itself is shell-chrome infrastructure, but its only entry
//! points (`begin_inline_edit`) are reached through the browser address bar or
//! a terminal tab rename — so it is dead code when neither capability is built.
#![cfg_attr(
    not(any(feature = "browser-runtime", feature = "terminal-runtime")),
    allow(dead_code)
)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    DeleteObject, ExcludeClipRect, GetDC, HDC, HFONT, HGDIOBJ, InvalidateRect, ReleaseDC,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{SetFocus, VK_ESCAPE, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::{
    self, ES_AUTOHSCROLL, WINDOW_EX_STYLE, WINDOW_STYLE, WNDPROC,
};
use windows::core::{PCWSTR, w};

/// Commit callback of an inline edit; receives the final text on Enter or
/// focus loss. Runs on the host window's UI thread.
pub type InlineEditCommit = Arc<dyn Fn(String) + Send + Sync>;

/// `EM_SETSEL` (select text range) lives in `Win32::UI::Controls` in the
/// windows crate; defined locally to avoid pulling the whole feature.
const EM_SETSEL: u32 = 0x00b1;

/// Per-control state stashed in the EDIT child's `GWLP_USERDATA`.
struct InlineEditState {
    /// The EDIT class window procedure being subclassed.
    original_proc: isize,
    /// Raw handle of the host (parent) window.
    host: isize,
    /// Chrome text font selected into the control; deleted on destroy.
    font: HFONT,
    on_commit: InlineEditCommit,
    /// Guards against double commit/cancel: destroying the control on
    /// Enter re-enters the proc with WM_KILLFOCUS.
    finished: bool,
}

/// Active inline edits: host window handle -> (edit handle, rect in host
/// client coordinates). One edit per host; starting a new one replaces the
/// previous control.
static ACTIVE_EDITS: OnceLock<Mutex<HashMap<isize, (isize, RECT)>>> = OnceLock::new();

fn active_edits() -> std::sync::MutexGuard<'static, HashMap<isize, (isize, RECT)>> {
    ACTIVE_EDITS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        // The registry has no invariants that poisoning can break.
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Clips the host's active inline-edit rect out of `hdc` so chrome
/// repaints leave the EDIT child's pixels alone. No-op when no edit is
/// active on `host`.
pub(super) fn exclude_active_inline_edit(hdc: HDC, host: HWND) {
    let rect = active_edits()
        .get(&(host.0 as isize))
        .map(|(_, rect)| *rect);
    let Some(rect) = rect else {
        return;
    };
    unsafe {
        let _ = ExcludeClipRect(hdc, rect.left, rect.top, rect.right, rect.bottom);
    }
}

/// Starts an inline edit over `rect` (host client coordinates) prefilled
/// with `initial_text`, selected. See the module docs for lifecycle and
/// threading. Returns `false` when the control could not be created.
pub fn begin_inline_edit(
    host_hwnd: HWND,
    rect: RECT,
    initial_text: &str,
    on_commit: InlineEditCommit,
) -> bool {
    // Replace any previous edit on this host; destroying it commits it
    // through its own kill-focus path before the new control appears.
    let previous = active_edits()
        .get(&(host_hwnd.0 as isize))
        .map(|(edit, _)| *edit);
    if let Some(previous) = previous {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(HWND(previous as *mut _));
        }
    }

    let width = (rect.right - rect.left).max(48);
    let height = (rect.bottom - rect.top).max(16);
    let text: Vec<u16> = initial_text
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let style = WINDOW_STYLE(
        WindowsAndMessaging::WS_CHILD.0
            | WindowsAndMessaging::WS_VISIBLE.0
            | WindowsAndMessaging::WS_CLIPSIBLINGS.0
            | ES_AUTOHSCROLL as u32,
    );
    let edit = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("EDIT"),
            PCWSTR(text.as_ptr()),
            style,
            rect.left,
            rect.top,
            width,
            height,
            Some(host_hwnd),
            None,
            None,
            None,
        )
    };
    let Ok(edit) = edit else {
        return false;
    };

    let font = create_inline_edit_font(edit);
    if !font.is_invalid() {
        unsafe {
            let _ = WindowsAndMessaging::SendMessageW(
                edit,
                WindowsAndMessaging::WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
    }

    let original_proc = unsafe {
        WindowsAndMessaging::SetWindowLongPtrW(
            edit,
            WindowsAndMessaging::GWLP_WNDPROC,
            inline_edit_proc as *const () as usize as isize,
        )
    };
    let state = Box::new(InlineEditState {
        original_proc,
        host: host_hwnd.0 as isize,
        font,
        on_commit,
        finished: false,
    });
    unsafe {
        WindowsAndMessaging::SetWindowLongPtrW(
            edit,
            WindowsAndMessaging::GWLP_USERDATA,
            Box::into_raw(state) as isize,
        );
    }

    active_edits().insert(
        host_hwnd.0 as isize,
        (
            edit.0 as isize,
            RECT {
                left: rect.left,
                top: rect.top,
                right: rect.left + width,
                bottom: rect.top + height,
            },
        ),
    );

    unsafe {
        let _ = SetFocus(Some(edit));
        // Select-all so typing replaces the previous title outright.
        let _ =
            WindowsAndMessaging::SendMessageW(edit, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
    }
    true
}

/// Chrome text font for the editor (same font `chrome::draw_text` uses).
fn create_inline_edit_font(edit: HWND) -> HFONT {
    unsafe {
        let hdc = GetDC(Some(edit));
        let font = super::chrome::create_chrome_text_font(hdc);
        if !hdc.is_invalid() {
            let _ = ReleaseDC(Some(edit), hdc);
        }
        font
    }
}

fn inline_edit_state(hwnd: HWND) -> *mut InlineEditState {
    let raw =
        unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA) };
    raw as *mut InlineEditState
}

/// Reads the control's current text.
fn inline_edit_text(hwnd: HWND) -> String {
    unsafe {
        let length = WindowsAndMessaging::GetWindowTextLengthW(hwnd).max(0) as usize;
        let mut buffer = vec![0u16; length + 1];
        let copied = WindowsAndMessaging::GetWindowTextW(hwnd, &mut buffer).max(0) as usize;
        String::from_utf16_lossy(&buffer[..copied.min(length)])
    }
}

/// Ends the edit: commits (unless cancelled), destroys the control, and
/// for keyboard-driven ends, returns focus to the host so terminal input
/// resumes without an extra click. Focus-loss ends leave focus where the
/// user put it.
fn finish_inline_edit(hwnd: HWND, commit: bool, refocus_host: bool) {
    let state = inline_edit_state(hwnd);
    if state.is_null() || unsafe { (*state).finished } {
        return;
    }
    unsafe {
        (*state).finished = true;
    }
    if commit {
        let text = inline_edit_text(hwnd);
        let on_commit = unsafe { Arc::clone(&(*state).on_commit) };
        on_commit(text);
    }
    let host = unsafe { (*state).host };
    unsafe {
        let _ = WindowsAndMessaging::DestroyWindow(hwnd);
        if refocus_host {
            let _ = SetFocus(Some(HWND(host as *mut _)));
        }
    }
}

/// Subclass procedure of the EDIT child: Enter commits, Esc cancels,
/// focus loss commits; `WM_NCDESTROY` unsubclasses and frees all state.
unsafe extern "system" fn inline_edit_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state = inline_edit_state(hwnd);
    if state.is_null() {
        return unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let original = unsafe { (*state).original_proc };

    match msg {
        WindowsAndMessaging::WM_KEYDOWN if wparam.0 == VK_RETURN.0 as usize => {
            finish_inline_edit(hwnd, true, true);
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_KEYDOWN if wparam.0 == VK_ESCAPE.0 as usize => {
            finish_inline_edit(hwnd, false, true);
            return LRESULT(0);
        }
        // Swallow the translated Enter/Esc characters (message beep).
        WindowsAndMessaging::WM_CHAR if wparam.0 == 0x0d || wparam.0 == 0x1b => {
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_KILLFOCUS => {
            finish_inline_edit(hwnd, true, false);
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_NCDESTROY => {
            let state = unsafe { Box::from_raw(state) };
            unsafe {
                WindowsAndMessaging::SetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA, 0);
                WindowsAndMessaging::SetWindowLongPtrW(
                    hwnd,
                    WindowsAndMessaging::GWLP_WNDPROC,
                    state.original_proc,
                );
                if !state.font.is_invalid() {
                    let _ = DeleteObject(HGDIOBJ(state.font.0));
                }
            }
            // Forget the edit (only if this control is still the host's
            // registered one) and repaint the chrome underneath it.
            let host = HWND(state.host as *mut _);
            {
                let mut edits = active_edits();
                if edits
                    .get(&state.host)
                    .is_some_and(|(edit, _)| *edit == hwnd.0 as isize)
                {
                    edits.remove(&state.host);
                }
            }
            unsafe {
                let _ = InvalidateRect(Some(host), None, false);
            }
            return unsafe { call_original(original, hwnd, msg, wparam, lparam) };
        }
        _ => {}
    }
    unsafe { call_original(original, hwnd, msg, wparam, lparam) }
}

/// Calls the EDIT class procedure captured at subclass time.
unsafe fn call_original(
    original: isize,
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let proc: WNDPROC = unsafe { std::mem::transmute(original) };
    unsafe { WindowsAndMessaging::CallWindowProcW(proc, hwnd, msg, wparam, lparam) }
}
