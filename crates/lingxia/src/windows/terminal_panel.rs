//! Native terminal panel sessions for the Windows shell.
//!
//! Owns all terminal *policy* for the Windows native terminal panel,
//! mirroring the macOS terminal workspace UX:
//!
//! - Multi-tab model: one PTY session per tab ([`TerminalTab`]); the panel
//!   shows the ACTIVE session, inactive sessions keep running. The tab id
//!   surfaced to the chrome is the session id.
//! - `exit` closes: a tab whose session exited is closed and a neighbor
//!   activated; closing the last tab closes the whole panel.
//! - Rename: tab titles default to the session's reported title and can be
//!   overridden per tab (inline rename via the shell's EDIT helper).
//! - Maximize: the panel toggles between its dock height and the whole
//!   content area (mechanics live in lingxia-webview's group layout).
//!
//! The webview layer supplies only generic mechanics (panel rects,
//! tab-strip data, chrome events); the shell layer draws the dock and
//! hosts the inline rename editor.

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

use lingxia_platform::windows::webview_host::WindowsPanelPosition;
#[cfg(feature = "terminal-runtime")]
use lingxia_platform::windows::webview_host::{WindowsHostPanelKeyEvent, WindowsHostPanelTab};
#[cfg(feature = "terminal-runtime")]
use lingxia_terminal::TerminalSnapshot;

/// One terminal tab: a PTY session plus its title state. `auto_title`
/// tracks the session's reported title; `custom_title` (set by rename)
/// wins when present.
#[cfg(feature = "terminal-runtime")]
struct TerminalTab {
    session_id: u64,
    custom_title: Option<String>,
    auto_title: String,
}

#[cfg(feature = "terminal-runtime")]
impl TerminalTab {
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
    /// Stop flag of the panel's poll thread.
    stop: Arc<AtomicBool>,
    /// Tab strip last pushed to the webview layer (pushed only on change).
    published_tabs: Vec<WindowsHostPanelTab>,
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
    if crate::terminal::ghostty_available() {
        return open_windows_terminal_session_panel(panel_id, title, position);
    }
    lingxia_platform::windows::webview_host::show_interactive_host_panel(
        panel_id,
        title,
        terminal_panel_status_text(),
        position,
    )
    .map_err(|err| err.to_string())
}

pub(super) fn close_windows_terminal_panel(panel_id: &str) -> Result<(), String> {
    #[cfg(feature = "terminal-runtime")]
    shutdown_windows_terminal_panel_state(panel_id);
    lingxia_platform::windows::webview_host::hide_host_panel(panel_id)
        .map_err(|err| err.to_string())
}

fn terminal_panel_status_text() -> &'static str {
    #[cfg(feature = "terminal-runtime")]
    {
        if crate::terminal::ghostty_available() {
            "Starting terminal..."
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

/// Makes `tab_id` the panel's active tab and shows its session.
pub(super) fn activate_terminal_tab(panel_id: &str, tab_id: u64) {
    #[cfg(feature = "terminal-runtime")]
    {
        {
            let mut panels = windows_terminal_panels();
            let Some(panel) = panels.get_mut(panel_id) else {
                return;
            };
            let Some(index) = panel.tabs.iter().position(|tab| tab.session_id == tab_id) else {
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

/// Closes `tab_id` (terminating its session) and activates a neighbor;
/// closing the last tab closes the whole panel.
pub(super) fn close_terminal_tab(panel_id: &str, tab_id: u64) {
    #[cfg(feature = "terminal-runtime")]
    close_terminal_tab_session(panel_id, tab_id);
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
            panel.tabs.push(TerminalTab {
                session_id,
                custom_title: None,
                auto_title: String::new(),
            });
            panel.active = panel.tabs.len() - 1;
        }
        publish_tab_strip(panel_id);
        publish_active_snapshot(panel_id);
    }
    #[cfg(not(feature = "terminal-runtime"))]
    let _ = panel_id;
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
        lingxia_platform::windows::webview_host::set_host_panel_maximized(panel_id, maximized);
    }
    #[cfg(not(feature = "terminal-runtime"))]
    let _ = panel_id;
}

/// Starts an inline rename of `tab_id`'s title (shell EDIT helper over the
/// painted title rect). Committing a non-empty text sets the tab's custom
/// title; committing an empty text reverts to the automatic title.
pub(super) fn begin_terminal_tab_rename(panel_id: &str, tab_id: u64) {
    #[cfg(all(feature = "terminal-runtime", feature = "shell-runtime"))]
    {
        let current = {
            let panels = windows_terminal_panels();
            let Some(title) = panels.get(panel_id).and_then(|panel| {
                panel
                    .tabs
                    .iter()
                    .find(|tab| tab.session_id == tab_id)
                    .map(|tab| tab.display_title().to_string())
            }) else {
                return;
            };
            title
        };
        let panel_key = panel_id.to_string();
        lingxia_shell::windows::terminal_grid::begin_tab_rename(
            panel_id,
            tab_id,
            &current,
            Arc::new(move |text: String| {
                set_terminal_tab_custom_title(&panel_key, tab_id, &text);
            }),
        );
    }
    #[cfg(not(all(feature = "terminal-runtime", feature = "shell-runtime")))]
    let _ = (panel_id, tab_id);
}

/// Shows the terminal context menu (Copy / Paste) at the given screen point
/// in response to a right-click on the panel.
pub(super) fn show_terminal_context_menu(
    owner_appid: &str,
    panel_id: &str,
    screen_x: i32,
    screen_y: i32,
) {
    #[cfg(all(feature = "terminal-runtime", feature = "shell-runtime"))]
    {
        let Some(window) = super::shell::owner_window_handle(owner_appid) else {
            return;
        };
        let panel_key = panel_id.to_string();
        lingxia_shell::windows::context_menu::show_context_menu(
            window,
            (screen_x, screen_y),
            vec!["Copy".to_string(), "Paste".to_string()],
            Arc::new(move |index| match index {
                0 => copy_panel_screen_to_clipboard(&panel_key),
                1 => paste_clipboard_into_panel(&panel_key),
                _ => {}
            }),
        );
    }
    #[cfg(not(all(feature = "terminal-runtime", feature = "shell-runtime")))]
    let _ = (owner_appid, panel_id, screen_x, screen_y);
}

/// Copies the active session's visible screen text to the clipboard
/// No cell-level selection support yet; Copy takes the whole screen.
#[cfg(all(feature = "terminal-runtime", feature = "shell-runtime"))]
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
        lingxia_shell::windows::clipboard::set_clipboard_text(&text);
    }
}

/// Pastes the clipboard text into the panel's active session (the context
/// menu's Paste). CRLF/LF normalize to CR, and the text is wrapped in
/// bracketed-paste escapes when the session requests it.
#[cfg_attr(
    not(all(feature = "terminal-runtime", feature = "shell-runtime")),
    allow(dead_code)
)]
pub(super) fn paste_clipboard_into_panel(panel_id: &str) {
    #[cfg(all(feature = "terminal-runtime", feature = "shell-runtime"))]
    {
        let Some(session_id) = active_session_id(panel_id) else {
            return;
        };
        let Some(text) = lingxia_shell::windows::clipboard::clipboard_text() else {
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
    #[cfg(not(all(feature = "terminal-runtime", feature = "shell-runtime")))]
    let _ = panel_id;
}

/// Rename commit: a non-empty text becomes the tab's custom title; empty
/// reverts to the automatic (session-reported) title. Only reachable from
/// the inline rename editor, which needs the shell chrome.
#[cfg(all(feature = "terminal-runtime", feature = "shell-runtime"))]
fn set_terminal_tab_custom_title(panel_id: &str, tab_id: u64, text: &str) {
    {
        let mut panels = windows_terminal_panels();
        let Some(tab) = panels
            .get_mut(panel_id)
            .and_then(|panel| panel.tabs.iter_mut().find(|tab| tab.session_id == tab_id))
        else {
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
    #[cfg(feature = "shell-runtime")]
    let (cols, rows) =
        lingxia_shell::windows::terminal_grid::desired_grid_size(panel_id).unwrap_or((100, 24));
    #[cfg(not(feature = "shell-runtime"))]
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
        return lingxia_platform::windows::webview_host::show_interactive_host_panel(
            panel_id,
            title,
            "Terminal failed to start",
            position,
        )
        .map_err(|err| err.to_string());
    }

    if let Err(err) = lingxia_platform::windows::webview_host::show_interactive_host_panel(
        panel_id,
        title,
        "Starting terminal...",
        position,
    ) {
        lingxia_terminal::terminal_close(session_id);
        return Err(err.to_string());
    }

    // The webview layer forwards structured key events; terminal escape-
    // sequence knowledge lives in lingxia-terminal's encoder. Input always
    // routes to the panel's ACTIVE session at event time.
    let input_panel_key = panel_id.to_string();
    lingxia_platform::windows::webview_host::set_host_panel_input_handler(
        panel_id,
        Arc::new(move |event: WindowsHostPanelKeyEvent| {
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
            tabs: vec![TerminalTab {
                session_id,
                custom_title: None,
                auto_title: String::new(),
            }],
            active: 0,
            maximized: false,
            stop: Arc::clone(&stop),
            published_tabs: Vec::new(),
        },
    );
    publish_tab_strip(&panel_key);

    thread::spawn(move || run_terminal_panel_poll_loop(&panel_key, &stop));
    Ok(())
}

/// Poll loop of one panel: reaps exited sessions (any tab), keeps the
/// active session's PTY grid in sync with the painted panel rect, tracks
/// automatic tab titles, and publishes the active session's snapshot when
/// it changes. Inactive sessions keep running; only their exit flag is
/// checked per tick.
#[cfg(feature = "terminal-runtime")]
fn run_terminal_panel_poll_loop(panel_key: &str, stop: &AtomicBool) {
    let mut last_generations: HashMap<u64, (u64, u64)> = HashMap::new();
    let mut last_active: Option<u64> = None;
    #[cfg(feature = "shell-runtime")]
    let mut pending_resize: Option<(u16, u16)> = None;
    loop {
        if stop.load(Ordering::Acquire) {
            break;
        }
        let sessions: Vec<u64> = {
            let panels = windows_terminal_panels();
            let Some(panel) = panels.get(panel_key) else {
                break;
            };
            panel.tabs.iter().map(|tab| tab.session_id).collect()
        };

        // `exit` closes the tab; the last tab closes the whole panel.
        let mut panel_closed = false;
        for session_id in sessions {
            if lingxia_terminal::terminal_exited(session_id)
                && close_terminal_tab_session(panel_key, session_id)
            {
                panel_closed = true;
                break;
            }
        }
        if panel_closed {
            break;
        }

        let Some(session_id) = active_session_id(panel_key) else {
            break;
        };
        let Some(snapshot) = lingxia_terminal::terminal_snapshot_data(session_id) else {
            if close_terminal_tab_session(panel_key, session_id) {
                break;
            }
            continue;
        };
        if snapshot.exited {
            if close_terminal_tab_session(panel_key, session_id) {
                break;
            }
            continue;
        }

        let switched = last_active != Some(session_id);
        #[cfg(feature = "shell-runtime")]
        {
            if switched {
                pending_resize = None;
            }
            let desired = lingxia_shell::windows::terminal_grid::desired_grid_size(panel_key)
                .filter(|&(cols, rows)| (cols, rows) != (snapshot.cols, snapshot.rows));
            // Resize the PTY only once the desired grid held for two
            // consecutive ticks, so divider drags don't cause resize storms
            // (grow/maximize converges within two ticks as well).
            match desired {
                Some((cols, rows)) if pending_resize == Some((cols, rows)) => {
                    lingxia_terminal::terminal_resize(session_id, cols, rows);
                    pending_resize = None;
                }
                other => pending_resize = other,
            }
        }

        update_auto_title(panel_key, session_id, &snapshot);

        let generations = (snapshot.generation, snapshot.title_generation);
        if switched || last_generations.get(&session_id) != Some(&generations) {
            publish_windows_terminal_snapshot(panel_key, snapshot);
            last_generations.insert(session_id, generations);
        }
        last_active = Some(session_id);
        thread::sleep(Duration::from_millis(80));
    }
}

/// Tracks the session-reported title of a tab and republishes the strip
/// when it changed (no-op while a custom title overrides it).
#[cfg(feature = "terminal-runtime")]
fn update_auto_title(panel_id: &str, session_id: u64, snapshot: &TerminalSnapshot) {
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
        let Some(tab) = panels.get_mut(panel_id).and_then(|panel| {
            panel
                .tabs
                .iter_mut()
                .find(|tab| tab.session_id == session_id)
        }) else {
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

#[cfg(feature = "terminal-runtime")]
fn active_session_id(panel_id: &str) -> Option<u64> {
    let panels = windows_terminal_panels();
    let panel = panels.get(panel_id)?;
    panel.tabs.get(panel.active).map(|tab| tab.session_id)
}

/// Closes one tab's session and removes the tab, activating a neighbor.
/// Returns `true` when this was the last tab and the whole panel was
/// closed (input handler cleared, panel hidden, layout re-synced).
#[cfg(feature = "terminal-runtime")]
fn close_terminal_tab_session(panel_id: &str, session_id: u64) -> bool {
    lingxia_terminal::terminal_close(session_id);
    let now_empty = {
        let mut panels = windows_terminal_panels();
        let Some(panel) = panels.get_mut(panel_id) else {
            return false;
        };
        let Some(index) = panel
            .tabs
            .iter()
            .position(|tab| tab.session_id == session_id)
        else {
            return false;
        };
        panel.tabs.remove(index);
        if panel.tabs.is_empty() {
            true
        } else {
            // Activate the neighbor: the next tab takes the removed slot;
            // tabs before the active one keep the active tab selected.
            if index < panel.active {
                panel.active -= 1;
            }
            if panel.active >= panel.tabs.len() {
                panel.active = panel.tabs.len() - 1;
            }
            false
        }
    };

    if now_empty {
        if let Err(err) = close_windows_terminal_panel(panel_id) {
            log::warn!("failed to close Windows terminal panel {panel_id}: {err}");
        }
        super::shell::sync_owner_shell_layout();
        true
    } else {
        publish_tab_strip(panel_id);
        publish_active_snapshot(panel_id);
        false
    }
}

/// Stops the poll thread, terminates all sessions, and clears the panel's
/// input handler and grid store. The panel window itself is hidden by the
/// caller (`close_windows_terminal_panel`).
#[cfg(feature = "terminal-runtime")]
fn shutdown_windows_terminal_panel_state(panel_id: &str) {
    lingxia_platform::windows::webview_host::clear_host_panel_input_handler(panel_id);
    #[cfg(feature = "shell-runtime")]
    lingxia_shell::windows::terminal_grid::clear_panel(panel_id);
    if let Some(panel) = windows_terminal_panels().remove(panel_id) {
        panel.stop.store(true, Ordering::Release);
        for tab in panel.tabs {
            lingxia_terminal::terminal_close(tab.session_id);
        }
    }
}

// ---- Publishing to the webview/shell layers ----

/// Pushes the panel's tab strip (id/title/active) to the webview layer
/// when it differs from the last published strip.
#[cfg(feature = "terminal-runtime")]
fn publish_tab_strip(panel_id: &str) {
    let strip = {
        let mut panels = windows_terminal_panels();
        let Some(panel) = panels.get_mut(panel_id) else {
            return;
        };
        let strip: Vec<WindowsHostPanelTab> = panel
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| WindowsHostPanelTab {
                id: tab.session_id,
                title: tab.display_title().to_string(),
                active: index == panel.active,
            })
            .collect();
        if strip == panel.published_tabs {
            return;
        }
        panel.published_tabs = strip.clone();
        strip
    };
    lingxia_platform::windows::webview_host::set_host_panel_tabs(panel_id, strip);
}

/// Publishes the active session's current snapshot immediately (tab
/// switches and structural changes shouldn't wait for the next poll tick).
#[cfg(feature = "terminal-runtime")]
fn publish_active_snapshot(panel_id: &str) {
    let Some(session_id) = active_session_id(panel_id) else {
        return;
    };
    let Some(snapshot) = lingxia_terminal::terminal_snapshot_data(session_id) else {
        return;
    };
    publish_windows_terminal_snapshot(panel_id, snapshot);
}

/// Hands the snapshot to the shell chrome's grid store and repaints the
/// panel.
#[cfg(all(feature = "terminal-runtime", feature = "shell-runtime"))]
fn publish_windows_terminal_snapshot(panel_id: &str, snapshot: TerminalSnapshot) {
    lingxia_shell::windows::terminal_grid::set_panel_snapshot(panel_id, snapshot);
    lingxia_platform::windows::webview_host::invalidate_host_panel(panel_id);
}

/// Without the shell chrome there is no grid painter; flatten the snapshot
/// to the panel's plain body text as before.
#[cfg(all(feature = "terminal-runtime", not(feature = "shell-runtime")))]
fn publish_windows_terminal_snapshot(panel_id: &str, snapshot: TerminalSnapshot) {
    let _ = lingxia_platform::windows::webview_host::update_host_panel_body(
        panel_id,
        &windows_terminal_snapshot_body(&snapshot),
    );
}

#[cfg(all(feature = "terminal-runtime", not(feature = "shell-runtime")))]
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
