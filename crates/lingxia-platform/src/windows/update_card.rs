//! Native Win32 post-download "ready to update" card.
//!
//! Per the unified update flow, the package downloads silently in the
//! background and the only user-facing moment is this card: app logo,
//! `v{version} · {size}`, scrollable release notes, and **Later / Restart
//! Now**. A forced update shows the card directly (no "Later"); a normal
//! update first shows the bottom-left callout ([`super::update_callout`]) and
//! the card opens when the user clicks it.
//!
//! The card is owned by the app window (so it centers over the app, tracks it
//! on drag via a follow timer, and hides/restores with the app) and lives on
//! its own UI thread with a message pump.

use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, CreateSolidBrush, DT_CENTER, DT_LEFT, DT_SINGLELINE,
    DT_VCENTER, DeleteObject, DrawTextW, EndPaint, FONT_QUALITY, FW_NORMAL, FW_SEMIBOLD, FillRect,
    HBRUSH, HDC, HFONT, HGDIOBJ, PAINTSTRUCT, PS_SOLID, RoundRect, SelectObject, SetBkMode,
    SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{DRAWITEMSTRUCT, ODS_SELECTED};
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use super::update::apply_staged_windows_update;

/// Update details the card renders.
#[derive(Clone, Default)]
pub(super) struct CardInfo {
    pub product_name: String,
    pub version: String,
    pub size_bytes: Option<u64>,
    pub release_notes: Vec<String>,
    pub is_force_update: bool,
    /// Brand logo PNG to show in the header (resolved by the caller).
    pub logo_path: Option<std::path::PathBuf>,
    /// BCP-47 system locale (e.g. "zh-CN") for the card/callout strings.
    pub locale: String,
}

/// UI language for the update card/callout. Mirrors the macOS `i18n/apple`
/// strings; localized here because `lingxia-platform` sits below the
/// `lingxia-logic` i18n catalog and can't depend on it.
#[derive(Clone, Copy)]
pub(super) enum Lang {
    En,
    Zh,
}

pub(super) fn lang_of(locale: &str) -> Lang {
    if locale.to_ascii_lowercase().starts_with("zh") {
        Lang::Zh
    } else {
        Lang::En
    }
}

pub(super) fn t_whats_new(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "What's new",
        Lang::Zh => "更新内容",
    }
}

pub(super) fn t_later(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Later",
        Lang::Zh => "稍后",
    }
}

pub(super) fn t_restart(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Restart Now",
        Lang::Zh => "立即重启",
    }
}

pub(super) fn t_available_title(lang: Lang, product: &str) -> String {
    match lang {
        Lang::En => format!("{product} update available"),
        Lang::Zh => format!("{product} 有可用更新"),
    }
}

pub(super) fn t_click_to_install(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Click to install",
        Lang::Zh => "点击安装",
    }
}

// Control ids.
const ID_LATER: usize = 1001;
const ID_RESTART: usize = 1003;
const ID_NOTES: usize = 1004;

const WM_APP_DISMISS: u32 = WM_APP + 3;

/// Live card window handle (0 = none).
static CARD_HWND: AtomicIsize = AtomicIsize::new(0);
/// The staged update's details, so the callout can open the card later and so
/// "Later" can re-show the reminder.
static LAST_READY_INFO: Mutex<Option<CardInfo>> = Mutex::new(None);

// ── Public entry points (called from update.rs / update_callout.rs) ──────────

/// Present the post-download "ready to update" affordance. Forced updates show
/// the card directly; normal updates show the dismissible bottom-left callout
/// (which opens the card on click).
pub(super) fn present_ready(info: CardInfo) {
    let forced = info.is_force_update;
    if let Ok(mut slot) = LAST_READY_INFO.lock() {
        *slot = Some(info.clone());
    }
    if forced {
        show_card(info);
    } else {
        super::update_callout::show();
    }
}

/// Open the card from the stored update details (the callout was clicked).
pub(super) fn open_ready_card() {
    let info = LAST_READY_INFO.lock().ok().and_then(|s| s.clone());
    if let Some(info) = info {
        show_card(info);
    }
}

/// Dismiss the card if one is open.
pub(super) fn dismiss() {
    let hwnd = CARD_HWND.load(Ordering::SeqCst);
    if hwnd != 0 {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(hwnd as *mut _)),
                WM_APP_DISMISS,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

fn show_card(info: CardInfo) {
    dismiss();
    std::thread::Builder::new()
        .name("lingxia-update-card".to_string())
        .spawn(move || run_card_thread(info))
        .ok();
}

// ── Card window implementation (card thread only) ───────────────────────────

struct Card {
    info: CardInfo,
    scale: f32,
    owner: Option<HWND>,
    title_font: HFONT,
    body_font: HFONT,
    small_font: HFONT,
    icon: Option<windows::Win32::UI::WindowsAndMessaging::HICON>,
    notes: HWND,
    later: HWND,
    restart: HWND,
}

fn run_card_thread(info: CardInfo) {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("LxUpdateCardClass");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(card_wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let scale = dpi_scale();
        let width = (380.0 * scale) as i32;
        let height = (220.0 * scale) as i32;
        // Own the card to the app window: it centers over the app, tracks it on
        // drag, and hides/restores with it. The check can fire before the main
        // window is up, so poll briefly.
        let mut owner = find_main_window();
        let mut tries = 0;
        while owner.is_none() && tries < 50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            owner = find_main_window();
            tries += 1;
        }
        let (x, y) = centered_origin(width, height, owner);

        let hwnd = match CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class_name,
            w!("Software Update"),
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
            Err(_) => return,
        };
        round_corners(hwnd);

        let title_font = make_font(scale, 16, true);
        let body_font = make_font(scale, 12, false);
        let small_font = make_font(scale, 11, true);

        let icon = info
            .logo_path
            .as_deref()
            .and_then(icon_from_png)
            .or_else(|| extract_app_icon(owner));

        let notes = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("EDIT"),
            PCWSTR::null(),
            WS_CHILD
                | WINDOW_STYLE(ES_MULTILINE as u32)
                | WINDOW_STYLE(ES_READONLY as u32)
                | WS_VSCROLL,
            0,
            0,
            10,
            10,
            Some(hwnd),
            Some(HMENU(ID_NOTES as *mut _)),
            Some(hinstance.into()),
            None,
        )
        .unwrap_or_default();

        let lang = lang_of(&info.locale);
        let later = make_button(hwnd, hinstance.into(), t_later(lang), ID_LATER);
        let restart = make_button(hwnd, hinstance.into(), t_restart(lang), ID_RESTART);

        for ctl in [notes, later, restart] {
            SendMessageW(
                ctl,
                WM_SETFONT,
                Some(WPARAM(body_font.0 as usize)),
                Some(LPARAM(1)),
            );
        }

        if !info.release_notes.is_empty() {
            let text = info
                .release_notes
                .iter()
                .map(|n| format!("\u{2022} {n}"))
                .collect::<Vec<_>>()
                .join("\r\n");
            let wide = to_wide(&text);
            let _ = SetWindowTextW(notes, PCWSTR(wide.as_ptr()));
        }

        let card = Box::new(Card {
            info,
            scale,
            owner,
            title_font,
            body_font,
            small_font,
            icon,
            notes,
            later,
            restart,
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(card) as isize);

        layout(hwnd);
        CARD_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        // ~33 fps follow timer so the card tracks the app window during a drag.
        SetTimer(Some(hwnd), 1, 30, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn card_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                paint(hwnd);
                LRESULT(0)
            }
            WM_DRAWITEM => {
                let dis = &*(lparam.0 as *const DRAWITEMSTRUCT);
                draw_button(dis);
                LRESULT(1)
            }
            WM_TIMER => {
                if let Some(card) = card_ref(hwnd)
                    && let Some(owner) = card.owner
                {
                    let mut cr = RECT::default();
                    if GetWindowRect(hwnd, &mut cr).is_ok() {
                        let (x, y) =
                            centered_origin(cr.right - cr.left, cr.bottom - cr.top, Some(owner));
                        if x != cr.left || y != cr.top {
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
                }
                LRESULT(0)
            }
            WM_APP_DISMISS => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xFFFF;
                match id {
                    ID_RESTART => {
                        apply_staged_windows_update();
                    }
                    ID_LATER => {
                        // Defer: close the card and leave the dismissible
                        // bottom-left reminder so the user can apply later.
                        let _ = DestroyWindow(hwnd);
                        super::update_callout::show();
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT => {
                let hdc = HDC(wparam.0 as *mut _);
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, rgb(28, 28, 30));
                LRESULT(card_brush().0 as isize)
            }
            WM_DESTROY => {
                CARD_HWND.store(0, Ordering::SeqCst);
                if let Some(card) = take_card(hwnd) {
                    let _ = DeleteObject(card.title_font.into());
                    let _ = DeleteObject(card.body_font.into());
                    let _ = DeleteObject(card.small_font.into());
                }
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Lay out controls and size the window to fit the content.
fn layout(hwnd: HWND) {
    unsafe {
        let Some(card) = card_ref(hwnd) else { return };
        let s = card.scale;
        let px = |v: f32| (v * s) as i32;
        let pad = px(24.0);
        let width = px(380.0);
        let header_h = px(52.0);
        let mut y = pad + header_h + px(14.0);
        y += px(1.0) + px(14.0); // divider + gap

        let has_notes = !card.info.release_notes.is_empty();
        if has_notes {
            y += px(18.0); // "What's new" label (painted)
            let notes_h = px(120.0);
            let _ = MoveWindow(card.notes, pad, y, width - pad * 2, notes_h, true);
            let _ = ShowWindow(card.notes, SW_SHOW);
            y += notes_h + px(16.0);
        } else {
            let _ = ShowWindow(card.notes, SW_HIDE);
        }

        let btn_w = px(120.0);
        let btn_h = px(30.0);
        let gap = px(10.0);
        let _ = ShowWindow(card.restart, SW_SHOW);
        let _ = MoveWindow(card.restart, width - pad - btn_w, y, btn_w, btn_h, true);
        if card.info.is_force_update {
            let _ = ShowWindow(card.later, SW_HIDE);
        } else {
            let _ = ShowWindow(card.later, SW_SHOW);
            let _ = MoveWindow(
                card.later,
                width - pad - btn_w * 2 - gap,
                y,
                btn_w,
                btn_h,
                true,
            );
        }
        y += btn_h;

        let total_h = y + pad;
        let (cx, cy) = centered_origin(width, total_h, card.owner);
        let _ = SetWindowPos(
            hwnd,
            None,
            cx,
            cy,
            width,
            total_h,
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
    }
}

fn paint(hwnd: HWND) {
    unsafe {
        let Some(card) = card_ref(hwnd) else { return };
        let s = card.scale;
        let px = |v: f32| (v * s) as i32;

        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut client = RECT::default();
        let _ = GetClientRect(hwnd, &mut client);

        let bg = card_brush();
        FillRect(hdc, &client, bg);
        let border_pen = CreatePen(PS_SOLID, 1, rgb(214, 214, 218));
        let old_pen = SelectObject(hdc, border_pen.into());
        let old_brush = SelectObject(hdc, null_brush());
        let _ = RoundRect(
            hdc,
            client.left,
            client.top,
            client.right - 1,
            client.bottom - 1,
            px(12.0),
            px(12.0),
        );
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(border_pen.into());

        let pad = px(24.0);
        let icon_sz = px(52.0);

        if let Some(icon) = card.icon {
            let _ = DrawIconEx(hdc, pad, pad, icon, icon_sz, icon_sz, 0, None, DI_NORMAL);
        }

        let text_x = pad + icon_sz + px(14.0);
        SetBkMode(hdc, TRANSPARENT);

        let old_font = SelectObject(hdc, card.title_font.into());
        SetTextColor(hdc, rgb(28, 28, 30));
        draw_text(
            hdc,
            &card.info.product_name,
            text_x,
            pad,
            client.right - pad,
            pad + px(22.0),
        );
        SelectObject(hdc, old_font);

        let mut subtitle = String::new();
        if !card.info.version.is_empty() {
            subtitle.push('v');
            subtitle.push_str(&card.info.version);
        }
        if let Some(size) = card.info.size_bytes {
            if !subtitle.is_empty() {
                subtitle.push_str(" \u{00b7} ");
            }
            subtitle.push_str(&human_size(size));
        }
        let of = SelectObject(hdc, card.body_font.into());
        SetTextColor(hdc, rgb(120, 120, 128));
        draw_text(
            hdc,
            &subtitle,
            text_x,
            pad + px(26.0),
            client.right - pad,
            pad + px(46.0),
        );

        let div_y = pad + icon_sz + px(14.0);
        let div_brush = CreateSolidBrush(rgb(228, 228, 232));
        let div_rect = RECT {
            left: pad,
            top: div_y,
            right: client.right - pad,
            bottom: div_y + 1,
        };
        FillRect(hdc, &div_rect, div_brush);
        let _ = DeleteObject(div_brush.into());

        if !card.info.release_notes.is_empty() {
            let body_top = div_y + px(14.0);
            let sf = SelectObject(hdc, card.small_font.into());
            SetTextColor(hdc, rgb(120, 120, 128));
            draw_text(
                hdc,
                t_whats_new(lang_of(&card.info.locale)),
                pad,
                body_top,
                client.right - pad,
                body_top + px(16.0),
            );
            SelectObject(hdc, sf);
        }
        SelectObject(hdc, of);

        let _ = EndPaint(hwnd, &ps);
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

unsafe fn card_ref(hwnd: HWND) -> Option<&'static Card> {
    unsafe { (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Card).as_ref() }
}

unsafe fn take_card(hwnd: HWND) -> Option<Box<Card>> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Card;
        if ptr.is_null() {
            return None;
        }
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        Some(Box::from_raw(ptr))
    }
}

fn card_brush() -> HBRUSH {
    static BRUSH: AtomicIsize = AtomicIsize::new(0);
    let existing = BRUSH.load(Ordering::SeqCst);
    if existing != 0 {
        return HBRUSH(existing as *mut _);
    }
    let brush = unsafe { CreateSolidBrush(rgb(255, 255, 255)) };
    BRUSH.store(brush.0 as isize, Ordering::SeqCst);
    brush
}

fn null_brush() -> HGDIOBJ {
    use windows::Win32::Graphics::Gdi::{GetStockObject, NULL_BRUSH};
    unsafe { GetStockObject(NULL_BRUSH) }
}

/// Owner-draw a flat, rounded macOS-style button. Primary (Restart) is an
/// accent fill with white text; secondary (Later) is a soft gray chip.
fn draw_button(dis: &DRAWITEMSTRUCT) {
    unsafe {
        let hdc = dis.hDC;
        let rc = dis.rcItem;
        let id = dis.CtlID as usize;
        let pressed = (dis.itemState.0 & ODS_SELECTED.0) != 0;
        let primary = id == ID_RESTART;

        let (bg, fg) = if primary {
            (
                if pressed {
                    rgb(0, 105, 217)
                } else {
                    rgb(10, 132, 255)
                },
                rgb(255, 255, 255),
            )
        } else {
            (
                if pressed {
                    rgb(225, 225, 230)
                } else {
                    rgb(240, 240, 244)
                },
                rgb(60, 60, 67),
            )
        };

        let brush = CreateSolidBrush(bg);
        let pen = CreatePen(PS_SOLID, 1, bg);
        let ob = SelectObject(hdc, brush.into());
        let op = SelectObject(hdc, pen.into());
        let radius = ((rc.bottom - rc.top) as f32 * 0.42) as i32;
        let _ = RoundRect(hdc, rc.left, rc.top, rc.right, rc.bottom, radius, radius);
        SelectObject(hdc, ob);
        SelectObject(hdc, op);
        let _ = DeleteObject(brush.into());
        let _ = DeleteObject(pen.into());

        let mut buf = [0u16; 64];
        let n = GetWindowTextW(dis.hwndItem, &mut buf);
        let font = SendMessageW(dis.hwndItem, WM_GETFONT, None, None);
        let of = if font.0 != 0 {
            Some(SelectObject(hdc, HGDIOBJ(font.0 as *mut _)))
        } else {
            None
        };
        SetBkMode(hdc, TRANSPARENT);
        SetTextColor(hdc, fg);
        let mut rect = rc;
        DrawTextW(
            hdc,
            &mut buf[..n.max(0) as usize],
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        if let Some(of) = of {
            SelectObject(hdc, of);
        }
    }
}

fn draw_text(hdc: HDC, text: &str, left: i32, top: i32, right: i32, bottom: i32) {
    let mut wide = to_wide(text);
    let n = wide.len().saturating_sub(1);
    let mut rect = RECT {
        left,
        top,
        right,
        bottom,
    };
    unsafe {
        DrawTextW(
            hdc,
            &mut wide[..n],
            &mut rect,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER,
        );
    }
}

fn make_button(
    parent: HWND,
    hinstance: windows::Win32::Foundation::HINSTANCE,
    text: &str,
    id: usize,
) -> HWND {
    let wide = to_wide(text);
    let style = WS_CHILD | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32);
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            PCWSTR(wide.as_ptr()),
            style,
            0,
            0,
            10,
            10,
            Some(parent),
            Some(HMENU(id as *mut _)),
            Some(hinstance),
            None,
        )
        .unwrap_or_default()
    }
}

fn make_font(scale: f32, pt: i32, semibold: bool) -> HFONT {
    let height = -((pt as f32 * scale * 96.0 / 72.0) as i32);
    let weight = if semibold { FW_SEMIBOLD } else { FW_NORMAL };
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            weight.0 as i32,
            0,
            0,
            0,
            windows::Win32::Graphics::Gdi::FONT_CHARSET(0),
            windows::Win32::Graphics::Gdi::FONT_OUTPUT_PRECISION(0),
            windows::Win32::Graphics::Gdi::FONT_CLIP_PRECISION(0),
            FONT_QUALITY(windows::Win32::Graphics::Gdi::CLEARTYPE_QUALITY.0),
            0,
            w!("Segoe UI"),
        )
    }
}

/// Build an `HICON` from a PNG logo file (same approach as `app_icon.rs`).
fn icon_from_png(path: &std::path::Path) -> Option<windows::Win32::UI::WindowsAndMessaging::HICON> {
    use windows::Win32::Graphics::Gdi::CreateBitmap;
    use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO};

    let img = image::open(path).ok()?;
    let size: u32 = 64;
    let img = img
        .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    let mut bgra = Vec::with_capacity(img.len());
    for p in img.pixels() {
        let [r, g, b, a] = p.0;
        bgra.extend_from_slice(&[b, g, r, a]);
    }
    unsafe {
        let color = CreateBitmap(size as i32, size as i32, 1, 32, Some(bgra.as_ptr().cast()));
        if color.is_invalid() {
            return None;
        }
        let mask = CreateBitmap(size as i32, size as i32, 1, 1, None);
        if mask.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(color.0));
            return None;
        }
        let info = ICONINFO {
            fIcon: windows::core::BOOL(1),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&info).ok();
        let _ = DeleteObject(HGDIOBJ(color.0));
        let _ = DeleteObject(HGDIOBJ(mask.0));
        icon.filter(|h: &HICON| !h.is_invalid())
    }
}

fn extract_app_icon(owner: Option<HWND>) -> Option<windows::Win32::UI::WindowsAndMessaging::HICON> {
    use windows::Win32::UI::WindowsAndMessaging::HICON;
    if let Some(main) = owner {
        unsafe {
            let big = SendMessageW(
                main,
                WM_GETICON,
                Some(WPARAM(ICON_BIG as usize)),
                Some(LPARAM(0)),
            );
            if big.0 != 0 {
                return Some(HICON(big.0 as *mut _));
            }
            let cls = GetClassLongPtrW(main, GCLP_HICON);
            if cls != 0 {
                return Some(HICON(cls as *mut _));
            }
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        let wide = to_wide(&exe.to_string_lossy());
        let mut large = HICON::default();
        let count = unsafe { ExtractIconExW(PCWSTR(wide.as_ptr()), 0, Some(&mut large), None, 1) };
        if count > 0 && !large.is_invalid() {
            return Some(large);
        }
    }
    unsafe { LoadIconW(None, IDI_APPLICATION).ok() }
}

/// Find this process's main app window (largest visible top-level window that
/// is not one of our own update popups), to own/center the card on.
pub(super) fn find_main_window() -> Option<HWND> {
    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
        unsafe {
            let best = &mut *(lparam.0 as *mut (isize, i64));
            let mut pid = 0u32;
            let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == windows::Win32::System::Threading::GetCurrentProcessId()
                && IsWindowVisible(hwnd).as_bool()
            {
                let mut buf = [0u16; 64];
                let n = GetClassNameW(hwnd, &mut buf);
                let cls = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
                if !cls.starts_with("LxUpdate") {
                    let mut r = RECT::default();
                    if GetWindowRect(hwnd, &mut r).is_ok() {
                        let area = (r.right - r.left) as i64 * (r.bottom - r.top) as i64;
                        if area > best.1 {
                            *best = (hwnd.0 as isize, area);
                        }
                    }
                }
            }
            windows::core::BOOL(1)
        }
    }
    let mut best: (isize, i64) = (0, 0);
    let _ = unsafe { EnumWindows(Some(enum_proc), LPARAM(&mut best as *mut _ as isize)) };
    if best.0 != 0 {
        Some(HWND(best.0 as *mut _))
    } else {
        None
    }
}

fn dpi_scale() -> f32 {
    unsafe {
        let dc = windows::Win32::Graphics::Gdi::GetDC(None);
        if dc.is_invalid() {
            return 1.0;
        }
        let dpi = windows::Win32::Graphics::Gdi::GetDeviceCaps(
            Some(dc),
            windows::Win32::Graphics::Gdi::LOGPIXELSX,
        );
        windows::Win32::Graphics::Gdi::ReleaseDC(None, dc);
        if dpi <= 0 { 1.0 } else { dpi as f32 / 96.0 }
    }
}

fn centered_origin(width: i32, height: i32, owner: Option<HWND>) -> (i32, i32) {
    unsafe {
        let mut rect = RECT::default();
        let have_owner = owner
            .map(|h| GetWindowRect(h, &mut rect).is_ok())
            .unwrap_or(false);
        if !have_owner {
            let _ = SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut rect as *mut _ as *mut _),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            );
        }
        let cx = (rect.left + rect.right) / 2 - width / 2;
        let cy = (rect.top + rect.bottom) / 2 - height / 2;
        (cx, cy)
    }
}

fn round_corners(hwnd: HWND) {
    use windows::Win32::Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DwmSetWindowAttribute};
    let pref: u32 = 2; // DWMWCP_ROUND
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < KB * KB {
        format!("{:.0} KB", b / KB)
    } else if b < KB * KB * KB {
        format!("{:.1} MB", b / (KB * KB))
    } else {
        format!("{:.1} GB", b / (KB * KB * KB))
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
