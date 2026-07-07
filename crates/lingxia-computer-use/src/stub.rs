//! Non-Windows fallback. The desktop backend is Windows-first; other platforms
//! report unsupported explicitly rather than pretending.

use crate::error::{Error, Result};
use crate::model::{
    Capabilities, Capture, CaptureTarget, Display, Doctor, Pixel, Window, WindowQuery, WindowTarget,
};

fn unsupported<T>() -> Result<T> {
    Err(Error::Unsupported(
        "desktop automation backend is only implemented on Windows".to_string(),
    ))
}

pub fn doctor() -> Doctor {
    Doctor {
        backend: "unsupported".to_string(),
        os: std::env::consts::OS.to_string(),
        os_version: String::new(),
        capabilities: Capabilities::default(),
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
pub fn window_activate(_q: WindowQuery) -> Result<Window> {
    unsupported()
}
