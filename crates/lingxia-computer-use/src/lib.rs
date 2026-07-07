//! Local desktop OS automation backend for LingXia devtools.
//!
//! This crate is linked directly into the process that drives the desktop
//! (`lxdev.exe` for the CLI; a future in-process JS binding). It is
//! session-less: it calls the local OS APIs directly. Every operation returns
//! typed DTOs ([`model`]) that serialize to the `desktop` command contract's
//! JSON, and a single [`Error`] taxonomy that maps to stable exit codes.

pub mod error;
pub mod model;

pub use error::{Error, ErrorCode, Result};
pub use model::{
    Ack, AxNode, AxQuery, Capabilities, Capture, CaptureTarget, Clipboard, Display, Doctor,
    LaunchResult, Modifier, MouseButton, Pixel, ProcessInfo, QuitTarget, Rect, Window, WindowQuery,
    WindowTarget,
};

/// App lifecycle (`desktop app ...`).
pub mod app {
    pub use crate::backend::{app_launch as launch, app_quit as quit};
}

/// Process control (`desktop process ...`).
pub mod process {
    pub use crate::backend::{process_kill as kill, process_list as list};
}

/// Native accessibility (`desktop ax ...`).
pub mod ax {
    pub use crate::backend::{
        ax_collapse as collapse, ax_expand as expand, ax_focus as focus, ax_invoke as invoke,
        ax_query as query, ax_scroll_into_view as scroll_into_view, ax_select as select,
        ax_set_value as set_value, ax_tree as tree, ax_wait as wait,
    };
}

/// Wait for a window to appear (`desktop wait window`).
pub fn wait_window(query: &WindowQuery, visible: Option<bool>, timeout_ms: u64) -> Result<Window> {
    backend::wait_window(query, visible, timeout_ms)
}

/// Wait for a pixel color (`desktop wait pixel`).
pub fn wait_pixel(x: i32, y: i32, hex: &str, tolerance: u8, timeout_ms: u64) -> Result<Pixel> {
    backend::wait_pixel(x, y, hex, tolerance, timeout_ms)
}

/// Clipboard access (`desktop clipboard ...`).
pub mod clipboard {
    pub use crate::backend::{
        clipboard_clear as clear, clipboard_get as get, clipboard_paste as paste,
        clipboard_set as set,
    };
}

/// Synthetic input (`desktop pointer` / `desktop key`). All mutating.
pub mod input {
    pub use crate::backend::{
        key_down, key_press, key_type, key_up, pointer_click, pointer_down, pointer_drag,
        pointer_move, pointer_scroll, pointer_up,
    };
}

/// Window management (`desktop window ...`). All mutating.
pub mod window {
    pub use crate::backend::{
        window_activate as activate, window_close as close, window_focus as focus,
        window_maximize as maximize, window_minimize as minimize, window_move as move_to,
        window_move_display as move_to_display, window_raise as raise, window_resize as resize,
        window_restore as restore, window_set_always_on_top as set_always_on_top,
        window_status as status,
    };
}

#[cfg(target_os = "windows")]
#[path = "win/mod.rs"]
mod backend;

#[cfg(not(target_os = "windows"))]
#[path = "stub.rs"]
mod backend;

/// Backend + capability report (`desktop doctor`).
pub fn doctor() -> Doctor {
    backend::doctor()
}

/// Enumerate monitors (`desktop displays`).
pub fn displays() -> Result<Vec<Display>> {
    backend::displays()
}

/// Enumerate top-level OS windows, optionally filtered (`desktop windows`).
pub fn windows(query: &WindowQuery) -> Result<Vec<Window>> {
    backend::windows(query)
}

/// Capture a display/window/region (`desktop screenshot`).
pub fn screenshot(target: CaptureTarget) -> Result<Capture> {
    backend::screenshot(target)
}

/// Read a single pixel's color (`desktop pixel`).
pub fn pixel(x: i32, y: i32) -> Result<Pixel> {
    backend::pixel(x, y)
}
