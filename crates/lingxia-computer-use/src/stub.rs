//! Non-Windows fallback. The desktop backend is Windows-first; other platforms
//! report unsupported explicitly rather than pretending.

use crate::error::{Error, Result};
use crate::model::{
    Ack, AxNode, AxQuery, Capabilities, Capture, CaptureTarget, Clipboard, Display, Doctor,
    LaunchResult, Modifier, MouseButton, Permissions, Pixel, ProcessInfo, QuitTarget, Window,
    WindowQuery, WindowTarget,
};

fn unsupported<T>() -> Result<T> {
    Err(Error::Unsupported(
        "desktop automation backend is only implemented on Windows".to_string(),
    ))
}

pub fn permissions() -> Permissions {
    Permissions::default()
}

pub fn request_permissions() -> Permissions {
    Permissions::default()
}

pub fn doctor() -> Doctor {
    Doctor {
        backend: "unsupported".to_string(),
        os: std::env::consts::OS.to_string(),
        os_version: String::new(),
        capabilities: Capabilities::default(),
        permissions: Permissions::default(),
    }
}

pub fn displays() -> Result<Vec<Display>> {
    unsupported()
}

pub fn windows(_query: &WindowQuery) -> Result<Vec<Window>> {
    unsupported()
}

pub fn screenshot(_target: CaptureTarget) -> Result<Capture> {
    unsupported()
}

pub fn pixel(_x: i32, _y: i32) -> Result<Pixel> {
    unsupported()
}

macro_rules! win_stub {
    ($($name:ident),* $(,)?) => {
        $(pub fn $name(_t: &WindowTarget) -> Result<Window> { unsupported() })*
    };
}
win_stub!(
    window_focus,
    window_raise,
    window_minimize,
    window_maximize,
    window_restore,
    window_status,
);

pub fn window_move(_t: &WindowTarget, _x: i32, _y: i32) -> Result<Window> {
    unsupported()
}
pub fn window_move_display(_t: &WindowTarget, _d: &str) -> Result<Window> {
    unsupported()
}
pub fn window_resize(_t: &WindowTarget, _w: i32, _h: i32) -> Result<Window> {
    unsupported()
}
pub fn window_set_always_on_top(_t: &WindowTarget, _on: bool) -> Result<Window> {
    unsupported()
}
pub fn window_close(_t: &WindowTarget) -> Result<Window> {
    unsupported()
}
pub fn window_activate(_t: &WindowTarget) -> Result<Window> {
    unsupported()
}

pub fn pointer_move(_x: i32, _y: i32, _target: Option<u32>) -> Result<Ack> {
    unsupported()
}
pub fn pointer_down(_x: i32, _y: i32, _b: MouseButton, _target: Option<u32>) -> Result<Ack> {
    unsupported()
}
pub fn pointer_up(_x: i32, _y: i32, _b: MouseButton, _target: Option<u32>) -> Result<Ack> {
    unsupported()
}
pub fn pointer_click(_x: i32, _y: i32, _b: MouseButton, _c: u32, _target: Option<u32>) -> Result<Ack> {
    unsupported()
}
pub fn pointer_scroll(_x: i32, _y: i32, _dx: i32, _dy: i32, _target: Option<u32>) -> Result<Ack> {
    unsupported()
}
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
pub fn key_type(_text: &str, _target: Option<u32>) -> Result<Ack> {
    unsupported()
}
pub fn key_press(_name: &str, _mods: &[Modifier], _target: Option<u32>) -> Result<Ack> {
    unsupported()
}
pub fn key_down(_name: &str, _target: Option<u32>) -> Result<Ack> {
    unsupported()
}
pub fn key_up(_name: &str, _target: Option<u32>) -> Result<Ack> {
    unsupported()
}

pub fn clipboard_get() -> Result<Clipboard> {
    unsupported()
}
pub fn clipboard_set(_text: &str) -> Result<Ack> {
    unsupported()
}
pub fn clipboard_clear() -> Result<Ack> {
    unsupported()
}
pub fn clipboard_paste() -> Result<Ack> {
    unsupported()
}

pub fn ax_tree(_window_id: &str, _depth: Option<u32>, _max: Option<usize>) -> Result<AxNode> {
    unsupported()
}
pub fn ax_hit_test(_x: i32, _y: i32) -> Result<AxNode> {
    unsupported()
}
pub fn ax_query(
    _window_id: &str,
    _q: &AxQuery,
    _all: bool,
    _index: Option<usize>,
) -> Result<Vec<AxNode>> {
    unsupported()
}
pub fn ax_invoke(_window_id: &str, _q: &AxQuery) -> Result<Ack> {
    unsupported()
}
pub fn ax_focus(_window_id: &str, _q: &AxQuery) -> Result<Ack> {
    unsupported()
}
pub fn ax_set_value(_window_id: &str, _q: &AxQuery, _v: &str) -> Result<Ack> {
    unsupported()
}
pub fn ax_select(_window_id: &str, _q: &AxQuery) -> Result<Ack> {
    unsupported()
}
pub fn ax_expand(_window_id: &str, _q: &AxQuery) -> Result<Ack> {
    unsupported()
}
pub fn ax_collapse(_window_id: &str, _q: &AxQuery) -> Result<Ack> {
    unsupported()
}
pub fn ax_scroll_into_view(_window_id: &str, _q: &AxQuery) -> Result<Ack> {
    unsupported()
}
pub fn ax_wait(_window_id: &str, _q: &AxQuery, _state: &str, _timeout_ms: u64) -> Result<Ack> {
    unsupported()
}
pub fn wait_window(_q: &WindowQuery, _visible: Option<bool>, _timeout_ms: u64) -> Result<Window> {
    unsupported()
}
pub fn wait_pixel(_x: i32, _y: i32, _hex: &str, _tol: u8, _timeout_ms: u64) -> Result<Pixel> {
    unsupported()
}

pub fn process_list(_filter: Option<&str>) -> Result<Vec<ProcessInfo>> {
    unsupported()
}
pub fn process_kill(_pid: u32, _force: bool) -> Result<Ack> {
    unsupported()
}
pub fn app_launch(
    _app: &str,
    _args: &[String],
    _wait_window: Option<&str>,
    _timeout_ms: u64,
) -> Result<LaunchResult> {
    unsupported()
}
pub fn app_quit(_target: QuitTarget, _force: bool) -> Result<Ack> {
    unsupported()
}
