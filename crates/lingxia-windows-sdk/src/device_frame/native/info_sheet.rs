use super::*;

use crate::{WindowsDeviceFrameSheetIcon, draw_windows_design_icon_with_color};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CombineRgn, CreateFontW, CreateRoundRectRgn, CreateSolidBrush, DT_CENTER,
    DT_END_ELLIPSIS, DT_LEFT, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, DeleteObject, DrawTextW,
    EndPaint, FillRect, GetTextExtentPoint32W, HBRUSH, HFONT, HGDIOBJ, InvalidateRect, PAINTSTRUCT,
    RGN_AND, SelectObject, SetBkMode, SetTextColor, SetWindowRgn, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::LWA_ALPHA;

// Sheet metrics are device-independent px; [`px`] applies the device frame's
// display scale, as the capsule and status bar do.
const HEADER_TOP: i32 = 16;
const HORIZONTAL_PADDING: i32 = 20;
const HEADER_HEIGHT: i32 = 24;
const HEADER_SEPARATOR_TOP_GAP: i32 = 12;
const SEPARATOR_HEIGHT: i32 = 1;
const BUTTON_ROW_TOP_GAP: i32 = 12;
const BUTTON_COLUMNS: usize = 5;
const BUTTON_LIMIT: usize = 10;
const BUTTON_ROW_HEIGHT: i32 = 88;
const BUTTON_ROW_GAP: i32 = 8;
const BOTTOM_PADDING: i32 = 16;
const BUTTON_GAP: i32 = 4;
const BUTTON_ICON_SIZE: i32 = 24;
const BUTTON_ICON_TOP: i32 = 12;
const BUTTON_LABEL_GAP: i32 = 6;
const SHEET_CORNER_RADIUS: i32 = 16;
const BADGE_HEIGHT: i32 = 16;
const BADGE_MIN_WIDTH: i32 = 34;
const BADGE_TEXT_HORIZONTAL_PADDING: i32 = 10;
const BADGE_VERSION_GAP: i32 = 4;
const HEADER_DIVIDER_WIDTH: i32 = 18;
const TITLE_FONT_HEIGHT: i32 = 16;
const VERSION_FONT_HEIGHT: i32 = 14;
const BADGE_FONT_HEIGHT: i32 = 10;
const ACTION_FONT_HEIGHT: i32 = 13;

/// A device-independent length at `scale`.
fn px(value: i32, scale: f64) -> i32 {
    (f64::from(value) * scale).round() as i32
}

fn content_scale(content: HWND) -> f64 {
    content_fit_scale(hwnd_handle(content)).unwrap_or(1.0)
}

/// Display scale of the content window that owns the sheet, read live so a
/// monitor change while the sheet is open repaints at the new scale.
fn owner_scale(sheet: HWND) -> f64 {
    let owner = unsafe { WindowsAndMessaging::GetWindow(sheet, WindowsAndMessaging::GW_OWNER) }
        .unwrap_or_default();
    content_scale(owner)
}

/// Opaque RGB (0xRRGGBB) for the capsule click sheet. Matches the iOS/Android
/// overlay palettes; the simulated host scheme, not the OS.
struct OverlayPalette {
    surface: u32,
    primary: u32,
    secondary: u32,
    separator_text: u32,
    action: u32,
    separator: u32,
    mask_alpha: u8,
}

fn overlay_palette() -> OverlayPalette {
    if super::overlay_is_dark() {
        OverlayPalette {
            surface: 0x1C1C1E,
            primary: 0xFFFFFF,
            secondary: 0xA1A1A1,
            separator_text: 0x8E8E93,
            action: 0xE5E5EA,
            separator: 0x3A3A3C,
            mask_alpha: 140,
        }
    } else {
        OverlayPalette {
            surface: 0xFFFFFF,
            primary: 0x000000,
            secondary: 0x999999,
            separator_text: 0xCCCCCC,
            action: 0x333333,
            separator: 0xEEEEEE,
            mask_alpha: 102,
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::device_frame) struct DeviceFrameInfoSheet {
    pub(in crate::device_frame) title: String,
    pub(in crate::device_frame) version: String,
    pub(in crate::device_frame) badge: Option<InfoSheetBadge>,
    pub(in crate::device_frame) actions: Vec<SheetAction>,
}

/// Caller-supplied header badge (text + colors); the device frame renders it
/// verbatim without interpreting what it means.
#[derive(Debug, Clone)]
pub(in crate::device_frame) struct InfoSheetBadge {
    pub(in crate::device_frame) text: String,
    pub(in crate::device_frame) foreground: u32,
    pub(in crate::device_frame) background: u32,
}

#[derive(Debug, Clone, Copy)]
struct AboutSheetWindows {
    mask: isize,
    sheet: isize,
    /// The capsule opens the sheet on mouse-*down*, so the opening click's
    /// mouse-*up* lands on the just-created mask and would
    /// instantly dismiss it. Swallow that first up.
    ignore_next_mouse_up: bool,
}

/// Caller-supplied action row: explicit icon + command, no inference.
#[derive(Debug, Clone)]
pub(in crate::device_frame) struct SheetAction {
    pub(in crate::device_frame) label: String,
    pub(in crate::device_frame) command: u32,
    pub(in crate::device_frame) icon: WindowsDeviceFrameSheetIcon,
}

static ABOUT_SHEETS: OnceLock<Mutex<HashMap<isize, AboutSheetWindows>>> = OnceLock::new();

pub(super) fn show_info_sheet(content: HWND, info: DeviceFrameInfoSheet) {
    if !is_window_handle_valid(hwnd_handle(content)) {
        return;
    }

    dismiss_info_sheet_for_content(hwnd_handle(content));

    let Some(rect) = content_rect(content) else {
        return;
    };
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    let instance = unsafe { LibraryLoader::GetModuleHandleW(None) }
        .ok()
        .map(|module| HINSTANCE(module.0));
    let mask = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_NOACTIVATE,
            info_mask_class(),
            PCWSTR::null(),
            WindowsAndMessaging::WS_POPUP,
            rect.left,
            rect.top,
            width,
            height,
            Some(content),
            None,
            instance,
            None,
        )
    };
    let Ok(mask) = mask else {
        return;
    };

    let sheet_height = sheet_height(info.actions.len(), content_scale(content)).min(height.max(1));
    let sheet = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_TOOLWINDOW | WindowsAndMessaging::WS_EX_NOACTIVATE,
            info_sheet_class(),
            PCWSTR::null(),
            WindowsAndMessaging::WS_POPUP,
            rect.left,
            rect.bottom - sheet_height,
            width,
            sheet_height,
            Some(content),
            None,
            instance,
            None,
        )
    };
    let Ok(sheet) = sheet else {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(mask);
        }
        return;
    };

    unsafe {
        WindowsAndMessaging::SetWindowLongPtrW(
            sheet,
            WindowsAndMessaging::GWLP_USERDATA,
            Box::into_raw(Box::new(info)) as isize,
        );
        let _ = WindowsAndMessaging::SetLayeredWindowAttributes(
            mask,
            COLORREF(0),
            overlay_palette().mask_alpha,
            LWA_ALPHA,
        );
    }

    let sheets = ABOUT_SHEETS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut sheets) = sheets.lock() {
        sheets.insert(
            hwnd_handle(content),
            AboutSheetWindows {
                mask: hwnd_handle(mask),
                sheet: hwnd_handle(sheet),
                ignore_next_mouse_up: true,
            },
        );
    }

    reposition_info_sheet(content);
}

fn info_mask_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(mask_proc),
            hInstance: module,
            hbrBackground: HBRUSH(
                unsafe {
                    windows::Win32::Graphics::Gdi::GetStockObject(
                        windows::Win32::Graphics::Gdi::BLACK_BRUSH,
                    )
                }
                .0,
            ),
            lpszClassName: w!("LingXiaDeviceAboutMask"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "device about mask class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaDeviceAboutMask")
}

fn info_sheet_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let cursor =
            unsafe { WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_ARROW) }
                .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(sheet_proc),
            hInstance: module,
            hCursor: cursor,
            lpszClassName: w!("LingXiaDeviceAboutSheet"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "device about sheet class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaDeviceAboutSheet")
}

unsafe extern "system" fn mask_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WindowsAndMessaging::WM_LBUTTONUP {
        if consume_initial_mouse_up_for_window(hwnd) {
            return LRESULT(0);
        }
        dismiss_info_sheet_for_window(hwnd);
        return LRESULT(0);
    }
    unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
}

unsafe extern "system" fn sheet_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_PAINT => {
            paint_sheet(hwnd);
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_LBUTTONDOWN => {
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_LBUTTONUP => {
            let x = (lparam.0 & 0xffff) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
            match sheet_hit_at(hwnd, x, y) {
                Some(SheetHit::Action(id)) => {
                    dismiss_info_sheet_for_window(hwnd);
                    dispatch_device_frame_command(id);
                }
                None => {
                    let _ = consume_initial_mouse_up_for_window(hwnd);
                }
            }
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_NCDESTROY => {
            let raw = unsafe {
                WindowsAndMessaging::SetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA, 0)
            };
            if raw != 0 {
                unsafe {
                    drop(Box::from_raw(raw as *mut DeviceFrameInfoSheet));
                }
            }
        }
        _ => {}
    }
    unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn sheet_height(action_count: usize, scale: f64) -> i32 {
    button_grid_top(scale) + action_grid_height(action_count, scale) + px(BOTTOM_PADDING, scale)
}

fn button_grid_top(scale: f64) -> i32 {
    px(
        HEADER_TOP
            + HEADER_HEIGHT
            + HEADER_SEPARATOR_TOP_GAP
            + SEPARATOR_HEIGHT
            + BUTTON_ROW_TOP_GAP,
        scale,
    )
}

fn action_grid_height(action_count: usize, scale: f64) -> i32 {
    let rows = action_count.clamp(1, BUTTON_LIMIT).div_ceil(BUTTON_COLUMNS) as i32;
    (rows - 1) * px(BUTTON_ROW_HEIGHT + BUTTON_ROW_GAP, scale) + px(BUTTON_ROW_HEIGHT, scale)
}

enum SheetHit {
    Action(u32),
}

fn sheet_hit_at(hwnd: HWND, x: i32, y: i32) -> Option<SheetHit> {
    let info = sheet_info(hwnd)?;
    let actions = &info.actions;
    if actions.is_empty() {
        return None;
    }
    let scale = owner_scale(hwnd);
    let grid = button_grid_rect(sheet_client_rect(hwnd), actions.len(), scale);
    if y < grid.top || y >= grid.bottom {
        return None;
    }
    for (index, action) in actions.iter().take(BUTTON_LIMIT).enumerate() {
        if rect_contains(action_button_rect(grid, actions.len(), index, scale), x, y) {
            return Some(SheetHit::Action(action.command));
        }
    }
    None
}

fn draw_separator(dc: HDC, left: i32, top: i32, right: i32, height: i32, color: u32) {
    let line = RECT {
        left,
        top,
        right,
        bottom: top + height,
    };
    unsafe {
        let brush = CreateSolidBrush(rgb_to_colorref(color));
        let _ = FillRect(dc, &line, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

fn paint_capsule_menu(
    dc: HDC,
    client: &RECT,
    info: &DeviceFrameInfoSheet,
    palette: &OverlayPalette,
    scale: f64,
) {
    paint_header(dc, client, info, palette, scale);
    let separator_top = px(HEADER_TOP + HEADER_HEIGHT + HEADER_SEPARATOR_TOP_GAP, scale);
    let separator_height = px(SEPARATOR_HEIGHT, scale).max(1);
    draw_separator(
        dc,
        0,
        separator_top,
        client.right,
        separator_height,
        palette.separator,
    );
    paint_action_buttons(dc, client, info, palette, scale);
}

/// Header text slots, sized from measured text: a per-character estimate
/// undersizes CJK glyphs and ellipsized short Chinese titles.
struct HeaderLayout {
    title: Option<RECT>,
    divider: RECT,
    version: RECT,
    badge: Option<RECT>,
}

/// `measure(text, height, weight)` is the drawn width of `text` in the font
/// `draw_sheet_text` creates for that `CreateFontW` height and weight.
fn header_layout(
    client_right: i32,
    info: &DeviceFrameInfoSheet,
    scale: f64,
    measure: impl Fn(&str, i32, i32) -> i32,
) -> HeaderLayout {
    let left = px(HORIZONTAL_PADDING, scale);
    let right = client_right - px(HORIZONTAL_PADDING, scale);
    let top = px(HEADER_TOP, scale);
    let bottom = px(HEADER_TOP + HEADER_HEIGHT, scale);
    let slot = |left: i32, right: i32| RECT {
        left,
        top,
        right,
        bottom,
    };
    let app_name = info.title.trim();
    let title = (!app_name.is_empty()).then(|| {
        let width =
            measure(app_name, -px(TITLE_FONT_HEIGHT, scale), 600).min((client_right - left) / 2);
        slot(left, left + width)
    });
    let divider_left = title.map_or(left, |title| title.right);
    let divider = slot(divider_left, divider_left + px(HEADER_DIVIDER_WIDTH, scale));
    let cursor = divider.right;
    let badge_width = info.badge.as_ref().map(|badge| {
        let padding = px(BADGE_TEXT_HORIZONTAL_PADDING, scale);
        measure(&badge.text, -px(BADGE_FONT_HEIGHT, scale), 600)
            .max(px(BADGE_MIN_WIDTH, scale) - padding)
            + padding
    });
    let gap = px(BADGE_VERSION_GAP, scale);
    let version_right = match badge_width {
        Some(badge_width) => {
            let available = (right - badge_width - gap - cursor).max(0);
            cursor
                + measure(info.version.trim(), -px(VERSION_FONT_HEIGHT, scale), 400).min(available)
        }
        None => right,
    };
    let version = slot(cursor, version_right);
    let badge = badge_width.map(|badge_width| {
        let badge_left = (version.right + gap).min(right - badge_width).max(cursor);
        let badge_top = top - px(2, scale);
        RECT {
            left: badge_left,
            top: badge_top,
            right: badge_left + badge_width,
            bottom: badge_top + px(BADGE_HEIGHT, scale),
        }
    });
    HeaderLayout {
        title,
        divider,
        version,
        badge,
    }
}

fn paint_header(
    dc: HDC,
    client: &RECT,
    info: &DeviceFrameInfoSheet,
    palette: &OverlayPalette,
    scale: f64,
) {
    let layout = header_layout(client.right, info, scale, |text, height, weight| {
        measure_sheet_text(dc, text, height, weight)
    });
    let title_height = -px(TITLE_FONT_HEIGHT, scale);
    if let Some(rect) = layout.title {
        draw_sheet_text(
            dc,
            info.title.trim(),
            rect,
            title_height,
            600,
            palette.primary,
            DT_LEFT,
        );
    }
    draw_sheet_text(
        dc,
        " | ",
        layout.divider,
        title_height,
        400,
        palette.separator_text,
        DT_LEFT,
    );
    draw_sheet_text(
        dc,
        info.version.trim(),
        layout.version,
        -px(VERSION_FONT_HEIGHT, scale),
        400,
        palette.secondary,
        DT_LEFT,
    );
    if let (Some(badge), Some(rect)) = (info.badge.as_ref(), layout.badge) {
        fill_rect(dc, rect, badge.background);
        draw_sheet_text(
            dc,
            &badge.text,
            rect,
            -px(BADGE_FONT_HEIGHT, scale),
            600,
            badge.foreground,
            DT_CENTER,
        );
    }
}

fn paint_action_buttons(
    dc: HDC,
    client: &RECT,
    info: &DeviceFrameInfoSheet,
    palette: &OverlayPalette,
    scale: f64,
) {
    let actions = &info.actions;
    if actions.is_empty() {
        return;
    }
    let grid = button_grid_rect(*client, actions.len(), scale);
    let icon_size = px(BUTTON_ICON_SIZE, scale);
    for (index, action) in actions.iter().take(BUTTON_LIMIT).enumerate() {
        let rect = action_button_rect(grid, actions.len(), index, scale);
        let icon_top = rect.top + px(BUTTON_ICON_TOP, scale);
        let icon_rect = RECT {
            left: rect.left + (rect.right - rect.left - icon_size) / 2,
            top: icon_top,
            right: rect.left + (rect.right - rect.left + icon_size) / 2,
            bottom: icon_top + icon_size,
        };
        match &action.icon {
            WindowsDeviceFrameSheetIcon::Design(icon) => {
                let _ = draw_windows_design_icon_with_color(dc, *icon, icon_rect, palette.action);
            }
            WindowsDeviceFrameSheetIcon::Path(path) => {
                let _ = crate::shell::draw_icon_from_path(dc, path, icon_rect, icon_size as u32);
            }
        }
        draw_sheet_wrapped_text(
            dc,
            &action.label,
            RECT {
                left: rect.left + px(4, scale),
                top: icon_rect.bottom + px(BUTTON_LABEL_GAP, scale),
                right: rect.right - px(4, scale),
                bottom: rect.bottom - px(8, scale),
            },
            -px(ACTION_FONT_HEIGHT, scale),
            400,
            palette.action,
            DT_CENTER,
        );
    }
}

fn button_grid_rect(client: RECT, action_count: usize, scale: f64) -> RECT {
    let top = button_grid_top(scale);
    RECT {
        left: px(HORIZONTAL_PADDING, scale),
        top,
        right: client.right - px(HORIZONTAL_PADDING, scale),
        bottom: top + action_grid_height(action_count, scale),
    }
}

fn action_button_rect(grid: RECT, action_count: usize, index: usize, scale: f64) -> RECT {
    let row = index / BUTTON_COLUMNS;
    let row_start = row * BUTTON_COLUMNS;
    let columns = BUTTON_COLUMNS.min(action_count.saturating_sub(row_start)) as i32;
    let gap = px(BUTTON_GAP, scale);
    let width = ((grid.right - grid.left - gap * (columns - 1)) / columns).max(1);
    let column = (index % BUTTON_COLUMNS) as i32;
    let left = grid.left + column * (width + gap);
    let top = grid.top + row as i32 * px(BUTTON_ROW_HEIGHT + BUTTON_ROW_GAP, scale);
    RECT {
        left,
        top,
        right: if column + 1 == columns {
            grid.right
        } else {
            left + width
        },
        bottom: top + px(BUTTON_ROW_HEIGHT, scale),
    }
}

fn paint_sheet(hwnd: HWND) {
    let Some(info) = sheet_info(hwnd).cloned() else {
        return;
    };
    let mut ps = PAINTSTRUCT::default();
    let dc = unsafe { BeginPaint(hwnd, &mut ps) };
    if dc.is_invalid() {
        return;
    }

    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    let palette = overlay_palette();
    fill_rect(dc, client, palette.surface);

    paint_capsule_menu(dc, &client, &info, &palette, owner_scale(hwnd));

    unsafe {
        let _ = EndPaint(hwnd, &ps);
    }
}

fn fill_rect(dc: HDC, rect: RECT, color: u32) {
    unsafe {
        let brush = CreateSolidBrush(rgb_to_colorref(color));
        let _ = FillRect(dc, &rect, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

fn draw_sheet_text(
    dc: HDC,
    text: &str,
    rect: RECT,
    height: i32,
    weight: i32,
    color: u32,
    align: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    draw_sheet_text_with_flags(
        dc,
        text,
        rect,
        height,
        weight,
        color,
        align | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
    );
}

fn draw_sheet_wrapped_text(
    dc: HDC,
    text: &str,
    rect: RECT,
    height: i32,
    weight: i32,
    color: u32,
    align: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    draw_sheet_text_with_flags(
        dc,
        text,
        rect,
        height,
        weight,
        color,
        align | DT_WORDBREAK | DT_END_ELLIPSIS,
    );
}

fn draw_sheet_text_with_flags(
    dc: HDC,
    text: &str,
    mut rect: RECT,
    height: i32,
    weight: i32,
    color: u32,
    flags: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    let font = sheet_font(height, weight);
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let old_font = SelectObject(dc, HGDIOBJ(font.0));
        let _ = SetBkMode(dc, TRANSPARENT);
        let _ = SetTextColor(dc, rgb_to_colorref(color));
        let _ = DrawTextW(dc, &mut wide, &mut rect, flags);
        if !old_font.is_invalid() {
            let _ = SelectObject(dc, old_font);
        }
        if !font.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
    }
}

/// Width of `text` drawn into `dc` by [`draw_sheet_text`] with the same
/// height and weight.
fn measure_sheet_text(dc: HDC, text: &str, height: i32, weight: i32) -> i32 {
    let font = sheet_font(height, weight);
    let wide: Vec<u16> = text.encode_utf16().collect();
    let mut size = SIZE::default();
    unsafe {
        let old_font = SelectObject(dc, HGDIOBJ(font.0));
        let _ = GetTextExtentPoint32W(dc, &wide, &mut size);
        if !old_font.is_invalid() {
            let _ = SelectObject(dc, old_font);
        }
        if !font.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
    }
    size.cx
}

/// Caller-owned "Segoe UI" font; delete it after use.
fn sheet_font(height: i32, weight: i32) -> HFONT {
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            w!("Segoe UI"),
        )
    }
}

fn sheet_info<'a>(hwnd: HWND) -> Option<&'a DeviceFrameInfoSheet> {
    let raw =
        unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA) };
    if raw == 0 {
        None
    } else {
        Some(unsafe { &*(raw as *const DeviceFrameInfoSheet) })
    }
}

pub(super) fn reposition_info_sheet(content: HWND) {
    let handle = hwnd_handle(content);
    let Some(windows) = ABOUT_SHEETS
        .get()
        .and_then(|sheets| sheets.lock().ok())
        .and_then(|sheets| sheets.get(&handle).copied())
    else {
        return;
    };
    let Some(rect) = content_rect(content) else {
        return;
    };
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    let screen_corner_radius = content_screen_corner_radius(content);
    let scale = content_scale(content);
    let sheet_height = if is_window_handle_valid(windows.sheet) {
        sheet_info(hwnd_from_handle(windows.sheet))
            .map(|info| sheet_height(info.actions.len(), scale))
            .unwrap_or_else(|| sheet_height(0, scale))
    } else {
        sheet_height(0, scale)
    }
    .min(height.max(1));
    unsafe {
        if is_window_handle_valid(windows.mask) {
            let mask_hwnd = hwnd_from_handle(windows.mask);
            let _ = WindowsAndMessaging::SetWindowPos(
                mask_hwnd,
                Some(WindowsAndMessaging::HWND_TOP),
                rect.left,
                rect.top,
                width,
                height,
                WindowsAndMessaging::SWP_NOACTIVATE | WindowsAndMessaging::SWP_SHOWWINDOW,
            );
            apply_screen_region(mask_hwnd, width, height, screen_corner_radius);
        }
        if is_window_handle_valid(windows.sheet) {
            let sheet_hwnd = hwnd_from_handle(windows.sheet);
            let _ = WindowsAndMessaging::SetWindowPos(
                sheet_hwnd,
                Some(WindowsAndMessaging::HWND_TOP),
                rect.left,
                rect.bottom - sheet_height,
                width,
                sheet_height,
                WindowsAndMessaging::SWP_NOACTIVATE | WindowsAndMessaging::SWP_SHOWWINDOW,
            );
            apply_sheet_region(
                sheet_hwnd,
                width,
                height,
                sheet_height,
                px(SHEET_CORNER_RADIUS, scale),
                screen_corner_radius,
            );
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(sheet_hwnd), None, true);
        }
    }
}

/// Returns true (and clears the flag) for the first mouse-up after the sheet
/// opens — the release of the capsule click that opened it, which must not
/// dismiss or trigger an action.
fn consume_initial_mouse_up_for_window(window: HWND) -> bool {
    let handle = hwnd_handle(window);
    ABOUT_SHEETS
        .get()
        .and_then(|sheets| sheets.lock().ok())
        .and_then(|mut sheets| {
            let (_, windows) = sheets
                .iter_mut()
                .find(|(_, windows)| windows.mask == handle || windows.sheet == handle)?;
            if windows.ignore_next_mouse_up {
                windows.ignore_next_mouse_up = false;
                Some(true)
            } else {
                Some(false)
            }
        })
        .unwrap_or(false)
}

fn apply_screen_region(window: HWND, width: i32, height: i32, screen_corner_radius: i32) {
    if width <= 0 || height <= 0 {
        return;
    }
    if screen_corner_radius <= 0 {
        unsafe {
            let _ = SetWindowRgn(window, None, true);
        }
        return;
    }
    let diameter = screen_corner_radius * 2;
    unsafe {
        let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, diameter, diameter);
        if region.is_invalid() {
            return;
        }
        let applied = SetWindowRgn(window, Some(region), true);
        if applied == 0 {
            let _ = DeleteObject(HGDIOBJ(region.0));
        }
    }
}

fn apply_sheet_region(
    sheet: HWND,
    width: i32,
    screen_height: i32,
    sheet_height: i32,
    sheet_corner_radius: i32,
    screen_corner_radius: i32,
) {
    if width <= 0 || screen_height <= 0 || sheet_height <= 0 {
        return;
    }
    let sheet_diameter = sheet_corner_radius * 2;
    unsafe {
        // Extend the rounded region below the window so the sheet keeps only
        // its own top corners, then intersect it with the device screen shape
        // so the bottom edge cannot paint outside the phone's lower corners.
        let sheet_region = CreateRoundRectRgn(
            0,
            0,
            width + 1,
            sheet_height + sheet_corner_radius + 1,
            sheet_diameter,
            sheet_diameter,
        );
        if sheet_region.is_invalid() {
            return;
        }

        if screen_corner_radius > 0 {
            let screen_top = -(screen_height - sheet_height).max(0);
            let screen_diameter = screen_corner_radius * 2;
            let screen_region = CreateRoundRectRgn(
                0,
                screen_top,
                width + 1,
                sheet_height + 1,
                screen_diameter,
                screen_diameter,
            );
            if !screen_region.is_invalid() {
                let _ = CombineRgn(
                    Some(sheet_region),
                    Some(sheet_region),
                    Some(screen_region),
                    RGN_AND,
                );
                let _ = DeleteObject(HGDIOBJ(screen_region.0));
            }
        }

        let applied = SetWindowRgn(sheet, Some(sheet_region), true);
        if applied == 0 {
            let _ = DeleteObject(HGDIOBJ(sheet_region.0));
        }
    }
}

fn content_screen_corner_radius(content: HWND) -> i32 {
    frame_state(hwnd_handle(content), |state| {
        screen_corner_radius(&state.spec)
    })
    .unwrap_or(0)
}

fn content_rect(content: HWND) -> Option<RECT> {
    let mut rect = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetWindowRect(content, &mut rect).is_err() {
            return None;
        }
    }
    (rect.right > rect.left && rect.bottom > rect.top).then_some(rect)
}

fn sheet_client_rect(hwnd: HWND) -> RECT {
    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    client
}

fn rect_contains(rect: RECT, x: i32, y: i32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

fn rgb_to_colorref(rgb: u32) -> COLORREF {
    let r = rgb & 0xff0000;
    let g = rgb & 0x00ff00;
    let b = rgb & 0x0000ff;
    COLORREF((r >> 16) | g | (b << 16))
}

fn dismiss_info_sheet_for_window(window: HWND) {
    let handle = hwnd_handle(window);
    let Some((content, windows)) = ABOUT_SHEETS
        .get()
        .and_then(|sheets| sheets.lock().ok())
        .and_then(|mut sheets| {
            let content = sheets
                .iter()
                .find(|(_, windows)| windows.mask == handle || windows.sheet == handle)
                .map(|(content, _)| *content)?;
            sheets.remove_entry(&content)
        })
    else {
        return;
    };
    destroy_about_windows(windows);
    restore_frame_overlays(content);
}

pub(super) fn dismiss_info_sheet_for_content(content: isize) {
    let Some(windows) = ABOUT_SHEETS
        .get()
        .and_then(|sheets| sheets.lock().ok())
        .and_then(|mut sheets| sheets.remove(&content))
    else {
        return;
    };
    destroy_about_windows(windows);
    restore_frame_overlays(content);
}

/// Re-apply the current overlay palette to an already-open sheet (appearance
/// flip while the capsule menu is showing).
pub(super) fn restyle_info_sheets() {
    let Some(sheets) = ABOUT_SHEETS.get().and_then(|sheets| sheets.lock().ok()) else {
        return;
    };
    let palette = overlay_palette();
    for windows in sheets.values() {
        let mask = hwnd_from_handle(windows.mask);
        let sheet = hwnd_from_handle(windows.sheet);
        if is_window_handle_valid(windows.mask) {
            unsafe {
                let _ = WindowsAndMessaging::SetLayeredWindowAttributes(
                    mask,
                    COLORREF(0),
                    palette.mask_alpha,
                    LWA_ALPHA,
                );
            }
        }
        if is_window_handle_valid(windows.sheet) {
            unsafe {
                let _ = InvalidateRect(Some(sheet), None, true);
            }
        }
    }
}

fn destroy_about_windows(windows: AboutSheetWindows) {
    for handle in [windows.sheet, windows.mask] {
        if handle != 0 && is_window_handle_valid(handle) {
            unsafe {
                let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(handle));
            }
        }
    }
}

fn restore_frame_overlays(content: isize) {
    if content != 0 && is_window_handle_valid(content) {
        let hwnd = hwnd_from_handle(content);
        reposition_status_bar(hwnd);
        reposition_cutout(hwnd);
        reposition_corner_mask(hwnd);
        reposition_capsule(hwnd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GDI's Segoe UI with CJK font linking: ideographs about one em wide,
    /// ASCII about half.
    fn measure(text: &str, height: i32, _weight: i32) -> i32 {
        let em = height.abs();
        text.chars()
            .map(|ch| if ch.is_ascii() { em / 2 } else { em })
            .sum()
    }

    fn sheet(title: &str, badge: Option<&str>, actions: usize) -> DeviceFrameInfoSheet {
        DeviceFrameInfoSheet {
            title: title.to_string(),
            version: "0.3.2".to_string(),
            badge: badge.map(|text| InfoSheetBadge {
                text: text.to_string(),
                foreground: 0,
                background: 0,
            }),
            actions: (0..actions)
                .map(|index| SheetAction {
                    label: format!("action {index}"),
                    command: index as u32,
                    icon: WindowsDeviceFrameSheetIcon::Path(String::new()),
                })
                .collect(),
        }
    }

    #[test]
    fn sheet_geometry_follows_the_device_scale() {
        let standard = sheet_height(5, 1.0);
        let scaled = sheet_height(5, 2.25);
        assert!(
            (f64::from(scaled) - f64::from(standard) * 2.25).abs() <= 2.0,
            "{scaled} vs {standard}"
        );

        let client = RECT {
            left: 0,
            top: 0,
            right: 967,
            bottom: scaled,
        };
        let grid = button_grid_rect(client, 5, 2.25);
        assert_eq!(grid.bottom + px(BOTTOM_PADDING, 2.25), scaled);
        let first = action_button_rect(grid, 5, 0, 2.25);
        let last = action_button_rect(grid, 5, 4, 2.25);
        assert_eq!(first.left, grid.left);
        assert_eq!(last.right, grid.right);
        assert_eq!(first.bottom - first.top, px(BUTTON_ROW_HEIGHT, 2.25));
    }

    #[test]
    fn header_keeps_the_measured_cjk_title() {
        let scale = 2.25;
        let layout = header_layout(967, &sheet("门店主数据", Some("DEV"), 5), scale, measure);
        let title = layout.title.expect("title slot");
        assert_eq!(
            title.right - title.left,
            measure("门店主数据", -px(TITLE_FONT_HEIGHT, scale), 600)
        );
        assert_eq!(layout.divider.left, title.right);
        assert_eq!(layout.version.left, layout.divider.right);
        let badge = layout.badge.expect("badge slot");
        assert!(badge.left >= layout.version.right);
        assert!(badge.right <= 967 - px(HORIZONTAL_PADDING, scale));
    }

    #[test]
    fn long_title_takes_at_most_half_the_sheet() {
        let layout = header_layout(967, &sheet(&"门".repeat(60), None, 1), 2.25, measure);
        let title = layout.title.expect("title slot");
        assert_eq!(
            title.right - title.left,
            (967 - px(HORIZONTAL_PADDING, 2.25)) / 2
        );
    }
}
