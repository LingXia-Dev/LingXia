//! Host-owned recovery chrome over a failed page: a message, Back when the page has
//! somewhere to go, and the core's Retry, Reload or Keep waiting. Page content cannot
//! replace it.
use crate::view;
use lingxia_view::session::host::{Failure, RecoveryAction, RecoveryMessage};
use std::cell::RefCell;
use std::collections::HashMap;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateSolidBrush, DT_CALCRECT, DT_CENTER, DT_WORDBREAK, DeleteObject,
    DrawTextW, EndPaint, FillRect, GetDC, HBRUSH, HDC, HFONT, HGDIOBJ, InvalidateRect, PAINTSTRUCT,
    ReleaseDC, SelectObject, SetBkColor, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    BS_PUSHBUTTON, CreateWindowExW, DefWindowProcW, DestroyWindow, ES_CENTER, ES_MULTILINE,
    ES_READONLY, GetClientRect, GetParent, HMENU, HWND_TOP, IDC_ARROW, LoadCursorW, PostMessageW,
    RegisterClassW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SendMessageW, SetWindowPos, SetWindowTextW,
    ShowWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_COMMAND, WM_CTLCOLORSTATIC,
    WM_ERASEBKGND, WM_PAINT, WM_SETFONT, WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS, WS_TABSTOP,
    WS_VISIBLE,
};
use windows::core::{HSTRING, w};

const BUTTON_ID: usize = 1;
const BACK_ID: usize = 2;
const BACK_REFUSED: u32 = WM_APP + 1;
/// Logical pixels.
const PADDING: f32 = 16.0;
const TEXT_WIDTH: f32 = 360.0;
const FONT: f32 = 15.0;
const BUTTON: [f32; 2] = [140.0, 32.0];
/// The error under the message: a smaller monospaced face, at most this many lines.
const DETAIL_FONT: f32 = 12.0;
const DETAIL_LINES: i32 = 6;
const DETAIL_WIDTH: f32 = 480.0;

/// Localized chrome text, the key the platform localizer knows and its English fallback.
pub(crate) type Text<'a> = &'a dyn Fn(&str, &str) -> String;
/// Leave the failed page; false when it could not, which re-enables the button.
pub(crate) type Back = std::sync::Arc<dyn Fn() -> bool + Send + Sync>;

struct Panel {
    window: isize,
    button: isize,
    back: isize,
    leave: Option<Back>,
    message: Vec<u16>,
    covers: bool,
    dark: bool,
    font: HFONT,
    /// A dev build's error text: a read-only edit, so it can be selected and copied.
    detail: isize,
    detail_text: Vec<u16>,
    detail_font: HFONT,
    /// Its laid-out height in pixels, which the message's centring includes.
    detail_height: i32,
    /// The button's label while its action runs; `None` when the action clears the panel.
    retrying: Option<String>,
}

thread_local! {
    /// Each page window's panel.
    static PANELS: RefCell<HashMap<isize, Panel>> = RefCell::new(HashMap::new());
}

/// Show `failure`'s chrome over `page`, or clear it with `None`.
pub(crate) fn show(
    page: isize,
    failure: Option<&Failure>,
    dark: bool,
    leave: Option<Back>,
    text: Text,
) {
    clear(page);
    let Some(failure) = failure else { return };
    let message = match failure.message {
        RecoveryMessage::UpdateRequired => {
            text("gpu.update_required", "Update the app to open this page.")
        }
        RecoveryMessage::SessionFailed => text(
            "gpu.session_failed",
            "This page stopped responding. Reload to continue. Unsaved input may be lost.",
        ),
        RecoveryMessage::UpdateFailed => text(
            "gpu.update_failed",
            "This page couldn't update. Your previous view is still available.",
        ),
        RecoveryMessage::DisplayFailed => {
            text("gpu.display_failed", "This page couldn't be displayed.")
        }
        RecoveryMessage::SlowLoad => text(
            "gpu.slow_load",
            "This page is taking longer than usual to load.",
        ),
    };
    // Waiting on clears the panel at once, so its button never reads "Retrying…".
    let waits = failure.action == Some(RecoveryAction::Wait);
    let action = failure.action.map(|action| match action {
        RecoveryAction::Retry => text("gpu.retry", "Retry"),
        RecoveryAction::Reload => text("gpu.reload", "Reload page"),
        RecoveryAction::Wait => text("gpu.keep_waiting", "Keep waiting"),
    });
    let class = w!("LingXiaGpuRecovery");
    // SAFETY: plain Win32 calls on this thread's windows; a repeat registration fails harmlessly.
    unsafe {
        let Ok(instance) = GetModuleHandleW(None) else {
            return;
        };
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: class,
            ..Default::default()
        });
        let Ok(window) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            None,
            // Shown once its buttons are in: a reader never meets the message alone.
            WS_CHILD | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            Some(HWND(page as *mut _)),
            None,
            Some(instance.into()),
            None,
        ) else {
            return;
        };
        let scale = scale(page);
        let font = CreateFontW(
            -(FONT * scale).round() as i32,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            0,
            w!("Segoe UI"),
        );
        // The window's text is the message, so screen readers and automation read it.
        let _ = SetWindowTextW(window, &HSTRING::from(message.as_str()));
        let make = |label: String, id: usize| {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                &HSTRING::from(label),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                0,
                0,
                0,
                0,
                Some(window),
                Some(HMENU(id as *mut _)),
                Some(instance.into()),
                None,
            )
            .ok()
            .map_or(0, |button| {
                SendMessageW(
                    button,
                    WM_SETFONT,
                    Some(WPARAM(font.0 as usize)),
                    Some(LPARAM(1)),
                );
                button.0 as isize
            })
        };
        let back = if leave.is_some() {
            make(text("gpu.back", "Go back"), BACK_ID)
        } else {
            0
        };
        let button = action.map_or(0, |label| make(label, BUTTON_ID));
        let detail_text = failure.detail.as_deref().unwrap_or_default();
        let detail_font = if detail_text.is_empty() {
            HFONT::default()
        } else {
            CreateFontW(
                -(DETAIL_FONT * scale).round() as i32,
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                0,
                w!("Consolas"),
            )
        };
        let detail = if detail_text.is_empty() {
            0
        } else {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EDIT"),
                &HSTRING::from(detail_text),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WINDOW_STYLE((ES_MULTILINE | ES_READONLY | ES_CENTER) as u32),
                0,
                0,
                0,
                0,
                Some(window),
                None,
                Some(instance.into()),
                None,
            )
            .ok()
            .map_or(0, |edit| {
                SendMessageW(
                    edit,
                    WM_SETFONT,
                    Some(WPARAM(detail_font.0 as usize)),
                    Some(LPARAM(1)),
                );
                edit.0 as isize
            })
        };
        PANELS.with(|panels| {
            panels.borrow_mut().insert(
                page,
                Panel {
                    window: window.0 as isize,
                    button,
                    back,
                    leave,
                    message: message.encode_utf16().collect(),
                    covers: failure.covers_page(),
                    dark,
                    font,
                    detail,
                    detail_text: detail_text.encode_utf16().collect(),
                    detail_font,
                    detail_height: 0,
                    retrying: (!waits).then(|| text("gpu.retrying", "Retrying…")),
                },
            )
        });
    }
    layout(page);
    if let Some(window) = PANELS.with(|panels| panels.borrow().get(&page).map(|p| p.window)) {
        // SAFETY: this thread's panel.
        unsafe {
            let _ = ShowWindow(HWND(window as *mut _), SW_SHOWNOACTIVATE);
        }
    }
    if failure.covers_page()
        && let Some(page) = view::page_for(page)
    {
        page.settled();
    }
}

fn clear(page: isize) {
    let Some(panel) = PANELS.with(|panels| panels.borrow_mut().remove(&page)) else {
        return;
    };
    // SAFETY: this thread's windows and font.
    unsafe {
        let _ = DestroyWindow(HWND(panel.window as *mut _));
        let _ = DeleteObject(HGDIOBJ(panel.font.0));
        if !panel.detail_font.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(panel.detail_font.0));
        }
    }
    crate::composition::clip_above(page, None);
}

/// Whether `page` shows a panel over all of it: a failed page that presents no frames.
pub(crate) fn covers(page: isize) -> bool {
    PANELS.with(|panels| panels.borrow().get(&page).is_some_and(|panel| panel.covers))
}

/// The page window went away: its panel's windows went with it.
pub(crate) fn forget(page: isize) {
    if let Some(panel) = PANELS.with(|panels| panels.borrow_mut().remove(&page)) {
        // SAFETY: fonts this thread made.
        let _ = unsafe { DeleteObject(HGDIOBJ(panel.font.0)) };
        if !panel.detail_font.is_invalid() {
            let _ = unsafe { DeleteObject(HGDIOBJ(panel.detail_font.0)) };
        }
    }
}

fn scale(page: isize) -> f32 {
    if let Some(page) = crate::view::page_for(page) {
        return page.shown_scale();
    }
    // SAFETY: a window of this thread.
    let dpi = unsafe { GetDpiForWindow(HWND(page as *mut _)) };
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

/// Fit the panel to its page: the whole page when it covers it, else a bottom banner.
pub(crate) fn layout(page: isize) {
    let Some((
        window,
        button,
        back,
        covers,
        font,
        mut message,
        detail,
        detail_font,
        mut detail_text,
    )) = PANELS.with(|panels| {
        panels.borrow().get(&page).map(|p| {
            (
                p.window,
                p.button,
                p.back,
                p.covers,
                p.font,
                p.message.clone(),
                p.detail,
                p.detail_font,
                p.detail_text.clone(),
            )
        })
    })
    else {
        return;
    };
    let scale = scale(page);
    let px = |logical: f32| (logical * scale).round() as i32;
    let (width, height) = view::client_size(page);
    let (width, height) = (width as i32, height as i32);
    let text_width = px(TEXT_WIDTH).min(width - 2 * px(PADDING)).max(1);
    let detail_width = px(DETAIL_WIDTH).min(width - 2 * px(PADDING)).max(1);
    let mut measured = RECT {
        right: text_width,
        ..Default::default()
    };
    let mut detail_measured = RECT {
        right: detail_width,
        ..Default::default()
    };
    let mut line = RECT::default();
    // SAFETY: a screen DC measures the text with the panel's fonts.
    unsafe {
        let dc = GetDC(None);
        let old = SelectObject(dc, HGDIOBJ(font.0));
        DrawTextW(
            dc,
            &mut message,
            &mut measured,
            DT_CALCRECT | DT_CENTER | DT_WORDBREAK,
        );
        if detail != 0 {
            SelectObject(dc, HGDIOBJ(detail_font.0));
            DrawTextW(
                dc,
                &mut detail_text,
                &mut detail_measured,
                DT_CALCRECT | DT_CENTER | DT_WORDBREAK,
            );
            DrawTextW(dc, &mut [b'M' as u16], &mut line, DT_CALCRECT);
        }
        SelectObject(dc, old);
        ReleaseDC(None, dc);
    }
    let text_height = measured.bottom;
    let gap = px(12.0);
    let detail_height = if detail != 0 {
        detail_measured
            .bottom
            .min(DETAIL_LINES * line.bottom.max(1))
    } else {
        0
    };
    let detail_block = if detail != 0 {
        px(8.0) + detail_height
    } else {
        0
    };
    let [bw, bh] = BUTTON.map(px);
    let stack = text_height
        + detail_block
        + if button != 0 || back != 0 {
            gap + bh
        } else {
            0
        };
    let panel_height = if covers {
        height
    } else {
        (stack + 2 * px(PADDING)).min(height)
    };
    let top = (panel_height - stack) / 2;
    PANELS.with(|panels| {
        if let Some(panel) = panels.borrow_mut().get_mut(&page) {
            panel.message = message;
            panel.detail_height = detail_height;
        }
    });
    // SAFETY: this thread's windows.
    unsafe {
        let _ = SetWindowPos(
            HWND(window as *mut _),
            Some(HWND_TOP),
            0,
            height - panel_height,
            width,
            panel_height,
            SWP_NOACTIVATE,
        );
        // One button centres; with two, Back sits on the leading side.
        let row: Vec<isize> = [back, button].into_iter().filter(|b| *b != 0).collect();
        let row_width = row.len() as i32 * bw + (row.len() as i32 - 1).max(0) * gap;
        for (i, button) in row.into_iter().enumerate() {
            let _ = SetWindowPos(
                HWND(button as *mut _),
                None,
                (width - row_width) / 2 + i as i32 * (bw + gap),
                top + text_height + detail_block + gap,
                bw,
                bh,
                SWP_NOACTIVATE,
            );
        }
        if detail != 0 {
            let _ = SetWindowPos(
                HWND(detail as *mut _),
                None,
                (width - detail_width) / 2,
                top + text_height + px(8.0),
                detail_width,
                detail_height,
                SWP_NOACTIVATE,
            );
        }
        let _ = InvalidateRect(Some(HWND(window as *mut _)), None, true);
    }
    // The page's frames compose above its child windows; they stop where the panel starts.
    crate::composition::clip_above(page, Some((height - panel_height) as f32));
}

fn page_of_panel(window: HWND) -> Option<isize> {
    // SAFETY: reads a window's parent.
    let page = unsafe { GetParent(window) }.ok()?.0 as isize;
    PANELS.with(|panels| {
        panels
            .borrow()
            .get(&page)
            .filter(|panel| panel.window == window.0 as isize)
            .map(|_| page)
    })
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => LRESULT(1),
        WM_CTLCOLORSTATIC => detail_colors(hwnd, HDC(wparam.0 as *mut _)),
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_COMMAND if wparam.0 & 0xffff == BUTTON_ID => {
            retry(hwnd);
            LRESULT(0)
        }
        WM_COMMAND if wparam.0 & 0xffff == BACK_ID => {
            back(hwnd);
            LRESULT(0)
        }
        BACK_REFUSED => {
            back_refused(hwnd);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn retry(window: HWND) {
    let Some(page) = page_of_panel(window) else {
        return;
    };
    let Some((button, label)) = PANELS.with(|panels| {
        panels
            .borrow()
            .get(&page)
            .map(|panel| (panel.button, panel.retrying.clone()))
    }) else {
        return;
    };
    let button = HWND(button as *mut _);
    if let Some(label) = label {
        // SAFETY: this thread's button.
        unsafe {
            let _ = EnableWindow(button, false);
            let _ = SetWindowTextW(button, &HSTRING::from(label));
        }
    }
    if let Some(page) = view::page_for(page) {
        page.host().retry();
    }
}

fn back(window: HWND) {
    let Some(page) = page_of_panel(window) else {
        return;
    };
    let Some((button, leave)) = PANELS.with(|panels| {
        panels
            .borrow()
            .get(&page)
            .and_then(|panel| Some((panel.back, panel.leave.clone()?)))
    }) else {
        return;
    };
    // SAFETY: this thread's button.
    let _ = unsafe { EnableWindow(HWND(button as *mut _), false) };
    // Off this thread: leaving lays out and captures this page's window, which waits on
    // this thread, so a call from its click handler would block until those time out.
    let panel = window.0 as isize;
    let _ = std::thread::Builder::new()
        .name("lingxia-gpu-recovery-back".into())
        .spawn(move || {
            if !leave() {
                // SAFETY: posting to a window that may be gone fails harmlessly.
                let _ = unsafe {
                    PostMessageW(
                        Some(HWND(panel as *mut _)),
                        BACK_REFUSED,
                        WPARAM(0),
                        LPARAM(0),
                    )
                };
            }
        });
}

/// Leaving failed: the panel still shows, so its Back works again.
fn back_refused(window: HWND) {
    let Some(page) = page_of_panel(window) else {
        return;
    };
    if let Some(button) = PANELS.with(|panels| panels.borrow().get(&page).map(|p| p.back)) {
        // SAFETY: this thread's button.
        let _ = unsafe { EnableWindow(HWND(button as *mut _), true) };
    }
}

/// The panel's surface colours, background then text, as COLORREF values.
fn colors(dark: bool) -> (u32, u32) {
    if dark {
        (0x0020_2020, 0x00f0_f0f0)
    } else {
        (0x00f3_f3f3, 0x001a_1a1a)
    }
}

/// The read-only error text on the panel's plate, in a muted ink.
fn detail_colors(hwnd: HWND, dc: HDC) -> LRESULT {
    let dark = page_of_panel(hwnd)
        .and_then(|page| PANELS.with(|panels| panels.borrow().get(&page).map(|p| p.dark)))
        .unwrap_or(false);
    let (background, _) = colors(dark);
    let muted = if dark { 0x00b4_b4b4 } else { 0x0050_5050 };
    thread_local! {
        static BRUSHES: RefCell<HashMap<u32, isize>> = RefCell::new(HashMap::new());
    }
    // SAFETY: colours on the DC the edit paints with; one brush per colour lives for the thread.
    unsafe {
        SetTextColor(dc, COLORREF(muted));
        SetBkColor(dc, COLORREF(background));
        let brush = BRUSHES.with(|brushes| {
            *brushes
                .borrow_mut()
                .entry(background)
                .or_insert_with(|| CreateSolidBrush(COLORREF(background)).0 as isize)
        });
        LRESULT(HBRUSH(brush as *mut _).0 as isize)
    }
}

fn paint(hwnd: HWND) {
    let Some(page) = page_of_panel(hwnd) else {
        return;
    };
    let Some((mut message, dark, font, buttons, detail)) = PANELS.with(|panels| {
        panels.borrow().get(&page).map(|p| {
            (
                p.message.clone(),
                p.dark,
                p.font,
                p.button != 0 || p.back != 0,
                if p.detail != 0 { p.detail_height } else { -1 },
            )
        })
    }) else {
        return;
    };
    let scale = scale(page);
    let px = |logical: f32| (logical * scale).round() as i32;
    let (background, foreground) = colors(dark);
    // SAFETY: painting this thread's window inside BeginPaint/EndPaint.
    unsafe {
        let mut paint = PAINTSTRUCT::default();
        let dc = BeginPaint(hwnd, &mut paint);
        let mut client = RECT::default();
        let _ = GetClientRect(hwnd, &mut client);
        let brush = CreateSolidBrush(COLORREF(background));
        FillRect(dc, &client, brush);
        let _ = DeleteObject(brush.into());
        let old = SelectObject(dc, HGDIOBJ(font.0));
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, COLORREF(foreground));
        let width = client.right - client.left;
        let text_width = px(TEXT_WIDTH).min(width - 2 * px(PADDING)).max(1);
        let mut measured = RECT {
            right: text_width,
            ..Default::default()
        };
        DrawTextW(
            dc,
            &mut message,
            &mut measured,
            DT_CALCRECT | DT_CENTER | DT_WORDBREAK,
        );
        let gap = px(12.0);
        let detail_block = if detail >= 0 { px(8.0) + detail } else { 0 };
        let stack = measured.bottom + detail_block + if buttons { gap + px(BUTTON[1]) } else { 0 };
        let top = (client.bottom - stack) / 2;
        let left = (width - text_width) / 2;
        let mut area = RECT {
            left,
            top,
            right: left + text_width,
            bottom: top + measured.bottom,
        };
        DrawTextW(dc, &mut message, &mut area, DT_CENTER | DT_WORDBREAK);
        SelectObject(dc, old);
        let _ = EndPaint(hwnd, &paint);
    }
}
