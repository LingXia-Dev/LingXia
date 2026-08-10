//! Apple platform implementation for LingXia
//!
//! This module provides Swift FFI interfaces for iOS and macOS platforms.

mod app;
mod device;
mod ffi;

/// True when the shell can use SMAppService (macOS 13+). Capability discovery
/// must not query live login-item state: that first ServiceManagement call can
/// block while macOS initializes its helper connection.
#[cfg(target_os = "macos")]
pub(crate) fn autostart_probe_supported() -> bool {
    ffi::autostart_is_supported()
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
