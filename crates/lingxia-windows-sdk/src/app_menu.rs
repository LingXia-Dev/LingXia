//! Windows application-menu facade.
//!
//! Menu content and command handling are Windows host SDK policy. The webview
//! crate only exposes HWND primitives used here to reach host windows.

mod model;
mod native;

pub use model::{
    WindowsAppMenu, WindowsAppMenuCommandHandler, WindowsAppMenuEntry, WindowsAppMenuItem,
};
pub use native::{set_windows_app_menu, set_windows_app_menu_command_handler};

#[cfg(feature = "runtime")]
pub(crate) use native::install_host_window_menu_support;
#[cfg(feature = "device-frame")]
pub(crate) use native::refresh_host_window_menu;
