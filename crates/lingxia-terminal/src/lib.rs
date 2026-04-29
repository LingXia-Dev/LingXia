//! Terminal runtime integration for LingXia hosts.
//!
//! Product terminal mode is intentionally single-path:
//! portable-pty owns process I/O, libghostty-vt owns terminal emulation.

mod ghostty_vt;

use ghostty_vt::{
    ATTR_BOLD, ATTR_INVERSE, ATTR_ITALIC, ATTR_UNDERLINE, GhosttyRenderStateCursorVisualStyle,
    PtyWriteCallback, ThemeColors, VtScreen,
};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::Serialize;
use std::collections::HashMap;
#[cfg(any(target_os = "macos", target_os = "ios"))]
use std::ffi::CStr;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;
use std::time::{Duration, Instant};

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);
static SESSIONS: LazyLock<Mutex<HashMap<u64, TerminalSession>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

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

pub fn ghostty_status_json() -> String {
    let status = ghostty_status();
    format!(
        r#"{{"backend":"ghostty-vt","available":{},"status":{},"sourceDir":{},"libDir":{}}}"#,
        if status.available { "true" } else { "false" },
        json_string(status.status),
        json_option_string(status.source_dir),
        json_option_string(status.lib_dir)
    )
}

/// Create a cross-platform terminal engine session.
///
/// The engine owns PTY/conpty transport plus libghostty-vt terminal semantics.
/// Platform SDKs should treat the returned JSON snapshots as the stable display
/// contract and keep native code focused on view/input/UX.
pub fn terminal_create(cols: u16, rows: u16) -> u64 {
    let cols = cols.max(1);
    let rows = rows.max(1);
    match TerminalSession::spawn(cols, rows) {
        Ok(session) => {
            let id = NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut sessions) = SESSIONS.lock() {
                sessions.insert(id, session);
                id
            } else {
                0
            }
        }
        Err(err) => {
            eprintln!("lingxia terminal spawn failed: {err}");
            0
        }
    }
}

pub fn terminal_write(id: u64, input: &str) -> bool {
    let Ok(mut sessions) = SESSIONS.lock() else {
        return false;
    };
    let Some(session) = sessions.get_mut(&id) else {
        return false;
    };
    session.write(input.as_bytes()).is_ok()
}

pub fn terminal_read(id: u64) -> String {
    let Ok(mut sessions) = SESSIONS.lock() else {
        return String::new();
    };
    let Some(session) = sessions.get_mut(&id) else {
        return String::new();
    };
    session.drain_text()
}

pub fn terminal_snapshot(id: u64) -> String {
    let Ok(mut sessions) = SESSIONS.lock() else {
        return TerminalSnapshot::closed().to_json();
    };
    let Some(session) = sessions.get_mut(&id) else {
        return TerminalSnapshot::closed().to_json();
    };
    session.drain_snapshot()
}

pub fn terminal_exited(id: u64) -> bool {
    let Ok(mut sessions) = SESSIONS.lock() else {
        return true;
    };
    let Some(session) = sessions.get_mut(&id) else {
        return true;
    };
    session.exited()
}

pub fn terminal_resize(id: u64, cols: u16, rows: u16) -> bool {
    let Ok(mut sessions) = SESSIONS.lock() else {
        return false;
    };
    let Some(session) = sessions.get_mut(&id) else {
        return false;
    };
    session.resize(cols.max(1), rows.max(1)).is_ok()
}

pub fn terminal_close(id: u64) {
    if let Ok(mut sessions) = SESSIONS.lock() {
        sessions.remove(&id);
    }
}

struct TerminalSession {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    output: Receiver<Vec<u8>>,
    vt: VtScreen,
    title_state: TerminalTitleState,
    _reader: thread::JoinHandle<()>,
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
struct TerminalSnapshot {
    cols: u16,
    rows: u16,
    lines: Vec<String>,
    cells: Vec<TerminalCell>,
    default_foreground: Option<String>,
    default_background: Option<String>,
    cursor_row: u16,
    cursor_col: u16,
    cursor_visible: bool,
    cursor_style: &'static str,
    application_cursor: bool,
    bracketed_paste: bool,
    alternate_screen: bool,
    process_title: Option<String>,
    title: Option<String>,
    generation: u64,
    exited: bool,
}

#[derive(Serialize)]
struct TerminalCell {
    row: u16,
    col: u16,
    text: String,
    fg: Option<String>,
    bg: Option<String>,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    inverse: bool,
    wide: bool,
}

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

        let shell = resolved_shell_path();
        let shell_title = process_name_from_path(&shell);
        let mut command = CommandBuilder::new(shell);
        command.arg("-i");
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

        let (tx, rx) = mpsc::channel();
        let reader_thread = thread::spawn(move || {
            let mut buffer = [0_u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buffer[..n].to_vec()).is_err() {
                            break;
                        }
                    }
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
        String::from_utf8_lossy(&self.drain_bytes()).into_owned()
    }

    fn drain_snapshot(&mut self) -> String {
        let bytes = self.drain_bytes();
        if !bytes.is_empty() {
            self.vt.feed(&bytes);
        }
        self.snapshot().to_json()
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
                    dim: false,
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
            process_title: Some(process_title),
            title: raw_title,
            generation: screen.generation.wrapping_add(title_generation << 48),
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

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
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
            process_title: None,
            title: None,
            generation: 0,
            exited: true,
        }
    }

    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| r#"{"exited":true}"#.to_string())
    }
}

fn cursor_style_name(style: GhosttyRenderStateCursorVisualStyle) -> &'static str {
    match style {
        GhosttyRenderStateCursorVisualStyle::Bar => "bar",
        GhosttyRenderStateCursorVisualStyle::Block => "block",
        GhosttyRenderStateCursorVisualStyle::Underline => "underline",
        GhosttyRenderStateCursorVisualStyle::BlockHollow => "hollow",
    }
}

fn resolved_shell_path() -> String {
    std::env::var("SHELL")
        .ok()
        .map(|shell| shell.trim().to_string())
        .filter(|shell| !shell.is_empty())
        .unwrap_or_else(|| "/bin/sh".to_string())
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
    let path = path.to_string_lossy();
    let Some(home) = std::env::var_os("HOME").filter(|home| !home.is_empty()) else {
        return path.into_owned();
    };
    let home = Path::new(&home).to_string_lossy().into_owned();
    if path == home {
        return "~".to_string();
    }
    if let Some(rest) = path.strip_prefix(&(home + "/")) {
        return format!("~/{rest}");
    }
    path.into_owned()
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

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "linux")))]
fn process_cwd(_pid: u32) -> Option<std::path::PathBuf> {
    None
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
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

fn env_rgb(key: &str) -> Option<[u8; 3]> {
    std::env::var(key)
        .ok()
        .and_then(|value| parse_hex_rgb(value.trim()))
}

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

fn json_option_string(value: Option<&str>) -> String {
    match value {
        Some(value) => json_string(value),
        None => "null".to_string(),
    }
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
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
