//! Request parameters for the `desktop.*` commands.
//!
//! A product runs these inside the app rather than in the calling process, so
//! every argument list has to cross a socket. These structs are that crossing:
//! one per call shape, shared literally by the host that answers and the
//! client that asks, so neither can grow a field the other does not know.
//!
//! They live here rather than with the method-name constants because they are
//! made of [`crate::model`] types; the constants stay in the control protocol
//! with every other namespace's.

use serde::{Deserialize, Serialize};

use crate::model::{
    AxQuery, CaptureTarget, Modifier, MouseButton, QuitTarget, WindowQuery, WindowTarget,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Windows {
    pub query: WindowQuery,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screenshot {
    pub target: CaptureTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitWindow {
    pub query: WindowQuery,
    pub visible: Option<bool>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitPixel {
    pub x: i32,
    pub y: i32,
    pub hex: String,
    pub tolerance: u8,
    pub timeout_ms: u64,
}

/// Every window command that only needs to say which window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowAction {
    pub target: WindowTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowMove {
    pub target: WindowTarget,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowMoveDisplay {
    pub target: WindowTarget,
    pub display_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowResize {
    pub target: WindowTarget,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowAlwaysOnTop {
    pub target: WindowTarget,
    pub on: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointerMove {
    pub x: i32,
    pub y: i32,
    pub target: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointerButton {
    pub x: i32,
    pub y: i32,
    pub button: MouseButton,
    pub target: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointerClick {
    pub x: i32,
    pub y: i32,
    pub button: MouseButton,
    pub count: u32,
    pub target: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointerScroll {
    pub x: i32,
    pub y: i32,
    pub dx: i32,
    pub dy: i32,
    pub target: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointerDrag {
    pub from_x: i32,
    pub from_y: i32,
    pub to_x: i32,
    pub to_y: i32,
    pub button: MouseButton,
    pub target: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyText {
    pub text: String,
    pub target: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
}

/// `key down` and `key up`, which name a key rather than typing text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyName {
    pub name: String,
    pub target: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPress {
    pub name: String,
    pub modifiers: Vec<Modifier>,
    pub target: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxTree {
    pub window_id: String,
    pub depth: Option<u32>,
    pub max_nodes: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxSearch {
    pub window_id: String,
    pub query: AxQuery,
    pub all: bool,
    pub index: Option<usize>,
}

/// Every accessibility command that acts on one matched node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxAction {
    pub window_id: String,
    pub query: AxQuery,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxSetValue {
    pub window_id: String,
    pub query: AxQuery,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxWait {
    pub window_id: String,
    pub query: AxQuery,
    pub state: String,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardSet {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessList {
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessKill {
    pub pid: u32,
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppLaunch {
    pub app: String,
    pub args: Vec<String>,
    pub wait_window: Option<String>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppQuit {
    pub target: QuitTarget,
    pub force: bool,
}

#[cfg(test)]
mod tests {
    use super::KeyText;

    #[test]
    fn input_window_metadata_is_backward_compatible() {
        let old: KeyText = serde_json::from_value(serde_json::json!({
            "text": "hello",
            "target": 42
        }))
        .expect("old clients omit viewer metadata");
        assert_eq!(old.target, Some(42));
        assert_eq!(old.window_id, None);

        let without = serde_json::to_value(KeyText {
            text: "hello".into(),
            target: Some(42),
            window_id: None,
        })
        .expect("serialize key input");
        assert!(without.get("window_id").is_none());

        let with = serde_json::to_value(KeyText {
            text: "hello".into(),
            target: Some(42),
            window_id: Some("0x7f".into()),
        })
        .expect("serialize targeted key input");
        assert_eq!(with["target"], 42);
        assert_eq!(with["window_id"], "0x7f");
    }
}
