//! Low-level GDI/GDI+ drawing helpers for shell chrome.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{COLORREF, RECT, SIZE};
use windows::Win32::Graphics::Gdi::{
    ANTIALIASED_QUALITY, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateRoundRectRgn,
    CreateSolidBrush, DEFAULT_CHARSET, DEFAULT_PITCH, DT_CENTER, DT_END_ELLIPSIS, DT_SINGLELINE,
    DT_VCENTER, DT_WORDBREAK, DeleteObject, DrawTextW, ExtSelectClipRgn, FF_SWISS, FONT_QUALITY,
    FillRect, GetDC, GetDeviceCaps, GetStockObject, GetTextExtentPoint32W, HDC, HFONT, HGDIOBJ,
    IntersectClipRect, LOGPIXELSY, NULL_PEN, OUT_DEFAULT_PRECIS, RGN_AND, ReleaseDC, RestoreDC,
    RoundRect, SaveDC, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::Graphics::GdiPlus;
use windows::Win32::UI::WindowsAndMessaging::{self, HICON};
use windows::core::w;

use super::icons::{cached_png_bytes_icon_handle, cached_png_icon_handle};
use super::*;

/// Process-lifetime GDI font cache; a repaint draws dozens of strings and
/// per-string HFONT create/delete is measurable churn. Cached fonts are
/// shared - callers must not `DeleteObject` them.
pub(in crate::shell) fn cached_font_with(
    face: &str,
    height: i32,
    weight: i32,
    quality: FONT_QUALITY,
    create: impl FnOnce() -> HFONT,
) -> HFONT {
    type FontKey = (String, i32, i32, u8);
    static FONTS: OnceLock<Mutex<HashMap<FontKey, isize>>> = OnceLock::new();
    let fonts = FONTS.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut fonts) = fonts.lock() else {
        return create();
    };
    let key = (face.to_string(), height, weight, quality.0);
    if let Some(&handle) = fonts.get(&key) {
        return HFONT(handle as *mut std::ffi::c_void);
    }
    let font = create();
    if !font.is_invalid() {
        fonts.insert(key, font.0 as isize);
    }
    font
}

/// Chrome text font ("Segoe UI" at the shell text size/weight) sized for
/// `hdc`'s DPI. Shared cache entry - do not delete.
pub(in crate::shell) fn chrome_text_font(hdc: HDC) -> HFONT {
    chrome_text_font_with_quality(hdc, CLEARTYPE_QUALITY)
}

pub(in crate::shell::chrome) fn measure_chrome_text_width(text: &str) -> i32 {
    static WIDTHS: OnceLock<Mutex<HashMap<String, i32>>> = OnceLock::new();
    let widths = WIDTHS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(widths) = widths.lock()
        && let Some(width) = widths.get(text)
    {
        return *width;
    }

    let wide = text.encode_utf16().collect::<Vec<_>>();
    let width = unsafe {
        let hdc = GetDC(None);
        if hdc.is_invalid() {
            0
        } else {
            let font = chrome_text_font(hdc);
            let old = SelectObject(hdc, HGDIOBJ(font.0));
            let mut size = SIZE::default();
            let _ = GetTextExtentPoint32W(hdc, &wide, &mut size);
            if !old.is_invalid() {
                let _ = SelectObject(hdc, old);
            }
            let _ = ReleaseDC(None, hdc);
            size.cx
        }
    };
    if let Ok(mut widths) = widths.lock() {
        widths.insert(text.to_string(), width);
    }
    width
}

fn chrome_text_font_with_quality(hdc: HDC, quality: FONT_QUALITY) -> HFONT {
    let height = logical_font_height(hdc, SHELL_TEXT_POINT_SIZE);
    cached_font_with("Segoe UI", height, SHELL_TEXT_WEIGHT, quality, || unsafe {
        CreateFontW(
            -height,
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
            quality,
            DEFAULT_PITCH.0 as u32 | FF_SWISS.0 as u32,
            w!("Segoe UI"),
        )
    })
}

pub(in crate::shell) fn draw_text(
    hdc: HDC,
    text: &str,
    rect: RECT,
    rgb: u32,
    horizontal: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    draw_text_with_quality(hdc, text, rect, rgb, horizontal, CLEARTYPE_QUALITY)
}

pub(in crate::shell) fn draw_text_antialiased(
    hdc: HDC,
    text: &str,
    rect: RECT,
    rgb: u32,
    horizontal: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    draw_text_with_quality(hdc, text, rect, rgb, horizontal, ANTIALIASED_QUALITY)
}

pub(in crate::shell) fn draw_text_multiline_antialiased(
    hdc: HDC,
    text: &str,
    rect: RECT,
    rgb: u32,
) {
    if text.is_empty() || rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }
    let mut wide = text.encode_utf16().collect::<Vec<_>>();
    let mut rect = rect;
    let font = chrome_text_font_with_quality(hdc, ANTIALIASED_QUALITY);
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
            DT_LEFT | DT_WORDBREAK | DT_END_ELLIPSIS,
        );
        if !old_font.is_invalid() {
            let _ = SelectObject(hdc, old_font);
        }
    }
}

fn draw_text_with_quality(
    hdc: HDC,
    text: &str,
    rect: RECT,
    rgb: u32,
    horizontal: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
    quality: FONT_QUALITY,
) {
    if text.is_empty() || rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }

    let mut wide: Vec<u16> = text.encode_utf16().collect();
    let mut rect = rect;
    let font = chrome_text_font_with_quality(hdc, quality);
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
    fill_round_rect_path(hdc, rect, radius, 0xff00_0000 | rgb);
}

/// Fills the horizontal band `band_top..band_bottom` of `rect`'s rounded
/// shape, anti-aliased. The band clip has straight edges only, so splitting a
/// card into differently colored strips (e.g. header over body) keeps every
/// arc pixel blended exactly once in its final color — no fringe from stacking
/// a second rounded fill on the same arc.
pub(in crate::shell) fn fill_round_rect_aa_band(
    hdc: HDC,
    rect: RECT,
    radius: i32,
    rgb: u32,
    band_top: i32,
    band_bottom: i32,
) {
    let band_top = band_top.max(rect.top);
    let band_bottom = band_bottom.min(rect.bottom);
    if band_bottom <= band_top {
        return;
    }
    unsafe {
        let saved = SaveDC(hdc);
        let _ = IntersectClipRect(hdc, rect.left, band_top, rect.right, band_bottom);
        fill_round_rect_aa(hdc, rect, radius, rgb);
        let _ = RestoreDC(hdc, saved);
    }
}

/// Anti-aliased fill with independent corner radii `[tl, tr, br, bl]` (0 =
/// square), for chrome bands that own only some corners of the workspace
/// silhouette. Plain fill when every corner is square or GDI+ is unavailable.
pub(in crate::shell) fn fill_round_rect_aa_corners(
    hdc: HDC,
    rect: RECT,
    radii: [i32; 4],
    rgb: u32,
) {
    let width = rect_width(&rect);
    let height = rect_height(&rect);
    if width == 0 || height == 0 {
        return;
    }
    let max_radius = (width / 2).min(height / 2);
    let [tl, tr, br, bl] = radii.map(|radius| radius.clamp(0, max_radius));
    if [tl, tr, br, bl] == [0; 4] || !ensure_gdiplus_started() {
        fill_rect(hdc, rect, rgb);
        return;
    }
    unsafe {
        let mut graphics: *mut GdiPlus::GpGraphics = std::ptr::null_mut();
        if GdiPlus::GdipCreateFromHDC(hdc, &mut graphics) != GdiPlus::Ok || graphics.is_null() {
            fill_rect(hdc, rect, rgb);
            return;
        }
        let _ = GdiPlus::GdipSetSmoothingMode(graphics, GdiPlus::SmoothingModeAntiAlias);
        let mut path: *mut GdiPlus::GpPath = std::ptr::null_mut();
        if GdiPlus::GdipCreatePath(GdiPlus::FillModeAlternate, &mut path) == GdiPlus::Ok
            && !path.is_null()
        {
            let (left, top) = (rect.left as f32, rect.top as f32);
            let (right, bottom) = (rect.right as f32, rect.bottom as f32);
            let [tl, tr, br, bl] = [tl as f32, tr as f32, br as f32, bl as f32];
            // Edge lines with quarter arcs at each rounded corner; GDI+
            // connects consecutive figure segments, so square corners fall
            // out of adjacent edges meeting.
            let _ = GdiPlus::GdipAddPathLine(path, left + tl, top, right - tr, top);
            if tr > 0.0 {
                let _ = GdiPlus::GdipAddPathArc(
                    path,
                    right - tr * 2.0,
                    top,
                    tr * 2.0,
                    tr * 2.0,
                    270.0,
                    90.0,
                );
            }
            let _ = GdiPlus::GdipAddPathLine(path, right, top + tr, right, bottom - br);
            if br > 0.0 {
                let _ = GdiPlus::GdipAddPathArc(
                    path,
                    right - br * 2.0,
                    bottom - br * 2.0,
                    br * 2.0,
                    br * 2.0,
                    0.0,
                    90.0,
                );
            }
            let _ = GdiPlus::GdipAddPathLine(path, right - br, bottom, left + bl, bottom);
            if bl > 0.0 {
                let _ = GdiPlus::GdipAddPathArc(
                    path,
                    left,
                    bottom - bl * 2.0,
                    bl * 2.0,
                    bl * 2.0,
                    90.0,
                    90.0,
                );
            }
            let _ = GdiPlus::GdipAddPathLine(path, left, bottom - bl, left, top + tl);
            if tl > 0.0 {
                let _ = GdiPlus::GdipAddPathArc(path, left, top, tl * 2.0, tl * 2.0, 180.0, 90.0);
            }
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

/// Intersects the DC's clip with a rounded rect one pixel inside `rect`.
/// Pairs with an outer [`fill_round_rect_aa`] of the same rect: the AA fill
/// provides the card's smooth edge, and plain square fills painted inside the
/// clip get their corners shaped by it without a second anti-aliased arc (a
/// re-blend over the card's arc leaves a fringe). Bracket with
/// `SaveDC`/`RestoreDC`.
pub(in crate::shell) fn clip_to_round_rect_inside(hdc: HDC, rect: RECT, radius: i32) {
    let radius = radius.clamp(1, (rect_width(&rect) / 2).min(rect_height(&rect) / 2));
    unsafe {
        let region = CreateRoundRectRgn(
            rect.left + 1,
            rect.top + 1,
            rect.right - 1,
            rect.bottom - 1,
            (radius - 1).max(1) * 2,
            (radius - 1).max(1) * 2,
        );
        if region.is_invalid() {
            return;
        }
        let _ = ExtSelectClipRgn(hdc, Some(region), RGN_AND);
        let _ = DeleteObject(HGDIOBJ(region.0));
    }
}

/// Translucent rounded wash (`0xAARRGGBB`) over whatever is already painted;
/// decorative, so it is skipped when GDI+ is unavailable.
pub(in crate::shell) fn fill_round_rect_overlay(hdc: HDC, rect: RECT, radius: i32, argb: u32) {
    let width = rect_width(&rect);
    let height = rect_height(&rect);
    if width == 0 || height == 0 || !ensure_gdiplus_started() {
        return;
    }
    let radius = radius.clamp(0, (width / 2).min(height / 2));
    fill_round_rect_path(hdc, rect, radius.max(1), argb);
}

/// Hover wash over `rect` when the chrome cursor is inside it; call before
/// the element's icon/text.
pub(in crate::shell) fn draw_hover_wash(
    hdc: HDC,
    rect: RECT,
    radius: i32,
    cursor: Option<(i32, i32)>,
) {
    if cursor.is_some_and(|point| rect_contains(&rect, point)) {
        fill_round_rect_overlay(hdc, rect, radius, hover_overlay());
    }
}

fn fill_round_rect_path(hdc: HDC, rect: RECT, radius: i32, argb: u32) {
    round_rect_path_op(hdc, rect, radius, |graphics, path| unsafe {
        let mut brush: *mut GdiPlus::GpSolidFill = std::ptr::null_mut();
        if GdiPlus::GdipCreateSolidFill(argb, &mut brush) == GdiPlus::Ok && !brush.is_null() {
            let _ = GdiPlus::GdipFillPath(graphics, brush.cast(), path);
            let _ = GdiPlus::GdipDeleteBrush(brush.cast());
        }
    });
}

/// Strokes a 1px anti-aliased rounded outline just inside `rect` (a hairline
/// card border). No-op when GDI+ is unavailable.
pub(in crate::shell) fn stroke_round_rect_aa(hdc: HDC, rect: RECT, radius: i32, rgb: u32) {
    if rect_width(&rect) == 0 || rect_height(&rect) == 0 || !ensure_gdiplus_started() {
        return;
    }
    // Inset by half the pen width so the stroke hugs the edge without
    // spilling outside the (alpha-masked) bounds.
    let inset = normalize_rect(RECT {
        left: rect.left,
        top: rect.top,
        right: rect.right - 1,
        bottom: rect.bottom - 1,
    });
    round_rect_path_op(hdc, inset, radius, |graphics, path| unsafe {
        let mut pen: *mut GdiPlus::GpPen = std::ptr::null_mut();
        if GdiPlus::GdipCreatePen1(0xff00_0000 | rgb, 1.0, GdiPlus::UnitPixel, &mut pen)
            == GdiPlus::Ok
            && !pen.is_null()
        {
            let _ = GdiPlus::GdipDrawPath(graphics, pen, path);
            let _ = GdiPlus::GdipDeletePen(pen);
        }
    });
}

fn round_rect_path_op(
    hdc: HDC,
    rect: RECT,
    radius: i32,
    op: impl FnOnce(*mut GdiPlus::GpGraphics, *mut GdiPlus::GpPath),
) {
    unsafe {
        let mut graphics: *mut GdiPlus::GpGraphics = std::ptr::null_mut();
        if GdiPlus::GdipCreateFromHDC(hdc, &mut graphics) != GdiPlus::Ok || graphics.is_null() {
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
            op(graphics, path);
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
/// sidebar icon - set once the shell knows its asset dir.
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

/// Draws the LingXia icon into `rect` - the default icon for sidebar entries
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
