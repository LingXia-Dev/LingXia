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

/// Windows selection highlight, blended toward each pane's background.
const SELECTION_ACCENT: u32 = 0x4b9cff;

const SELECTION_ACCENT_PERCENT: u32 = 46;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GridPoint {
    row: u16,
    col: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GridSelection {
    anchor: GridPoint,
    focus: GridPoint,
}

impl GridSelection {
    fn normalized(self) -> Option<(GridPoint, GridPoint)> {
        if self.anchor == self.focus {
            return None;
        }
        let anchor_after_focus = self.anchor.row > self.focus.row
            || (self.anchor.row == self.focus.row && self.anchor.col > self.focus.col);
        Some(if anchor_after_focus {
            (self.focus, self.anchor)
        } else {
            (self.anchor, self.focus)
        })
    }
}

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
    selection: Option<GridSelection>,
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
    /// Pane whose selection is currently being dragged.
    selection_session: Option<u64>,
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
    for panel in panel_grids().values_mut() {
        if panel.selection_session == Some(session_id) {
            panel.selection_session = None;
        }
    }
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

#[cfg(feature = "shell-chrome")]
fn selection_point(
    panel_id: &str,
    session_id: Option<u64>,
    client_x: i32,
    client_y: i32,
) -> Option<(u64, GridPoint)> {
    let (body, cell_width, line_height) = {
        let panels = panel_grids();
        let panel = panels.get(panel_id)?;
        let body = panel.body?;
        let (cell_width, line_height) = panel.cell?;
        (body, cell_width.max(1), line_height.max(1))
    };
    let frames = super::terminal_panel::active_pane_frames(panel_id, body);
    let frame = match session_id {
        Some(session_id) => frames
            .into_iter()
            .find(|frame| frame.session_id == session_id)?,
        None => frames.into_iter().find(|frame| {
            client_x >= frame.rect.left
                && client_x < frame.rect.right
                && client_y >= frame.rect.top
                && client_y < frame.rect.bottom
        })?,
    };
    let grid = inset_rect(frame.rect, GRID_PADDING, GRID_PADDING);
    let grids = session_grids();
    let snapshot = grids.get(&frame.session_id)?.snapshot.as_ref()?;
    let relative_x = (client_x - grid.left).clamp(0, rect_width(&grid).max(0));
    let relative_y = (client_y - grid.top).clamp(0, rect_height(&grid).saturating_sub(1));
    Some((
        frame.session_id,
        GridPoint {
            row: (relative_y / line_height).clamp(0, i32::from(snapshot.rows.saturating_sub(1)))
                as u16,
            col: (relative_x / cell_width).clamp(0, i32::from(snapshot.cols)) as u16,
        },
    ))
}

/// Session and zero-based grid cell under a host-client point.
#[cfg(feature = "shell-chrome")]
pub(super) fn session_cell_at(
    panel_id: &str,
    client_x: i32,
    client_y: i32,
) -> Option<(u64, u16, u16)> {
    selection_point(panel_id, None, client_x, client_y)
        .map(|(session_id, point)| (session_id, point.col, point.row))
}

/// Starts a cell selection in the pane under the pointer.
#[cfg(feature = "shell-chrome")]
pub(crate) fn begin_selection_at(panel_id: &str, client_x: i32, client_y: i32) -> bool {
    let Some((session_id, point)) = selection_point(panel_id, None, client_x, client_y) else {
        return false;
    };
    {
        let mut grids = session_grids();
        for state in grids.values_mut() {
            state.selection = None;
        }
        grids.entry(session_id).or_default().selection = Some(GridSelection {
            anchor: point,
            focus: point,
        });
    }
    panel_grids()
        .entry(panel_id.to_string())
        .or_default()
        .selection_session = Some(session_id);
    true
}

/// Updates the active drag selection, clamping beyond the pane edges.
#[cfg(feature = "shell-chrome")]
pub(crate) fn update_selection_at(panel_id: &str, client_x: i32, client_y: i32) -> bool {
    let session_id = panel_grids()
        .get(panel_id)
        .and_then(|panel| panel.selection_session);
    let Some(session_id) = session_id else {
        return false;
    };
    let Some((_, point)) = selection_point(panel_id, Some(session_id), client_x, client_y) else {
        return false;
    };
    let mut grids = session_grids();
    let Some(selection) = grids
        .get_mut(&session_id)
        .and_then(|state| state.selection.as_mut())
    else {
        return false;
    };
    selection.focus = point;
    true
}

/// Finishes the current drag while preserving a non-empty selection.
#[cfg(feature = "shell-chrome")]
pub(crate) fn end_selection(panel_id: &str) -> bool {
    let session_id = {
        let mut panels = panel_grids();
        let Some(panel) = panels.get_mut(panel_id) else {
            return false;
        };
        panel.selection_session.take()
    };
    let Some(session_id) = session_id else {
        return false;
    };
    let mut grids = session_grids();
    if let Some(state) = grids.get_mut(&session_id)
        && state
            .selection
            .is_some_and(|selection| selection.normalized().is_none())
    {
        state.selection = None;
    }
    true
}

#[cfg(feature = "shell-chrome")]
pub(crate) fn clear_selection(session_id: u64) {
    if let Some(state) = session_grids().get_mut(&session_id) {
        state.selection = None;
    }
}

/// Text covered by the current selection, preserving line boundaries.
#[cfg(feature = "shell-chrome")]
pub(crate) fn selected_text(session_id: u64) -> Option<String> {
    let grids = session_grids();
    let state = grids.get(&session_id)?;
    let snapshot = state.snapshot.as_ref()?;
    selected_text_from_snapshot(snapshot, state.selection?)
}

fn selected_text_from_snapshot(
    snapshot: &TerminalSnapshot,
    selection: GridSelection,
) -> Option<String> {
    let (start, end) = selection.normalized()?;
    let mut lines = Vec::new();
    for row in start.row..=end.row {
        let start_col = if row == start.row { start.col } else { 0 };
        let end_col = if row == end.row {
            end.col
        } else {
            snapshot.cols
        };
        lines.push(text_in_row(snapshot, row, start_col, end_col));
    }
    let text = lines.join("\r\n");
    (!text.is_empty()).then_some(text)
}

fn text_in_row(snapshot: &TerminalSnapshot, row: u16, start_col: u16, end_col: u16) -> String {
    let mut text = String::new();
    let mut next_col = start_col;
    for cell in snapshot.cells.iter().filter(|cell| {
        cell.row == row && cell.col >= start_col && cell.col < end_col && !cell.text.is_empty()
    }) {
        if cell.col > next_col {
            text.extend(std::iter::repeat_n(' ', usize::from(cell.col - next_col)));
        }
        text.push_str(&cell.text);
        next_col = cell.col.saturating_add(if cell.wide { 2 } else { 1 });
    }
    text.trim_end().to_string()
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
    let selection = state.selection;

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
        if !cell.inverse && cell.bg.is_none() {
            continue;
        }
        let (_, mut cell_background) = resolved_cell_colors(cell, background, foreground);
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

    if let Some((start, end)) = selection.and_then(GridSelection::normalized) {
        draw_selection_overlay(
            hdc,
            snapshot.cols,
            start,
            end,
            grid_rect,
            cell_width,
            line_height,
            background,
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

    // Match macOS: only the focused pane paints the terminal cursor. Drawing
    // hollow cursors in every split makes cursor-heavy TUIs appear to flicker
    // at several positions at once.
    if !dim && !snapshot.exited && snapshot.cursor_visible {
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

#[allow(clippy::too_many_arguments)]
fn draw_selection_overlay(
    hdc: HDC,
    cols: u16,
    start: GridPoint,
    end: GridPoint,
    grid_rect: RECT,
    cell_width: i32,
    line_height: i32,
    background: u32,
) {
    let highlight = blend_rgb(SELECTION_ACCENT, background, SELECTION_ACCENT_PERCENT);
    for row in start.row..=end.row {
        let start_col = if row == start.row { start.col } else { 0 };
        let end_col = if row == end.row { end.col } else { cols };
        if end_col <= start_col {
            continue;
        }
        fill_rect(
            hdc,
            RECT {
                left: grid_rect.left + i32::from(start_col) * cell_width,
                top: grid_rect.top + i32::from(row) * line_height,
                right: grid_rect.left + i32::from(end_col) * cell_width,
                bottom: grid_rect.top + i32::from(row + 1) * line_height,
            },
            highlight,
        );
    }
}

/// Per-run text style after color resolution (inverse swap + dim blend).
#[derive(Clone, Copy, PartialEq, Eq)]
struct RunStyle {
    color: u32,
    bold: bool,
    italic: bool,
    underline: bool,
}

fn resolved_cell_colors(cell: &TerminalCell, background: u32, foreground: u32) -> (u32, u32) {
    let normal_foreground = cell
        .fg
        .as_deref()
        .and_then(parse_hex_color)
        .unwrap_or(foreground);
    let normal_background = cell
        .bg
        .as_deref()
        .and_then(parse_hex_color)
        .unwrap_or(background);
    if cell.inverse {
        (normal_background, normal_foreground)
    } else {
        (normal_foreground, normal_background)
    }
}

fn cell_style(cell: &TerminalCell, background: u32, foreground: u32, dim: bool) -> RunStyle {
    let (mut color, cell_background) = resolved_cell_colors(cell, background, foreground);
    if cell.dim {
        color = blend_rgb(color, cell_background, GRID_DIM_FOREGROUND_PERCENT);
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
        // Block cursor: inverse video: a foreground-filled cell with the
        // covered glyph redrawn in the background color.
        _ => {
            let covered = snapshot
                .cells
                .iter()
                .find(|cell| cell.row == snapshot.cursor_row && cell.col == snapshot.cursor_col);
            let (cursor_background, cursor_foreground) = covered
                .map(|cell| resolved_cell_colors(cell, background, foreground))
                .unwrap_or((foreground, background));
            fill_rect(hdc, cell_rect, cursor_background);
            if let Some(cell) = covered.filter(|cell| !cell.text.is_empty())
                && fonts.select(hdc, cell.bold, cell.italic, cell.underline)
            {
                let text: Vec<u16> = cell.text.encode_utf16().collect();
                unsafe {
                    let _ = SetTextColor(hdc, rgb_to_colorref(cursor_foreground));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(row: u16, col: u16, text: &str) -> TerminalCell {
        TerminalCell {
            row,
            col,
            text: text.to_string(),
            fg: None,
            bg: None,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            inverse: false,
            wide: false,
        }
    }

    fn snapshot(cells: Vec<TerminalCell>) -> TerminalSnapshot {
        TerminalSnapshot {
            cols: 8,
            rows: 2,
            lines: Vec::new(),
            cells,
            default_foreground: None,
            default_background: None,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: false,
            cursor_style: "block",
            application_cursor: false,
            bracketed_paste: false,
            alternate_screen: false,
            process_title: None,
            title: None,
            generation: 0,
            title_generation: 0,
            exited: false,
        }
    }

    #[test]
    fn normalizes_reverse_selection() {
        let selection = GridSelection {
            anchor: GridPoint { row: 1, col: 4 },
            focus: GridPoint { row: 0, col: 2 },
        };
        assert_eq!(
            selection.normalized(),
            Some((GridPoint { row: 0, col: 2 }, GridPoint { row: 1, col: 4 }))
        );
    }

    #[test]
    fn extracts_selected_cells_with_spaces_and_lines() {
        let snapshot = snapshot(vec![
            cell(0, 1, "a"),
            cell(0, 2, "b"),
            cell(0, 5, "c"),
            cell(1, 0, "d"),
            cell(1, 1, "e"),
        ]);
        let text = selected_text_from_snapshot(
            &snapshot,
            GridSelection {
                anchor: GridPoint { row: 0, col: 1 },
                focus: GridPoint { row: 1, col: 2 },
            },
        );
        assert_eq!(text.as_deref(), Some("ab  c\r\nde"));
    }

    #[test]
    fn inverse_default_colors_are_swapped() {
        let mut inverse = cell(0, 0, "x");
        inverse.inverse = true;
        inverse.fg = Some("#ddeeff".to_string());

        assert_eq!(
            resolved_cell_colors(&inverse, 0x112233, 0xaabbcc),
            (0x112233, 0xddeeff)
        );
        assert_eq!(
            cell_style(&inverse, 0x112233, 0xaabbcc, false).color,
            0x112233
        );
    }
}

/// Terminal mono font: prefer modern terminal faces, then Windows' built-ins.
/// Faces are verified via `GetTextFaceW` (the GDI font mapper silently
/// substitutes missing faces); when none resolves, the empty face name
/// lets the mapper pick any fixed-pitch font via the pitch/family hint.
fn create_terminal_font(hdc: HDC, height: i32, bold: bool, italic: bool, underline: bool) -> HFONT {
    let weight = if bold { 700 } else { 400 };
    for face in [
        "Cascadia Mono",
        "Cascadia Code",
        "JetBrains Mono",
        "Sarasa Mono SC",
        "Consolas",
        "Courier New",
        "",
    ] {
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
