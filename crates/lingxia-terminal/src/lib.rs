//! Terminal runtime integration for LingXia hosts.
//!
//! Product terminal mode is intentionally single-path:
//! portable-pty owns process I/O, alacritty_terminal owns terminal
//! emulation.

mod alacritty_vt;
mod kitty;
mod links;
mod osc;
mod paste;
#[cfg(windows)]
mod process_windows;
mod restore;
mod search;
mod shell_integration;
mod theme;

use alacritty_vt::{CursorVisualStyle, PtyWriteCallback, ThemeColors, VtScreen};
// A renderer reads `FrameCell::attrs`, so the bits are part of the contract.
pub use alacritty_vt::{
    ATTR_BOLD, ATTR_DIM, ATTR_HIDDEN, ATTR_INVERSE, ATTR_ITALIC, ATTR_STRIKE, ATTR_UNDERLINE,
};
pub use alacritty_vt::{
    CommandBlock, FrameCell, FrameUpdate as TerminalFrameUpdate,
    LogicalLine as TerminalLogicalLine, RowDamage, TerminalActivity, TerminalEvent,
    TerminalEventBatch, TerminalEventKind, TerminalFrame, TerminalProgress, TerminalProgressState,
    TextView as TerminalTextView, UnderlineStyle as TerminalUnderlineStyle,
};
pub use kitty::{
    TerminalImage, TerminalImagePlacement, TerminalImageSnapshot, UnicodePlaceholder,
    decode_unicode_placeholder, placeholder_diacritic_index,
};
pub use links::{DetectedLink, LinkSource as TerminalLinkSource};
pub use paste::{PasteRisk as TerminalPasteRisk, classify_paste, classify_paste_json};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
pub use restore::{
    DEFAULT_RESTORE_SCROLLBACK_LIMIT, TERMINAL_RESTORE_VERSION, TerminalRestoreError,
    TerminalRestoreState,
};
pub use search::{
    SearchMatch as TerminalSearchMatch, SearchMode as TerminalSearchMode,
    SearchResults as TerminalSearchResults,
};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
#[cfg(any(target_os = "macos", target_os = "ios"))]
use std::ffi::CStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, TrySendError};
use std::sync::{Arc, Condvar, LazyLock, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
pub use theme::{TerminalTheme, TerminalThemeError, parse_hex_rgb};

#[cfg(windows)]
use process_windows::process_cwd;

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);
static SESSIONS: LazyLock<Mutex<HashMap<u64, Arc<Mutex<TerminalSession>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Clone the session handle out of the registry so per-session I/O
/// never holds the global map lock — a blocked write on one session
/// must not freeze the others.
fn session(id: u64) -> Option<Arc<Mutex<TerminalSession>>> {
    SESSIONS.lock().ok()?.get(&id).cloned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalBackend {
    Alacritty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendStatus {
    pub backend: TerminalBackend,
    pub available: bool,
    pub status: &'static str,
}

pub fn backend_status() -> BackendStatus {
    BackendStatus {
        backend: TerminalBackend::Alacritty,
        available: true,
        status: "alacritty terminal emulation ready",
    }
}

pub fn backend_available() -> bool {
    backend_status().available
}

#[cfg(target_os = "windows")]
pub fn terminal_set_conpty_path(path: PathBuf) -> Result<(), String> {
    portable_pty::win::set_conpty_path(path).map_err(|error| format!("{error:?}"))
}

#[cfg(target_os = "windows")]
pub fn terminal_clear_conpty_path() {
    portable_pty::win::clear_conpty_path();
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BackendStatusJson {
    backend: &'static str,
    available: bool,
    status: &'static str,
}

pub fn backend_status_json() -> String {
    let status = backend_status();
    let json = BackendStatusJson {
        backend: "alacritty",
        available: status.available,
        status: status.status,
    };
    serde_json::to_string(&json)
        .unwrap_or_else(|_| r#"{"backend":"alacritty","available":true}"#.to_string())
}

/// Options describing how a terminal session should be created.
///
/// Hosts resolve their own profiles into this spec; the crate never
/// reads user configuration files. `None`/empty fields fall back to the
/// engine defaults: the user's shell in the host process directory with
/// the default scrollback limit.
#[derive(Debug, Clone, Default)]
pub struct TerminalSessionSpec {
    /// Working directory for the spawned program. `None` inherits the
    /// host process directory.
    pub cwd: Option<PathBuf>,
    /// Program to run. `None` resolves the user's shell.
    pub program: Option<String>,
    /// Arguments for `program`. `None` uses the engine defaults for the
    /// resolved program (e.g. `-i` for POSIX shells); `Some(vec![])`
    /// runs the program with no arguments.
    pub args: Option<Vec<String>>,
    /// Environment overlay applied on top of the engine defaults.
    pub env: Vec<(String, String)>,
    /// Scrollback line cap. `None` uses the engine default.
    pub scrollback_limit: Option<usize>,
    /// Enable shell integration: known interactive shells are spawned
    /// so they emit OSC 133 command marks and OSC 7 cwd reports. Only
    /// applies to the engine-default invocation (no explicit `args`);
    /// user rc files are never modified.
    pub shell_integration: bool,
    /// Color scheme for this session. `None` uses the theme set by
    /// [`terminal_set_default_theme`]. Themes are live-swappable
    /// afterwards via [`terminal_set_theme`].
    pub theme: Option<TerminalTheme>,
}

/// Create a cross-platform terminal engine session.
///
/// The engine owns PTY/conpty transport plus alacritty terminal
/// semantics. Platform SDKs should treat the returned JSON snapshots as
/// the stable display contract and keep native code focused on
/// view/input/UX.
pub fn terminal_create(cols: u16, rows: u16) -> u64 {
    terminal_create_at(cols, rows, None)
}

/// Create a terminal session whose shell starts in `cwd`.
///
/// `None` inherits the host process directory. Returns `0` if the PTY, shell,
/// or session registry cannot be initialized, matching [`terminal_create`].
pub fn terminal_create_at(cols: u16, rows: u16, cwd: Option<&Path>) -> u64 {
    terminal_create_with_spec(
        cols,
        rows,
        TerminalSessionSpec {
            cwd: cwd.map(Path::to_path_buf),
            ..TerminalSessionSpec::default()
        },
    )
}

/// Create a terminal session from a full [`TerminalSessionSpec`].
///
/// Returns `0` on failure, matching [`terminal_create`].
pub fn terminal_create_with_spec(cols: u16, rows: u16, spec: TerminalSessionSpec) -> u64 {
    let cols = cols.max(1);
    let rows = rows.max(1);
    let result = TerminalSession::spawn(cols, rows, &spec).and_then(|session| {
        let mut sessions = SESSIONS
            .lock()
            .map_err(|_| "session registry lock poisoned".to_string())?;
        let id = NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed);
        sessions.insert(id, Arc::new(Mutex::new(session)));
        Ok(id)
    });
    match result {
        Ok(id) => id,
        // Single reporting point; 0 is the public error sentinel.
        Err(err) => {
            eprintln!("lingxia terminal create failed: {err}");
            0
        }
    }
}

/// Best-effort current directory of the foreground process, falling back to
/// the session shell. Returns `None` for a closed session or when the platform
/// cannot resolve a process directory.
pub fn terminal_current_directory(id: u64) -> Option<std::path::PathBuf> {
    let session = session(id)?;
    let session = session.lock().ok()?;
    session
        .foreground_process_pid()
        .and_then(process_cwd)
        .or_else(|| session.title_state.shell_pid.and_then(process_cwd))
}

pub fn terminal_write(id: u64, input: &str) -> bool {
    let Some(session) = session(id) else {
        return false;
    };
    let writer = {
        let Ok(session) = session.lock() else {
            return false;
        };
        Arc::clone(&session.writer)
    };
    writer.enqueue(input.as_bytes())
}

pub fn terminal_read(id: u64) -> String {
    let Some(session) = session(id) else {
        return String::new();
    };
    let Ok(mut session) = session.lock() else {
        return String::new();
    };
    session.drain_text()
}

pub fn terminal_snapshot(id: u64) -> String {
    terminal_snapshot_data(id)
        .map(|snapshot| snapshot.to_json())
        .unwrap_or_else(|| TerminalSnapshot::closed().to_json())
}

/// Structured variant of [`terminal_snapshot`]: returns the snapshot
/// data directly instead of its JSON encoding. `None` when the session
/// does not exist (or its lock is poisoned).
pub fn terminal_snapshot_data(session_id: u64) -> Option<TerminalSnapshot> {
    let session = session(session_id)?;
    let mut session = session.lock().ok()?;
    Some(session.drain_snapshot())
}

/// Drain pending semantic events (cwd, title, bell, progress,
/// notification, clipboard, command marks, exit) as JSON.
///
/// Events are monotonically sequenced per session; `dropped` reports
/// queue overflows so hosts know when they fell behind.
pub fn terminal_events_drain(id: u64) -> String {
    terminal_events_drain_data(id)
        .map(|batch| serde_json::to_string(&batch).unwrap_or_else(|_| "{}".to_string()))
        .unwrap_or_else(|| "{}".to_string())
}

/// Structured variant of [`terminal_events_drain`]. `None` when the
/// session does not exist.
pub fn terminal_events_drain_data(id: u64) -> Option<TerminalEventBatch> {
    let session = session(id)?;
    let mut session = session.lock().ok()?;
    Some(session.drain_events())
}

/// Recent command blocks (OSC 133 prompt/input/output extents with
/// exit codes) as a JSON array, oldest first.
pub fn terminal_command_blocks(id: u64) -> String {
    terminal_command_blocks_data(id)
        .map(|blocks| serde_json::to_string(&blocks).unwrap_or_else(|_| "[]".to_string()))
        .unwrap_or_else(|| "[]".to_string())
}

/// Structured variant of [`terminal_command_blocks`].
pub fn terminal_command_blocks_data(id: u64) -> Option<Vec<CommandBlock>> {
    let session = session(id)?;
    let mut session = session.lock().ok()?;
    // Marks arrive with PTY output; flush pending bytes first.
    let bytes = session.drain_bytes();
    if !bytes.is_empty() {
        session.vt.feed(&bytes);
    }
    Some(session.vt.command_blocks())
}

/// The renderer's frame for a session, diffed against the frame the
/// caller last drew.
///
/// This is the path a GPU renderer should take instead of
/// [`terminal_snapshot`]: cells are fixed-size records over one text
/// blob (no per-cell allocation, no JSON), and `damage` names the rows
/// that actually changed, so a quiet poll costs nothing and a busy one
/// uploads only what moved. Pass `0` for the first frame; afterwards
/// pass the `generation` of the frame you last drew.
pub fn terminal_frame_data(id: u64, since_generation: u64) -> Option<TerminalFrameUpdate> {
    let session = session(id)?;
    let mut session = session.lock().ok()?;
    let bytes = session.drain_bytes();
    if !bytes.is_empty() {
        session.vt.feed(&bytes);
    }
    Some(session.vt.frame(since_generation))
}

/// Sample a renderer frame and its Kitty image state under one session lock.
///
/// In-process hosts should prefer this over separate frame/image calls: a PTY
/// writer may enqueue output between two calls, while this pair always
/// describes one published synchronized-update boundary.
pub fn terminal_render_data(
    id: u64,
    since_frame_generation: u64,
    since_image_generation: u64,
) -> Option<(TerminalFrameUpdate, TerminalImageSnapshot)> {
    let session = session(id)?;
    let mut session = session.lock().ok()?;
    let bytes = session.drain_bytes();
    if !bytes.is_empty() {
        session.vt.feed(&bytes);
    }
    Some((
        session.vt.frame(since_frame_generation),
        session.vt.image_snapshot(since_image_generation),
    ))
}

/// A session's retained frame, described by raw pointers for hosts that
/// read it across an FFI boundary.
///
/// The buffers belong to the session and stay valid until the next
/// [`terminal_frame_view`] call for the same session or until it closes,
/// whichever comes first — copy what you keep before either happens.
/// Titles are deliberately absent: computing them costs process lookups,
/// which have no place on a render-rate poll (see
/// [`terminal_title_state_json`]).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TerminalFrameView {
    /// False when nothing changed since `since_generation`; every other
    /// field is then stale and the renderer should keep its last frame.
    pub changed: bool,
    pub full_damage: bool,
    pub generation: u64,
    pub image_generation: u64,
    pub cols: u16,
    pub rows: u16,
    pub cells: *const FrameCell,
    pub cells_len: usize,
    /// UTF-8 cluster blob addressed by `FrameCell::text_offset/len`.
    pub text: *const u8,
    pub text_len: usize,
    pub damage: *const RowDamage,
    pub damage_len: usize,
    pub default_fg: u32,
    pub default_bg: u32,
    pub cursor_col: u16,
    pub cursor_row: u16,
    pub cursor_visible: bool,
    /// 0 block, 1 bar, 2 underline, 3 hollow block.
    pub cursor_style: u8,
    pub application_cursor: bool,
    pub bracketed_paste: bool,
    pub alternate_screen: bool,
    pub scrollbar_total: u64,
    pub scrollbar_offset: u64,
    pub scrollbar_len: u64,
    pub exited: bool,
}

impl TerminalFrameView {
    fn unchanged(generation: u64, image_generation: u64, exited: bool) -> Self {
        Self {
            changed: false,
            full_damage: false,
            generation,
            image_generation,
            cols: 0,
            rows: 0,
            cells: std::ptr::null(),
            cells_len: 0,
            text: std::ptr::null(),
            text_len: 0,
            damage: std::ptr::null(),
            damage_len: 0,
            default_fg: 0,
            default_bg: 0,
            cursor_col: 0,
            cursor_row: 0,
            cursor_visible: false,
            cursor_style: 0,
            application_cursor: false,
            bracketed_paste: false,
            alternate_screen: false,
            scrollbar_total: 0,
            scrollbar_offset: 0,
            scrollbar_len: 0,
            // A shell that exits without writing leaves the grid
            // untouched, so this has to be answered even when there is
            // no new frame — otherwise the pane never tears down.
            exited,
        }
    }
}

/// Produce the next frame and retain it, returning pointers into its
/// buffers. This is [`terminal_frame_data`] for FFI hosts: nothing is
/// copied, serialized, or allocated per cell.
/// Scroll position and the child's exit state — the two things a renderer
/// needs that a frame deliberately leaves out.
///
/// A frame describes the grid; neither of these is part of it, and both change
/// on their own schedule. Kept out of [`terminal_frame_data`] so asking for a
/// frame stays "what changed on screen".
pub fn terminal_view_state(id: u64) -> Option<(Option<TerminalScrollbar>, bool)> {
    let session = session(id)?;
    let mut session = session.lock().ok()?;
    let exited = session.poll_child().is_some();
    let scrollbar = session.vt.scrollbar().map(|bar| TerminalScrollbar {
        total: bar.total,
        offset: bar.offset,
        len: bar.len,
    });
    Some((scrollbar, exited))
}

pub fn terminal_frame_view(id: u64, since_generation: u64) -> Option<TerminalFrameView> {
    let session = session(id)?;
    let mut session = session.lock().ok()?;
    let bytes = session.drain_bytes();
    if !bytes.is_empty() {
        session.vt.feed(&bytes);
    }
    match session.vt.frame(since_generation) {
        TerminalFrameUpdate::Unchanged { generation } => Some(TerminalFrameView::unchanged(
            generation,
            session.vt.image_generation(),
            session.poll_child().is_some(),
        )),
        TerminalFrameUpdate::Changed(frame) => {
            let scrollbar = session.vt.scrollbar().unwrap_or_default();
            let view = TerminalFrameView {
                changed: true,
                full_damage: frame.full_damage,
                generation: frame.generation,
                image_generation: session.vt.image_generation(),
                cols: frame.cols,
                rows: frame.rows,
                cells: frame.cells.as_ptr(),
                cells_len: frame.cells.len(),
                text: frame.text.as_ptr(),
                text_len: frame.text.len(),
                damage: frame.damage.as_ptr(),
                damage_len: frame.damage.len(),
                default_fg: frame.default_fg,
                default_bg: frame.default_bg,
                cursor_col: frame.cursor.col,
                cursor_row: frame.cursor.row,
                cursor_visible: frame.cursor.visible,
                cursor_style: match frame.cursor.style {
                    CursorVisualStyle::Block => 0,
                    CursorVisualStyle::Bar => 1,
                    CursorVisualStyle::Underline => 2,
                    CursorVisualStyle::BlockHollow => 3,
                },
                application_cursor: session.vt.is_decckm(),
                bracketed_paste: session.vt.is_bracketed_paste(),
                alternate_screen: session.vt.is_alternate_screen(),
                scrollbar_total: scrollbar.total,
                scrollbar_offset: scrollbar.offset,
                scrollbar_len: scrollbar.len,
                exited: session.poll_child().is_some(),
            };
            // Retain the buffers the pointers address; the previous
            // frame is dropped here, which is what bounds their life.
            session.last_frame = Some(frame);
            Some(view)
        }
    }
}

/// Process/window title state, for hosts polling it at a lower rate than
/// frames: resolving it walks the foreground process, so it must not sit on
/// the render path.
pub fn terminal_title_state_data(id: u64) -> Option<TerminalTitleView> {
    let session = session(id)?;
    let mut session = session.lock().ok()?;
    let foreground_pid = session.foreground_process_pid();
    let alternate_screen = session.vt.is_alternate_screen();
    Some(TerminalTitleView {
        process_title: session.title_state.title(foreground_pid, alternate_screen),
        title: session
            .vt
            .osc_title()
            .map(|title| title.trim().to_string())
            .filter(|title| !title.is_empty()),
        generation: session.title_state.generation(),
    })
}

/// What a tab shows for a session, and the generation that says whether it
/// moved since the caller last looked.
#[derive(Debug, Clone, Default)]
pub struct TerminalTitleView {
    pub process_title: String,
    pub title: Option<String>,
    pub generation: u64,
}

/// [`terminal_title_state_data`] as JSON, for hosts reached over FFI.
pub fn terminal_title_state_json(id: u64) -> String {
    let Some(session) = session(id) else {
        return "{}".to_string();
    };
    let Ok(mut session) = session.lock() else {
        return "{}".to_string();
    };
    let foreground_pid = session.foreground_process_pid();
    let alternate_screen = session.vt.is_alternate_screen();
    let process_title = session.title_state.title(foreground_pid, alternate_screen);
    let state = TerminalTitleStateJson {
        process_title,
        title: session
            .vt
            .osc_title()
            .map(|title| title.trim().to_string())
            .filter(|title| !title.is_empty()),
        title_generation: session.title_state.generation(),
    };
    serde_json::to_string(&state).unwrap_or_else(|_| "{}".to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalTitleStateJson {
    process_title: String,
    title: Option<String>,
    title_generation: u64,
}

/// Set the theme new sessions inherit when their spec carries none.
///
/// Existing sessions keep their theme; use [`terminal_set_theme_all`]
/// to switch everything at once.
pub fn terminal_set_default_theme(theme: &TerminalTheme) -> Result<(), TerminalThemeError> {
    let colors = theme.to_colors()?;
    if let Ok(mut default) = DEFAULT_THEME.lock() {
        *default = colors;
    }
    Ok(())
}

/// Swap one session's theme in place.
///
/// Colors are resolved at snapshot time, so this is a repaint — the
/// grid, scrollback and running program are untouched, which is what
/// makes live theme preview free. The session's generation bumps so a
/// polling host picks the new colors up on its next frame.
pub fn terminal_set_theme(id: u64, theme: &TerminalTheme) -> Result<bool, TerminalThemeError> {
    let colors = theme.to_colors()?;
    let Some(session) = session(id) else {
        return Ok(false);
    };
    let Ok(session) = session.lock() else {
        return Ok(false);
    };
    session.vt.set_theme(colors);
    Ok(true)
}

/// Swap the theme of every live session and of sessions created later.
/// Returns how many live sessions were updated.
pub fn terminal_set_theme_all(theme: &TerminalTheme) -> Result<usize, TerminalThemeError> {
    let colors = theme.to_colors()?;
    if let Ok(mut default) = DEFAULT_THEME.lock() {
        *default = colors.clone();
    }
    // Copy the handles out before touching per-session locks; holding
    // the registry lock while each session repaints would serialize
    // them behind one another.
    let sessions: Vec<Arc<Mutex<TerminalSession>>> = SESSIONS
        .lock()
        .map(|sessions| sessions.values().cloned().collect())
        .unwrap_or_default();
    let mut updated = 0;
    for session in sessions {
        if let Ok(session) = session.lock() {
            session.vt.set_theme(colors.clone());
            updated += 1;
        }
    }
    Ok(updated)
}

/// [`terminal_set_theme`] from a scheme JSON document, for hosts that
/// cross an FFI boundary. Returns `false` for an unparsable scheme,
/// an invalid color, or an unknown session.
pub fn terminal_set_theme_json(id: u64, scheme_json: &str) -> bool {
    match TerminalTheme::from_json(scheme_json) {
        Ok(theme) => terminal_set_theme(id, &theme).unwrap_or_else(|err| {
            eprintln!("lingxia terminal theme rejected: {err}");
            false
        }),
        Err(err) => {
            eprintln!("lingxia terminal theme rejected: {err}");
            false
        }
    }
}

/// [`terminal_set_theme_all`] from a scheme JSON document. Returns
/// `false` for an unparsable scheme or an invalid color.
pub fn terminal_set_theme_all_json(scheme_json: &str) -> bool {
    let theme = match TerminalTheme::from_json(scheme_json) {
        Ok(theme) => theme,
        Err(err) => {
            eprintln!("lingxia terminal theme rejected: {err}");
            return false;
        }
    };
    match terminal_set_theme_all(&theme) {
        Ok(_) => true,
        Err(err) => {
            eprintln!("lingxia terminal theme rejected: {err}");
            false
        }
    }
}

/// Logical lines, cursor position and visible range as JSON, for
/// accessibility trees.
///
/// `start_line` is an absolute line (oldest scrollback line = 0);
/// negative means "from the first visible line". At most `max_lines`
/// logical lines are returned, so a screen reader never pulls the whole
/// scrollback. Selection stays with the host — it owns the gesture and
/// the mapping to screen geometry.
pub fn terminal_text_view(id: u64, start_line: i64, max_lines: usize) -> String {
    let start = (start_line >= 0).then_some(start_line);
    terminal_text_view_data(id, start, max_lines)
        .map(|view| serde_json::to_string(&view).unwrap_or_else(|_| "{}".to_string()))
        .unwrap_or_else(|| "{}".to_string())
}

/// Structured variant of [`terminal_text_view`]. `start_line` of `None`
/// starts at the first visible line.
pub fn terminal_text_view_data(
    id: u64,
    start_line: Option<i64>,
    max_lines: usize,
) -> Option<TerminalTextView> {
    let session = session(id)?;
    let mut session = session.lock().ok()?;
    let bytes = session.drain_bytes();
    if !bytes.is_empty() {
        session.vt.feed(&bytes);
    }
    Some(session.vt.text_view(start_line, max_lines.max(1)))
}

/// Progress, attention and lifecycle state of a session, in the form
/// hosts render as tab badges.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalStatus {
    #[serde(flatten)]
    pub activity: TerminalActivity,
    pub exited: bool,
    /// The child's exit code, once it has exited.
    pub exit_code: Option<i32>,
}

/// Session status (progress state, bell/notification counters, exit) as
/// JSON.
///
/// This is the resync path for badges: the event stream carries the
/// same information incrementally, but a host that missed events —
/// dropped queue, re-attach — reads current truth here instead of
/// guessing from output text.
pub fn terminal_status(id: u64) -> String {
    terminal_status_data(id)
        .map(|status| serde_json::to_string(&status).unwrap_or_else(|_| "{}".to_string()))
        .unwrap_or_else(|| "{}".to_string())
}

/// Structured variant of [`terminal_status`]. `None` when the session
/// does not exist.
pub fn terminal_status_data(id: u64) -> Option<TerminalStatus> {
    let session = session(id)?;
    let mut session = session.lock().ok()?;
    // Progress marks arrive with PTY output; flush pending bytes first.
    let bytes = session.drain_bytes();
    if !bytes.is_empty() {
        session.vt.feed(&bytes);
    }
    let activity = session.vt.activity();
    let status = session.poll_child();
    Some(TerminalStatus {
        activity,
        exited: status.is_some(),
        exit_code: status.flatten(),
    })
}

/// Search the complete logical scrollback as JSON.
///
/// `mode` is `"plain"` (case-insensitive), `"case"`, or `"regex"`.
/// Match ranges are absolute (line, cell column) coordinates. The grid
/// is copied under a short session lock and matching happens after it
/// is released, so search never stalls PTY processing or other
/// sessions.
pub fn terminal_search(id: u64, pattern: &str, mode: &str, max_matches: usize) -> String {
    let mode = match mode {
        "case" => TerminalSearchMode::CaseSensitive,
        "word" => TerminalSearchMode::WholeWord,
        "case-word" => TerminalSearchMode::CaseSensitiveWholeWord,
        "regex" => TerminalSearchMode::Regex,
        _ => TerminalSearchMode::Plain,
    };
    terminal_search_data(id, pattern, mode, max_matches)
        .map(|results| serde_json::to_string(&results).unwrap_or_else(|_| "{}".to_string()))
        .unwrap_or_else(|| "{}".to_string())
}

/// Structured variant of [`terminal_search`].
pub fn terminal_search_data(
    id: u64,
    pattern: &str,
    mode: TerminalSearchMode,
    max_matches: usize,
) -> Option<TerminalSearchResults> {
    let session = session(id)?;
    let (rows, cancel) = {
        let mut session = session.lock().ok()?;
        let bytes = session.drain_bytes();
        if !bytes.is_empty() {
            session.vt.feed(&bytes);
        }
        session.search_cancel.store(false, Ordering::Relaxed);
        (session.vt.grid_text(), Arc::clone(&session.search_cancel))
    };
    Some(search::search_rows(
        &rows,
        pattern,
        mode,
        max_matches.max(1),
        &cancel,
    ))
}

/// Cancel a running search on the session; the search returns promptly
/// with `cancelled: true` and the matches gathered so far.
pub fn terminal_search_cancel(id: u64) {
    if let Some(session) = session(id)
        && let Ok(session) = session.lock()
    {
        session.search_cancel.store(true, Ordering::Relaxed);
    }
}

/// Move the viewport so an absolute retained line is visible.
pub fn terminal_scroll_to_line(id: u64, line: i64) -> bool {
    let Some(session) = session(id) else {
        return false;
    };
    let Ok(mut session) = session.lock() else {
        return false;
    };
    let bytes = session.drain_bytes();
    if !bytes.is_empty() {
        session.vt.feed(&bytes);
    }
    session.vt.scroll_viewport_to_line(line)
}

/// Return image placements changed after the supplied generation as JSON.
pub fn terminal_image_snapshot(id: u64, since_generation: u64) -> String {
    let Some(session) = session(id) else {
        return "{}".to_string();
    };
    let Ok(mut session) = session.lock() else {
        return "{}".to_string();
    };
    let bytes = session.drain_bytes();
    if !bytes.is_empty() {
        session.vt.feed(&bytes);
    }
    serde_json::to_string(&session.vt.image_snapshot(since_generation))
        .unwrap_or_else(|_| "{}".to_string())
}

/// A clickable link on the live screen: explicit OSC 8 hyperlinks and
/// heuristic URL/path detections with cwd-resolved targets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalLink {
    /// Absolute line (oldest scrollback line = 0).
    pub line: i64,
    pub start_col: u16,
    /// Exclusive end cell column.
    pub end_col: u16,
    /// URL verbatim or the cwd-resolved normalized path.
    pub target: String,
    pub source: TerminalLinkSource,
    /// `:line[:column]` suffix parsed from a path target, when present.
    pub target_line: Option<u32>,
    pub target_column: Option<u32>,
}

/// Links visible on the session's live screen as JSON.
pub fn terminal_links(id: u64) -> String {
    terminal_links_data(id)
        .map(|links| serde_json::to_string(&links).unwrap_or_else(|_| "[]".to_string()))
        .unwrap_or_else(|| "[]".to_string())
}

/// Structured variant of [`terminal_links`].
///
/// Heuristic path targets resolve against the OSC 7-reported cwd,
/// falling back to the foreground process directory. OSC 8 ranges are
/// authoritative: heuristic matches overlapping them are dropped.
pub fn terminal_links_data(id: u64) -> Option<Vec<TerminalLink>> {
    let session = session(id)?;
    let mut session = session.lock().ok()?;
    let bytes = session.drain_bytes();
    if !bytes.is_empty() {
        session.vt.feed(&bytes);
    }
    let cwd = session
        .vt
        .cwd()
        .or_else(|| session.foreground_process_pid().and_then(process_cwd))
        .or_else(|| session.title_state.shell_pid.and_then(process_cwd));

    let mut result: Vec<TerminalLink> = Vec::new();
    for row in session.vt.screen_text() {
        for detected in links::detect_links(&row.text, cwd.as_deref()) {
            let start_col = row
                .cells
                .get(detected.start)
                .map(|&(col, _)| col)
                .unwrap_or(0);
            let end_col = if detected.end > detected.start {
                row.cells
                    .get(detected.end - 1)
                    .map(|&(col, width)| col.saturating_add(u16::from(width)))
                    .unwrap_or(start_col)
            } else {
                start_col
            };
            result.push(TerminalLink {
                line: row.line,
                start_col,
                end_col,
                target: detected.target,
                source: TerminalLinkSource::Heuristic,
                target_line: detected.line,
                target_column: detected.column,
            });
        }
    }

    let mut osc8_ranges: Vec<(i64, u16, u16)> = Vec::new();
    for (line, start_col, end_col, uri) in session.vt.screen_hyperlinks() {
        osc8_ranges.push((line, start_col, end_col));
        result.push(TerminalLink {
            line,
            start_col,
            end_col,
            target: uri,
            source: TerminalLinkSource::Osc8,
            target_line: None,
            target_column: None,
        });
    }
    result.retain(|link| {
        link.source == TerminalLinkSource::Osc8
            || !osc8_ranges.iter().any(|&(line, start, end)| {
                line == link.line && start < link.end_col && link.start_col < end
            })
    });
    result.sort_by_key(|link| (link.line, link.start_col));
    Some(result)
}

/// Export the session's restorable state: cwd, title, a host-supplied
/// profile reference, and plain-text scrollback clipped to
/// `max_scrollback_bytes` (`0` selects the default budget).
pub fn terminal_export_restore_state(
    id: u64,
    profile_id: Option<&str>,
    max_scrollback_bytes: usize,
) -> Option<TerminalRestoreState> {
    let session = session(id)?;
    let mut session = session.lock().ok()?;
    let bytes = session.drain_bytes();
    if !bytes.is_empty() {
        session.vt.feed(&bytes);
    }
    let budget = if max_scrollback_bytes == 0 {
        DEFAULT_RESTORE_SCROLLBACK_LIMIT
    } else {
        max_scrollback_bytes
    };
    let (scrollback, truncated) = restore::clip_scrollback(
        session
            .vt
            .grid_text()
            .into_iter()
            .map(|row| row.text)
            .collect(),
        budget,
    );
    let cwd = session
        .vt
        .cwd()
        .or_else(|| session.foreground_process_pid().and_then(process_cwd))
        .or_else(|| session.title_state.shell_pid.and_then(process_cwd));
    let title = session.vt.snapshot().title;
    Some(TerminalRestoreState {
        version: TERMINAL_RESTORE_VERSION,
        cwd,
        title,
        profile_id: profile_id.map(str::to_string),
        scrollback,
        truncated,
    })
}

/// Create a session from a spec and replay a validated restore state
/// into it. The shell starts fresh from a clean emulator state; the
/// restored scrollback precedes its output, and a `Restored` event
/// marks the content boundary. Unknown schema versions fail with
/// [`TerminalRestoreError::UnknownVersion`] instead of being misread.
pub fn terminal_create_with_restore(
    cols: u16,
    rows: u16,
    spec: TerminalSessionSpec,
    restore: &TerminalRestoreState,
) -> Result<u64, TerminalRestoreError> {
    restore.validate()?;
    let id = terminal_create_with_spec(cols, rows, spec);
    if id == 0 {
        return Err(TerminalRestoreError::Invalid(
            "session spawn failed".to_string(),
        ));
    }
    if !restore.scrollback.is_empty()
        && let Some(session) = session(id)
        && let Ok(session) = session.lock()
    {
        let mut replay = restore.scrollback.join("\r\n");
        replay.push_str("\r\n");
        session.vt.feed(replay.as_bytes());
        session.vt.push_event(TerminalEventKind::Restored {
            lines: restore.scrollback.len(),
        });
    }
    Ok(id)
}

pub fn terminal_exited(id: u64) -> bool {
    let Some(session) = session(id) else {
        return true;
    };
    let Ok(mut session) = session.lock() else {
        return true;
    };
    session.exited()
}

pub fn terminal_resize(id: u64, cols: u16, rows: u16) -> bool {
    terminal_resize_pixels(id, cols, rows, 1, 1)
}

pub fn terminal_resize_pixels(
    id: u64,
    cols: u16,
    rows: u16,
    cell_width: u16,
    cell_height: u16,
) -> bool {
    let Some(session) = session(id) else {
        return false;
    };
    let Ok(mut session) = session.lock() else {
        return false;
    };
    session
        .resize(
            cols.max(1),
            rows.max(1),
            cell_width.max(1),
            cell_height.max(1),
        )
        .is_ok()
}

/// Handle vertical scroll input at a viewport cell.
///
/// Negative values move up and positive values move down. Applications that
/// requested mouse reporting receive wheel events at `(col, row)`; alternate
/// screens with mode 1007 receive cursor keys. Otherwise this moves the native
/// scrollback viewport. Read-only hosts set `allow_application_input` to false
/// so scrolling never writes to the PTY.
pub fn terminal_scroll(
    id: u64,
    delta_rows: i32,
    col: u16,
    row: u16,
    allow_application_input: bool,
) -> bool {
    if delta_rows == 0 {
        return false;
    }
    let Some(session) = session(id) else {
        return false;
    };
    let Ok(mut session) = session.lock() else {
        return false;
    };
    session.scroll(delta_rows, col, row, allow_application_input)
}

pub fn terminal_close(id: u64) {
    if let Ok(mut sessions) = SESSIONS.lock() {
        sessions.remove(&id);
    }
}

/// A structured key event from a host window, to be encoded into the byte
/// sequence a PTY expects.
///
/// Either `character` is set (translated character input, e.g. `WM_CHAR` on
/// Windows) or `vk` carries a Windows virtual-key code for raw key-down
/// input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalKeyEvent {
    /// Virtual-key code for key-down events; `0` for character events.
    pub vk: u32,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Translated character for character events.
    pub character: Option<char>,
}

/// Encodes a host key event into the string to write to a terminal PTY.
///
/// Character input maps backspace to DEL and passes printable characters
/// through; key-down input maps arrow/delete keys to ANSI escape sequences.
/// Returns `None` when the event has no terminal encoding (the caller
/// should leave the originating window message unhandled).
pub fn encode_key_event(event: TerminalKeyEvent) -> Option<String> {
    if let Some(character) = event.character {
        let mut encoded = match character as u32 {
            0x08 => Some("\u{7f}".to_string()),
            0x09 => Some("\t".to_string()),
            0x0d => Some("\r".to_string()),
            0x1b => Some("\u{1b}".to_string()),
            0x01..=0x09 | 0x0b..=0x1a => Some(character.to_string()),
            _ if !character.is_control() => Some(character.to_string()),
            _ => None,
        }?;
        // AltGr reports Ctrl+Alt while producing a translated character;
        // only a standalone Alt modifier is terminal Meta/ESC.
        if event.alt && !event.ctrl {
            encoded.insert(0, '\u{1b}');
        }
        return Some(encoded);
    }

    let sequence = match event.vk {
        0x25 => "\u{1b}[D",  // VK_LEFT
        0x26 => "\u{1b}[A",  // VK_UP
        0x27 => "\u{1b}[C",  // VK_RIGHT
        0x28 => "\u{1b}[B",  // VK_DOWN
        0x2e => "\u{1b}[3~", // VK_DELETE
        _ => return None,
    };
    Some(sequence.to_string())
}

struct TerminalSession {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    writer: Arc<PtyWriterQueue>,
    output: Receiver<Vec<u8>>,
    vt: VtScreen,
    title_state: TerminalTitleState,
    exit_event_sent: bool,
    /// Cached child status: `try_wait` reaps the child, so the outcome
    /// is kept here for every later reader. `Some(None)` means exited
    /// with no readable code.
    child_status: Option<Option<i32>>,
    /// Frame retained so a host can read its buffers by pointer until
    /// its next frame call.
    last_frame: Option<Box<TerminalFrame>>,
    search_cancel: Arc<AtomicBool>,
    _reader: thread::JoinHandle<()>,
    _writer: thread::JoinHandle<()>,
}

const PTY_WRITE_CHUNK_SIZE: usize = 4096;
const PTY_WRITE_QUEUE_CAPACITY: usize = 4 * 1024 * 1024;

#[derive(Default)]
struct PtyWriterQueueState {
    pending: VecDeque<Vec<u8>>,
    pending_bytes: usize,
    closed: bool,
}

#[derive(Default)]
struct PtyWriterQueue {
    state: Mutex<PtyWriterQueueState>,
    ready: Condvar,
}

impl PtyWriterQueue {
    fn enqueue(&self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return true;
        }
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.closed
            || bytes.len() > PTY_WRITE_QUEUE_CAPACITY
            || state.pending_bytes > PTY_WRITE_QUEUE_CAPACITY - bytes.len()
        {
            return false;
        }
        state.pending_bytes += bytes.len();
        state.pending.push_back(bytes.to_vec());
        self.ready.notify_one();
        true
    }

    fn run(&self, mut writer: Box<dyn Write + Send>) {
        loop {
            let bytes = {
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                while state.pending.is_empty() && !state.closed {
                    state = self
                        .ready
                        .wait(state)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
                if state.closed {
                    return;
                }
                state.pending.pop_front().unwrap_or_default()
            };

            let result = write_pty_chunked(writer.as_mut(), &bytes);
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.pending_bytes = state.pending_bytes.saturating_sub(bytes.len());
            if result.is_err() {
                state.closed = true;
                state.pending.clear();
                state.pending_bytes = 0;
                self.ready.notify_all();
                return;
            }
        }
    }

    fn close(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.closed = true;
        state.pending.clear();
        state.pending_bytes = 0;
        self.ready.notify_all();
    }
}

fn write_pty_chunked(writer: &mut dyn Write, bytes: &[u8]) -> std::io::Result<()> {
    for chunk in bytes.chunks(PTY_WRITE_CHUNK_SIZE) {
        writer.write_all(chunk)?;
    }
    writer.flush()
}

struct TerminalTitleState {
    shell_pid: Option<u32>,
    shell_title: String,
    current_title: String,
    candidate: Option<ForegroundCandidate>,
    generation: u64,
}

struct ForegroundCandidate {
    pid: u32,
    name: String,
    first_seen: Instant,
}

impl TerminalTitleState {
    const PROMOTION_DELAY: Duration = Duration::from_millis(700);

    fn new(shell_pid: Option<u32>, shell_title: String) -> Self {
        let current_title =
            current_directory_title(shell_pid).unwrap_or_else(|| shell_title.clone());
        Self {
            shell_pid,
            shell_title,
            current_title,
            candidate: None,
            generation: 0,
        }
    }

    fn title(&mut self, foreground_pid: Option<u32>, alternate_screen: bool) -> String {
        let shell_title =
            current_directory_title(self.shell_pid).unwrap_or_else(|| self.shell_title.clone());

        let Some(pid) = foreground_pid.filter(|pid| Some(*pid) != self.shell_pid) else {
            self.candidate = None;
            self.set_current_title(shell_title);
            return self.current_title.clone();
        };

        let Some(name) =
            process_name(pid).filter(|name| !looks_like_shell_title(name, &self.shell_title))
        else {
            self.candidate = None;
            self.set_current_title(shell_title);
            return self.current_title.clone();
        };

        if alternate_screen {
            self.candidate = None;
            self.set_current_title(name);
            return self.current_title.clone();
        }

        let now = Instant::now();
        match self.candidate.as_mut() {
            Some(candidate) if candidate.pid == pid && candidate.name == name => {
                if now.duration_since(candidate.first_seen) >= Self::PROMOTION_DELAY {
                    let title = candidate.name.clone();
                    self.set_current_title(title);
                } else {
                    self.set_current_title(shell_title);
                }
            }
            _ => {
                self.candidate = Some(ForegroundCandidate {
                    pid,
                    name,
                    first_seen: now,
                });
                self.set_current_title(shell_title);
            }
        }

        self.current_title.clone()
    }

    fn generation(&self) -> u64 {
        self.generation
    }

    fn set_current_title(&mut self, title: String) {
        if self.current_title != title {
            self.current_title = title;
            self.generation = self.generation.wrapping_add(1);
        }
    }
}

#[derive(Serialize)]
pub struct TerminalSnapshot {
    pub cols: u16,
    pub rows: u16,
    pub lines: Vec<String>,
    pub cells: Vec<TerminalCell>,
    pub default_foreground: Option<String>,
    pub default_background: Option<String>,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cursor_visible: bool,
    pub cursor_style: &'static str,
    pub application_cursor: bool,
    pub bracketed_paste: bool,
    pub alternate_screen: bool,
    pub scrollbar: Option<TerminalScrollbar>,
    pub process_title: Option<String>,
    pub title: Option<String>,
    /// Screen-content generation only; bumps when VT output lands.
    pub generation: u64,
    /// Bumps when the computed process title changes.
    pub title_generation: u64,
    pub exited: bool,
}

/// Viewport position in the complete scrollable row space.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct TerminalScrollbar {
    pub total: u64,
    pub offset: u64,
    pub len: u64,
}

#[derive(Serialize)]
pub struct TerminalCell {
    pub row: u16,
    pub col: u16,
    pub text: String,
    pub fg: Option<String>,
    pub bg: Option<String>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    /// True for every underline style; `underline_style` names which.
    pub underline: bool,
    /// `none` | `single` | `double` | `curly` | `dotted` | `dashed`.
    pub underline_style: &'static str,
    /// SGR 58 underline color, when the cell sets one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline_color: Option<String>,
    pub strike: bool,
    pub inverse: bool,
    /// SGR 8: `text` is empty but the cell keeps its colors, and the
    /// concealed run still occupies its columns.
    pub hidden: bool,
    /// Grid columns this cell's text occupies; 0 marks a continuation
    /// column covered by an earlier wide char or joined cluster.
    pub columns: u8,
    pub wide: bool,
    /// OSC 8 hyperlink URI attached to the cell, when any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyperlink: Option<String>,
}

impl Default for TerminalCell {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            text: String::new(),
            fg: None,
            bg: None,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            underline_style: TerminalUnderlineStyle::None.as_str(),
            underline_color: None,
            strike: false,
            inverse: false,
            hidden: false,
            columns: 1,
            wide: false,
            hyperlink: None,
        }
    }
}

impl TerminalSession {
    fn spawn(cols: u16, rows: u16, spec: &TerminalSessionSpec) -> Result<Self, String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| format!("open pty failed: {err}"))?;

        let mut shell = spec
            .program
            .as_ref()
            .filter(|program| !program.trim().is_empty())
            .map(|program| TerminalShell {
                path: program.clone(),
                args: spec.args.clone().unwrap_or_default(),
            })
            .unwrap_or_else(|| {
                let shell = resolved_shell();
                TerminalShell {
                    path: shell.path,
                    args: spec.args.clone().unwrap_or(shell.args),
                }
            });
        // Shell integration rewrites only the engine-default invocation;
        // explicit args mean the caller drives the shell itself.
        let integration_env = if spec.shell_integration && spec.args.is_none() {
            match shell_integration::plan_for(&shell.path) {
                Some(plan) => {
                    if let Some(args) = plan.args {
                        shell.args = args;
                    }
                    plan.env
                }
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };
        let shell_title = process_name_from_path(&shell.path);
        let mut command = CommandBuilder::new(shell.path);
        for arg in shell.args {
            command.arg(arg);
        }
        if let Some(cwd) = spec.cwd.as_deref() {
            command.cwd(cwd);
        }
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "LingXia");
        command.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
        // Image-capable TUIs commonly use this as the Kitty graphics feature
        // hint instead of probing the protocol. Keep LingXia's own identity in
        // TERM_PROGRAM while allowing those clients to enable inline images.
        command.env("KITTY_WINDOW_ID", "1");
        if std::env::var_os("LANG").is_none() {
            command.env("LANG", "en_US.UTF-8");
        }
        for (key, value) in &integration_env {
            command.env(key, value);
        }
        for (key, value) in &spec.env {
            command.env(key, value);
        }

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|err| format!("spawn shell failed: {err}"))?;
        let shell_pid = child.process_id();
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|err| format!("clone pty reader failed: {err}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|err| format!("take pty writer failed: {err}"))?;
        let writer_queue = Arc::new(PtyWriterQueue::default());
        let worker_queue = Arc::clone(&writer_queue);
        let writer_thread = thread::spawn(move || worker_queue.run(writer));
        let callback_writer = Arc::downgrade(&writer_queue);
        let write_pty: PtyWriteCallback = Arc::new(move |bytes: &[u8]| {
            if let Some(writer) = callback_writer.upgrade() {
                let _ = writer.enqueue(bytes);
            }
        });
        let theme = match spec.theme.as_ref() {
            Some(theme) => theme.to_colors().map_err(|err| err.to_string())?,
            None => default_theme(),
        };
        let vt = VtScreen::new_with_options(
            cols,
            rows,
            Some(&theme),
            Some(write_pty),
            spec.scrollback_limit,
        );

        // Bounded so a consumer that stops polling can't buffer PTY
        // output without limit. When the channel fills the reader
        // blocks, the kernel PTY buffer fills, and the child throttles
        // — correct terminal backpressure semantics.
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(4096);
        let reader_thread = thread::spawn(move || {
            let mut buffer = [0_u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => match tx.try_send(buffer[..n].to_vec()) {
                        Ok(()) => {}
                        Err(TrySendError::Full(chunk)) => {
                            // Receiver dropped (session closed) makes
                            // this fail with disconnect, exiting the
                            // thread.
                            if tx.send(chunk).is_err() {
                                break;
                            }
                        }
                        Err(TrySendError::Disconnected(_)) => break,
                    },
                    Err(_) => break,
                }
            }
        });

        let title_state = TerminalTitleState::new(shell_pid, shell_title.clone());
        Ok(Self {
            master: pair.master,
            child,
            writer: writer_queue,
            output: rx,
            vt,
            title_state,
            exit_event_sent: false,
            child_status: None,
            last_frame: None,
            search_cancel: Arc::new(AtomicBool::new(false)),
            _reader: reader_thread,
            _writer: writer_thread,
        })
    }

    /// Feed pending PTY output into the VT, surface the child exit once,
    /// then drain the semantic event queue.
    fn drain_events(&mut self) -> TerminalEventBatch {
        let bytes = self.drain_bytes();
        if !bytes.is_empty() {
            self.vt.feed(&bytes);
        }
        if let Some(exit_code) = self.poll_child()
            && !self.exit_event_sent
        {
            self.exit_event_sent = true;
            self.vt.push_event(TerminalEventKind::Exited { exit_code });
        }
        self.vt.drain_events()
    }

    /// `Some(code)` once the child has exited, cached across calls.
    /// A `try_wait` error counts as exited: the child is no longer
    /// observable, and reporting a live session forever is worse than
    /// reporting an unknown exit code.
    fn poll_child(&mut self) -> Option<Option<i32>> {
        if self.child_status.is_none() {
            match self.child.try_wait() {
                Ok(Some(status)) => self.child_status = Some(Some(status.exit_code() as i32)),
                Ok(None) => {}
                Err(_) => self.child_status = Some(None),
            }
        }
        self.child_status
    }

    fn drain_text(&mut self) -> String {
        let bytes = self.drain_bytes();
        // Keep the emulated screen consistent for callers that mix
        // terminal_read with terminal_snapshot: drained bytes must
        // still reach the VT.
        if !bytes.is_empty() {
            self.vt.feed(&bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn drain_snapshot(&mut self) -> TerminalSnapshot {
        let bytes = self.drain_bytes();
        if !bytes.is_empty() {
            self.vt.feed(&bytes);
        }
        self.snapshot()
    }

    fn exited(&mut self) -> bool {
        self.poll_child().is_some()
    }

    fn resize(
        &mut self,
        cols: u16,
        rows: u16,
        cell_width: u16,
        cell_height: u16,
    ) -> Result<(), String> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: cols.saturating_mul(cell_width),
                pixel_height: rows.saturating_mul(cell_height),
            })
            .map_err(|err| err.to_string())?;
        self.vt
            .resize(cols, rows, u32::from(cell_width), u32::from(cell_height))?;
        Ok(())
    }

    fn scroll(
        &mut self,
        delta_rows: i32,
        col: u16,
        row: u16,
        allow_application_input: bool,
    ) -> bool {
        let bytes = self.drain_bytes();
        if !bytes.is_empty() {
            self.vt.feed(&bytes);
        }

        let mouse_tracking = allow_application_input && self.vt.mouse_tracking_active();
        if allow_application_input
            && self.vt.is_alternate_screen()
            && !mouse_tracking
            && self.vt.is_alt_scroll()
        {
            let sequence = match (delta_rows < 0, self.vt.is_decckm()) {
                (true, true) => b"\x1bOA".as_slice(),
                (true, false) => b"\x1b[A".as_slice(),
                (false, true) => b"\x1bOB".as_slice(),
                (false, false) => b"\x1b[B".as_slice(),
            };
            return self
                .write_repeated(sequence, delta_rows.unsigned_abs())
                .is_ok();
        }

        if mouse_tracking {
            let sequence = encode_mouse_wheel(self.vt.is_sgr_mouse(), delta_rows < 0, col, row);
            return self
                .write_repeated(&sequence, delta_rows.unsigned_abs())
                .is_ok();
        }
        self.vt.scroll_viewport_delta(delta_rows as isize)
    }

    fn write_repeated(&mut self, bytes: &[u8], count: u32) -> std::io::Result<()> {
        const MAX_SCROLL_STEPS: u32 = 4096;

        let count = count.min(MAX_SCROLL_STEPS) as usize;
        let Some(capacity) = bytes.len().checked_mul(count) else {
            return Err(std::io::Error::other("terminal input is too large"));
        };
        let mut repeated = Vec::with_capacity(capacity);
        for _ in 0..count {
            repeated.extend_from_slice(bytes);
        }
        if self.writer.enqueue(&repeated) {
            Ok(())
        } else {
            Err(std::io::Error::other("terminal writer queue is full"))
        }
    }

    fn drain_bytes(&mut self) -> Vec<u8> {
        let mut bytes = Vec::new();
        while let Ok(chunk) = self.output.try_recv() {
            bytes.extend_from_slice(&chunk);
            if bytes.len() >= 256 * 1024 {
                break;
            }
        }
        bytes
    }

    fn snapshot(&mut self) -> TerminalSnapshot {
        let screen = self.vt.snapshot();
        let scrollbar = self.vt.scrollbar().map(|state| TerminalScrollbar {
            total: state.total,
            offset: state.offset,
            len: state.len,
        });
        let raw_title = screen
            .title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToOwned::to_owned);
        let foreground_pid = self.foreground_process_pid();
        let process_title = self
            .title_state
            .title(foreground_pid, self.vt.is_alternate_screen());
        let title_generation = self.title_state.generation();
        let mut cells = Vec::with_capacity(screen.cells.len());
        let mut lines = vec![String::new(); screen.rows as usize];

        for row in 0..screen.rows {
            let mut line = String::with_capacity(screen.cols as usize);
            for col in 0..screen.cols {
                let idx = row as usize * screen.cols as usize + col as usize;
                let Some(cell) = screen.cells.get(idx).cloned() else {
                    line.push(' ');
                    continue;
                };
                let line_ch = cell.text.chars().next().unwrap_or(' ');
                if line_ch == '\0' || line_ch == ' ' {
                    line.push(' ');
                } else {
                    line.push(line_ch);
                }

                let has_background = rgba_alpha(cell.bg) != 0;
                if cell.text.is_empty() && !has_background {
                    continue;
                }
                cells.push(TerminalCell {
                    row,
                    col,
                    text: cell.text.clone(),
                    fg: color_from_rgba(cell.fg, true),
                    bg: color_from_rgba(cell.bg, false),
                    bold: cell.attrs & ATTR_BOLD != 0,
                    dim: cell.attrs & ATTR_DIM != 0,
                    italic: cell.attrs & ATTR_ITALIC != 0,
                    underline: cell.attrs & ATTR_UNDERLINE != 0,
                    underline_style: cell.underline.as_str(),
                    underline_color: cell
                        .underline_color
                        .and_then(|color| color_from_rgba(color, true)),
                    strike: cell.attrs & ATTR_STRIKE != 0,
                    inverse: cell.attrs & ATTR_INVERSE != 0,
                    hidden: cell.attrs & ATTR_HIDDEN != 0,
                    columns: cell.columns,
                    wide: cell.wide,
                    hyperlink: cell.hyperlink.clone(),
                });
            }
            if let Some(slot) = lines.get_mut(row as usize) {
                *slot = line.trim_end().to_string();
            }
        }

        TerminalSnapshot {
            cols: screen.cols,
            rows: screen.rows,
            lines,
            cells,
            default_foreground: color_from_rgba(screen.default_fg, true),
            default_background: color_from_rgba(screen.default_bg, true),
            cursor_row: screen.cursor.row,
            cursor_col: screen.cursor.col,
            cursor_visible: screen.cursor.visible,
            cursor_style: cursor_style_name(screen.cursor.style),
            application_cursor: self.vt.is_decckm(),
            bracketed_paste: self.vt.is_bracketed_paste(),
            alternate_screen: self.vt.is_alternate_screen(),
            scrollbar,
            process_title: Some(process_title),
            title: raw_title,
            generation: screen.generation,
            title_generation,
            exited: self.exited(),
        }
    }

    fn foreground_process_pid(&self) -> Option<u32> {
        #[cfg(unix)]
        {
            self.master
                .process_group_leader()
                .and_then(|pid| u32::try_from(pid).ok())
        }
        #[cfg(not(unix))]
        {
            None
        }
    }
}

/// Encode one vertical wheel step at a zero-based viewport cell.
///
/// Modern applications use SGR mouse reporting. The legacy X10 form is kept
/// for programs that request mouse tracking without enabling SGR coordinates.
fn encode_mouse_wheel(sgr: bool, up: bool, col: u16, row: u16) -> Vec<u8> {
    let button = if up { 64_u8 } else { 65_u8 };
    if sgr {
        return format!(
            "\x1b[<{button};{};{}M",
            u32::from(col) + 1,
            u32::from(row) + 1
        )
        .into_bytes();
    }

    // Classic X10 coordinates are encoded as one byte with a 32 bias.
    // Clamp to its representable 223-column/row range.
    let x = col.saturating_add(1).min(223) as u8;
    let y = row.saturating_add(1).min(223) as u8;
    vec![0x1b, b'[', b'M', button + 32, x + 32, y + 32]
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.writer.close();
        // Kill, then reap — without the wait the dead child lingers as
        // a zombie until the host process exits.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl TerminalSnapshot {
    fn closed() -> Self {
        Self {
            cols: 0,
            rows: 0,
            lines: Vec::new(),
            cells: Vec::new(),
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
            exited: true,
        }
    }

    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| r#"{"exited":true}"#.to_string())
    }
}

fn cursor_style_name(style: CursorVisualStyle) -> &'static str {
    match style {
        CursorVisualStyle::Bar => "bar",
        CursorVisualStyle::Block => "block",
        CursorVisualStyle::Underline => "underline",
        CursorVisualStyle::BlockHollow => "hollow",
    }
}

#[derive(Clone)]
struct TerminalShell {
    path: String,
    args: Vec<String>,
}

#[cfg(windows)]
const POWERSHELL_CWD_INTEGRATION: &str = r#"
$global:__LingXiaOriginalPrompt = $function:prompt
function global:prompt {
    try {
        [Environment]::CurrentDirectory = $executionContext.SessionState.Path.CurrentFileSystemLocation.Path
    } catch {}
    if ($global:__LingXiaOriginalPrompt) {
        & $global:__LingXiaOriginalPrompt
    } else {
        "PS $($executionContext.SessionState.Path.CurrentLocation)> "
    }
}
"#;

fn resolved_shell() -> TerminalShell {
    static RESOLVED_SHELL: OnceLock<TerminalShell> = OnceLock::new();

    RESOLVED_SHELL.get_or_init(resolve_shell_uncached).clone()
}

fn resolve_shell_uncached() -> TerminalShell {
    if let Some(path) = env_non_empty("LINGXIA_TERMINAL_SHELL") {
        return TerminalShell {
            path,
            args: Vec::new(),
        };
    }

    #[cfg(windows)]
    {
        if command_available("pwsh.exe") {
            return TerminalShell {
                path: "pwsh.exe".to_string(),
                args: powershell_terminal_args(),
            };
        }
        if command_available("powershell.exe") {
            return TerminalShell {
                path: "powershell.exe".to_string(),
                args: powershell_terminal_args(),
            };
        }
        TerminalShell {
            path: env_non_empty("COMSPEC").unwrap_or_else(|| "cmd.exe".to_string()),
            args: Vec::new(),
        }
    }

    #[cfg(not(windows))]
    {
        TerminalShell {
            path: env_non_empty("SHELL").unwrap_or_else(|| "/bin/sh".to_string()),
            args: vec!["-i".to_string()],
        }
    }
}

#[cfg(windows)]
fn powershell_terminal_args() -> Vec<String> {
    vec![
        "-NoLogo".to_string(),
        "-NoExit".to_string(),
        "-Command".to_string(),
        POWERSHELL_CWD_INTEGRATION.to_string(),
    ]
}

fn env_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(windows)]
fn command_available(command: &str) -> bool {
    let command = std::path::Path::new(command);
    if command.components().count() > 1 {
        return command.is_file();
    }

    let extensions: Vec<String> = if command.extension().is_some() {
        vec![String::new()]
    } else {
        std::env::var_os("PATHEXT")
            .map(|value| {
                value
                    .to_string_lossy()
                    .split(';')
                    .filter(|ext| !ext.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .filter(|extensions: &Vec<String>| !extensions.is_empty())
            .unwrap_or_else(|| vec![".EXE".to_string(), ".BAT".to_string(), ".CMD".to_string()])
    };

    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };

    std::env::split_paths(&path_var).any(|dir| {
        extensions.iter().any(|ext| {
            let mut candidate = dir.join(command);
            if !ext.is_empty() {
                candidate.set_extension(ext.trim_start_matches('.'));
            }
            candidate.is_file()
        })
    })
}

fn process_name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("terminal")
        .to_string()
}

fn looks_like_shell_title(value: &str, fallback: &str) -> bool {
    let token = value.trim();
    if token.is_empty() {
        return false;
    }
    let normalized = token.to_ascii_lowercase();
    let fallback_normalized = fallback.trim().to_ascii_lowercase();
    normalized == fallback_normalized
        || matches!(
            normalized.as_str(),
            "zsh" | "bash" | "fish" | "sh" | "nu" | "pwsh" | "powershell" | "cmd" | "cmd.exe"
        )
}

fn current_directory_title(pid: Option<u32>) -> Option<String> {
    let pid = pid?;
    process_cwd(pid).map(|path| compact_path_title(&path))
}

fn compact_path_title(path: &Path) -> String {
    let home = std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|home| !home.is_empty()));
    let Some(home) = home else {
        return path.to_string_lossy().into_owned();
    };
    match path.strip_prefix(Path::new(&home)) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn process_cwd(pid: u32) -> Option<std::path::PathBuf> {
    let mut info = unsafe { std::mem::zeroed::<libc::proc_vnodepathinfo>() };
    let size = std::mem::size_of::<libc::proc_vnodepathinfo>();
    let rc = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDVNODEPATHINFO,
            0,
            &mut info as *mut _ as *mut libc::c_void,
            size as libc::c_int,
        )
    };
    if rc < size as libc::c_int {
        return None;
    }
    let cwd = unsafe { CStr::from_ptr(info.pvi_cdir.vip_path.as_ptr() as *const libc::c_char) };
    cwd.to_str()
        .ok()
        .filter(|value| !value.is_empty())
        .map(Into::into)
}

#[cfg(target_os = "linux")]
fn process_cwd(pid: u32) -> Option<std::path::PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
}

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "linux", windows)))]
fn process_cwd(_pid: u32) -> Option<std::path::PathBuf> {
    None
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessGroupMember {
    pid: u32,
    parent_pid: u32,
    name: String,
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn apple_process_name(pid: u32) -> Option<String> {
    let mut buffer = [0_i8; 256];
    let rc = unsafe {
        libc::proc_name(
            pid as libc::c_int,
            buffer.as_mut_ptr() as *mut libc::c_void,
            buffer.len() as u32,
        )
    };
    if rc <= 0 {
        return None;
    }
    unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_str()
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(target_os = "macos")]
fn process_name(process_group_id: u32) -> Option<String> {
    macos_process_group_members(process_group_id)
        .and_then(|members| deepest_process_name(&members))
        .or_else(|| apple_process_name(process_group_id))
}

#[cfg(target_os = "macos")]
fn macos_process_group_members(process_group_id: u32) -> Option<Vec<ProcessGroupMember>> {
    let process_group_id = libc::pid_t::try_from(process_group_id).ok()?;
    let estimated_count =
        unsafe { libc::proc_listpgrppids(process_group_id, std::ptr::null_mut(), 0) };
    if estimated_count <= 0 {
        return None;
    }

    let mut pids = vec![0 as libc::pid_t; estimated_count as usize + 16];
    let buffer_size = libc::c_int::try_from(std::mem::size_of_val(pids.as_slice())).ok()?;
    let count =
        unsafe { libc::proc_listpgrppids(process_group_id, pids.as_mut_ptr().cast(), buffer_size) };
    if count <= 0 {
        return None;
    }

    let mut members = Vec::with_capacity(count as usize);
    for pid in pids.into_iter().take(count as usize).filter(|pid| *pid > 0) {
        let mut info = unsafe { std::mem::zeroed::<libc::proc_bsdinfo>() };
        let info_size = std::mem::size_of::<libc::proc_bsdinfo>();
        let read = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDTBSDINFO,
                0,
                (&mut info as *mut libc::proc_bsdinfo).cast(),
                info_size as libc::c_int,
            )
        };
        if read < info_size as libc::c_int {
            continue;
        }
        if let Some(name) = apple_process_name(info.pbi_pid) {
            members.push(ProcessGroupMember {
                pid: info.pbi_pid,
                parent_pid: info.pbi_ppid,
                name,
            });
        }
    }
    (!members.is_empty()).then_some(members)
}

#[cfg(target_os = "macos")]
fn deepest_process_name(members: &[ProcessGroupMember]) -> Option<String> {
    fn depth(member: &ProcessGroupMember, members: &[ProcessGroupMember]) -> usize {
        let mut depth = 0;
        let mut parent_pid = member.parent_pid;
        for _ in 0..members.len() {
            let Some(parent) = members.iter().find(|candidate| candidate.pid == parent_pid) else {
                break;
            };
            depth += 1;
            parent_pid = parent.parent_pid;
        }
        depth
    }

    members
        .iter()
        .max_by_key(|member| (depth(member, members), member.pid))
        .map(|member| member.name.clone())
}

#[cfg(target_os = "ios")]
fn process_name(pid: u32) -> Option<String> {
    apple_process_name(pid)
}

#[cfg(target_os = "linux")]
fn process_name(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "linux")))]
fn process_name(_pid: u32) -> Option<String> {
    None
}

fn rgba_alpha(value: u32) -> u8 {
    (value & 0xff) as u8
}

fn color_from_rgba(value: u32, include_transparent: bool) -> Option<String> {
    let alpha = rgba_alpha(value);
    if alpha == 0 && !include_transparent {
        return None;
    }
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        (value >> 24) & 0xff,
        (value >> 16) & 0xff,
        (value >> 8) & 0xff
    ))
}

/// Theme new sessions inherit when their spec carries none.
static DEFAULT_THEME: LazyLock<Mutex<ThemeColors>> =
    LazyLock::new(|| Mutex::new(default_theme_colors()));

fn default_theme_colors() -> ThemeColors {
    TerminalTheme::default()
        .to_colors()
        .expect("built-in theme is valid")
}

fn default_theme() -> ThemeColors {
    DEFAULT_THEME
        .lock()
        .map(|theme| theme.clone())
        .unwrap_or_else(|_| default_theme_colors())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    struct ChunkRecorder {
        writes: Arc<Mutex<Vec<usize>>>,
    }

    impl Write for ChunkRecorder {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.writes.lock().unwrap().push(bytes.len());
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn pty_writes_are_split_into_bounded_chunks() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let mut writer = ChunkRecorder {
            writes: Arc::clone(&writes),
        };
        let input = vec![0_u8; PTY_WRITE_CHUNK_SIZE * 2 + 17];

        write_pty_chunked(&mut writer, &input).unwrap();

        assert_eq!(
            *writes.lock().unwrap(),
            vec![PTY_WRITE_CHUNK_SIZE, PTY_WRITE_CHUNK_SIZE, 17]
        );
    }

    struct BlockingWriter {
        started: Option<mpsc::Sender<()>>,
        release: Arc<(Mutex<bool>, Condvar)>,
    }

    impl Write for BlockingWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if let Some(started) = self.started.take() {
                let _ = started.send(());
            }
            let (lock, ready) = &*self.release;
            let mut released = lock.lock().unwrap();
            while !*released {
                released = ready.wait(released).unwrap();
            }
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn writer_queue_does_not_block_when_pty_write_stalls() {
        let queue = Arc::new(PtyWriterQueue::default());
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let (started_tx, started_rx) = mpsc::channel();
        let worker_queue = Arc::clone(&queue);
        let worker_release = Arc::clone(&release);
        let worker = thread::spawn(move || {
            worker_queue.run(Box::new(BlockingWriter {
                started: Some(started_tx),
                release: worker_release,
            }));
        });

        assert!(queue.enqueue(b"first"));
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        let enqueue_started = Instant::now();
        assert!(queue.enqueue(b"second"));
        assert!(enqueue_started.elapsed() < Duration::from_millis(100));

        queue.close();
        let (lock, ready) = &*release;
        *lock.lock().unwrap() = true;
        ready.notify_all();
        worker.join().unwrap();
    }

    #[test]
    fn status_json_is_valid_shape() {
        let json = backend_status_json();
        assert!(json.contains(r#""backend":"alacritty""#));
        assert!(json.contains(r#""available":true"#));
    }

    #[test]
    fn closed_snapshot_is_valid_json() {
        let json = TerminalSnapshot::closed().to_json();
        assert!(json.contains(r#""exited":true"#));
    }

    fn char_event(character: char) -> TerminalKeyEvent {
        TerminalKeyEvent {
            character: Some(character),
            ..TerminalKeyEvent::default()
        }
    }

    fn keydown_event(vk: u32) -> TerminalKeyEvent {
        TerminalKeyEvent {
            vk,
            ..TerminalKeyEvent::default()
        }
    }

    #[test]
    fn encodes_printable_characters_verbatim() {
        assert_eq!(encode_key_event(char_event('a')).as_deref(), Some("a"));
        assert_eq!(encode_key_event(char_event('Z')).as_deref(), Some("Z"));
        assert_eq!(encode_key_event(char_event('~')).as_deref(), Some("~"));
        assert_eq!(encode_key_event(char_event('中')).as_deref(), Some("中"));
    }

    #[test]
    fn encodes_alt_character_as_meta_and_preserves_altgr_text() {
        let mut alt = char_event('v');
        alt.alt = true;
        assert_eq!(encode_key_event(alt).as_deref(), Some("\x1bv"));

        alt.ctrl = true;
        assert_eq!(encode_key_event(alt).as_deref(), Some("v"));
    }

    #[test]
    fn encodes_special_characters() {
        assert_eq!(
            encode_key_event(char_event('\u{8}')).as_deref(),
            Some("\u{7f}"),
            "backspace becomes DEL"
        );
        assert_eq!(encode_key_event(char_event('\t')).as_deref(), Some("\t"));
        assert_eq!(encode_key_event(char_event('\r')).as_deref(), Some("\r"));
        assert_eq!(
            encode_key_event(char_event('\u{1b}')).as_deref(),
            Some("\u{1b}")
        );
    }

    #[test]
    fn encodes_supported_control_characters() {
        assert_eq!(
            encode_key_event(char_event('\u{3}')).as_deref(),
            Some("\u{3}")
        );
    }

    #[test]
    fn rejects_unsupported_control_characters() {
        assert_eq!(encode_key_event(char_event('\n')), None);
        assert_eq!(encode_key_event(char_event('\u{7f}')), None);
    }

    #[test]
    fn encodes_navigation_virtual_keys() {
        assert_eq!(
            encode_key_event(keydown_event(0x25)).as_deref(),
            Some("\u{1b}[D")
        );
        assert_eq!(
            encode_key_event(keydown_event(0x26)).as_deref(),
            Some("\u{1b}[A")
        );
        assert_eq!(
            encode_key_event(keydown_event(0x27)).as_deref(),
            Some("\u{1b}[C")
        );
        assert_eq!(
            encode_key_event(keydown_event(0x28)).as_deref(),
            Some("\u{1b}[B")
        );
        assert_eq!(
            encode_key_event(keydown_event(0x2e)).as_deref(),
            Some("\u{1b}[3~")
        );
    }

    #[test]
    fn rejects_unmapped_virtual_keys() {
        assert_eq!(encode_key_event(keydown_event(0x41)), None, "plain VK_A");
        assert_eq!(encode_key_event(keydown_event(0x10)), None, "VK_SHIFT");
        assert_eq!(encode_key_event(TerminalKeyEvent::default()), None);
    }

    #[test]
    fn rejects_scroll_for_missing_session_or_zero_delta() {
        assert!(!terminal_scroll(0, -3, 0, 0, true));
        assert!(!terminal_scroll(u64::MAX, 0, 0, 0, true));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn names_the_deepest_process_group_member() {
        let members = vec![
            ProcessGroupMember {
                pid: 10,
                parent_pid: 1,
                name: "runtime".to_string(),
            },
            ProcessGroupMember {
                pid: 11,
                parent_pid: 10,
                name: "cli-tool".to_string(),
            },
        ];
        assert_eq!(deepest_process_name(&members).as_deref(), Some("cli-tool"));
    }

    #[test]
    #[cfg(unix)]
    fn session_spec_runs_program_with_args_env_and_scrollback_limit() {
        let id = terminal_create_with_spec(
            80,
            5,
            TerminalSessionSpec {
                program: Some("/bin/sh".to_string()),
                args: Some(vec![
                    "-c".to_string(),
                    "i=0; while [ $i -lt 40 ]; do echo line$i; i=$((i+1)); done; echo \"var=$LINGXIA_SPEC_TEST_VAR\"; sleep 30".to_string(),
                ]),
                env: vec![("LINGXIA_SPEC_TEST_VAR".to_string(), "spec-ok".to_string())],
                scrollback_limit: Some(5),
                ..TerminalSessionSpec::default()
            },
        );
        assert_ne!(id, 0);

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut snapshot = terminal_snapshot_data(id);
        while Instant::now() < deadline {
            snapshot = terminal_snapshot_data(id);
            if snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.lines.iter().any(|line| line.contains("var=")))
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let snapshot = snapshot.expect("snapshot for live session");
        let text = snapshot.lines.join("\n");
        assert!(text.contains("var=spec-ok"), "spec env visible: {text}");
        let scrollbar = snapshot.scrollbar.expect("scrollbar present");
        assert!(
            scrollbar.total <= 5 + 5,
            "scrollback capped at 5 lines + 5 rows: {scrollbar:?}"
        );
        terminal_close(id);
    }

    #[test]
    #[cfg(unix)]
    fn shell_integration_marks_commands_in_bash() {
        if !Path::new("/bin/bash").exists() {
            return;
        }
        let id = terminal_create_with_spec(
            80,
            24,
            TerminalSessionSpec {
                program: Some("/bin/bash".to_string()),
                shell_integration: true,
                ..TerminalSessionSpec::default()
            },
        );
        assert_ne!(id, 0);

        // Wait for the first prompt mark, then run a command.
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut saw_prompt = false;
        while Instant::now() < deadline && !saw_prompt {
            let batch = terminal_events_drain_data(id).expect("live session");
            saw_prompt = batch
                .events
                .iter()
                .any(|event| matches!(event.kind, TerminalEventKind::PromptStart { .. }));
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(saw_prompt, "bash integration emits prompt marks");
        // A subshell keeps the shell alive so the next prompt reports
        // the command's exit code via OSC 133 D.
        assert!(terminal_write(id, "( exit 3 )\n"));

        let deadline = Instant::now() + Duration::from_secs(10);
        let mut exit_code = None;
        while Instant::now() < deadline {
            let batch = terminal_events_drain_data(id).expect("live session");
            for event in batch.events {
                if let TerminalEventKind::CommandFinished {
                    exit_code: code, ..
                } = event.kind
                {
                    exit_code = code;
                }
                if matches!(event.kind, TerminalEventKind::Exited { .. }) {
                    break;
                }
            }
            if exit_code.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        terminal_close(id);
        assert_eq!(exit_code, Some(3), "command exit code from OSC 133 D");
    }

    #[test]
    #[cfg(unix)]
    fn session_surfaces_cwd_and_exit_events() {
        let id = terminal_create_with_spec(
            80,
            24,
            TerminalSessionSpec {
                program: Some("/bin/sh".to_string()),
                args: Some(vec![
                    "-c".to_string(),
                    "printf '\\033]7;file:///tmp\\a'; sleep 0.1; exit 7".to_string(),
                ]),
                ..TerminalSessionSpec::default()
            },
        );
        assert_ne!(id, 0);

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut events = Vec::new();
        while Instant::now() < deadline {
            let batch = terminal_events_drain_data(id).expect("live session");
            events.extend(batch.events.into_iter().map(|event| event.kind));
            if events
                .iter()
                .any(|kind| matches!(kind, TerminalEventKind::Exited { .. }))
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        terminal_close(id);

        assert!(
            events.iter().any(|kind| matches!(
                kind,
                TerminalEventKind::Cwd { path } if path == "/tmp"
            )),
            "cwd event surfaced: {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|kind| matches!(kind, TerminalEventKind::Exited { exit_code: Some(7) })),
            "exit event surfaced: {events:?}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn frame_view_exposes_retained_buffers() {
        let id = terminal_create_with_spec(
            20,
            4,
            TerminalSessionSpec {
                program: Some("/bin/sh".to_string()),
                args: Some(vec![
                    "-c".to_string(),
                    "printf 'frame-view'; sleep 30".to_string(),
                ]),
                ..TerminalSessionSpec::default()
            },
        );
        assert_ne!(id, 0);

        // Read the buffers exactly as a host would: through the pointers,
        // before the next frame call invalidates them.
        let read = |view: &TerminalFrameView| -> String {
            let cells = unsafe { std::slice::from_raw_parts(view.cells, view.cells_len) };
            let text = unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(view.text, view.text_len))
            };
            cells[..view.cols as usize]
                .iter()
                .map(|cell| {
                    let start = cell.text_offset as usize;
                    &text[start..start + cell.text_len as usize]
                })
                .collect()
        };

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut generation = 0;
        let mut row = String::new();
        while Instant::now() < deadline && !row.contains("frame-view") {
            let view = terminal_frame_view(id, generation).expect("live session");
            if view.changed {
                generation = view.generation;
                assert_eq!(view.cols, 20);
                assert_eq!(view.cells_len, 20 * 4);
                row = read(&view);
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(row.starts_with("frame-view"), "row from pointers: {row:?}");

        // A quiet poll reports no change and hands out no buffers.
        let quiet = terminal_frame_view(id, generation).expect("live session");
        assert!(!quiet.changed);
        assert!(quiet.cells.is_null());
        assert_eq!(quiet.generation, generation);
        assert!(
            !quiet.exited,
            "a quiet poll still answers the lifecycle question"
        );

        // Titles stay off the frame path but remain reachable.
        let titles = terminal_title_state_json(id);
        assert!(titles.contains("processTitle"), "titles: {titles}");

        terminal_close(id);
        assert!(terminal_frame_view(id, generation).is_none());
    }

    #[test]
    #[cfg(unix)]
    fn theme_applies_at_spawn_and_swaps_live() {
        let spawn_theme = TerminalTheme {
            background: "#101112".to_string(),
            red: "#ff0001".to_string(),
            ..TerminalTheme::default()
        };
        let id = terminal_create_with_spec(
            40,
            4,
            TerminalSessionSpec {
                program: Some("/bin/sh".to_string()),
                args: Some(vec![
                    "-c".to_string(),
                    "printf '\\033[31mRED\\033[m'; sleep 30".to_string(),
                ]),
                theme: Some(spawn_theme),
                ..TerminalSessionSpec::default()
            },
        );
        assert_ne!(id, 0);

        let red_cell = |id: u64| -> Option<TerminalCell> {
            let snapshot = terminal_snapshot_data(id)?;
            snapshot.cells.into_iter().find(|cell| cell.text == "R")
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut cell = None;
        while Instant::now() < deadline && cell.is_none() {
            cell = red_cell(id);
            std::thread::sleep(Duration::from_millis(25));
        }
        let cell = cell.expect("colored output rendered");
        assert_eq!(cell.fg.as_deref(), Some("#ff0001"), "spec theme at spawn");
        assert_eq!(
            terminal_snapshot_data(id)
                .unwrap()
                .default_background
                .as_deref(),
            Some("#101112")
        );

        // Live swap: same session, same grid, new colors.
        let updated = terminal_set_theme_all(&TerminalTheme {
            background: "#202122".to_string(),
            red: "#0000ff".to_string(),
            ..TerminalTheme::default()
        })
        .expect("valid theme");
        assert!(updated >= 1, "live sessions repainted");

        let snapshot = terminal_snapshot_data(id).expect("live session");
        let cell = snapshot
            .cells
            .iter()
            .find(|cell| cell.text == "R")
            .expect("text survives a theme swap");
        assert_eq!(cell.fg.as_deref(), Some("#0000ff"));
        assert_eq!(snapshot.default_background.as_deref(), Some("#202122"));

        // An invalid color is rejected before anything is applied.
        let bad = terminal_set_theme(
            id,
            &TerminalTheme {
                red: "nope".to_string(),
                ..TerminalTheme::default()
            },
        );
        assert_eq!(bad.unwrap_err().field, "red");
        assert_eq!(
            terminal_snapshot_data(id)
                .unwrap()
                .cells
                .iter()
                .find(|cell| cell.text == "R")
                .unwrap()
                .fg
                .as_deref(),
            Some("#0000ff"),
            "rejected theme left the session untouched"
        );
        terminal_close(id);
        // The swap above moved the process-wide default; put it back so
        // sessions spawned by other tests keep the built-in palette.
        terminal_set_default_theme(&TerminalTheme::default()).expect("built-in theme");
    }

    #[test]
    #[cfg(unix)]
    fn session_status_reports_progress_and_exit() {
        let id = terminal_create_with_spec(
            80,
            24,
            TerminalSessionSpec {
                program: Some("/bin/sh".to_string()),
                args: Some(vec![
                    "-c".to_string(),
                    // The status has to be observed while the command is
                    // still running, and the poll below samples every 25ms.
                    // A 100ms lifetime lets a loaded runner miss the window
                    // entirely and see an exited session on its first read.
                    "printf '\\033]9;4;1;70\\a\\a'; sleep 2; exit 5".to_string(),
                ]),
                ..TerminalSessionSpec::default()
            },
        );
        assert_ne!(id, 0);

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut running = None;
        while Instant::now() < deadline {
            let status = terminal_status_data(id).expect("live session");
            if status.activity.progress.state == TerminalProgressState::Running {
                running = Some(status);
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let running = running.expect("progress report reaches status");
        assert_eq!(running.activity.progress.percent, Some(70));
        assert_eq!(running.activity.bells, 1);
        assert!(!running.exited);

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut exited = None;
        while Instant::now() < deadline {
            let status = terminal_status_data(id).expect("live session");
            if status.exited {
                exited = Some(status);
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let exited = exited.expect("child exit reaches status");
        assert_eq!(exited.exit_code, Some(5));
        // The event stream still reports the exit after status observed
        // it: reading status must not consume the child's status.
        let events = terminal_events_drain_data(id).expect("live session");
        assert!(
            events.events.iter().any(|event| matches!(
                event.kind,
                TerminalEventKind::Exited { exit_code: Some(5) }
            )),
            "exit event survives a status read: {events:?}"
        );
        terminal_close(id);
    }

    #[test]
    #[cfg(unix)]
    fn restore_state_exports_and_replays_into_a_fresh_shell() {
        let id = terminal_create_with_spec(
            60,
            6,
            TerminalSessionSpec {
                program: Some("/bin/sh".to_string()),
                args: Some(vec![
                    "-c".to_string(),
                    "i=0; while [ $i -lt 20 ]; do echo \"restore-line-$i\"; i=$((i+1)); done; sleep 30".to_string(),
                ]),
                scrollback_limit: Some(100),
                ..TerminalSessionSpec::default()
            },
        );
        assert_ne!(id, 0);

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut state = None;
        while Instant::now() < deadline {
            let exported = terminal_export_restore_state(id, Some("profile-a"), 0);
            if exported.as_ref().is_some_and(|state| {
                state
                    .scrollback
                    .iter()
                    .any(|line| line.contains("restore-line-19"))
            }) {
                state = exported;
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let state = state.expect("export captured all lines");
        terminal_close(id);
        assert_eq!(state.version, TERMINAL_RESTORE_VERSION);
        assert_eq!(state.profile_id.as_deref(), Some("profile-a"));
        assert!(state.cwd.is_some());

        // Replay into a fresh shell; restored content stays searchable.
        let restored_id = terminal_create_with_restore(
            60,
            6,
            TerminalSessionSpec {
                program: Some("/bin/sh".to_string()),
                args: Some(vec!["-c".to_string(), "sleep 30".to_string()]),
                ..TerminalSessionSpec::default()
            },
            &state,
        )
        .expect("restore creates session");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut marks = (false, false);
        while Instant::now() < deadline {
            let batch = terminal_events_drain_data(restored_id).expect("live session");
            for event in batch.events {
                if let TerminalEventKind::Restored { lines } = event.kind {
                    marks.0 = lines == state.scrollback.len();
                }
            }
            let results = terminal_search_data(
                restored_id,
                "restore-line-19",
                TerminalSearchMode::Plain,
                10,
            );
            if results.as_ref().is_some_and(|r| r.total == 1) {
                marks.1 = true;
            }
            if marks.0 && marks.1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        terminal_close(restored_id);
        assert!(marks.0, "Restored event with replayed line count");
        assert!(marks.1, "restored scrollback searchable in fresh shell");

        // Unknown versions are rejected, not misread.
        let mut future = state.clone();
        future.version = TERMINAL_RESTORE_VERSION + 1;
        assert!(matches!(
            terminal_create_with_restore(60, 6, TerminalSessionSpec::default(), &future),
            Err(TerminalRestoreError::UnknownVersion(_))
        ));
    }

    #[test]
    #[cfg(unix)]
    fn session_links_detect_urls_and_resolve_paths() {
        let cwd = std::env::temp_dir().canonicalize().unwrap();
        let id = terminal_create_with_spec(
            80,
            24,
            TerminalSessionSpec {
                cwd: Some(cwd.clone()),
                program: Some("/bin/sh".to_string()),
                args: Some(vec![
                    "-c".to_string(),
                    "echo 'see https://example.com/docs'; echo 'edit src/main.rs:3'; sleep 30"
                        .to_string(),
                ]),
                ..TerminalSessionSpec::default()
            },
        );
        assert_ne!(id, 0);

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut found = Vec::new();
        while Instant::now() < deadline {
            found = terminal_links_data(id).unwrap_or_default();
            if found.len() >= 2 {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        terminal_close(id);

        let url = found
            .iter()
            .find(|link| link.target == "https://example.com/docs");
        assert!(url.is_some(), "url detected: {found:?}");
        let path = found.iter().find(|link| link.target_line == Some(3));
        let path = path.expect("path with line suffix detected");
        let expected = cwd.join("src/main.rs");
        assert_eq!(path.target, expected.to_string_lossy());
    }

    #[test]
    #[cfg(unix)]
    fn session_search_finds_matches_across_scrollback() {
        let id = terminal_create_with_spec(
            40,
            4,
            TerminalSessionSpec {
                program: Some("/bin/sh".to_string()),
                args: Some(vec![
                    "-c".to_string(),
                    "i=0; while [ $i -lt 30 ]; do echo \"row $i needle here\"; i=$((i+1)); done; sleep 30".to_string(),
                ]),
                scrollback_limit: Some(100),
                ..TerminalSessionSpec::default()
            },
        );
        assert_ne!(id, 0);

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut results = None;
        while Instant::now() < deadline {
            let found = terminal_search_data(id, "needle", TerminalSearchMode::Plain, 100);
            if found.as_ref().is_some_and(|r| r.total >= 30) {
                results = found;
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let results = results.expect("all 30 rows searchable");
        assert_eq!(results.total, 30);
        // Matches span scrollback and screen: first rows scrolled out
        // of the 4-row viewport but stay searchable.
        let lines: Vec<i64> = results.matches.iter().map(|m| m.start_line).collect();
        assert_eq!(lines.first(), Some(&0));
        assert_eq!(lines.last(), Some(&29));
        let regex = terminal_search_data(id, r"row 1\d", TerminalSearchMode::Regex, 100)
            .expect("regex search");
        assert_eq!(regex.total, 10, "rows 10-19: {regex:?}");
        terminal_close(id);
    }

    #[test]
    fn encodes_sgr_and_legacy_mouse_wheel() {
        assert_eq!(
            encode_mouse_wheel(true, true, 4, 2).as_slice(),
            b"\x1b[<64;5;3M"
        );
        assert_eq!(
            encode_mouse_wheel(false, false, 4, 2),
            vec![0x1b, b'[', b'M', 97, 37, 35]
        );
    }

    #[test]
    fn session_renders_shell_output() {
        let id = terminal_create(80, 24);
        assert_ne!(id, 0);

        assert!(terminal_write(id, "printf 'LINGXIA_TERMINAL_VT_OK\\n'\n"));
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let snapshot = terminal_snapshot(id);
            if snapshot.contains("LINGXIA_TERMINAL_VT_OK") {
                terminal_close(id);
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        let snapshot = terminal_snapshot(id);
        terminal_close(id);
        panic!("terminal snapshot did not contain shell output: {snapshot}");
    }

    #[cfg(windows)]
    #[test]
    fn windows_conpty_preserves_kitty_graphics_apc() {
        let executable = std::env::current_exe().expect("test executable");
        let directory = executable.parent().expect("test executable directory");
        if !directory.join("conpty.dll").is_file() || !directory.join("OpenConsole.exe").is_file() {
            eprintln!("skipping: redistributable ConPTY sidecar is not staged");
            return;
        }
        let script =
            "import os;os.write(1,b'\\x1b_Ga=T,f=32,s=1,v=1,i=91,c=2,r=2,C=1;/wAA/w==\\x1b\\\\')";
        let id = terminal_create_with_spec(
            80,
            24,
            TerminalSessionSpec {
                program: Some("python.exe".to_string()),
                args: Some(vec!["-c".to_string(), script.to_string()]),
                ..TerminalSessionSpec::default()
            },
        );
        assert_ne!(id, 0);

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let (_, images) = terminal_render_data(id, 0, 0).expect("render data");
            if !images.placements.is_empty() {
                assert_eq!(images.images.len(), 1);
                terminal_close(id);
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        let images = terminal_image_snapshot(id, 0);
        terminal_close(id);
        panic!("redistributable ConPTY did not preserve Kitty APC: {images}");
    }

    #[test]
    fn session_can_start_in_an_explicit_directory() {
        let expected = std::env::temp_dir().canonicalize().unwrap();
        let id = terminal_create_at(80, 24, Some(&expected));
        assert_ne!(id, 0);

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if terminal_current_directory(id).as_deref() == Some(expected.as_path()) {
                terminal_close(id);
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        let actual = terminal_current_directory(id);
        terminal_close(id);
        panic!("terminal cwd mismatch: expected={expected:?} actual={actual:?}");
    }

    #[cfg(windows)]
    #[test]
    fn windows_process_cwd_follows_directory_changes() {
        let root = std::env::temp_dir().join(format!(
            "lingxia-terminal-cwd-{}-{}",
            std::process::id(),
            NEXT_SESSION_ID.load(Ordering::Relaxed)
        ));
        let destination = root.join("destination");
        std::fs::create_dir_all(&destination).unwrap();
        let expected = destination.canonicalize().unwrap();
        let command = format!(
            "Set-Location -LiteralPath '{0}'; [Environment]::CurrentDirectory = '{0}'; Start-Sleep -Seconds 10",
            destination.to_string_lossy().replace('\'', "''")
        );
        let mut child = std::process::Command::new("powershell.exe")
            .args(["-NoLogo", "-NoProfile", "-Command", &command])
            .current_dir(&root)
            .spawn()
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let actual = process_cwd(child.id()).and_then(|path| path.canonicalize().ok());
            if actual.as_deref() == Some(expected.as_path()) {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_dir_all(&root);
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        let actual = process_cwd(child.id());
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&root);
        panic!("process cwd did not follow cd: expected={expected:?} actual={actual:?}");
    }

    #[cfg(windows)]
    #[test]
    fn windows_terminal_title_tracks_cd() {
        let root = std::env::temp_dir().join(format!(
            "lingxia-terminal-title-{}-{}",
            std::process::id(),
            NEXT_SESSION_ID.load(Ordering::Relaxed)
        ));
        let destination = root.join("destination");
        std::fs::create_dir_all(&destination).unwrap();
        let expected = destination.canonicalize().unwrap();
        let expected_title = compact_path_title(&destination);
        let id = terminal_create_at(80, 24, Some(&root));
        assert_ne!(id, 0);

        std::thread::sleep(Duration::from_millis(250));
        let shell = resolved_shell().path.to_ascii_lowercase();
        let command = if shell.contains("powershell") || shell.contains("pwsh") {
            format!(
                "Set-Location -LiteralPath '{}'\r",
                destination.to_string_lossy().replace('\'', "''")
            )
        } else {
            format!("cd /d \"{}\"\r", destination.display())
        };
        assert!(terminal_write(id, &command));

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let snapshot = terminal_snapshot_data(id);
            let cwd = terminal_current_directory(id).and_then(|path| path.canonicalize().ok());
            if cwd.as_deref() == Some(expected.as_path())
                && snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.process_title.as_deref())
                    == Some(expected_title.as_str())
            {
                terminal_close(id);
                let _ = std::fs::remove_dir_all(&root);
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        let actual_cwd = terminal_current_directory(id);
        let actual_title = terminal_snapshot_data(id).and_then(|snapshot| snapshot.process_title);
        terminal_close(id);
        let _ = std::fs::remove_dir_all(&root);
        panic!(
            "terminal title did not follow cd: expected={expected:?} cwd={actual_cwd:?} title={actual_title:?}"
        );
    }
}
