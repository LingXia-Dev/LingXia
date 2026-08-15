//! Terminal emulation backed by `alacritty_terminal`.
//!
//! `Term` owns the grid/scrollback/mode state and the re-exported `vte`
//! `Processor` owns escape-sequence parsing (including DEC 2026
//! synchronized-output buffering). This module adapts them to the
//! polling snapshot model the platform SDKs consume: `feed` bytes in,
//! `snapshot` a themed cell grid out.
//!
//! `Term` reports cell colors symbolically (named / indexed / rgb);
//! resolution against the host theme happens here at snapshot time so
//! OSC 4/10/11 overrides tracked by the terminal still win.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{ClipboardType, Config, Term, TermDamage, TermMode};
use alacritty_terminal::vte::ansi::{
    Color as AnsiColor, CursorShape, NamedColor, Processor, Rgb as AnsiRgb, StdSyncHandler,
};
use parking_lot::Mutex;
use serde::Serialize;

use crate::kitty::{GraphicsAnchor, KittyGraphics, TerminalImageSnapshot};
use crate::osc::{OscProgress, OscSemantic, OscTap, TappedControl, parse_osc};
use crate::search::SearchRow;

// Attr bits packed into `Cell.attrs` (bit 0 = bold, 1 = italic, 2 =
// underline, 3 = strike, 4 = inverse, 5 = dim/faint, 6 = hidden).
pub const ATTR_BOLD: u8 = 1 << 0;
pub const ATTR_ITALIC: u8 = 1 << 1;
pub const ATTR_UNDERLINE: u8 = 1 << 2;
pub const ATTR_STRIKE: u8 = 1 << 3;
pub const ATTR_INVERSE: u8 = 1 << 4;
pub const ATTR_DIM: u8 = 1 << 5;
/// SGR 8: the cell keeps its styling but renders no text.
pub const ATTR_HIDDEN: u8 = 1 << 6;

const SCROLLBACK_LINES: usize = 10_000;

/// Cursor shape names shared with the platform renderers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CursorVisualStyle {
    Bar,
    #[default]
    Block,
    Underline,
    BlockHollow,
}

impl CursorVisualStyle {
    /// The name a renderer branches on, and the one the JSON snapshot carries.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bar => "bar",
            Self::Block => "block",
            Self::Underline => "underline",
            Self::BlockHollow => "block_hollow",
        }
    }
}

/// Default colors resolved against cells at snapshot time.
///
/// Indices 0–15 are the user's ANSI theme; 16–231 form the standard
/// xterm 6×6×6 cube; 232–255 form the 24-step grayscale ramp. OSC
/// palette overrides recorded by the terminal take precedence.
#[derive(Debug, Clone)]
pub struct ThemeColors {
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    pub palette: [[u8; 3]; 256],
}

impl ThemeColors {
    /// Build a 256-color palette from a 16-entry ANSI base.
    pub fn from_ansi16(fg: [u8; 3], bg: [u8; 3], ansi16: [[u8; 3]; 16]) -> Self {
        let mut palette = [[0u8; 3]; 256];
        palette[..16].copy_from_slice(&ansi16);
        let step = |x: u8| -> u8 { if x == 0 { 0 } else { 55 + 40 * x } };
        for (i, color) in palette.iter_mut().enumerate().take(232).skip(16) {
            let idx = (i - 16) as u8;
            *color = [step(idx / 36), step((idx / 6) % 6), step(idx % 6)];
        }
        for (i, color) in palette.iter_mut().enumerate().skip(232) {
            let v = 8u8.saturating_add(((i - 232) as u8).saturating_mul(10));
            *color = [v, v, v];
        }
        Self { fg, bg, palette }
    }
}

// ── Snapshot (renderer's view) ─────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    /// Foreground RGBA (0xRRGGBBAA).
    pub fg: u32,
    /// Background RGBA. Alpha 0 marks the default background so the
    /// renderer can apply pane opacity; explicit SGR backgrounds are
    /// opaque.
    pub bg: u32,
    pub attrs: u8,
    /// Which underline SGR is active; `ATTR_UNDERLINE` stays set for any
    /// of them so renderers that only draw one style keep working.
    pub underline: UnderlineStyle,
    /// SGR 58 underline color as RGBA, when it differs from the text
    /// color.
    pub underline_color: Option<u32>,
    /// Grid columns this cell's text occupies: 1 normally, 2 for a wide
    /// char, more for a joined cluster, and 0 for a continuation column
    /// covered by the cell that precedes it. A row's spans sum to the
    /// column count.
    pub columns: u8,
    pub wide: bool,
    /// OSC 8 hyperlink URI attached to the cell.
    pub hyperlink: Option<String>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            text: String::new(),
            fg: 0,
            bg: 0,
            attrs: 0,
            underline: UnderlineStyle::None,
            underline_color: None,
            // An untouched grid slot is still one empty column.
            columns: 1,
            wide: false,
            hyperlink: None,
        }
    }
}

/// Underline shape carried by SGR 4:x.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UnderlineStyle {
    #[default]
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

impl UnderlineStyle {
    fn from_flags(flags: Flags) -> Self {
        // Order matters: alacritty keeps only one underline flag set,
        // but check the specific shapes before the plain one.
        if flags.contains(Flags::DOUBLE_UNDERLINE) {
            Self::Double
        } else if flags.contains(Flags::UNDERCURL) {
            Self::Curly
        } else if flags.contains(Flags::DOTTED_UNDERLINE) {
            Self::Dotted
        } else if flags.contains(Flags::DASHED_UNDERLINE) {
            Self::Dashed
        } else if flags.contains(Flags::UNDERLINE) {
            Self::Single
        } else {
            Self::None
        }
    }

    /// Stable wire name for the host-facing snapshot.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Single => "single",
            Self::Double => "double",
            Self::Curly => "curly",
            Self::Dotted => "dotted",
            Self::Dashed => "dashed",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    pub col: u16,
    pub row: u16,
    pub visible: bool,
    pub style: CursorVisualStyle,
}

#[derive(Debug, Clone, Default)]
pub struct ScreenSnapshot {
    pub cols: u16,
    pub rows: u16,
    pub cells: Vec<Cell>,
    pub cursor: Cursor,
    pub default_fg: u32,
    pub default_bg: u32,
    pub title: Option<String>,
    pub generation: u64,
}

/// One logical line: a shell line as typed, with the rows it wrapped
/// across joined back together.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalLine {
    /// Absolute line where it starts (oldest scrollback line = 0).
    pub line: i64,
    /// Grid rows it spans.
    pub rows: u16,
    pub text: String,
}

/// Read-only text view of a session, the shape an accessibility tree
/// needs: logical lines, where the cursor sits, and what is on screen.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextView {
    pub lines: Vec<LogicalLine>,
    /// Absolute line and cell column of the cursor.
    pub cursor_line: i64,
    pub cursor_column: u16,
    /// Absolute lines currently on screen, inclusive.
    pub viewport_first_line: i64,
    pub viewport_last_line: i64,
    /// Scrollback rows plus screen rows.
    pub total_lines: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ViewportScrollbar {
    pub total: u64,
    pub offset: u64,
    pub len: u64,
}

pub type PtyWriteCallback = Arc<dyn Fn(&[u8]) + Send + Sync + 'static>;

// ── Semantic events (host state/recovery view) ─────────────────────────

/// Maximum queued semantic events per session; oldest are dropped and
/// counted so hosts can detect the gap.
const EVENT_QUEUE_CAPACITY: usize = 4096;
/// Maximum clipboard text surfaced by an OSC 52 store event.
const CLIPBOARD_EVENT_LIMIT: usize = 64 * 1024;

/// Progress/task state carried by OSC 9;4, unified with command
/// completion into one machine:
/// `idle | running(indeterminate|percent) | paused | succeeded | failed`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalProgressState {
    #[default]
    Idle,
    Running,
    Paused,
    Succeeded,
    Failed,
}

/// The session's current progress state, with a percentage when the
/// source reported one (`Running` without a percent is indeterminate).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalProgress {
    pub state: TerminalProgressState,
    pub percent: Option<u8>,
}

/// Everything a host needs to render tab badges and attention state
/// without inspecting output text: the unified progress machine plus
/// monotonic attention counters.
///
/// Counters never reset, so a host that stores the last values it
/// displayed derives "unread" by comparison — and stays correct across
/// dropped events or a re-attach that missed the event stream.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalActivity {
    pub progress: TerminalProgress,
    /// Exit code of the most recent OSC 133 D mark, when it carried one.
    pub last_exit_code: Option<i32>,
    /// BELs received since session start.
    pub bells: u64,
    /// OSC 9/99/777 notifications received since session start.
    pub notifications: u64,
}

/// A typed semantic event extracted from the terminal byte stream.
///
/// These mirror what the bytes *mean* (cwd change, command boundary,
/// progress, notification…) so hosts never re-parse escape sequences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TerminalEventKind {
    Title {
        title: Option<String>,
    },
    /// OSC 7 working-directory report.
    Cwd {
        path: String,
    },
    Bell,
    /// OSC 9;4 progress update.
    Progress {
        state: TerminalProgressState,
        percent: Option<u8>,
    },
    /// OSC 9/99/777 notification, payload sanitized and capped.
    Notification {
        title: Option<String>,
        body: String,
    },
    /// OSC 52 clipboard write request, decoded and length-capped.
    ClipboardStore {
        clipboard: String,
        text: String,
    },
    /// OSC 133 marks; `line` is the absolute grid line (scrollback
    /// lines + screen lines counted from the oldest scrollback line).
    PromptStart {
        line: i64,
    },
    InputStart {
        line: i64,
    },
    OutputStart {
        line: i64,
    },
    CommandFinished {
        line: i64,
        exit_code: Option<i32>,
    },
    /// The session's child process exited.
    Exited {
        exit_code: Option<i32>,
    },
    /// Restored scrollback was replayed into this session; `lines` is
    /// the number of replayed lines, which also marks the boundary
    /// between restored content and fresh shell output.
    Restored {
        lines: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TerminalEvent {
    pub seq: u64,
    #[serde(flatten)]
    pub kind: TerminalEventKind,
}

/// Result of draining the semantic event queue.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalEventBatch {
    /// Sequence number the next produced event will carry.
    pub next_seq: u64,
    /// Events dropped because the queue overflowed since the last drain.
    pub dropped: u64,
    pub events: Vec<TerminalEvent>,
}

#[derive(Default)]
struct EventQueue {
    events: VecDeque<TerminalEvent>,
    next_seq: u64,
    dropped: u64,
}

// ── Command blocks (OSC 133) ───────────────────────────────────────────

/// Maximum completed command blocks retained per session.
const MAX_COMMAND_BLOCKS: usize = 512;

/// One shell command's extent in the scrollback, built from OSC 133
/// marks. Lines are absolute grid coordinates (see
/// [`TerminalEventKind`]); blocks without shell integration simply
/// never appear.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandBlock {
    pub prompt_line: i64,
    pub input_line: Option<i64>,
    pub output_line: Option<i64>,
    pub end_line: Option<i64>,
    pub exit_code: Option<i32>,
}

#[derive(Default)]
struct CommandBlockTracker {
    completed: VecDeque<CommandBlock>,
    current: Option<CommandBlock>,
}

impl CommandBlockTracker {
    fn record(&mut self, kind: &TerminalEventKind) {
        match *kind {
            TerminalEventKind::PromptStart { line } => {
                if let Some(block) = self.current.take() {
                    self.push_completed(block);
                }
                self.current = Some(CommandBlock {
                    prompt_line: line,
                    ..CommandBlock::default()
                });
            }
            TerminalEventKind::InputStart { line } => {
                self.block_at(line).input_line = Some(line);
            }
            TerminalEventKind::OutputStart { line } => {
                self.block_at(line).output_line = Some(line);
            }
            TerminalEventKind::CommandFinished { line, exit_code } => {
                let mut block = self.current.take().unwrap_or_else(|| CommandBlock {
                    prompt_line: line,
                    ..CommandBlock::default()
                });
                block.end_line = Some(line);
                block.exit_code = exit_code;
                self.push_completed(block);
            }
            _ => {}
        }
    }

    /// The open block, creating a loose one when a mark arrives without
    /// a preceding prompt start (e.g. partial integration).
    fn block_at(&mut self, line: i64) -> &mut CommandBlock {
        self.current.get_or_insert_with(|| CommandBlock {
            prompt_line: line,
            ..CommandBlock::default()
        })
    }

    fn push_completed(&mut self, block: CommandBlock) {
        if self.completed.len() >= MAX_COMMAND_BLOCKS {
            self.completed.pop_front();
        }
        self.completed.push_back(block);
    }

    fn blocks(&self) -> Vec<CommandBlock> {
        let mut blocks: Vec<CommandBlock> = self.completed.iter().cloned().collect();
        blocks.extend(self.current.iter().cloned());
        blocks
    }
}

impl EventQueue {
    fn push(&mut self, kind: TerminalEventKind) {
        if self.events.len() >= EVENT_QUEUE_CAPACITY {
            self.events.pop_front();
            self.dropped += 1;
        }
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.events.push_back(TerminalEvent { seq, kind });
    }

    fn drain(&mut self) -> TerminalEventBatch {
        TerminalEventBatch {
            next_seq: self.next_seq,
            dropped: std::mem::take(&mut self.dropped),
            events: self.events.drain(..).collect(),
        }
    }
}

// ── Event listener ─────────────────────────────────────────────────────

/// Requests the terminal raises mid-`advance` that need `Term` state to
/// answer. They are queued and replied to after `advance` returns,
/// when the `Term` borrow is released.
enum PendingReply {
    Color(usize, Arc<dyn Fn(AnsiRgb) -> String + Sync + Send>),
    TextAreaSize(Arc<dyn Fn(WindowSize) -> String + Sync + Send>),
}

struct Listener {
    write_pty: Option<PtyWriteCallback>,
    title: Mutex<Option<String>>,
    events: Mutex<EventQueue>,
    pending: Sender<PendingReply>,
    /// Attention counters; bells arrive here rather than through the OSC
    /// tap, so both live outside the `VtInner` lock.
    bells: AtomicU64,
    notifications: AtomicU64,
}

/// Newtype so the foreign `EventListener` trait can be implemented for
/// a shared listener handle.
#[derive(Clone)]
struct ListenerHandle(Arc<Listener>);

impl std::ops::Deref for ListenerHandle {
    type Target = Listener;

    fn deref(&self) -> &Listener {
        &self.0
    }
}

impl EventListener for ListenerHandle {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => {
                if let Some(write_pty) = &self.write_pty {
                    write_pty(text.as_bytes());
                }
            }
            Event::Title(title) => {
                *self.title.lock() = Some(title.clone());
                self.events
                    .lock()
                    .push(TerminalEventKind::Title { title: Some(title) });
            }
            Event::ResetTitle => {
                *self.title.lock() = None;
                self.events
                    .lock()
                    .push(TerminalEventKind::Title { title: None });
            }
            Event::Bell => {
                self.bells.fetch_add(1, Ordering::Relaxed);
                self.events.lock().push(TerminalEventKind::Bell);
            }
            Event::ClipboardStore(clipboard, text) => {
                let clipboard = match clipboard {
                    ClipboardType::Clipboard => "clipboard",
                    ClipboardType::Selection => "selection",
                }
                .to_string();
                let text: String = text.chars().take(CLIPBOARD_EVENT_LIMIT).collect();
                self.events
                    .lock()
                    .push(TerminalEventKind::ClipboardStore { clipboard, text });
            }
            Event::ColorRequest(index, format) => {
                let _ = self.pending.send(PendingReply::Color(index, format));
            }
            Event::TextAreaSizeRequest(format) => {
                let _ = self.pending.send(PendingReply::TextAreaSize(format));
            }
            _ => {}
        }
    }
}

// ── Safe wrapper ───────────────────────────────────────────────────────

pub struct VtScreen {
    inner: Mutex<VtInner>,
}

struct VtInner {
    term: Term<ListenerHandle>,
    parser: Processor<StdSyncHandler>,
    tap: OscTap,
    graphics: KittyGraphics,
    listener: Arc<Listener>,
    replies: Receiver<PendingReply>,
    theme: ThemeColors,
    cell_width_px: u16,
    cell_height_px: u16,
    generation: u64,
    blocks: CommandBlockTracker,
    /// Last working directory reported via OSC 7.
    cwd: Option<std::path::PathBuf>,
    /// Unified progress state: OSC 9;4 when the program reports it,
    /// otherwise inferred from OSC 133 command boundaries.
    progress: TerminalProgress,
    /// The running command reported OSC 9;4, so command boundaries must
    /// not overwrite its state mid-command.
    progress_reported: bool,
    /// Exit code from the most recent OSC 133 D mark.
    last_exit_code: Option<i32>,
    /// Rows touched since the last frame a renderer consumed.
    damage: DamageSet,
    /// Generation of the last frame handed out, so a renderer that asks
    /// from a different point gets a full redraw instead of a diff
    /// against a frame it never saw.
    last_frame_generation: u64,
    /// Image generation safe to publish outside a DEC 2026 transaction.
    published_image_generation: u64,
}

// ── Renderer frame (damage-tracked, allocation-free cells) ─────────────

/// Per-screen-row damage bounds, in cell columns.
#[derive(Debug, Clone, Default)]
struct DamageSet {
    /// `(left, right_inclusive)` per row; `left > right` means clean.
    rows: Vec<(u16, u16)>,
    full: bool,
    /// Cursor row of the previous frame, redrawn so the old cursor is
    /// erased even when nothing else on that row changed.
    cursor_row: u16,
}

impl DamageSet {
    fn mark_full(&mut self) {
        self.full = true;
    }

    fn expand(&mut self, row: usize, left: usize, right: usize, rows: usize) {
        if row >= rows {
            return;
        }
        if self.rows.len() < rows {
            self.rows.resize(rows, (u16::MAX, 0));
        }
        let entry = &mut self.rows[row];
        entry.0 = entry.0.min(left.min(u16::MAX as usize) as u16);
        entry.1 = entry.1.max(right.min(u16::MAX as usize) as u16);
    }

    fn is_dirty(&self) -> bool {
        self.full || self.rows.iter().any(|(left, right)| left <= right)
    }

    /// Drain into renderer-facing rows and reset to clean.
    fn take(&mut self, rows: u16, cols: u16) -> (Vec<RowDamage>, bool) {
        let full = self.full || self.rows.len() < rows as usize;
        let damage = if full {
            (0..rows)
                .map(|row| RowDamage {
                    row,
                    start_col: 0,
                    end_col: cols,
                })
                .collect()
        } else {
            self.rows
                .iter()
                .enumerate()
                .filter(|(_, (left, right))| left <= right)
                .map(|(row, (left, right))| RowDamage {
                    row: row as u16,
                    start_col: *left,
                    end_col: right.saturating_add(1).min(cols),
                })
                .collect()
        };
        self.full = false;
        self.rows.clear();
        self.rows.resize(rows as usize, (u16::MAX, 0));
        (damage, full)
    }
}

/// A damaged span of one screen row; `end_col` is exclusive.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowDamage {
    pub row: u16,
    pub start_col: u16,
    pub end_col: u16,
}

/// One cell of a [`TerminalFrame`]: fixed size and allocation-free, so a
/// whole frame is two buffers a renderer can upload directly.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameCell {
    /// Foreground RGBA (0xRRGGBBAA).
    pub fg: u32,
    /// Background RGBA; alpha 0 marks the default background.
    pub bg: u32,
    /// SGR 58 underline color; alpha 0 means "use the foreground".
    pub underline_color: u32,
    /// Byte offset of this cell's cluster in [`TerminalFrame::text`].
    pub text_offset: u32,
    /// Byte length of the cluster; 0 for a blank cell.
    pub text_len: u8,
    /// `ATTR_*` bits.
    pub attrs: u8,
    /// [`UnderlineStyle`] as an index.
    pub underline: u8,
    /// Grid columns covered; 0 marks a continuation column.
    pub columns: u8,
}

/// A renderer-ready frame: the full grid plus the rows that changed
/// since the caller's last frame.
///
/// Cell text lives in one `text` blob addressed by offset/length, so a
/// frame costs two allocations instead of one per cell, and a glyph
/// cache can key directly on the returned `&str`.
#[derive(Debug, Clone, Default)]
pub struct TerminalFrame {
    pub cols: u16,
    pub rows: u16,
    pub generation: u64,
    /// `rows * cols` cells, row-major.
    pub cells: Vec<FrameCell>,
    pub text: String,
    /// Rows to repaint. Covers the whole grid when `full_damage`.
    pub damage: Vec<RowDamage>,
    pub full_damage: bool,
    pub cursor: Cursor,
    pub default_fg: u32,
    pub default_bg: u32,
    pub alternate_screen: bool,
}

impl TerminalFrame {
    /// Grid position of the cell at `index`. Cells are row-major, so this is
    /// arithmetic rather than something a cell has to carry.
    pub fn position(&self, index: usize) -> (u16, u16) {
        let cols = usize::from(self.cols).max(1);
        ((index / cols) as u16, (index % cols) as u16)
    }

    /// Cluster text of a cell, empty for blanks and continuation cells.
    pub fn cell_text(&self, cell: &FrameCell) -> &str {
        let start = cell.text_offset as usize;
        let end = start + cell.text_len as usize;
        self.text.get(start..end).unwrap_or("")
    }
}

/// Result of asking a session for the frame after a given generation.
#[derive(Debug, Clone)]
pub enum FrameUpdate {
    /// Nothing changed; the renderer keeps its last frame.
    Unchanged {
        generation: u64,
    },
    Changed(Box<TerminalFrame>),
}

/// Viewport dimensions handed to `Term::new` / `Term::resize`.
struct GridSize {
    columns: usize,
    screen_lines: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.screen_lines
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

fn default_theme() -> ThemeColors {
    // Base16-ish defaults; hosts push their real theme via
    // `new_with_options`.
    ThemeColors::from_ansi16(
        [0xff, 0xff, 0xff],
        [0x28, 0x2c, 0x34],
        [
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
        ],
    )
}

impl VtScreen {
    pub fn new_with_options(
        cols: u16,
        rows: u16,
        theme: Option<&ThemeColors>,
        write_pty: Option<PtyWriteCallback>,
        scrollback_limit: Option<usize>,
    ) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let (pending, replies) = channel();
        let listener = Arc::new(Listener {
            write_pty,
            title: Mutex::new(None),
            events: Mutex::new(EventQueue::default()),
            pending,
            bells: AtomicU64::new(0),
            notifications: AtomicU64::new(0),
        });
        let config = Config {
            scrolling_history: scrollback_limit.unwrap_or(SCROLLBACK_LINES),
            ..Config::default()
        };
        let size = GridSize {
            columns: cols as usize,
            screen_lines: rows as usize,
        };
        let term = Term::new(config, &size, ListenerHandle(Arc::clone(&listener)));
        Self {
            inner: Mutex::new(VtInner {
                term,
                parser: Processor::new(),
                tap: OscTap::default(),
                graphics: KittyGraphics::default(),
                listener,
                replies,
                theme: theme.cloned().unwrap_or_else(default_theme),
                cell_width_px: 1,
                cell_height_px: 1,
                generation: 0,
                blocks: CommandBlockTracker::default(),
                cwd: None,
                progress: TerminalProgress::default(),
                progress_reported: false,
                last_exit_code: None,
                // A renderer that has drawn nothing needs a full frame,
                // even before any output arrives.
                damage: DamageSet {
                    full: true,
                    ..DamageSet::default()
                },
                last_frame_generation: 0,
                published_image_generation: 0,
            }),
        }
    }

    /// Feed bytes from the PTY into the parser.
    ///
    /// The OSC tap runs alongside the parser so semantic sequences the
    /// emulator drops (OSC 7/9/99/133/777) still produce typed events.
    /// Bytes are advanced through each tapped sequence before its semantics
    /// are recorded, giving marks an exact grid position while preserving
    /// the parser's control-string state across PTY reads. DEC 2026 buffering
    /// is briefly drained around tapped controls so their positions observe
    /// all preceding bytes in the synchronized frame.
    pub fn feed(&self, bytes: &[u8]) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;
        let tapped = inner.tap.feed_controls(bytes);
        let linefeeds = inner.tap.take_linefeeds();
        let cell_size_queries = inner.tap.take_cell_size_queries();
        inner.answer_cell_size_queries(cell_size_queries);
        let mut last = 0;
        for control in tapped {
            let (start, end) = match &control {
                TappedControl::Osc(osc) => (osc.start, osc.end),
                TappedControl::KittyGraphics { start, end, .. } => (*start, *end),
                TappedControl::ClearScreen { start, end, .. } => (*start, *end),
            };
            if start < last {
                // The sequence started in an earlier feed call and its
                // bytes were already parsed; only record its semantics.
                inner.record_control(control);
                continue;
            }
            inner.advance_grid(&bytes[last..start], &linefeeds, last);
            let synchronized = inner.parser.sync_timeout().sync_timeout().is_some();
            if synchronized {
                inner.parser.stop_sync(&mut inner.term);
            }
            inner.advance_grid(&bytes[start..end], &linefeeds, start);
            inner.record_control(control);
            if synchronized {
                inner.parser.advance(&mut inner.term, b"\x1b[?2026h");
            }
            last = end;
        }
        inner.advance_grid(&bytes[last..], &linefeeds, last);
        inner.answer_pending_replies();
        if inner.parser.sync_timeout().sync_timeout().is_none() {
            inner.publish_update();
        }
    }

    /// Push an externally-sourced event (e.g. process exit) into the
    /// session's semantic event queue.
    pub fn push_event(&self, kind: TerminalEventKind) {
        self.inner.lock().listener.events.lock().push(kind);
    }

    /// Drain all pending semantic events.
    pub fn drain_events(&self) -> TerminalEventBatch {
        self.inner.lock().listener.events.lock().drain()
    }

    /// Completed command blocks plus the open one, oldest first.
    pub fn command_blocks(&self) -> Vec<CommandBlock> {
        self.inner.lock().blocks.blocks()
    }

    /// The title the application set via OSC 0/2, without building a
    /// snapshot.
    pub fn osc_title(&self) -> Option<String> {
        self.inner.lock().listener.title.lock().clone()
    }

    /// The last working directory reported via OSC 7.
    pub fn cwd(&self) -> Option<std::path::PathBuf> {
        self.inner.lock().cwd.clone()
    }

    /// Replace the palette cell colors resolve against. Cheap by
    /// construction: the grid stores colors symbolically, so a theme
    /// change is a repaint and never a reflow.
    pub fn set_theme(&self, theme: ThemeColors) {
        let mut inner = self.inner.lock();
        inner.theme = theme;
        // Every resolved color changes, so nothing on screen is reusable.
        inner.damage.mark_full();
        inner.generation = inner.generation.wrapping_add(1);
    }

    /// Current progress and attention counters.
    pub fn activity(&self) -> TerminalActivity {
        let inner = self.inner.lock();
        TerminalActivity {
            progress: inner.progress,
            last_exit_code: inner.last_exit_code,
            bells: inner.listener.bells.load(Ordering::Relaxed),
            notifications: inner.listener.notifications.load(Ordering::Relaxed),
        }
    }

    /// Text of the live screen rows only (the tail of [`Self::grid_text`]).
    pub fn screen_text(&self) -> Vec<SearchRow> {
        let rows = self.grid_text();
        let screen_lines = self.inner.lock().term.grid().screen_lines();
        let skip = rows.len().saturating_sub(screen_lines);
        rows.into_iter().skip(skip).collect()
    }

    /// OSC 8 hyperlink ranges on the live screen: (absolute line,
    /// start column, end column exclusive, URI).
    pub fn screen_hyperlinks(&self) -> Vec<(i64, u16, u16, String)> {
        let inner = self.inner.lock();
        let grid = inner.term.grid();
        let history = grid.history_size() as i64;
        let screen_lines = grid.screen_lines() as i64;
        let mut links = Vec::new();
        for offset in 0..screen_lines {
            let row = &grid[Line(offset as i32)];
            let mut active: Option<(u16, String)> = None;
            for col in 0..row.len() {
                let cell = &row[Column(col)];
                if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    continue;
                }
                let uri = cell.hyperlink().map(|link| link.uri().to_owned());
                match (&active, uri) {
                    (Some((_, active_uri)), Some(uri)) if uri == *active_uri => {}
                    (Some(_), uri) => {
                        let (start, active_uri) = active.take().expect("matched Some");
                        links.push((history + offset, start, col as u16, active_uri));
                        active = uri.map(|uri| (col as u16, uri));
                    }
                    (None, uri) => active = uri.map(|uri| (col as u16, uri)),
                }
            }
            if let Some((start, uri)) = active {
                links.push((history + offset, start, row.len() as u16, uri));
            }
        }
        links
    }

    /// Copy the full scrollback + screen text for searching/export.
    ///
    /// Cell columns and wide-char widths are preserved per character so
    /// offsets map back to highlightable cell ranges; absolute lines
    /// count from the oldest scrollback line.
    pub fn grid_text(&self) -> Vec<SearchRow> {
        let inner = self.inner.lock();
        let grid = inner.term.grid();
        let history = grid.history_size() as i64;
        let screen_lines = grid.screen_lines() as i64;
        let mut rows = Vec::with_capacity((history + screen_lines).max(0) as usize);
        for offset in -history..screen_lines {
            rows.push(row_search_text(
                &grid[Line(offset as i32)],
                history + offset,
            ));
        }
        rows
    }

    /// Logical lines around `start_line` for an accessibility tree,
    /// with the cursor and the visible range.
    ///
    /// `start_line` defaults to the first visible line; wrapped rows are
    /// joined into the logical line they belong to, so the walk starts
    /// at that line's real beginning. At most `max_lines` logical lines
    /// are returned, which keeps a query off the full scrollback.
    pub fn text_view(&self, start_line: Option<i64>, max_lines: usize) -> TextView {
        let inner = self.inner.lock();
        let grid = inner.term.grid();
        let history = grid.history_size() as i64;
        let screen_lines = grid.screen_lines() as i64;
        let total_lines = history + screen_lines;
        let viewport_first_line = history - grid.display_offset() as i64;
        let row_at = |line: i64| &grid[Line((line - history) as i32)];

        let mut start = start_line
            .unwrap_or(viewport_first_line)
            .clamp(0, total_lines.saturating_sub(1).max(0));
        // Wrapped rows belong to the logical line that started earlier.
        while start > 0
            && row_at(start - 1)
                .last()
                .is_some_and(|cell| cell.flags.contains(Flags::WRAPLINE))
        {
            start -= 1;
        }

        let mut lines: Vec<LogicalLine> = Vec::new();
        let mut line = start;
        while line < total_lines && lines.len() < max_lines {
            let mut logical = LogicalLine {
                line,
                rows: 0,
                text: String::new(),
            };
            loop {
                let row = row_search_text(row_at(line), line);
                logical.text.push_str(&row.text);
                logical.rows += 1;
                line += 1;
                if !row.wraps || line >= total_lines {
                    break;
                }
            }
            lines.push(logical);
        }

        TextView {
            lines,
            cursor_line: history + grid.cursor.point.line.0 as i64,
            cursor_column: grid.cursor.point.column.0.min(u16::MAX as usize) as u16,
            viewport_first_line,
            viewport_last_line: viewport_first_line + screen_lines - 1,
            total_lines,
        }
    }

    pub fn resize(
        &self,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result<(), String> {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let mut inner = self.inner.lock();
        let grid_changed =
            inner.term.columns() != cols as usize || inner.term.screen_lines() != rows as usize;
        if grid_changed {
            inner.graphics.clear_physical_placements();
            inner.published_image_generation = inner.graphics.generation();
        }
        inner.term.resize(GridSize {
            columns: cols as usize,
            screen_lines: rows as usize,
        });
        inner.cell_width_px = cell_width_px.clamp(1, u16::MAX as u32) as u16;
        inner.cell_height_px = cell_height_px.clamp(1, u16::MAX as u32) as u16;
        inner.damage.mark_full();
        inner.generation = inner.generation.wrapping_add(1);
        Ok(())
    }

    /// The renderer's view of the grid, diffed against the frame the
    /// caller last drew.
    ///
    /// Returns [`FrameUpdate::Unchanged`] when nothing moved since
    /// `since_generation`, so a polling renderer does no work on a quiet
    /// frame. A `since_generation` other than the last frame handed out
    /// forces a full repaint — a renderer that lost its state, or a new
    /// one attaching, must not receive a diff against a frame it never
    /// saw.
    pub fn frame(&self, since_generation: u64) -> FrameUpdate {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;
        inner.flush_expired_sync();

        if inner.parser.sync_timeout().sync_timeout().is_some() {
            return FrameUpdate::Unchanged {
                generation: inner.generation,
            };
        }

        let resumed = since_generation != inner.last_frame_generation;
        if !resumed && since_generation == inner.generation && !inner.damage.is_dirty() {
            return FrameUpdate::Unchanged {
                generation: inner.generation,
            };
        }
        if resumed {
            inner.damage.mark_full();
        }

        let cols = inner.term.columns().min(u16::MAX as usize) as u16;
        let rows = inner.term.screen_lines().min(u16::MAX as usize) as u16;
        let total = cols as usize * rows as usize;

        let content = inner.term.renderable_content();
        let display_offset = content.display_offset;
        let colors = content.colors;
        let theme = &inner.theme;
        let default_fg = colors[NamedColor::Foreground]
            .map(|rgb| [rgb.r, rgb.g, rgb.b])
            .unwrap_or(theme.fg);
        let default_bg = colors[NamedColor::Background]
            .map(|rgb| [rgb.r, rgb.g, rgb.b])
            .unwrap_or(theme.bg);
        let resolve = resolver(colors, theme);

        // The blob is filled in column order, which is what lets a
        // joined cluster be described by widening the leader's length.
        let mut text = String::with_capacity(total);
        let mut cells = vec![FrameCell::default(); total];
        let mut ordered: Vec<(usize, &alacritty_terminal::term::cell::Cell)> = Vec::new();
        for indexed in content.display_iter {
            let viewport_row = indexed.point.line.0 + display_offset as i32;
            if viewport_row < 0 || viewport_row >= rows as i32 {
                continue;
            }
            let col = indexed.point.column.0;
            if col >= cols as usize {
                continue;
            }
            ordered.push((viewport_row as usize * cols as usize + col, indexed.cell));
        }
        ordered.sort_unstable_by_key(|(index, _)| *index);
        for (index, cell) in ordered {
            cells[index] = convert_frame_cell(cell, &resolve, default_fg, default_bg, &mut text);
        }
        for row in cells.chunks_mut(cols as usize) {
            join_zwj_frame_cells(row, &text);
        }

        let cursor = frame_cursor(content.cursor, content.mode, display_offset, rows);
        let (damage, full_damage) = inner.damage.take(rows, cols);
        inner.last_frame_generation = inner.generation;

        FrameUpdate::Changed(Box::new(TerminalFrame {
            cols,
            rows,
            generation: inner.generation,
            cells,
            text,
            damage,
            full_damage,
            cursor,
            default_fg: pack_rgb(default_fg, 0xFF),
            default_bg: pack_rgb(default_bg, 0xFF),
            alternate_screen: content.mode.contains(TermMode::ALT_SCREEN),
        }))
    }

    pub fn snapshot(&self) -> ScreenSnapshot {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;
        inner.flush_expired_sync();

        let cols = inner.term.columns().min(u16::MAX as usize) as u16;
        let rows = inner.term.screen_lines().min(u16::MAX as usize) as u16;
        let total = cols as usize * rows as usize;

        let content = inner.term.renderable_content();
        let display_offset = content.display_offset;
        let colors = content.colors;
        let theme = &inner.theme;

        let default_fg = colors[NamedColor::Foreground]
            .map(|rgb| [rgb.r, rgb.g, rgb.b])
            .unwrap_or(theme.fg);
        let default_bg = colors[NamedColor::Background]
            .map(|rgb| [rgb.r, rgb.g, rgb.b])
            .unwrap_or(theme.bg);

        let resolve = |color: AnsiColor, base: [u8; 3]| -> [u8; 3] {
            match color {
                AnsiColor::Spec(rgb) => [rgb.r, rgb.g, rgb.b],
                AnsiColor::Indexed(index) => colors[index as usize]
                    .map(|rgb| [rgb.r, rgb.g, rgb.b])
                    .unwrap_or(theme.palette[index as usize]),
                AnsiColor::Named(named) => {
                    let index = named as usize;
                    if index < 256 {
                        colors[index]
                            .map(|rgb| [rgb.r, rgb.g, rgb.b])
                            .unwrap_or(theme.palette[index])
                    } else {
                        base
                    }
                }
            }
        };

        let mut cells = vec![Cell::default(); total];
        for indexed in content.display_iter {
            let viewport_row = indexed.point.line.0 + display_offset as i32;
            if viewport_row < 0 || viewport_row >= rows as i32 {
                continue;
            }
            let col = indexed.point.column.0;
            if col >= cols as usize {
                continue;
            }
            cells[viewport_row as usize * cols as usize + col] =
                convert_cell(indexed.cell, &resolve, default_fg, default_bg);
        }
        for row in cells.chunks_mut(cols as usize) {
            join_zwj_clusters(row);
        }

        // Cursor in viewport coordinates; scrolled-back history must
        // not show the live cursor.
        let cursor_row = content.cursor.point.line.0 + display_offset as i32;
        let cursor_visible = content.cursor.shape != CursorShape::Hidden
            && content.mode.contains(TermMode::SHOW_CURSOR)
            && (0..rows as i32).contains(&cursor_row);
        let cursor = Cursor {
            col: content.cursor.point.column.0.min(u16::MAX as usize) as u16,
            row: cursor_row.clamp(0, rows.saturating_sub(1) as i32) as u16,
            visible: cursor_visible,
            style: match content.cursor.shape {
                CursorShape::Beam => CursorVisualStyle::Bar,
                CursorShape::Underline => CursorVisualStyle::Underline,
                CursorShape::HollowBlock => CursorVisualStyle::BlockHollow,
                CursorShape::Block | CursorShape::Hidden => CursorVisualStyle::Block,
            },
        };

        ScreenSnapshot {
            cols,
            rows,
            cells,
            cursor,
            default_fg: pack_rgb(default_fg, 0xFF),
            default_bg: pack_rgb(default_bg, 0xFF),
            title: inner.listener.title.lock().clone(),
            generation: inner.generation,
        }
    }

    /// Returns `true` when at least one mouse-tracking mode is set.
    /// Host-view mouse handlers gate mouse reporting on this so wheel /
    /// click / move don't leak escape sequences into shells that
    /// didn't ask for them.
    pub fn mouse_tracking_active(&self) -> bool {
        self.mode_active(TermMode::MOUSE_MODE)
    }

    /// SGR (1006) mouse format is the extended coord encoding.
    pub fn is_sgr_mouse(&self) -> bool {
        self.mode_active(TermMode::SGR_MOUSE)
    }

    /// Alt-screen scroll (1007): mouse wheel in alt-screen apps is
    /// translated to arrow keys. Apps like less / vim opt in.
    pub fn is_alt_scroll(&self) -> bool {
        self.mode_active(TermMode::ALTERNATE_SCROLL)
    }

    /// Bracketed-paste mode (2004): paste payloads should be wrapped
    /// in `ESC[200~ … ESC[201~`.
    pub fn is_bracketed_paste(&self) -> bool {
        self.mode_active(TermMode::BRACKETED_PASTE)
    }

    /// DECCKM (mode 1): arrow keys must use the application-cursor
    /// encoding (`ESC O A/B/C/D`).
    pub fn is_decckm(&self) -> bool {
        self.mode_active(TermMode::APP_CURSOR)
    }

    /// Returns `true` while the alternate screen buffer is active.
    pub fn is_alternate_screen(&self) -> bool {
        self.mode_active(TermMode::ALT_SCREEN)
    }

    fn mode_active(&self, mode: TermMode) -> bool {
        self.inner.lock().term.mode().intersects(mode)
    }

    /// Current viewport position in the full scrollable row space,
    /// offset measured from the top.
    pub fn scrollbar(&self) -> Option<ViewportScrollbar> {
        let inner = self.inner.lock();
        let grid = inner.term.grid();
        let total = grid.total_lines() as u64;
        let len = grid.screen_lines() as u64;
        let offset = total
            .saturating_sub(len)
            .saturating_sub(grid.display_offset() as u64);
        Some(ViewportScrollbar { total, offset, len })
    }

    /// Scroll the visible viewport by terminal rows. Negative is up;
    /// positive is down. Returns `true` when a scroll was applied.
    pub fn scroll_viewport_delta(&self, delta_rows: isize) -> bool {
        if delta_rows == 0 {
            return false;
        }
        let mut inner = self.inner.lock();
        // Scroll::Delta counts toward history (up); our callers pass
        // negative-up deltas.
        let delta =
            i32::try_from(-delta_rows).unwrap_or(if delta_rows < 0 { i32::MAX } else { i32::MIN });
        inner.term.scroll_display(Scroll::Delta(delta));
        // The viewport moved over the grid: every row shows new content.
        inner.damage.mark_full();
        inner.generation = inner.generation.wrapping_add(1);
        true
    }

    /// Put an absolute retained line at the top of the viewport.
    pub fn scroll_viewport_to_line(&self, line: i64) -> bool {
        let mut inner = self.inner.lock();
        let grid = inner.term.grid();
        let max_top = grid.total_lines().saturating_sub(grid.screen_lines()) as i64;
        let target = line.clamp(0, max_top);
        let current = max_top.saturating_sub(grid.display_offset() as i64);
        if target == current {
            return false;
        }
        let delta_rows = target.saturating_sub(current);
        let delta =
            i32::try_from(-delta_rows).unwrap_or(if delta_rows < 0 { i32::MAX } else { i32::MIN });
        inner.term.scroll_display(Scroll::Delta(delta));
        inner.damage.mark_full();
        inner.generation = inner.generation.wrapping_add(1);
        true
    }

    pub fn image_generation(&self) -> u64 {
        self.inner.lock().published_image_generation
    }

    pub fn image_snapshot(&self, since_generation: u64) -> TerminalImageSnapshot {
        let inner = self.inner.lock();
        if inner.parser.sync_timeout().sync_timeout().is_some()
            || since_generation == inner.published_image_generation
        {
            TerminalImageSnapshot {
                changed: false,
                generation: inner.published_image_generation,
                ..TerminalImageSnapshot::default()
            }
        } else {
            // Only expose graphics that crossed the same published VT
            // boundary as the paired frame. Comparing against the live
            // graphics generation here can incorrectly report `unchanged`
            // when the host already stored the pre-update generation.
            inner
                .graphics
                .snapshot(inner.published_image_generation.wrapping_sub(1))
        }
    }
}

impl VtInner {
    fn answer_cell_size_queries(&mut self, count: usize) {
        if count > 0
            && let Some(write_pty) = &self.listener.write_pty
        {
            let response = format!(
                "\x1b[6;{};{}t",
                self.cell_height_px.max(1),
                self.cell_width_px.max(1)
            );
            for _ in 0..count {
                write_pty(response.as_bytes());
            }
        }
    }

    fn advance_grid(&mut self, bytes: &[u8], linefeeds: &[usize], offset: usize) {
        let mut start = 0;
        for index in linefeeds
            .iter()
            .copied()
            .filter(|index| *index >= offset && *index < offset + bytes.len())
            .map(|index| index - offset)
        {
            self.parser.advance(&mut self.term, &bytes[start..index]);
            let at_bottom = self.term.mode().intersects(TermMode::ALT_SCREEN)
                && self.term.grid().cursor.point.line.0
                    >= self.term.screen_lines().saturating_sub(1) as i32;
            self.parser.advance(&mut self.term, &bytes[index..=index]);
            if at_bottom {
                self.graphics.scroll_alternate_screen(1);
            }
            start = index + 1;
        }
        self.parser.advance(&mut self.term, &bytes[start..]);
    }

    fn record_control(&mut self, control: TappedControl) {
        match control {
            TappedControl::Osc(osc) => self.record_osc(&osc.body),
            TappedControl::KittyGraphics { body, .. } => self.record_kitty_graphics(&body),
            TappedControl::ClearScreen {
                mode, erased_lines, ..
            } => {
                let cursor = self.term.grid().cursor.point;
                if mode == 2
                    || (mode == 0 && cursor.line.0 == 0 && cursor.column.0 == 0)
                    || (mode == 3 && erased_lines >= self.term.screen_lines())
                {
                    self.graphics.clear_physical_placements();
                }
            }
        }
    }

    fn record_kitty_graphics(&mut self, body: &[u8]) {
        let grid = self.term.grid();
        let anchor = GraphicsAnchor {
            line: grid.history_size() as i64 + grid.cursor.point.line.0 as i64,
            col: grid.cursor.point.column.0.min(u16::MAX as usize) as u16,
            alternate_screen: self.term.mode().intersects(TermMode::ALT_SCREEN),
        };
        let result = self.graphics.handle(
            body,
            anchor,
            self.cell_width_px.max(1),
            self.cell_height_px.max(1),
        );
        if let Some(response) = result.response
            && let Some(write_pty) = &self.listener.write_pty
        {
            write_pty(&response);
        }
        if let Some((columns, rows)) = result.cursor_move {
            let movement = format!("\x1b[{columns}C\x1b[{rows}B");
            self.parser.advance(&mut self.term, movement.as_bytes());
        }
    }

    /// Record the semantics of a tapped OSC sequence, positioned at the
    /// current grid state (the parser has already consumed all
    /// preceding bytes).
    fn record_osc(&mut self, body: &[u8]) {
        let Some(semantic) = parse_osc(body) else {
            return;
        };
        let absolute_line = || {
            let grid = self.term.grid();
            grid.history_size() as i64 + i64::from(grid.cursor.point.line.0)
        };
        let kind = match semantic {
            OscSemantic::Cwd(path) => {
                self.cwd = Some(std::path::PathBuf::from(&path));
                TerminalEventKind::Cwd { path }
            }
            OscSemantic::Progress(progress) => {
                let (state, percent) = match progress {
                    OscProgress::Idle => (TerminalProgressState::Idle, None),
                    // OSC 9;4 has no explicit completion state; a set
                    // value reaching 100% is the protocol's completion
                    // signal.
                    OscProgress::Running { percent: Some(100) } => {
                        (TerminalProgressState::Succeeded, Some(100))
                    }
                    OscProgress::Running { percent } => (TerminalProgressState::Running, percent),
                    OscProgress::Paused { percent } => (TerminalProgressState::Paused, percent),
                    OscProgress::Failed => (TerminalProgressState::Failed, None),
                };
                self.progress = TerminalProgress { state, percent };
                self.progress_reported = state != TerminalProgressState::Idle;
                TerminalEventKind::Progress { state, percent }
            }
            OscSemantic::Notification { title, body } => {
                self.listener.notifications.fetch_add(1, Ordering::Relaxed);
                TerminalEventKind::Notification { title, body }
            }
            OscSemantic::PromptStart => TerminalEventKind::PromptStart {
                line: absolute_line(),
            },
            OscSemantic::InputStart => TerminalEventKind::InputStart {
                line: absolute_line(),
            },
            OscSemantic::OutputStart => {
                // A command starts: indeterminate until it reports its
                // own progress, and the previous command's OSC 9;4 state
                // no longer applies.
                self.progress_reported = false;
                self.progress = TerminalProgress {
                    state: TerminalProgressState::Running,
                    percent: None,
                };
                TerminalEventKind::OutputStart {
                    line: absolute_line(),
                }
            }
            OscSemantic::CommandFinished { exit_code } => {
                self.last_exit_code = exit_code;
                self.progress_reported = false;
                // Completion outranks any progress the command reported.
                // An absent exit code says "finished, outcome unknown",
                // which is idle rather than a success badge.
                self.progress = TerminalProgress {
                    state: match exit_code {
                        Some(0) => TerminalProgressState::Succeeded,
                        Some(_) => TerminalProgressState::Failed,
                        None => TerminalProgressState::Idle,
                    },
                    percent: None,
                };
                TerminalEventKind::CommandFinished {
                    line: absolute_line(),
                    exit_code,
                }
            }
        };
        self.blocks.record(&kind);
        self.listener.events.lock().push(kind);
    }

    /// An application that enters synchronized output (DEC 2026) and
    /// never leaves keeps bytes buffered inside the parser. Flush once
    /// vte's own deadline passes so the screen can't freeze.
    fn flush_expired_sync(&mut self) {
        if let Some(deadline) = self.parser.sync_timeout().sync_timeout()
            && Instant::now() >= deadline
        {
            self.parser.stop_sync(&mut self.term);
            self.answer_pending_replies();
            self.publish_update();
        }
    }

    fn publish_update(&mut self) {
        self.collect_damage();
        self.generation = self.generation.wrapping_add(1);
        self.published_image_generation = self.graphics.generation();
    }

    /// Fold the terminal's per-line damage into our own set.
    ///
    /// `Term` clears its damage on read, so it is accumulated here
    /// instead: a renderer polls on its own cadence and must not lose
    /// the rows that changed between two of its frames.
    fn collect_damage(&mut self) {
        let rows = self.term.screen_lines();
        let cursor_row = self.term.grid().cursor.point.line.0.max(0) as usize;
        match self.term.damage() {
            TermDamage::Full => self.damage.mark_full(),
            TermDamage::Partial(lines) => {
                let lines: Vec<_> = lines.collect();
                for line in lines {
                    self.damage.expand(line.line, line.left, line.right, rows);
                }
            }
        }
        self.term.reset_damage();
        // Repaint where the cursor was and where it is, so it is erased
        // from its old row even when that row's text did not change.
        let columns = self.term.columns().saturating_sub(1);
        let previous = self.damage.cursor_row as usize;
        self.damage.expand(previous, 0, columns, rows);
        self.damage.expand(cursor_row, 0, columns, rows);
        self.damage.cursor_row = cursor_row.min(u16::MAX as usize) as u16;
    }

    /// Answer queued OSC color / size queries. Runs after
    /// `parser.advance` returns so `Term` state is readable again.
    fn answer_pending_replies(&mut self) {
        while let Ok(reply) = self.replies.try_recv() {
            let Some(write_pty) = &self.listener.write_pty else {
                continue;
            };
            let response = match reply {
                PendingReply::Color(index, format) => {
                    let rgb = self.term.colors()[index].unwrap_or_else(|| {
                        let [r, g, b] = match index {
                            idx @ 0..=255 => self.theme.palette[idx],
                            idx if idx == NamedColor::Background as usize => self.theme.bg,
                            _ => self.theme.fg,
                        };
                        AnsiRgb { r, g, b }
                    });
                    format(rgb)
                }
                PendingReply::TextAreaSize(format) => format(WindowSize {
                    num_lines: self.term.screen_lines().min(u16::MAX as usize) as u16,
                    num_cols: self.term.columns().min(u16::MAX as usize) as u16,
                    cell_width: self.cell_width_px,
                    cell_height: self.cell_height_px,
                }),
            };
            write_pty(response.as_bytes());
        }
    }
}

type ResolveColor<'a> = dyn Fn(AnsiColor, [u8; 3]) -> [u8; 3] + 'a;

/// Resolve a symbolic cell color against OSC overrides, then the theme.
fn resolver<'a>(
    colors: &'a alacritty_terminal::term::color::Colors,
    theme: &'a ThemeColors,
) -> impl Fn(AnsiColor, [u8; 3]) -> [u8; 3] + 'a {
    move |color: AnsiColor, base: [u8; 3]| match color {
        AnsiColor::Spec(rgb) => [rgb.r, rgb.g, rgb.b],
        AnsiColor::Indexed(index) => colors[index as usize]
            .map(|rgb| [rgb.r, rgb.g, rgb.b])
            .unwrap_or(theme.palette[index as usize]),
        AnsiColor::Named(named) => {
            let index = named as usize;
            if index < 256 {
                colors[index]
                    .map(|rgb| [rgb.r, rgb.g, rgb.b])
                    .unwrap_or(theme.palette[index])
            } else {
                base
            }
        }
    }
}

/// Cursor in viewport coordinates; scrolled-back history must not show
/// the live cursor.
fn frame_cursor(
    cursor: alacritty_terminal::term::RenderableCursor,
    mode: TermMode,
    display_offset: usize,
    rows: u16,
) -> Cursor {
    let row = cursor.point.line.0 + display_offset as i32;
    Cursor {
        col: cursor.point.column.0.min(u16::MAX as usize) as u16,
        row: row.clamp(0, rows.saturating_sub(1) as i32) as u16,
        visible: cursor.shape != CursorShape::Hidden
            && mode.contains(TermMode::SHOW_CURSOR)
            && (0..rows as i32).contains(&row),
        style: match cursor.shape {
            CursorShape::Beam => CursorVisualStyle::Bar,
            CursorShape::Underline => CursorVisualStyle::Underline,
            CursorShape::HollowBlock => CursorVisualStyle::BlockHollow,
            CursorShape::Block | CursorShape::Hidden => CursorVisualStyle::Block,
        },
    }
}

/// Longest cluster a [`FrameCell`] can address; longer ones are cut at a
/// char boundary rather than corrupting the blob's offsets.
const MAX_CLUSTER_BYTES: usize = u8::MAX as usize;

/// Convert one grid cell, appending its cluster to the frame's blob.
fn convert_frame_cell(
    cell: &alacritty_terminal::term::cell::Cell,
    resolve: &impl Fn(AnsiColor, [u8; 3]) -> [u8; 3],
    default_fg: [u8; 3],
    default_bg: [u8; 3],
    text: &mut String,
) -> FrameCell {
    let converted = convert_cell(cell, resolve, default_fg, default_bg);
    let placeholder = cell.c == '\u{10EEEE}';
    let display_text = &converted.text;
    let offset = text.len().min(u32::MAX as usize) as u32;
    let mut len = display_text.len();
    if len > MAX_CLUSTER_BYTES {
        len = MAX_CLUSTER_BYTES;
        while len > 0 && !display_text.is_char_boundary(len) {
            len -= 1;
        }
    }
    text.push_str(&display_text[..len]);
    FrameCell {
        // Placeholder colors are protocol identifiers, not display colors.
        // Preserve indexed IDs instead of resolving them through the palette.
        fg: if placeholder {
            placeholder_color_id(cell.fg).unwrap_or(converted.fg >> 8) << 8 | 0xFF
        } else {
            converted.fg
        },
        bg: converted.bg,
        underline_color: if placeholder {
            cell.underline_color()
                .and_then(placeholder_color_id)
                .map_or(0, |id| id << 8 | 0xFF)
        } else {
            converted.underline_color.unwrap_or(0)
        },
        text_offset: offset,
        text_len: len as u8,
        attrs: converted.attrs,
        underline: converted.underline as u8,
        columns: converted.columns,
    }
}

fn placeholder_color_id(color: AnsiColor) -> Option<u32> {
    match color {
        AnsiColor::Spec(rgb) => {
            Some((u32::from(rgb.r) << 16) | (u32::from(rgb.g) << 8) | u32::from(rgb.b))
        }
        AnsiColor::Indexed(index) => Some(u32::from(index)),
        AnsiColor::Named(named) => match named {
            NamedColor::Black => Some(0),
            NamedColor::Red => Some(1),
            NamedColor::Green => Some(2),
            NamedColor::Yellow => Some(3),
            NamedColor::Blue => Some(4),
            NamedColor::Magenta => Some(5),
            NamedColor::Cyan => Some(6),
            NamedColor::White => Some(7),
            NamedColor::BrightBlack => Some(8),
            NamedColor::BrightRed => Some(9),
            NamedColor::BrightGreen => Some(10),
            NamedColor::BrightYellow => Some(11),
            NamedColor::BrightBlue => Some(12),
            NamedColor::BrightMagenta => Some(13),
            NamedColor::BrightCyan => Some(14),
            NamedColor::BrightWhite => Some(15),
            _ => None,
        },
    }
}

/// The [`join_zwj_clusters`] pass for frame cells. Clusters are already
/// contiguous in the blob (cells are filled in column order), so joining
/// is only widening the leader's length.
fn join_zwj_frame_cells(row: &mut [FrameCell], text: &str) {
    let ends_with_zwj = |cell: &FrameCell| {
        let start = cell.text_offset as usize;
        let end = start + cell.text_len as usize;
        text.get(start..end)
            .is_some_and(|s| s.ends_with('\u{200D}'))
    };
    let mut lead = 0;
    while lead < row.len() {
        if !ends_with_zwj(&row[lead]) {
            lead += 1;
            continue;
        }
        let mut next = lead + 1;
        while ends_with_zwj(&row[lead]) && next < row.len() {
            if row[next].columns == 0 && row[next].text_len == 0 {
                // Wide-char spacer: part of the cluster's span.
                next += 1;
                continue;
            }
            if row[next].text_len == 0 {
                break;
            }
            let joined = std::mem::take(&mut row[next].text_len);
            row[lead].text_len = row[lead].text_len.saturating_add(joined);
            let columns = std::mem::replace(&mut row[next].columns, 0);
            row[lead].columns = row[lead].columns.saturating_add(columns);
            next += 1;
        }
        lead = next.max(lead + 1);
    }
}

fn convert_cell(
    cell: &alacritty_terminal::term::cell::Cell,
    resolve: &ResolveColor<'_>,
    default_fg: [u8; 3],
    default_bg: [u8; 3],
) -> Cell {
    let flags = cell.flags;
    let is_spacer = flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER);

    let bg_is_default = cell.bg == AnsiColor::Named(NamedColor::Background);
    let fg = resolve(cell.fg, default_fg);
    let bg = resolve(cell.bg, default_bg);

    let mut attrs: u8 = 0;
    if flags.contains(Flags::BOLD) {
        attrs |= ATTR_BOLD;
    }
    if flags.contains(Flags::ITALIC) {
        attrs |= ATTR_ITALIC;
    }
    if flags.contains(Flags::DIM) {
        attrs |= ATTR_DIM;
    }
    if flags.intersects(Flags::ALL_UNDERLINES) {
        attrs |= ATTR_UNDERLINE;
    }
    if flags.contains(Flags::STRIKEOUT) {
        attrs |= ATTR_STRIKE;
    }
    if flags.contains(Flags::INVERSE) {
        attrs |= ATTR_INVERSE;
    }
    if flags.contains(Flags::HIDDEN) {
        attrs |= ATTR_HIDDEN;
    }

    // Untouched grid cells surface as plain spaces on the default
    // colors; report them as empty text so hosts can keep skipping
    // them. Styled spaces (explicit bg, inverse, underline…) still
    // carry their text.
    let blank = cell.c == ' '
        && cell.zerowidth().is_none_or(<[char]>::is_empty)
        && attrs == 0
        && bg_is_default;
    let text = if is_spacer || blank || flags.contains(Flags::HIDDEN) {
        String::new()
    } else {
        let mut text = String::new();
        text.push(cell.c);
        if let Some(zerowidth) = cell.zerowidth() {
            text.extend(zerowidth);
        }
        text
    };

    Cell {
        text,
        fg: pack_rgb(fg, 0xFF),
        bg: pack_rgb(bg, if bg_is_default { 0 } else { 0xFF }),
        attrs,
        underline: UnderlineStyle::from_flags(flags),
        underline_color: cell
            .underline_color()
            .map(|color| pack_rgb(resolve(color, fg), 0xFF)),
        columns: if is_spacer {
            0
        } else if flags.contains(Flags::WIDE_CHAR) {
            2
        } else {
            1
        },
        wide: flags.contains(Flags::WIDE_CHAR),
        hyperlink: cell.hyperlink().map(|link| link.uri().to_owned()),
    }
}

/// Merge ZWJ-joined emoji into one cell.
///
/// The emulator stores each emoji of a ZWJ sequence in its own cell,
/// with the joiner trailing the cell before it. Renderers need the whole
/// cluster in one text run to shape a single glyph, so the leading cell
/// takes the joined text and the columns it swallowed, and the followers
/// become continuation columns.
fn join_zwj_clusters(row: &mut [Cell]) {
    const ZWJ: char = '\u{200D}';
    let mut lead = 0;
    while lead < row.len() {
        if !row[lead].text.ends_with(ZWJ) {
            lead += 1;
            continue;
        }
        let mut next = lead + 1;
        while row[lead].text.ends_with(ZWJ) && next < row.len() {
            if row[next].columns == 0 {
                // Wide-char spacer: still part of the cluster's span.
                next += 1;
                continue;
            }
            let joined = std::mem::take(&mut row[next].text);
            if joined.is_empty() {
                // A dangling joiner before empty space: nothing to join.
                break;
            }
            row[lead].text.push_str(&joined);
            let columns = std::mem::replace(&mut row[next].columns, 0);
            row[lead].columns = row[lead].columns.saturating_add(columns);
            next += 1;
        }
        lead = next.max(lead + 1);
    }
}

/// Text of one grid row, with each character's cell column and width so
/// offsets map back to highlightable ranges. Trailing blanks are cut.
fn row_search_text(
    row: &alacritty_terminal::grid::Row<alacritty_terminal::term::cell::Cell>,
    line: i64,
) -> SearchRow {
    let columns = row.len();
    let mut text = String::new();
    let mut cells: Vec<(u16, u8)> = Vec::new();
    let mut occupied = 0_usize;
    for col in 0..columns {
        let cell = &row[Column(col)];
        if cell
            .flags
            .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
        {
            continue;
        }
        let width = if cell.flags.contains(Flags::WIDE_CHAR) {
            2
        } else {
            1
        };
        text.push(cell.c);
        cells.push((col as u16, width));
        if let Some(zerowidth) = cell.zerowidth() {
            for ch in zerowidth {
                text.push(*ch);
                cells.push((col as u16, width));
            }
        }
        let non_blank = cell.c != ' ' || cell.zerowidth().is_some_and(|extra| !extra.is_empty());
        if non_blank {
            occupied = text.chars().count();
        }
    }
    let text: String = text.chars().take(occupied).collect();
    cells.truncate(occupied);
    SearchRow {
        line,
        text,
        cells,
        wraps: columns > 0 && row[Column(columns - 1)].flags.contains(Flags::WRAPLINE),
    }
}

fn pack_rgb(rgb: [u8; 3], alpha: u8) -> u32 {
    ((rgb[0] as u32) << 24) | ((rgb[1] as u32) << 16) | ((rgb[2] as u32) << 8) | (alpha as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot_text(snapshot: &ScreenSnapshot) -> String {
        snapshot
            .cells
            .iter()
            .map(|cell| cell.text.as_str())
            .collect::<String>()
    }

    #[test]
    fn renders_fed_bytes() {
        let screen = VtScreen::new_with_options(20, 3, None, None, None);
        screen.feed(b"hello \x1b[1mworld\x1b[0m");
        let snapshot = screen.snapshot();
        let text = snapshot_text(&snapshot);
        assert!(text.contains("hello"), "snapshot text: {text:?}");
        assert!(text.contains("world"), "snapshot text: {text:?}");
        let bold = snapshot
            .cells
            .iter()
            .find(|cell| cell.text == "w")
            .expect("bold cell present");
        assert_ne!(bold.attrs & ATTR_BOLD, 0);
    }

    #[test]
    fn scroll_viewport_reveals_scrollback() {
        let screen = VtScreen::new_with_options(12, 3, None, None, None);
        screen.feed(b"one\r\ntwo\r\nthree\r\nfour\r\nfive");

        let bottom = snapshot_text(&screen.snapshot());
        assert!(bottom.contains("five"), "bottom snapshot: {bottom:?}");
        assert!(!bottom.contains("one"), "bottom snapshot: {bottom:?}");

        assert!(screen.scroll_viewport_delta(-2));
        let snapshot = screen.snapshot();
        let scrolled = snapshot_text(&snapshot);
        assert!(scrolled.contains("one"), "scrolled snapshot: {scrolled:?}");
        assert!(
            !scrolled.contains("five"),
            "scrolled snapshot: {scrolled:?}"
        );
        assert!(
            !snapshot.cursor.visible,
            "history must not show the live cursor"
        );
    }

    #[test]
    fn tracks_modes_and_title() {
        let screen = VtScreen::new_with_options(10, 2, None, None, None);
        assert!(!screen.is_bracketed_paste());
        screen.feed(b"\x1b[?2004h\x1b[?1h\x1b[?1049h\x1b[?1000h\x1b[?1006h");
        assert!(screen.is_bracketed_paste());
        assert!(screen.is_decckm());
        assert!(screen.is_alternate_screen());
        assert!(screen.mouse_tracking_active());
        assert!(screen.is_sgr_mouse());

        screen.feed(b"\x1b]0;my title\x07");
        assert_eq!(screen.snapshot().title.as_deref(), Some("my title"));
    }

    #[test]
    fn responds_to_device_status_report_via_write_pty() {
        let written: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&written);
        let write_pty: PtyWriteCallback = Arc::new(move |bytes: &[u8]| {
            sink.lock().extend_from_slice(bytes);
        });
        let screen = VtScreen::new_with_options(8, 2, None, Some(write_pty), None);
        screen.feed(b"\x1b[6n");
        let response = written.lock().clone();
        assert_eq!(response, b"\x1b[1;1R");
    }

    #[test]
    fn reports_cell_pixel_size_for_inline_image_clients() {
        let written: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&written);
        let write_pty: PtyWriteCallback = Arc::new(move |bytes: &[u8]| {
            sink.lock().extend_from_slice(bytes);
        });
        let screen = VtScreen::new_with_options(80, 24, None, Some(write_pty), None);
        screen.resize(80, 24, 14, 20).unwrap();
        screen.feed(b"\x1b[");
        screen.feed(b"16t");
        assert_eq!(written.lock().as_slice(), b"\x1b[6;20;14t");
    }

    #[test]
    fn cell_size_query_inside_control_string_is_ignored() {
        let written: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&written);
        let write_pty: PtyWriteCallback = Arc::new(move |bytes: &[u8]| {
            sink.lock().extend_from_slice(bytes);
        });
        let screen = VtScreen::new_with_options(80, 24, None, Some(write_pty), None);

        screen.feed(b"\x1bPignored\x1b[16t\x1b\\");

        assert!(written.lock().is_empty());
    }

    #[test]
    fn kitty_graphics_reaches_image_snapshot_and_replies() {
        let written: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&written);
        let write_pty: PtyWriteCallback = Arc::new(move |bytes: &[u8]| {
            sink.lock().extend_from_slice(bytes);
        });
        let screen = VtScreen::new_with_options(8, 2, None, Some(write_pty), None);
        screen.resize(8, 2, 8, 16).unwrap();
        screen.feed(b"x\x1b_Ga=T,f=32,s=1,v=1,i=9,c=1,r=1,C=1;/wAA/w==\x1b\\");

        let snapshot = screen.image_snapshot(0);
        assert!(snapshot.changed);
        assert_eq!(snapshot.images.len(), 1);
        assert_eq!(snapshot.placements.len(), 1);
        assert_eq!(snapshot.placements[0].col, 1);
        assert!(!snapshot.placements[0].alternate_screen);
        assert_eq!(written.lock().as_slice(), b"\x1b_Gi=9;OK\x1b\\");
    }

    #[test]
    fn kitty_snapshot_remains_changed_until_renderer_accepts_generation() {
        let screen = VtScreen::new_with_options(8, 2, None, None, None);
        let initial_generation = screen.image_generation();

        screen.feed(b"\x1b_Ga=T,f=32,s=1,v=1,i=9,c=1,r=1,C=1;/wAA/w==\x1b\\");

        let first = screen.image_snapshot(initial_generation);
        assert!(first.changed);
        assert_eq!(first.placements.len(), 1);
        let repeated = screen.image_snapshot(initial_generation);
        assert!(repeated.changed);
        assert_eq!(repeated.placements.len(), 1);
        assert!(!screen.image_snapshot(first.generation).changed);
    }

    #[test]
    fn clear_screen_removes_physical_kitty_placements() {
        let screen = VtScreen::new_with_options(8, 2, None, None, None);
        screen.resize(8, 2, 8, 16).unwrap();
        screen.feed(b"\x1b_Ga=T,f=32,s=1,v=1,i=9,c=1,r=1,C=1;/wAA/w==\x1b\\");
        let before = screen.image_snapshot(0);
        assert_eq!(before.images.len(), 1);
        assert_eq!(before.placements.len(), 1);

        screen.feed(b"\x1b[2J");

        let cleared = screen.image_snapshot(before.generation);
        assert!(cleared.changed);
        assert_eq!(cleared.images.len(), 1);
        assert!(cleared.placements.is_empty());
    }

    #[test]
    fn home_then_erase_to_end_removes_physical_kitty_placements() {
        let screen = VtScreen::new_with_options(8, 2, None, None, None);
        screen.resize(8, 2, 8, 16).unwrap();
        screen.feed(b"text\x1b_Ga=T,f=32,s=1,v=1,i=9,c=1,r=1,C=1;/wAA/w==\x1b\\");
        let before = screen.image_snapshot(0);

        screen.feed(b"\x1b[H\x1b[J");

        let cleared = screen.image_snapshot(before.generation);
        assert!(cleared.changed);
        assert!(cleared.placements.is_empty());
    }

    #[test]
    fn partial_erase_to_end_keeps_physical_kitty_placements() {
        let screen = VtScreen::new_with_options(8, 2, None, None, None);
        screen.resize(8, 2, 8, 16).unwrap();
        screen.feed(b"text\x1b_Ga=T,f=32,s=1,v=1,i=9,c=1,r=1,C=1;/wAA/w==\x1b\\");
        let before = screen.image_snapshot(0);

        screen.feed(b"\x1b[J");

        let unchanged = screen.image_snapshot(before.generation);
        assert!(!unchanged.changed);
        assert_eq!(screen.image_snapshot(0).placements.len(), 1);
    }

    #[test]
    fn conpty_console_clear_removes_physical_kitty_placements() {
        let screen = VtScreen::new_with_options(8, 2, None, None, None);
        screen.resize(8, 2, 8, 16).unwrap();
        screen.feed(b"text\x1b_Ga=T,f=32,s=1,v=1,i=9,c=1,r=1,C=1;/wAA/w==\x1b\\");
        let before = screen.image_snapshot(0);

        screen.feed(b"\x1b[H\x1b[K\r\n\x1b[K\x1b[H\x1b[3J");

        let cleared = screen.image_snapshot(before.generation);
        assert!(cleared.changed);
        assert!(cleared.placements.is_empty());
    }

    #[test]
    fn clear_screen_preserves_following_kitty_placement() {
        let screen = VtScreen::new_with_options(8, 2, None, None, None);
        screen.resize(8, 2, 8, 16).unwrap();
        screen.feed(
            b"\x1b_Ga=T,f=32,s=1,v=1,i=9,c=1,r=1,C=1;/wAA/w==\x1b\\\
              \x1b[2J\
              \x1b_Ga=T,f=32,s=1,v=1,i=10,c=1,r=1,C=1;/wAA/w==\x1b\\",
        );

        let snapshot = screen.image_snapshot(0);
        assert_eq!(snapshot.placements.len(), 1);
        assert_eq!(snapshot.placements[0].image_id, 10);
    }

    #[test]
    fn unicode_placeholder_preserves_ansi_image_id() {
        let screen = VtScreen::new_with_options(4, 1, None, None, None);
        screen.feed("\x1b[91m\u{10EEEE}".as_bytes());

        let frame = changed(screen.frame(0));
        assert_eq!(frame.cells[0].fg >> 8, 9);
    }

    #[test]
    fn kitty_placement_scrolls_with_alternate_screen_content() {
        let screen = VtScreen::new_with_options(8, 4, None, None, None);
        screen.resize(8, 4, 8, 16).unwrap();
        screen.feed(b"\x1b[?1049h\x1b[H\x1b_Ga=T,f=32,s=1,v=1,i=9,c=2,r=2,C=1;/wAA/w==\x1b\\");
        let first = screen.image_snapshot(0);
        assert_eq!(first.placements[0].line, 0);
        assert!(first.placements[0].alternate_screen);

        screen.feed(b"\r\n\r\n\r\n\r\n");
        let scrolled = screen.image_snapshot(first.generation);
        assert!(scrolled.changed);
        assert_eq!(scrolled.placements[0].line, -1);
    }

    #[test]
    fn kitty_placement_scroll_uses_cursor_position_before_linefeed() {
        let screen = VtScreen::new_with_options(8, 4, None, None, None);
        screen.resize(8, 4, 8, 16).unwrap();
        screen.feed(b"\x1b[?1049h\x1b[H\x1b_Ga=T,f=32,s=1,v=1,i=9,c=2,r=2,C=1;/wAA/w==\x1b\\");
        let first = screen.image_snapshot(0);

        // Full-screen TUIs commonly move to the last row and emit the linefeed
        // in one PTY write while redrawing after a resize.
        screen.feed(b"\x1b[3B\r\n");

        let scrolled = screen.image_snapshot(first.generation);
        assert!(scrolled.changed);
        assert_eq!(scrolled.placements[0].line, -1);
    }

    #[test]
    fn control_string_linefeed_does_not_scroll_kitty_placement() {
        let screen = VtScreen::new_with_options(8, 4, None, None, None);
        screen.resize(8, 4, 8, 16).unwrap();
        screen
            .feed(b"\x1b[?1049h\x1b[H\x1b_Ga=T,f=32,s=1,v=1,i=9,c=2,r=2,C=1;/wAA/w==\x1b\\\x1b[3B");
        let first = screen.image_snapshot(0);

        screen.feed(b"\x1bPignored\ncontent\x1b\\");

        let unchanged = screen.image_snapshot(first.generation);
        assert!(!unchanged.changed);
        assert_eq!(screen.image_snapshot(0).placements[0].line, 0);
    }

    #[test]
    fn kitty_no_move_placement_respects_tui_reserved_rows() {
        let screen = VtScreen::new_with_options(20, 12, None, None, None);
        screen.resize(20, 12, 8, 16).unwrap();
        screen.feed(
            b"\x1b[?1049h\x1b[H\n\n\n\n\x1b[4A\x1b_Ga=T,f=32,s=1,v=1,i=9,c=6,r=5,C=1;/wAA/w==\x1b\\\x1b[4B\r\nAFTER_IMAGE",
        );
        let images = screen.image_snapshot(0);
        assert_eq!(images.placements[0].line, 0);
        let snapshot = screen.snapshot();
        let after_row = snapshot
            .cells
            .chunks(snapshot.cols as usize)
            .position(|row| row.iter().any(|cell| cell.text == "A"))
            .expect("AFTER_IMAGE row");
        assert_eq!(after_row, 5);
        assert!(after_row as i64 >= i64::from(images.placements[0].rows));
    }

    #[test]
    fn chunked_kitty_placement_preserves_following_cursor_movement() {
        let screen = VtScreen::new_with_options(20, 12, None, None, None);
        screen.resize(20, 12, 8, 16).unwrap();
        for chunk in [
            b"\x1b[?1049h\x1b[H\n\n\n\n\x1b[4A\x1b_Ga=T,f=32,s=1,v=1,i=9,c=6,r=5,C=1,m=1;"
                .as_slice(),
            b"/wAA".as_slice(),
            b"\x1b\\\x1b_Gm=0;/w==\x1b".as_slice(),
            b"\\\x1b[4".as_slice(),
            b"B\r\nAFTER_IMAGE".as_slice(),
        ] {
            screen.feed(chunk);
        }
        let images = screen.image_snapshot(0);
        assert_eq!(images.placements[0].line, 0);
        let snapshot = screen.snapshot();
        let after_row = snapshot
            .cells
            .chunks(snapshot.cols as usize)
            .position(|row| row.iter().any(|cell| cell.text == "A"))
            .expect("AFTER_IMAGE row");
        assert_eq!(after_row, 5);
    }

    #[test]
    fn main_screen_full_redraw_reanchors_image_after_resize() {
        let screen = VtScreen::new_with_options(20, 12, None, None, None);
        screen.resize(20, 12, 8, 16).unwrap();
        let redraw = |image_id: u32| {
            format!(
                "\x1b_Ga=d,d=I,i={image_id},q=2\x1b\\\x1b[2J\x1b[H\x1b[3J\
HEADER\r\n\r\n[image]\r\n\r\n\x1b[2A\
\x1b_Ga=T,f=32,s=1,v=1,i={image_id},c=3,r=3,C=1,q=2;/wAA/w==\x1b\\\x1b[2B\
\r\nERROR\r\n\r\nINPUT"
            )
        };
        screen.feed(redraw(91).as_bytes());

        screen.resize(32, 18, 8, 16).unwrap();
        screen.feed(redraw(92).as_bytes());

        let snapshot = screen.snapshot();
        let image_row = snapshot
            .cells
            .chunks(snapshot.cols as usize)
            .position(|row| {
                row.iter()
                    .map(|cell| cell.text.as_str())
                    .collect::<String>()
                    .contains("[image]")
            })
            .expect("image label row");
        let placement = screen.image_snapshot(0).placements.remove(0);
        let viewport_top = screen.scrollbar().unwrap().offset as i64;
        assert_eq!(placement.line - viewport_top, image_row as i64);
    }

    #[test]
    fn resize_does_not_render_a_physical_image_against_a_reflowed_grid() {
        let screen = VtScreen::new_with_options(20, 12, None, None, None);
        screen.resize(20, 12, 8, 16).unwrap();
        screen.feed(
            b"HEADER\r\n\r\n\r\n\x1b[2A\x1b_Ga=T,f=32,s=1,v=1,i=91,c=3,r=3,C=1,q=2;/wAA/w==\x1b\\\x1b[2B",
        );
        let before = screen.image_snapshot(0);
        assert_eq!(before.placements.len(), 1);

        // The application redraw follows SIGWINCH asynchronously. Until its
        // fresh placement arrives, an old pixel rectangle must not be paired
        // with the newly reflowed character grid.
        screen.resize(32, 18, 8, 16).unwrap();
        let resized = screen.image_snapshot(before.generation);
        assert!(resized.changed);
        assert!(resized.placements.is_empty());
    }

    #[test]
    fn main_screen_redraw_reanchors_after_small_viewport_scrollback() {
        let screen = VtScreen::new_with_options(194, 12, None, None, None);
        screen.resize(194, 12, 8, 16).unwrap();
        let redraw = |image_id: u32| {
            let mut output = format!("\x1b_Ga=d,d=I,i={image_id},q=2\x1b\\\x1b[2J\x1b[H\x1b[3J");
            for line in 0..15 {
                output.push_str(&format!("HEADER {line}\r\n"));
            }
            output.push_str("[image]\r\n");
            output.push_str(&"\r\n".repeat(8));
            output.push_str("\x1b[8A");
            output.push_str(&format!(
                "\x1b_Ga=T,f=32,s=1,v=1,i={image_id},c=12,r=9,C=1,q=2;/wAA/w==\x1b\\"
            ));
            output.push_str("\x1b[8B\r\nERROR\r\n\r\nINPUT");
            output
        };
        screen.feed(redraw(91).as_bytes());
        assert!(screen.scrollbar().unwrap().total > 12);

        screen.resize(194, 49, 8, 16).unwrap();
        screen.feed(redraw(92).as_bytes());

        let snapshot = screen.snapshot();
        let label_row = snapshot
            .cells
            .chunks(snapshot.cols as usize)
            .position(|row| {
                row.iter()
                    .map(|cell| cell.text.as_str())
                    .collect::<String>()
                    .contains("[image]")
            })
            .expect("image label row");
        let placement = screen.image_snapshot(0).placements.remove(0);
        let viewport_top = screen.scrollbar().unwrap().offset as i64;
        assert_eq!(placement.line - viewport_top, label_row as i64 + 1);
    }

    #[test]
    fn synchronized_redraw_rebases_placement_after_history_is_cleared() {
        let screen = VtScreen::new_with_options(194, 12, None, None, None);
        screen.resize(194, 12, 8, 22).unwrap();
        let redraw = |image_id: u32| {
            format!(
                "\x1b[?2026h\x1b_Ga=d,d=I,i={image_id},q=2\x1b\\\x1b[2J\x1b[H\x1b[3J\
TITLE\r\n\r\n[image]\r\n{}\x1b[8A\
\x1b_Ga=T,f=32,s=1,v=1,i={image_id},c=12,r=9,C=1,q=2;/wAA/w==\x1b\\\
\x1b[8B\r\nERROR\r\n\r\nINPUT\x1b[?2026l",
                "\r\n".repeat(8)
            )
        };
        screen.feed(redraw(91).as_bytes());
        assert!(screen.scrollbar().unwrap().offset > 0);

        screen.resize(194, 49, 8, 22).unwrap();
        let second = redraw(92);
        for chunk in second.as_bytes().chunks(4096) {
            screen.feed(chunk);
        }

        let snapshot = screen.snapshot();
        let label_row = snapshot
            .cells
            .chunks(snapshot.cols as usize)
            .position(|row| {
                row.iter()
                    .map(|cell| cell.text.as_str())
                    .collect::<String>()
                    .contains("[image]")
            })
            .expect("image label row");
        let placement = screen.image_snapshot(0).placements.remove(0);
        let viewport_top = screen.scrollbar().unwrap().offset as i64;
        assert_eq!(placement.line - viewport_top, label_row as i64 + 1);
    }

    #[test]
    fn synchronized_kitty_update_is_not_published_before_esu() {
        let screen = VtScreen::new_with_options(20, 6, None, None, None);
        screen.resize(20, 6, 8, 16).unwrap();
        screen.feed(b"before");
        let first = changed(screen.frame(0));
        let first_image_generation = screen.image_generation();

        screen.feed(
            b"\x1b[?2026h\x1b[2J\x1b[Hafter\r\n\x1b_Ga=T,f=32,s=1,v=1,i=9,c=2,r=2,C=1;/wAA/w==\x1b\\",
        );
        assert!(matches!(
            screen.frame(first.generation),
            FrameUpdate::Unchanged { generation } if generation == first.generation
        ));
        assert_eq!(screen.image_generation(), first_image_generation);
        assert!(!screen.image_snapshot(first_image_generation).changed);

        screen.feed(b"\x1b[?2026l");
        let committed = changed(screen.frame(first.generation));
        assert_eq!(frame_row(&committed, 0).trim_end(), "after");
        assert!(screen.image_generation() > first_image_generation);
        assert_eq!(
            screen
                .image_snapshot(first_image_generation)
                .placements
                .len(),
            1
        );
    }

    #[test]
    fn default_background_cells_carry_alpha_zero_sentinel() {
        let screen = VtScreen::new_with_options(8, 2, None, None, None);
        screen.feed(b"a\x1b[41mb\x1b[0m");
        let snapshot = screen.snapshot();
        let plain = snapshot
            .cells
            .iter()
            .find(|cell| cell.text == "a")
            .expect("plain cell");
        assert_eq!(plain.bg & 0xff, 0, "default bg keeps alpha 0");
        let red = snapshot
            .cells
            .iter()
            .find(|cell| cell.text == "b")
            .expect("styled cell");
        assert_eq!(red.bg & 0xff, 0xff, "explicit bg is opaque");
    }

    #[test]
    fn semantic_events_flow_from_osc_sequences() {
        let screen = VtScreen::new_with_options(20, 3, None, None, None);
        screen.feed(b"pwd \x1b]7;file:///tmp/work\x07");
        screen.feed(b"\x1b]9;4;1;40\x07");
        screen.feed(b"\x1b]9;build finished\x07");
        screen.feed(b"\x1b]133;A\x1b\\\x1b]133;D;3\x07");
        screen.feed(b"\x1b]0;new title\x07");
        screen.feed(b"\x07"); // bell
        screen.feed(b"\x1b]52;c;aGVsbG8=\x07");

        let batch = screen.drain_events();
        let kinds: Vec<&TerminalEventKind> = batch.events.iter().map(|event| &event.kind).collect();
        assert!(
            kinds.contains(&&TerminalEventKind::Cwd {
                path: "/tmp/work".to_string()
            }),
            "cwd event: {kinds:?}"
        );
        assert!(
            kinds.contains(&&TerminalEventKind::Progress {
                state: TerminalProgressState::Running,
                percent: Some(40)
            }),
            "progress event: {kinds:?}"
        );
        assert!(
            kinds.contains(&&TerminalEventKind::Notification {
                title: None,
                body: "build finished".to_string()
            }),
            "notification event: {kinds:?}"
        );
        assert!(
            kinds.contains(&&TerminalEventKind::CommandFinished {
                line: 0,
                exit_code: Some(3)
            }),
            "command finished with grid line: {kinds:?}"
        );
        assert!(
            kinds.contains(&&TerminalEventKind::Title {
                title: Some("new title".to_string())
            }),
            "title event: {kinds:?}"
        );
        assert!(kinds.contains(&&TerminalEventKind::Bell), "bell: {kinds:?}");
        assert!(
            kinds.contains(&&TerminalEventKind::ClipboardStore {
                clipboard: "clipboard".to_string(),
                text: "hello".to_string()
            }),
            "clipboard store decoded: {kinds:?}"
        );

        // Sequences are monotonic and the queue is empty after drain.
        let seqs: Vec<u64> = batch.events.iter().map(|event| event.seq).collect();
        assert!(seqs.windows(2).all(|pair| pair[1] > pair[0]));
        assert_eq!(batch.next_seq as usize, batch.events.len());
        assert!(screen.drain_events().events.is_empty());
    }

    #[test]
    fn semantic_marks_track_absolute_grid_lines() {
        let screen = VtScreen::new_with_options(10, 2, None, None, None);
        // Push two lines into scrollback, then mark the prompt.
        screen.feed(b"one\r\ntwo\r\nthree\r\n");
        screen.feed(b"\x1b]133;A\x07");
        let batch = screen.drain_events();
        let mark = batch
            .events
            .iter()
            .find_map(|event| match event.kind {
                TerminalEventKind::PromptStart { line } => Some(line),
                _ => None,
            })
            .expect("prompt mark");
        // Two lines scrolled into history; the cursor sits on screen
        // row 1, so the prompt line is absolute line 3.
        assert_eq!(mark, 3);
    }

    #[test]
    fn command_blocks_follow_semantic_marks() {
        let screen = VtScreen::new_with_options(10, 3, None, None, None);
        screen.feed(b"\x1b]133;A\x07prompt$ \x1b]133;B\x07ls\r\n");
        screen.feed(b"\x1b]133;C\x07file1 file2\r\n");
        screen.feed(b"\x1b]133;D;2\x07");
        screen.feed(b"\x1b]133;A\x07prompt$ ");

        let blocks = screen.command_blocks();
        assert_eq!(blocks.len(), 2, "one completed + one open: {blocks:?}");
        let first = &blocks[0];
        assert_eq!(first.prompt_line, 0);
        assert_eq!(first.input_line, Some(0));
        assert!(
            first.output_line >= first.input_line,
            "output follows input: {first:?}"
        );
        assert!(
            first.end_line >= first.output_line,
            "block ends after output: {first:?}"
        );
        assert_eq!(first.exit_code, Some(2));
        let second = &blocks[1];
        assert!(second.end_line.is_none(), "second block still open");
        assert!(
            second.prompt_line >= first.end_line.unwrap_or(0),
            "next prompt follows the completed block: {blocks:?}"
        );
    }

    #[test]
    fn osc8_hyperlinks_reach_cells_and_ranges() {
        let screen = VtScreen::new_with_options(20, 2, None, None, None);
        screen.feed(b"\x1b]8;;https://example.com\x07link text\x1b]8;;\x07 plain");
        let snapshot = screen.snapshot();
        let linked: Vec<&Cell> = snapshot
            .cells
            .iter()
            .filter(|cell| cell.hyperlink.is_some())
            .collect();
        assert_eq!(linked.len(), 9, "'link text' cells carry the URI");
        assert_eq!(linked[0].hyperlink.as_deref(), Some("https://example.com"));

        let ranges = screen.screen_hyperlinks();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].0, 0, "screen row 0 as absolute line");
        assert_eq!((ranges[0].1, ranges[0].2), (0, 9));
        assert_eq!(ranges[0].3, "https://example.com");
    }

    #[test]
    fn cell_styles_cover_the_sgr_surface() {
        let screen = VtScreen::new_with_options(20, 2, None, None, None);
        // Curly underline in red, strike, then a concealed run.
        screen.feed(b"\x1b[4:3m\x1b[58;2;255;0;0mU\x1b[m\x1b[9mS\x1b[m\x1b[8mhide\x1b[m");
        let snapshot = screen.snapshot();

        let underlined = &snapshot.cells[0];
        assert_eq!(underlined.text, "U");
        assert_eq!(underlined.underline, UnderlineStyle::Curly);
        assert_ne!(underlined.attrs & ATTR_UNDERLINE, 0, "style-agnostic bit");
        assert_eq!(underlined.underline_color, Some(0xFF0000FF));

        let struck = &snapshot.cells[1];
        assert_eq!(struck.text, "S");
        assert_ne!(struck.attrs & ATTR_STRIKE, 0);
        assert_eq!(struck.underline, UnderlineStyle::None);

        // Concealed cells keep their columns and styling but no text.
        let hidden: Vec<&Cell> = snapshot.cells[2..6].iter().collect();
        assert_eq!(hidden.len(), 4);
        assert!(
            hidden
                .iter()
                .all(|cell| cell.text.is_empty() && cell.attrs & ATTR_HIDDEN != 0),
            "hidden run: {hidden:?}"
        );
    }

    #[test]
    fn underline_styles_map_to_their_sgr() {
        for (sgr, expected) in [
            ("\x1b[4m", UnderlineStyle::Single),
            // SGR 21 is cancel-bold in this stack, not double underline.
            ("\x1b[4:2m", UnderlineStyle::Double),
            ("\x1b[4:3m", UnderlineStyle::Curly),
            ("\x1b[4:4m", UnderlineStyle::Dotted),
            ("\x1b[4:5m", UnderlineStyle::Dashed),
            ("\x1b[24m", UnderlineStyle::None),
        ] {
            let screen = VtScreen::new_with_options(4, 1, None, None, None);
            screen.feed(format!("{sgr}x").as_bytes());
            assert_eq!(
                screen.snapshot().cells[0].underline,
                expected,
                "underline style for {sgr:?}"
            );
        }
    }

    #[test]
    fn progress_follows_osc94_reports() {
        let screen = VtScreen::new_with_options(20, 3, None, None, None);
        assert_eq!(screen.activity().progress, TerminalProgress::default());

        screen.feed(b"\x1b]9;4;1;40\x07");
        assert_eq!(
            screen.activity().progress,
            TerminalProgress {
                state: TerminalProgressState::Running,
                percent: Some(40),
            }
        );

        screen.feed(b"\x1b]9;4;4;60\x07");
        assert_eq!(
            screen.activity().progress,
            TerminalProgress {
                state: TerminalProgressState::Paused,
                percent: Some(60),
            }
        );

        screen.feed(b"\x1b]9;4;1;100\x07");
        assert_eq!(
            screen.activity().progress.state,
            TerminalProgressState::Succeeded,
            "a full progress bar is the protocol's completion signal"
        );

        screen.feed(b"\x1b]9;4;2;\x07");
        assert_eq!(
            screen.activity().progress.state,
            TerminalProgressState::Failed
        );

        screen.feed(b"\x1b]9;4;0;\x07");
        assert_eq!(screen.activity().progress, TerminalProgress::default());
    }

    #[test]
    fn progress_falls_back_to_command_boundaries() {
        let screen = VtScreen::new_with_options(20, 3, None, None, None);
        screen.feed(b"\x1b]133;A\x07$ \x1b]133;B\x07build\r\n");
        assert_eq!(
            screen.activity().progress.state,
            TerminalProgressState::Idle,
            "typing at the prompt is not a running command"
        );

        screen.feed(b"\x1b]133;C\x07working\r\n");
        assert_eq!(
            screen.activity().progress,
            TerminalProgress {
                state: TerminalProgressState::Running,
                percent: None,
            },
            "a command with no progress reports is indeterminate"
        );

        // A command that reports OSC 9;4 keeps ownership of the state...
        screen.feed(b"\x1b]9;4;1;30\x07");
        assert_eq!(screen.activity().progress.percent, Some(30));
        // ...until it finishes, where the exit code wins.
        screen.feed(b"\x1b]133;D;1\x07");
        assert_eq!(
            screen.activity(),
            TerminalActivity {
                progress: TerminalProgress {
                    state: TerminalProgressState::Failed,
                    percent: None,
                },
                last_exit_code: Some(1),
                bells: 0,
                notifications: 0,
            }
        );

        // The next command starts clean rather than inheriting state.
        screen.feed(b"\x1b]133;A\x07$ \x1b]133;C\x07\x1b]133;D;0\x07");
        assert_eq!(
            screen.activity().progress.state,
            TerminalProgressState::Succeeded
        );

        // Without an exit code the command finished with an unknown
        // outcome; that is idle, not success.
        screen.feed(b"\x1b]133;C\x07\x1b]133;D\x07");
        assert_eq!(
            screen.activity().progress.state,
            TerminalProgressState::Idle
        );
    }

    #[test]
    fn attention_counters_accumulate() {
        let screen = VtScreen::new_with_options(20, 3, None, None, None);
        // The BELs terminating the OSC sequences are not rung.
        screen.feed(b"\x07\x1b]9;build done\x07\x07");
        screen.feed(b"\x1b]777;notify;title;body\x07");
        let activity = screen.activity();
        assert_eq!(activity.bells, 2);
        assert_eq!(activity.notifications, 2);
        // Counters are monotonic: draining events does not reset them.
        screen.drain_events();
        assert_eq!(screen.activity().bells, 2);
    }

    /// Row text rebuilt from a frame's cells and blob.
    fn frame_row(frame: &TerminalFrame, row: u16) -> String {
        let start = row as usize * frame.cols as usize;
        frame.cells[start..start + frame.cols as usize]
            .iter()
            .map(|cell| frame.cell_text(cell))
            .collect::<String>()
    }

    fn changed(update: FrameUpdate) -> Box<TerminalFrame> {
        match update {
            FrameUpdate::Changed(frame) => frame,
            FrameUpdate::Unchanged { generation } => {
                panic!("expected a frame, got unchanged at generation {generation}")
            }
        }
    }

    #[test]
    fn frame_reports_only_damaged_rows() {
        let screen = VtScreen::new_with_options(10, 4, None, None, None);
        screen.feed(b"one\r\ntwo\r\nthree\r\n");

        // First frame: nothing drawn yet, so everything is damaged.
        let first = changed(screen.frame(0));
        assert!(first.full_damage);
        assert_eq!(first.damage.len(), 4, "every row: {:?}", first.damage);
        assert_eq!(frame_row(&first, 0).trim_end(), "one");
        assert_eq!(frame_row(&first, 2).trim_end(), "three");

        // A quiet poll does no work.
        match screen.frame(first.generation) {
            FrameUpdate::Unchanged { generation } => assert_eq!(generation, first.generation),
            FrameUpdate::Changed(_) => panic!("nothing changed since the last frame"),
        }

        // Writing one row damages that row (plus the cursor's).
        screen.feed(b"four");
        let second = changed(screen.frame(first.generation));
        assert!(!second.full_damage, "a single row changed");
        let rows: Vec<u16> = second.damage.iter().map(|damage| damage.row).collect();
        assert_eq!(rows, vec![3], "only the written row: {:?}", second.damage);
        assert_eq!(second.damage[0].start_col, 0);
        assert!(second.damage[0].end_col <= second.cols);
        assert_eq!(frame_row(&second, 3).trim_end(), "four");

        // A renderer asking from a generation it never saw gets a full
        // repaint rather than a diff against a frame it does not have.
        let resumed = changed(screen.frame(second.generation.wrapping_sub(5)));
        assert!(resumed.full_damage);
    }

    #[test]
    fn first_frame_of_an_idle_session_is_full() {
        let screen = VtScreen::new_with_options(8, 2, None, None, None);
        let first = changed(screen.frame(0));
        assert!(first.full_damage, "a renderer with nothing drawn gets all");
        assert_eq!(first.cells.len(), 16);
        assert!(matches!(
            screen.frame(first.generation),
            FrameUpdate::Unchanged { .. }
        ));
    }

    #[test]
    fn frame_cells_are_allocation_free_and_carry_style() {
        let screen = VtScreen::new_with_options(12, 2, None, None, None);
        screen.feed("\x1b[31mR\x1b[m中\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}".as_bytes());
        let frame = changed(screen.frame(0));

        assert_eq!(
            frame_row(&frame, 0),
            "R中\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"
        );
        assert_eq!(frame.cells[0].columns, 1);
        assert_ne!(frame.cells[0].fg, frame.default_fg, "ANSI red resolved");
        assert_eq!(frame.cells[1].columns, 2, "wide char spans two columns");
        assert_eq!(frame.cells[2].columns, 0, "wide-char spacer");
        assert_eq!(frame.cells[3].columns, 6, "joined family emoji");
        // The blob holds every cluster exactly once, in column order.
        assert_eq!(frame.cells[1].text_offset as usize, "R".len());
        assert!(frame.text.starts_with("R中"));

        // A row's spans still cover the row.
        let total: u32 = frame.cells[..frame.cols as usize]
            .iter()
            .map(|cell| u32::from(cell.columns))
            .sum();
        assert_eq!(total, u32::from(frame.cols));
    }

    #[test]
    fn frame_and_snapshot_agree_on_content() {
        let screen = VtScreen::new_with_options(12, 2, None, None, None);
        screen.feed("ab\u{1F468}\u{200D}\u{1F469}中\x1b[4:3mU".as_bytes());
        let frame = changed(screen.frame(0));
        let snapshot = screen.snapshot();

        let snapshot_row: String = snapshot.cells[..snapshot.cols as usize]
            .iter()
            .map(|cell| cell.text.as_str())
            .collect();
        assert_eq!(frame_row(&frame, 0), snapshot_row, "same clusters");
        for (index, (cell, expected)) in frame
            .cells
            .iter()
            .zip(snapshot.cells.iter())
            .enumerate()
            .take(snapshot.cols as usize)
        {
            assert_eq!(cell.columns, expected.columns, "columns at {index}");
            assert_eq!(cell.fg, expected.fg, "fg at {index}");
            assert_eq!(cell.attrs, expected.attrs, "attrs at {index}");
            assert_eq!(
                cell.underline, expected.underline as u8,
                "underline at {index}"
            );
        }
    }

    #[test]
    fn theme_and_resize_damage_everything() {
        let screen = VtScreen::new_with_options(10, 3, None, None, None);
        screen.feed(b"hello");
        let first = changed(screen.frame(0));

        screen.set_theme(ThemeColors::from_ansi16(
            [0x11, 0x22, 0x33],
            [0x44, 0x55, 0x66],
            [[0u8; 3]; 16],
        ));
        let themed = changed(screen.frame(first.generation));
        assert!(themed.full_damage, "every resolved color changed");
        assert_eq!(themed.default_bg, 0x445566ff);

        screen.resize(20, 3, 1, 1).expect("resize");
        let resized = changed(screen.frame(themed.generation));
        assert!(resized.full_damage);
        assert_eq!(resized.cols, 20);
    }

    #[test]
    fn theme_swap_repaints_without_reflow() {
        let screen = VtScreen::new_with_options(10, 2, None, None, None);
        screen.feed(b"\x1b[31mRED");
        let before = screen.snapshot();
        assert_eq!(before.cells[0].text, "R");

        let mut ansi = [[0u8; 3]; 16];
        ansi[1] = [0x12, 0x34, 0x56];
        screen.set_theme(ThemeColors::from_ansi16(
            [0xaa, 0xaa, 0xaa],
            [0x01, 0x02, 0x03],
            ansi,
        ));
        let after = screen.snapshot();

        assert_eq!(after.cells[0].text, "R", "grid content untouched");
        assert_ne!(
            after.cells[0].fg, before.cells[0].fg,
            "ANSI red re-resolved"
        );
        assert_eq!(after.cells[0].fg, 0x123456ff);
        assert_eq!(after.default_bg, 0x010203ff);
        assert!(
            after.generation > before.generation,
            "generation bumps so a polling host repaints"
        );
    }

    #[test]
    fn text_view_joins_wrapped_rows_and_locates_the_cursor() {
        let screen = VtScreen::new_with_options(10, 3, None, None, None);
        screen.feed(b"short\r\n");
        // 14 columns of text wrap across two 10-column rows.
        screen.feed(b"wrapped-line-x\r\n");
        screen.feed(b"tail");

        let view = screen.text_view(Some(0), 10);
        assert_eq!(
            view.lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            vec!["short", "wrapped-line-x", "tail"]
        );
        assert_eq!(view.lines[1].rows, 2, "the wrapped line spans two rows");
        assert_eq!(view.lines[2].line, 3, "absolute line after the wrap");
        assert_eq!(view.total_lines, 4, "one scrolled row + three screen rows");
        assert_eq!(
            (view.cursor_line, view.cursor_column),
            (3, 4),
            "cursor sits after 'tail'"
        );
        assert_eq!(
            (view.viewport_first_line, view.viewport_last_line),
            (1, 3),
            "the live screen is the last three lines"
        );

        // Starting mid-wrap rewinds to the logical line's beginning.
        let view = screen.text_view(Some(2), 1);
        assert_eq!(view.lines.len(), 1);
        assert_eq!(view.lines[0].line, 1);
        assert_eq!(view.lines[0].text, "wrapped-line-x");

        // Default start is the first visible line.
        assert_eq!(screen.text_view(None, 1).lines[0].line, 1);
    }

    #[test]
    fn clusters_stay_in_one_cell() {
        let screen = VtScreen::new_with_options(20, 2, None, None, None);
        // Combining acute, a ZWJ family emoji, and a variation selector.
        screen
            .feed("e\u{301}\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{2764}\u{FE0F}".as_bytes());
        let snapshot = screen.snapshot();
        let texts: Vec<&str> = snapshot
            .cells
            .iter()
            .filter(|cell| !cell.text.is_empty())
            .map(|cell| cell.text.as_str())
            .collect();
        assert_eq!(
            texts,
            vec![
                "e\u{301}",
                "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
                "\u{2764}\u{FE0F}"
            ],
            "each cluster arrives as one cell's text"
        );

        let spans: Vec<u8> = snapshot.cells[..8]
            .iter()
            .map(|cell| cell.columns)
            .collect();
        assert_eq!(
            spans,
            vec![1, 6, 0, 0, 0, 0, 0, 1],
            "the family emoji spans the six columns it swallowed"
        );
        for row in snapshot.cells.chunks(snapshot.cols as usize) {
            let total: u32 = row.iter().map(|cell| u32::from(cell.columns)).sum();
            assert_eq!(total, u32::from(snapshot.cols), "row spans cover the row");
        }
    }

    #[test]
    fn wide_characters_flag_and_spacer() {
        let screen = VtScreen::new_with_options(8, 2, None, None, None);
        screen.feed("中".as_bytes());
        let snapshot = screen.snapshot();
        let wide = snapshot
            .cells
            .iter()
            .find(|cell| cell.text == "中")
            .expect("wide cell");
        assert!(wide.wide);
        // The spacer column after the wide char must not repeat text.
        assert!(snapshot.cells.iter().filter(|c| c.text == "中").count() == 1);
    }
}
