//! Native terminal panel sessions for the Windows shell.
//!
//! Owns all terminal *policy* for the Windows native terminal panel,
//! mirroring the macOS terminal workspace UX:
//!
//! - Multi-tab model: each tab ([`TerminalTab`]) owns a *pane tree* of PTY
//!   sessions ([`PaneNode`]); the panel shows the ACTIVE tab's panes laid
//!   out side by side / stacked, inactive tabs keep running. The tab id
//!   surfaced to the chrome is the tab's focused session id.
//! - Split: the focused pane splits left/right/up/down into two panes
//!   (a fresh PTY session), mirroring the macOS surface context menu.
//! - New tab/split sessions inherit the focused pane's current directory,
//!   matching the macOS terminal workspace rather than the host process cwd.
//! - `exit` closes: a pane whose session exited is removed and its sibling
//!   takes its place; the last pane of a tab closes the tab; closing the
//!   last tab closes the whole panel.
//! - Rename: tab titles default to the focused session's reported title and
//!   can be overridden per tab (inline rename via the shell's EDIT helper).
//! - Maximize: the panel toggles between its dock height and the whole
//!   content area (mechanics live in lingxia-webview's group layout).
//!
//! The webview layer supplies only generic mechanics (panel rects,
//! tab-strip data, chrome events); the shell layer draws the dock, the pane
//! grids, and hosts the inline rename editor.

#[cfg(feature = "terminal-runtime")]
use std::collections::HashMap;
#[cfg(feature = "terminal-runtime")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "terminal-runtime")]
use std::sync::{Arc, Mutex, OnceLock};
#[cfg(feature = "terminal-runtime")]
use std::thread;
#[cfg(feature = "terminal-runtime")]
use std::time::Duration;

#[cfg(feature = "terminal-runtime")]
use lingxia_terminal::TerminalSnapshot;
use lingxia_windows_contract::WindowsPanelPosition;
#[cfg(feature = "terminal-runtime")]
use lingxia_windows_contract::{WindowsHostPanelKeyEvent, WindowsHostPanelTab};
#[cfg(feature = "terminal-runtime")]
use windows::Win32::Foundation::RECT;

#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
static SEARCH_GENERATIONS: OnceLock<Mutex<HashMap<u64, u64>>> = OnceLock::new();

#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn search_generations() -> std::sync::MutexGuard<'static, HashMap<u64, u64>> {
    SEARCH_GENERATIONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Direction a pane splits in, mirroring the macOS surface context menu.
#[cfg(feature = "shell-chrome")]
#[cfg_attr(not(feature = "terminal-runtime"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SplitDir {
    Left,
    Right,
    Up,
    Down,
}

/// Pixel thickness of the gap between two sibling panes - a thin hairline
/// divider in the iTerm style (the grab area is widened separately for drag).
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(super) const PANE_DIVIDER: i32 = 1;

/// How a split node arranges its two children.
#[cfg(feature = "terminal-runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaneOrientation {
    /// Side by side, separated by a vertical divider.
    Cols,
    /// Stacked, separated by a horizontal divider.
    Rows,
}

/// A tab's pane layout: either a single PTY session (leaf) or a split of
/// two child trees sharing the space by `ratio` (the first child's share).
#[cfg(feature = "terminal-runtime")]
enum PaneNode {
    Leaf(u64),
    Split {
        orient: PaneOrientation,
        /// First child's fraction of the long axis, in `0.05..=0.95`.
        ratio: f32,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

/// Lock-free copy of a pane tree used while assembling an automation
/// snapshot. Grid state has its own mutex, so carrying the live tree across
/// that lookup would invert the renderer's lock order.
#[cfg(feature = "terminal-runtime")]
#[derive(Clone)]
enum AutomationPaneNode {
    Leaf(u64),
    Split {
        orient: PaneOrientation,
        first: Box<AutomationPaneNode>,
        second: Box<AutomationPaneNode>,
    },
}

#[cfg(feature = "terminal-runtime")]
impl From<&PaneNode> for AutomationPaneNode {
    fn from(value: &PaneNode) -> Self {
        match value {
            PaneNode::Leaf(id) => Self::Leaf(*id),
            PaneNode::Split {
                orient,
                first,
                second,
                ..
            } => Self::Split {
                orient: *orient,
                first: Box::new(Self::from(first.as_ref())),
                second: Box::new(Self::from(second.as_ref())),
            },
        }
    }
}

#[cfg(feature = "terminal-runtime")]
struct AutomationTabState {
    id: u64,
    focused: u64,
    active: bool,
    tree: AutomationPaneNode,
}

#[cfg(feature = "terminal-runtime")]
impl PaneNode {
    /// Collects every leaf session id, left-to-right / top-to-bottom.
    fn collect(&self, out: &mut Vec<u64>) {
        match self {
            PaneNode::Leaf(id) => out.push(*id),
            PaneNode::Split { first, second, .. } => {
                first.collect(out);
                second.collect(out);
            }
        }
    }

    /// First leaf session id in layout order (focus fallback after a close).
    fn first_leaf(&self) -> u64 {
        match self {
            PaneNode::Leaf(id) => *id,
            PaneNode::Split { first, .. } => first.first_leaf(),
        }
    }

    /// Replaces the leaf `target` with a split adding `new_id` on the
    /// `dir` side. Returns `true` once the target leaf was found.
    #[cfg(feature = "shell-chrome")]
    fn split(&mut self, target: u64, new_id: u64, dir: SplitDir) -> bool {
        match self {
            PaneNode::Leaf(id) if *id == target => {
                let orient = match dir {
                    SplitDir::Left | SplitDir::Right => PaneOrientation::Cols,
                    SplitDir::Up | SplitDir::Down => PaneOrientation::Rows,
                };
                let existing = PaneNode::Leaf(*id);
                let fresh = PaneNode::Leaf(new_id);
                let (first, second) = match dir {
                    SplitDir::Right | SplitDir::Down => (existing, fresh),
                    SplitDir::Left | SplitDir::Up => (fresh, existing),
                };
                *self = PaneNode::Split {
                    orient,
                    ratio: 0.5,
                    first: Box::new(first),
                    second: Box::new(second),
                };
                true
            }
            PaneNode::Leaf(_) => false,
            PaneNode::Split { first, second, .. } => {
                first.split(target, new_id, dir) || second.split(target, new_id, dir)
            }
        }
    }

    /// Moves an existing leaf beside `target` without replacing its PTY
    /// session. The source is first collapsed out of its old parent, then
    /// inserted through the same split path used for a fresh pane.
    #[cfg(feature = "shell-chrome")]
    fn move_leaf(&mut self, source: u64, target: u64, dir: SplitDir) -> bool {
        if source == target {
            return false;
        }
        let mut sessions = Vec::new();
        self.collect(&mut sessions);
        if !sessions.contains(&source) || !sessions.contains(&target) {
            return false;
        }

        // A valid move always leaves at least the target leaf behind.
        let Some(mut root) = remove_leaf(std::mem::replace(self, PaneNode::Leaf(source)), source)
        else {
            return false;
        };
        let moved = root.split(target, source, dir);
        *self = root;
        moved
    }
}

/// Removes leaf `target` from `node`, collapsing the parent split into the
/// surviving sibling. Returns `None` when the whole tree was the target
/// (i.e. the tab is now empty).
#[cfg(feature = "terminal-runtime")]
fn remove_leaf(node: PaneNode, target: u64) -> Option<PaneNode> {
    match node {
        PaneNode::Leaf(id) => (id != target).then_some(PaneNode::Leaf(id)),
        PaneNode::Split {
            orient,
            ratio,
            first,
            second,
        } => match (remove_leaf(*first, target), remove_leaf(*second, target)) {
            (Some(first), Some(second)) => Some(PaneNode::Split {
                orient,
                ratio,
                first: Box::new(first),
                second: Box::new(second),
            }),
            (Some(only), None) | (None, Some(only)) => Some(only),
            (None, None) => None,
        },
    }
}

/// Lays `node` out within `rect`, pushing `(session_id, rect)` for each
/// leaf. Sibling panes are separated by a [`PANE_DIVIDER`]-wide gap.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn layout_node(node: &PaneNode, rect: RECT, out: &mut Vec<(u64, RECT)>) {
    match node {
        PaneNode::Leaf(id) => out.push((*id, rect)),
        PaneNode::Split {
            orient,
            ratio,
            first,
            second,
        } => {
            let (r1, r2) = split_rect(rect, *orient, *ratio);
            layout_node(first, r1, out);
            layout_node(second, r2, out);
        }
    }
}

/// Splits `rect` into the two child rects of a split node, reserving the
/// divider gap between them.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn split_rect(rect: RECT, orient: PaneOrientation, ratio: f32) -> (RECT, RECT) {
    let ratio = ratio.clamp(0.05, 0.95);
    match orient {
        PaneOrientation::Cols => {
            let usable = (rect.right - rect.left - PANE_DIVIDER).max(0);
            let first_w = (usable as f32 * ratio).round() as i32;
            let mid = rect.left + first_w;
            (
                RECT { right: mid, ..rect },
                RECT {
                    left: mid + PANE_DIVIDER,
                    ..rect
                },
            )
        }
        PaneOrientation::Rows => {
            let usable = (rect.bottom - rect.top - PANE_DIVIDER).max(0);
            let first_h = (usable as f32 * ratio).round() as i32;
            let mid = rect.top + first_h;
            (
                RECT {
                    bottom: mid,
                    ..rect
                },
                RECT {
                    top: mid + PANE_DIVIDER,
                    ..rect
                },
            )
        }
    }
}

/// One terminal tab: a pane tree plus its title state. `auto_title` tracks
/// the focused session's reported title; `custom_title` (set by rename)
/// wins when present.
#[cfg(feature = "terminal-runtime")]
struct TerminalTab {
    root: PaneNode,
    /// Session id of the focused pane (receives input and pane actions).
    focused: u64,
    custom_title: Option<String>,
    auto_title: String,
}

#[cfg(feature = "terminal-runtime")]
impl TerminalTab {
    fn new(session_id: u64) -> Self {
        Self {
            root: PaneNode::Leaf(session_id),
            focused: session_id,
            custom_title: None,
            auto_title: String::new(),
        }
    }

    fn sessions(&self) -> Vec<u64> {
        let mut out = Vec::new();
        self.root.collect(&mut out);
        out
    }

    fn display_title(&self) -> &str {
        self.custom_title
            .as_deref()
            .filter(|title| !title.is_empty())
            .or_else(|| Some(self.auto_title.as_str()).filter(|title| !title.is_empty()))
            .unwrap_or("terminal")
    }
}

#[cfg(feature = "terminal-runtime")]
struct WindowsTerminalPanel {
    tabs: Vec<TerminalTab>,
    /// Index of the active tab in `tabs`.
    active: usize,
    maximized: bool,
    /// The layout's last word on whether this panel fills the content area.
    /// A re-present carrying the same value is incidental — a title change
    /// syncing the shell — and must not overwrite the user's own toggle; a
    /// different value is the layout moving the panel between roles, which
    /// the user's toggle does not get to veto.
    projected_maximized: Option<bool>,
    /// When set, keyboard input to the panel's panes is dropped (the menu's
    /// "Terminal Read-only" toggle), mirroring the macOS surface.
    read_only: bool,
    /// Stop flag of the panel's poll thread.
    stop: Arc<AtomicBool>,
    /// Tab strip last pushed to the webview layer (pushed only on change).
    published_tabs: Vec<WindowsHostPanelTab>,
}

#[cfg(feature = "terminal-runtime")]
impl WindowsTerminalPanel {
    fn active_tab(&self) -> Option<&TerminalTab> {
        self.tabs.get(self.active)
    }

    fn active_tab_mut(&mut self) -> Option<&mut TerminalTab> {
        self.tabs.get_mut(self.active)
    }
}

#[cfg(feature = "terminal-runtime")]
static WINDOWS_TERMINAL_PANELS: OnceLock<Mutex<HashMap<String, WindowsTerminalPanel>>> =
    OnceLock::new();

#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
static TERMINAL_WHEEL_REMAINDERS: OnceLock<Mutex<HashMap<u64, i32>>> = OnceLock::new();

#[cfg(feature = "terminal-runtime")]
fn windows_terminal_panels() -> std::sync::MutexGuard<'static, HashMap<String, WindowsTerminalPanel>>
{
    WINDOWS_TERMINAL_PANELS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        // The registry map has no invariants that poisoning can break.
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(super) fn open_windows_terminal_panel(
    panel_id: &str,
    title: &str,
    position: WindowsPanelPosition,
) -> Result<(), String> {
    #[cfg(feature = "terminal-runtime")]
    if lingxia_terminal::backend_available() {
        return open_windows_terminal_session_panel(panel_id, title, position);
    }
    lingxia_windows_contract::show_interactive_host_panel(
        panel_id,
        title,
        terminal_panel_status_text(),
        position,
    )
    .map_err(|err| err.to_string())
}

pub(super) fn show_existing_windows_terminal_panel(
    panel_id: &str,
    title: &str,
    position: WindowsPanelPosition,
) -> Result<bool, String> {
    #[cfg(feature = "terminal-runtime")]
    {
        if !windows_terminal_panels().contains_key(panel_id) {
            return Ok(false);
        }
        let body = super::terminal_grid::panel_snapshot_text(panel_id)
            .filter(|body| !body.trim().is_empty())
            .unwrap_or_else(|| "Terminal session started".to_string());
        lingxia_windows_contract::show_interactive_host_panel(panel_id, title, &body, position)
            .map_err(|err| err.to_string())?;
        publish_tab_strip(panel_id);
        publish_active_snapshot(panel_id);
        Ok(true)
    }
    #[cfg(not(feature = "terminal-runtime"))]
    {
        let _ = (panel_id, title, position);
        Ok(false)
    }
}

fn terminal_panel_status_text() -> &'static str {
    #[cfg(feature = "terminal-runtime")]
    {
        if lingxia_terminal::backend_available() {
            "Terminal session is waiting for output"
        } else {
            "Terminal runtime is not available"
        }
    }
    #[cfg(not(feature = "terminal-runtime"))]
    {
        "Terminal runtime is disabled"
    }
}

// ---- Chrome-event entry points (called from the shell facade). All are
// no-ops without the terminal runtime; the chrome only emits these events
// for terminal panels this module opened. ----

/// Makes `tab_id` the panel's active tab and shows its panes.
pub(super) fn activate_terminal_tab(panel_id: &str, tab_id: u64) {
    #[cfg(feature = "terminal-runtime")]
    {
        {
            let mut panels = windows_terminal_panels();
            let Some(panel) = panels.get_mut(panel_id) else {
                return;
            };
            let Some(index) = panel
                .tabs
                .iter()
                .position(|tab| tab.sessions().contains(&tab_id))
            else {
                return;
            };
            panel.active = index;
        }
        publish_tab_strip(panel_id);
        publish_active_snapshot(panel_id);
    }
    #[cfg(not(feature = "terminal-runtime"))]
    let _ = (panel_id, tab_id);
}

/// Closes `tab_id`'s tab (terminating every pane session in it) and
/// activates a neighbor; closing the last tab closes the whole panel.
pub(super) fn close_terminal_tab(panel_id: &str, tab_id: u64) {
    #[cfg(feature = "terminal-runtime")]
    {
        // `tab_id` is a session id; find the tab that owns it.
        let session_ids = {
            let panels = windows_terminal_panels();
            let Some(panel) = panels.get(panel_id) else {
                return;
            };
            let Some(tab) = panel
                .tabs
                .iter()
                .find(|tab| tab.sessions().contains(&tab_id))
            else {
                return;
            };
            tab.sessions()
        };
        close_terminal_tab_by_sessions(panel_id, &session_ids);
    }
    #[cfg(not(feature = "terminal-runtime"))]
    let _ = (panel_id, tab_id);
}

/// Creates a new session/tab in the panel and activates it.
pub(super) fn open_terminal_tab(panel_id: &str) {
    #[cfg(feature = "terminal-runtime")]
    {
        let session_id = create_panel_session(panel_id, active_session_id(panel_id));
        if session_id == 0 {
            log::warn!("failed to create terminal session for new tab in {panel_id}");
            return;
        }
        {
            let mut panels = windows_terminal_panels();
            let Some(panel) = panels.get_mut(panel_id) else {
                lingxia_terminal::terminal_close(session_id);
                return;
            };
            panel.tabs.push(TerminalTab::new(session_id));
            panel.active = panel.tabs.len() - 1;
        }
        publish_tab_strip(panel_id);
        publish_active_snapshot(panel_id);
    }
    #[cfg(not(feature = "terminal-runtime"))]
    let _ = panel_id;
}

/// Splits the active tab's focused pane in `dir`, creating a fresh session
/// for the new pane and focusing it.
#[cfg_attr(
    not(all(feature = "terminal-runtime", feature = "shell-chrome")),
    allow(dead_code)
)]
pub(super) fn split_focused_pane(panel_id: &str, dir: SplitDir) -> bool {
    #[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
    {
        let session_id = create_panel_session(panel_id, active_session_id(panel_id));
        if session_id == 0 {
            log::warn!("failed to create terminal session to split pane in {panel_id}");
            return false;
        }
        let split = {
            let mut panels = windows_terminal_panels();
            let Some(tab) = panels
                .get_mut(panel_id)
                .and_then(|panel| panel.active_tab_mut())
            else {
                lingxia_terminal::terminal_close(session_id);
                return false;
            };
            let target = tab.focused;
            if tab.root.split(target, session_id, dir) {
                tab.focused = session_id;
                true
            } else {
                false
            }
        };
        if split {
            publish_tab_strip(panel_id);
            publish_active_snapshot(panel_id);
        } else {
            lingxia_terminal::terminal_close(session_id);
        }
        split
    }
    #[cfg(not(all(feature = "terminal-runtime", feature = "shell-chrome")))]
    {
        let _ = (panel_id, dir);
        false
    }
}

/// Closes the active tab's focused pane; its sibling takes the space. When
/// it was the tab's last pane the tab closes (and the panel, if last tab).
#[cfg_attr(
    not(all(feature = "terminal-runtime", feature = "shell-chrome")),
    allow(dead_code)
)]
pub(super) fn close_focused_pane(panel_id: &str) {
    #[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
    {
        let focused = {
            let panels = windows_terminal_panels();
            panels
                .get(panel_id)
                .and_then(|panel| panel.active_tab())
                .map(|tab| tab.focused)
        };
        if let Some(focused) = focused {
            close_pane_session(panel_id, focused);
        }
    }
    #[cfg(not(all(feature = "terminal-runtime", feature = "shell-chrome")))]
    let _ = panel_id;
}

/// Focuses the pane under `(client_x, client_y)` (host-window client
/// coordinates) in the active tab, if a pane covers that point.
#[cfg_attr(
    not(all(feature = "terminal-runtime", feature = "shell-chrome")),
    allow(dead_code)
)]
pub(super) fn focus_pane_at(panel_id: &str, client_x: i32, client_y: i32) {
    #[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
    {
        let body = super::terminal_grid::panel_body_rect(panel_id);
        let Some(body) = body else {
            return;
        };
        let changed = {
            let mut panels = windows_terminal_panels();
            let Some(panel) = panels.get_mut(panel_id) else {
                return;
            };
            let Some(tab) = panel.active_tab_mut() else {
                return;
            };
            let mut frames = Vec::new();
            layout_node(&tab.root, body, &mut frames);
            let hit = frames.iter().find(|(_, rect)| {
                client_x >= rect.left
                    && client_x < rect.right
                    && client_y >= rect.top
                    && client_y < rect.bottom
            });
            match hit {
                Some((session_id, _)) if *session_id != tab.focused => {
                    tab.focused = *session_id;
                    true
                }
                _ => false,
            }
        };
        if changed {
            publish_active_snapshot(panel_id);
        }
    }
    #[cfg(not(all(feature = "terminal-runtime", feature = "shell-chrome")))]
    let _ = (panel_id, client_x, client_y);
}

/// Scrolls the pane under a host-client point. Wheel gestures follow the
/// pointer instead of whichever split pane owns keyboard focus.
#[cfg_attr(
    not(all(feature = "terminal-runtime", feature = "shell-chrome")),
    allow(dead_code)
)]
pub(crate) fn scroll_pane_at(
    panel_id: &str,
    client_x: i32,
    client_y: i32,
    wheel_delta: i32,
) -> bool {
    #[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
    {
        if wheel_delta == 0 {
            return false;
        }
        let Some((session_id, col, row)) =
            super::terminal_grid::session_cell_at(panel_id, client_x, client_y)
        else {
            return false;
        };

        const WHEEL_DELTA: i32 = 120;
        const ROWS_PER_NOTCH: i32 = 3;
        let delta_rows = {
            let remainders = TERMINAL_WHEEL_REMAINDERS.get_or_init(|| Mutex::new(HashMap::new()));
            let mut remainders = remainders
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let remainder = remainders.entry(session_id).or_default();
            *remainder += wheel_delta.saturating_mul(ROWS_PER_NOTCH);
            let rows = *remainder / WHEEL_DELTA;
            *remainder %= WHEEL_DELTA;
            if *remainder == 0 {
                remainders.remove(&session_id);
            }
            rows
        };
        if delta_rows == 0 {
            return true;
        }
        let allow_application_input = !is_panel_read_only(panel_id);
        if !lingxia_terminal::terminal_scroll(
            session_id,
            -delta_rows,
            col,
            row,
            allow_application_input,
        ) {
            return false;
        }
        super::terminal_grid::reveal_scrollbar(session_id);
        super::terminal_grid::clear_selection(session_id);
        publish_session_frame(panel_id, session_id);
        invalidate_panel(panel_id);
        true
    }
    #[cfg(not(all(feature = "terminal-runtime", feature = "shell-chrome")))]
    {
        let _ = (panel_id, client_x, client_y, wheel_delta);
        false
    }
}

pub(crate) fn begin_terminal_selection(panel_id: &str, client_x: i32, client_y: i32) -> bool {
    #[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
    {
        let started = super::terminal_grid::begin_selection_at(panel_id, client_x, client_y);
        if started {
            invalidate_panel(panel_id);
        }
        started
    }
    #[cfg(not(all(feature = "terminal-runtime", feature = "shell-chrome")))]
    {
        let _ = (panel_id, client_x, client_y);
        false
    }
}

pub(crate) fn update_terminal_selection(panel_id: &str, client_x: i32, client_y: i32) -> bool {
    #[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
    {
        let updated = super::terminal_grid::update_selection_at(panel_id, client_x, client_y);
        if updated {
            invalidate_panel(panel_id);
        }
        updated
    }
    #[cfg(not(all(feature = "terminal-runtime", feature = "shell-chrome")))]
    {
        let _ = (panel_id, client_x, client_y);
        false
    }
}

pub(crate) fn end_terminal_selection(panel_id: &str) -> bool {
    #[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
    {
        let ended = super::terminal_grid::end_selection(panel_id);
        if ended {
            invalidate_panel(panel_id);
        }
        ended
    }
    #[cfg(not(all(feature = "terminal-runtime", feature = "shell-chrome")))]
    {
        let _ = panel_id;
        false
    }
}

/// Whether the panel is currently expanded to the full content area, or
/// `None` when no terminal panel owns `panel_id`.
#[cfg_attr(not(feature = "terminal-runtime"), allow(dead_code))]
pub(super) fn terminal_panel_maximized(panel_id: &str) -> Option<bool> {
    #[cfg(feature = "terminal-runtime")]
    {
        windows_terminal_panels()
            .get(panel_id)
            .map(|panel| panel.maximized)
    }
    #[cfg(not(feature = "terminal-runtime"))]
    {
        let _ = panel_id;
        None
    }
}

/// Toggles the panel between its dock height and the full content area.
pub(super) fn toggle_terminal_panel_maximized(panel_id: &str) {
    #[cfg(feature = "terminal-runtime")]
    {
        let next = {
            let panels = windows_terminal_panels();
            let Some(panel) = panels.get(panel_id) else {
                return;
            };
            !panel.maximized
        };
        set_terminal_panel_maximized_by_user(panel_id, next);
    }
    #[cfg(not(feature = "terminal-runtime"))]
    let _ = panel_id;
}

/// The user's own choice, from the chrome toggle or an automation driver. It
/// overrides the current state outright and survives a repeated layout
/// projection; only the layout moving the panel between roles replaces it.
#[cfg_attr(not(feature = "terminal-runtime"), allow(dead_code))]
pub(super) fn set_terminal_panel_maximized_by_user(panel_id: &str, maximized: bool) {
    #[cfg(feature = "terminal-runtime")]
    {
        {
            let mut panels = windows_terminal_panels();
            let Some(panel) = panels.get_mut(panel_id) else {
                return;
            };
            panel.maximized = maximized;
        }
        lingxia_windows_contract::set_host_panel_maximized(panel_id, maximized);
    }
    #[cfg(not(feature = "terminal-runtime"))]
    let _ = (panel_id, maximized);
}

/// Applies the shared layout projection without treating it as a user toggle.
///
/// A layout sync runs for ordinary reasons — opening a tab renames the active
/// tab, which syncs the shell — so repeating the projection the panel already
/// has must leave the user's own toggle alone. A *changed* projection is the
/// layout moving the panel between roles, and that wins.
pub(super) fn set_terminal_panel_maximized(panel_id: &str, maximized: bool) {
    #[cfg(feature = "terminal-runtime")]
    {
        let maximized = {
            let mut panels = windows_terminal_panels();
            let Some(panel) = panels.get_mut(panel_id) else {
                return;
            };
            let unchanged = panel.projected_maximized == Some(maximized);
            panel.projected_maximized = Some(maximized);
            if !unchanged {
                panel.maximized = maximized;
            }
            panel.maximized
        };
        // Showing an existing panel recreates its host entry with the default
        // docked state. Reapply the projection even when the terminal registry
        // already held the requested value.
        lingxia_windows_contract::set_host_panel_maximized(panel_id, maximized);
    }
    #[cfg(not(feature = "terminal-runtime"))]
    let _ = (panel_id, maximized);
}

/// Starts an inline rename of `tab_id`'s title (shell EDIT helper over the
/// painted title rect). Committing a non-empty text sets the tab's custom
/// title; committing an empty text reverts to the automatic title.
pub(super) fn begin_terminal_tab_rename(panel_id: &str, tab_id: u64) {
    #[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
    {
        let current = {
            let panels = windows_terminal_panels();
            let Some(title) = panels.get(panel_id).and_then(|panel| {
                panel
                    .tabs
                    .iter()
                    .find(|tab| tab.sessions().contains(&tab_id))
                    .map(|tab| tab.display_title().to_string())
            }) else {
                return;
            };
            title
        };
        let panel_key = panel_id.to_string();
        super::terminal_grid::begin_tab_rename(
            panel_id,
            tab_id,
            &current,
            Arc::new(move |text: String| {
                set_terminal_tab_custom_title(&panel_key, tab_id, &text);
            }),
        );
    }
    #[cfg(not(all(feature = "terminal-runtime", feature = "shell-chrome")))]
    let _ = (panel_id, tab_id);
}

/// Shows the terminal context menu at the given screen point in response to
/// a right-click on the panel. The right-clicked pane is focused first so
/// the menu's pane actions (split / close) target it.
pub(super) fn show_terminal_context_menu(
    owner_appid: &str,
    panel_id: &str,
    screen_x: i32,
    screen_y: i32,
) {
    #[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
    {
        let Some(window) = super::runtime::owner_window_handle(owner_appid) else {
            return;
        };
        // Focus the pane under the cursor (the screen point maps to client
        // coordinates via the host window) so split/close act on it.
        if let Some((cx, cy)) =
            super::runtime::screen_to_panel_client(owner_appid, screen_x, screen_y)
        {
            focus_pane_at(panel_id, cx, cy);
        }
        let panel_key = panel_id.to_string();
        let multi_pane = pane_count(panel_id) > 1;
        use lingxia_logic::I18nKey;
        use lingxia_logic::i18n::t;
        // Item order mirrors the macOS surface context menu.
        let mut items = vec![t(I18nKey::TerminalCopy), t(I18nKey::TerminalPaste)];
        items.push(format!("{}\tCtrl+F", t(I18nKey::TerminalSearch)));
        items.push(String::new()); // separator marker
        items.push(t(I18nKey::TerminalSplitRight));
        items.push(t(I18nKey::TerminalSplitLeft));
        items.push(t(I18nKey::TerminalSplitDown));
        items.push(t(I18nKey::TerminalSplitUp));
        items.push(String::new());
        items.push(t(I18nKey::TerminalNewTab));
        if multi_pane {
            items.push(t(I18nKey::TerminalClosePane));
        }
        items.push(t(I18nKey::TerminalChangeTitle));
        items.push(t(I18nKey::TerminalReset));
        let read_only_index = items.len();
        items.push(t(I18nKey::TerminalReadOnly));
        let mut checked = vec![false; items.len()];
        checked[read_only_index] = is_panel_read_only(panel_id);
        super::context_menu::show_context_menu_checked(
            window,
            (screen_x, screen_y),
            items.clone(),
            checked,
            Arc::new(move |index| {
                // Map back through the same label list so the indices stay
                // in sync with the (locale-dependent) items above.
                handle_context_menu_choice(&panel_key, &items, index);
            }),
        );
    }
    #[cfg(not(all(feature = "terminal-runtime", feature = "shell-chrome")))]
    let _ = (owner_appid, panel_id, screen_x, screen_y);
}

/// Dispatches a context-menu selection by matching the chosen label
/// against the localized item list (separators are empty strings).
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn handle_context_menu_choice(panel_id: &str, items: &[String], index: usize) {
    use lingxia_logic::I18nKey;
    use lingxia_logic::i18n::t;
    let Some(label) = items.get(index) else {
        return;
    };
    if label.is_empty() {
        return;
    }
    let label = label.as_str();
    if label == t(I18nKey::TerminalCopy) {
        copy_panel_screen_to_clipboard(panel_id);
    } else if label == t(I18nKey::TerminalPaste) {
        paste_clipboard_into_panel(panel_id);
    } else if label == format!("{}\tCtrl+F", t(I18nKey::TerminalSearch)) {
        begin_terminal_search(panel_id);
    } else if label == t(I18nKey::TerminalSplitRight) {
        split_focused_pane(panel_id, SplitDir::Right);
    } else if label == t(I18nKey::TerminalSplitLeft) {
        split_focused_pane(panel_id, SplitDir::Left);
    } else if label == t(I18nKey::TerminalSplitDown) {
        split_focused_pane(panel_id, SplitDir::Down);
    } else if label == t(I18nKey::TerminalSplitUp) {
        split_focused_pane(panel_id, SplitDir::Up);
    } else if label == t(I18nKey::TerminalNewTab) {
        open_terminal_tab(panel_id);
    } else if label == t(I18nKey::TerminalClosePane) {
        close_focused_pane(panel_id);
    } else if label == t(I18nKey::TerminalChangeTitle) {
        begin_focused_tab_rename(panel_id);
    } else if label == t(I18nKey::TerminalReset) {
        reset_focused_pane(panel_id);
    } else if label == t(I18nKey::TerminalReadOnly) {
        toggle_read_only(panel_id);
    }
}

/// Whether the panel currently drops keyboard input (read-only).
#[cfg(feature = "terminal-runtime")]
fn is_panel_read_only(panel_id: &str) -> bool {
    windows_terminal_panels()
        .get(panel_id)
        .map(|panel| panel.read_only)
        .unwrap_or(false)
}

/// Toggles the panel's read-only state (the menu's "Terminal Read-only").
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn toggle_read_only(panel_id: &str) {
    let mut panels = windows_terminal_panels();
    if let Some(panel) = panels.get_mut(panel_id) {
        panel.read_only = !panel.read_only;
    }
}

/// Begins an inline rename of the active tab (the focused session id is the
/// tab id surfaced to the chrome).
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn begin_focused_tab_rename(panel_id: &str) {
    let focused = {
        let panels = windows_terminal_panels();
        panels
            .get(panel_id)
            .and_then(|panel| panel.active_tab())
            .map(|tab| tab.focused)
    };
    if let Some(focused) = focused {
        begin_terminal_tab_rename(panel_id, focused);
    }
}

/// Replaces leaf `target` with `replacement` everywhere in the tree,
/// keeping the pane layout (used by Reset Terminal).
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn replace_leaf(node: PaneNode, target: u64, replacement: u64) -> PaneNode {
    match node {
        PaneNode::Leaf(id) => PaneNode::Leaf(if id == target { replacement } else { id }),
        PaneNode::Split {
            orient,
            ratio,
            first,
            second,
        } => PaneNode::Split {
            orient,
            ratio,
            first: Box::new(replace_leaf(*first, target, replacement)),
            second: Box::new(replace_leaf(*second, target, replacement)),
        },
    }
}

/// Restarts the focused pane's PTY session in place: the old session is
/// closed and a fresh one takes its leaf, keeping the pane layout.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn reset_focused_pane(panel_id: &str) {
    let old = {
        let panels = windows_terminal_panels();
        panels
            .get(panel_id)
            .and_then(|panel| panel.active_tab())
            .map(|tab| tab.focused)
    };
    let Some(old) = old else {
        return;
    };
    let fresh = create_panel_session(panel_id, None);
    if fresh == 0 {
        log::warn!("failed to create replacement session resetting pane in {panel_id}");
        return;
    }
    let replaced = {
        let mut panels = windows_terminal_panels();
        match panels
            .get_mut(panel_id)
            .and_then(|panel| panel.active_tab_mut())
        {
            Some(tab) => {
                let root = std::mem::replace(&mut tab.root, PaneNode::Leaf(0));
                tab.root = replace_leaf(root, old, fresh);
                if tab.focused == old {
                    tab.focused = fresh;
                }
                true
            }
            None => false,
        }
    };
    if replaced {
        super::terminal_grid::clear_session(old);
        lingxia_terminal::terminal_close(old);
        publish_tab_strip(panel_id);
        publish_active_snapshot(panel_id);
    } else {
        lingxia_terminal::terminal_close(fresh);
    }
}

/// Copies the focused pane's selected cells to the clipboard.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn copy_panel_screen_to_clipboard(panel_id: &str) {
    let Some(session_id) = active_session_id(panel_id) else {
        return;
    };
    let Some(text) = super::terminal_grid::selected_text(session_id) else {
        return;
    };
    super::clipboard::set_clipboard_text(&text);
}

#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn clear_panel_selection(panel_id: &str, session_id: u64) {
    super::terminal_grid::clear_selection(session_id);
    invalidate_panel(panel_id);
}

/// Pastes the clipboard text into the panel's focused session (the context
/// menu's Paste). CRLF/LF normalize to CR, and the text is wrapped in
/// bracketed-paste escapes when the session requests it.
#[cfg_attr(
    not(all(feature = "terminal-runtime", feature = "shell-chrome")),
    allow(dead_code)
)]
pub(super) fn paste_clipboard_into_panel(panel_id: &str) {
    #[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
    {
        if is_panel_read_only(panel_id) {
            return;
        }
        let Some(session_id) = active_session_id(panel_id) else {
            return;
        };
        let Some(text) = super::clipboard::clipboard_text() else {
            return;
        };
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        let bracketed = lingxia_terminal::terminal_snapshot_data(session_id)
            .is_some_and(|snapshot| snapshot.bracketed_paste && snapshot.alternate_screen);
        let payload = if bracketed {
            format!("\x1b[200~{text}\x1b[201~")
        } else {
            text
        };
        if lingxia_terminal::terminal_write(session_id, &payload) {
            clear_panel_selection(panel_id, session_id);
        }
    }
    #[cfg(not(all(feature = "terminal-runtime", feature = "shell-chrome")))]
    let _ = panel_id;
}

/// Rename commit: a non-empty text becomes the tab's custom title; empty
/// reverts to the automatic (session-reported) title. Only reachable from
/// the inline rename editor, which needs the shell chrome.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn set_terminal_tab_custom_title(panel_id: &str, tab_id: u64, text: &str) {
    {
        let mut panels = windows_terminal_panels();
        let Some(tab) = panels.get_mut(panel_id).and_then(|panel| {
            panel
                .tabs
                .iter_mut()
                .find(|tab| tab.sessions().contains(&tab_id))
        }) else {
            return;
        };
        let trimmed = text.trim();
        tab.custom_title = (!trimmed.is_empty()).then(|| trimmed.to_string());
    }
    publish_tab_strip(panel_id);
}

// ---- Session lifecycle ----

/// Creates a PTY session sized to the panel's current grid. When
/// `inherit_from` names a live session, its current directory becomes the
/// new shell's initial directory.
#[cfg(feature = "terminal-runtime")]
fn create_panel_session(panel_id: &str, inherit_from: Option<u64>) -> u64 {
    let _ = panel_id;
    #[cfg(feature = "shell-chrome")]
    let (cols, rows) = super::terminal_grid::desired_panel_grid_size(panel_id).unwrap_or((100, 24));
    #[cfg(not(feature = "shell-chrome"))]
    let (cols, rows) = (100, 24);
    let cwd = inherit_from.and_then(lingxia_terminal::terminal_current_directory);
    ensure_configuration_loaded();
    lingxia_terminal::terminal_create_with_spec(
        cols,
        rows,
        lingxia_terminal::TerminalSessionSpec {
            cwd,
            ..lingxia_terminal::TerminalSessionSpec::default()
        },
    )
}

/// Load `terminal.json` once and apply the configuration in effect. Later
/// changes arrive through the settings API and bump the shared generation.
#[cfg(feature = "terminal-runtime")]
pub(super) fn ensure_configuration_loaded() {
    use std::sync::OnceLock;
    static LOADED: OnceLock<()> = OnceLock::new();
    if LOADED.set(()).is_err() {
        return;
    }
    lingxia::terminal::set_installed_fonts(crate::terminal_fonts::installed_fonts());
    let _ = lingxia::terminal::load_for_app(system_prefers_dark());
}

/// Windows' light/dark preference, as the shell chrome already reads it.
#[cfg(feature = "terminal-runtime")]
fn system_prefers_dark() -> bool {
    super::theme::is_dark()
}

#[cfg(feature = "terminal-runtime")]
fn open_windows_terminal_session_panel(
    panel_id: &str,
    title: &str,
    position: WindowsPanelPosition,
) -> Result<(), String> {
    shutdown_windows_terminal_panel_state(panel_id);
    let session_id = create_panel_session(panel_id, None);
    if session_id == 0 {
        return lingxia_windows_contract::show_interactive_host_panel(
            panel_id,
            title,
            "Terminal failed to start",
            position,
        )
        .map_err(|err| err.to_string());
    }

    let initial_snapshot = lingxia_terminal::terminal_snapshot_data(session_id);
    let initial_body = initial_snapshot
        .as_ref()
        .map(windows_terminal_snapshot_body)
        .filter(|body| !body.trim().is_empty())
        .unwrap_or_else(|| "Terminal session started".to_string());

    #[cfg(feature = "shell-chrome")]
    super::terminal_gpu::activate_panel(panel_id);
    if let Err(err) = lingxia_windows_contract::show_interactive_host_panel(
        panel_id,
        title,
        &initial_body,
        position,
    ) {
        #[cfg(feature = "shell-chrome")]
        super::terminal_gpu::drop_panel(panel_id);
        lingxia_terminal::terminal_close(session_id);
        return Err(err.to_string());
    }

    // The webview layer forwards structured key events; terminal escape-
    // sequence knowledge lives in lingxia-terminal's encoder. Input always
    // routes to the active tab's FOCUSED pane session at event time.
    let input_panel_key = panel_id.to_string();
    lingxia_windows_contract::set_host_panel_input_handler(
        panel_id,
        Arc::new(move |event: WindowsHostPanelKeyEvent| {
            #[cfg(feature = "shell-chrome")]
            if event.ctrl && !event.shift && !event.alt && event.vk == 0x46 {
                begin_terminal_search(&input_panel_key);
                return true;
            }
            #[cfg(feature = "shell-chrome")]
            if event.ctrl && event.vk == 0 && matches!(event.character, Some('\u{6}' | 'F' | 'f')) {
                return true;
            }
            #[cfg(feature = "shell-chrome")]
            if event.ctrl && event.shift && event.vk == 0x43 {
                copy_panel_screen_to_clipboard(&input_panel_key);
                return true;
            }
            // TranslateMessage posts WM_CHAR before WM_KEYDOWN is dispatched,
            // so consuming Ctrl+Shift+C/V above does not suppress the trailing
            // control character. Swallow it or copy would also send ETX and
            // paste would also send SYN to the foreground process.
            #[cfg(feature = "shell-chrome")]
            if event.ctrl
                && event.shift
                && event.vk == 0
                && matches!(
                    event.character,
                    Some('\u{3}' | '\u{16}' | 'C' | 'V' | 'c' | 'v')
                )
            {
                return true;
            }
            if is_panel_read_only(&input_panel_key) {
                return false;
            }
            #[cfg(feature = "shell-chrome")]
            if event.ctrl && event.shift && event.vk == 0x56 {
                paste_clipboard_into_panel(&input_panel_key);
                return true;
            }
            let Some(session_id) = active_session_id(&input_panel_key) else {
                return false;
            };
            let encoded = lingxia_terminal::encode_key_event(lingxia_terminal::TerminalKeyEvent {
                vk: event.vk,
                ctrl: event.ctrl,
                shift: event.shift,
                alt: event.alt,
                character: event.character,
            });
            match encoded {
                Some(input) => {
                    if lingxia_terminal::terminal_write(session_id, &input) {
                        #[cfg(feature = "shell-chrome")]
                        clear_panel_selection(&input_panel_key, session_id);
                    }
                    true
                }
                None => false,
            }
        }),
    );

    let stop = Arc::new(AtomicBool::new(false));
    let panel_key = panel_id.to_string();
    windows_terminal_panels().insert(
        panel_key.clone(),
        WindowsTerminalPanel {
            tabs: vec![TerminalTab::new(session_id)],
            active: 0,
            maximized: false,
            projected_maximized: None,
            read_only: false,
            stop: Arc::clone(&stop),
            published_tabs: Vec::new(),
        },
    );
    publish_tab_strip(&panel_key);
    // The plain-text body wants the snapshot it was handed; the grid store
    // asks the engine for a frame either way.
    #[cfg(not(feature = "shell-chrome"))]
    if let Some(snapshot) = initial_snapshot {
        publish_windows_terminal_snapshot(&panel_key, session_id, snapshot);
    }
    #[cfg(feature = "shell-chrome")]
    let _ = initial_snapshot;
    publish_active_snapshot(&panel_key);

    thread::spawn(move || run_terminal_panel_poll_loop(&panel_key, &stop));
    Ok(())
}

#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn begin_terminal_search(panel_id: &str) {
    let Some(session_id) = active_session_id(panel_id) else {
        return;
    };
    let Some((hwnd, rect)) = super::terminal_grid::search_edit_geometry(panel_id) else {
        return;
    };
    let panel_for_change = panel_id.to_string();
    let panel_for_nav = panel_id.to_string();
    let panel_for_close = panel_id.to_string();
    lingxia_windows_contract::post_to_window_thread(
        hwnd,
        Box::new(move || {
            super::text_input::begin_search_edit(
                windows::Win32::Foundation::HWND(hwnd as *mut _),
                rect,
                "",
                super::text_input::SearchEditCallbacks {
                    on_change: Arc::new(move |query, case_sensitive, whole_word| {
                        perform_terminal_search(
                            &panel_for_change,
                            session_id,
                            query,
                            case_sensitive,
                            whole_word,
                        );
                    }),
                    on_navigate: Arc::new(move |delta| {
                        if let Some(line) = super::terminal_grid::navigate_search(session_id, delta)
                        {
                            lingxia_terminal::terminal_scroll_to_line(session_id, line);
                            publish_active_snapshot(&panel_for_nav);
                            invalidate_panel(&panel_for_nav);
                        }
                        super::text_input::update_search_status(
                            windows::Win32::Foundation::HWND(hwnd as *mut _),
                            super::terminal_grid::search_status(session_id),
                        );
                    }),
                    on_close: Arc::new(move || {
                        cancel_terminal_search(&panel_for_close, session_id);
                    }),
                },
            );
        }),
    );
}

#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn perform_terminal_search(
    panel_id: &str,
    session_id: u64,
    query: String,
    case_sensitive: bool,
    whole_word: bool,
) {
    let generation = {
        let mut searches = search_generations();
        let generation = searches.get(&session_id).copied().unwrap_or(0) + 1;
        searches.insert(session_id, generation);
        generation
    };
    if query.is_empty() {
        super::terminal_grid::clear_search(session_id);
        invalidate_panel(panel_id);
        if let Some((hwnd, _)) = super::terminal_grid::search_edit_geometry(panel_id) {
            super::text_input::update_search_status(
                windows::Win32::Foundation::HWND(hwnd as *mut _),
                (None, 0),
            );
        }
        return;
    }
    lingxia_terminal::terminal_search_cancel(session_id);
    let panel_id = panel_id.to_string();
    thread::spawn(move || {
        let mode = match (case_sensitive, whole_word) {
            (true, true) => lingxia_terminal::TerminalSearchMode::CaseSensitiveWholeWord,
            (true, false) => lingxia_terminal::TerminalSearchMode::CaseSensitive,
            (false, true) => lingxia_terminal::TerminalSearchMode::WholeWord,
            (false, false) => lingxia_terminal::TerminalSearchMode::Plain,
        };
        let Some(results) =
            lingxia_terminal::terminal_search_data(session_id, &query, mode, 10_000)
        else {
            return;
        };
        if results.cancelled || search_generations().get(&session_id).copied() != Some(generation) {
            return;
        }
        if let Some(line) = super::terminal_grid::set_search_results(session_id, results) {
            lingxia_terminal::terminal_scroll_to_line(session_id, line);
            publish_active_snapshot(&panel_id);
        }
        if let Some((hwnd, _)) = super::terminal_grid::search_edit_geometry(&panel_id) {
            super::text_input::update_search_status(
                windows::Win32::Foundation::HWND(hwnd as *mut _),
                super::terminal_grid::search_status(session_id),
            );
        }
        invalidate_panel(&panel_id);
    });
}

#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn cancel_terminal_search(panel_id: &str, session_id: u64) {
    let mut searches = search_generations();
    let generation = searches.get(&session_id).copied().unwrap_or(0) + 1;
    searches.insert(session_id, generation);
    drop(searches);
    super::terminal_grid::clear_search(session_id);
    invalidate_panel(panel_id);
}

#[cfg(feature = "terminal-runtime")]
fn automation_node_count(node: &AutomationPaneNode) -> usize {
    match node {
        AutomationPaneNode::Leaf(_) => 1,
        AutomationPaneNode::Split { first, second, .. } => {
            automation_node_count(first) + automation_node_count(second)
        }
    }
}

#[cfg(feature = "terminal-runtime")]
fn automation_tree_json(
    node: &AutomationPaneNode,
    focused: u64,
    active: bool,
    visible: bool,
    frames: &HashMap<u64, RECT>,
) -> Option<serde_json::Value> {
    match node {
        AutomationPaneNode::Leaf(session_id) => {
            let grid = super::terminal_grid::automation_grid_snapshot(*session_id)?;
            let rect = frames.get(session_id).copied().unwrap_or_default();
            Some(serde_json::json!({
                "kind": "leaf",
                "pane": {
                    "paneId": session_id.to_string(),
                    "active": active && focused == *session_id,
                    "visible": visible
                        && rect.right - rect.left > 1
                        && rect.bottom - rect.top > 1,
                    "frame": {
                        "x": rect.left,
                        "y": rect.top,
                        "width": (rect.right - rect.left).max(0),
                        "height": (rect.bottom - rect.top).max(0),
                    },
                    "grid": grid,
                },
            }))
        }
        AutomationPaneNode::Split {
            orient,
            first,
            second,
        } => Some(serde_json::json!({
            "kind": "split",
            "axis": match orient {
                PaneOrientation::Cols => "horizontal",
                PaneOrientation::Rows => "vertical",
            },
            "children": [
                automation_tree_json(first, focused, active, visible, frames)?,
                automation_tree_json(second, focused, active, visible, frames)?,
            ],
        })),
    }
}

#[cfg(feature = "terminal-runtime")]
fn automation_snapshot_json(panel_id: &str) -> Option<String> {
    let tabs = {
        let panels = windows_terminal_panels();
        let panel = panels.get(panel_id)?;
        panel
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| AutomationTabState {
                id: tab.root.first_leaf(),
                focused: tab.focused,
                active: index == panel.active,
                tree: AutomationPaneNode::from(&tab.root),
            })
            .collect::<Vec<_>>()
    };
    let body = super::terminal_grid::panel_body_rect(panel_id)?;
    let frames = active_pane_frames(panel_id, body)
        .into_iter()
        .map(|frame| (frame.session_id, frame.rect))
        .collect::<HashMap<_, _>>();
    let visible = crate::window_host::is_panel_visible(panel_id);
    let mut tab_values = Vec::with_capacity(tabs.len());
    let mut pane_count = 0;
    for tab in &tabs {
        let count = automation_node_count(&tab.tree);
        pane_count += count;
        let tree = automation_tree_json(&tab.tree, tab.focused, tab.active, visible, &frames);
        if tab.active && tree.is_none() {
            // Do not expose a half-laid-out active tree. The driver waits for
            // the next poll, after both host geometry and the engine frame are
            // available.
            return None;
        }
        let mut value = serde_json::json!({
            "id": tab.id.to_string(),
            "active": tab.active,
            "activePaneId": tab.focused.to_string(),
            "paneCount": count,
        });
        if let Some(tree) = tree {
            value["tree"] = tree;
        }
        tab_values.push(value);
    }
    let active_tab = tabs.iter().find(|tab| tab.active)?.id.to_string();
    let config = serde_json::from_str::<serde_json::Value>(
        &lingxia_terminal_config::runtime::current_json(),
    )
    .unwrap_or_else(|_| serde_json::json!({}));
    let chrome = serde_json::from_str::<serde_json::Value>(
        &lingxia_terminal_config::runtime::current_chrome_json(),
    )
    .unwrap_or_else(|_| serde_json::json!({}));
    serde_json::to_string(&serde_json::json!({
        "surfaceId": panel_id,
        "presentation": super::runtime::terminal_surface_presentation(panel_id),
        "visible": visible,
        // Whether the panel is expanded to the full content area. A layout
        // sync used to silently reset this, so automation has to be able to
        // assert it survives one.
        "maximized": terminal_panel_maximized(panel_id).unwrap_or(false),
        "activeTabId": active_tab,
        "tabCount": tabs.len(),
        "paneCount": pane_count,
        "configGeneration": lingxia_terminal_config::runtime::generation(),
        "visualGeneration": lingxia_terminal_config::runtime::visual_generation(),
        "config": config,
        "chrome": chrome,
        "tabs": tab_values,
    }))
    .ok()
}

#[cfg(feature = "terminal-runtime")]
fn terminal_automation_authority() -> lxapp::terminal_automation::TerminalAutomationAuthority {
    lxapp::terminal_automation::TerminalAutomationAuthority::__native_host()
}

#[cfg(feature = "terminal-runtime")]
fn take_automation_command(panel_id: &str) -> Option<u64> {
    let raw = lxapp::terminal_automation::take_command(&terminal_automation_authority(), panel_id);
    if raw.is_empty() {
        return None;
    }
    let value = match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(value) => value,
        Err(error) => {
            log::warn!("invalid terminal automation command: {error}");
            return None;
        }
    };
    let id = value.get("id")?.as_u64()?;
    let action = value.get("action").and_then(serde_json::Value::as_str);
    if action == Some("newTab") {
        open_terminal_tab(panel_id);
        return Some(id);
    }
    if action == Some("setMaximized") {
        let Some(maximized) = value
            .pointer("/params/maximized")
            .and_then(serde_json::Value::as_bool)
        else {
            lxapp::terminal_automation::complete_command(
                &terminal_automation_authority(),
                id,
                false,
                "setMaximized requires a boolean 'maximized'",
            );
            return None;
        };
        set_terminal_panel_maximized_by_user(panel_id, maximized);
        return Some(id);
    }
    if action == Some("input") {
        let Some(text) = value
            .pointer("/params/text")
            .and_then(serde_json::Value::as_str)
        else {
            lxapp::terminal_automation::complete_command(
                &terminal_automation_authority(),
                id,
                false,
                "input requires text",
            );
            return None;
        };
        let Some(session_id) = active_session_id(panel_id) else {
            lxapp::terminal_automation::complete_command(
                &terminal_automation_authority(),
                id,
                false,
                "terminal surface has no active pane",
            );
            return None;
        };
        if lingxia_terminal::terminal_write(session_id, text) {
            return Some(id);
        }
        lxapp::terminal_automation::complete_command(
            &terminal_automation_authority(),
            id,
            false,
            "terminal input was rejected",
        );
        return None;
    }
    if action != Some("split") {
        let name = action.unwrap_or("missing");
        lxapp::terminal_automation::complete_command(
            &terminal_automation_authority(),
            id,
            false,
            &format!("unknown terminal automation action '{name}'"),
        );
        return None;
    }
    let direction = value
        .pointer("/params/direction")
        .and_then(serde_json::Value::as_str);
    let direction = match direction {
        Some("left") => SplitDir::Left,
        Some("right") => SplitDir::Right,
        Some("up") => SplitDir::Up,
        Some("down") => SplitDir::Down,
        _ => {
            lxapp::terminal_automation::complete_command(
                &terminal_automation_authority(),
                id,
                false,
                "split requires left, right, up, or down",
            );
            return None;
        }
    };
    if split_focused_pane(panel_id, direction) {
        Some(id)
    } else {
        lxapp::terminal_automation::complete_command(
            &terminal_automation_authority(),
            id,
            false,
            "terminal surface has no active pane",
        );
        None
    }
}

/// Poll loop of one panel: reaps exited sessions (any pane of any tab),
/// keeps the active tab's pane PTY grids in sync with their painted
/// rects, tracks the focused pane's automatic title, and publishes the
/// active tab's pane snapshots when they change. Inactive tabs keep
/// running; only their exit flag is checked per tick.
#[cfg(feature = "terminal-runtime")]
fn run_terminal_panel_poll_loop(panel_key: &str, stop: &Arc<AtomicBool>) {
    let mut last_generations: HashMap<u64, (u64, u64, u64)> = HashMap::new();
    let mut last_active_set: Vec<u64> = Vec::new();
    let mut refresh_tick: u32 = 0;
    let mut last_config = lingxia_terminal_config::runtime::generation();
    let mut last_visual = lingxia_terminal_config::runtime::visual_generation();
    let mut pending_automation = None;
    #[cfg(feature = "shell-chrome")]
    let mut pending_resize: HashMap<u64, (u16, u16)> = HashMap::new();
    loop {
        if stop.load(Ordering::Acquire) || !terminal_panel_poll_is_current(panel_key, stop) {
            break;
        }
        if pending_automation.is_none() {
            pending_automation = take_automation_command(panel_key);
        }
        let all_sessions: Vec<u64> = {
            let panels = windows_terminal_panels();
            let Some(panel) = panels.get(panel_key) else {
                break;
            };
            panel.tabs.iter().flat_map(|tab| tab.sessions()).collect()
        };

        // `exit` closes the pane; the last pane closes the tab; the last tab
        // closes the whole panel.
        let mut panel_closed = false;
        for session_id in all_sessions {
            if lingxia_terminal::terminal_exited(session_id)
                && close_pane_session(panel_key, session_id)
            {
                panel_closed = true;
                break;
            }
        }
        if panel_closed {
            break;
        }

        // Active tab's panes (session id + desired pixel rect for resize).
        let active_sessions: Vec<u64> = {
            let panels = windows_terminal_panels();
            let Some(tab) = panels.get(panel_key).and_then(|panel| panel.active_tab()) else {
                break;
            };
            tab.sessions()
        };
        if active_sessions.is_empty() {
            break;
        }

        let switched = last_active_set != active_sessions;
        let mut any_change = switched;
        for &session_id in &active_sessions {
            #[cfg(feature = "shell-chrome")]
            if super::terminal_grid::expire_scrollbar(session_id) {
                any_change = true;
            }
            // Frame first, then the two things a frame leaves out. None of it
            // walks the grid cell by cell, so an idle pane costs a generation
            // comparison rather than a copy of the screen.
            let Some((scrollbar, exited)) = lingxia_terminal::terminal_view_state(session_id)
            else {
                if close_pane_session(panel_key, session_id) {
                    panel_closed = true;
                }
                break;
            };
            if exited {
                if close_pane_session(panel_key, session_id) {
                    panel_closed = true;
                }
                break;
            }
            let title = lingxia_terminal::terminal_title_state_data(session_id).unwrap_or_default();
            let since = super::terminal_grid::session_generation(session_id);
            let image_since = super::terminal_grid::session_image_generation(session_id);
            let update = lingxia_terminal::terminal_render_data(session_id, since, image_since);
            let (update, images) = update.unwrap_or((
                lingxia_terminal::TerminalFrameUpdate::Unchanged { generation: since },
                lingxia_terminal::TerminalImageSnapshot {
                    generation: image_since,
                    ..Default::default()
                },
            ));
            let (grid_generation, frame) = match update {
                lingxia_terminal::TerminalFrameUpdate::Changed(frame) => {
                    (frame.generation, Some(frame))
                }
                lingxia_terminal::TerminalFrameUpdate::Unchanged { generation } => {
                    (generation, None)
                }
            };
            let grid_size = frame
                .as_ref()
                .map(|frame| (frame.cols, frame.rows))
                .or_else(|| super::terminal_grid::session_grid_size(session_id));

            #[cfg(feature = "shell-chrome")]
            {
                if switched {
                    pending_resize.remove(&session_id);
                }
                let desired = super::terminal_grid::desired_session_grid_size(session_id)
                    .filter(|&size| Some(size) != grid_size);
                // Resize the PTY only once the desired grid held for two
                // consecutive ticks, so divider/grow drags don't cause
                // resize storms (converges within two ticks).
                match desired {
                    Some((cols, rows))
                        if pending_resize.get(&session_id) == Some(&(cols, rows)) =>
                    {
                        lingxia_terminal::terminal_resize(session_id, cols, rows);
                        pending_resize.remove(&session_id);
                    }
                    Some(target) => {
                        pending_resize.insert(session_id, target);
                    }
                    None => {
                        pending_resize.remove(&session_id);
                    }
                }
            }

            update_focused_auto_title(panel_key, session_id, &title);
            super::terminal_grid::set_session_view_state(session_id, scrollbar, exited);

            let image_generation = images.generation;
            let generations = (grid_generation, title.generation, image_generation);
            if last_generations.get(&session_id) != Some(&generations) {
                any_change = true;
                super::terminal_grid::set_session_render(
                    session_id,
                    frame.map(|frame| *frame),
                    images.changed.then_some(images),
                );
                last_generations.insert(session_id, generations);
            }
        }
        if panel_closed {
            break;
        }

        // A font or theme change moves nothing in the snapshot, so without
        // this the card keeps its old colors and metrics until something else
        // happens to dirty it — up to two seconds of looking broken.
        let config = lingxia_terminal_config::runtime::generation();
        let config_changed = config != last_config;
        last_config = config;
        let visual = lingxia_terminal_config::runtime::visual_generation();
        let visual_changed = visual != last_visual;
        last_visual = visual;

        refresh_tick = refresh_tick.wrapping_add(1);
        if any_change || config_changed || visual_changed || refresh_tick.is_multiple_of(25) {
            invalidate_panel(panel_key);
        }
        if let Some(snapshot) = automation_snapshot_json(panel_key) {
            // Hold the panel registry across publication so a close/reopen of
            // the same surface id cannot let this stale worker overwrite the
            // new workspace's snapshot after its stop flag was set.
            let panels = windows_terminal_panels();
            if !panels
                .get(panel_key)
                .is_some_and(|panel| Arc::ptr_eq(&panel.stop, stop))
            {
                break;
            }
            let _ = lxapp::terminal_automation::publish_snapshot(
                &terminal_automation_authority(),
                panel_key,
                &snapshot,
            );
            if let Some(id) = pending_automation.take() {
                let _ = lxapp::terminal_automation::complete_command(
                    &terminal_automation_authority(),
                    id,
                    true,
                    &snapshot,
                );
            }
        }
        last_active_set = active_sessions;
        thread::sleep(Duration::from_millis(80));
    }
    if let Some(id) = pending_automation {
        let _ = lxapp::terminal_automation::complete_command(
            &terminal_automation_authority(),
            id,
            false,
            "terminal surface closed before the command completed",
        );
    }
}

#[cfg(feature = "terminal-runtime")]
fn terminal_panel_poll_is_current(panel_key: &str, stop: &Arc<AtomicBool>) -> bool {
    windows_terminal_panels()
        .get(panel_key)
        .is_some_and(|panel| Arc::ptr_eq(&panel.stop, stop))
}

/// Tracks the session-reported title of the active tab's focused pane and
/// republishes the strip when it changed (no-op while a custom title
/// overrides it or the session is not the focused pane).
#[cfg(feature = "terminal-runtime")]
fn update_focused_auto_title(
    panel_id: &str,
    session_id: u64,
    title: &lingxia_terminal::TerminalTitleView,
) {
    let process_title = (!title.process_title.is_empty()).then_some(title.process_title.as_str());
    let auto_title = stable_tab_title(process_title, title.title.as_deref()).to_string();
    let changed = {
        let mut panels = windows_terminal_panels();
        let Some(tab) = panels
            .get_mut(panel_id)
            .and_then(|panel| panel.active_tab_mut())
            .filter(|tab| tab.focused == session_id)
        else {
            return;
        };
        let changed = tab.auto_title != auto_title;
        tab.auto_title = auto_title;
        changed
    };
    if changed {
        publish_tab_strip(panel_id);
    }
}

/// Prefer the stable process identity over OSC titles: TUI apps may animate
/// their OSC title, which would otherwise repaint the tab on every frame.
#[cfg(feature = "terminal-runtime")]
fn stable_tab_title<'a>(
    process_title: Option<&'a str>,
    terminal_title: Option<&'a str>,
) -> &'a str {
    [process_title, terminal_title]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|title| !title.is_empty())
        .unwrap_or("terminal")
}

/// Session id that input/copy/paste route to: the active tab's focused pane.
#[cfg(feature = "terminal-runtime")]
fn active_session_id(panel_id: &str) -> Option<u64> {
    let panels = windows_terminal_panels();
    let panel = panels.get(panel_id)?;
    panel.active_tab().map(|tab| tab.focused)
}

/// Number of panes in the active tab.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn pane_count(panel_id: &str) -> usize {
    let panels = windows_terminal_panels();
    panels
        .get(panel_id)
        .and_then(|panel| panel.active_tab())
        .map(|tab| tab.sessions().len())
        .unwrap_or(0)
}

/// Closes one pane's session and removes its leaf, collapsing the parent
/// split. When the pane was its tab's last, the tab closes (a neighbor is
/// activated); when that was the panel's last tab, the panel closes.
/// Returns `true` when the whole panel was closed.
#[cfg(feature = "terminal-runtime")]
fn close_pane_session(panel_id: &str, session_id: u64) -> bool {
    let is_last_session = windows_terminal_panels()
        .get(panel_id)
        .is_some_and(|panel| {
            panel.tabs.len() == 1 && panel.tabs[0].sessions().as_slice() == [session_id]
        });
    if is_last_session && super::runtime::terminal_surface_is_protected_root(panel_id) {
        let replacement = create_panel_session(panel_id, None);
        if replacement == 0 {
            log::warn!("failed to preserve root terminal session for {panel_id}");
            return false;
        }
        let replaced = {
            let mut panels = windows_terminal_panels();
            panels.get_mut(panel_id).is_some_and(|panel| {
                if panel.tabs.len() != 1 || panel.tabs[0].sessions().as_slice() != [session_id] {
                    return false;
                }
                panel.tabs = vec![TerminalTab::new(replacement)];
                panel.active = 0;
                true
            })
        };
        if !replaced {
            lingxia_terminal::terminal_close(replacement);
            return false;
        } else {
            lingxia_terminal::terminal_close(session_id);
            #[cfg(feature = "shell-chrome")]
            super::terminal_grid::clear_session(session_id);
            #[cfg(feature = "shell-chrome")]
            if let Some(remainders) = TERMINAL_WHEEL_REMAINDERS.get() {
                remainders
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .remove(&session_id);
            }
            publish_tab_strip(panel_id);
            publish_active_snapshot(panel_id);
            return false;
        }
    }

    lingxia_terminal::terminal_close(session_id);
    #[cfg(feature = "shell-chrome")]
    super::terminal_grid::clear_session(session_id);
    #[cfg(feature = "shell-chrome")]
    if let Some(remainders) = TERMINAL_WHEEL_REMAINDERS.get() {
        remainders
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&session_id);
    }

    let outcome = {
        let mut panels = windows_terminal_panels();
        let Some(panel) = panels.get_mut(panel_id) else {
            return false;
        };
        let Some(tab_index) = panel
            .tabs
            .iter()
            .position(|tab| tab.sessions().contains(&session_id))
        else {
            return false;
        };

        // Remove the leaf from the tab's tree; collapse the tab if empty.
        let tab = &mut panel.tabs[tab_index];
        let root = std::mem::replace(&mut tab.root, PaneNode::Leaf(0));
        match remove_leaf(root, session_id) {
            Some(new_root) => {
                if tab.focused == session_id {
                    tab.focused = new_root.first_leaf();
                }
                tab.root = new_root;
                CloseOutcome::Pane
            }
            None => {
                panel.tabs.remove(tab_index);
                if panel.tabs.is_empty() {
                    CloseOutcome::Panel
                } else {
                    if tab_index < panel.active {
                        panel.active -= 1;
                    }
                    if panel.active >= panel.tabs.len() {
                        panel.active = panel.tabs.len() - 1;
                    }
                    CloseOutcome::Tab
                }
            }
        }
    };

    match outcome {
        CloseOutcome::Panel => super::runtime::close_exhausted_terminal_surface(panel_id),
        CloseOutcome::Tab | CloseOutcome::Pane => {
            publish_tab_strip(panel_id);
            publish_active_snapshot(panel_id);
            false
        }
    }
}

/// What closing a pane session did to the panel structure.
#[cfg(feature = "terminal-runtime")]
enum CloseOutcome {
    /// The pane was removed; its tab and the panel remain.
    Pane,
    /// The pane was the tab's last; the tab was removed, panel remains.
    Tab,
    /// The tab was the panel's last; the whole panel was closed.
    Panel,
}

/// Closes every session in `session_ids` (one tab's panes). Returns once
/// the panel was closed (last tab) or the tab was removed.
#[cfg(feature = "terminal-runtime")]
fn close_terminal_tab_by_sessions(panel_id: &str, session_ids: &[u64]) {
    for &session_id in session_ids {
        if close_pane_session(panel_id, session_id) {
            // Panel closed; remaining ids belonged to the now-gone panel.
            return;
        }
    }
}

/// Stops the poll thread, terminates all sessions, and clears the panel's
/// input handler and grid store. The caller hides the panel window.
#[cfg(feature = "terminal-runtime")]
fn shutdown_windows_terminal_panel_state(panel_id: &str) {
    lxapp::terminal_automation::remove_workspace(&terminal_automation_authority(), panel_id);
    lingxia_windows_contract::clear_host_panel_input_handler(panel_id);
    #[cfg(feature = "shell-chrome")]
    super::terminal_gpu::drop_panel(panel_id);
    #[cfg(feature = "shell-chrome")]
    super::terminal_grid::clear_panel(panel_id);
    if let Some(panel) = windows_terminal_panels().remove(panel_id) {
        panel.stop.store(true, Ordering::Release);
        for tab in panel.tabs {
            for session_id in tab.sessions() {
                #[cfg(feature = "shell-chrome")]
                super::terminal_grid::clear_session(session_id);
                #[cfg(feature = "shell-chrome")]
                if let Some(remainders) = TERMINAL_WHEEL_REMAINDERS.get() {
                    remainders
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .remove(&session_id);
                }
                lingxia_terminal::terminal_close(session_id);
            }
        }
    }
}

#[cfg(feature = "terminal-runtime")]
pub(super) fn destroy_windows_terminal_panel(panel_id: &str) {
    shutdown_windows_terminal_panel_state(panel_id);
    crate::window_host::remove_interactive_host_panel(panel_id);
}

// ---- Pane layout query (used by the grid painter) ----

/// One pane's placement within the panel body, in host client coordinates.
/// Only the terminal-runtime grid painter consumes these.
#[cfg(all(feature = "shell-chrome", feature = "terminal-runtime"))]
pub(super) struct PaneFrame {
    pub(super) session_id: u64,
    pub(super) rect: RECT,
    pub(super) focused: bool,
}

/// Lays out the active tab's panes within `body` (host client coordinates)
/// and returns each pane's rect plus whether it is the focused pane. Empty
/// when the panel has no terminal state yet.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(super) fn active_pane_frames(panel_id: &str, body: RECT) -> Vec<PaneFrame> {
    let panels = windows_terminal_panels();
    let Some(tab) = panels.get(panel_id).and_then(|panel| panel.active_tab()) else {
        return Vec::new();
    };
    let mut frames = Vec::new();
    layout_node(&tab.root, body, &mut frames);
    frames
        .into_iter()
        .map(|(session_id, rect)| PaneFrame {
            session_id,
            rect,
            focused: session_id == tab.focused,
        })
        .collect()
}

/// Session id of the active tab's focused pane, used by the chrome painter
/// to pick the card's surface background and fallback body text.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(super) fn focused_session(panel_id: &str) -> Option<u64> {
    let panels = windows_terminal_panels();
    panels
        .get(panel_id)
        .and_then(|panel| panel.active_tab())
        .map(|tab| tab.focused)
}

/// Without the terminal runtime there is no focused pane.
#[cfg(all(not(feature = "terminal-runtime"), feature = "shell-chrome"))]
pub(super) fn focused_session(_panel_id: &str) -> Option<u64> {
    None
}

// ---- Pane drag (moving an existing session within the active tab) ----

/// Hit target at the top center of each pane. The painter draws three dots
/// inside this rect so the control stays easy to grab without looking
/// heavy over terminal content.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(super) fn pane_drag_handle_rect(rect: RECT) -> RECT {
    let width = 44.min((rect.right - rect.left).max(0));
    let left = rect.left + ((rect.right - rect.left - width) / 2).max(0);
    RECT {
        left,
        top: rect.top + 2,
        right: left + width,
        bottom: (rect.top + 14).min(rect.bottom),
    }
}

/// Trailing-edge close target for a split pane. It deliberately stays clear
/// of the centered drag handle so each control has one unambiguous action.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(super) fn pane_close_button_rect(rect: RECT) -> RECT {
    const SIZE: i32 = 24;
    const INSET: i32 = 4;
    RECT {
        left: (rect.right - INSET - SIZE).max(rect.left),
        top: (rect.top + INSET).min(rect.bottom),
        right: (rect.right - INSET).max(rect.left),
        bottom: (rect.top + INSET + SIZE).min(rect.bottom),
    }
}

/// Top-edge reveal zone for a split pane's drag and close controls.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(super) fn pane_controls_hover_rect(rect: RECT) -> RECT {
    const HEIGHT: i32 = 32;
    RECT {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: (rect.top + HEIGHT).min(rect.bottom),
    }
}

/// Pane top edge under the pointer, used to invalidate hover-only split
/// controls when the pointer enters, leaves, or crosses a pane boundary.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(crate) fn pane_hover_rect(panel_id: &str, x: i32, y: i32) -> Option<RECT> {
    let body = super::terminal_grid::panel_body_rect(panel_id)?;
    let frames = active_pane_frames(panel_id, body);
    (frames.len() > 1)
        .then(|| {
            frames
                .into_iter()
                .map(|frame| pane_controls_hover_rect(frame.rect))
                .find(|rect| point_in_rect(*rect, x, y))
        })
        .flatten()
}

/// Closes the exact split pane under the pointer. A lone pane keeps its close
/// action in the tab strip, matching the split controls on macOS.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(crate) fn close_pane_at(panel_id: &str, x: i32, y: i32) -> bool {
    let Some(body) = super::terminal_grid::panel_body_rect(panel_id) else {
        return false;
    };
    let frames = active_pane_frames(panel_id, body);
    if frames.len() <= 1 {
        return false;
    }
    let Some(session_id) = frames
        .into_iter()
        .find(|frame| point_in_rect(pane_close_button_rect(frame.rect), x, y))
        .map(|frame| frame.session_id)
    else {
        return false;
    };
    close_pane_session(panel_id, session_id);
    true
}

#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
#[derive(Clone)]
struct ActivePaneDrag {
    panel_id: String,
    source: u64,
    target: Option<(u64, SplitDir)>,
}

#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
static ACTIVE_PANE_DRAG: OnceLock<Mutex<Option<ActivePaneDrag>>> = OnceLock::new();

#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn active_pane_drag() -> std::sync::MutexGuard<'static, Option<ActivePaneDrag>> {
    ACTIVE_PANE_DRAG
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn point_in_rect(rect: RECT, x: i32, y: i32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn pane_drop_direction(rect: RECT, x: i32, y: i32, current: Option<SplitDir>) -> Option<SplitDir> {
    let width = (rect.right - rect.left).max(1) as f64;
    let height = (rect.bottom - rect.top).max(1) as f64;
    let x = (f64::from(x - rect.left) / width).clamp(0.0, 1.0);
    let y = (f64::from(y - rect.top) / height).clamp(0.0, 1.0);
    let candidates = [
        (x, SplitDir::Left),
        (1.0 - x, SplitDir::Right),
        (y, SplitDir::Up),
        (1.0 - y, SplitDir::Down),
    ];
    let nearest = candidates
        .into_iter()
        .min_by(|left, right| left.0.total_cmp(&right.0))?;
    const DROP_ZONE: f64 = 0.35;
    const HYSTERESIS: f64 = 0.06;
    if let Some(current) = current
        && let Some((current_distance, _)) = candidates
            .into_iter()
            .find(|(_, direction)| *direction == current)
        && current_distance <= DROP_ZONE + HYSTERESIS
        && (nearest.0 > DROP_ZONE || nearest.0 + HYSTERESIS >= current_distance)
    {
        return Some(current);
    }
    (nearest.0 <= DROP_ZONE).then_some(nearest.1)
}

/// Returns whether the pointer is over a pane's drag handle. Handles only
/// exist once a tab is split; dragging a lone pane has no destination.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(crate) fn pane_drag_handle_at(panel_id: &str, x: i32, y: i32) -> bool {
    let Some(body) = super::terminal_grid::panel_body_rect(panel_id) else {
        return false;
    };
    let frames = active_pane_frames(panel_id, body);
    frames.len() > 1
        && frames
            .into_iter()
            .any(|frame| point_in_rect(pane_drag_handle_rect(frame.rect), x, y))
}

/// Begins moving the pane whose top-center handle is under the pointer.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(crate) fn begin_pane_drag(panel_id: &str, x: i32, y: i32) -> bool {
    let Some(body) = super::terminal_grid::panel_body_rect(panel_id) else {
        return false;
    };
    let frames = active_pane_frames(panel_id, body);
    if frames.len() <= 1 {
        return false;
    }
    let Some(source) = frames
        .into_iter()
        .find(|frame| point_in_rect(pane_drag_handle_rect(frame.rect), x, y))
        .map(|frame| frame.session_id)
    else {
        return false;
    };
    *active_pane_drag() = Some(ActivePaneDrag {
        panel_id: panel_id.to_string(),
        source,
        target: None,
    });
    invalidate_panel(panel_id);
    true
}

/// Tracks the drop edge under the pointer while the host owns mouse capture.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(crate) fn update_pane_drag(x: i32, y: i32) -> bool {
    let Some((panel_id, source, previous)) = active_pane_drag()
        .as_ref()
        .map(|drag| (drag.panel_id.clone(), drag.source, drag.target))
    else {
        return false;
    };
    let target = super::terminal_grid::panel_body_rect(&panel_id).and_then(|body| {
        active_pane_frames(&panel_id, body)
            .into_iter()
            .find(|frame| frame.session_id != source && point_in_rect(frame.rect, x, y))
            .and_then(|frame| {
                let current = previous
                    .filter(|(target, _)| *target == frame.session_id)
                    .map(|(_, direction)| direction);
                pane_drop_direction(frame.rect, x, y, current)
                    .map(|direction| (frame.session_id, direction))
            })
    });
    if target != previous {
        if let Some(drag) = active_pane_drag().as_mut() {
            drag.target = target;
        }
        invalidate_panel(&panel_id);
    }
    true
}

/// Ends the active pane drag. A valid drop rewrites only the pane tree; the
/// source session id, PTY process, scrollback, and title state stay intact.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(crate) fn end_pane_drag(commit: bool) -> bool {
    let Some(drag) = active_pane_drag().take() else {
        return false;
    };
    let moved = if commit {
        drag.target.is_some_and(|(target, direction)| {
            let mut panels = windows_terminal_panels();
            let Some(tab) = panels
                .get_mut(&drag.panel_id)
                .and_then(|panel| panel.active_tab_mut())
            else {
                return false;
            };
            if tab.root.move_leaf(drag.source, target, direction) {
                tab.focused = drag.source;
                true
            } else {
                false
            }
        })
    } else {
        false
    };
    invalidate_panel(&drag.panel_id);
    if moved {
        publish_tab_strip(&drag.panel_id);
        publish_active_snapshot(&drag.panel_id);
    }
    true
}

/// Current drag source, plus the directional half-pane drop indicator.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(super) fn pane_drag_visuals(panel_id: &str, body: RECT) -> (Option<u64>, Option<RECT>) {
    let Some(drag) = active_pane_drag()
        .as_ref()
        .filter(|drag| drag.panel_id == panel_id)
        .cloned()
    else {
        return (None, None);
    };
    let indicator = drag.target.and_then(|(target, direction)| {
        active_pane_frames(panel_id, body)
            .into_iter()
            .find(|frame| frame.session_id == target)
            .map(|frame| pane_drop_rect(frame.rect, direction))
    });
    (Some(drag.source), indicator)
}

#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn pane_drop_rect(rect: RECT, direction: SplitDir) -> RECT {
    match direction {
        SplitDir::Left => RECT {
            right: rect.left + (rect.right - rect.left) / 2,
            ..rect
        },
        SplitDir::Right => RECT {
            left: rect.left + (rect.right - rect.left) / 2,
            ..rect
        },
        SplitDir::Up => RECT {
            bottom: rect.top + (rect.bottom - rect.top) / 2,
            ..rect
        },
        SplitDir::Down => RECT {
            top: rect.top + (rect.bottom - rect.top) / 2,
            ..rect
        },
    }
}

#[cfg(all(not(feature = "terminal-runtime"), feature = "shell-chrome"))]
pub(crate) fn pane_drag_handle_at(_panel_id: &str, _x: i32, _y: i32) -> bool {
    false
}

#[cfg(not(all(feature = "terminal-runtime", feature = "shell-chrome")))]
pub(crate) fn pane_hover_rect(
    _panel_id: &str,
    _x: i32,
    _y: i32,
) -> Option<windows::Win32::Foundation::RECT> {
    None
}

#[cfg(not(all(feature = "terminal-runtime", feature = "shell-chrome")))]
pub(crate) fn close_pane_at(_panel_id: &str, _x: i32, _y: i32) -> bool {
    false
}

#[cfg(all(not(feature = "terminal-runtime"), feature = "shell-chrome"))]
pub(crate) fn begin_pane_drag(_panel_id: &str, _x: i32, _y: i32) -> bool {
    false
}

#[cfg(all(not(feature = "terminal-runtime"), feature = "shell-chrome"))]
pub(crate) fn update_pane_drag(_x: i32, _y: i32) -> bool {
    false
}

#[cfg(all(not(feature = "terminal-runtime"), feature = "shell-chrome"))]
pub(crate) fn end_pane_drag(_commit: bool) -> bool {
    false
}

// ---- Divider drag (resizing split ratios) ----

/// The split divider currently being dragged via the window proc's capture
/// loop.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
struct ActiveDivider {
    panel_id: String,
    /// Descent path (false = first child, true = second) to the Split node.
    path: Vec<bool>,
    /// Rect of the split node being divided (for ratio math).
    bounds: RECT,
    /// Whether the divider is vertical (a `Cols` split 鈫?horizontal drag).
    vertical: bool,
}

#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
static ACTIVE_DIVIDER: OnceLock<Mutex<Option<ActiveDivider>>> = OnceLock::new();

#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn active_divider() -> std::sync::MutexGuard<'static, Option<ActiveDivider>> {
    ACTIVE_DIVIDER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Extra grab tolerance (px) around the thin divider gap.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
const DIVIDER_GRAB: i32 = 3;

/// Collects each split's draggable divider as `(hit_rect, vertical, bounds,
/// path)` over the laid-out tree.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn collect_dividers(
    node: &PaneNode,
    rect: RECT,
    path: &mut Vec<bool>,
    out: &mut Vec<(RECT, bool, RECT, Vec<bool>)>,
) {
    if let PaneNode::Split {
        orient,
        ratio,
        first,
        second,
    } = node
    {
        let (r1, r2) = split_rect(rect, *orient, *ratio);
        let vertical = matches!(orient, PaneOrientation::Cols);
        let hit = if vertical {
            RECT {
                left: r1.right - DIVIDER_GRAB,
                top: rect.top,
                right: r2.left + DIVIDER_GRAB,
                bottom: rect.bottom,
            }
        } else {
            RECT {
                left: rect.left,
                top: r1.bottom - DIVIDER_GRAB,
                right: rect.right,
                bottom: r2.top + DIVIDER_GRAB,
            }
        };
        out.push((hit, vertical, rect, path.clone()));
        path.push(false);
        collect_dividers(first, r1, path, out);
        path.pop();
        path.push(true);
        collect_dividers(second, r2, path, out);
        path.pop();
    }
}

#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn divider_under(panel_id: &str, x: i32, y: i32) -> Option<(bool, RECT, Vec<bool>)> {
    let body = super::terminal_grid::panel_body_rect(panel_id)?;
    let panels = windows_terminal_panels();
    let tab = panels.get(panel_id).and_then(|panel| panel.active_tab())?;
    let mut dividers = Vec::new();
    collect_dividers(&tab.root, body, &mut Vec::new(), &mut dividers);
    dividers
        .into_iter()
        .find(|(hit, ..)| x >= hit.left && x < hit.right && y >= hit.top && y < hit.bottom)
        .map(|(_, vertical, bounds, path)| (vertical, bounds, path))
}

/// Whether a divider sits under `(x, y)` in the active tab, and its
/// orientation (`Some(true)` = vertical). The window proc uses this for the
/// resize cursor.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(crate) fn divider_orientation_at(panel_id: &str, x: i32, y: i32) -> Option<bool> {
    divider_under(panel_id, x, y).map(|(vertical, ..)| vertical)
}

/// Begins dragging the divider under `(x, y)`. Returns `Some(vertical)` when
/// one was hit (driven by the window proc's capture loop).
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(crate) fn begin_divider_drag(panel_id: &str, x: i32, y: i32) -> Option<bool> {
    let (vertical, bounds, path) = divider_under(panel_id, x, y)?;
    *active_divider() = Some(ActiveDivider {
        panel_id: panel_id.to_string(),
        path,
        bounds,
        vertical,
    });
    Some(vertical)
}

/// Updates the dragged divider's ratio from the cursor position and repaints.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(crate) fn update_divider_drag(x: i32, y: i32) {
    let (panel_id, path, ratio) = {
        let guard = active_divider();
        let Some(divider) = guard.as_ref() else {
            return;
        };
        let ratio = if divider.vertical {
            let span = (divider.bounds.right - divider.bounds.left) as f32;
            if span <= 0.0 {
                return;
            }
            (x - divider.bounds.left) as f32 / span
        } else {
            let span = (divider.bounds.bottom - divider.bounds.top) as f32;
            if span <= 0.0 {
                return;
            }
            (y - divider.bounds.top) as f32 / span
        };
        (
            divider.panel_id.clone(),
            divider.path.clone(),
            ratio.clamp(0.05, 0.95),
        )
    };
    {
        let mut panels = windows_terminal_panels();
        let Some(tab) = panels
            .get_mut(&panel_id)
            .and_then(|panel| panel.active_tab_mut())
        else {
            return;
        };
        if let Some(PaneNode::Split {
            ratio: node_ratio, ..
        }) = node_at_path_mut(&mut tab.root, &path)
        {
            *node_ratio = ratio;
        }
    }
    // Repaint with the new layout; the poll loop resizes each pane's PTY to
    // its new rect within a couple of ticks.
    invalidate_panel(&panel_id);
}

/// Ends the divider drag.
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
pub(crate) fn end_divider_drag() {
    *active_divider() = None;
}

/// Navigates to the node at `path` (false = first child, true = second).
#[cfg(all(feature = "terminal-runtime", feature = "shell-chrome"))]
fn node_at_path_mut<'a>(mut node: &'a mut PaneNode, path: &[bool]) -> Option<&'a mut PaneNode> {
    for &second in path {
        match node {
            PaneNode::Split {
                first, second: sec, ..
            } => {
                node = if second { sec } else { first };
            }
            PaneNode::Leaf(_) => return None,
        }
    }
    Some(node)
}

// ---- Divider drag: no-op stubs without the terminal runtime ----

#[cfg(all(not(feature = "terminal-runtime"), feature = "shell-chrome"))]
pub(crate) fn divider_orientation_at(_panel_id: &str, _x: i32, _y: i32) -> Option<bool> {
    None
}

#[cfg(all(not(feature = "terminal-runtime"), feature = "shell-chrome"))]
pub(crate) fn begin_divider_drag(_panel_id: &str, _x: i32, _y: i32) -> Option<bool> {
    None
}

#[cfg(all(not(feature = "terminal-runtime"), feature = "shell-chrome"))]
pub(crate) fn update_divider_drag(_x: i32, _y: i32) {}

#[cfg(all(not(feature = "terminal-runtime"), feature = "shell-chrome"))]
pub(crate) fn end_divider_drag() {}

// ---- Publishing to the webview/shell layers ----

/// Pushes the panel's tab strip (id/title/active) to the webview layer
/// when it differs from the last published strip. The tab id surfaced to
/// the chrome is the tab's focused session id.
#[cfg(feature = "terminal-runtime")]
fn publish_tab_strip(panel_id: &str) {
    let (strip, active_title) = {
        let mut panels = windows_terminal_panels();
        let Some(panel) = panels.get_mut(panel_id) else {
            return;
        };
        let active = panel.active;
        let strip: Vec<WindowsHostPanelTab> = panel
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| WindowsHostPanelTab {
                id: tab.focused,
                title: tab.display_title().to_string(),
                active: index == active,
            })
            .collect();
        if strip == panel.published_tabs {
            return;
        }
        panel.published_tabs = strip.clone();
        let active_title = strip
            .iter()
            .find(|tab| tab.active)
            .map(|tab| tab.title.clone());
        (strip, active_title)
    };
    lingxia_windows_contract::set_host_panel_tabs(panel_id, strip);
    if let Some(title) = active_title {
        super::runtime::on_terminal_panel_active_title_changed(panel_id, &title);
    }
}

/// Publishes the active tab's pane snapshots immediately (tab switches and
/// structural changes shouldn't wait for the next poll tick).
#[cfg(feature = "terminal-runtime")]
fn publish_active_snapshot(panel_id: &str) {
    let sessions: Vec<u64> = {
        let panels = windows_terminal_panels();
        let Some(tab) = panels.get(panel_id).and_then(|panel| panel.active_tab()) else {
            return;
        };
        tab.sessions()
    };
    let mut changed = false;
    for session_id in sessions {
        changed |= publish_session_frame(panel_id, session_id);
    }
    // An idle session hands back "unchanged" and there is nothing to repaint;
    // invalidating anyway would put the whole grid through the GPU for output
    // that never arrived.
    if changed {
        invalidate_panel(panel_id);
    }
}

/// Pulls whatever changed since the store's generation. Returns whether the
/// pane needs repainting.
#[cfg(feature = "terminal-runtime")]
fn publish_session_frame(_panel_id: &str, session_id: u64) -> bool {
    let since = super::terminal_grid::session_generation(session_id);
    let image_since = super::terminal_grid::session_image_generation(session_id);
    let update = lingxia_terminal::terminal_render_data(session_id, since, image_since);
    let (scrollbar, exited) =
        lingxia_terminal::terminal_view_state(session_id).unwrap_or((None, false));
    let view_changed = super::terminal_grid::set_session_view_state(session_id, scrollbar, exited);
    match update {
        Some((frame, images)) => {
            let frame = match frame {
                lingxia_terminal::TerminalFrameUpdate::Changed(frame) => Some(*frame),
                lingxia_terminal::TerminalFrameUpdate::Unchanged { .. } => None,
            };
            let changed = frame.is_some() || images.changed;
            super::terminal_grid::set_session_render(
                session_id,
                frame,
                images.changed.then_some(images),
            );
            changed || view_changed
        }
        // The grid is untouched, but the scrollbar or the child's exit still
        // has to reach the screen.
        _ => view_changed,
    }
}

/// Repaints the panel window (the chrome redraws every active pane).
#[cfg(feature = "terminal-runtime")]
fn invalidate_panel(panel_id: &str) {
    lingxia_windows_contract::invalidate_host_panel(panel_id);
}

/// Without the shell chrome there is no grid painter; flatten the focused
/// pane's snapshot to the panel's plain body text as before.
#[cfg(all(feature = "terminal-runtime", not(feature = "shell-chrome")))]
fn publish_windows_terminal_snapshot(panel_id: &str, session_id: u64, snapshot: TerminalSnapshot) {
    // Only the focused session drives the (single) plain-text body.
    if active_session_id(panel_id) == Some(session_id) {
        let _ = lingxia_windows_contract::update_host_panel_body(
            panel_id,
            &windows_terminal_snapshot_body(&snapshot),
        );
    }
}

#[cfg(feature = "terminal-runtime")]
fn windows_terminal_snapshot_body(snapshot: &TerminalSnapshot) -> String {
    let mut lines = snapshot.lines.as_slice();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines = &lines[..lines.len() - 1];
    }
    if lines.is_empty() {
        let title = snapshot
            .title
            .as_deref()
            .or(snapshot.process_title.as_deref())
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .unwrap_or("terminal");
        title.to_string()
    } else {
        lines.join("\n")
    }
}

#[cfg(all(test, feature = "terminal-runtime"))]
mod tests {
    #[cfg(windows)]
    use super::create_panel_session;
    use super::stable_tab_title;
    #[cfg(feature = "shell-chrome")]
    use super::{
        PaneNode, PaneOrientation, SplitDir, pane_close_button_rect, pane_controls_hover_rect,
        pane_drag_handle_rect, pane_drop_direction, point_in_rect,
    };
    #[cfg(windows)]
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    #[cfg(feature = "shell-chrome")]
    use windows::Win32::Foundation::RECT;

    #[test]
    fn stable_process_title_ignores_animated_terminal_title() {
        assert_eq!(stable_tab_title(Some("codex"), Some("⠋ Working")), "codex");
        assert_eq!(stable_tab_title(Some("codex"), Some("⠙ Working")), "codex");
    }

    #[test]
    fn terminal_title_is_only_a_non_empty_fallback() {
        assert_eq!(stable_tab_title(None, Some("vim")), "vim");
        assert_eq!(stable_tab_title(Some("  "), Some("vim")), "vim");
        assert_eq!(stable_tab_title(Some("  "), Some("  ")), "terminal");
    }

    #[cfg(windows)]
    #[test]
    fn panel_sessions_can_inherit_an_active_terminal_directory() {
        let fixture_id = format!(
            "terminal-cwd-inheritance-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let directory = std::env::temp_dir().join(&fixture_id);
        std::fs::create_dir_all(&directory).unwrap();
        let expected = directory.canonicalize().unwrap();
        let source = lingxia_terminal::terminal_create_at(80, 24, Some(&expected));
        if source == 0 {
            let _ = std::fs::remove_dir_all(&directory);
            panic!("source terminal failed to start");
        }

        let inherited = create_panel_session(&fixture_id, Some(source));
        let directory_inherited = inherited != 0 && wait_for_directory(inherited, &expected);

        lingxia_terminal::terminal_close(source);
        if inherited != 0 {
            lingxia_terminal::terminal_close(inherited);
        }
        let _ = std::fs::remove_dir_all(&directory);
        assert!(
            directory_inherited,
            "new session did not inherit {expected:?}"
        );
    }

    #[cfg(windows)]
    fn wait_for_directory(session_id: u64, expected: &std::path::Path) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let current = lingxia_terminal::terminal_current_directory(session_id)
                .and_then(|path| path.canonicalize().ok());
            if current.as_deref() == Some(expected) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        false
    }

    #[cfg(feature = "shell-chrome")]
    #[test]
    fn moving_a_pane_preserves_its_session_and_collapses_the_old_split() {
        let mut root = PaneNode::Leaf(1);
        assert!(root.split(1, 2, SplitDir::Right));
        assert!(root.split(2, 3, SplitDir::Down));

        assert!(root.move_leaf(3, 1, SplitDir::Left));

        assert_eq!(describe(&root), "cols(cols(3,1),2)");
        let mut sessions = Vec::new();
        root.collect(&mut sessions);
        assert_eq!(sessions, [3, 1, 2]);
    }

    #[cfg(feature = "shell-chrome")]
    #[test]
    fn pane_drop_direction_is_normalized_and_has_a_center_dead_zone() {
        let wide = RECT {
            left: 0,
            top: 0,
            right: 1_000,
            bottom: 200,
        };
        assert_eq!(
            pane_drop_direction(wide, 100, 100, None),
            Some(SplitDir::Left)
        );
        assert_eq!(pane_drop_direction(wide, 500, 20, None), Some(SplitDir::Up));
        assert_eq!(pane_drop_direction(wide, 500, 100, None), None);

        let tall = RECT {
            left: 0,
            top: 0,
            right: 200,
            bottom: 1_000,
        };
        assert_eq!(
            pane_drop_direction(tall, 100, 100, None),
            Some(SplitDir::Up)
        );
    }

    #[cfg(feature = "shell-chrome")]
    #[test]
    fn pane_drop_direction_keeps_the_current_edge_near_a_boundary() {
        let square = RECT {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        };
        assert_eq!(
            pane_drop_direction(square, 34, 34, Some(SplitDir::Up)),
            Some(SplitDir::Up)
        );
        assert_eq!(
            pane_drop_direction(square, 20, 34, Some(SplitDir::Up)),
            Some(SplitDir::Left)
        );
    }

    #[cfg(feature = "shell-chrome")]
    #[test]
    fn pane_close_stays_at_trailing_edge_clear_of_drag_handle() {
        let pane = RECT {
            left: 20,
            top: 40,
            right: 420,
            bottom: 300,
        };
        let close = pane_close_button_rect(pane);
        let drag = pane_drag_handle_rect(pane);
        assert_eq!(close.right, pane.right - 4);
        assert!(close.left >= drag.right);
        assert!(close.top >= pane.top);
        assert!(close.bottom <= pane.bottom);
    }

    #[cfg(feature = "shell-chrome")]
    #[test]
    fn pane_controls_reveal_zone_is_limited_to_the_top_edge() {
        let pane = RECT {
            left: 20,
            top: 40,
            right: 420,
            bottom: 300,
        };
        let hover = pane_controls_hover_rect(pane);
        assert_eq!(hover.left, pane.left);
        assert_eq!(hover.top, pane.top);
        assert_eq!(hover.right, pane.right);
        assert_eq!(hover.bottom, pane.top + 32);
        assert!(point_in_rect(hover, 200, pane.top + 31));
        assert!(!point_in_rect(hover, 200, pane.top + 32));
    }

    #[cfg(feature = "shell-chrome")]
    fn describe(node: &PaneNode) -> String {
        match node {
            PaneNode::Leaf(id) => id.to_string(),
            PaneNode::Split {
                orient,
                first,
                second,
                ..
            } => format!(
                "{}({},{})",
                match orient {
                    PaneOrientation::Cols => "cols",
                    PaneOrientation::Rows => "rows",
                },
                describe(first),
                describe(second)
            ),
        }
    }
}
