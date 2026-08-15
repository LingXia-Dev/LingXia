//! Terminal panel state: the frame store, the grid geometry, and everything
//! the shell hit-tests against.
//!
//! The facade's poll thread pushes renderer-ready frames through
//! [`set_session_frame`] and reads [`desired_session_grid_size`] to keep each
//! pane's PTY sized to its rect. Drawing belongs to the renderer, which takes
//! a pane's frame through [`with_pane`] and records the geometry it drew at,
//! so hit-testing and PTY sizing agree with what is on screen.
//!
//! A frame is two buffers and a damage list, not a cell tree: colors arrive as
//! packed RGBA and every cluster lives in one text blob. The store keeps the
//! generation it last accepted so an idle session costs nothing — the engine
//! answers `Unchanged` and there is no repaint at all.
#![cfg_attr(not(feature = "terminal-runtime"), allow(dead_code))]

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use super::chrome::{inset_rect, rect_height, rect_width};
use lingxia_terminal::{TerminalFrame, TerminalImageSnapshot, TerminalScrollbar};
use windows::Win32::Foundation::{HWND, RECT};

/// Inner padding between the terminal card edge and the cell grid.
pub(super) const GRID_PADDING: i32 = 8;

/// Fallback surface color, for a pane that has no frame yet. Once one
/// arrives it carries the scheme's own defaults, so there is nothing to fall
/// back to for the foreground.
pub(super) const GRID_DEFAULT_BACKGROUND: u32 = 0x282c34;

/// Dim cells blend the foreground this far toward the background (the
/// macOS surface draws dim text at 0.58 alpha).
pub(super) const GRID_DIM_FOREGROUND_PERCENT: u32 = 58;

/// Minimum grid reported to the PTY, mirroring the macOS surface clamp.
const GRID_MIN_COLS: i32 = 20;

const GRID_MIN_ROWS: i32 = 4;

/// Outline drawn around the pane a dragged pane would land on.
pub(super) const PANE_DROP_TARGET_COLOR: u32 = 0x4b9cff;

/// Keep the overlay visible briefly after the latest wheel gesture.
const SCROLLBAR_VISIBLE_FOR: Duration = Duration::from_millis(900);

pub(super) const SCROLLBAR_WIDTH: i32 = 3;
pub(super) const SCROLLBAR_MARGIN: i32 = 2;
pub(super) const SCROLLBAR_MIN_THUMB: i32 = 12;
pub(super) const SCROLLBAR_MAX_THUMB: i32 = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct GridPoint {
    pub(super) row: u16,
    pub(super) col: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SearchHighlight {
    pub(super) row: u16,
    pub(super) start_col: u16,
    pub(super) end_col: u16,
    pub(super) active: bool,
}

#[derive(Default)]
struct GridSearch {
    matches: Vec<lingxia_terminal::TerminalSearchMatch>,
    active: Option<usize>,
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

/// What a pane shows beyond its grid.
#[derive(Clone, Copy, Default)]
pub(super) struct PaneView {
    pub scrollbar: Option<TerminalScrollbar>,
    pub exited: bool,
}

/// Per-session state: the latest frame and the geometry it last painted into
/// (so the facade can keep each pane's PTY grid sized to its rect).
#[derive(Default)]
struct SessionGridState {
    frame: Option<TerminalFrame>,
    images: TerminalImageSnapshot,
    /// Generation of `frame`; handed back to the engine so it can answer
    /// "nothing changed" instead of building a frame nobody needs.
    generation: u64,
    /// The child is gone — the renderer stops drawing a cursor for it.
    exited: bool,
    /// Scroll position. Not part of a frame: it moves on its own schedule.
    scrollbar: Option<TerminalScrollbar>,
    geometry: Option<GridGeometry>,
    selection: Option<GridSelection>,
    search: GridSearch,
    scrollbar_visible_until: Option<Instant>,
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
    /// Terminal body rect, below the header, as last drawn.
    body: Option<RECT>,
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

/// Atomically publish whichever parts of a renderer update changed.
pub fn set_session_render(
    session_id: u64,
    frame: Option<TerminalFrame>,
    images: Option<TerminalImageSnapshot>,
) {
    let mut grids = session_grids();
    let state = grids.entry(session_id).or_default();
    if let Some(frame) = frame {
        state.generation = frame.generation;
        state.frame = Some(frame);
    }
    if let Some(images) = images {
        state.images = images;
    }
}

/// The parts of a pane that move without the grid changing, so an exit or a
/// scroll still reaches the renderer on a generation it already has.
/// Returns whether anything moved, so a caller can skip a repaint the grid
/// did not ask for.
pub fn set_session_view_state(
    session_id: u64,
    scrollbar: Option<TerminalScrollbar>,
    exited: bool,
) -> bool {
    let mut grids = session_grids();
    let state = grids.entry(session_id).or_default();
    let moved = state.exited != exited
        || state.scrollbar.map(|bar| (bar.total, bar.offset, bar.len))
            != scrollbar.map(|bar| (bar.total, bar.offset, bar.len));
    state.scrollbar = scrollbar;
    state.exited = exited;
    moved
}

/// The grid size the store last painted, for callers deciding whether the PTY
/// needs resizing when no new frame arrived.
pub fn session_grid_size(session_id: u64) -> Option<(u16, u16)> {
    let grids = session_grids();
    let frame = grids.get(&session_id)?.frame.as_ref()?;
    Some((frame.cols, frame.rows))
}

/// The generation the store already holds, so the caller can ask the engine
/// only for what changed since.
pub fn session_generation(session_id: u64) -> u64 {
    session_grids()
        .get(&session_id)
        .map(|state| state.generation)
        .unwrap_or(0)
}

pub fn session_image_generation(session_id: u64) -> u64 {
    session_grids()
        .get(&session_id)
        .map(|state| state.images.generation)
        .unwrap_or(0)
}

/// Compact semantic frame state for trusted terminal automation. Keep this in
/// the frame store so the automation path observes exactly what the renderer
/// has accepted, without asking the engine for a second copy of the grid.
#[cfg(feature = "terminal-runtime")]
pub(super) fn automation_grid_snapshot(session_id: u64) -> Option<serde_json::Value> {
    let grids = session_grids();
    let frame = grids.get(&session_id)?.frame.as_ref()?;
    Some(serde_json::json!({
        "cols": frame.cols,
        "rows": frame.rows,
        "generation": frame.generation,
        "imageGeneration": grids[&session_id].images.generation,
        "imageCount": grids[&session_id].images.images.len(),
        "imagePlacementCount": grids[&session_id].images.placements.len(),
        "defaultForeground": frame.default_fg,
        "defaultBackground": frame.default_bg,
        "cursorRow": frame.cursor.row,
        "cursorCol": frame.cursor.col,
        "cursorVisible": frame.cursor.visible,
        "cursorStyle": frame.cursor.style.as_str().replace('_', "-"),
    }))
}

/// Reveals the lightweight scrollbar for one pane after a scroll gesture.
#[cfg(feature = "shell-chrome")]
pub(crate) fn reveal_scrollbar(session_id: u64) {
    session_grids()
        .entry(session_id)
        .or_default()
        .scrollbar_visible_until = Some(Instant::now() + SCROLLBAR_VISIBLE_FOR);
}

/// Clears an expired transient scrollbar and reports whether a repaint is
/// needed to remove its thumb.
#[cfg(feature = "shell-chrome")]
pub(crate) fn expire_scrollbar(session_id: u64) -> bool {
    let mut grids = session_grids();
    let Some(state) = grids.get_mut(&session_id) else {
        return false;
    };
    if state
        .scrollbar_visible_until
        .is_some_and(|deadline| Instant::now() >= deadline)
    {
        state.scrollbar_visible_until = None;
        return true;
    }
    false
}

/// Drops all stored state for one pane session (snapshot and geometry).
pub fn clear_session(session_id: u64) {
    session_grids().remove(&session_id);
    #[cfg(feature = "shell-chrome")]
    super::terminal_gpu::drop_session(session_id);
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
/// The card's colors, from the scheme in effect. One rule shared with the
/// Apple host, so a theme change moves the whole card.
pub(crate) fn surface_chrome() -> super::PanelChrome {
    let chrome = lingxia_terminal_config::runtime::current_chrome();
    super::PanelChrome {
        surface: chrome.surface,
        header: chrome.header,
        separator: chrome.separator,
        text: chrome.text,
        text_muted: chrome.text_muted,
    }
}

pub(super) fn session_surface_background(session_id: u64) -> Option<u32> {
    let grids = session_grids();
    let frame = grids.get(&session_id)?.frame.as_ref()?;
    // Frame colors are packed 0xRRGGBBAA; the painter wants 0xRRGGBB. The
    // frame's defaults always carry alpha — only a *cell* uses alpha 0 to mean
    // "inherit the default".
    Some(frame.default_bg >> 8)
}

/// Plain-text fallback for the focused pane's snapshot. Used only when the
/// cell-grid painter cannot draw with the current DC/font state.
pub(super) fn panel_snapshot_text(panel_id: &str) -> Option<String> {
    let session_id = super::terminal_panel::focused_session(panel_id)?;
    // Asked for once, when a panel is shown — not on the render path. Paying
    // for a full snapshot here costs nothing and keeps the frame store free of
    // the line text and process title only this needs.
    let snapshot = lingxia_terminal::terminal_snapshot_data(session_id)?;
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

/// Host window and body-relative placement for the terminal find field.
pub(super) fn search_edit_geometry(panel_id: &str) -> Option<(isize, RECT)> {
    let panels = panel_grids();
    let panel = panels.get(panel_id)?;
    let hwnd = panel.header.as_ref()?.hwnd;
    let body = panel.body?;
    let width = rect_width(&body).clamp(320, 410);
    Some((
        hwnd,
        RECT {
            left: body.right - width - 12,
            top: body.top + 10,
            right: body.right - 12,
            bottom: (body.top + 56).min(body.bottom),
        },
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TerminalImageHit {
    session_id: u64,
    pub(crate) image_id: u32,
}

pub(crate) struct TerminalPreviewImage {
    pub(crate) id: u32,
    pub(crate) png: Vec<u8>,
}

pub(crate) fn search_status(session_id: u64) -> (Option<usize>, u64) {
    let grids = session_grids();
    let Some(search) = grids.get(&session_id).map(|state| &state.search) else {
        return (None, 0);
    };
    (search.active, search.matches.len() as u64)
}

/// Focused terminal cursor cell in host-window client coordinates. Windows
/// IME uses this to keep composition and candidate UI attached to the prompt.
#[cfg(feature = "shell-chrome")]
pub(crate) fn focused_cursor_rect(panel_id: &str) -> Option<RECT> {
    let body = panel_grids().get(panel_id)?.body?;
    let frame = super::terminal_panel::active_pane_frames(panel_id, body)
        .into_iter()
        .find(|frame| frame.focused)?;
    let grids = session_grids();
    let state = grids.get(&frame.session_id)?;
    let painted = state.frame.as_ref()?;
    let geometry = state.geometry?;
    cursor_rect_for_grid(
        inset_rect(frame.rect, GRID_PADDING, GRID_PADDING),
        painted.cursor.col,
        painted.cursor.row,
        painted.cols,
        painted.rows,
        geometry.cell_width,
        geometry.line_height,
    )
}

#[allow(clippy::too_many_arguments)]
fn cursor_rect_for_grid(
    grid: RECT,
    cursor_col: u16,
    cursor_row: u16,
    cols: u16,
    rows: u16,
    cell_width: i32,
    line_height: i32,
) -> Option<RECT> {
    if cols == 0 || rows == 0 || cell_width <= 0 || line_height <= 0 {
        return None;
    }
    let col = i32::from(cursor_col.min(cols - 1));
    let row = i32::from(cursor_row.min(rows - 1));
    let left = grid.left + col * cell_width;
    let top = grid.top + row * line_height;
    Some(RECT {
        left,
        top,
        right: (left + cell_width).min(grid.right),
        bottom: (top + line_height).min(grid.bottom),
    })
    .filter(|rect| rect.right > rect.left && rect.bottom > rect.top)
}

#[cfg(feature = "shell-chrome")]
fn selection_point(
    panel_id: &str,
    session_id: Option<u64>,
    client_x: i32,
    client_y: i32,
) -> Option<(u64, GridPoint)> {
    let (cell_width, line_height) = super::terminal_gpu::cell_size()?;
    let (cell_width, line_height) = (cell_width.max(1), line_height.max(1));
    let body = {
        let panels = panel_grids();
        panels.get(panel_id)?.body?
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
    let painted = grids.get(&frame.session_id)?.frame.as_ref()?;
    let relative_x = (client_x - grid.left).clamp(0, rect_width(&grid).max(0));
    let relative_y = (client_y - grid.top).clamp(0, rect_height(&grid).saturating_sub(1));
    Some((
        frame.session_id,
        GridPoint {
            row: (relative_y / line_height).clamp(0, i32::from(painted.rows.saturating_sub(1)))
                as u16,
            col: (relative_x / cell_width).clamp(0, i32::from(painted.cols)) as u16,
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

/// Image placement under a host-client point, using the renderer's exact
/// clipped draw order. The session is retained because Kitty image ids are
/// scoped to one terminal session and can repeat in adjacent panes.
#[cfg(feature = "shell-chrome")]
pub(crate) fn image_hit_at(
    panel_id: &str,
    client_x: i32,
    client_y: i32,
) -> Option<TerminalImageHit> {
    let cell = super::terminal_gpu::cell_size_exact()?;
    let body = panel_grids().get(panel_id)?.body?;
    let pane = super::terminal_panel::active_pane_frames(panel_id, body)
        .into_iter()
        .find(|pane| {
            client_x >= pane.rect.left
                && client_x < pane.rect.right
                && client_y >= pane.rect.top
                && client_y < pane.rect.bottom
        })?;
    let grids = session_grids();
    let state = grids.get(&pane.session_id)?;
    let frame = state.frame.as_ref()?;
    let image_id = super::terminal_gpu::image_id_at(
        frame,
        &state.images,
        PaneView {
            scrollbar: state.scrollbar,
            exited: state.exited,
        },
        pane.rect,
        cell,
        (client_x, client_y),
    )?;
    Some(TerminalImageHit {
        session_id: pane.session_id,
        image_id,
    })
}

#[cfg(feature = "shell-chrome")]
pub(crate) fn preview_image(hit: TerminalImageHit) -> Option<TerminalPreviewImage> {
    let grids = session_grids();
    let image = grids
        .get(&hit.session_id)?
        .images
        .images
        .iter()
        .find(|image| image.id == hit.image_id)?;
    Some(TerminalPreviewImage {
        id: image.id,
        png: image.png.clone(),
    })
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

pub(crate) fn set_search_results(
    session_id: u64,
    results: lingxia_terminal::TerminalSearchResults,
) -> Option<i64> {
    let mut grids = session_grids();
    let search = &mut grids.entry(session_id).or_default().search;
    search.matches = results.matches;
    search.active = (!search.matches.is_empty()).then_some(0);
    search
        .active
        .and_then(|index| search.matches.get(index))
        .map(|found| found.start_line)
}

pub(crate) fn navigate_search(session_id: u64, delta: i32) -> Option<i64> {
    let mut grids = session_grids();
    let search = &mut grids.get_mut(&session_id)?.search;
    if search.matches.is_empty() {
        return None;
    }
    let active = next_search_index(search.active, search.matches.len(), delta)?;
    search.active = Some(active);
    Some(search.matches[active].start_line)
}

fn next_search_index(current: Option<usize>, count: usize, delta: i32) -> Option<usize> {
    let count = i32::try_from(count).ok().filter(|count| *count > 0)?;
    Some((current.unwrap_or(0) as i32 + delta).rem_euclid(count) as usize)
}

pub(crate) fn clear_search(session_id: u64) {
    if let Some(state) = session_grids().get_mut(&session_id) {
        state.search = GridSearch::default();
    }
    lingxia_terminal::terminal_search_cancel(session_id);
}

fn visible_search_highlights(
    frame: &TerminalFrame,
    scrollbar: Option<TerminalScrollbar>,
    search: &GridSearch,
) -> Vec<SearchHighlight> {
    let top = scrollbar.map_or(0, |bar| bar.offset as i64);
    let bottom = top + i64::from(frame.rows);
    let mut spans = Vec::new();
    for (index, found) in search.matches.iter().enumerate() {
        let first = found.start_line.max(top);
        let last = found.end_line.min(bottom - 1);
        if first > last {
            continue;
        }
        for line in first..=last {
            let start_col = if line == found.start_line {
                found.start_col
            } else {
                0
            };
            let end_col = if line == found.end_line {
                found.end_col.min(frame.cols)
            } else {
                frame.cols
            };
            if end_col > start_col {
                spans.push(SearchHighlight {
                    row: (line - top) as u16,
                    start_col,
                    end_col,
                    active: search.active == Some(index),
                });
            }
        }
    }
    spans
}

/// Text covered by the current selection, preserving line boundaries.
#[cfg(feature = "shell-chrome")]
pub(crate) fn selected_text(session_id: u64) -> Option<String> {
    let grids = session_grids();
    let state = grids.get(&session_id)?;
    let frame = state.frame.as_ref()?;
    selected_text_from_frame(frame, state.selection?)
}

fn selected_text_from_frame(snapshot: &TerminalFrame, selection: GridSelection) -> Option<String> {
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

fn text_in_row(frame: &TerminalFrame, row: u16, start_col: u16, end_col: u16) -> String {
    let mut text = String::new();
    let mut next_col = start_col;
    let cols = usize::from(frame.cols);
    let base = usize::from(row) * cols;
    for col in start_col..end_col.min(frame.cols) {
        let Some(cell) = frame.cells.get(base + usize::from(col)) else {
            break;
        };
        // A continuation column carries no cluster of its own.
        let cluster = frame.cell_text(cell);
        if cluster.is_empty() {
            continue;
        }
        if col > next_col {
            text.extend(std::iter::repeat_n(' ', usize::from(col - next_col)));
        }
        text.push_str(cluster);
        next_col = col.saturating_add(u16::from(cell.columns.max(1)));
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
    let (cell_width, line_height) = super::terminal_gpu::cell_size()?;
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

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use lingxia_terminal::{FrameCell, TerminalSearchMatch};

    /// A frame is row-major and its clusters live in one blob, so a fixture
    /// places text by index rather than carrying a coordinate per cell.
    fn frame(cols: u16, rows: u16, placed: &[(u16, u16, &str)]) -> TerminalFrame {
        let mut cells = vec![FrameCell::default(); usize::from(cols) * usize::from(rows)];
        let mut text = String::new();
        for (row, col, cluster) in placed {
            let index = usize::from(*row) * usize::from(cols) + usize::from(*col);
            cells[index] = FrameCell {
                text_offset: text.len() as u32,
                text_len: cluster.len() as u8,
                columns: 1,
                ..FrameCell::default()
            };
            text.push_str(cluster);
        }
        TerminalFrame {
            cols,
            rows,
            cells,
            text,
            ..TerminalFrame::default()
        }
    }

    #[test]
    fn search_highlights_map_absolute_lines_into_viewport() {
        let painted = TerminalFrame {
            cols: 10,
            rows: 3,
            ..Default::default()
        };
        let search = GridSearch {
            matches: vec![
                TerminalSearchMatch {
                    start_line: 5,
                    start_col: 2,
                    end_line: 6,
                    end_col: 4,
                },
                TerminalSearchMatch {
                    start_line: 9,
                    start_col: 0,
                    end_line: 9,
                    end_col: 2,
                },
            ],
            active: Some(0),
        };
        assert_eq!(
            visible_search_highlights(
                &painted,
                Some(TerminalScrollbar {
                    total: 20,
                    offset: 5,
                    len: 3,
                }),
                &search,
            ),
            vec![
                SearchHighlight {
                    row: 0,
                    start_col: 2,
                    end_col: 10,
                    active: true,
                },
                SearchHighlight {
                    row: 1,
                    start_col: 0,
                    end_col: 4,
                    active: true,
                },
            ]
        );
    }

    #[test]
    fn search_navigation_wraps_in_both_directions() {
        assert_eq!(next_search_index(Some(2), 3, 1), Some(0));
        assert_eq!(next_search_index(Some(0), 3, -1), Some(2));
        assert_eq!(next_search_index(None, 0, 1), None);
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
        let painted = frame(
            8,
            2,
            &[
                (0, 1, "a"),
                (0, 2, "b"),
                (0, 5, "c"),
                (1, 0, "d"),
                (1, 1, "e"),
            ],
        );
        let text = selected_text_from_frame(
            &painted,
            GridSelection {
                anchor: GridPoint { row: 0, col: 1 },
                focus: GridPoint { row: 1, col: 2 },
            },
        );
        assert_eq!(text.as_deref(), Some("ab  c\r\nde"));
    }

    #[test]
    fn terminal_cursor_rect_tracks_the_grid_cell() {
        assert_eq!(
            cursor_rect_for_grid(
                RECT {
                    left: 8,
                    top: 20,
                    right: 808,
                    bottom: 420,
                },
                7,
                3,
                80,
                20,
                10,
                20,
            ),
            Some(RECT {
                left: 78,
                top: 80,
                right: 88,
                bottom: 100,
            })
        );
    }
}

/// Record where a panel's body is being drawn, so the PTY can be sized to it
/// before any pane has reported geometry.
pub(super) fn set_panel_body(panel_id: &str, body: RECT) {
    panel_grids().entry(panel_id.to_string()).or_default().body = Some(body);
}

/// Hand one pane's snapshot to the renderer, recording the geometry it is
/// being drawn at so the facade can keep the PTY sized to the rect.
pub(super) fn with_pane(
    session_id: u64,
    rect: RECT,
    cell: (i32, i32),
    draw: impl FnOnce(
        &TerminalFrame,
        &TerminalImageSnapshot,
        PaneView,
        Option<(GridPoint, GridPoint)>,
        &[SearchHighlight],
    ),
) {
    let mut grids = session_grids();
    let state = grids.entry(session_id).or_default();
    state.geometry = Some(GridGeometry {
        cell_width: cell.0.max(1),
        line_height: cell.1.max(1),
        grid_width: (rect.right - rect.left) - 2 * GRID_PADDING,
        grid_height: (rect.bottom - rect.top) - 2 * GRID_PADDING,
    });
    let selection = state.selection.and_then(GridSelection::normalized);
    let view = PaneView {
        scrollbar: state.scrollbar,
        exited: state.exited,
    };
    if let Some(frame) = state.frame.as_ref() {
        let search = visible_search_highlights(frame, state.scrollbar, &state.search);
        draw(frame, &state.images, view, selection, &search);
    }
}
