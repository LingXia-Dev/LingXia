#![allow(clippy::manual_async_fn)]

//! Windows platform implementation for LingXia.

mod app;
mod device;
mod file;
mod keyboard;
mod location;
mod media;
mod media_preview;
mod mouse;
mod network;
mod pull_to_refresh;
mod registry;
mod screenshot;
mod secure_store;
mod surface;
mod ui_update;
mod update;
mod update_callout;
mod update_card;
mod user_feedback;
mod video_compress;
mod video_info;
mod video_player;
mod wifi;

pub(crate) use app::request_windows_app_exit;
pub use app::{
    Platform, current_locale, set_windows_activate_browser_tab_handler,
    set_windows_app_exit_handler, set_windows_builtin_browser_page_handler,
    set_windows_close_browser_tab_handler, set_windows_lxapp_main_activation_handler,
    set_windows_open_url_handler, set_windows_shell_pins_handler,
    set_windows_sidebar_actions_handler, set_windows_tray_click_intercept_handler,
    set_windows_tray_menu_handler,
};
pub use media_preview::{
    WindowsMediaPreviewCancel, WindowsMediaPreviewOpen, register_windows_media_preview_host,
};
pub use pull_to_refresh::set_windows_pull_to_refresh_handler;
pub use surface::{
    WindowsUrlSurfaceWebTag, install_windows_aside_panel_bridge, set_windows_layout_plan_handler,
    set_windows_managed_aside_event_handler, set_windows_managed_native_surface_open_handler,
    set_windows_managed_surface_close_handler, set_windows_managed_surface_visible_handler,
    set_windows_page_visibility_handler, set_windows_surface_closed_handler,
    set_windows_surface_dispose_handler, set_windows_url_surface_handler,
};
pub use ui_update::{
    set_windows_capsule_rect_provider, set_windows_home_first_ready_handler,
    set_windows_host_appearance_dark, set_windows_ui_update_async_handler,
    set_windows_ui_update_handler, sync_windows_ui,
};
pub use update::apply_staged_windows_update;
pub use video_player::{WindowsVideoCommandDispatcher, register_windows_video_command_dispatcher};

use crate::error::PlatformError;

pub(crate) fn not_supported<T>(name: &str) -> Result<T, PlatformError> {
    Err(PlatformError::NotSupported(format!(
        "{name} is not supported on Windows yet"
    )))
}
