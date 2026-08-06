//! Terminal panel state: the snapshot store, the grid geometry, and
//! everything the shell hit-tests against.
//!
//! The facade's poll thread pushes full [`TerminalSnapshot`]s through
//! [`set_session_snapshot`] and reads [`desired_session_grid_size`] to keep
//! each pane's PTY sized to its rect. Drawing belongs to the renderer, which
//! takes a pane's snapshot through [`with_pane`] and records the geometry it
//! drew at, so hit-testing and PTY sizing agree with what is on screen.
#![cfg_attr(not(feature = "terminal-runtime"), allow(dead_code))]

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use super::chrome::{inset_rect, rect_height, rect_width};
use lingxia_terminal::TerminalSnapshot;
use windows::Win32::Foundation::{HWND, RECT};

/// Inner padding between the terminal card edge and the cell grid.
pub(super) const GRID_PADDING: i32 = 8;

/// Fallback surface colors, for a pane whose snapshot has not reported the
/// scheme's own yet.
pub(super) const GRID_DEFAULT_BACKGROUND: u32 = 0x282c34;

pub(super) const GRID_DEFAULT_FOREGROUND: u32 = 0xffffff;

/// Dim cells blend the foreground this far toward the background (the
/// macOS surface draws dim text at 0.58 alpha).
pub(super) const GRID_DIM_FOREGROUND_PERCENT: u32 = 58;

/// Minimum grid reported to the PTY, mirroring the macOS surface clamp.
const GRID_MIN_COLS: i32 = 20;

const GRID_MIN_ROWS: i32 = 4;

/// Outline drawn around the pane a dragged pane would land on.
pub(super) const PANE_DROP_TARGET_COLOR: u32 = 0x4b9cff;

/// Windows selection highlight, blended toward each pane's background.
pub(super) const SELECTION_ACCENT: u32 = 0x4b9cff;

pub(super) const SELECTION_ACCENT_PERCENT: u32 = 46;

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

/// Stores the latest snapshot for `session_id`; the chrome painter renders
/// it on the next repaint of the host window.
pub fn set_session_snapshot(session_id: u64, snapshot: TerminalSnapshot) {
    session_grids().entry(session_id).or_default().snapshot = Some(snapshot);
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
    let snapshot = state.snapshot.as_ref()?;
    let geometry = state.geometry?;
    cursor_rect_for_grid(
        inset_rect(frame.rect, GRID_PADDING, GRID_PADDING),
        snapshot.cursor_col,
        snapshot.cursor_row,
        snapshot.cols,
        snapshot.rows,
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

/// Parses the `#rrggbb` color tokens produced by `lingxia-terminal`.
fn parse_hex_color(token: &str) -> Option<u32> {
    let hex = token.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lingxia_terminal::TerminalCell;

    fn cell(row: u16, col: u16, text: &str) -> TerminalCell {
        TerminalCell {
            row,
            col,
            text: text.to_string(),
            ..TerminalCell::default()
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
            scrollbar: None,
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
    draw: impl FnOnce(&TerminalSnapshot, Option<(GridPoint, GridPoint)>),
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
    if let Some(snapshot) = state.snapshot.as_ref() {
        draw(snapshot, selection);
    }
}
