//! Sessionless desktop device I/O for LingXia tools and hosts.
//!
//! This crate is linked directly into the process that drives the desktop,
//! such as `lxdev`, a host runtime, or the in-process JS binding. It is
//! session-less: it calls the local OS APIs directly. Every operation returns
//! typed DTOs ([`model`]) that serialize to the `desktop` command contract's
//! JSON, and a single [`Error`] taxonomy that maps to stable exit codes.

pub mod error;
pub mod model;
#[cfg(feature = "wire")]
pub mod wire;

#[cfg(feature = "supervision")]
mod supervision_state;

pub use error::{Error, ErrorCode, Result};
pub use model::{
    Ack, Acted, AxNode, AxQuery, Capabilities, Capture, CaptureTarget, Clipboard, Display, Doctor,
    LaunchResult, Modifier, MouseButton, Permissions, Pixel, ProcessInfo, QuitTarget, Rect, Window,
    WindowQuery, WindowTarget,
};

/// Who the user must grant permission to, by the name they will look for.
///
/// macOS records these grants against the responsible process: the app bundle
/// when a product answers these commands, and the terminal when a development
/// tool runs them in its own process. Saying "this terminal" to someone whose
/// *app* was refused sends them to the wrong row in System Settings, and
/// naming the binary of a bare CLI sends them to a row that does not exist.
#[cfg(all(any(feature = "input", feature = "window"), target_os = "macos"))]
pub(crate) fn responsible_app() -> String {
    if let Some(name) = backend::responsible_app_name() {
        return name;
    }
    "this terminal".to_string()
}

/// App lifecycle (`desktop app ...`).
#[cfg(feature = "app")]
pub mod app {
    pub use crate::backend::{app_launch as launch, app_quit as quit};
}

/// Process control (`desktop process ...`).
#[cfg(feature = "process")]
pub mod process {
    pub use crate::backend::{process_kill as kill, process_list as list};
}

/// Native accessibility (`desktop ax ...`).
#[cfg(feature = "ax")]
pub mod ax {
    pub use crate::backend::{
        ax_collapse as collapse, ax_expand as expand, ax_focus as focus, ax_hit_test as hit_test,
        ax_invoke as invoke, ax_query as query, ax_scroll_into_view as scroll_into_view,
        ax_select as select, ax_set_value as set_value, ax_tree as tree, ax_wait as wait,
    };
}

/// Wait for a window to appear (`desktop wait window`).
#[cfg(feature = "window")]
pub fn wait_window(query: &WindowQuery, visible: Option<bool>, timeout_ms: u64) -> Result<Window> {
    backend::wait_window(query, visible, timeout_ms)
}

/// Clipboard access (`desktop clipboard ...`).
#[cfg(feature = "clipboard")]
pub mod clipboard {
    pub use crate::backend::{
        clipboard_clear as clear, clipboard_get as get, clipboard_paste as paste,
        clipboard_set as set,
    };
}

/// Synthetic input (`desktop pointer` / `desktop key`). All mutating.
#[cfg(feature = "input")]
pub mod input {
    pub use crate::backend::{
        key_down, key_press, key_type, key_up, pointer_click, pointer_down, pointer_drag,
        pointer_move, pointer_scroll, pointer_up,
    };
}

/// Persistent session disclosure and the transient activity preview.
///
/// The Cargo feature only compiles the mechanism. A product session must hold
/// [`supervision::SupervisionGuard`] for its lifetime; a remote caller cannot
/// dismiss disclosure. Ordinary snapshot/capture does not take this feature.
#[cfg(feature = "supervision")]
pub mod supervision;

/// Window management (`desktop window ...`). All mutating.
#[cfg(feature = "window")]
pub mod window {
    pub use crate::backend::{
        window_activate as activate, window_close as close, window_focus as focus,
        window_maximize as maximize, window_minimize as minimize, window_move as move_to,
        window_move_display as move_to_display, window_raise as raise, window_resize as resize,
        window_restore as restore, window_set_always_on_top as set_always_on_top,
        window_status as status,
    };
}

/// Sessionless, one-shot visual capture and the optional desktop realtime
/// adapter. Snapshot stays visual-only and never enables the capture contract.
#[cfg(any(feature = "snapshot", feature = "realtime-capture-provider"))]
pub mod capture {
    #[cfg(feature = "snapshot")]
    pub use crate::backend::{pixel, screenshot as snapshot, wait_pixel};

    #[cfg(feature = "realtime-capture-provider")]
    pub use crate::geometry::{identity_geometry, map_normalized_pointer};

    #[cfg(all(
        feature = "realtime-capture-provider",
        any(target_os = "windows", target_os = "macos")
    ))]
    pub use crate::realtime::DesktopRealtimeProvider;
}

#[cfg(feature = "desktop-capture-engine")]
mod engine;

#[cfg(feature = "realtime-capture-provider")]
mod geometry;

#[cfg(all(
    feature = "realtime-capture-provider",
    any(target_os = "windows", target_os = "macos")
))]
mod realtime;

#[cfg(all(feature = "native", target_os = "windows"))]
#[path = "win/mod.rs"]
mod backend;

#[cfg(all(feature = "native", target_os = "macos"))]
#[path = "mac/mod.rs"]
mod backend;

#[cfg(all(
    feature = "native",
    not(any(target_os = "windows", target_os = "macos"))
))]
#[path = "stub.rs"]
mod backend;

/// Backend + capability + live-permission report (`desktop doctor`).
#[cfg(feature = "diagnostics")]
pub fn doctor() -> Doctor {
    backend::doctor()
}

/// The host process's current OS-permission grants, without prompting
/// (`desktop permissions`).
#[cfg(feature = "diagnostics")]
pub fn permissions() -> Permissions {
    backend::permissions()
}

/// Trigger the OS permission prompts for anything not yet granted, then report
/// the resulting state (`desktop permissions --request`). The OS cannot grant
/// silently: the user must approve (and often relaunch) for the change to take
/// effect, so a follow-up call may still show `false` until then.
#[cfg(feature = "diagnostics")]
pub fn request_permissions() -> Permissions {
    backend::request_permissions()
}

/// Enumerate monitors (`desktop displays`).
#[cfg(feature = "window")]
pub fn displays() -> Result<Vec<Display>> {
    backend::displays()
}

/// Enumerate top-level OS windows, optionally filtered (`desktop windows`).
#[cfg(feature = "window")]
pub fn windows(query: &WindowQuery) -> Result<Vec<Window>> {
    backend::windows(query)
}

/// Resolve the top-level window that native pointer input would reach.
/// Internal host-viewer plumbing, not a control-surface method.
#[cfg(all(feature = "input", feature = "window", target_os = "windows"))]
#[doc(hidden)]
pub fn input_window_at_point(x: i32, y: i32) -> Option<Window> {
    backend::input_window_at_point(x, y)
}

#[cfg(all(
    test,
    feature = "diagnostics",
    any(target_os = "macos", target_os = "windows")
))]
mod tests {
    #[test]
    fn doctor_reports_only_compiled_capabilities() {
        let capabilities = super::doctor().capabilities;
        assert_eq!(capabilities.displays, cfg!(feature = "window"));
        assert_eq!(capabilities.windows, cfg!(feature = "window"));
        assert_eq!(capabilities.window_management, cfg!(feature = "window"));
        assert_eq!(capabilities.screenshot, cfg!(feature = "snapshot"));
        assert_eq!(capabilities.pixel, cfg!(feature = "snapshot"));
        assert_eq!(capabilities.pointer, cfg!(feature = "input"));
        assert_eq!(capabilities.key, cfg!(feature = "input"));
        assert_eq!(capabilities.clipboard, cfg!(feature = "clipboard"));
        assert_eq!(capabilities.ax_tree, cfg!(feature = "ax"));
    }
}
