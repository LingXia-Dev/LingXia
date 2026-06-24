use super::*;

use crate::{WindowsDesignIcon, draw_windows_design_icon_with_color};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateRoundRectRgn, CreateSolidBrush, DT_CENTER, DT_END_ELLIPSIS,
    DT_LEFT, DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawTextW, EndPaint, FillRect, HBRUSH,
    HGDIOBJ, PAINTSTRUCT, SelectObject, SetBkMode, SetTextColor, SetWindowRgn, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::LWA_ALPHA;

const HEADER_TOP: i32 = 16;
const HORIZONTAL_PADDING: i32 = 20;
const HEADER_HEIGHT: i32 = 24;
const HEADER_SEPARATOR_TOP_GAP: i32 = 12;
const SEPARATOR_HEIGHT: i32 = 1;
const BUTTON_ROW_TOP_GAP: i32 = 12;
const BUTTON_ROW_HEIGHT: i32 = 72;
const BOTTOM_PADDING: i32 = 16;
const BUTTON_GAP: i32 = 16;
const BUTTON_ICON_SIZE: i32 = 24;
const BUTTON_ICON_TOP: i32 = 12;
const BUTTON_LABEL_GAP: i32 = 6;
const MASK_ALPHA: u8 = 128;
const PRIMARY_TEXT_COLOR: u32 = 0x000000;
const SECONDARY_TEXT_COLOR: u32 = 0x999999;
const SEPARATOR_TEXT_COLOR: u32 = 0xCCCCCC;
const ACTION_TEXT_COLOR: u32 = 0x333333;
const SEPARATOR_COLOR: u32 = 0xEEEEEE;
const SHEET_CORNER_RADIUS: i32 = 16;

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
}

/// Caller-supplied action row: explicit icon + command, no inference.
#[derive(Debug, Clone)]
pub(in crate::device_frame) struct SheetAction {
    pub(in crate::device_frame) label: String,
    pub(in crate::device_frame) command: u32,
    pub(in crate::device_frame) icon: WindowsDesignIcon,
}

static ABOUT_SHEETS: OnceLock<Mutex<HashMap<isize, AboutSheetWindows>>> = OnceLock::new();

pub(super) fn show_info_sheet(content: HWND, info: DeviceFrameInfoSheet) {
    if !is_window_handle_valid(hwnd_handle(content)) {
        return;
    }

    dismiss_about_sheet_for_content(hwnd_handle(content));

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
                | WindowsAndMessaging::WS_EX_NOACTIVATE
                | WindowsAndMessaging::WS_EX_TOPMOST,
            about_mask_class(),
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

    let sheet_height = sheet_height().min(height.max(1));
    let sheet = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_NOACTIVATE
                | WindowsAndMessaging::WS_EX_TOPMOST,
            about_sheet_class(),
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
            MASK_ALPHA,
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
            },
        );
    }

    reposition_about_sheet(content);
}

fn about_mask_class() -> PCWSTR {
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

fn about_sheet_class() -> PCWSTR {
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
        dismiss_about_sheet_for_window(hwnd);
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
                    dismiss_about_sheet_for_window(hwnd);
                    dispatch_device_frame_command(id);
                }
                None => {}
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

fn sheet_height() -> i32 {
    HEADER_TOP
        + HEADER_HEIGHT
        + HEADER_SEPARATOR_TOP_GAP
        + SEPARATOR_HEIGHT
        + BUTTON_ROW_TOP_GAP
        + BUTTON_ROW_HEIGHT
        + BOTTOM_PADDING
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
    let row = button_row_rect(sheet_client_rect(hwnd));
    if y < row.top || y >= row.bottom {
        return None;
    }
    for (index, action) in actions.iter().enumerate() {
        if rect_contains(action_button_rect(row, actions.len(), index), x, y) {
            return Some(SheetHit::Action(action.command));
        }
    }
    None
}

fn draw_separator(dc: HDC, left: i32, top: i32, right: i32) {
    let line = RECT {
        left,
        top,
        right,
        bottom: top + 1,
    };
    unsafe {
        let brush = CreateSolidBrush(rgb_to_colorref(SEPARATOR_COLOR));
        let _ = FillRect(dc, &line, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

fn paint_capsule_menu(dc: HDC, client: &RECT, info: &DeviceFrameInfoSheet) {
    paint_header(dc, client, info);
    let separator_top = HEADER_TOP + HEADER_HEIGHT + HEADER_SEPARATOR_TOP_GAP;
    draw_separator(dc, 0, separator_top, client.right);
    paint_action_buttons(dc, client, info);
}

fn paint_header(dc: HDC, client: &RECT, info: &DeviceFrameInfoSheet) {
    let left = HORIZONTAL_PADDING;
    let top = HEADER_TOP;
    let mut cursor = left;
    let app_name = info.title.trim();
    if !app_name.is_empty() {
        let width = text_width_estimate(app_name, 16, 600).min((client.right - left) / 2);
        draw_sheet_text(
            dc,
            app_name,
            RECT {
                left: cursor,
                top,
                right: cursor + width,
                bottom: top + HEADER_HEIGHT,
            },
            -16,
            600,
            PRIMARY_TEXT_COLOR,
            DT_LEFT,
        );
        cursor += width;
    }
    draw_sheet_text(
        dc,
        " | ",
        RECT {
            left: cursor,
            top,
            right: cursor + 18,
            bottom: top + HEADER_HEIGHT,
        },
        -16,
        400,
        SEPARATOR_TEXT_COLOR,
        DT_LEFT,
    );
    cursor += 18;
    draw_sheet_text(
        dc,
        info.version.trim(),
        RECT {
            left: cursor,
            top,
            right: client.right - HORIZONTAL_PADDING - 42,
            bottom: top + HEADER_HEIGHT,
        },
        -14,
        400,
        SECONDARY_TEXT_COLOR,
        DT_LEFT,
    );
    if let Some(badge) = info.badge.as_ref() {
        let badge_rect = RECT {
            left: client.right - HORIZONTAL_PADDING - 34,
            top: top + 1,
            right: client.right - HORIZONTAL_PADDING,
            bottom: top + 17,
        };
        fill_rect(dc, badge_rect, badge.background);
        draw_sheet_text(
            dc,
            &badge.text,
            badge_rect,
            -10,
            600,
            badge.foreground,
            DT_CENTER,
        );
    }
}

fn paint_action_buttons(dc: HDC, client: &RECT, info: &DeviceFrameInfoSheet) {
    let actions = &info.actions;
    if actions.is_empty() {
        return;
    }
    let row = button_row_rect(*client);
    for (index, action) in actions.iter().enumerate() {
        let rect = action_button_rect(row, actions.len(), index);
        let icon_rect = RECT {
            left: rect.left + (rect.right - rect.left - BUTTON_ICON_SIZE) / 2,
            top: rect.top + BUTTON_ICON_TOP,
            right: rect.left + (rect.right - rect.left + BUTTON_ICON_SIZE) / 2,
            bottom: rect.top + BUTTON_ICON_TOP + BUTTON_ICON_SIZE,
        };
        let _ = draw_windows_design_icon_with_color(dc, action.icon, icon_rect, ACTION_TEXT_COLOR);
        draw_sheet_text(
            dc,
            &action.label,
            RECT {
                left: rect.left + 4,
                top: icon_rect.bottom + BUTTON_LABEL_GAP,
                right: rect.right - 4,
                bottom: rect.bottom - 12,
            },
            -13,
            400,
            ACTION_TEXT_COLOR,
            DT_CENTER,
        );
    }
}

fn button_row_rect(client: RECT) -> RECT {
    let top = HEADER_TOP
        + HEADER_HEIGHT
        + HEADER_SEPARATOR_TOP_GAP
        + SEPARATOR_HEIGHT
        + BUTTON_ROW_TOP_GAP;
    RECT {
        left: HORIZONTAL_PADDING,
        top,
        right: client.right - HORIZONTAL_PADDING,
        bottom: top + BUTTON_ROW_HEIGHT,
    }
}

fn action_button_rect(row: RECT, count: usize, index: usize) -> RECT {
    let count_i32 = count.max(1) as i32;
    let gaps = BUTTON_GAP * (count_i32 - 1).max(0);
    let width = ((row.right - row.left - gaps) / count_i32).max(1);
    let left = row.left + index as i32 * (width + BUTTON_GAP);
    RECT {
        left,
        top: row.top,
        right: if index + 1 == count {
            row.right
        } else {
            left + width
        },
        bottom: row.bottom,
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
    fill_rect(dc, client, 0xFFFFFF);

    paint_capsule_menu(dc, &client, &info);

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
    mut rect: RECT,
    height: i32,
    weight: i32,
    color: u32,
    align: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    let font = unsafe {
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
    };
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let old_font = SelectObject(dc, HGDIOBJ(font.0));
        let _ = SetBkMode(dc, TRANSPARENT);
        let _ = SetTextColor(dc, rgb_to_colorref(color));
        let _ = DrawTextW(
            dc,
            &mut wide,
            &mut rect,
            align | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        );
        if !old_font.is_invalid() {
            let _ = SelectObject(dc, old_font);
        }
        if !font.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
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

pub(super) fn reposition_about_sheet(content: HWND) {
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
    let sheet_height = sheet_height().min(height.max(1));
    unsafe {
        if is_window_handle_valid(windows.mask) {
            let _ = WindowsAndMessaging::SetWindowPos(
                hwnd_from_handle(windows.mask),
                Some(WindowsAndMessaging::HWND_TOPMOST),
                rect.left,
                rect.top,
                width,
                height,
                WindowsAndMessaging::SWP_NOACTIVATE
                    | WindowsAndMessaging::SWP_NOOWNERZORDER
                    | WindowsAndMessaging::SWP_SHOWWINDOW,
            );
        }
        if is_window_handle_valid(windows.sheet) {
            let sheet_hwnd = hwnd_from_handle(windows.sheet);
            let _ = WindowsAndMessaging::SetWindowPos(
                sheet_hwnd,
                Some(WindowsAndMessaging::HWND_TOPMOST),
                rect.left,
                rect.bottom - sheet_height,
                width,
                sheet_height,
                WindowsAndMessaging::SWP_NOACTIVATE
                    | WindowsAndMessaging::SWP_NOOWNERZORDER
                    | WindowsAndMessaging::SWP_SHOWWINDOW,
            );
            apply_sheet_region(sheet_hwnd, width, sheet_height);
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(sheet_hwnd), None, true);
        }
    }
}

fn apply_sheet_region(sheet: HWND, width: i32, height: i32) {
    if width <= 0 || height <= 0 {
        return;
    }
    let radius = SHEET_CORNER_RADIUS * 2;
    unsafe {
        // Extend the rounded region below the window so only the top corners are rounded.
        let region = CreateRoundRectRgn(
            0,
            0,
            width + 1,
            height + SHEET_CORNER_RADIUS + 1,
            radius,
            radius,
        );
        if region.is_invalid() {
            return;
        }
        let applied = SetWindowRgn(sheet, Some(region), true);
        if applied == 0 {
            let _ = DeleteObject(HGDIOBJ(region.0));
        }
    }
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

fn text_width_estimate(text: &str, font_size: i32, weight: i32) -> i32 {
    let weight_extra = if weight >= 600 { 2 } else { 0 };
    text.chars().count() as i32 * ((font_size / 2) + weight_extra) + 4
}

fn rgb_to_colorref(rgb: u32) -> COLORREF {
    let r = rgb & 0xff0000;
    let g = rgb & 0x00ff00;
    let b = rgb & 0x0000ff;
    COLORREF((r >> 16) | g | (b << 16))
}

fn dismiss_about_sheet_for_window(window: HWND) {
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

pub(super) fn dismiss_about_sheet_for_content(content: isize) {
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
        reposition_capsule(hwnd);
        reposition_cutout(hwnd);
    }
}
