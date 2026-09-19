//! Top-right banner card: a per-pixel-alpha layered window pinned to the host
//! window, or to the work area while the host is minimized or hidden.

// The presenter is compiled out of test builds; only the pure layout is tested.
#![cfg_attr(test, allow(dead_code))]

use crate::traits::app_runtime::{
    DesktopBannerActionStyle, DesktopBannerBackground, DesktopBannerOutcome, DesktopBannerShow,
};
use std::sync::Mutex;
use std::time::Instant;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CLEARTYPE_QUALITY, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateRoundRectRgn,
    DIB_RGB_COLORS, DT_CALCRECT, DT_CENTER, DT_EDITCONTROL, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX,
    DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, DeleteDC, DeleteObject, DrawTextW, FONT_CHARSET,
    FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, FONT_QUALITY, FW_NORMAL, FW_SEMIBOLD, GdiFlush,
    GetDC, GetTextFaceW, GetTextMetricsW, HBITMAP, HDC, HFONT, HGDIOBJ, ReleaseDC, RestoreDC,
    SaveDC, SelectClipRgn, SelectObject, SetBkMode, SetTextColor, TEXTMETRICW, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent};
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use super::update_callout::{dpi_scale, rgb, to_wide};

/// `WM_MOUSELEAVE` (winuser.h) — not on `WindowsAndMessaging` in this crate rev.
const WM_MOUSELEAVE: u32 = 0x02A3;
const WM_APP_HIDE: u32 = WM_APP + 21;
const TIMER_ANIM: usize = 1;
const TIMER_PIN: usize = 2;

const PAD: f32 = 12.0;
const ICON: f32 = 36.0;
const ICON_RADIUS: f32 = 8.0;
const GAP: f32 = 10.0;
const CLOSE: f32 = 20.0;
const CLOSE_INSET: f32 = 8.0;
const BTN_H: f32 = 24.0;
const BTN_MIN_W: f32 = 56.0;
const BTN_PAD_X: f32 = 20.0;
const BTN_GAP: f32 = 8.0;
const BTN_RADIUS: f32 = 6.0;
const CARD_W: f32 = 328.0;
const CARD_RADIUS: f32 = 12.0;
const MARGIN: f32 = 16.0;
/// Transparent gutter around the card that holds the soft shadow.
const SHADOW: f32 = 26.0;
const SHADOW_DROP: f32 = 6.0;
const SHADOW_ALPHA: f32 = 0.22;
const SLIDE: f32 = 18.0;
const ENTER_MS: f32 = 220.0;
const EXIT_MS: f32 = 140.0;
const TITLE_PX: f32 = 14.0;
const BODY_PX: f32 = 13.0;
const BTN_PX: f32 = 12.0;
const GLYPH_PX: f32 = 10.0;
/// ChromeClose in Segoe Fluent Icons / Segoe MDL2 Assets.
const CLOSE_GLYPH: &str = "\u{E8BB}";

type Rgb = (u8, u8, u8);

/// The live card. `generation` moves on every present and hide, so a window
/// that finishes creating after it was superseded retires itself instead of
/// leaking on screen outside the slot.
struct Slot {
    generation: u64,
    hwnd: isize,
}

static SLOT: Mutex<Slot> = Mutex::new(Slot {
    generation: 0,
    hwnd: 0,
});

fn slot() -> std::sync::MutexGuard<'static, Slot> {
    SLOT.lock().unwrap_or_else(|error| error.into_inner())
}

/// Retire the current card and return the generation the next one owns.
fn supersede() -> u64 {
    let mut slot = slot();
    let hwnd = std::mem::take(&mut slot.hwnd);
    slot.generation += 1;
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
    slot.generation
}

pub(crate) fn present(request: &DesktopBannerShow) -> bool {
    let generation = supersede();
    let request = request.clone();
    std::thread::Builder::new()
        .name("lingxia-desktop-banner".into())
        .spawn(move || run_banner_thread(request, generation))
        .is_ok()
}

pub(crate) fn hide() {
    supersede();
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Hit {
    None,
    Close,
    Action(usize),
}

struct Fonts {
    title: HFONT,
    body: HFONT,
    button: HFONT,
    /// Icon font for the close mark; `None` falls back to a text `×`.
    glyph: Option<HFONT>,
}

impl Fonts {
    fn load(hdc: HDC, scale: f32) -> Self {
        const TEXT: &[&str] = &["Segoe UI Variable Text", "Segoe UI"];
        const GLYPH: &[&str] = &["Segoe Fluent Icons", "Segoe MDL2 Assets"];
        let text = |size: f32, semibold: bool| {
            resolve_font(hdc, scale, size, semibold, TEXT)
                .unwrap_or_else(|| make_font(scale, size, semibold, "Segoe UI"))
        };
        Self {
            title: text(TITLE_PX, true),
            body: text(BODY_PX, false),
            button: text(BTN_PX, true),
            glyph: resolve_font(hdc, scale, GLYPH_PX, false, GLYPH),
        }
    }

    fn release(&self) {
        unsafe {
            for font in [
                Some(self.title),
                Some(self.body),
                Some(self.button),
                self.glyph,
            ]
            .into_iter()
            .flatten()
            .filter(|font| !font.is_invalid())
            {
                let _ = DeleteObject(font.into());
            }
        }
    }
}

/// 32-bit top-down DIB the card is drawn into and handed to
/// `UpdateLayeredWindow`.
struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    bits: *mut u32,
    width: i32,
    height: i32,
}

impl Surface {
    fn new(width: i32, height: i32) -> Option<Self> {
        if width <= 0 || height <= 0 {
            return None;
        }
        unsafe {
            let screen = GetDC(None);
            let dc = CreateCompatibleDC(Some(screen));
            ReleaseDC(None, screen);
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            let Ok(bitmap) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
            else {
                let _ = DeleteDC(dc);
                return None;
            };
            let previous = SelectObject(dc, bitmap.into());
            Some(Self {
                dc,
                bitmap,
                previous,
                bits: bits.cast(),
                width,
                height,
            })
        }
    }

    fn pixels(&mut self) -> &mut [u32] {
        unsafe { std::slice::from_raw_parts_mut(self.bits, (self.width * self.height) as usize) }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

struct Card {
    id: String,
    title: String,
    body: String,
    actions: Vec<(String, String, DesktopBannerActionStyle)>,
    dismissible: bool,
    palette: Palette,
    owner: Option<HWND>,
    scale: f32,
    fonts: Fonts,
    icon: Option<HICON>,
    layout: Layout,
    surface: Option<Surface>,
    hover: Hit,
    pressed: Hit,
    shown_at: Instant,
    closing_at: Option<Instant>,
    hook: Option<HWINEVENTHOOK>,
}

/// Window-relative device-pixel geometry; the card sits `SHADOW` inside.
#[derive(Debug, Clone, PartialEq)]
struct Layout {
    window: (i32, i32),
    card: RECT,
    icon: RECT,
    title: RECT,
    body: Option<RECT>,
    close: Option<RECT>,
    buttons: Vec<RECT>,
}

fn px(value: f32, scale: f32) -> i32 {
    (value * scale).round() as i32
}

fn compute_layout(
    scale: f32,
    title_height: i32,
    body_height: i32,
    button_widths: &[i32],
    dismissible: bool,
) -> Layout {
    let px = |value: f32| px(value, scale);
    let inset = px(SHADOW);
    let pad = px(PAD);
    let icon = px(ICON);
    let column = title_height
        + if body_height > 0 {
            px(2.0) + body_height
        } else {
            0
        };
    let mut height = pad + icon.max(column) + pad;
    if !button_widths.is_empty() {
        height += px(10.0) + px(BTN_H);
    }
    let card = RECT {
        left: inset,
        top: inset,
        right: inset + px(CARD_W),
        bottom: inset + height,
    };
    let icon_rect = RECT {
        left: card.left + pad,
        top: card.top + pad,
        right: card.left + pad + icon,
        bottom: card.top + pad + icon,
    };
    let close = dismissible.then(|| RECT {
        left: card.right - px(CLOSE_INSET) - px(CLOSE),
        top: card.top + px(CLOSE_INSET),
        right: card.right - px(CLOSE_INSET),
        bottom: card.top + px(CLOSE_INSET) + px(CLOSE),
    });
    // A text column shorter than the icon rides its midline.
    let text_top = card.top + pad + ((icon - column) / 2).max(0);
    let text_left = icon_rect.right + px(GAP);
    let title = RECT {
        left: text_left,
        top: text_top,
        right: close.map_or(card.right - pad, |close| close.left - px(4.0)),
        bottom: text_top + title_height,
    };
    let body = (body_height > 0).then(|| RECT {
        left: text_left,
        top: title.bottom + px(2.0),
        right: card.right - pad,
        bottom: title.bottom + px(2.0) + body_height,
    });
    let mut right = card.right - pad;
    let mut buttons = vec![RECT::default(); button_widths.len()];
    for (index, width) in button_widths.iter().enumerate().rev() {
        buttons[index] = RECT {
            left: right - width,
            top: card.bottom - pad - px(BTN_H),
            right,
            bottom: card.bottom - pad,
        };
        right -= width + px(BTN_GAP);
    }
    Layout {
        window: (card.right + inset, card.bottom + inset),
        card,
        icon: icon_rect,
        title,
        body,
        close,
        buttons,
    }
}

/// One or two lines, never the blank second line a short body would leave.
fn body_lines(measured_height: i32, line_height: i32) -> i32 {
    let line_height = line_height.max(1);
    ((measured_height + line_height - 1) / line_height).clamp(1, 2)
}

fn measure_layout(hdc: HDC, fonts: &Fonts, request: &Card, scale: f32) -> Layout {
    let px = |value: f32| px(value, scale);
    let title_height = line_height(hdc, fonts.title);
    let body_height = if request.body.is_empty() {
        0
    } else {
        let line = line_height(hdc, fonts.body);
        let width = px(CARD_W) - px(PAD) * 2 - px(ICON) - px(GAP);
        let measured = measure_text(hdc, fonts.body, &request.body, width, true).1;
        body_lines(measured, line) * line
    };
    let button_widths = request
        .actions
        .iter()
        .map(|(_, label, _)| {
            let text = measure_text(hdc, fonts.button, label, px(CARD_W), false).0;
            (text + px(BTN_PAD_X)).max(px(BTN_MIN_W))
        })
        .collect::<Vec<_>>();
    compute_layout(
        scale,
        title_height,
        body_height,
        &button_widths,
        request.dismissible,
    )
}

fn run_banner_thread(request: DesktopBannerShow, generation: u64) {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("LxDesktopBannerClass");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(banner_wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let owner = super::update_card::find_main_window();
        let scale = owner_scale(owner);
        // Unowned on purpose: an owned popup is hidden with a minimized host,
        // and a permission prompt must still reach the user then.
        let hwnd = match CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name,
            w!(""),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(hinstance.into()),
            None,
        ) {
            Ok(hwnd) => hwnd,
            Err(_) => {
                crate::desktop::banner::fail(&request.id, "failed to present banner");
                return;
            }
        };

        let mut card = Box::new(Card {
            id: request.id.clone(),
            title: request.title.clone(),
            body: request.body.clone(),
            actions: request
                .actions
                .iter()
                .map(|action| (action.id.clone(), action.label.clone(), action.style))
                .collect(),
            dismissible: request.actions.is_empty(),
            palette: Palette::of(&request.background),
            owner,
            scale,
            fonts: Fonts {
                title: HFONT::default(),
                body: HFONT::default(),
                button: HFONT::default(),
                glyph: None,
            },
            icon: None,
            layout: compute_layout(scale, 0, 0, &[], false),
            surface: None,
            hover: Hit::None,
            pressed: Hit::None,
            shown_at: Instant::now(),
            closing_at: None,
            hook: None,
        });
        rebuild(&mut card, scale);
        card.hook = owner.and_then(watch_owner);
        let polling = owner.is_some() && card.hook.is_none();
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(card) as isize);

        let current = {
            let mut slot = slot();
            let current = slot.generation == generation;
            if current {
                slot.hwnd = hwnd.0 as isize;
            }
            current
        };
        if current {
            if let Some(card) = card_mut(hwnd) {
                card.shown_at = Instant::now();
                draw(card);
                commit(hwnd, card);
            }
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            SetTimer(Some(hwnd), TIMER_ANIM, 15, None);
            if polling {
                SetTimer(Some(hwnd), TIMER_PIN, 30, None);
            }
        } else {
            let _ = DestroyWindow(hwnd);
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// (Re)load every DPI-dependent resource and lay the card out again.
fn rebuild(card: &mut Card, scale: f32) {
    card.scale = scale;
    card.fonts.release();
    if let Some(icon) = card.icon.take() {
        unsafe {
            let _ = DestroyIcon(icon);
        }
    }
    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        ReleaseDC(None, screen);
        card.fonts = Fonts::load(dc, scale);
        card.layout = measure_layout(dc, &card.fonts, card, scale);
        let _ = DeleteDC(dc);
    }
    card.icon = load_app_icon(px(ICON, scale));
    card.surface = Surface::new(card.layout.window.0, card.layout.window.1);
}

fn owner_scale(owner: Option<HWND>) -> f32 {
    let dpi = owner.map_or(0, |owner| unsafe { GetDpiForWindow(owner) });
    if dpi == 0 {
        dpi_scale()
    } else {
        dpi as f32 / 96.0
    }
}

/// Follow the host through move / size / minimize / hide without polling.
fn watch_owner(owner: HWND) -> Option<HWINEVENTHOOK> {
    unsafe {
        let thread = GetWindowThreadProcessId(owner, None);
        if thread == 0 {
            return None;
        }
        let hook = SetWinEventHook(
            EVENT_OBJECT_SHOW,
            EVENT_OBJECT_LOCATIONCHANGE,
            None,
            Some(owner_event),
            GetCurrentProcessId(),
            thread,
            WINEVENT_OUTOFCONTEXT,
        );
        (!hook.is_invalid()).then_some(hook)
    }
}

unsafe extern "system" fn owner_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    source: HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    if object != OBJID_WINDOW.0 || child != 0 {
        return;
    }
    if !matches!(
        event,
        EVENT_OBJECT_SHOW | EVENT_OBJECT_HIDE | EVENT_OBJECT_LOCATIONCHANGE
    ) {
        return;
    }
    let banner = slot().hwnd;
    if banner == 0 {
        return;
    }
    let banner = HWND(banner as *mut _);
    if let Some(card) = card_mut(banner)
        && card.owner == Some(source)
    {
        commit(banner, card);
    }
}

/// Visible frame of a host the card can sit on; `None` while it is minimized,
/// hidden, or gone. `GetWindowRect` would include the invisible resize border.
fn owner_frame(owner: HWND) -> Option<RECT> {
    unsafe {
        if !IsWindow(Some(owner)).as_bool()
            || !IsWindowVisible(owner).as_bool()
            || IsIconic(owner).as_bool()
        {
            return None;
        }
        let mut rect = RECT::default();
        if DwmGetWindowAttribute(
            owner,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut _ as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
        .is_ok()
            && rect.right > rect.left
        {
            return Some(rect);
        }
        GetWindowRect(owner, &mut rect).ok().map(|_| rect)
    }
}

fn work_area() -> RECT {
    let mut rect = RECT::default();
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
    rect
}

/// Top-left of the card (not the window) inside `anchor`.
fn banner_origin(anchor: RECT, width: i32, margin: i32) -> (i32, i32) {
    let x = (anchor.right - width - margin).max(anchor.left + margin.min(8));
    let y = anchor.top + margin;
    (x, y)
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// (slide offset factor, opacity) for the entrance and the fade-out.
fn animation_state(card: &Card) -> (f32, f32) {
    let enter = ease_out(card.shown_at.elapsed().as_secs_f32() * 1000.0 / ENTER_MS);
    let exit = card.closing_at.map_or(1.0, |at| {
        1.0 - (at.elapsed().as_secs_f32() * 1000.0 / EXIT_MS).clamp(0.0, 1.0)
    });
    (1.0 - enter, enter * exit)
}

/// Push the drawn surface to the screen at the pinned position.
fn commit(hwnd: HWND, card: &Card) {
    let Some(surface) = card.surface.as_ref() else {
        return;
    };
    let anchor = card.owner.and_then(owner_frame).unwrap_or_else(work_area);
    let width = card.layout.card.right - card.layout.card.left;
    let (x, y) = banner_origin(anchor, width, px(MARGIN, card.scale));
    let (slide, opacity) = animation_state(card);
    let destination = POINT {
        x: x - card.layout.card.left + (SLIDE * card.scale * slide) as i32,
        y: y - card.layout.card.top,
    };
    let size = SIZE {
        cx: surface.width,
        cy: surface.height,
    };
    let origin = POINT { x: 0, y: 0 };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        SourceConstantAlpha: (opacity * 255.0) as u8,
        AlphaFormat: AC_SRC_ALPHA as u8,
        ..Default::default()
    };
    unsafe {
        let _ = UpdateLayeredWindow(
            hwnd,
            None,
            Some(&destination),
            Some(&size),
            Some(surface.dc),
            Some(&origin),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );
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
        let format = DT_NOPREFIX
            | DT_CALCRECT
            | if wrap {
                DT_WORDBREAK | DT_EDITCONTROL
            } else {
                DT_SINGLELINE
            };
        let _ = DrawTextW(hdc, &mut wide[..n], &mut rect, format);
        SelectObject(hdc, old);
        (
            (rect.right - rect.left).max(0),
            (rect.bottom - rect.top).max(0),
        )
    }
}

fn make_font(scale: f32, size: f32, semibold: bool, face: &str) -> HFONT {
    let weight = if semibold { FW_SEMIBOLD } else { FW_NORMAL };
    let face = to_wide(face);
    unsafe {
        CreateFontW(
            -px(size, scale),
            0,
            0,
            0,
            weight.0 as i32,
            0,
            0,
            0,
            FONT_CHARSET(0),
            FONT_OUTPUT_PRECISION(0),
            FONT_CLIP_PRECISION(0),
            FONT_QUALITY(CLEARTYPE_QUALITY.0),
            0,
            PCWSTR(face.as_ptr()),
        )
    }
}

/// First face the font mapper really resolves; GDI silently substitutes a
/// missing face, which would turn the close glyph into a tofu box.
fn resolve_font(hdc: HDC, scale: f32, size: f32, semibold: bool, faces: &[&str]) -> Option<HFONT> {
    for face in faces {
        let font = make_font(scale, size, semibold, face);
        let resolved = unsafe {
            let old = SelectObject(hdc, font.into());
            let mut name = [0u16; 64];
            let len = GetTextFaceW(hdc, Some(&mut name));
            SelectObject(hdc, old);
            String::from_utf16_lossy(&name[..(len.max(1) - 1) as usize])
        };
        if resolved.eq_ignore_ascii_case(face) {
            return Some(font);
        }
        unsafe {
            let _ = DeleteObject(font.into());
        }
    }
    None
}

/// The exe icon at the drawn size; `ExtractIconExW` alone yields a 32px icon
/// that blurs when stretched at high DPI.
fn load_app_icon(size: i32) -> Option<HICON> {
    let exe = std::env::current_exe().ok()?;
    let wide = to_wide(&exe.to_string_lossy());
    unsafe {
        if wide.len() <= 260 {
            let mut path = [0u16; 260];
            path[..wide.len()].copy_from_slice(&wide);
            let mut icons = [HICON::default()];
            let count = PrivateExtractIconsW(&path, 0, size, size, Some(&mut icons), None, 0);
            if count > 0 && count != u32::MAX && !icons[0].is_invalid() {
                return Some(icons[0]);
            }
        }
        let mut large = HICON::default();
        let count = ExtractIconExW(PCWSTR(wide.as_ptr()), 0, Some(&mut large), None, 1);
        (count > 0 && !large.is_invalid()).then_some(large)
    }
}

struct Palette {
    background: Rgb,
    /// Card opacity from a `#RRGGBBAA` background.
    opacity: f32,
    title: Rgb,
    body: Rgb,
    /// Dark content on a light card; drives the neutral tints.
    light: bool,
}

impl Palette {
    fn of(background: &DesktopBannerBackground) -> Self {
        let light = match background {
            DesktopBannerBackground::System => !super::ui_update::windows_host_appearance_is_dark(),
            other => other.prefers_dark_content(),
        };
        let (fill, opacity) = match background {
            DesktopBannerBackground::Color { r, g, b, a } => ((*r, *g, *b), *a as f32 / 255.0),
            _ if light => ((249, 249, 251), 1.0),
            _ => ((44, 44, 46), 1.0),
        };
        let (title, body) = if light {
            ((28, 28, 30), (99, 99, 104))
        } else {
            ((255, 255, 255), (174, 174, 178))
        };
        Self {
            background: fill,
            opacity,
            title,
            body,
            light,
        }
    }

    /// Neutral overlay: ink on a light card, white on a dark one.
    fn tint(&self, strength: f32) -> Rgb {
        let ink = if self.light {
            (0, 0, 0)
        } else {
            (255, 255, 255)
        };
        mix(self.background, ink, strength)
    }

    fn button(&self, style: DesktopBannerActionStyle, hover: bool, pressed: bool) -> (Rgb, Rgb) {
        let accent = |base: Rgb| {
            let fill = if pressed {
                mix(base, (0, 0, 0), 0.12)
            } else if hover {
                mix(base, (255, 255, 255), 0.14)
            } else {
                base
            };
            (fill, (255, 255, 255))
        };
        match style {
            DesktopBannerActionStyle::Primary => accent((10, 132, 255)),
            DesktopBannerActionStyle::Destructive => accent((255, 69, 58)),
            DesktopBannerActionStyle::Default => {
                let base = if self.light { 0.06 } else { 0.12 };
                let boost = if pressed {
                    0.10
                } else if hover {
                    0.05
                } else {
                    0.0
                };
                (self.tint(base + boost), self.title)
            }
        }
    }
}

fn mix(from: Rgb, to: Rgb, amount: f32) -> Rgb {
    let amount = amount.clamp(0.0, 1.0);
    let channel = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
    (
        channel(from.0, to.0),
        channel(from.1, to.1),
        channel(from.2, to.2),
    )
}

fn colorref(color: Rgb) -> COLORREF {
    rgb(color.0, color.1, color.2)
}

fn opaque(color: Rgb) -> u32 {
    0xff00_0000 | (u32::from(color.0) << 16) | (u32::from(color.1) << 8) | u32::from(color.2)
}

/// Signed distance from a pixel centre to a rounded rect (negative inside).
fn rounded_distance(x: i32, y: i32, rect: &RECT, radius: f32) -> f32 {
    let half_w = (rect.right - rect.left) as f32 / 2.0;
    let half_h = (rect.bottom - rect.top) as f32 / 2.0;
    let radius = radius.min(half_w).min(half_h);
    let qx = (x as f32 + 0.5 - (rect.left as f32 + half_w)).abs() - (half_w - radius);
    let qy = (y as f32 + 0.5 - (rect.top as f32 + half_h)).abs() - (half_h - radius);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - radius
}

fn coverage(distance: f32) -> f32 {
    (0.5 - distance).clamp(0.0, 1.0)
}

/// Antialiased rounded-rect fill over opaque pixels.
fn fill_rounded(pixels: &mut [u32], stride: i32, rect: &RECT, radius: f32, color: Rgb) {
    for y in rect.top..rect.bottom {
        for x in rect.left..rect.right {
            let cover = coverage(rounded_distance(x, y, rect, radius));
            if cover <= 0.0 {
                continue;
            }
            let index = (y * stride + x) as usize;
            let under = pixels[index];
            let under = (
                ((under >> 16) & 0xff) as u8,
                ((under >> 8) & 0xff) as u8,
                (under & 0xff) as u8,
            );
            pixels[index] = opaque(mix(under, color, cover));
        }
    }
}

/// Hairline, silhouette, and shadow. GDI leaves garbage in the alpha byte, so
/// alpha is rebuilt from geometry and the colours premultiplied for
/// `ULW_ALPHA`.
fn compose(pixels: &mut [u32], layout: &Layout, palette: &Palette, scale: f32) {
    let (width, height) = layout.window;
    let card = layout.card;
    let radius = CARD_RADIUS * scale;
    let stroke = scale.round().max(1.0);
    let hairline = palette.tint(if palette.light { 0.10 } else { 0.16 });
    let drop = px(SHADOW_DROP, scale);
    let shadow_rect = RECT {
        top: card.top + drop,
        bottom: card.bottom + drop,
        ..card
    };
    let blur = ((SHADOW - SHADOW_DROP) * scale).max(1.0);
    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) as usize;
            let distance = rounded_distance(x, y, &card, radius);
            let cover = coverage(distance) * palette.opacity;
            let shadow = if cover >= 1.0 {
                0.0
            } else {
                let fade = 1.0 - rounded_distance(x, y, &shadow_rect, radius).max(0.0) / blur;
                SHADOW_ALPHA * fade.clamp(0.0, 1.0).powi(3)
            };
            let alpha = cover + (1.0 - coverage(distance)) * shadow;
            if alpha <= 0.0 {
                pixels[index] = 0;
                continue;
            }
            let pixel = pixels[index];
            let mut color = (
                ((pixel >> 16) & 0xff) as u8,
                ((pixel >> 8) & 0xff) as u8,
                (pixel & 0xff) as u8,
            );
            if distance > -stroke - 0.5 {
                color = mix(color, hairline, coverage(-(distance + stroke)));
            }
            let premultiplied = |channel: u8| (channel as f32 * cover).round() as u32;
            pixels[index] = (((alpha * 255.0).round() as u32) << 24)
                | (premultiplied(color.0) << 16)
                | (premultiplied(color.1) << 8)
                | premultiplied(color.2);
        }
    }
}

fn draw(card: &mut Card) {
    let Some(mut surface) = card.surface.take() else {
        return;
    };
    let scale = card.scale;
    let layout = &card.layout;
    let palette = &card.palette;
    let stride = surface.width;
    let dc = surface.dc;

    let pixels = surface.pixels();
    pixels.fill(opaque(palette.background));
    if card.icon.is_none() {
        fill_rounded(
            pixels,
            stride,
            &layout.icon,
            ICON_RADIUS * scale,
            (10, 132, 255),
        );
    }
    if let Some(close) = layout.close.as_ref()
        && (card.hover == Hit::Close || card.pressed == Hit::Close)
    {
        let strength = if card.pressed == Hit::Close {
            0.16
        } else {
            0.10
        };
        fill_rounded(pixels, stride, close, CLOSE * scale, palette.tint(strength));
    }
    let mut labels = Vec::with_capacity(card.actions.len());
    for (index, ((_, label, style), rect)) in card.actions.iter().zip(&layout.buttons).enumerate() {
        let hit = Hit::Action(index);
        let (fill, text) = palette.button(*style, card.hover == hit, card.pressed == hit);
        fill_rounded(pixels, stride, rect, BTN_RADIUS * scale, fill);
        labels.push((label, *rect, text));
    }

    unsafe {
        SetBkMode(dc, TRANSPARENT);
        if let Some(icon) = card.icon {
            let rect = layout.icon;
            let radius = px(ICON_RADIUS, scale) * 2;
            let region = CreateRoundRectRgn(
                rect.left,
                rect.top,
                rect.right + 1,
                rect.bottom + 1,
                radius,
                radius,
            );
            let saved = SaveDC(dc);
            SelectClipRgn(dc, Some(region));
            let size = rect.right - rect.left;
            let _ = DrawIconEx(
                dc, rect.left, rect.top, icon, size, size, 0, None, DI_NORMAL,
            );
            let _ = RestoreDC(dc, saved);
            let _ = DeleteObject(region.into());
        }
        let single = DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX;
        draw_text(
            dc,
            card.fonts.title,
            &card.title,
            layout.title,
            palette.title,
            DT_LEFT | DT_END_ELLIPSIS | single,
        );
        if let Some(body) = layout.body {
            draw_text(
                dc,
                card.fonts.body,
                &card.body,
                body,
                palette.body,
                DT_LEFT | DT_WORDBREAK | DT_EDITCONTROL | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
        }
        if let Some(close) = layout.close {
            let color = if card.hover == Hit::Close {
                palette.title
            } else {
                palette.body
            };
            match card.fonts.glyph {
                Some(font) => draw_text(dc, font, CLOSE_GLYPH, close, color, DT_CENTER | single),
                None => draw_text(dc, card.fonts.title, "×", close, color, DT_CENTER | single),
            }
        }
        for (label, rect, color) in labels {
            draw_text(
                dc,
                card.fonts.button,
                label,
                rect,
                color,
                DT_CENTER | single,
            );
        }
        let _ = GdiFlush();
    }

    compose(surface.pixels(), layout, palette, scale);
    card.surface = Some(surface);
}

fn draw_text(
    hdc: HDC,
    font: HFONT,
    text: &str,
    mut rect: RECT,
    color: Rgb,
    format: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    unsafe {
        let old = SelectObject(hdc, font.into());
        SetTextColor(hdc, colorref(color));
        let mut wide = to_wide(text);
        let n = wide.len().saturating_sub(1);
        DrawTextW(hdc, &mut wide[..n], &mut rect, format);
        SelectObject(hdc, old);
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
            WM_MOUSEMOVE => {
                set_hit(hwnd, Some(hit_test(hwnd, lparam)), None);
                track_mouse_leave(hwnd);
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                set_hit(hwnd, Some(Hit::None), Some(Hit::None));
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                set_hit(hwnd, None, Some(hit_test(hwnd, lparam)));
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                on_click(hwnd, lparam);
                LRESULT(0)
            }
            WM_TIMER => {
                on_timer(hwnd, wparam.0);
                LRESULT(0)
            }
            WM_APP_HIDE => {
                match card_mut(hwnd) {
                    Some(card) if card.closing_at.is_none() => {
                        card.closing_at = Some(Instant::now());
                        SetTimer(Some(hwnd), TIMER_ANIM, 15, None);
                    }
                    Some(_) => {}
                    None => {
                        let _ = DestroyWindow(hwnd);
                    }
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DPICHANGED => {
                if let Some(card) = card_mut(hwnd) {
                    let dpi = (wparam.0 & 0xFFFF) as f32;
                    rebuild(card, if dpi > 0.0 { dpi / 96.0 } else { card.scale });
                    draw(card);
                    commit(hwnd, card);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                if let Some(card) = take_card(hwnd) {
                    if let Some(hook) = card.hook {
                        let _ = UnhookWinEvent(hook);
                    }
                    card.fonts.release();
                    if let Some(icon) = card.icon {
                        let _ = DestroyIcon(icon);
                    }
                }
                {
                    let mut slot = slot();
                    if slot.hwnd == hwnd.0 as isize {
                        slot.hwnd = 0;
                    }
                }
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn on_timer(hwnd: HWND, timer: usize) {
    let Some(card) = card_mut(hwnd) else {
        return;
    };
    commit(hwnd, card);
    if timer != TIMER_ANIM {
        return;
    }
    let entered = card.shown_at.elapsed().as_secs_f32() * 1000.0 >= ENTER_MS;
    let closed = card
        .closing_at
        .is_some_and(|at| at.elapsed().as_secs_f32() * 1000.0 >= EXIT_MS);
    unsafe {
        if closed {
            let _ = DestroyWindow(hwnd);
        } else if entered && card.closing_at.is_none() {
            let _ = KillTimer(Some(hwnd), TIMER_ANIM);
        }
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

fn hit_test(hwnd: HWND, lparam: LPARAM) -> Hit {
    let Some(card) = card_ref(hwnd) else {
        return Hit::None;
    };
    let point = point_from(lparam);
    if card.layout.close.is_some_and(|close| pt_in(close, point)) {
        return Hit::Close;
    }
    card.layout
        .buttons
        .iter()
        .position(|rect| pt_in(*rect, point))
        .map_or(Hit::None, Hit::Action)
}

fn set_hit(hwnd: HWND, hover: Option<Hit>, pressed: Option<Hit>) {
    let Some(card) = card_mut(hwnd) else {
        return;
    };
    let hover = hover.unwrap_or(card.hover);
    let pressed = pressed.unwrap_or(card.pressed);
    if hover == card.hover && pressed == card.pressed {
        return;
    }
    card.hover = hover;
    card.pressed = pressed;
    draw(card);
    commit(hwnd, card);
}

fn on_click(hwnd: HWND, lparam: LPARAM) {
    let hit = hit_test(hwnd, lparam);
    let armed = card_ref(hwnd).is_some_and(|card| card.pressed == hit && card.closing_at.is_none());
    set_hit(hwnd, None, Some(Hit::None));
    let Some(card) = card_ref(hwnd) else {
        return;
    };
    if !armed {
        return;
    }
    let id = card.id.clone();
    let outcome = match hit {
        Hit::None => return,
        Hit::Close => DesktopBannerOutcome::Dismissed { id: id.clone() },
        Hit::Action(index) => match card.actions.get(index) {
            Some((action, _, _)) => DesktopBannerOutcome::Action {
                id: id.clone(),
                action: action.clone(),
            },
            None => return,
        },
    };
    crate::desktop::banner::complete(&id, outcome);
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
    use super::*;

    #[test]
    fn banner_origin_pins_to_the_anchor_top_right() {
        let anchor = RECT {
            left: 40,
            top: 20,
            right: 840,
            bottom: 620,
        };
        assert_eq!(banner_origin(anchor, 328, 16), (496, 36));
    }

    #[test]
    fn banner_origin_stays_inside_a_narrow_anchor() {
        let anchor = RECT {
            left: 100,
            top: 10,
            right: 300,
            bottom: 400,
        };
        assert_eq!(banner_origin(anchor, 328, 16), (108, 26));
    }

    #[test]
    fn body_takes_only_the_lines_it_needs() {
        assert_eq!(body_lines(17, 17), 1);
        assert_eq!(body_lines(34, 17), 2);
        assert_eq!(body_lines(85, 17), 2);
        assert_eq!(body_lines(0, 17), 1);
    }

    #[test]
    fn title_only_toast_is_icon_high_with_a_centred_title() {
        let layout = compute_layout(1.0, 18, 0, &[], true);
        let card = layout.card;
        assert_eq!(card.bottom - card.top, 60);
        assert_eq!(
            layout.title.top + layout.title.bottom,
            layout.icon.top + layout.icon.bottom
        );
        assert!(layout.title.right <= layout.close.unwrap().left);
        assert_eq!(layout.window, (328 + 52, 60 + 52));
    }

    #[test]
    fn buttons_are_right_aligned_in_request_order() {
        let layout = compute_layout(1.0, 18, 34, &[56, 70], false);
        let card = layout.card;
        assert_eq!(layout.buttons[1].right, card.right - 12);
        assert_eq!(layout.buttons[0].right, layout.buttons[1].left - 8);
        assert_eq!(layout.buttons[1].bottom, card.bottom - 12);
        assert!(layout.body.unwrap().bottom + 10 <= layout.buttons[0].top);
    }

    #[test]
    fn rounded_coverage_clears_the_corner_and_fills_the_middle() {
        let rect = RECT {
            left: 0,
            top: 0,
            right: 100,
            bottom: 60,
        };
        assert_eq!(coverage(rounded_distance(0, 0, &rect, 12.0)), 0.0);
        assert_eq!(coverage(rounded_distance(50, 30, &rect, 12.0)), 1.0);
        assert_eq!(coverage(rounded_distance(50, 0, &rect, 12.0)), 1.0);
    }
}
