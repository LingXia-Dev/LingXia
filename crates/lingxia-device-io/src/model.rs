//! Serde DTOs for the `desktop` surface. These are the single source of truth
//! for the CLI `--json` output, the local-control wire, and any future
//! in-process JS binding, so the consumers can never drift.
//!
//! Every type round-trips. A product runs these commands inside the app and a
//! client reads the answers back into the same structs, which is what lets one
//! set of printers serve both the in-process and over-the-socket mounts.

use serde::{Deserialize, Serialize};

/// A rectangle in backend-native global desktop coordinates.
///
/// Windows uses physical pixels. macOS uses global display points and reports
/// each display/window scale so callers can convert to backing pixels.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// A monitor/display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Display {
    pub id: String,
    pub primary: bool,
    pub bounds: Rect,
    pub work_area: Rect,
    pub scale: f64,
    pub dpi: u32,
}

/// A top-level OS window.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowQuery {
    /// Substring against title / class / process (the bare `text` form).
    pub text: Option<String>,
    pub title: Option<String>,
    pub class: Option<String>,
    pub process: Option<String>,
    pub pid: Option<u32>,
    /// A prefix was given but its value was malformed (e.g. `pid:abc`); such a
    /// query must match nothing, never everything.
    malformed: bool,
}

impl WindowQuery {
    pub fn is_empty(&self) -> bool {
        !self.malformed
            && self.text.is_none()
            && self.title.is_none()
            && self.class.is_none()
            && self.process.is_none()
            && self.pid.is_none()
    }

    /// A malformed query (e.g. `pid:abc`) must match nothing.
    pub fn is_malformed(&self) -> bool {
        self.malformed
    }

    /// A query that matches windows owned by `pid`.
    pub fn by_pid(pid: u32) -> Self {
        WindowQuery {
            pid: Some(pid),
            ..Default::default()
        }
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
            match rest.trim().parse() {
                Ok(pid) => q.pid = Some(pid),
                Err(_) => q.malformed = true,
            }
        } else {
            q.text = Some(input.to_string());
        }
        q
    }
}

/// A generic "it worked" acknowledgement for input/mutation commands that have
/// no richer result to return.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Canonical modifier keys (platform-neutral; `Meta` maps to Win/Command).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
    Meta,
}

/// How a window action selects its target: a runtime id, or a match query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WindowTarget {
    Id(String),
    Match(WindowQuery),
}

/// A node in the native accessibility tree (`desktop ax`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxNode {
    pub id: String,
    pub role: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub enabled: bool,
    pub focused: bool,
    pub rect: Rect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<AxNode>,
}

/// An accessibility node query (`--match`): bare `text`, or a
/// `name:` / `role:` / `value:` / `id:` prefix.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AxQuery {
    pub text: Option<String>,
    pub name: Option<String>,
    pub role: Option<String>,
    pub value: Option<String>,
    pub id: Option<String>,
}

impl AxQuery {
    pub fn parse(input: &str) -> Self {
        let mut q = AxQuery::default();
        if let Some(r) = input.strip_prefix("name:") {
            q.name = Some(r.to_string());
        } else if let Some(r) = input.strip_prefix("role:") {
            q.role = Some(r.to_string());
        } else if let Some(r) = input.strip_prefix("value:") {
            q.value = Some(r.to_string());
        } else if let Some(r) = input.strip_prefix("id:") {
            q.id = Some(r.to_string());
        } else if let Some(r) = input.strip_prefix("text:") {
            q.text = Some(r.to_string());
        } else {
            q.text = Some(input.to_string());
        }
        q
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_none()
            && self.name.is_none()
            && self.role.is_none()
            && self.value.is_none()
            && self.id.is_none()
    }

    /// Does a node satisfy this query? (case-insensitive substring, exact id).
    pub fn matches(&self, node: &AxNode) -> bool {
        let ci = |needle: &str, hay: &str| hay.to_lowercase().contains(&needle.to_lowercase());
        if let Some(id) = &self.id {
            return &node.id == id;
        }
        if let Some(n) = &self.name {
            return ci(n, &node.name);
        }
        if let Some(r) = &self.role {
            return ci(r, &node.role);
        }
        if let Some(v) = &self.value {
            return node.value.as_deref().is_some_and(|nv| ci(v, nv));
        }
        if let Some(t) = &self.text {
            return ci(t, &node.name)
                || ci(t, &node.role)
                || node.value.as_deref().is_some_and(|nv| ci(t, nv));
        }
        true
    }
}

/// A running process (`desktop process list`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
}

/// Result of launching an app (`desktop app launch`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchResult {
    /// Durable target pid: the matched window's owning process when
    /// `--wait-window` found one, otherwise the launched process. Prefer this
    /// for follow-up `app quit`/`process kill`.
    pub pid: u32,
    /// The pid `CreateProcess` returned. Differs from `pid` when the launched
    /// binary is a relauncher/stub (e.g. the Store-hosted notepad), whose
    /// process exits after spawning the real app under a new pid.
    pub launcher_pid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<Window>,
}

/// How `desktop app quit` selects its target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuitTarget {
    Match(WindowQuery),
    Pid(u32),
    Window(String),
}

/// Clipboard contents (`desktop clipboard get`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clipboard {
    pub available_formats: Vec<String>,
    pub text: Option<String>,
}

/// A single pixel's color (`desktop pixel`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pixel {
    pub x: i32,
    pub y: i32,
    pub hex: String,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// What to capture (`desktop screenshot`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaptureTarget {
    /// The whole virtual screen (all monitors).
    Screen,
    /// A monitor by 1-based index (as listed by `desktop displays`).
    Display(usize),
    /// A window by id ("0x...").
    Window(String),
    /// A region in backend-native global desktop coordinates.
    Region { x: i32, y: i32, w: i32, h: i32 },
}

/// The result of a capture. `png` holds the encoded bytes; the CLI decides
/// whether to write a file or emit a base64 envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capture {
    pub width: u32,
    pub height: u32,
    /// Base64 on the wire. A `Vec<u8>` serializes as a JSON array of decimal
    /// numbers, which turns a few megabytes of PNG into three times that in
    /// digits and commas.
    #[serde(with = "png_base64")]
    pub png: Vec<u8>,
    /// True when the backend captured window pixels regardless of occlusion
    /// (PrintWindow), false for on-screen BitBlt captures.
    pub occlusion_independent: bool,
    pub backend: String,
}

/// Backend capability + permission report (`desktop doctor`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Doctor {
    pub backend: String,
    pub os: String,
    pub os_version: String,
    pub capabilities: Capabilities,
    pub permissions: Permissions,
}

/// Live OS-permission grants for the host process (`desktop permissions`).
/// A capability can be present in [`Capabilities`] yet unusable until the
/// matching permission here is granted.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Permissions {
    /// Accessibility — required for the AX tree/actions, synthetic input, and
    /// managing other apps' windows. (macOS "Accessibility".)
    pub accessibility: bool,
    /// Screen capture — required to screenshot other apps' pixels and read
    /// window titles. (macOS "Screen Recording".)
    pub screen_recording: bool,
    /// Permission to post synthetic input events. (macOS folds this into
    /// Accessibility; reported separately for a precise diagnosis.)
    pub input: bool,
}

impl Permissions {
    /// Every permission this platform needs is granted.
    pub fn all_granted() -> Self {
        Permissions {
            accessibility: true,
            screen_recording: true,
            input: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {

    /// Every model crosses the control socket and is read back into the same
    /// struct on the other side, so a field that serializes but will not
    /// deserialize breaks the mount rather than one command. The empty cases
    /// are the ones that bite: `skip_serializing_if` omits them, and without a
    /// default the trip back fails on exactly the common shape.
    #[test]
    fn models_survive_the_round_trip() {
        let node = AxNode {
            id: "1".into(),
            role: "AXButton".into(),
            name: "OK".into(),
            value: None,
            enabled: true,
            focused: false,
            rect: Rect {
                x: 0,
                y: 0,
                w: 10,
                h: 10,
            },
            children: Vec::new(),
        };
        let text = serde_json::to_string(&node).unwrap();
        assert!(!text.contains("children"), "empty children are omitted");
        let back: AxNode = serde_json::from_str(&text).unwrap();
        assert_eq!(back.role, "AXButton");
        assert!(back.children.is_empty());
        assert!(back.value.is_none());

        let back: AxQuery =
            serde_json::from_str(&serde_json::to_string(&AxQuery::default()).unwrap()).unwrap();
        assert!(back.text.is_none() && back.role.is_none());

        let window = WindowQuery::default();
        let back: WindowQuery =
            serde_json::from_str(&serde_json::to_string(&window).unwrap()).unwrap();
        assert!(back.text.is_none() && back.pid.is_none());
    }

    /// Capture bytes travel as base64. As a `Vec<u8>` they would serialize to a
    /// JSON array of decimal numbers — several times the size, for a payload
    /// that is already megabytes.
    #[test]
    fn captures_travel_as_base64() {
        let capture = Capture {
            width: 1,
            height: 1,
            png: vec![0x89, 0x50, 0x4e, 0x47],
            occlusion_independent: true,
            backend: "test".into(),
        };
        let text = serde_json::to_string(&capture).unwrap();
        assert!(text.contains("\"iVBORw==\""), "png is base64: {text}");
        let back: Capture = serde_json::from_str(&text).unwrap();
        assert_eq!(back.png, capture.png);
    }
    use super::*;

    fn win(pid: u32) -> Window {
        Window {
            id: format!("0x{pid:X}"),
            title: "T".into(),
            process: "p".into(),
            pid,
            bounds: Rect {
                x: 0,
                y: 0,
                w: 1,
                h: 1,
            },
            display_id: "display-1".into(),
            scale: 1.0,
            dpi: 96,
            visible: true,
            focused: false,
            minimized: false,
            maximized: false,
            always_on_top: false,
            z: 0,
        }
    }

    #[test]
    fn window_query_prefixes() {
        assert_eq!(WindowQuery::parse("title:AI").title.as_deref(), Some("AI"));
        assert_eq!(WindowQuery::parse("pid:42").pid, Some(42));
        assert_eq!(WindowQuery::parse("hello").text.as_deref(), Some("hello"));
    }

    #[test]
    fn malformed_pid_matches_nothing_not_everything() {
        let q = WindowQuery::parse("pid:abc");
        assert!(q.is_malformed());
        assert!(
            !q.is_empty(),
            "malformed query must not read as empty (match-all)"
        );
    }

    #[test]
    fn empty_query_is_empty() {
        assert!(WindowQuery::default().is_empty());
    }

    #[test]
    fn ax_query_matching() {
        let node = AxNode {
            id: "ax:0".into(),
            role: "button".into(),
            name: "OK".into(),
            value: Some("v".into()),
            enabled: true,
            focused: false,
            rect: Rect {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            },
            children: vec![],
        };
        assert!(AxQuery::parse("name:ok").matches(&node));
        assert!(AxQuery::parse("role:button").matches(&node));
        assert!(AxQuery::parse("id:ax:0").matches(&node));
        assert!(!AxQuery::parse("id:ax:9").matches(&node));
        assert!(AxQuery::parse("OK").matches(&node));
        assert!(!AxQuery::parse("nope").matches(&node));
    }

    #[test]
    fn window_display_id_unused_ok() {
        // Smoke: constructing a Window is fine (guards the DTO shape).
        let _ = win(1);
    }
}

/// What a command just acted on, as far as the viewer needs to know.
///
/// Synthetic input is always in **global** desktop coordinates, on every
/// backend. A command's `target` narrows *delivery* — it names the process the
/// event is posted to — and never changes the space the coordinates are in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Acted {
    /// A point in global desktop coordinates.
    At { x: i32, y: i32 },
    /// The display holding a point, without implying that input occurred at
    /// that point. Used when process-directed input identifies a display but
    /// not a unique top-level window.
    Display { x: i32, y: i32 },
    /// A point delivered to a specific window. The viewer follows the window
    /// while still marking the global point that was acted on.
    AtWindow { x: i32, y: i32, id: String },
    /// A window whose operation may make it disappear. The point is only a
    /// fallback display location and must not be drawn as an acted-on marker.
    WindowWithFallback { id: String, x: i32, y: i32 },
    /// Something changed, but nothing said where.
    Somewhere,
}

/// PNG bytes as base64, so a capture crossing the control socket costs its own
/// size rather than triple.
mod png_base64 {
    use base64::Engine as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        base64::engine::general_purpose::STANDARD
            .encode(bytes)
            .serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(deserializer)?;
        base64::engine::general_purpose::STANDARD
            .decode(text)
            .map_err(serde::de::Error::custom)
    }
}
