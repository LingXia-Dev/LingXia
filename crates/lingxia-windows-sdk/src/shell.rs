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
#[cfg(feature = "terminal-runtime")]
pub mod terminal_grid;
/// Terminal pane rendering lives behind `terminal-runtime` because it links
/// libghostty-vt (`lingxia-terminal`). Hosts that enable the shell chrome via
/// `browser-runtime` but do not opt into a terminal (e.g. the device-frame
/// runner) must not link that native library, so provide inert stubs for the
/// handful of entry points the panel painter calls.
#[cfg(not(feature = "terminal-runtime"))]
pub mod terminal_grid {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::HDC;

    pub(super) fn session_surface_background(_session_id: u64) -> Option<u32> {
        None
    }
    pub(super) fn panel_snapshot_text(_panel_id: &str) -> Option<String> {
        None
    }
    pub(super) fn set_panel_tab_title_rects(
        _panel_id: &str,
        _hwnd: isize,
        _titles: Vec<(u64, RECT)>,
    ) {
    }
    pub(super) fn draw_panel_panes(_hdc: HDC, _panel_id: &str, _body: RECT) -> bool {
        false
    }
}
mod terminal_panel;
pub mod text_input;
mod theme;

pub use chrome::{
    WindowsShellAddressBarLayout, WindowsShellAuxiliaryItemLayout, WindowsShellHeaderActionLayout,
    WindowsShellNavigationBarLayout, WindowsShellPanelActivatorLayout,
    WindowsShellTabBarItemLayout, WindowsShellTabBarLayout, WindowsShellTabBarPosition,
    WindowsShellWindowLayout,
};
// Inline address-bar editing exists only for the browser (asides + tabs).
#[cfg(feature = "browser-runtime")]
pub use chrome::{begin_address_edit, begin_panel_address_edit};

pub(crate) use chrome::shell_chrome_dirty_rects;
pub(crate) use chrome::{
    PHONE_SWITCHER_SHEET_RADIUS, PhoneTabSwitcherHit, PhoneTabSwitcherLayout,
    paint_phone_tab_switcher, paint_shell_notice, phone_tab_click_command, phone_tab_close_command,
    phone_tab_switcher_hit, phone_tab_switcher_layout,
};
pub(crate) use chrome::{
    SIDEBAR_TABBAR_POPUP_RADIUS, collapsed_sidebar_tabbar_click_command,
    collapsed_sidebar_tabbar_popup, collapsed_sidebar_tabbar_popup_hit,
    paint_collapsed_sidebar_tabbar_popup, paint_transparent_tabbar_overlay,
    transparent_tabbar_overlay_rect,
};

pub(crate) use chrome::{workspace_corner_radii, workspace_silhouette_rect};

/// Height of the shell-owned top caption strip. Lxapp navigation bars belong
/// to the main region below it; browser address chrome may use this strip.
pub(crate) fn shell_top_bar_height() -> i32 {
    style::SHELL_TOP_BAR_HEIGHT
}

/// Corner radius of the rounded workspace silhouette (the content card).
pub(crate) fn shell_content_radius() -> i32 {
    style::SHELL_CONTENT_RADIUS
}

/// Corner radius of free-standing (floating) panel cards.
pub(crate) fn shell_panel_radius() -> i32 {
    style::SHELL_PANEL_RADIUS
}

/// The shell background surrounding the workspace card (theme-dependent) —
/// the backdrop color the webview corner wedges paint outside the arc.
pub(crate) fn shell_window_background() -> u32 {
    style::shell_palette().window_background
}

/// Re-read the Win11 light/dark + system-accent theme into the shell palette
/// cache. Returns `true` when the values changed, so the window proc can
/// repaint only on a real theme change.
pub(crate) fn refresh_system_theme() -> bool {
    theme::refresh()
}

#[cfg(feature = "browser-shell")]
pub(crate) use runtime::handle_menu_bar_surface_action;
pub(crate) use runtime::{open_home_app, set_home_app_id, update_surface_width};

pub fn set_windows_default_shell_tabbar_position(position: WindowsShellTabBarPosition) {
    runtime::set_default_tabbar_position(position);
}

pub fn set_windows_shell_tabbar_position(appid: &str, position: WindowsShellTabBarPosition) {
    runtime::set_tabbar_position(appid, position);
}

#[cfg(feature = "device-frame")]
pub(crate) fn set_windows_shell_tabbar_position_on_window_thread(
    appid: &str,
    position: WindowsShellTabBarPosition,
) {
    runtime::set_tabbar_position_on_window_thread(appid, position);
}

/// The shell window/chrome background color (`0xRRGGBB`), adapting to the
/// system light/dark theme. Runners use it to tint the device frame's rounded
/// screen corners so they blend with the surrounding chrome (status-bar area +
/// tab bar) instead of reading as hard dark wedges.
pub fn windows_shell_background_color() -> u32 {
    style::shell_palette().window_background
}
pub(crate) use terminal_panel::{
    begin_divider_drag, begin_terminal_selection, divider_orientation_at, end_divider_drag,
    end_terminal_selection, scroll_pane_at, update_divider_drag, update_terminal_selection,
};

/// Registers the Windows SDK default shell.
///
/// Must run before the first window is created so hosts get the custom
/// borderless frame and shell event routing.
pub(crate) fn install() {
    chrome::install();
    runtime::install();
}
