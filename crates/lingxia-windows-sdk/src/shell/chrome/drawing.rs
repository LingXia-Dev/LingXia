//! Low-level GDI/GDI+ drawing helpers for shell chrome.

use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateSolidBrush, DEFAULT_CHARSET,
    DEFAULT_PITCH, DT_CENTER, DT_END_ELLIPSIS, DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawTextW,
    FF_SWISS, FillRect, GetDeviceCaps, GetStockObject, HDC, HFONT, HGDIOBJ, LOGPIXELSY, NULL_PEN,
    OUT_DEFAULT_PRECIS, RoundRect, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::Graphics::GdiPlus;
use windows::Win32::UI::WindowsAndMessaging::{self, HICON};
use windows::core::w;

use super::icons::{cached_png_bytes_icon_handle, cached_png_icon_handle};
use super::*;

/// Chrome text font ("Segoe UI" at the shell text size/weight) sized for
/// `hdc`'s DPI. The caller owns the returned font and deletes it after use.
pub(in crate::shell) fn create_chrome_text_font(hdc: HDC) -> HFONT {
    unsafe {
        CreateFontW(
            -logical_font_height(hdc, SHELL_TEXT_POINT_SIZE),
            0,
            0,
            0,
            SHELL_TEXT_WEIGHT,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32 | FF_SWISS.0 as u32,
            w!("Segoe UI"),
        )
    }
}

pub(in crate::shell) fn draw_text(
    hdc: HDC,
    text: &str,
    rect: RECT,
    rgb: u32,
    horizontal: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    if text.is_empty() || rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }

    let mut wide: Vec<u16> = text.encode_utf16().collect();
    let mut rect = rect;
    let font = create_chrome_text_font(hdc);
    unsafe {
        let old_font = if font.is_invalid() {
            HGDIOBJ::default()
        } else {
            SelectObject(hdc, HGDIOBJ(font.0))
        };
        let _ = SetBkMode(hdc, TRANSPARENT);
        let _ = SetTextColor(hdc, rgb_to_colorref(rgb));
        let _ = DrawTextW(
            hdc,
            &mut wide,
            &mut rect,
            horizontal | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        );
        if !old_font.is_invalid() {
            let _ = SelectObject(hdc, old_font);
        }
        if !font.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
    }
}

pub(in crate::shell) fn logical_font_height(hdc: HDC, point_size: i32) -> i32 {
    let dpi_y = unsafe { GetDeviceCaps(Some(hdc), LOGPIXELSY) };
    let dpi_y = if dpi_y > 0 { dpi_y } else { 96 };
    (point_size * dpi_y + 36) / 72
}

pub(in crate::shell) fn draw_badge(hdc: HDC, item_rect: RECT, badge: &str) {
    let badge_rect = RECT {
        left: item_rect.right - 30,
        top: item_rect.top + 7,
        right: item_rect.right - 8,
        bottom: item_rect.top + 25,
    };
    fill_rect(hdc, badge_rect, SHELL_BADGE_RED);
    draw_text(hdc, badge, badge_rect, 0xffffff, DT_CENTER);
}

pub(in crate::shell) fn draw_red_dot(hdc: HDC, item_rect: RECT) {
    let dot_rect = RECT {
        left: item_rect.right - 18,
        top: item_rect.top + 9,
        right: item_rect.right - 10,
        bottom: item_rect.top + 17,
    };
    fill_rect(hdc, dot_rect, SHELL_BADGE_RED);
}

pub(in crate::shell) fn draw_top_border(hdc: HDC, rect: RECT, rgb: u32) {
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.top + 1,
        },
        rgb,
    );
}

pub(in crate::shell) fn draw_bottom_border(hdc: HDC, rect: RECT, rgb: u32) {
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: rect.bottom - 1,
            right: rect.right,
            bottom: rect.bottom,
        },
        rgb,
    );
}

pub(in crate::shell) fn draw_left_border(hdc: HDC, rect: RECT, rgb: u32) {
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: rect.top,
            right: rect.left + 1,
            bottom: rect.bottom,
        },
        rgb,
    );
}

pub(in crate::shell) fn draw_right_border(hdc: HDC, rect: RECT, rgb: u32) {
    fill_rect(
        hdc,
        RECT {
            left: rect.right - 1,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        },
        rgb,
    );
}

pub(in crate::shell) fn fill_rect(hdc: HDC, rect: RECT, rgb: u32) {
    if rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }
    unsafe {
        let brush = CreateSolidBrush(rgb_to_colorref(rgb));
        if brush.is_invalid() {
            return;
        }
        let _ = FillRect(hdc, &rect, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

/// Starts GDI+ once for the process (the shell paints chrome until exit,
/// so the library is never shut down). Returns `false` when startup failed;
/// rounded fills then fall back to aliased GDI `RoundRect`.
fn ensure_gdiplus_started() -> bool {
    static STARTED: OnceLock<bool> = OnceLock::new();
    *STARTED.get_or_init(|| {
        let input = GdiPlus::GdiplusStartupInput {
            GdiplusVersion: 1,
            ..Default::default()
        };
        let mut token = 0usize;
        let status = unsafe { GdiPlus::GdiplusStartup(&mut token, &input, std::ptr::null_mut()) };
        if status != GdiPlus::Ok {
            log::warn!(
                "GdiplusStartup failed ({}); rounded chrome falls back to aliased GDI",
                status.0
            );
        }
        status == GdiPlus::Ok
    })
}

/// Fills a rounded rectangle with an anti-aliased GDI+ path. `radius` is
/// the true corner radius (matching the corner-cap overlays, not GDI
/// `RoundRect`'s ellipse-diameter semantics), clamped to the rect. Used for
/// every rounded shape the chrome paints over a contrasting background;
/// plain GDI fills alias the arc into a hard staircase.
pub(in crate::shell) fn fill_round_rect_aa(hdc: HDC, rect: RECT, radius: i32, rgb: u32) {
    let width = rect_width(&rect);
    let height = rect_height(&rect);
    if width == 0 || height == 0 {
        return;
    }
    let radius = radius.clamp(0, (width / 2).min(height / 2));
    if radius == 0 {
        fill_rect(hdc, rect, rgb);
        return;
    }
    if !ensure_gdiplus_started() {
        fill_round_rect_gdi(hdc, rect, rgb, radius * 2);
        return;
    }
    unsafe {
        let mut graphics: *mut GdiPlus::GpGraphics = std::ptr::null_mut();
        if GdiPlus::GdipCreateFromHDC(hdc, &mut graphics) != GdiPlus::Ok || graphics.is_null() {
            fill_round_rect_gdi(hdc, rect, rgb, radius * 2);
            return;
        }
        let _ = GdiPlus::GdipSetSmoothingMode(graphics, GdiPlus::SmoothingModeAntiAlias);
        let mut path: *mut GdiPlus::GpPath = std::ptr::null_mut();
        if GdiPlus::GdipCreatePath(GdiPlus::FillModeAlternate, &mut path) == GdiPlus::Ok
            && !path.is_null()
        {
            let (left, top) = (rect.left as f32, rect.top as f32);
            let (right, bottom) = (rect.right as f32, rect.bottom as f32);
            let diameter = (radius * 2) as f32;
            // Quarter arcs at the four corners; GDI+ connects consecutive
            // figure segments (and the close) with straight edges.
            let _ = GdiPlus::GdipAddPathArc(path, left, top, diameter, diameter, 180.0, 90.0);
            let _ = GdiPlus::GdipAddPathArc(
                path,
                right - diameter,
                top,
                diameter,
                diameter,
                270.0,
                90.0,
            );
            let _ = GdiPlus::GdipAddPathArc(
                path,
                right - diameter,
                bottom - diameter,
                diameter,
                diameter,
                0.0,
                90.0,
            );
            let _ = GdiPlus::GdipAddPathArc(
                path,
                left,
                bottom - diameter,
                diameter,
                diameter,
                90.0,
                90.0,
            );
            let _ = GdiPlus::GdipClosePathFigure(path);
            let mut brush: *mut GdiPlus::GpSolidFill = std::ptr::null_mut();
            if GdiPlus::GdipCreateSolidFill(0xff00_0000 | rgb, &mut brush) == GdiPlus::Ok
                && !brush.is_null()
            {
                let _ = GdiPlus::GdipFillPath(graphics, brush.cast(), path);
                let _ = GdiPlus::GdipDeleteBrush(brush.cast());
            }
            let _ = GdiPlus::GdipDeletePath(path);
        }
        let _ = GdiPlus::GdipDeleteGraphics(graphics);
    }
}

/// Aliased GDI rounded fill, kept only as the fallback when GDI+ is
/// unavailable. `corner_diameter` follows `RoundRect`'s ellipse semantics
/// (twice the corner radius).
fn fill_round_rect_gdi(hdc: HDC, rect: RECT, rgb: u32, corner_diameter: i32) {
    if rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }
    unsafe {
        let brush = CreateSolidBrush(rgb_to_colorref(rgb));
        if brush.is_invalid() {
            return;
        }
        let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
        let pen = GetStockObject(NULL_PEN);
        let old_pen = SelectObject(hdc, pen);
        let _ = RoundRect(
            hdc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            corner_diameter,
            corner_diameter,
        );
        if !old_pen.is_invalid() {
            let _ = SelectObject(hdc, old_pen);
        }
        if !old_brush.is_invalid() {
            let _ = SelectObject(hdc, old_brush);
        }
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

pub(in crate::shell) fn centered_icon_rect(rect: RECT, size: i32) -> RECT {
    let left = rect.left + (rect_width(&rect) - size).max(0) / 2;
    let top = rect.top + (rect_height(&rect) - size).max(0) / 2;
    normalize_rect(RECT {
        left,
        top,
        right: left + size,
        bottom: top + size,
    })
}

pub(in crate::shell) fn draw_icon_from_path(hdc: HDC, path: &str, rect: RECT, size: u32) -> bool {
    let Some(handle) = cached_png_icon_handle(path, size) else {
        return false;
    };
    draw_icon_handle(hdc, handle, rect)
}

/// Absolute path to the LingXia icon, copied next to the app by the CLI
/// (`<asset_dir>/icons/lingxia.png`) and loaded from disk like every other
/// sidebar icon — set once the shell knows its asset dir.
static DEFAULT_ICON_PATH: OnceLock<Mutex<Option<String>>> = OnceLock::new();

/// Records the resolved path of the LingXia icon (from the runtime, which
/// knows the app's asset dir).
pub(in crate::shell) fn set_default_icon_path(path: String) {
    let slot = DEFAULT_ICON_PATH.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(path);
    }
}

fn default_icon_path() -> Option<String> {
    DEFAULT_ICON_PATH
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone())
}

/// Draws the LingXia icon into `rect` — the default icon for sidebar entries
/// with no icon of their own (lxapp items / browser tabs that report none,
/// built-in/internal pages). Loaded from the CLI-copied asset path; returns
/// `false` when no asset dir is known yet or the file is missing.
pub(in crate::shell) fn draw_default_app_icon(hdc: HDC, rect: RECT) -> bool {
    let Some(path) = default_icon_path() else {
        return false;
    };
    draw_icon_from_path(hdc, &path, rect, rect_width(&rect).max(1) as u32)
}

/// Draws `path`'s icon (PNG or SVG), falling back to the default LingXia mark
/// when the path is empty or fails to load.
pub(in crate::shell) fn draw_icon_or_default(hdc: HDC, path: &str, rect: RECT, size: u32) -> bool {
    if !path.trim().is_empty() && draw_icon_from_path(hdc, path, rect, size) {
        return true;
    }
    draw_default_app_icon(hdc, rect)
}

/// Draws a PNG supplied as in-memory bytes (e.g. a tab favicon) into
/// `rect`, decoding through the id-keyed icon cache in `lingxia-webview`.
/// Returns `false` when the bytes cannot be decoded.
pub(in crate::shell) fn draw_icon_from_png_bytes(
    hdc: HDC,
    cache_key: &str,
    png: &[u8],
    rect: RECT,
) -> bool {
    let Some(handle) =
        cached_png_bytes_icon_handle(cache_key, png, rect_width(&rect).max(1) as u32)
    else {
        return false;
    };
    draw_icon_handle(hdc, handle, rect)
}

fn draw_icon_handle(hdc: HDC, handle: isize, rect: RECT) -> bool {
    unsafe {
        WindowsAndMessaging::DrawIconEx(
            hdc,
            rect.left,
            rect.top,
            HICON(handle as *mut c_void),
            rect_width(&rect),
            rect_height(&rect),
            0,
            None,
            WindowsAndMessaging::DI_NORMAL,
        )
        .is_ok()
    }
}

pub(in crate::shell) fn rgb_to_colorref(rgb: u32) -> COLORREF {
    let r = (rgb >> 16) & 0xff;
    let g = (rgb >> 8) & 0xff;
    let b = rgb & 0xff;
    COLORREF(r | (g << 8) | (b << 16))
}
