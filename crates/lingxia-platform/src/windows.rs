#![allow(clippy::manual_async_fn)]

//! Windows platform implementation for LingXia.

mod app;
mod device;
mod file;
mod location;
mod media;
mod mouse;
mod screenshot;
mod secure_store;
mod surface;
mod ui_update;
mod user_feedback;
mod video_player;

pub use app::{Platform, set_windows_app_exit_handler, set_windows_open_url_handler};
pub(crate) use app::request_windows_app_exit;
pub use ui_update::set_windows_ui_update_handler;
pub use video_player::{WindowsVideoCommandDispatcher, register_windows_video_command_dispatcher};

use crate::error::PlatformError;

pub(crate) fn not_supported<T>(name: &str) -> Result<T, PlatformError> {
    Err(PlatformError::NotSupported(format!(
        "{name} is not supported on Windows yet"
    )))
}
