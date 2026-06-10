//! Native terminal panel sessions for the Windows shell.

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
use lingxia_webview::platform::windows::WindowsPanelPosition;

#[cfg(feature = "terminal-runtime")]
struct WindowsTerminalPanelSession {
    session_id: u64,
    stop: Arc<AtomicBool>,
}

#[cfg(feature = "terminal-runtime")]
static WINDOWS_TERMINAL_PANELS: OnceLock<Mutex<HashMap<String, WindowsTerminalPanelSession>>> =
    OnceLock::new();

#[cfg(feature = "terminal-runtime")]
fn windows_terminal_panels()
-> std::sync::MutexGuard<'static, HashMap<String, WindowsTerminalPanelSession>> {
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
    lingxia_webview::platform::windows::show_native_terminal_panel(
        panel_id,
        title,
        terminal_panel_status_text(),
        position,
    )
    .map_err(|err| err.to_string())
}

pub(super) fn close_windows_terminal_panel(panel_id: &str) -> Result<(), String> {
    #[cfg(feature = "terminal-runtime")]
    close_existing_windows_terminal_session(panel_id);
    lingxia_webview::platform::windows::hide_native_panel(panel_id).map_err(|err| err.to_string())
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

#[cfg(feature = "terminal-runtime")]
fn open_windows_terminal_session_panel(
    panel_id: &str,
    title: &str,
    position: WindowsPanelPosition,
) -> Result<(), String> {
    close_existing_windows_terminal_session(panel_id);
    let session_id = lingxia_terminal::terminal_create(100, 24);
    if session_id == 0 {
        return lingxia_webview::platform::windows::show_native_terminal_panel(
            panel_id,
            title,
            "Terminal failed to start",
            position,
        )
        .map_err(|err| err.to_string());
    }

    if let Err(err) = lingxia_webview::platform::windows::show_native_terminal_panel(
        panel_id,
        title,
        "Starting terminal...",
        position,
    ) {
        lingxia_webview::platform::windows::clear_native_panel_input_handler(panel_id);
        lingxia_terminal::terminal_close(session_id);
        return Err(err.to_string());
    }

    // The webview layer forwards structured key events; terminal escape-
    // sequence knowledge lives in lingxia-terminal's encoder, and the end of
    // the chain stays a plain write-string-to-pty closure.
    let write_session_id = session_id;
    let write_to_pty: Arc<dyn Fn(String) + Send + Sync> = Arc::new(move |input: String| {
        let _ = lingxia_terminal::terminal_write(write_session_id, &input);
    });
    lingxia_webview::platform::windows::set_native_panel_input_handler(
        panel_id,
        Arc::new(
            move |event: lingxia_webview::platform::windows::WindowsPanelKeyEvent| {
                let encoded =
                    lingxia_terminal::encode_key_event(lingxia_terminal::TerminalKeyEvent {
                        vk: event.vk,
                        ctrl: event.ctrl,
                        shift: event.shift,
                        alt: event.alt,
                        character: event.character,
                    });
                match encoded {
                    Some(input) => {
                        write_to_pty(input);
                        true
                    }
                    None => false,
                }
            },
        ),
    );

    let stop = Arc::new(AtomicBool::new(false));
    let panel_key = panel_id.to_string();
    windows_terminal_panels().insert(
        panel_key.clone(),
        WindowsTerminalPanelSession {
            session_id,
            stop: Arc::clone(&stop),
        },
    );

    thread::spawn(move || {
        let mut last_generations = None;
        #[cfg(feature = "shell-runtime")]
        let mut pending_resize: Option<(u16, u16)> = None;
        loop {
            if stop.load(Ordering::Acquire) {
                break;
            }
            let Some(snapshot) = lingxia_terminal::terminal_snapshot_data(session_id) else {
                break;
            };
            let generations = (snapshot.generation, snapshot.title_generation);
            let exited = snapshot.exited;
            #[cfg(feature = "shell-runtime")]
            if !exited {
                let desired =
                    lingxia_shell::windows::terminal_grid::desired_grid_size(&panel_key)
                        .filter(|&(cols, rows)| (cols, rows) != (snapshot.cols, snapshot.rows));
                // Resize the PTY only once the desired grid held for two
                // consecutive ticks, so panel drags don't cause resize storms.
                match desired {
                    Some((cols, rows)) if pending_resize == Some((cols, rows)) => {
                        lingxia_terminal::terminal_resize(session_id, cols, rows);
                        pending_resize = None;
                    }
                    other => pending_resize = other,
                }
            }
            if exited || last_generations != Some(generations) {
                publish_windows_terminal_snapshot(&panel_key, snapshot);
                last_generations = Some(generations);
            }
            if exited {
                break;
            }
            thread::sleep(Duration::from_millis(80));
        }
    });

    Ok(())
}

#[cfg(feature = "terminal-runtime")]
fn close_existing_windows_terminal_session(panel_id: &str) {
    lingxia_webview::platform::windows::clear_native_panel_input_handler(panel_id);
    #[cfg(feature = "shell-runtime")]
    lingxia_shell::windows::terminal_grid::clear_panel(panel_id);
    if let Some(session) = windows_terminal_panels().remove(panel_id) {
        session.stop.store(true, Ordering::Release);
        lingxia_terminal::terminal_close(session.session_id);
    }
}

/// Hands the snapshot to the shell chrome's grid store and repaints the
/// panel. Exited sessions keep their final snapshot in the store; the grid
/// painter renders the `[process exited]` state itself.
#[cfg(all(feature = "terminal-runtime", feature = "shell-runtime"))]
fn publish_windows_terminal_snapshot(panel_id: &str, snapshot: TerminalSnapshot) {
    lingxia_shell::windows::terminal_grid::set_panel_snapshot(panel_id, snapshot);
    lingxia_webview::platform::windows::invalidate_native_panel(panel_id);
}

/// Without the shell chrome there is no grid painter; flatten the snapshot
/// to the panel's plain body text as before.
#[cfg(all(feature = "terminal-runtime", not(feature = "shell-runtime")))]
fn publish_windows_terminal_snapshot(panel_id: &str, snapshot: TerminalSnapshot) {
    let _ = lingxia_webview::platform::windows::update_native_panel_body(
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
        if snapshot.exited {
            format!("{title}\n[process exited]")
        } else {
            title.to_string()
        }
    } else if snapshot.exited {
        format!("{}\n[process exited]", lines.join("\n"))
    } else {
        lines.join("\n")
    }
}
