//! Windows SDK default shell UI.
//!
//! This module is the Windows counterpart of the Apple SDK shell: it owns
//! native window chrome, sidebar/tabbar/panel layout, and the host glue that
//! embeds LingXia WebViews into that native UI.

mod chrome;
pub mod clipboard;
pub mod context_menu;
mod runtime;
mod style;
pub mod terminal_grid;
mod terminal_panel;
pub mod text_input;

pub use chrome::{
    WindowsShellAddressBarLayout, WindowsShellAuxiliaryItemLayout, WindowsShellHeaderActionLayout,
    WindowsShellNavigationBarLayout, WindowsShellPanelActivatorLayout,
    WindowsShellTabBarItemLayout, WindowsShellTabBarLayout, WindowsShellTabBarPosition,
    WindowsShellWindowLayout, begin_address_edit,
};

pub(crate) use runtime::open_home_app;

/// Registers the Windows SDK default shell.
///
/// Must run before the first window is created so hosts get the custom
/// borderless frame and shell event routing.
pub(crate) fn install() {
    chrome::install();
    runtime::install();
}
