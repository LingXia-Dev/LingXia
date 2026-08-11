//! Backend for targets without desktop device-I/O implementations.
//!
//! Only the selected capability surface is compiled, and calls report the
//! target limitation explicitly.

#[cfg(any(feature = "process", feature = "window"))]
use crate::error::{Error, Result};
#[cfg(any(
    feature = "ax",
    feature = "clipboard",
    feature = "input",
    feature = "process"
))]
use crate::model::Ack;
#[cfg(feature = "clipboard")]
use crate::model::Clipboard;
#[cfg(feature = "process")]
use crate::model::ProcessInfo;
#[cfg(feature = "ax")]
use crate::model::{AxNode, AxQuery};
#[cfg(feature = "diagnostics")]
use crate::model::{Capabilities, Doctor, Permissions};
#[cfg(feature = "snapshot")]
use crate::model::{Capture, CaptureTarget, Pixel};
#[cfg(feature = "window")]
use crate::model::{Display, Window, WindowQuery, WindowTarget};
#[cfg(feature = "app")]
use crate::model::{LaunchResult, QuitTarget};
#[cfg(feature = "input")]
use crate::model::{Modifier, MouseButton};

#[cfg(any(feature = "process", feature = "window"))]
fn unsupported<T>() -> Result<T> {
    Err(Error::Unsupported(format!(
        "desktop device I/O is not implemented for {}",
        std::env::consts::OS
    )))
}

#[cfg(feature = "diagnostics")]
pub fn permissions() -> Permissions {
    Permissions::default()
}

#[cfg(feature = "diagnostics")]
pub fn request_permissions() -> Permissions {
    Permissions::default()
}

#[cfg(feature = "diagnostics")]
pub fn doctor() -> Doctor {
    Doctor {
        backend: "unsupported".to_string(),
        os: std::env::consts::OS.to_string(),
        os_version: String::new(),
        capabilities: Capabilities::default(),
        permissions: Permissions::default(),
    }
}

#[cfg(feature = "window")]
pub fn displays() -> Result<Vec<Display>> {
    unsupported()
}

#[cfg(feature = "window")]
pub fn windows(_query: &WindowQuery) -> Result<Vec<Window>> {
    unsupported()
}

#[cfg(feature = "snapshot")]
pub fn screenshot(_target: CaptureTarget) -> Result<Capture> {
    unsupported()
}

#[cfg(feature = "snapshot")]
pub fn pixel(_x: i32, _y: i32) -> Result<Pixel> {
    unsupported()
}

#[cfg(feature = "window")]
macro_rules! window_stub {
    ($($name:ident),* $(,)?) => {
        $(pub fn $name(_t: &WindowTarget) -> Result<Window> { unsupported() })*
    };
}
#[cfg(feature = "window")]
window_stub!(
    window_focus,
    window_raise,
    window_minimize,
    window_maximize,
    window_restore,
    window_status,
);

#[cfg(feature = "window")]
pub fn window_move(_t: &WindowTarget, _x: i32, _y: i32) -> Result<Window> {
    unsupported()
}
#[cfg(feature = "window")]
pub fn window_move_display(_t: &WindowTarget, _d: &str) -> Result<Window> {
    unsupported()
}
#[cfg(feature = "window")]
pub fn window_resize(_t: &WindowTarget, _w: i32, _h: i32) -> Result<Window> {
    unsupported()
}
#[cfg(feature = "window")]
pub fn window_set_always_on_top(_t: &WindowTarget, _on: bool) -> Result<Window> {
    unsupported()
}
#[cfg(feature = "window")]
pub fn window_close(_t: &WindowTarget) -> Result<Window> {
    unsupported()
}
#[cfg(feature = "window")]
pub fn window_activate(_t: &WindowTarget) -> Result<Window> {
    unsupported()
}

#[cfg(feature = "input")]
pub fn pointer_move(_x: i32, _y: i32, _target: Option<u32>) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "input")]
pub fn pointer_down(_x: i32, _y: i32, _b: MouseButton, _target: Option<u32>) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "input")]
pub fn pointer_up(_x: i32, _y: i32, _b: MouseButton, _target: Option<u32>) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "input")]
pub fn pointer_click(
    _x: i32,
    _y: i32,
    _b: MouseButton,
    _c: u32,
    _target: Option<u32>,
) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "input")]
pub fn pointer_scroll(_x: i32, _y: i32, _dx: i32, _dy: i32, _target: Option<u32>) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "input")]
pub fn pointer_drag(
    _fx: i32,
    _fy: i32,
    _tx: i32,
    _ty: i32,
    _b: MouseButton,
    _target: Option<u32>,
) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "input")]
pub fn key_type(_text: &str, _target: Option<u32>) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "input")]
pub fn key_press(_name: &str, _mods: &[Modifier], _target: Option<u32>) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "input")]
pub fn key_down(_name: &str, _target: Option<u32>) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "input")]
pub fn key_up(_name: &str, _target: Option<u32>) -> Result<Ack> {
    unsupported()
}

#[cfg(feature = "clipboard")]
pub fn clipboard_get() -> Result<Clipboard> {
    unsupported()
}
#[cfg(feature = "clipboard")]
pub fn clipboard_set(_text: &str) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "clipboard")]
pub fn clipboard_clear() -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "clipboard")]
pub fn clipboard_paste() -> Result<Ack> {
    unsupported()
}

#[cfg(feature = "ax")]
pub fn ax_tree(_window_id: &str, _depth: Option<u32>, _max: Option<usize>) -> Result<AxNode> {
    unsupported()
}
#[cfg(feature = "ax")]
pub fn ax_hit_test(_x: i32, _y: i32) -> Result<AxNode> {
    unsupported()
}
#[cfg(feature = "ax")]
pub fn ax_query(
    _window_id: &str,
    _q: &AxQuery,
    _all: bool,
    _index: Option<usize>,
) -> Result<Vec<AxNode>> {
    unsupported()
}
#[cfg(feature = "ax")]
pub fn ax_invoke(_window_id: &str, _q: &AxQuery) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "ax")]
pub fn ax_focus(_window_id: &str, _q: &AxQuery) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "ax")]
pub fn ax_set_value(_window_id: &str, _q: &AxQuery, _v: &str) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "ax")]
pub fn ax_select(_window_id: &str, _q: &AxQuery) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "ax")]
pub fn ax_expand(_window_id: &str, _q: &AxQuery) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "ax")]
pub fn ax_collapse(_window_id: &str, _q: &AxQuery) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "ax")]
pub fn ax_scroll_into_view(_window_id: &str, _q: &AxQuery) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "ax")]
pub fn ax_wait(_window_id: &str, _q: &AxQuery, _state: &str, _timeout_ms: u64) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "window")]
pub fn wait_window(_q: &WindowQuery, _visible: Option<bool>, _timeout_ms: u64) -> Result<Window> {
    unsupported()
}
#[cfg(feature = "snapshot")]
pub fn wait_pixel(_x: i32, _y: i32, _hex: &str, _tol: u8, _timeout_ms: u64) -> Result<Pixel> {
    unsupported()
}

#[cfg(feature = "process")]
pub fn process_list(_filter: Option<&str>) -> Result<Vec<ProcessInfo>> {
    unsupported()
}
#[cfg(feature = "process")]
pub fn process_kill(_pid: u32, _force: bool) -> Result<Ack> {
    unsupported()
}
#[cfg(feature = "app")]
pub fn app_launch(
    _app: &str,
    _args: &[String],
    _wait_window: Option<&str>,
    _timeout_ms: u64,
) -> Result<LaunchResult> {
    unsupported()
}
#[cfg(feature = "app")]
pub fn app_quit(_target: QuitTarget, _force: bool) -> Result<Ack> {
    unsupported()
}
