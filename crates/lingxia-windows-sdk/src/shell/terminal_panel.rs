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
use lingxia_windows_host::WindowsPanelPosition;
#[cfg(feature = "terminal-runtime")]
use lingxia_windows_host::{WindowsHostPanelKeyEvent, WindowsHostPanelTab};
#[cfg(feature = "browser-shell")]
use windows::Win32::Foundation::RECT;

/// Direction a pane splits in, mirroring the macOS surface context menu.
#[cfg(feature = "browser-shell")]
#[cfg_attr(not(feature = "terminal-runtime"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SplitDir {
    Left,
    Right,
    Up,
    Down,
}

/// Pixel thickness of the gap between two sibling panes — a thin hairline
/// divider like ghostty's (the grab area is widened separately for drag).
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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
    #[cfg(feature = "browser-shell")]
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
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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
    if lingxia_terminal::ghostty_available() {
        return open_windows_terminal_session_panel(panel_id, title, position);
    }
    lingxia_windows_host::show_interactive_host_panel(
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
        lingxia_windows_host::show_interactive_host_panel(panel_id, title, &body, position)
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
        if lingxia_terminal::ghostty_available() {
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
        let session_id = create_panel_session(panel_id);
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
    not(all(feature = "terminal-runtime", feature = "browser-shell")),
    allow(dead_code)
)]
pub(super) fn split_focused_pane(panel_id: &str, dir: SplitDir) {
    #[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
    {
        let session_id = create_panel_session(panel_id);
        if session_id == 0 {
            log::warn!("failed to create terminal session to split pane in {panel_id}");
            return;
        }
        let split = {
            let mut panels = windows_terminal_panels();
            let Some(tab) = panels
                .get_mut(panel_id)
                .and_then(|panel| panel.active_tab_mut())
            else {
                lingxia_terminal::terminal_close(session_id);
                return;
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
    }
    #[cfg(not(all(feature = "terminal-runtime", feature = "browser-shell")))]
    let _ = (panel_id, dir);
}

/// Closes the active tab's focused pane; its sibling takes the space. When
/// it was the tab's last pane the tab closes (and the panel, if last tab).
#[cfg_attr(
    not(all(feature = "terminal-runtime", feature = "browser-shell")),
    allow(dead_code)
)]
pub(super) fn close_focused_pane(panel_id: &str) {
    #[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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
    #[cfg(not(all(feature = "terminal-runtime", feature = "browser-shell")))]
    let _ = panel_id;
}

/// Focuses the pane under `(client_x, client_y)` (host-window client
/// coordinates) in the active tab, if a pane covers that point.
#[cfg_attr(
    not(all(feature = "terminal-runtime", feature = "browser-shell")),
    allow(dead_code)
)]
pub(super) fn focus_pane_at(panel_id: &str, client_x: i32, client_y: i32) {
    #[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
    {
        let changed = {
            let mut panels = windows_terminal_panels();
            let Some(panel) = panels.get_mut(panel_id) else {
                return;
            };
            let body = super::terminal_grid::panel_body_rect(panel_id);
            let Some(body) = body else {
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
    #[cfg(not(all(feature = "terminal-runtime", feature = "browser-shell")))]
    let _ = (panel_id, client_x, client_y);
}

/// Toggles the panel between its dock height and the full content area.
pub(super) fn toggle_terminal_panel_maximized(panel_id: &str) {
    #[cfg(feature = "terminal-runtime")]
    {
        let maximized = {
            let mut panels = windows_terminal_panels();
            let Some(panel) = panels.get_mut(panel_id) else {
                return;
            };
            panel.maximized = !panel.maximized;
            panel.maximized
        };
        lingxia_windows_host::set_host_panel_maximized(panel_id, maximized);
    }
    #[cfg(not(feature = "terminal-runtime"))]
    let _ = panel_id;
}

/// Starts an inline rename of `tab_id`'s title (shell EDIT helper over the
/// painted title rect). Committing a non-empty text sets the tab's custom
/// title; committing an empty text reverts to the automatic title.
pub(super) fn begin_terminal_tab_rename(panel_id: &str, tab_id: u64) {
    #[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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
    #[cfg(not(all(feature = "terminal-runtime", feature = "browser-shell")))]
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
    #[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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
    #[cfg(not(all(feature = "terminal-runtime", feature = "browser-shell")))]
    let _ = (owner_appid, panel_id, screen_x, screen_y);
}

/// Dispatches a context-menu selection by matching the chosen label
/// against the localized item list (separators are empty strings).
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
fn toggle_read_only(panel_id: &str) {
    let mut panels = windows_terminal_panels();
    if let Some(panel) = panels.get_mut(panel_id) {
        panel.read_only = !panel.read_only;
    }
}

/// Begins an inline rename of the active tab (the focused session id is the
/// tab id surfaced to the chrome).
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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
    let fresh = create_panel_session(panel_id);
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

/// Copies the active session's visible screen text to the clipboard
/// No cell-level selection support yet; Copy takes the whole screen.
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
fn copy_panel_screen_to_clipboard(panel_id: &str) {
    let Some(session_id) = active_session_id(panel_id) else {
        return;
    };
    let Some(snapshot) = lingxia_terminal::terminal_snapshot_data(session_id) else {
        return;
    };
    let mut lines: Vec<&str> = snapshot.lines.iter().map(|line| line.trim_end()).collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    let text = lines.join("\r\n");
    if !text.is_empty() {
        super::clipboard::set_clipboard_text(&text);
    }
}

/// Pastes the clipboard text into the panel's focused session (the context
/// menu's Paste). CRLF/LF normalize to CR, and the text is wrapped in
/// bracketed-paste escapes when the session requests it.
#[cfg_attr(
    not(all(feature = "terminal-runtime", feature = "browser-shell")),
    allow(dead_code)
)]
pub(super) fn paste_clipboard_into_panel(panel_id: &str) {
    #[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
    {
        let Some(session_id) = active_session_id(panel_id) else {
            return;
        };
        let Some(text) = super::clipboard::clipboard_text() else {
            return;
        };
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        let bracketed = lingxia_terminal::terminal_snapshot_data(session_id)
            .is_some_and(|snapshot| snapshot.bracketed_paste);
        let payload = if bracketed {
            format!("\x1b[200~{text}\x1b[201~")
        } else {
            text
        };
        let _ = lingxia_terminal::terminal_write(session_id, &payload);
    }
    #[cfg(not(all(feature = "terminal-runtime", feature = "browser-shell")))]
    let _ = panel_id;
}

/// Rename commit: a non-empty text becomes the tab's custom title; empty
/// reverts to the automatic (session-reported) title. Only reachable from
/// the inline rename editor, which needs the shell chrome.
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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

/// Creates a PTY session sized to the panel's current grid (falls back to
/// 100x24 before the first paint or without the shell grid store).
#[cfg(feature = "terminal-runtime")]
fn create_panel_session(panel_id: &str) -> u64 {
    let _ = panel_id;
    #[cfg(feature = "browser-shell")]
    let (cols, rows) = super::terminal_grid::desired_panel_grid_size(panel_id).unwrap_or((100, 24));
    #[cfg(not(feature = "browser-shell"))]
    let (cols, rows) = (100, 24);
    lingxia_terminal::terminal_create(cols, rows)
}

#[cfg(feature = "terminal-runtime")]
fn open_windows_terminal_session_panel(
    panel_id: &str,
    title: &str,
    position: WindowsPanelPosition,
) -> Result<(), String> {
    shutdown_windows_terminal_panel_state(panel_id);
    let session_id = create_panel_session(panel_id);
    if session_id == 0 {
        return lingxia_windows_host::show_interactive_host_panel(
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

    if let Err(err) =
        lingxia_windows_host::show_interactive_host_panel(panel_id, title, &initial_body, position)
    {
        lingxia_terminal::terminal_close(session_id);
        return Err(err.to_string());
    }

    // The webview layer forwards structured key events; terminal escape-
    // sequence knowledge lives in lingxia-terminal's encoder. Input always
    // routes to the active tab's FOCUSED pane session at event time.
    let input_panel_key = panel_id.to_string();
    lingxia_windows_host::set_host_panel_input_handler(
        panel_id,
        Arc::new(move |event: WindowsHostPanelKeyEvent| {
            if is_panel_read_only(&input_panel_key) {
                return false;
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
                    let _ = lingxia_terminal::terminal_write(session_id, &input);
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
            read_only: false,
            stop: Arc::clone(&stop),
            published_tabs: Vec::new(),
        },
    );
    publish_tab_strip(&panel_key);
    if let Some(snapshot) = initial_snapshot {
        publish_windows_terminal_snapshot(&panel_key, session_id, snapshot);
    } else {
        publish_active_snapshot(&panel_key);
    }

    thread::spawn(move || run_terminal_panel_poll_loop(&panel_key, &stop));
    Ok(())
}

/// Poll loop of one panel: reaps exited sessions (any pane of any tab),
/// keeps the active tab's pane PTY grids in sync with their painted
/// rects, tracks the focused pane's automatic title, and publishes the
/// active tab's pane snapshots when they change. Inactive tabs keep
/// running; only their exit flag is checked per tick.
#[cfg(feature = "terminal-runtime")]
fn run_terminal_panel_poll_loop(panel_key: &str, stop: &AtomicBool) {
    let mut last_generations: HashMap<u64, (u64, u64)> = HashMap::new();
    let mut last_active_set: Vec<u64> = Vec::new();
    let mut refresh_tick: u32 = 0;
    #[cfg(feature = "browser-shell")]
    let mut pending_resize: HashMap<u64, (u16, u16)> = HashMap::new();
    loop {
        if stop.load(Ordering::Acquire) {
            break;
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
            let Some(snapshot) = lingxia_terminal::terminal_snapshot_data(session_id) else {
                if close_pane_session(panel_key, session_id) {
                    panel_closed = true;
                }
                break;
            };
            if snapshot.exited {
                if close_pane_session(panel_key, session_id) {
                    panel_closed = true;
                }
                break;
            }

            #[cfg(feature = "browser-shell")]
            {
                if switched {
                    pending_resize.remove(&session_id);
                }
                let desired = super::terminal_grid::desired_session_grid_size(session_id)
                    .filter(|&(cols, rows)| (cols, rows) != (snapshot.cols, snapshot.rows));
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

            update_focused_auto_title(panel_key, session_id, &snapshot);

            let generations = (snapshot.generation, snapshot.title_generation);
            if last_generations.get(&session_id) != Some(&generations) {
                any_change = true;
                store_session_snapshot(panel_key, session_id, snapshot);
                last_generations.insert(session_id, generations);
            }
        }
        if panel_closed {
            break;
        }

        refresh_tick = refresh_tick.wrapping_add(1);
        if any_change || refresh_tick.is_multiple_of(25) {
            invalidate_panel(panel_key);
        }
        last_active_set = active_sessions;
        thread::sleep(Duration::from_millis(80));
    }
}

/// Tracks the session-reported title of the active tab's focused pane and
/// republishes the strip when it changed (no-op while a custom title
/// overrides it or the session is not the focused pane).
#[cfg(feature = "terminal-runtime")]
fn update_focused_auto_title(panel_id: &str, session_id: u64, snapshot: &TerminalSnapshot) {
    let auto_title = snapshot
        .title
        .as_deref()
        .or(snapshot.process_title.as_deref())
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .unwrap_or("terminal")
        .to_string();
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

/// Session id that input/copy/paste route to: the active tab's focused pane.
#[cfg(feature = "terminal-runtime")]
fn active_session_id(panel_id: &str) -> Option<u64> {
    let panels = windows_terminal_panels();
    let panel = panels.get(panel_id)?;
    panel.active_tab().map(|tab| tab.focused)
}

/// Number of panes in the active tab.
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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
    lingxia_terminal::terminal_close(session_id);
    #[cfg(feature = "browser-shell")]
    super::terminal_grid::clear_session(session_id);

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
        CloseOutcome::Panel => {
            shutdown_windows_terminal_panel_state(panel_id);
            if let Err(err) =
                lingxia_windows_host::hide_host_panel(panel_id).map_err(|err| err.to_string())
            {
                log::warn!("failed to close Windows terminal panel {panel_id}: {err}");
            }
            super::runtime::sync_owner_shell_layout();
            true
        }
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
    lingxia_windows_host::clear_host_panel_input_handler(panel_id);
    #[cfg(feature = "browser-shell")]
    super::terminal_grid::clear_panel(panel_id);
    if let Some(panel) = windows_terminal_panels().remove(panel_id) {
        panel.stop.store(true, Ordering::Release);
        for tab in panel.tabs {
            for session_id in tab.sessions() {
                #[cfg(feature = "browser-shell")]
                super::terminal_grid::clear_session(session_id);
                lingxia_terminal::terminal_close(session_id);
            }
        }
    }
}

// ---- Pane layout query (used by the grid painter) ----

/// One pane's placement within the panel body, in host client coordinates.
#[cfg(feature = "browser-shell")]
pub(super) struct PaneFrame {
    pub(super) session_id: u64,
    pub(super) rect: RECT,
    pub(super) focused: bool,
}

/// Lays out the active tab's panes within `body` (host client coordinates)
/// and returns each pane's rect plus whether it is the focused pane. Empty
/// when the panel has no terminal state yet.
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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

/// Without the terminal runtime there are no panes to lay out.
#[cfg(all(not(feature = "terminal-runtime"), feature = "browser-shell"))]
pub(super) fn active_pane_frames(_panel_id: &str, _body: RECT) -> Vec<PaneFrame> {
    Vec::new()
}

/// Session id of the active tab's focused pane, used by the chrome painter
/// to pick the card's surface background and fallback body text.
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
pub(super) fn focused_session(panel_id: &str) -> Option<u64> {
    let panels = windows_terminal_panels();
    panels
        .get(panel_id)
        .and_then(|panel| panel.active_tab())
        .map(|tab| tab.focused)
}

/// Without the terminal runtime there is no focused pane.
#[cfg(all(not(feature = "terminal-runtime"), feature = "browser-shell"))]
pub(super) fn focused_session(_panel_id: &str) -> Option<u64> {
    None
}

// ---- Divider drag (resizing split ratios) ----

/// The split divider currently being dragged via the window proc's capture
/// loop.
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
struct ActiveDivider {
    panel_id: String,
    /// Descent path (false = first child, true = second) to the Split node.
    path: Vec<bool>,
    /// Rect of the split node being divided (for ratio math).
    bounds: RECT,
    /// Whether the divider is vertical (a `Cols` split → horizontal drag).
    vertical: bool,
}

#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
static ACTIVE_DIVIDER: OnceLock<Mutex<Option<ActiveDivider>>> = OnceLock::new();

#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
fn active_divider() -> std::sync::MutexGuard<'static, Option<ActiveDivider>> {
    ACTIVE_DIVIDER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Extra grab tolerance (px) around the thin divider gap.
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
const DIVIDER_GRAB: i32 = 3;

/// Collects each split's draggable divider as `(hit_rect, vertical, bounds,
/// path)` over the laid-out tree.
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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

#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
pub(crate) fn divider_orientation_at(panel_id: &str, x: i32, y: i32) -> Option<bool> {
    divider_under(panel_id, x, y).map(|(vertical, ..)| vertical)
}

/// Begins dragging the divider under `(x, y)`. Returns `Some(vertical)` when
/// one was hit (driven by the window proc's capture loop).
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
pub(crate) fn end_divider_drag() {
    *active_divider() = None;
}

/// Navigates to the node at `path` (false = first child, true = second).
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
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

#[cfg(all(not(feature = "terminal-runtime"), feature = "browser-shell"))]
pub(crate) fn divider_orientation_at(_panel_id: &str, _x: i32, _y: i32) -> Option<bool> {
    None
}

#[cfg(all(not(feature = "terminal-runtime"), feature = "browser-shell"))]
pub(crate) fn begin_divider_drag(_panel_id: &str, _x: i32, _y: i32) -> Option<bool> {
    None
}

#[cfg(all(not(feature = "terminal-runtime"), feature = "browser-shell"))]
pub(crate) fn update_divider_drag(_x: i32, _y: i32) {}

#[cfg(all(not(feature = "terminal-runtime"), feature = "browser-shell"))]
pub(crate) fn end_divider_drag() {}

// ---- Publishing to the webview/shell layers ----

/// Pushes the panel's tab strip (id/title/active) to the webview layer
/// when it differs from the last published strip. The tab id surfaced to
/// the chrome is the tab's focused session id.
#[cfg(feature = "terminal-runtime")]
fn publish_tab_strip(panel_id: &str) {
    let strip = {
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
        strip
    };
    lingxia_windows_host::set_host_panel_tabs(panel_id, strip);
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
    for session_id in sessions {
        if let Some(snapshot) = lingxia_terminal::terminal_snapshot_data(session_id) {
            store_session_snapshot(panel_id, session_id, snapshot);
        }
    }
    invalidate_panel(panel_id);
}

/// Stores `session_id`'s snapshot in the grid store and (without the shell
/// chrome) flattens it to the panel's plain body text.
#[cfg(feature = "terminal-runtime")]
fn store_session_snapshot(panel_id: &str, session_id: u64, snapshot: TerminalSnapshot) {
    publish_windows_terminal_snapshot(panel_id, session_id, snapshot);
}

/// Repaints the panel window (the chrome redraws every active pane).
#[cfg(feature = "terminal-runtime")]
fn invalidate_panel(panel_id: &str) {
    lingxia_windows_host::invalidate_host_panel(panel_id);
}

/// Hands the snapshot to the shell chrome's grid store (keyed by session).
#[cfg(all(feature = "terminal-runtime", feature = "browser-shell"))]
fn publish_windows_terminal_snapshot(panel_id: &str, session_id: u64, snapshot: TerminalSnapshot) {
    let _ = panel_id;
    super::terminal_grid::set_session_snapshot(session_id, snapshot);
}

/// Without the shell chrome there is no grid painter; flatten the focused
/// pane's snapshot to the panel's plain body text as before.
#[cfg(all(feature = "terminal-runtime", not(feature = "browser-shell")))]
fn publish_windows_terminal_snapshot(panel_id: &str, session_id: u64, snapshot: TerminalSnapshot) {
    // Only the focused session drives the (single) plain-text body.
    if active_session_id(panel_id) == Some(session_id) {
        let _ = lingxia_windows_host::update_host_panel_body(
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
