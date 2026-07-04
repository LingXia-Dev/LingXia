//! Apple platform implementation for LingXia
//!
//! This module provides Swift FFI interfaces for iOS and macOS platforms.

mod app;
mod device;
mod ffi;

/// True when the shell reports a usable SMAppService (macOS 13+). -1 from the
/// bridge means unsupported; 0/1 are real disabled/enabled states.
#[cfg(target_os = "macos")]
pub(crate) fn autostart_probe_supported() -> bool {
    ffi::autostart_is_enabled() >= 0
}
mod file;
mod keyboard;
mod location;
mod media;
mod mouse;
mod network;
mod pull_to_refresh;
mod resources;
mod screenshot;
mod surface;
mod ui_update;
mod user_feedback;
mod video_player;
mod wifi;

// Re-export Platform
pub use app::Platform;
pub use app::apply_staged_macos_update;
