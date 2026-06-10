//! Terminal panel grid: the snapshot store shared with the product facade
//! and the GDI cell-grid painter used by the shell chrome.
//!
//! The facade's poll thread pushes full [`TerminalSnapshot`]s through
//! [`set_panel_snapshot`] and reads [`desired_grid_size`] to keep the PTY
//! grid in sync with the panel rect; the chrome painter consumes the latest
//! snapshot on repaint and records the grid geometry it painted into, so
//! both sides agree on cell metrics. Styling mirrors the macOS terminal
//! surface (`SurfaceCore.swift`): dark `#282C34` background, white default
//! foreground, mono font, dim text blended at 58%.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

use lingxia_terminal::{TerminalCell, TerminalSnapshot};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, DEFAULT_CHARSET, DeleteObject,
    ETO_OPTIONS, ExtTextOutW, FF_MODERN, FIXED_PITCH, GetTextFaceW, GetTextMetricsW, HDC, HFONT,
    HGDIOBJ, IntersectClipRect, OUT_DEFAULT_PRECIS, RestoreDC, SaveDC, SelectObject, SetBkMode,
    SetTextColor, TEXTMETRICW, TRANSPARENT,
};
use windows::core::PCWSTR;

use super::chrome::{
    fill_rect, fill_round_rect, inset_rect, logical_font_height, rect_height, rect_width,
    rgb_to_colorref,
};

/// Inner padding between the terminal card edge and the cell grid.
const GRID_PADDING: i32 = 8;

/// Terminal text size; 10pt GDI tracks the macOS surface's 13pt Menlo look.
const GRID_FONT_POINT_SIZE: i32 = 10;

/// Fallback surface colors mirroring the macOS terminal surface
/// (`lxTerminalBackground` #282C34, `lxTerminalForeground` white).
const GRID_DEFAULT_BACKGROUND: u32 = 0x282c34;

const GRID_DEFAULT_FOREGROUND: u32 = 0xffffff;

/// Dim cells blend the foreground this far toward the background (the
/// macOS surface draws dim text at 0.58 alpha).
const GRID_DIM_FOREGROUND_PERCENT: u32 = 58;

/// Corner radius of the terminal card (same as the body-text fallback).
const GRID_CARD_RADIUS: i32 = 8;

/// Minimum grid reported to the PTY, mirroring the macOS surface clamp.
const GRID_MIN_COLS: i32 = 20;

const GRID_MIN_ROWS: i32 = 4;

/// Cell metrics and grid area recorded at the last paint of a panel.
#[derive(Clone, Copy)]
struct GridGeometry {
    cell_width: i32,
    line_height: i32,
    grid_width: i32,
    grid_height: i32,
}

#[derive(Default)]
struct PanelGridState {
    snapshot: Option<TerminalSnapshot>,
    geometry: Option<GridGeometry>,
}

static PANEL_GRIDS: OnceLock<Mutex<HashMap<String, PanelGridState>>> = OnceLock::new();

fn panel_grids() -> MutexGuard<'static, HashMap<String, PanelGridState>> {
    PANEL_GRIDS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        // The store has no invariants that poisoning can break.
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Stores the latest snapshot for `panel_id`; the chrome painter renders it
/// on the next repaint of the host window.
pub fn set_panel_snapshot(panel_id: &str, snapshot: TerminalSnapshot) {
    panel_grids()
        .entry(panel_id.to_string())
        .or_default()
        .snapshot = Some(snapshot);
}

/// Drops all stored state for `panel_id` (snapshot and paint geometry).
pub fn clear_panel(panel_id: &str) {
    panel_grids().remove(panel_id);
}

/// Grid size `(cols, rows)` that fits the panel rect seen at the last
/// paint, or `None` before the panel was first painted.
pub fn desired_grid_size(panel_id: &str) -> Option<(u16, u16)> {
    let grids = panel_grids();
    let geometry = grids.get(panel_id)?.geometry?;
    if geometry.cell_width <= 0 || geometry.line_height <= 0 {
        return None;
    }
    let cols = (geometry.grid_width / geometry.cell_width).max(GRID_MIN_COLS);
    let rows = (geometry.grid_height / geometry.line_height).max(GRID_MIN_ROWS);
    Some((
        cols.min(u16::MAX as i32) as u16,
        rows.min(u16::MAX as i32) as u16,
    ))
}

/// Paints the terminal cell grid for `panel_id` into `rect` (the card area
/// below the panel title row). Returns `false` when no snapshot is stored
/// yet so the caller can fall back to body-text rendering; grid geometry is
/// recorded either way so [`desired_grid_size`] tracks the panel rect from
/// the first paint on.
pub(super) fn draw_panel_grid(hdc: HDC, panel_id: &str, rect: RECT) -> bool {
    let grid_rect = inset_rect(rect, GRID_PADDING, GRID_PADDING);
    if rect_width(&grid_rect) == 0 || rect_height(&grid_rect) == 0 {
        return false;
    }

    let mut fonts = GridFonts::new(hdc);
    // SaveDC/RestoreDC bracket all font/clip/color changes; the fonts are
    // deleted (on drop) only after the DC stopped referencing them.
    let saved = unsafe { SaveDC(hdc) };
    let drew = draw_panel_grid_clipped(hdc, panel_id, rect, grid_rect, &mut fonts);
    unsafe {
        let _ = RestoreDC(hdc, saved);
    }
    drew
}

fn draw_panel_grid_clipped(
    hdc: HDC,
    panel_id: &str,
    rect: RECT,
    grid_rect: RECT,
    fonts: &mut GridFonts,
) -> bool {
    if !fonts.select(hdc, false, false, false) {
        return false;
    }
    let mut text_metrics = TEXTMETRICW::default();
    if unsafe { !GetTextMetricsW(hdc, &mut text_metrics).as_bool() } {
        return false;
    }
    let cell_width = text_metrics.tmAveCharWidth.max(1);
    let line_height = (text_metrics.tmHeight + text_metrics.tmExternalLeading).max(1);

    let mut grids = panel_grids();
    let state = grids.entry(panel_id.to_string()).or_default();
    state.geometry = Some(GridGeometry {
        cell_width,
        line_height,
        grid_width: rect_width(&grid_rect),
        grid_height: rect_height(&grid_rect),
    });
    let Some(snapshot) = &state.snapshot else {
        return false;
    };

    let background = snapshot
        .default_background
        .as_deref()
        .and_then(parse_hex_color)
        .unwrap_or(GRID_DEFAULT_BACKGROUND);
    let foreground = snapshot
        .default_foreground
        .as_deref()
        .and_then(parse_hex_color)
        .unwrap_or(GRID_DEFAULT_FOREGROUND);
    fill_round_rect(hdc, rect, background, GRID_CARD_RADIUS);

    unsafe {
        let _ = IntersectClipRect(
            hdc,
            grid_rect.left,
            grid_rect.top,
            grid_rect.right,
            grid_rect.bottom,
        );
        let _ = SetBkMode(hdc, TRANSPARENT);
    }

    // All cell backgrounds first: a later cell's background must not cover
    // the right half of a wide glyph drawn by the previous cell.
    for cell in &snapshot.cells {
        let token = if cell.inverse {
            cell.fg.as_deref()
        } else {
            cell.bg.as_deref()
        };
        let Some(cell_background) = token.and_then(parse_hex_color) else {
            continue;
        };
        let left = grid_rect.left + i32::from(cell.col) * cell_width;
        let top = grid_rect.top + i32::from(cell.row) * line_height;
        if left >= grid_rect.right || top >= grid_rect.bottom {
            continue;
        }
        let span = if cell.wide { 2 } else { 1 };
        fill_rect(
            hdc,
            RECT {
                left,
                top,
                right: left + span * cell_width,
                bottom: top + line_height,
            },
            cell_background,
        );
    }

    draw_cell_runs(
        hdc,
        snapshot,
        grid_rect,
        cell_width,
        line_height,
        background,
        foreground,
        fonts,
    );

    if snapshot.exited {
        draw_exited_overlay(
            hdc,
            grid_rect,
            line_height,
            snapshot.cursor_row,
            background,
            foreground,
            fonts,
        );
    } else if snapshot.cursor_visible {
        draw_cursor(
            hdc,
            snapshot,
            grid_rect,
            cell_width,
            line_height,
            background,
            foreground,
            fonts,
        );
    }
    true
}

/// Per-run text style after color resolution (inverse swap + dim blend).
#[derive(Clone, Copy, PartialEq, Eq)]
struct RunStyle {
    color: u32,
    bold: bool,
    italic: bool,
    underline: bool,
}

fn cell_style(cell: &TerminalCell, background: u32, foreground: u32) -> RunStyle {
    let token = if cell.inverse {
        cell.bg.as_deref()
    } else {
        cell.fg.as_deref()
    };
    let mut color = token.and_then(parse_hex_color).unwrap_or(foreground);
    if cell.dim {
        color = blend_rgb(color, background, GRID_DIM_FOREGROUND_PERCENT);
    }
    RunStyle {
        color,
        bold: cell.bold,
        italic: cell.italic,
        underline: cell.underline,
    }
}

/// One horizontal run of equally styled cells, flushed as a single
/// `ExtTextOutW` call with per-cell advances so the grid stays column
/// aligned regardless of actual glyph widths.
struct GridRun {
    text: Vec<u16>,
    dx: Vec<i32>,
    row: u16,
    start_col: u16,
    next_col: u16,
    style: RunStyle,
}

#[allow(clippy::too_many_arguments)]
fn draw_cell_runs(
    hdc: HDC,
    snapshot: &TerminalSnapshot,
    grid_rect: RECT,
    cell_width: i32,
    line_height: i32,
    background: u32,
    foreground: u32,
    fonts: &mut GridFonts,
) {
    let mut run: Option<GridRun> = None;
    // Snapshot cells arrive in row-major order.
    for cell in &snapshot.cells {
        if cell.text.is_empty() {
            continue;
        }
        let style = cell_style(cell, background, foreground);
        let continues = run
            .as_ref()
            .is_some_and(|run| run.row == cell.row && run.next_col == cell.col && run.style == style);
        if !continues {
            flush_run(hdc, grid_rect, cell_width, line_height, fonts, &mut run);
            run = Some(GridRun {
                text: Vec::new(),
                dx: Vec::new(),
                row: cell.row,
                start_col: cell.col,
                next_col: cell.col,
                style,
            });
        }
        let Some(run) = run.as_mut() else {
            continue;
        };
        let span: u16 = if cell.wide { 2 } else { 1 };
        let advance = cell_width * i32::from(span);
        for (index, unit) in cell.text.encode_utf16().enumerate() {
            run.text.push(unit);
            // lpDx is per UTF-16 unit; trailing units (surrogate halves,
            // combining marks) advance 0 so they stack on the base cell.
            run.dx.push(if index == 0 { advance } else { 0 });
        }
        run.next_col = cell.col.saturating_add(span);
    }
    flush_run(hdc, grid_rect, cell_width, line_height, fonts, &mut run);
}

fn flush_run(
    hdc: HDC,
    grid_rect: RECT,
    cell_width: i32,
    line_height: i32,
    fonts: &mut GridFonts,
    run: &mut Option<GridRun>,
) {
    let Some(run) = run.take() else {
        return;
    };
    if run.text.is_empty()
        || !fonts.select(hdc, run.style.bold, run.style.italic, run.style.underline)
    {
        return;
    }
    let x = grid_rect.left + i32::from(run.start_col) * cell_width;
    let y = grid_rect.top + i32::from(run.row) * line_height;
    unsafe {
        let _ = SetTextColor(hdc, rgb_to_colorref(run.style.color));
        let _ = ExtTextOutW(
            hdc,
            x,
            y,
            ETO_OPTIONS(0),
            None,
            PCWSTR(run.text.as_ptr()),
            run.text.len() as u32,
            Some(run.dx.as_ptr()),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_cursor(
    hdc: HDC,
    snapshot: &TerminalSnapshot,
    grid_rect: RECT,
    cell_width: i32,
    line_height: i32,
    background: u32,
    foreground: u32,
    fonts: &mut GridFonts,
) {
    let left = grid_rect.left + i32::from(snapshot.cursor_col) * cell_width;
    let top = grid_rect.top + i32::from(snapshot.cursor_row) * line_height;
    if left >= grid_rect.right || top >= grid_rect.bottom {
        return;
    }
    let cell_rect = RECT {
        left,
        top,
        right: left + cell_width,
        bottom: top + line_height,
    };
    match snapshot.cursor_style {
        "bar" => fill_rect(
            hdc,
            RECT {
                right: left + 2,
                ..cell_rect
            },
            foreground,
        ),
        "underline" => fill_rect(
            hdc,
            RECT {
                top: cell_rect.bottom - 2,
                ..cell_rect
            },
            foreground,
        ),
        "hollow" => {
            for edge in [
                RECT {
                    bottom: top + 1,
                    ..cell_rect
                },
                RECT {
                    top: cell_rect.bottom - 1,
                    ..cell_rect
                },
                RECT {
                    right: left + 1,
                    ..cell_rect
                },
                RECT {
                    left: cell_rect.right - 1,
                    ..cell_rect
                },
            ] {
                fill_rect(hdc, edge, foreground);
            }
        }
        // Block cursor: inverse video — a foreground-filled cell with the
        // covered glyph redrawn in the background color.
        _ => {
            fill_rect(hdc, cell_rect, foreground);
            let covered = snapshot.cells.iter().find(|cell| {
                cell.row == snapshot.cursor_row && cell.col == snapshot.cursor_col
            });
            if let Some(cell) = covered.filter(|cell| !cell.text.is_empty())
                && fonts.select(hdc, cell.bold, cell.italic, cell.underline)
            {
                let text: Vec<u16> = cell.text.encode_utf16().collect();
                unsafe {
                    let _ = SetTextColor(hdc, rgb_to_colorref(background));
                    let _ = ExtTextOutW(
                        hdc,
                        left,
                        top,
                        ETO_OPTIONS(0),
                        None,
                        PCWSTR(text.as_ptr()),
                        text.len() as u32,
                        None,
                    );
                }
            }
        }
    }
}

/// Exited sessions keep their final snapshot in the store; the painter
/// renders this dimmed status line under the last cursor row instead of
/// switching back to body text.
fn draw_exited_overlay(
    hdc: HDC,
    grid_rect: RECT,
    line_height: i32,
    cursor_row: u16,
    background: u32,
    foreground: u32,
    fonts: &mut GridFonts,
) {
    if !fonts.select(hdc, false, false, false) {
        return;
    }
    let visible_rows = (rect_height(&grid_rect) / line_height).max(1);
    let row = (i32::from(cursor_row) + 1).clamp(0, visible_rows - 1);
    let top = grid_rect.top + row * line_height;
    fill_rect(
        hdc,
        RECT {
            left: grid_rect.left,
            top,
            right: grid_rect.right,
            bottom: top + line_height,
        },
        background,
    );
    let text: Vec<u16> = "[process exited]".encode_utf16().collect();
    unsafe {
        let _ = SetTextColor(
            hdc,
            rgb_to_colorref(blend_rgb(foreground, background, GRID_DIM_FOREGROUND_PERCENT)),
        );
        let _ = ExtTextOutW(
            hdc,
            grid_rect.left,
            top,
            ETO_OPTIONS(0),
            None,
            PCWSTR(text.as_ptr()),
            text.len() as u32,
            None,
        );
    }
}

/// Lazily created terminal font variants for one paint pass, keyed by
/// (bold, italic, underline). Deleted on drop; the caller must restore the
/// DC's original font selection (`RestoreDC`) before the cache drops.
struct GridFonts {
    height: i32,
    fonts: [Option<HFONT>; 8],
}

impl GridFonts {
    fn new(hdc: HDC) -> Self {
        Self {
            height: logical_font_height(hdc, GRID_FONT_POINT_SIZE),
            fonts: [None; 8],
        }
    }

    /// Selects the font variant into `hdc`, creating it on first use.
    /// Returns `false` when font creation failed entirely.
    fn select(&mut self, hdc: HDC, bold: bool, italic: bool, underline: bool) -> bool {
        let index = usize::from(bold) | usize::from(italic) << 1 | usize::from(underline) << 2;
        let height = self.height;
        let font = *self.fonts[index]
            .get_or_insert_with(|| create_terminal_font(hdc, height, bold, italic, underline));
        if font.is_invalid() {
            return false;
        }
        unsafe {
            let _ = SelectObject(hdc, HGDIOBJ(font.0));
        }
        true
    }
}

impl Drop for GridFonts {
    fn drop(&mut self) {
        for font in self.fonts.into_iter().flatten() {
            if !font.is_invalid() {
                unsafe {
                    let _ = DeleteObject(HGDIOBJ(font.0));
                }
            }
        }
    }
}

/// Terminal mono font: Cascadia Mono (Win11), falling back to Consolas.
/// Faces are verified via `GetTextFaceW` (the GDI font mapper silently
/// substitutes missing faces); when neither resolves, the empty face name
/// lets the mapper pick any fixed-pitch font via the pitch/family hint.
fn create_terminal_font(hdc: HDC, height: i32, bold: bool, italic: bool, underline: bool) -> HFONT {
    let weight = if bold { 700 } else { 400 };
    for face in ["Cascadia Mono", "Consolas", ""] {
        let face_wide: Vec<u16> = face.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let font = CreateFontW(
                -height,
                0,
                0,
                0,
                weight,
                u32::from(italic),
                u32::from(underline),
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                FIXED_PITCH.0 as u32 | FF_MODERN.0 as u32,
                PCWSTR(face_wide.as_ptr()),
            );
            if font.is_invalid() {
                continue;
            }
            if face.is_empty() {
                return font;
            }
            let old_font = SelectObject(hdc, HGDIOBJ(font.0));
            let mut resolved = [0u16; 64];
            let copied = GetTextFaceW(hdc, Some(&mut resolved)).max(0) as usize;
            if !old_font.is_invalid() {
                let _ = SelectObject(hdc, old_font);
            }
            let resolved_len = resolved
                .iter()
                .position(|&unit| unit == 0)
                .unwrap_or(copied.min(resolved.len()));
            let resolved = String::from_utf16_lossy(&resolved[..resolved_len]);
            if resolved.eq_ignore_ascii_case(face) {
                return font;
            }
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
    }
    HFONT::default()
}

/// Parses the `#rrggbb` color tokens produced by `lingxia-terminal`.
fn parse_hex_color(token: &str) -> Option<u32> {
    let hex = token.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

/// Blends `fg_percent`% of `fg` with the remainder of `bg`, per channel.
fn blend_rgb(fg: u32, bg: u32, fg_percent: u32) -> u32 {
    let blend = |shift: u32| {
        let fg_channel = (fg >> shift) & 0xff;
        let bg_channel = (bg >> shift) & 0xff;
        ((fg_channel * fg_percent + bg_channel * (100 - fg_percent)) / 100) << shift
    };
    blend(16) | blend(8) | blend(0)
}
