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
mod theme;

pub use chrome::{
    WindowsShellAddressBarLayout, WindowsShellAuxiliaryItemLayout, WindowsShellHeaderActionLayout,
    WindowsShellNavigationBarLayout, WindowsShellPanelActivatorLayout,
    WindowsShellTabBarItemLayout, WindowsShellTabBarLayout, WindowsShellTabBarPosition,
    WindowsShellWindowLayout, begin_address_edit, begin_panel_address_edit,
};

pub(crate) use chrome::shell_chrome_dirty_rects;

/// Height of the shell's top caption strip (where the lxapp navbar and browser
/// asides' address bars live). Exposed so the host's invalidation can repaint
/// the whole top band when the attached panel layout changes.
pub(crate) fn shell_top_bar_height() -> i32 {
    style::SHELL_TOP_BAR_HEIGHT
}

/// Re-read the Win11 light/dark + system-accent theme into the shell palette
/// cache. Returns `true` when the values changed, so the window proc can
/// repaint only on a real theme change.
pub(crate) fn refresh_system_theme() -> bool {
    theme::refresh()
}

pub(crate) use runtime::{
    handle_menu_bar_surface_action, open_home_app, set_home_app_id, update_surface_width,
};
pub(crate) use terminal_panel::{
    begin_divider_drag, divider_orientation_at, end_divider_drag, update_divider_drag,
};

/// Registers the Windows SDK default shell.
///
/// Must run before the first window is created so hosts get the custom
/// borderless frame and shell event routing.
pub(crate) fn install() {
    chrome::install();
    runtime::install();
}
