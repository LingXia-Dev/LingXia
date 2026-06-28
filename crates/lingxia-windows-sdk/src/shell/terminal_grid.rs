//! Terminal panel grid: the snapshot store shared with the product facade
//! and the GDI cell-grid painter used by the shell chrome.
//!
//! The facade's poll thread pushes full [`TerminalSnapshot`]s through
//! [`set_session_snapshot`] and reads [`desired_session_grid_size`] to keep
//! each pane's PTY grid in sync with its rect; the chrome painter consumes
//! the latest snapshot of every active pane on repaint and records the grid
//! geometry it painted into, so both sides agree on cell metrics. The
//! chrome painter also records the
//! header tab-title rects it painted, which [`begin_tab_rename`] uses to
//! place the inline rename editor. Styling mirrors the macOS terminal
//! surface (`SurfaceCore.swift`): dark `#282C34` background, white default
//! foreground, mono font, dim text blended at 58%.
#![cfg_attr(not(feature = "terminal-runtime"), allow(dead_code))]

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use lingxia_terminal::{TerminalCell, TerminalSnapshot};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, DEFAULT_CHARSET, DeleteObject,
    ETO_OPTIONS, ExtTextOutW, FF_MODERN, FIXED_PITCH, GetTextFaceW, GetTextMetricsW, HDC, HFONT,
    HGDIOBJ, IntersectClipRect, OUT_DEFAULT_PRECIS, RestoreDC, SaveDC, SelectObject, SetBkMode,
    SetTextColor, TEXTMETRICW, TRANSPARENT,
};
use windows::core::PCWSTR;

use super::chrome::{
    fill_rect, inset_rect, logical_font_height, rect_height, rect_width, rgb_to_colorref,
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

/// Minimum grid reported to the PTY, mirroring the macOS surface clamp.
const GRID_MIN_COLS: i32 = 20;

const GRID_MIN_ROWS: i32 = 4;

/// Hairline divider between panes - a soft gray line on the terminal
/// surface, a la ghostty's split divider.
const PANE_DIVIDER_COLOR: u32 = 0x3a3f4a;

/// Unfocused panes keep this fraction of their colors (the remainder blends
/// toward the surface background); the focused pane reads as active without
/// an obtrusive border, closer to ghostty's split focus treatment.
const UNFOCUSED_KEEP_PERCENT: u32 = 62;

/// Cell metrics and grid area recorded at the last paint of a pane.
#[derive(Clone, Copy)]
struct GridGeometry {
    cell_width: i32,
    line_height: i32,
    grid_width: i32,
    grid_height: i32,
}

/// Per-session state: the latest snapshot and the geometry it last painted
/// into (so the facade can keep each pane's PTY grid sized to its rect).
#[derive(Default)]
struct SessionGridState {
    snapshot: Option<TerminalSnapshot>,
    geometry: Option<GridGeometry>,
}

/// Host window and header tab-title rects recorded at the last paint of a
/// panel's header, used to place the inline rename editor.
#[derive(Clone, Default)]
struct PanelHeaderGeometry {
    /// Raw handle of the host window the header was painted into.
    hwnd: isize,
    /// `(tab_id, title rect)` pairs in host client coordinates.
    titles: Vec<(u64, RECT)>,
}

/// Per-panel state: header geometry plus the body rect and cell metrics of
/// the last paint, used to size newly created panes and hit-test clicks.
#[derive(Default)]
struct PanelGridState {
    header: Option<PanelHeaderGeometry>,
    /// Terminal body rect (below the header) at the last paint.
    body: Option<RECT>,
    /// `(cell_width, line_height)` from the last pane paint (font-derived,
    /// shared by every pane).
    cell: Option<(i32, i32)>,
}

static SESSION_GRIDS: OnceLock<Mutex<HashMap<u64, SessionGridState>>> = OnceLock::new();
static PANEL_GRIDS: OnceLock<Mutex<HashMap<String, PanelGridState>>> = OnceLock::new();

fn session_grids() -> MutexGuard<'static, HashMap<u64, SessionGridState>> {
    SESSION_GRIDS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        // The store has no invariants that poisoning can break.
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn panel_grids() -> MutexGuard<'static, HashMap<String, PanelGridState>> {
    PANEL_GRIDS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Stores the latest snapshot for `session_id`; the chrome painter renders
/// it on the next repaint of the host window.
pub fn set_session_snapshot(session_id: u64, snapshot: TerminalSnapshot) {
    session_grids().entry(session_id).or_default().snapshot = Some(snapshot);
}

/// Drops all stored state for one pane session (snapshot and geometry).
pub fn clear_session(session_id: u64) {
    session_grids().remove(&session_id);
}

/// Drops a panel's header/body geometry (its pane sessions are cleared
/// individually via [`clear_session`]).
pub fn clear_panel(panel_id: &str) {
    panel_grids().remove(panel_id);
}

/// Surface background of one session's last snapshot (the `#rrggbb` default
/// background reported by the terminal), or `None` before its first
/// snapshot. The chrome painter fills the dock card with the focused pane's
/// color so the header, card corners, and cell grid agree.
pub(super) fn session_surface_background(session_id: u64) -> Option<u32> {
    session_grids()
        .get(&session_id)?
        .snapshot
        .as_ref()?
        .default_background
        .as_deref()
        .and_then(parse_hex_color)
}

/// Plain-text fallback for the focused pane's snapshot. Used only when the
/// cell-grid painter cannot draw with the current DC/font state.
pub(super) fn panel_snapshot_text(panel_id: &str) -> Option<String> {
    let session_id = super::terminal_panel::focused_session(panel_id)?;
    let grids = session_grids();
    let snapshot = grids.get(&session_id)?.snapshot.as_ref()?;
    let mut lines: Vec<&str> = snapshot.lines.iter().map(|line| line.trim_end()).collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        snapshot
            .title
            .as_deref()
            .or(snapshot.process_title.as_deref())
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToOwned::to_owned)
    } else {
        Some(lines.join("\r\n"))
    }
}

/// The terminal body rect recorded at the last paint of `panel_id` (host
/// client coordinates), used to hit-test pane focus clicks.
pub(super) fn panel_body_rect(panel_id: &str) -> Option<RECT> {
    panel_grids().get(panel_id)?.body
}

/// Records the host window and tab-title rects the chrome painter drew for
/// `panel_id`'s header, so [`begin_tab_rename`] can place the inline
/// editor over the renamed title.
pub(super) fn set_panel_tab_title_rects(panel_id: &str, hwnd: isize, titles: Vec<(u64, RECT)>) {
    panel_grids()
        .entry(panel_id.to_string())
        .or_default()
        .header = Some(PanelHeaderGeometry { hwnd, titles });
}

/// Starts an inline rename of `tab_id`'s title in `panel_id`'s header: an
/// EDIT child (see [`super::text_input`]) is created over the title rect
/// recorded at the last paint. Safe to call from any thread; the editor
/// is marshalled onto the host window's UI thread. `on_commit` receives
/// the edited text on Enter/focus loss (Esc cancels); it runs on that UI
/// thread. Returns `false` when the tab has not been painted yet or the
/// host window is gone.
pub fn begin_tab_rename(
    panel_id: &str,
    tab_id: u64,
    initial_text: &str,
    on_commit: Arc<dyn Fn(String) + Send + Sync>,
) -> bool {
    let header = panel_grids()
        .get(panel_id)
        .and_then(|state| state.header.clone());
    let Some(header) = header else {
        return false;
    };
    let Some((_, rect)) = header.titles.iter().find(|(id, _)| *id == tab_id).copied() else {
        return false;
    };
    let hwnd = header.hwnd;
    let initial = initial_text.to_string();
    lingxia_windows_contract::post_to_window_thread(
        hwnd,
        Box::new(move || {
            super::text_input::begin_inline_edit(
                HWND(hwnd as *mut c_void),
                rect,
                &initial,
                on_commit,
            );
        }),
    )
}

/// Grid size `(cols, rows)` that fits one pane session's painted rect, or
/// `None` before that pane was first painted.
pub fn desired_session_grid_size(session_id: u64) -> Option<(u16, u16)> {
    let grids = session_grids();
    let geometry = grids.get(&session_id)?.geometry?;
    grid_size_from_geometry(geometry)
}

/// Grid size `(cols, rows)` for the whole panel body (a sensible default
/// for a freshly created pane before it has been painted), or `None` before
/// the panel was first painted.
pub fn desired_panel_grid_size(panel_id: &str) -> Option<(u16, u16)> {
    let grids = panel_grids();
    let state = grids.get(panel_id)?;
    let body = state.body?;
    let (cell_width, line_height) = state.cell?;
    grid_size_from_geometry(GridGeometry {
        cell_width,
        line_height,
        grid_width: rect_width(&inset_rect(body, GRID_PADDING, GRID_PADDING)),
        grid_height: rect_height(&inset_rect(body, GRID_PADDING, GRID_PADDING)),
    })
}

fn grid_size_from_geometry(geometry: GridGeometry) -> Option<(u16, u16)> {
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

/// Paints every pane of `panel_id`'s active tab into `body` (the dock body
/// below the panel header row). Sibling panes are separated by a hairline
/// divider and, when a tab has more than one pane, the unfocused panes are
/// dimmed so the focused one reads as active (no border). Returns `false`
/// when no pane has a snapshot yet so the caller can fall back to body-text
/// rendering; pane geometry is recorded either way so the facade can size
/// each PTY from the first paint on.
pub(super) fn draw_panel_panes(hdc: HDC, panel_id: &str, body: RECT) -> bool {
    panel_grids().entry(panel_id.to_string()).or_default().body = Some(body);

    let frames = super::terminal_panel::active_pane_frames(panel_id, body);
    if frames.is_empty() {
        return false;
    }
    let multi = frames.len() > 1;

    // Divider gaps show through as dark lines between panes.
    if multi {
        fill_rect(hdc, body, PANE_DIVIDER_COLOR);
    }

    let mut fonts = GridFonts::new(hdc);
    let mut drew_any = false;
    for frame in &frames {
        // Each pane fills its own surface background so panes with distinct
        // backgrounds (and the divider gaps) stay correct.
        let surface =
            session_surface_background(frame.session_id).unwrap_or(GRID_DEFAULT_BACKGROUND);
        fill_rect(hdc, frame.rect, surface);

        let grid_rect = inset_rect(frame.rect, GRID_PADDING, GRID_PADDING);
        if rect_width(&grid_rect) > 0 && rect_height(&grid_rect) > 0 {
            // SaveDC/RestoreDC bracket all font/clip/color changes; the
            // fonts are deleted (on drop) only after the DC stopped
            // referencing them.
            let saved = unsafe { SaveDC(hdc) };
            // Dim every pane except the focused one (only when split).
            let dim = multi && !frame.focused;
            drew_any |=
                draw_pane_grid_clipped(hdc, panel_id, frame.session_id, grid_rect, dim, &mut fonts);
            unsafe {
                let _ = RestoreDC(hdc, saved);
            }
        }
    }
    drew_any
}

fn draw_pane_grid_clipped(
    hdc: HDC,
    panel_id: &str,
    session_id: u64,
    grid_rect: RECT,
    dim: bool,
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

    panel_grids().entry(panel_id.to_string()).or_default().cell = Some((cell_width, line_height));

    // Hold the session store lock while drawing this pane (the snapshot is
    // not `Clone`); the lock is released between panes.
    let mut grids = session_grids();
    let state = grids.entry(session_id).or_default();
    state.geometry = Some(GridGeometry {
        cell_width,
        line_height,
        grid_width: rect_width(&grid_rect),
        grid_height: rect_height(&grid_rect),
    });
    let Some(snapshot) = state.snapshot.as_ref() else {
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
        let Some(mut cell_background) = token.and_then(parse_hex_color) else {
            continue;
        };
        if dim {
            cell_background = dim_unfocused(cell_background, background);
        }
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
        dim,
        fonts,
    );

    // Exited sessions are closed by the facade as soon as their snapshot
    // reports `exited` (the pane closes; the last pane closes the tab), so
    // no `[process exited]` overlay is drawn; at most one repaint shows
    // the final screen without a cursor. Unfocused panes show a hollow
    // cursor (drawn dimmed) like a conventional inactive terminal.
    if !snapshot.exited && snapshot.cursor_visible {
        draw_cursor(
            hdc,
            snapshot,
            grid_rect,
            cell_width,
            line_height,
            background,
            foreground,
            dim,
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

fn cell_style(cell: &TerminalCell, background: u32, foreground: u32, dim: bool) -> RunStyle {
    let token = if cell.inverse {
        cell.bg.as_deref()
    } else {
        cell.fg.as_deref()
    };
    let mut color = token.and_then(parse_hex_color).unwrap_or(foreground);
    if cell.dim {
        color = blend_rgb(color, background, GRID_DIM_FOREGROUND_PERCENT);
    }
    // Unfocused split pane: fade the text toward the surface background.
    if dim {
        color = dim_unfocused(color, background);
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
    dim: bool,
    fonts: &mut GridFonts,
) {
    let mut run: Option<GridRun> = None;
    // Snapshot cells arrive in row-major order.
    for cell in &snapshot.cells {
        if cell.text.is_empty() {
            continue;
        }
        let style = cell_style(cell, background, foreground, dim);
        let continues = run.as_ref().is_some_and(|run| {
            run.row == cell.row && run.next_col == cell.col && run.style == style
        });
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
    dim: bool,
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
    // Unfocused split pane: a dimmed hollow cursor (conventional inactive
    // terminal look), regardless of the session's cursor style.
    if dim {
        let color = dim_unfocused(foreground, background);
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
            fill_rect(hdc, edge, color);
        }
        return;
    }
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
        // Block cursor: inverse video: a foreground-filled cell with the
        // covered glyph redrawn in the background color.
        _ => {
            fill_rect(hdc, cell_rect, foreground);
            let covered = snapshot
                .cells
                .iter()
                .find(|cell| cell.row == snapshot.cursor_row && cell.col == snapshot.cursor_col);
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

/// Fades `color` toward `background` for an unfocused split pane.
fn dim_unfocused(color: u32, background: u32) -> u32 {
    blend_rgb(color, background, UNFOCUSED_KEEP_PERCENT)
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
