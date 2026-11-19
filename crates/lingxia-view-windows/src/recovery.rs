//! Host-owned recovery chrome over a failed page: a message and, when the core offers one,
//! a Retry or Reload button. Page content cannot replace it.
use crate::view;
use lingxia_view::session::host::{Failure, RecoveryAction, RecoveryMessage};
use std::cell::RefCell;
use std::collections::HashMap;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateSolidBrush, DT_CALCRECT, DT_CENTER, DT_WORDBREAK, DeleteObject,
    DrawTextW, EndPaint, FillRect, GetDC, HFONT, HGDIOBJ, InvalidateRect, PAINTSTRUCT, ReleaseDC,
    SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    BS_PUSHBUTTON, CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetParent, HMENU,
    HWND_TOP, IDC_ARROW, LoadCursorW, RegisterClassW, SWP_NOACTIVATE, SendMessageW, SetWindowPos,
    SetWindowTextW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_COMMAND, WM_ERASEBKGND, WM_PAINT, WM_SETFONT,
    WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{HSTRING, w};

const BUTTON_ID: usize = 1;
/// Logical pixels.
const PADDING: f32 = 16.0;
const TEXT_WIDTH: f32 = 360.0;
const FONT: f32 = 15.0;
const BUTTON: [f32; 2] = [140.0, 32.0];

/// Localized chrome text, the key the platform localizer knows and its English fallback.
pub(crate) type Text<'a> = &'a dyn Fn(&str, &str) -> String;

struct Panel {
    window: isize,
    button: isize,
    message: Vec<u16>,
    covers: bool,
    dark: bool,
    font: HFONT,
    retrying: String,
}

thread_local! {
    /// Each page window's panel.
    static PANELS: RefCell<HashMap<isize, Panel>> = RefCell::new(HashMap::new());
}

/// Show `failure`'s chrome over `page`, or clear it with `None`.
pub(crate) fn show(page: isize, failure: Option<&Failure>, dark: bool, text: Text) {
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
    };
    let action = failure.action.map(|action| match action {
        RecoveryAction::Retry => text("gpu.retry", "Retry"),
        RecoveryAction::Reload => text("gpu.reload", "Reload page"),
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
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
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
        let button = action
            .and_then(|label| {
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
                    Some(HMENU(BUTTON_ID as *mut _)),
                    Some(instance.into()),
                    None,
                )
                .ok()
            })
            .map_or(0, |button| {
                SendMessageW(
                    button,
                    WM_SETFONT,
                    Some(WPARAM(font.0 as usize)),
                    Some(LPARAM(1)),
                );
                button.0 as isize
            });
        PANELS.with(|panels| {
            panels.borrow_mut().insert(
                page,
                Panel {
                    window: window.0 as isize,
                    button,
                    message: message.encode_utf16().collect(),
                    covers: failure.covers_page(),
                    dark,
                    font,
                    retrying: text("gpu.retrying", "Retrying…"),
                },
            )
        });
    }
    layout(page);
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
        // SAFETY: a font this thread made.
        let _ = unsafe { DeleteObject(HGDIOBJ(panel.font.0)) };
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
    let Some((window, button, covers, font, mut message)) = PANELS.with(|panels| {
        panels
            .borrow()
            .get(&page)
            .map(|p| (p.window, p.button, p.covers, p.font, p.message.clone()))
    }) else {
        return;
    };
    let scale = scale(page);
    let px = |logical: f32| (logical * scale).round() as i32;
    let (width, height) = view::client_size(page);
    let (width, height) = (width as i32, height as i32);
    let text_width = px(TEXT_WIDTH).min(width - 2 * px(PADDING)).max(1);
    let mut measured = RECT {
        right: text_width,
        ..Default::default()
    };
    // SAFETY: a screen DC measures the text with the panel's font.
    unsafe {
        let dc = GetDC(None);
        let old = SelectObject(dc, HGDIOBJ(font.0));
        DrawTextW(
            dc,
            &mut message,
            &mut measured,
            DT_CALCRECT | DT_CENTER | DT_WORDBREAK,
        );
        SelectObject(dc, old);
        ReleaseDC(None, dc);
    }
    let text_height = measured.bottom;
    let gap = px(12.0);
    let [bw, bh] = BUTTON.map(px);
    let stack = text_height + if button != 0 { gap + bh } else { 0 };
    let panel_height = if covers {
        height
    } else {
        (stack + 2 * px(PADDING)).min(height)
    };
    let top = (panel_height - stack) / 2;
    PANELS.with(|panels| {
        if let Some(panel) = panels.borrow_mut().get_mut(&page) {
            panel.message = message;
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
        if button != 0 {
            let _ = SetWindowPos(
                HWND(button as *mut _),
                None,
                (width - bw) / 2,
                top + text_height + gap,
                bw,
                bh,
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
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_COMMAND if wparam.0 & 0xffff == BUTTON_ID => {
            retry(hwnd);
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
    // SAFETY: this thread's button.
    unsafe {
        let _ = EnableWindow(button, false);
        let _ = SetWindowTextW(button, &HSTRING::from(label));
    }
    if let Some(page) = view::page_for(page) {
        page.host().retry();
    }
}

fn paint(hwnd: HWND) {
    let Some(page) = page_of_panel(hwnd) else {
        return;
    };
    let Some((mut message, dark, font, button)) = PANELS.with(|panels| {
        panels
            .borrow()
            .get(&page)
            .map(|p| (p.message.clone(), p.dark, p.font, p.button))
    }) else {
        return;
    };
    let scale = scale(page);
    let px = |logical: f32| (logical * scale).round() as i32;
    let (background, foreground) = if dark {
        (0x0020_2020, 0x00f0_f0f0)
    } else {
        (0x00f3_f3f3, 0x001a_1a1a)
    };
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
        let stack = measured.bottom + if button != 0 { gap + px(BUTTON[1]) } else { 0 };
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
