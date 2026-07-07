//! Serde DTOs for the `desktop` surface. These are the single source of truth
//! for both the CLI `--json` output and any future in-process JS binding, so
//! the two consumers can never drift.

use serde::Serialize;

/// A rectangle in global virtual-screen physical pixels.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// A monitor/display.
#[derive(Debug, Clone, Serialize)]
pub struct Display {
    pub id: String,
    pub primary: bool,
    pub bounds: Rect,
    pub work_area: Rect,
    pub scale: f64,
    pub dpi: u32,
}

/// A top-level OS window.
#[derive(Debug, Clone, Serialize)]
pub struct Window {
    pub id: String,
    pub title: String,
    pub process: String,
    pub pid: u32,
    pub bounds: Rect,
    pub display_id: String,
    pub scale: f64,
    pub dpi: u32,
    pub visible: bool,
    pub focused: bool,
    pub minimized: bool,
    pub maximized: bool,
    pub always_on_top: bool,
    /// Front-to-back z index (0 = frontmost).
    pub z: u32,
}

/// A window-selection query (`--match`), one field set.
#[derive(Debug, Clone, Default)]
pub struct WindowQuery {
    /// Substring against title / class / process (the bare `text` form).
    pub text: Option<String>,
    pub title: Option<String>,
    pub class: Option<String>,
    pub process: Option<String>,
    pub pid: Option<u32>,
}

impl WindowQuery {
    pub fn is_empty(&self) -> bool {
        self.text.is_none()
            && self.title.is_none()
            && self.class.is_none()
            && self.process.is_none()
            && self.pid.is_none()
    }

    /// Parse the proposal's window match grammar:
    /// bare `text`, or `title:`, `class:`, `process:`, `pid:` prefixes.
    pub fn parse(input: &str) -> Self {
        let mut q = WindowQuery::default();
        if let Some(rest) = input.strip_prefix("title:") {
            q.title = Some(rest.to_string());
        } else if let Some(rest) = input.strip_prefix("class:") {
            q.class = Some(rest.to_string());
        } else if let Some(rest) = input.strip_prefix("process:") {
            q.process = Some(rest.to_string());
        } else if let Some(rest) = input.strip_prefix("pid:") {
            q.pid = rest.trim().parse().ok();
        } else {
            q.text = Some(input.to_string());
        }
        q
    }
}

/// A generic "it worked" acknowledgement for input/mutation commands that have
/// no richer result to return.
#[derive(Debug, Clone, Serialize)]
pub struct Ack {
    pub ok: bool,
    pub action: String,
}

impl Ack {
    pub fn new(action: impl Into<String>) -> Self {
        Ack {
            ok: true,
            action: action.into(),
        }
    }
}

/// Mouse button for pointer input.
#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Canonical modifier keys (platform-neutral; `Meta` maps to Win/Command).
#[derive(Debug, Clone, Copy)]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    Meta,
}

/// How a window action selects its target: a runtime id, or a match query.
#[derive(Debug, Clone)]
pub enum WindowTarget {
    Id(String),
    Match(WindowQuery),
}

/// Clipboard contents (`desktop clipboard get`).
#[derive(Debug, Clone, Serialize)]
pub struct Clipboard {
    pub available_formats: Vec<String>,
    pub text: Option<String>,
}

/// A single pixel's color (`desktop pixel`).
#[derive(Debug, Clone, Serialize)]
pub struct Pixel {
    pub x: i32,
    pub y: i32,
    pub hex: String,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// What to capture (`desktop screenshot`).
#[derive(Debug, Clone)]
pub enum CaptureTarget {
    /// The whole virtual screen (all monitors).
    Screen,
    /// A monitor by 1-based index (as listed by `desktop displays`).
    Display(usize),
    /// A window by id ("0x...").
    Window(String),
    /// A region in global physical pixels.
    Region { x: i32, y: i32, w: i32, h: i32 },
}

/// The result of a capture. `png` holds the encoded bytes; the CLI decides
/// whether to write a file or emit a base64 envelope, so this is not itself
/// serialized.
#[derive(Debug, Clone)]
pub struct Capture {
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
    /// True when the backend captured window pixels regardless of occlusion
    /// (PrintWindow), false for on-screen BitBlt captures.
    pub occlusion_independent: bool,
    pub backend: String,
}

/// Backend capability + permission report (`desktop doctor`).
#[derive(Debug, Clone, Serialize)]
pub struct Doctor {
    pub backend: String,
    pub os: String,
    pub os_version: String,
    pub capabilities: Capabilities,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Capabilities {
    pub displays: bool,
    pub windows: bool,
    pub screenshot: bool,
    pub window_screenshot_occlusion_independent: bool,
    pub pixel: bool,
    pub pointer: bool,
    pub key: bool,
    pub window_management: bool,
    pub clipboard: bool,
    pub ax_tree: bool,
    pub ocr: bool,
    pub image_match: bool,
}
