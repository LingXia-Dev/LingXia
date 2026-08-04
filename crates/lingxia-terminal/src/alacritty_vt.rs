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
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{ClipboardType, Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{
    Color as AnsiColor, CursorShape, NamedColor, Processor, Rgb as AnsiRgb, StdSyncHandler,
};
use parking_lot::Mutex;
use serde::Serialize;

use crate::osc::{OscProgress, OscSemantic, OscTap, parse_osc};
use crate::search::SearchRow;

// Attr bits packed into `Cell.attrs`. Kept in sync with the HLSL
// pixel shader's interpretation (bit 0 = bold, 1 = italic, 2 =
// underline, 3 = strike, 4 = inverse, 5 = dim/faint).
pub const ATTR_BOLD: u8 = 1 << 0;
pub const ATTR_ITALIC: u8 = 1 << 1;
pub const ATTR_UNDERLINE: u8 = 1 << 2;
pub const ATTR_STRIKE: u8 = 1 << 3;
pub const ATTR_INVERSE: u8 = 1 << 4;
pub const ATTR_DIM: u8 = 1 << 5;

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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    /// Foreground RGBA (0xRRGGBBAA).
    pub fg: u32,
    /// Background RGBA. Alpha 0 marks the default background so the
    /// renderer can apply pane opacity; explicit SGR backgrounds are
    /// opaque.
    pub bg: u32,
    pub attrs: u8,
    pub wide: bool,
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

/// Progress/task state carried by OSC 9;4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalProgressState {
    Idle,
    Running,
    Paused,
    Failed,
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
            Event::Bell => self.events.lock().push(TerminalEventKind::Bell),
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
    listener: Arc<Listener>,
    replies: Receiver<PendingReply>,
    theme: ThemeColors,
    cell_width_px: u16,
    cell_height_px: u16,
    generation: u64,
    blocks: CommandBlockTracker,
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
                listener,
                replies,
                theme: theme.cloned().unwrap_or_else(default_theme),
                cell_width_px: 1,
                cell_height_px: 1,
                generation: 0,
                blocks: CommandBlockTracker::default(),
            }),
        }
    }

    /// Feed bytes from the PTY into the parser.
    ///
    /// The OSC tap runs alongside the parser so semantic sequences the
    /// emulator drops (OSC 7/9/99/133/777) still produce typed events.
    /// Bytes are advanced up to each tapped sequence before the event is
    /// recorded, giving marks an exact grid position; the tapped bytes
    /// are then fed through the parser as usual so sequences the
    /// emulator does handle (title, hyperlink, clipboard) keep working.
    pub fn feed(&self, bytes: &[u8]) {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;
        let tapped = inner.tap.feed(bytes);
        let mut last = 0;
        for osc in tapped {
            if osc.start < last {
                // The sequence started in an earlier feed call and its
                // bytes were already parsed; only record its semantics.
                inner.record_osc(&osc.body);
                continue;
            }
            inner
                .parser
                .advance(&mut inner.term, &bytes[last..osc.start]);
            inner.record_osc(&osc.body);
            inner
                .parser
                .advance(&mut inner.term, &bytes[osc.start..osc.end]);
            last = osc.end;
        }
        inner.parser.advance(&mut inner.term, &bytes[last..]);
        inner.answer_pending_replies();
        inner.generation = inner.generation.wrapping_add(1);
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
            let row = &grid[Line(offset as i32)];
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
                let non_blank =
                    cell.c != ' ' || cell.zerowidth().is_some_and(|extra| !extra.is_empty());
                if non_blank {
                    occupied = text.chars().count();
                }
            }
            let text: String = text.chars().take(occupied).collect();
            cells.truncate(occupied);
            let wraps = columns > 0 && row[Column(columns - 1)].flags.contains(Flags::WRAPLINE);
            rows.push(SearchRow {
                line: history + offset,
                text,
                cells,
                wraps,
            });
        }
        rows
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
        inner.term.resize(GridSize {
            columns: cols as usize,
            screen_lines: rows as usize,
        });
        inner.cell_width_px = cell_width_px.clamp(1, u16::MAX as u32) as u16;
        inner.cell_height_px = cell_height_px.clamp(1, u16::MAX as u32) as u16;
        inner.generation = inner.generation.wrapping_add(1);
        Ok(())
    }

    pub fn snapshot(&self) -> ScreenSnapshot {
        let mut inner = self.inner.lock();
        let inner = &mut *inner;

        // An application that enters synchronized output (DEC 2026) and
        // never leaves keeps bytes buffered inside the parser. Flush
        // once vte's own deadline passes so the frame can't freeze.
        if let Some(deadline) = inner.parser.sync_timeout().sync_timeout()
            && Instant::now() >= deadline
        {
            inner.parser.stop_sync(&mut inner.term);
            inner.answer_pending_replies();
            inner.generation = inner.generation.wrapping_add(1);
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
        inner.generation = inner.generation.wrapping_add(1);
        true
    }
}

impl VtInner {
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
            OscSemantic::Cwd(path) => TerminalEventKind::Cwd { path },
            OscSemantic::Progress(progress) => {
                let (state, percent) = match progress {
                    OscProgress::Idle => (TerminalProgressState::Idle, None),
                    OscProgress::Running { percent } => (TerminalProgressState::Running, percent),
                    OscProgress::Paused { percent } => (TerminalProgressState::Paused, percent),
                    OscProgress::Failed => (TerminalProgressState::Failed, None),
                };
                TerminalEventKind::Progress { state, percent }
            }
            OscSemantic::Notification { title, body } => {
                TerminalEventKind::Notification { title, body }
            }
            OscSemantic::PromptStart => TerminalEventKind::PromptStart {
                line: absolute_line(),
            },
            OscSemantic::InputStart => TerminalEventKind::InputStart {
                line: absolute_line(),
            },
            OscSemantic::OutputStart => TerminalEventKind::OutputStart {
                line: absolute_line(),
            },
            OscSemantic::CommandFinished { exit_code } => TerminalEventKind::CommandFinished {
                line: absolute_line(),
                exit_code,
            },
        };
        self.blocks.record(&kind);
        self.listener.events.lock().push(kind);
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
        wide: flags.contains(Flags::WIDE_CHAR),
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
