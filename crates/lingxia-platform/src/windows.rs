#![allow(clippy::manual_async_fn)]

//! Windows platform implementation for LingXia.

mod app;
mod device;
mod file;
mod location;
mod media;
mod media_preview;
mod mouse;
mod network;
mod pull_to_refresh;
mod screenshot;
mod secure_store;
mod surface;
mod ui_update;
mod user_feedback;
mod video_info;
mod video_player;
mod wifi;

pub(crate) use app::request_windows_app_exit;
pub use app::{Platform, set_windows_app_exit_handler, set_windows_open_url_handler};
pub use media_preview::{
    WindowsMediaPreviewCancel, WindowsMediaPreviewOpen, register_windows_media_preview_host,
};
pub use surface::{
    set_windows_page_visibility_handler, set_windows_surface_closed_handler,
    set_windows_surface_dispose_handler,
};
pub use ui_update::set_windows_ui_update_handler;
pub use video_player::{WindowsVideoCommandDispatcher, register_windows_video_command_dispatcher};

use crate::error::PlatformError;

pub(crate) fn not_supported<T>(name: &str) -> Result<T, PlatformError> {
    Err(PlatformError::NotSupported(format!(
        "{name} is not supported on Windows yet"
    )))
}
