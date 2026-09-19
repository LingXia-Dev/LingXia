//! Top-right banner card, pinned to the host window when one is visible.

use crate::traits::app_runtime::{
    DesktopBannerActionStyle, DesktopBannerBackground, DesktopBannerOutcome, DesktopBannerShow,
};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, CreateRoundRectRgn, CreateSolidBrush, DT_CALCRECT, DT_CENTER,
    DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, DeleteObject,
    DrawTextW, EndPaint, FillRect, GetDC, GetTextMetricsW, HBRUSH, HDC, HFONT, InvalidateRect,
    PAINTSTRUCT, PS_SOLID, ReleaseDC, RestoreDC, RoundRect, SaveDC, SelectClipRgn, SelectObject,
    SetBkMode, SetTextColor, SetWindowRgn, TEXTMETRICW, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use super::update_callout::{make_font, rgb, round_corners, to_wide};

/// `WM_MOUSELEAVE` (winuser.h) — not on `WindowsAndMessaging` in this crate rev.
const WM_MOUSELEAVE: u32 = 0x02A3;

static HWND_SLOT: AtomicIsize = AtomicIsize::new(0);
/// Set by `hide` so a dismiss that wins the race against `CreateWindowExW`
/// still retires the card. `present` clears it after hiding the previous one.
static HIDE_PENDING: AtomicBool = AtomicBool::new(false);

const WM_APP_HIDE: u32 = WM_APP + 21;
const PAD: f32 = 12.0;
const ICON: f32 = 36.0;
const ICON_RADIUS: f32 = 8.0;
const GAP: f32 = 10.0;
const CLOSE: f32 = 20.0;
const BTN_H: f32 = 24.0;
const BTN_MIN_W: f32 = 56.0;
const BTN_PAD_X: f32 = 20.0;
const BTN_RADIUS: f32 = 6.0;
const CARD_W: f32 = 328.0;
const CARD_RADIUS: f32 = 12.0;

struct Card {
    id: String,
    title: String,
    body: String,
    actions: Vec<(String, String, DesktopBannerActionStyle)>,
    dismissible: bool,
    title_font: HFONT,
    body_font: HFONT,
    btn_font: HFONT,
    icon: Option<HICON>,
    title_height: i32,
    body_height: i32,
    button_widths: Vec<i32>,
    hover: Hover,
    background: DesktopBannerBackground,
    owner: Option<HWND>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Hover {
    None,
    Close,
    Action(usize),
}

pub(crate) fn present(request: &DesktopBannerShow) -> bool {
    hide();
    HIDE_PENDING.store(false, Ordering::SeqCst);
    let request = request.clone();
    std::thread::Builder::new()
        .name("lingxia-desktop-banner".into())
        .spawn(move || run_banner_thread(request))
        .is_ok()
}

pub(crate) fn hide() {
    HIDE_PENDING.store(true, Ordering::SeqCst);
    let hwnd = HWND_SLOT.swap(0, Ordering::SeqCst);
    if hwnd != 0 {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(hwnd as *mut _)),
                WM_APP_HIDE,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

fn run_banner_thread(request: DesktopBannerShow) {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("LxDesktopBannerClass");
        let wc = WNDCLASSW {
            style: CS_DROPSHADOW,
            lpfnWndProc: Some(banner_wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH(std::ptr::null_mut()),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let scale = super::update_callout::dpi_scale();
        let px = |v: f32| (v * scale) as i32;
        let metrics = measure_card(&request, scale);
        let width = px(CARD_W);
        let height = metrics.height;

        let owner = super::update_card::find_main_window();
        let mut anchor = RECT::default();
        let have_owner = owner
            .map(|h| GetWindowRect(h, &mut anchor).is_ok())
            .unwrap_or(false);
        if !have_owner {
            let _ = SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut anchor as *mut _ as *mut _),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            );
        }
        let margin = px(16.0);
        let (x, y) = banner_origin(anchor, width, height, margin);

        let hwnd = match CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name,
            w!(""),
            WS_POPUP,
            x,
            y,
            width,
            height,
            owner,
            None,
            Some(hinstance.into()),
            None,
        ) {
            Ok(h) => h,
            Err(_) => {
                crate::desktop::banner::fail(&request.id, "failed to present banner");
                return;
            }
        };
        round_corners(hwnd);
        clip_round_window(hwnd, px(CARD_RADIUS));

        let card = Box::new(Card {
            id: request.id.clone(),
            title: request.title.clone(),
            body: request.body.clone(),
            actions: request
                .actions
                .iter()
                .map(|action| (action.id.clone(), action.label.clone(), action.style))
                .collect(),
            dismissible: request.actions.is_empty(),
            title_font: make_font(scale, 13, true),
            body_font: make_font(scale, 12, false),
            btn_font: make_font(scale, 12, true),
            icon: load_app_icon(),
            title_height: metrics.title_height,
            body_height: metrics.body_height,
            button_widths: metrics.button_widths,
            hover: Hover::None,
            background: request.background.clone(),
            owner,
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(card) as isize);
        HWND_SLOT.store(hwnd.0 as isize, Ordering::SeqCst);
        if HIDE_PENDING.load(Ordering::SeqCst) {
            let _ = DestroyWindow(hwnd);
        } else {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            SetTimer(Some(hwnd), 1, 30, None);
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn banner_origin(anchor: RECT, width: i32, _height: i32, margin: i32) -> (i32, i32) {
    let x = (anchor.right - width - margin).max(anchor.left + margin.min(8));
    let y = anchor.top + margin;
    (x, y)
}

fn pin_to_owner(hwnd: HWND) {
    let Some(card) = card_ref(hwnd) else {
        return;
    };
    let Some(owner) = card.owner else {
        return;
    };
    unsafe {
        let mut anchor = RECT::default();
        let mut current = RECT::default();
        if GetWindowRect(owner, &mut anchor).is_err() || GetWindowRect(hwnd, &mut current).is_err()
        {
            return;
        }
        let width = current.right - current.left;
        let height = current.bottom - current.top;
        let scale = super::update_callout::dpi_scale();
        let margin = (16.0 * scale) as i32;
        let (x, y) = banner_origin(anchor, width, height, margin);
        if x == current.left && y == current.top {
            return;
        }
        let _ = SetWindowPos(
            hwnd,
            None,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

struct CardMetrics {
    height: i32,
    title_height: i32,
    body_height: i32,
    button_widths: Vec<i32>,
}

fn measure_card(request: &DesktopBannerShow, scale: f32) -> CardMetrics {
    let px = |v: f32| (v * scale) as i32;
    let pad = px(PAD);
    let icon = px(ICON);
    unsafe {
        let hdc = GetDC(None);
        let title_font = make_font(scale, 13, true);
        let body_font = make_font(scale, 12, false);
        let btn_font = make_font(scale, 12, true);
        let title_height = line_height(hdc, title_font);
        let body_line = line_height(hdc, body_font);
        let body_height = if request.body.is_empty() {
            0
        } else {
            // Always keep the macOS two-line slot. CALCRECT on the screen DC
            // can undershoot DrawText wrap on the window DC and clip line 2.
            body_line * 2 + 4
        };
        let button_widths = request
            .actions
            .iter()
            .map(|action| {
                let text_w = measure_text(hdc, btn_font, &action.label, px(CARD_W), false).0;
                (text_w + px(BTN_PAD_X)).max(px(BTN_MIN_W))
            })
            .collect::<Vec<_>>();
        let _ = DeleteObject(title_font.into());
        let _ = DeleteObject(body_font.into());
        let _ = DeleteObject(btn_font.into());
        ReleaseDC(None, hdc);
        let mut y = pad + title_height;
        if body_height > 0 {
            y += px(2.0) + body_height;
        }
        if !button_widths.is_empty() {
            y += px(10.0) + px(BTN_H);
        }
        y += pad;
        CardMetrics {
            height: y.max(pad + icon + pad),
            title_height,
            body_height,
            button_widths,
        }
    }
}

fn line_height(hdc: HDC, font: HFONT) -> i32 {
    unsafe {
        let old = SelectObject(hdc, font.into());
        let mut metrics = TEXTMETRICW::default();
        let _ = GetTextMetricsW(hdc, &mut metrics);
        SelectObject(hdc, old);
        (metrics.tmHeight + metrics.tmExternalLeading).max(1)
    }
}

fn clip_round_window(hwnd: HWND, radius: i32) {
    unsafe {
        let mut window = RECT::default();
        if GetWindowRect(hwnd, &mut window).is_err() {
            return;
        }
        let width = window.right - window.left;
        let height = window.bottom - window.top;
        if width <= 0 || height <= 0 {
            return;
        }
        let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, radius * 2, radius * 2);
        let _ = SetWindowRgn(hwnd, Some(region), true);
    }
}

fn measure_text(hdc: HDC, font: HFONT, text: &str, max_width: i32, wrap: bool) -> (i32, i32) {
    unsafe {
        let old = SelectObject(hdc, font.into());
        let mut wide = to_wide(text);
        let n = wide.len().saturating_sub(1);
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: max_width.max(1),
            bottom: 0,
        };
        let format = DT_NOPREFIX | DT_CALCRECT | if wrap { DT_WORDBREAK } else { DT_SINGLELINE };
        let _ = DrawTextW(hdc, &mut wide[..n], &mut rect, format);
        SelectObject(hdc, old);
        (
            (rect.right - rect.left).max(0),
            (rect.bottom - rect.top).max(0),
        )
    }
}

unsafe extern "system" fn banner_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Rust 2024: `unsafe fn` does not make the body an unsafe block.
    unsafe {
        match msg {
            WM_PAINT => {
                paint(hwnd);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                track_hover(hwnd, lparam);
                track_mouse_leave(hwnd);
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                clear_hover(hwnd);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                on_click(hwnd, lparam);
                LRESULT(0)
            }
            WM_TIMER => {
                pin_to_owner(hwnd);
                LRESULT(0)
            }
            WM_APP_HIDE | WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                if let Some(card) = take_card(hwnd) {
                    let _ = DeleteObject(card.title_font.into());
                    let _ = DeleteObject(card.body_font.into());
                    let _ = DeleteObject(card.btn_font.into());
                    if let Some(icon) = card.icon {
                        let _ = DestroyIcon(icon);
                    }
                }
                let this = hwnd.0 as isize;
                let _ = HWND_SLOT.compare_exchange(this, 0, Ordering::SeqCst, Ordering::SeqCst);
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_DPICHANGED => {
                let _ = InvalidateRect(Some(hwnd), None, true);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn paint(hwnd: HWND) {
    unsafe {
        let Some(card) = card_ref(hwnd) else {
            return;
        };
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut client = RECT::default();
        let _ = GetClientRect(hwnd, &mut client);
        let scale = super::update_callout::dpi_scale();
        let px = |v: f32| (v * scale) as i32;

        let (bg_color, title_color, body_color, border) = theme_colors(&card.background);
        let bg = CreateSolidBrush(bg_color);
        let border_pen = CreatePen(PS_SOLID, 1, border);
        let old_brush = SelectObject(hdc, bg.into());
        let old_pen = SelectObject(hdc, border_pen.into());
        let corner = px(CARD_RADIUS) * 2;
        let _ = RoundRect(
            hdc,
            client.left,
            client.top,
            client.right,
            client.bottom,
            corner,
            corner,
        );
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(bg.into());
        let _ = DeleteObject(border_pen.into());

        let pad = px(PAD);
        let icon = px(ICON);
        draw_icon(hdc, pad, pad, icon, card.icon);

        let text_left = pad + icon + px(GAP);
        let title_right = client.right - pad - if card.dismissible { px(CLOSE) } else { 0 };
        let body_right = client.right - pad;

        SetBkMode(hdc, TRANSPARENT);
        let old = SelectObject(hdc, card.title_font.into());
        SetTextColor(hdc, title_color);
        let mut title_rect = RECT {
            left: text_left,
            top: pad,
            right: title_right,
            bottom: pad + card.title_height,
        };
        let mut title = to_wide(&card.title);
        let title_n = title.len().saturating_sub(1);
        DrawTextW(
            hdc,
            &mut title[..title_n],
            &mut title_rect,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        );

        if !card.body.is_empty() {
            SelectObject(hdc, card.body_font.into());
            SetTextColor(hdc, body_color);
            let body_top = pad + card.title_height + px(2.0);
            let mut body_rect = RECT {
                left: text_left,
                top: body_top,
                right: body_right,
                bottom: body_top + card.body_height,
            };
            let mut body = to_wide(&card.body);
            let body_n = body.len().saturating_sub(1);
            DrawTextW(
                hdc,
                &mut body[..body_n],
                &mut body_rect,
                DT_LEFT | DT_WORDBREAK | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
        }
        SelectObject(hdc, old);

        if card.dismissible {
            let close = close_rect(hwnd, scale);
            SetTextColor(
                hdc,
                if card.hover == Hover::Close {
                    title_color
                } else {
                    body_color
                },
            );
            let tf = SelectObject(hdc, card.title_font.into());
            let mut mark = to_wide("×");
            let n = mark.len().saturating_sub(1);
            let mut close_mut = close;
            DrawTextW(
                hdc,
                &mut mark[..n],
                &mut close_mut,
                DT_CENTER | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
            );
            SelectObject(hdc, tf);
        }

        for (index, (_, label, style)) in card.actions.iter().enumerate() {
            let rect = action_rect(hwnd, scale, card, index);
            paint_button(
                hdc,
                &rect,
                label,
                *style,
                card.hover == Hover::Action(index),
                card.btn_font,
                card.background.prefers_dark_content(),
            );
        }

        let _ = EndPaint(hwnd, &ps);
    }
}

fn draw_icon(hdc: HDC, x: i32, y: i32, size: i32, icon: Option<HICON>) {
    unsafe {
        let radius = ((size as f32) * ICON_RADIUS / ICON).round() as i32;
        let region = CreateRoundRectRgn(x, y, x + size + 1, y + size + 1, radius * 2, radius * 2);
        let saved = SaveDC(hdc);
        SelectClipRgn(hdc, Some(region));
        if let Some(icon) = icon {
            let _ = DrawIconEx(hdc, x, y, icon, size, size, 0, None, DI_NORMAL);
        } else {
            let brush = CreateSolidBrush(rgb(10, 132, 255));
            let rect = RECT {
                left: x,
                top: y,
                right: x + size,
                bottom: y + size,
            };
            FillRect(hdc, &rect, brush);
            let _ = DeleteObject(brush.into());
        }
        let _ = RestoreDC(hdc, saved);
        let _ = DeleteObject(region.into());
    }
}

fn load_app_icon() -> Option<HICON> {
    let exe = std::env::current_exe().ok()?;
    let wide = to_wide(&exe.to_string_lossy());
    let mut large = HICON::default();
    let count = unsafe { ExtractIconExW(PCWSTR(wide.as_ptr()), 0, Some(&mut large), None, 1) };
    if count > 0 && !large.is_invalid() {
        Some(large)
    } else {
        None
    }
}

fn track_mouse_leave(hwnd: HWND) {
    let mut track = TRACKMOUSEEVENT {
        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_LEAVE,
        hwndTrack: hwnd,
        dwHoverTime: 0,
    };
    unsafe {
        let _ = TrackMouseEvent(&mut track);
    }
}

fn clear_hover(hwnd: HWND) {
    let Some(card) = card_mut(hwnd) else {
        return;
    };
    if card.hover == Hover::None {
        return;
    }
    card.hover = Hover::None;
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, true);
    }
}

fn theme_colors(background: &DesktopBannerBackground) -> (COLORREF, COLORREF, COLORREF, COLORREF) {
    match background {
        DesktopBannerBackground::Light => (
            rgb(245, 245, 247),
            rgb(28, 28, 30),
            rgb(110, 110, 115),
            rgb(210, 210, 215),
        ),
        DesktopBannerBackground::Color { r, g, b, .. } => {
            let (title, body, border) = if background.prefers_dark_content() {
                (rgb(28, 28, 30), rgb(110, 110, 115), rgb(200, 200, 205))
            } else {
                (rgb(255, 255, 255), rgb(174, 174, 178), rgb(70, 70, 74))
            };
            (rgb(*r, *g, *b), title, body, border)
        }
        DesktopBannerBackground::System | DesktopBannerBackground::Dark => (
            rgb(44, 44, 46),
            rgb(255, 255, 255),
            rgb(174, 174, 178),
            rgb(70, 70, 74),
        ),
    }
}

fn paint_button(
    hdc: HDC,
    rect: &RECT,
    label: &str,
    style: DesktopBannerActionStyle,
    hover: bool,
    font: HFONT,
    dark_content: bool,
) {
    unsafe {
        let (bg, fg) = match style {
            DesktopBannerActionStyle::Primary => (
                if hover {
                    rgb(64, 156, 255)
                } else {
                    rgb(10, 132, 255)
                },
                rgb(255, 255, 255),
            ),
            DesktopBannerActionStyle::Destructive => (
                if hover {
                    rgb(255, 105, 97)
                } else {
                    rgb(255, 69, 58)
                },
                rgb(255, 255, 255),
            ),
            DesktopBannerActionStyle::Default if dark_content => {
                if hover {
                    (rgb(226, 226, 230), rgb(28, 28, 30))
                } else {
                    (rgb(236, 236, 239), rgb(28, 28, 30))
                }
            }
            DesktopBannerActionStyle::Default => {
                if hover {
                    (rgb(72, 72, 74), rgb(255, 255, 255))
                } else {
                    (rgb(58, 58, 60), rgb(255, 255, 255))
                }
            }
        };
        let brush = CreateSolidBrush(bg);
        let pen = CreatePen(PS_SOLID, 1, bg);
        let old_brush = SelectObject(hdc, brush.into());
        let old_pen = SelectObject(hdc, pen.into());
        let radius = ((rect.bottom - rect.top) as f32 * (BTN_RADIUS / BTN_H) * 2.0).round() as i32;
        let _ = RoundRect(
            hdc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            radius,
            radius,
        );
        SelectObject(hdc, old_brush);
        SelectObject(hdc, old_pen);
        let _ = DeleteObject(brush.into());
        let _ = DeleteObject(pen.into());
        let old = SelectObject(hdc, font.into());
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, fg);
        let mut text = to_wide(label);
        let n = text.len().saturating_sub(1);
        let mut text_rect = *rect;
        DrawTextW(
            hdc,
            &mut text[..n],
            &mut text_rect,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );
        SelectObject(hdc, old);
    }
}

fn close_rect(hwnd: HWND, scale: f32) -> RECT {
    let px = |v: f32| (v * scale) as i32;
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut client);
    }
    RECT {
        left: client.right - px(PAD) - px(CLOSE),
        top: px(PAD),
        right: client.right - px(PAD),
        bottom: px(PAD) + px(CLOSE),
    }
}

fn action_rect(hwnd: HWND, scale: f32, card: &Card, index: usize) -> RECT {
    let px = |v: f32| (v * scale) as i32;
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut client);
    }
    let height = px(BTN_H);
    let gap = px(8.0);
    let mut right = client.right - px(PAD);
    for width in card.button_widths.iter().skip(index + 1).rev() {
        right -= *width + gap;
    }
    let width = card
        .button_widths
        .get(index)
        .copied()
        .unwrap_or(px(BTN_MIN_W));
    RECT {
        left: right - width,
        top: client.bottom - px(PAD) - height,
        right,
        bottom: client.bottom - px(PAD),
    }
}

fn track_hover(hwnd: HWND, lparam: LPARAM) {
    let Some(card) = card_mut(hwnd) else {
        return;
    };
    let point = point_from(lparam);
    let scale = super::update_callout::dpi_scale();
    let hover = if card.dismissible && pt_in(close_rect(hwnd, scale), point) {
        Hover::Close
    } else {
        card.actions
            .iter()
            .enumerate()
            .find_map(|(index, _)| {
                pt_in(action_rect(hwnd, scale, card, index), point).then_some(Hover::Action(index))
            })
            .unwrap_or(Hover::None)
    };
    if hover != card.hover {
        card.hover = hover;
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, true);
        }
    }
}

fn on_click(hwnd: HWND, lparam: LPARAM) {
    let Some(card) = card_ref(hwnd) else {
        return;
    };
    let point = point_from(lparam);
    let scale = super::update_callout::dpi_scale();
    if card.dismissible && pt_in(close_rect(hwnd, scale), point) {
        let id = card.id.clone();
        crate::desktop::banner::complete(&id, DesktopBannerOutcome::Dismissed { id: id.clone() });
        return;
    }
    for (index, (action, _, _)) in card.actions.iter().enumerate() {
        if pt_in(action_rect(hwnd, scale, card, index), point) {
            let id = card.id.clone();
            let action = action.clone();
            crate::desktop::banner::complete(
                &id,
                DesktopBannerOutcome::Action {
                    id: id.clone(),
                    action,
                },
            );
            return;
        }
    }
}

fn point_from(lparam: LPARAM) -> POINT {
    POINT {
        x: (lparam.0 & 0xFFFF) as i16 as i32,
        y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
    }
}

fn pt_in(rect: RECT, point: POINT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

fn card_ref(hwnd: HWND) -> Option<&'static Card> {
    unsafe { (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Card).as_ref() }
}

fn card_mut(hwnd: HWND) -> Option<&'static mut Card> {
    unsafe { (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Card).as_mut() }
}

fn take_card(hwnd: HWND) -> Option<Box<Card>> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Card;
        if ptr.is_null() {
            return None;
        }
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        Some(Box::from_raw(ptr))
    }
}

#[cfg(test)]
mod tests {
    use super::banner_origin;
    use windows::Win32::Foundation::RECT;

    #[test]
    fn banner_origin_pins_to_the_anchor_top_right() {
        let anchor = RECT {
            left: 40,
            top: 20,
            right: 840,
            bottom: 620,
        };
        assert_eq!(banner_origin(anchor, 328, 120, 16), (496, 36));
    }

    #[test]
    fn banner_origin_stays_inside_a_narrow_anchor() {
        let anchor = RECT {
            left: 100,
            top: 10,
            right: 300,
            bottom: 400,
        };
        assert_eq!(banner_origin(anchor, 328, 80, 16), (108, 26));
    }
}
