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
//! `lingxia_windows_contract::post_to_window_thread`; the commit
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
#[cfg(feature = "terminal-runtime")]
use windows::Win32::Graphics::Gdi::{BeginPaint, DT_CENTER, EndPaint, PAINTSTRUCT};
use windows::Win32::Graphics::Gdi::{
    ExcludeClipRect, GetDC, HDC, HFONT, InvalidateRect, ReleaseDC,
};
#[cfg(feature = "terminal-runtime")]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{SetFocus, VK_ESCAPE, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::{
    self, ES_AUTOHSCROLL, WINDOW_EX_STYLE, WINDOW_STYLE, WNDPROC,
};
#[cfg(feature = "terminal-runtime")]
use windows::Win32::UI::WindowsAndMessaging::{CREATESTRUCTW, WNDCLASSW, WS_CLIPCHILDREN};
#[cfg(feature = "terminal-runtime")]
use windows::Win32::{
    Foundation::COLORREF,
    Graphics::Gdi::{CreateSolidBrush, DeleteObject, HBRUSH, HGDIOBJ, SetBkColor, SetTextColor},
};
use windows::core::{PCWSTR, w};

/// Commit callback of an inline edit; receives the final text on Enter or
/// focus loss. Runs on the host window's UI thread.
pub type InlineEditCommit = Arc<dyn Fn(String) + Send + Sync>;

/// Callbacks for the terminal's persistent find field. Text changes search
/// immediately, Enter/Shift+Enter move between results, and Escape closes it.
pub struct SearchEditCallbacks {
    pub on_change: Arc<dyn Fn(String, bool, bool) + Send + Sync>,
    pub on_navigate: Arc<dyn Fn(i32) + Send + Sync>,
    pub on_close: Arc<dyn Fn() + Send + Sync>,
}

/// `EM_SETSEL` (select text range) lives in `Win32::UI::Controls` in the
/// windows crate; defined locally to avoid pulling the whole feature.
const EM_SETSEL: u32 = 0x00b1;
#[cfg(feature = "terminal-runtime")]
const EM_SETMARGINS: u32 = 0x00d3;
#[cfg(feature = "terminal-runtime")]
const EC_LEFTMARGIN: usize = 0x0001;
#[cfg(feature = "terminal-runtime")]
const EC_RIGHTMARGIN: usize = 0x0002;
#[cfg(feature = "terminal-runtime")]
const SEARCH_OVERLAY_CLASS: PCWSTR = w!("LingXiaTerminalSearchOverlay");
#[cfg(feature = "terminal-runtime")]
const SEARCH_FIELD: RECT = RECT {
    left: 12,
    top: 9,
    right: 157,
    bottom: 37,
};

#[cfg(feature = "terminal-runtime")]
struct SearchEditAppearance {
    background: COLORREF,
    foreground: COLORREF,
    brush: HBRUSH,
}

/// Per-control state stashed in the EDIT child's `GWLP_USERDATA`.
struct InlineEditState {
    /// The EDIT class window procedure being subclassed.
    original_proc: isize,
    /// Raw handle of the host (parent) window.
    host: isize,
    on_commit: Option<InlineEditCommit>,
    search: Option<SearchEditCallbacks>,
    search_overlay: isize,
    search_case_sensitive: bool,
    search_whole_word: bool,
    #[cfg(feature = "terminal-runtime")]
    search_active: Option<usize>,
    #[cfg(feature = "terminal-runtime")]
    search_total: u64,
    #[cfg(feature = "terminal-runtime")]
    search_appearance: Option<SearchEditAppearance>,
    last_search_text: Option<String>,
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
    begin_inline_edit_impl(host_hwnd, rect, initial_text, on_commit, true)
}

fn begin_inline_edit_impl(
    host_hwnd: HWND,
    rect: RECT,
    initial_text: &str,
    on_commit: InlineEditCommit,
    focus_immediately: bool,
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
        on_commit: Some(on_commit),
        search: None,
        search_overlay: 0,
        search_case_sensitive: false,
        search_whole_word: false,
        #[cfg(feature = "terminal-runtime")]
        search_active: None,
        #[cfg(feature = "terminal-runtime")]
        search_total: 0,
        #[cfg(feature = "terminal-runtime")]
        search_appearance: None,
        last_search_text: None,
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
        // The terminal body is a sibling GPU child window. Keep the editor
        // above it or the control exists and receives focus but remains
        // completely hidden behind the composed terminal surface.
        let _ = WindowsAndMessaging::SetWindowPos(
            edit,
            Some(WindowsAndMessaging::HWND_TOP),
            0,
            0,
            0,
            0,
            WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
        if focus_immediately {
            let _ = SetFocus(Some(edit));
            // Select-all so typing replaces the previous title outright.
            let _ = WindowsAndMessaging::SendMessageW(
                edit,
                EM_SETSEL,
                Some(WPARAM(0)),
                Some(LPARAM(-1)),
            );
        }
    }
    true
}

/// Opens a live terminal search field. Unlike [`begin_inline_edit`], Enter
/// navigates without destroying the control; Escape or focus loss closes it.
#[cfg(feature = "terminal-runtime")]
pub fn begin_search_edit(
    host_hwnd: HWND,
    rect: RECT,
    initial_text: &str,
    callbacks: SearchEditCallbacks,
) -> bool {
    // Create the native editor without focusing it until the self-painted card
    // is above it; otherwise its focus transition can race the first frame.
    let started = begin_inline_edit_impl(host_hwnd, rect, initial_text, Arc::new(|_| {}), false);
    if !started {
        return false;
    }
    let edit = active_edits()
        .get(&(host_hwnd.0 as isize))
        .map(|(edit, _)| *edit);
    let Some(edit) = edit else {
        return false;
    };
    let state = inline_edit_state(HWND(edit as *mut _));
    if state.is_null() {
        return false;
    }
    unsafe {
        (*state).on_commit = None;
        (*state).search = Some(callbacks);
        (*state).last_search_text = Some(initial_text.to_string());

        let chrome = super::terminal_grid::surface_chrome();
        // Match the EDIT brush to the card's painted field. A different fill
        // becomes visible whenever the native caret invalidates the control.
        let background = blend_rgb(chrome.surface, chrome.header, 76);
        let background = rgb_to_colorref(background);
        let foreground = rgb_to_colorref(chrome.text);
        let brush = CreateSolidBrush(background);
        if !brush.is_invalid() {
            (*state).search_appearance = Some(SearchEditAppearance {
                background,
                foreground,
                brush,
            });
        }

        let margin = 10isize | (10isize << 16);
        let _ = WindowsAndMessaging::SendMessageW(
            HWND(edit as *mut _),
            EM_SETMARGINS,
            Some(WPARAM(EC_LEFTMARGIN | EC_RIGHTMARGIN)),
            Some(LPARAM(margin)),
        );
        let (_, rect) = active_edits()
            .get(&(host_hwnd.0 as isize))
            .copied()
            .unwrap_or((edit, RECT::default()));
        if let Some(overlay) = create_search_overlay(host_hwnd, HWND(edit as *mut _), rect) {
            (*state).search_overlay = overlay.0 as isize;
            // Keep EDIT as a host sibling behind the self-painted card. It
            // retains native keyboard, selection, clipboard and IME behavior;
            // the card paints the query so D3D child composition cannot hide
            // the glyphs while leaving the caret visible.
            let _ = WindowsAndMessaging::SetWindowPos(
                overlay,
                Some(WindowsAndMessaging::HWND_TOP),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                WindowsAndMessaging::SWP_NOACTIVATE | WindowsAndMessaging::SWP_SHOWWINDOW,
            );
            let _ = SetFocus(Some(HWND(edit as *mut _)));
        }
    }
    true
}

#[cfg(feature = "terminal-runtime")]
fn create_search_overlay(host: HWND, edit: HWND, rect: RECT) -> Option<HWND> {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| unsafe {
        let instance = GetModuleHandleW(None).ok();
        let class = WNDCLASSW {
            hInstance: instance.map(|module| module.into()).unwrap_or_default(),
            lpszClassName: SEARCH_OVERLAY_CLASS,
            lpfnWndProc: Some(search_overlay_proc),
            ..Default::default()
        };
        let _ = WindowsAndMessaging::RegisterClassW(&class);
    });
    let instance = unsafe { GetModuleHandleW(None).ok() };
    unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            SEARCH_OVERLAY_CLASS,
            PCWSTR::null(),
            WINDOW_STYLE(
                WindowsAndMessaging::WS_CHILD.0
                    | WindowsAndMessaging::WS_VISIBLE.0
                    | WS_CLIPCHILDREN.0,
            ),
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            Some(host),
            None,
            instance.map(|module| module.into()),
            Some(edit.0),
        )
        .ok()
    }
}

#[cfg(feature = "terminal-runtime")]
unsafe extern "system" fn search_overlay_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WindowsAndMessaging::WM_NCCREATE {
        let create = lparam.0 as *const CREATESTRUCTW;
        if !create.is_null() {
            unsafe {
                WindowsAndMessaging::SetWindowLongPtrW(
                    hwnd,
                    WindowsAndMessaging::GWLP_USERDATA,
                    (*create).lpCreateParams as isize,
                );
            }
        }
    }
    let edit = HWND(unsafe {
        WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
    } as *mut _);
    match msg {
        WindowsAndMessaging::WM_ERASEBKGND => LRESULT(1),
        WindowsAndMessaging::WM_CTLCOLOREDIT => {
            control_color(HWND(lparam.0 as *mut _), HDC(wparam.0 as *mut _)).unwrap_or(LRESULT(0))
        }
        WindowsAndMessaging::WM_PAINT => {
            paint_search_overlay(hwnd, edit);
            LRESULT(0)
        }
        WindowsAndMessaging::WM_LBUTTONUP => {
            let point = ((lparam.0 as i16) as i32, ((lparam.0 >> 16) as i16) as i32);
            handle_search_overlay_click(hwnd, edit, point);
            LRESULT(0)
        }
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(feature = "terminal-runtime")]
fn search_control_rect(index: usize) -> RECT {
    const RECTS: [RECT; 8] = [
        SEARCH_FIELD,
        RECT {
            left: 163,
            top: 9,
            right: 195,
            bottom: 37,
        },
        RECT {
            left: 199,
            top: 9,
            right: 231,
            bottom: 37,
        },
        RECT {
            left: 235,
            top: 9,
            right: 275,
            bottom: 37,
        },
        RECT {
            left: 279,
            top: 9,
            right: 305,
            bottom: 37,
        },
        RECT {
            left: 307,
            top: 9,
            right: 333,
            bottom: 37,
        },
        RECT {
            left: 339,
            top: 9,
            right: 365,
            bottom: 37,
        },
        RECT {
            left: 371,
            top: 9,
            right: 399,
            bottom: 37,
        },
    ];
    RECTS[index]
}

#[cfg(feature = "terminal-runtime")]
fn paint_search_overlay(hwnd: HWND, edit: HWND) {
    let mut paint = PAINTSTRUCT::default();
    let hdc = unsafe { BeginPaint(hwnd, &mut paint) };
    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    let chrome = super::terminal_grid::surface_chrome();
    super::chrome::fill_round_rect_aa(
        hdc,
        client,
        11,
        blend_rgb(chrome.header, chrome.surface, 70),
    );
    super::chrome::stroke_round_rect_aa(
        hdc,
        client,
        11,
        blend_rgb(chrome.separator, chrome.text, 80),
    );
    super::chrome::fill_round_rect_aa(
        hdc,
        SEARCH_FIELD,
        8,
        blend_rgb(chrome.surface, chrome.header, 76),
    );
    let state = inline_edit_state(edit);
    if let Some(state) = unsafe { state.as_ref() } {
        let query_rect = RECT {
            left: SEARCH_FIELD.left + 10,
            top: SEARCH_FIELD.top,
            right: SEARCH_FIELD.right - 6,
            bottom: SEARCH_FIELD.bottom,
        };
        super::chrome::draw_text_antialiased(
            hdc,
            state.last_search_text.as_deref().unwrap_or_default(),
            query_rect,
            chrome.text,
            windows::Win32::Graphics::Gdi::DT_LEFT,
        );
        for (index, label) in ["Aa", "ab", "", "↑", "↓", "⌫", "×"].into_iter().enumerate() {
            let rect = search_control_rect(index + 1);
            let selected = (index == 0 && state.search_case_sensitive)
                || (index == 1 && state.search_whole_word);
            if selected {
                super::chrome::fill_round_rect_aa(
                    hdc,
                    rect,
                    7,
                    blend_rgb(chrome.text, chrome.surface, 22),
                );
            }
            let text = if index == 2 {
                match (state.search_active, state.search_total) {
                    (Some(active), total) if total > 0 => format!("{}/{}", active + 1, total),
                    (_, 0) => "0/0".to_string(),
                    _ => format!("0/{}", state.search_total),
                }
            } else {
                label.to_string()
            };
            super::chrome::draw_text_antialiased(
                hdc,
                &text,
                rect,
                if selected {
                    chrome.text
                } else {
                    chrome.text_muted
                },
                DT_CENTER,
            );
        }
    }
    unsafe {
        let _ = EndPaint(hwnd, &paint);
    }
}

#[cfg(feature = "terminal-runtime")]
fn handle_search_overlay_click(hwnd: HWND, edit: HWND, point: (i32, i32)) {
    let hit = (1..8).find(|index| {
        let rect = search_control_rect(*index);
        point.0 >= rect.left && point.0 < rect.right && point.1 >= rect.top && point.1 < rect.bottom
    });
    let state = inline_edit_state(edit);
    let Some(state) = (unsafe { state.as_mut() }) else {
        return;
    };
    match hit {
        Some(1) => {
            state.search_case_sensitive = !state.search_case_sensitive;
            notify_search_changed_force(edit, state);
        }
        Some(2) => {
            state.search_whole_word = !state.search_whole_word;
            notify_search_changed_force(edit, state);
        }
        Some(4) => {
            if let Some(search) = state.search.as_ref() {
                (search.on_navigate)(-1);
            }
        }
        Some(5) => {
            if let Some(search) = state.search.as_ref() {
                (search.on_navigate)(1);
            }
        }
        Some(6) => unsafe {
            let _ = WindowsAndMessaging::SetWindowTextW(edit, w!(""));
            notify_search_changed_force(edit, state);
        },
        Some(7) => {
            finish_inline_edit(edit, false, true);
            return;
        }
        _ => {}
    }
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, false);
        let _ = SetFocus(Some(edit));
    }
}

#[cfg(feature = "terminal-runtime")]
pub(crate) fn update_search_status(host: HWND, status: (Option<usize>, u64)) {
    let host = host.0 as isize;
    lingxia_windows_contract::post_to_window_thread(
        host,
        Box::new(move || {
            let edit = active_edits().get(&host).map(|entry| entry.0);
            let Some(edit) = edit else {
                return;
            };
            let state = inline_edit_state(HWND(edit as *mut _));
            if let Some(state) = unsafe { state.as_mut() } {
                state.search_active = status.0;
                state.search_total = status.1;
                if state.search_overlay != 0 {
                    unsafe {
                        let _ =
                            InvalidateRect(Some(HWND(state.search_overlay as *mut _)), None, false);
                    }
                }
            }
        }),
    );
}

/// Close the persistent search popover owned by `host`. Returns whether an
/// active search was handled, so the terminal does not also receive Escape.
#[cfg(feature = "terminal-runtime")]
pub(crate) fn close_search_edit(host: HWND) -> bool {
    let edit = active_edits().get(&(host.0 as isize)).map(|entry| entry.0);
    let Some(edit) = edit else {
        return false;
    };
    let edit = HWND(edit as *mut _);
    let state = inline_edit_state(edit);
    if unsafe { state.as_ref() }.is_none_or(|state| state.search.is_none()) {
        return false;
    }
    finish_inline_edit(edit, false, true);
    true
}

/// Supplies the themed colors for an active search edit when its host receives
/// `WM_CTLCOLOREDIT`.
#[cfg(feature = "terminal-runtime")]
pub(crate) fn control_color(edit: HWND, hdc: HDC) -> Option<LRESULT> {
    let state = inline_edit_state(edit);
    let appearance = unsafe { state.as_ref()?.search_appearance.as_ref()? };
    unsafe {
        let _ = SetTextColor(hdc, appearance.foreground);
        let _ = SetBkColor(hdc, appearance.background);
    }
    Some(LRESULT(appearance.brush.0 as isize))
}

#[cfg(feature = "terminal-runtime")]
fn rgb_to_colorref(rgb: u32) -> COLORREF {
    let r = (rgb >> 16) & 0xff;
    let g = (rgb >> 8) & 0xff;
    let b = rgb & 0xff;
    COLORREF(r | (g << 8) | (b << 16))
}

#[cfg(feature = "terminal-runtime")]
fn blend_rgb(foreground: u32, background: u32, foreground_percent: u32) -> u32 {
    let blend = |shift: u32| {
        let foreground = (foreground >> shift) & 0xff;
        let background = (background >> shift) & 0xff;
        ((foreground * foreground_percent + background * (100 - foreground_percent)) / 100) << shift
    };
    blend(16) | blend(8) | blend(0)
}

/// Chrome text font for the editor (same font `chrome::draw_text` uses).
/// Shared cache entry - not owned by the control.
fn create_inline_edit_font(edit: HWND) -> HFONT {
    unsafe {
        let hdc = GetDC(Some(edit));
        let font = super::chrome::chrome_text_font(hdc);
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

fn notify_search_changed(hwnd: HWND, state: *mut InlineEditState) {
    let text = inline_edit_text(hwnd);
    let changed = unsafe { (*state).last_search_text.as_deref() != Some(text.as_str()) };
    if !changed {
        return;
    }
    unsafe {
        (*state).last_search_text = Some(text.clone());
        if (*state).search_overlay != 0 {
            let _ = InvalidateRect(Some(HWND((*state).search_overlay as *mut _)), None, false);
        }
    }
    if let Some(search) = unsafe { (*state).search.as_ref() } {
        (search.on_change)(text, unsafe { (*state).search_case_sensitive }, unsafe {
            (*state).search_whole_word
        });
    }
}

#[cfg(feature = "terminal-runtime")]
fn notify_search_changed_force(hwnd: HWND, state: &mut InlineEditState) {
    let text = inline_edit_text(hwnd);
    state.last_search_text = Some(text.clone());
    if let Some(search) = state.search.as_ref() {
        (search.on_change)(text, state.search_case_sensitive, state.search_whole_word);
    }
    if state.search_overlay != 0 {
        unsafe {
            let _ = InvalidateRect(Some(HWND(state.search_overlay as *mut _)), None, false);
        }
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
        let on_commit = unsafe { (*state).on_commit.as_ref().map(Arc::clone) };
        if let Some(on_commit) = on_commit {
            on_commit(text);
        }
    }
    let on_close = unsafe {
        (*state)
            .search
            .as_ref()
            .map(|search| Arc::clone(&search.on_close))
    };
    if let Some(on_close) = on_close {
        on_close();
    }
    let host = unsafe { (*state).host };
    let overlay = unsafe { (*state).search_overlay };
    unsafe {
        if overlay != 0 {
            let _ = WindowsAndMessaging::DestroyWindow(HWND(overlay as *mut _));
        }
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
            if let Some(search) = unsafe { (*state).search.as_ref() } {
                let backwards = unsafe {
                    windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(
                        windows::Win32::UI::Input::KeyboardAndMouse::VK_SHIFT.0 as i32,
                    ) < 0
                };
                (search.on_navigate)(if backwards { -1 } else { 1 });
                return LRESULT(0);
            }
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
            // Search is a persistent popover, not a transient rename field.
            // Clicking the terminal body or one of the overlay's own buttons
            // may move focus away from the EDIT, but must not dismiss it.
            if unsafe { (*state).search.is_some() } {
                return unsafe { call_original(original, hwnd, msg, wparam, lparam) };
            }
            finish_inline_edit(hwnd, true, false);
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_NCDESTROY => {
            let state = unsafe { Box::from_raw(state) };
            #[cfg(feature = "terminal-runtime")]
            if let Some(appearance) = state.search_appearance.as_ref() {
                unsafe {
                    let _ = DeleteObject(HGDIOBJ(appearance.brush.0));
                }
            }
            unsafe {
                WindowsAndMessaging::SetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA, 0);
                WindowsAndMessaging::SetWindowLongPtrW(
                    hwnd,
                    WindowsAndMessaging::GWLP_WNDPROC,
                    state.original_proc,
                );
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
    let result = unsafe { call_original(original, hwnd, msg, wparam, lparam) };
    // The original EDIT proc may destroy the control for messages we do not
    // own. Never dereference its userdata after that teardown.
    if msg == WindowsAndMessaging::WM_NCDESTROY || inline_edit_state(hwnd) != state {
        return result;
    }
    let text_may_have_changed = matches!(
        msg,
        WindowsAndMessaging::WM_CHAR
            | WindowsAndMessaging::WM_KEYUP
            | WindowsAndMessaging::WM_PASTE
            | WindowsAndMessaging::WM_CUT
            | WindowsAndMessaging::WM_CLEAR
            | WindowsAndMessaging::WM_UNDO
    );
    if text_may_have_changed && unsafe { (*state).search.is_some() } {
        notify_search_changed(hwnd, state);
    }
    result
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
