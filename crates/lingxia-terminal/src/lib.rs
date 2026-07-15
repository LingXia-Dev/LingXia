//! Terminal runtime integration for LingXia hosts.
//!
//! Product terminal mode is intentionally single-path:
//! portable-pty owns process I/O, libghostty-vt owns terminal emulation.

#[cfg(lingxia_ghostty_vt_available)]
mod ghostty_vt;

#[cfg(lingxia_ghostty_vt_available)]
use ghostty_vt::{
    ATTR_BOLD, ATTR_DIM, ATTR_INVERSE, ATTR_ITALIC, ATTR_UNDERLINE,
    GhosttyRenderStateCursorVisualStyle, PtyWriteCallback, ThemeColors, VtScreen,
};
#[cfg(lingxia_ghostty_vt_available)]
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::Serialize;
#[cfg(lingxia_ghostty_vt_available)]
use std::collections::HashMap;
#[cfg(all(
    lingxia_ghostty_vt_available,
    any(target_os = "macos", target_os = "ios")
))]
use std::ffi::CStr;
#[cfg(lingxia_ghostty_vt_available)]
use std::io::{Read, Write};
#[cfg(lingxia_ghostty_vt_available)]
use std::path::Path;
#[cfg(lingxia_ghostty_vt_available)]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(lingxia_ghostty_vt_available)]
use std::sync::mpsc::{self, Receiver, TrySendError};
#[cfg(lingxia_ghostty_vt_available)]
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
#[cfg(lingxia_ghostty_vt_available)]
use std::thread;
#[cfg(lingxia_ghostty_vt_available)]
use std::time::{Duration, Instant};

#[cfg(lingxia_ghostty_vt_available)]
static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);
#[cfg(lingxia_ghostty_vt_available)]
static SESSIONS: LazyLock<Mutex<HashMap<u64, Arc<Mutex<TerminalSession>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Clone the session handle out of the registry so per-session I/O
/// never holds the global map lock — a blocked write on one session
/// must not freeze the others.
#[cfg(lingxia_ghostty_vt_available)]
fn session(id: u64) -> Option<Arc<Mutex<TerminalSession>>> {
    SESSIONS.lock().ok()?.get(&id).cloned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalBackend {
    GhosttyVt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendStatus {
    pub backend: TerminalBackend,
    pub available: bool,
    pub status: &'static str,
    pub source_dir: Option<&'static str>,
    pub lib_dir: Option<&'static str>,
}

pub fn ghostty_status() -> BackendStatus {
    BackendStatus {
        backend: TerminalBackend::GhosttyVt,
        available: option_env!("LINGXIA_GHOSTTY_AVAILABLE") == Some("1"),
        status: option_env!("LINGXIA_GHOSTTY_STATUS").unwrap_or("libghostty-vt not prepared"),
        source_dir: option_env!("LINGXIA_GHOSTTY_SOURCE_DIR"),
        lib_dir: option_env!("LINGXIA_GHOSTTY_LIB_DIR"),
    }
}

pub fn ghostty_available() -> bool {
    ghostty_status().available
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GhosttyStatusJson {
    backend: &'static str,
    available: bool,
    status: &'static str,
    source_dir: Option<&'static str>,
    lib_dir: Option<&'static str>,
}

pub fn ghostty_status_json() -> String {
    let status = ghostty_status();
    let json = GhosttyStatusJson {
        backend: "ghostty-vt",
        available: status.available,
        status: status.status,
        source_dir: status.source_dir,
        lib_dir: status.lib_dir,
    };
    serde_json::to_string(&json)
        .unwrap_or_else(|_| r#"{"backend":"ghostty-vt","available":false}"#.to_string())
}

/// Create a cross-platform terminal engine session.
///
/// The engine owns PTY/conpty transport plus libghostty-vt terminal semantics.
/// Platform SDKs should treat the returned JSON snapshots as the stable display
/// contract and keep native code focused on view/input/UX.
#[cfg(lingxia_ghostty_vt_available)]
pub fn terminal_create(cols: u16, rows: u16) -> u64 {
    let cols = cols.max(1);
    let rows = rows.max(1);
    let result = TerminalSession::spawn(cols, rows).and_then(|session| {
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

#[cfg(not(lingxia_ghostty_vt_available))]
pub fn terminal_create(_cols: u16, _rows: u16) -> u64 {
    0
}

#[cfg(lingxia_ghostty_vt_available)]
pub fn terminal_write(id: u64, input: &str) -> bool {
    let Some(session) = session(id) else {
        return false;
    };
    let Ok(mut session) = session.lock() else {
        return false;
    };
    session.write(input.as_bytes()).is_ok()
}

#[cfg(not(lingxia_ghostty_vt_available))]
pub fn terminal_write(_id: u64, _input: &str) -> bool {
    false
}

#[cfg(lingxia_ghostty_vt_available)]
pub fn terminal_read(id: u64) -> String {
    let Some(session) = session(id) else {
        return String::new();
    };
    let Ok(mut session) = session.lock() else {
        return String::new();
    };
    session.drain_text()
}

#[cfg(not(lingxia_ghostty_vt_available))]
pub fn terminal_read(_id: u64) -> String {
    String::new()
}

#[cfg(lingxia_ghostty_vt_available)]
pub fn terminal_snapshot(id: u64) -> String {
    terminal_snapshot_data(id)
        .map(|snapshot| snapshot.to_json())
        .unwrap_or_else(|| TerminalSnapshot::closed().to_json())
}

#[cfg(not(lingxia_ghostty_vt_available))]
pub fn terminal_snapshot(_id: u64) -> String {
    TerminalSnapshot::closed().to_json()
}

/// Structured variant of [`terminal_snapshot`]: returns the snapshot
/// data directly instead of its JSON encoding. `None` when the session
/// does not exist (or its lock is poisoned).
#[cfg(lingxia_ghostty_vt_available)]
pub fn terminal_snapshot_data(session_id: u64) -> Option<TerminalSnapshot> {
    let session = session(session_id)?;
    let mut session = session.lock().ok()?;
    Some(session.drain_snapshot())
}

#[cfg(not(lingxia_ghostty_vt_available))]
pub fn terminal_snapshot_data(_session_id: u64) -> Option<TerminalSnapshot> {
    None
}

#[cfg(lingxia_ghostty_vt_available)]
pub fn terminal_exited(id: u64) -> bool {
    let Some(session) = session(id) else {
        return true;
    };
    let Ok(mut session) = session.lock() else {
        return true;
    };
    session.exited()
}

#[cfg(not(lingxia_ghostty_vt_available))]
pub fn terminal_exited(_id: u64) -> bool {
    true
}

#[cfg(lingxia_ghostty_vt_available)]
pub fn terminal_resize(id: u64, cols: u16, rows: u16) -> bool {
    let Some(session) = session(id) else {
        return false;
    };
    let Ok(mut session) = session.lock() else {
        return false;
    };
    session.resize(cols.max(1), rows.max(1)).is_ok()
}

#[cfg(not(lingxia_ghostty_vt_available))]
pub fn terminal_resize(_id: u64, _cols: u16, _rows: u16) -> bool {
    false
}

/// Handle vertical scroll input at a viewport cell.
///
/// Negative values move up and positive values move down. Applications that
/// requested mouse reporting receive wheel events at `(col, row)`; alternate
/// screens with mode 1007 receive cursor keys. Otherwise this moves the native
/// scrollback viewport. Read-only hosts set `allow_application_input` to false
/// so scrolling never writes to the PTY.
#[cfg(lingxia_ghostty_vt_available)]
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

#[cfg(not(lingxia_ghostty_vt_available))]
pub fn terminal_scroll(
    _id: u64,
    _delta_rows: i32,
    _col: u16,
    _row: u16,
    _allow_application_input: bool,
) -> bool {
    false
}

#[cfg(lingxia_ghostty_vt_available)]
pub fn terminal_close(id: u64) {
    if let Ok(mut sessions) = SESSIONS.lock() {
        sessions.remove(&id);
    }
}

#[cfg(not(lingxia_ghostty_vt_available))]
pub fn terminal_close(_id: u64) {}

/// A structured key event from a host window, to be encoded into the byte
/// sequence a PTY expects.
///
/// Either `character` is set (translated character input, e.g. `WM_CHAR` on
/// Windows) or `vk` carries a Windows virtual-key code for raw key-down
/// input. This type is available regardless of whether a terminal backend
/// is compiled in.
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
        return match character as u32 {
            0x08 => Some("\u{7f}".to_string()),
            0x09 => Some("\t".to_string()),
            0x0d => Some("\r".to_string()),
            0x1b => Some("\u{1b}".to_string()),
            0x01..=0x09 | 0x0b..=0x1a => Some(character.to_string()),
            _ if !character.is_control() => Some(character.to_string()),
            _ => None,
        };
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

#[cfg(lingxia_ghostty_vt_available)]
struct TerminalSession {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    output: Receiver<Vec<u8>>,
    vt: VtScreen,
    title_state: TerminalTitleState,
    _reader: thread::JoinHandle<()>,
}

#[cfg(lingxia_ghostty_vt_available)]
struct TerminalTitleState {
    shell_pid: Option<u32>,
    shell_title: String,
    current_title: String,
    candidate: Option<ForegroundCandidate>,
    generation: u64,
}

#[cfg(lingxia_ghostty_vt_available)]
struct ForegroundCandidate {
    pid: u32,
    name: String,
    first_seen: Instant,
}

#[cfg(lingxia_ghostty_vt_available)]
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

/// Ghostty viewport position in the complete scrollable row space.
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
    pub underline: bool,
    pub inverse: bool,
    pub wide: bool,
}

#[cfg(lingxia_ghostty_vt_available)]
impl TerminalSession {
    fn spawn(cols: u16, rows: u16) -> Result<Self, String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| format!("open pty failed: {err}"))?;

        let shell = resolved_shell();
        let shell_title = process_name_from_path(&shell.path);
        let mut command = CommandBuilder::new(shell.path);
        for arg in shell.args {
            command.arg(arg);
        }
        command.env("TERM", "xterm-ghostty");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "LingXia");
        command.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
        if std::env::var_os("LANG").is_none() {
            command.env("LANG", "en_US.UTF-8");
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
        let writer = Arc::new(Mutex::new(writer));
        let callback_writer = Arc::clone(&writer);
        let write_pty: PtyWriteCallback = Arc::new(move |bytes: &[u8]| {
            if let Ok(mut writer) = callback_writer.lock() {
                let _ = writer.write_all(bytes);
                let _ = writer.flush();
            }
        });
        let theme = terminal_theme();
        let vt = VtScreen::new_with_write_pty(cols, rows, Some(&theme), Some(write_pty))?;

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
            writer,
            output: rx,
            vt,
            title_state,
            _reader: reader_thread,
        })
    }

    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| std::io::Error::other("terminal writer lock poisoned"))?;
        writer.write_all(bytes)?;
        writer.flush()
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
        self.child
            .try_wait()
            .map(|status| status.is_some())
            .unwrap_or(true)
    }

    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), String> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| err.to_string())?;
        self.vt.resize(cols, rows, 1, 1)?;
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

        let mut writer = self
            .writer
            .lock()
            .map_err(|_| std::io::Error::other("terminal writer lock poisoned"))?;
        for _ in 0..count.min(MAX_SCROLL_STEPS) {
            writer.write_all(bytes)?;
        }
        writer.flush()
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
                    inverse: cell.attrs & ATTR_INVERSE != 0,
                    wide: cell.wide,
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
#[cfg(lingxia_ghostty_vt_available)]
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

#[cfg(lingxia_ghostty_vt_available)]
impl Drop for TerminalSession {
    fn drop(&mut self) {
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

#[cfg(lingxia_ghostty_vt_available)]
fn cursor_style_name(style: GhosttyRenderStateCursorVisualStyle) -> &'static str {
    match style {
        GhosttyRenderStateCursorVisualStyle::Bar => "bar",
        GhosttyRenderStateCursorVisualStyle::Block => "block",
        GhosttyRenderStateCursorVisualStyle::Underline => "underline",
        GhosttyRenderStateCursorVisualStyle::BlockHollow => "hollow",
    }
}

#[cfg(lingxia_ghostty_vt_available)]
#[derive(Clone)]
struct TerminalShell {
    path: String,
    args: Vec<String>,
}

#[cfg(lingxia_ghostty_vt_available)]
fn resolved_shell() -> TerminalShell {
    static RESOLVED_SHELL: OnceLock<TerminalShell> = OnceLock::new();

    RESOLVED_SHELL.get_or_init(resolve_shell_uncached).clone()
}

#[cfg(lingxia_ghostty_vt_available)]
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
                args: vec!["-NoLogo".to_string()],
            };
        }
        if command_available("powershell.exe") {
            return TerminalShell {
                path: "powershell.exe".to_string(),
                args: vec!["-NoLogo".to_string()],
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

#[cfg(lingxia_ghostty_vt_available)]
fn env_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(all(lingxia_ghostty_vt_available, windows))]
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

#[cfg(lingxia_ghostty_vt_available)]
fn process_name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("terminal")
        .to_string()
}

#[cfg(lingxia_ghostty_vt_available)]
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

#[cfg(lingxia_ghostty_vt_available)]
fn current_directory_title(pid: Option<u32>) -> Option<String> {
    let pid = pid?;
    process_cwd(pid).map(|path| compact_path_title(&path))
}

#[cfg(lingxia_ghostty_vt_available)]
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

#[cfg(all(
    lingxia_ghostty_vt_available,
    any(target_os = "macos", target_os = "ios")
))]
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

#[cfg(all(lingxia_ghostty_vt_available, target_os = "linux"))]
fn process_cwd(pid: u32) -> Option<std::path::PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
}

#[cfg(all(
    lingxia_ghostty_vt_available,
    not(any(target_os = "macos", target_os = "ios", target_os = "linux"))
))]
fn process_cwd(_pid: u32) -> Option<std::path::PathBuf> {
    None
}

#[cfg(all(
    lingxia_ghostty_vt_available,
    any(target_os = "macos", target_os = "ios")
))]
fn process_name(pid: u32) -> Option<String> {
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

#[cfg(all(lingxia_ghostty_vt_available, target_os = "linux"))]
fn process_name(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(all(
    lingxia_ghostty_vt_available,
    not(any(target_os = "macos", target_os = "ios", target_os = "linux"))
))]
fn process_name(_pid: u32) -> Option<String> {
    None
}

#[cfg(lingxia_ghostty_vt_available)]
fn rgba_alpha(value: u32) -> u8 {
    (value & 0xff) as u8
}

#[cfg(lingxia_ghostty_vt_available)]
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

#[cfg(lingxia_ghostty_vt_available)]
fn terminal_theme() -> ThemeColors {
    let fg = env_rgb("LINGXIA_TERMINAL_FOREGROUND").unwrap_or([0xff, 0xff, 0xff]);
    let bg = env_rgb("LINGXIA_TERMINAL_BACKGROUND").unwrap_or([0x28, 0x2c, 0x34]);
    let mut ansi16 = [
        [0x1d, 0x1f, 0x21],
        [0xcc, 0x66, 0x66],
        [0xb5, 0xbd, 0x68],
        [0xf0, 0xc6, 0x74],
        [0x81, 0xa2, 0xbe],
        [0xb2, 0x94, 0xbb],
        [0x8a, 0xbe, 0xb7],
        [0xc5, 0xc8, 0xc6],
        [0x66, 0x66, 0x66],
        [0xd5, 0x4e, 0x53],
        [0xb9, 0xca, 0x4a],
        [0xe7, 0xc5, 0x47],
        [0x7a, 0xa6, 0xda],
        [0xc3, 0x97, 0xd8],
        [0x70, 0xc0, 0xb1],
        [0xea, 0xea, 0xea],
    ];
    for (index, color) in ansi16.iter_mut().enumerate() {
        if let Some(value) = env_rgb(&format!("LINGXIA_TERMINAL_ANSI_{index}")) {
            *color = value;
        }
    }
    ThemeColors::from_ansi16(fg, bg, ansi16)
}

#[cfg(lingxia_ghostty_vt_available)]
fn env_rgb(key: &str) -> Option<[u8; 3]> {
    std::env::var(key)
        .ok()
        .and_then(|value| parse_hex_rgb(value.trim()))
}

#[cfg(lingxia_ghostty_vt_available)]
fn parse_hex_rgb(value: &str) -> Option<[u8; 3]> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 {
        return None;
    }
    let rgb = u32::from_str_radix(hex, 16).ok()?;
    Some([
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(lingxia_ghostty_vt_available)]
    use std::time::{Duration, Instant};

    #[test]
    fn status_json_is_valid_shape() {
        let json = ghostty_status_json();
        assert!(json.contains(r#""backend":"ghostty-vt""#));
        assert!(json.contains(r#""available":"#));
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
    #[cfg(lingxia_ghostty_vt_available)]
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
    #[cfg(lingxia_ghostty_vt_available)]
    fn ghostty_vt_session_renders_shell_output() {
        let id = terminal_create(80, 24);
        assert_ne!(id, 0);

        assert!(terminal_write(id, "printf 'LINGXIA_GHOSTTY_VT_OK\\n'\n"));
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let snapshot = terminal_snapshot(id);
            if snapshot.contains("LINGXIA_GHOSTTY_VT_OK") {
                terminal_close(id);
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        let snapshot = terminal_snapshot(id);
        terminal_close(id);
        panic!("terminal snapshot did not contain shell output: {snapshot}");
    }
}
