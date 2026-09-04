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

/// The panel card's colors. The terminal owns them — its scheme decides what
/// the card around it looks like — so both the real grid module and its stub
/// hand back the same shape and the painter never reaches past them.
#[derive(Clone, Copy)]
pub(crate) struct PanelChrome {
    pub surface: u32,
    pub header: u32,
    pub separator: u32,
    pub text: u32,
    pub text_muted: u32,
}

#[cfg(feature = "terminal-runtime")]
pub mod terminal_grid;
/// Terminal pane rendering lives behind `terminal-runtime` because it pulls
/// the terminal engine (`lingxia-terminal`). Hosts that enable the shell
/// chrome via `browser-runtime` but do not opt into a terminal (e.g. the
/// device-frame runner) must not carry that dependency, so provide inert
/// stubs for the handful of entry points the panel painter calls.
#[cfg(not(feature = "terminal-runtime"))]
pub mod terminal_grid {
    use windows::Win32::Foundation::RECT;

    pub(super) fn session_surface_background(_session_id: u64) -> Option<u32> {
        None
    }
    /// Never drawn without a terminal, but the painter is compiled either way.
    pub(crate) fn surface_chrome() -> super::PanelChrome {
        super::PanelChrome {
            surface: 0x1e1e1e,
            header: 0x252526,
            separator: 0x333333,
            text: 0xcccccc,
            text_muted: 0x8a8a8a,
        }
    }
    pub(super) fn set_panel_tab_title_rects(
        _panel_id: &str,
        _hwnd: isize,
        _titles: Vec<(u64, RECT)>,
    ) {
    }
}
#[cfg(feature = "terminal-runtime")]
mod terminal_gpu;
#[cfg(feature = "terminal-runtime")]
mod terminal_image_preview;
/// Without the terminal runtime there is no grid to composite, so the panel
/// painter's call compiles away to "GDI keeps it".
#[cfg(not(feature = "terminal-runtime"))]
mod terminal_gpu {
    use windows::Win32::Foundation::{HWND, RECT};

    pub(super) fn present(_: HWND, _: &str, _: RECT, _: [i32; 4], _: Option<(i32, i32)>) {}
}
mod terminal_panel;
pub mod text_input;
mod theme;

pub use chrome::{
    WindowsShellAddressBarLayout, WindowsShellAuxiliaryItemLayout, WindowsShellFooterActionLayout,
    WindowsShellHeaderActionLayout, WindowsShellNavigationBarLayout,
    WindowsShellSidebarActionSource, WindowsShellTabBarItemLayout, WindowsShellTabBarLayout,
    WindowsShellTabBarPosition, WindowsShellWindowLayout,
};
// Inline address-bar editing exists only for self browser tabs.
#[cfg(feature = "browser-runtime")]
pub use chrome::begin_address_edit;

pub(crate) use chrome::shell_chrome_dirty_rects;
pub(crate) use chrome::{
    PHONE_SWITCHER_SHEET_RADIUS, PhoneTabSwitcherHit, PhoneTabSwitcherLayout,
    paint_phone_tab_switcher, paint_shell_notice, phone_tab_click_command, phone_tab_close_command,
    phone_tab_switcher_hit, phone_tab_switcher_layout,
};
pub(crate) use chrome::{
    SIDEBAR_RAIL_TOOLTIP_RADIUS, SIDEBAR_TABBAR_POPUP_RADIUS, bottom_tabbar_rect,
    collapsed_sidebar_tabbar_click_command, collapsed_sidebar_tabbar_popup,
    collapsed_sidebar_tabbar_popup_hit, collapsed_sidebar_tooltip,
    paint_collapsed_sidebar_tabbar_popup, paint_collapsed_sidebar_tooltip,
    paint_transparent_tabbar_overlay, transparent_tabbar_overlay_rect,
};
pub(crate) use chrome::{
    TABBAR_OVERFLOW_PANEL_RADIUS, TabbarOverflowHit, TabbarOverflowLayout, paint_tabbar_overflow,
    tabbar_overflow_hit, tabbar_overflow_layout,
};

pub(crate) use chrome::{workspace_corner_radii, workspace_silhouette_rect};

#[cfg(feature = "device-frame")]
pub(crate) fn draw_icon_from_path(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    path: &str,
    rect: windows::Win32::Foundation::RECT,
    size: u32,
) -> bool {
    chrome::draw_icon_from_path(hdc, path, rect, size)
}

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
#[cfg(feature = "browser-runtime")]
pub(crate) use runtime::open_declared_browser;
#[cfg(feature = "terminal-runtime")]
pub(crate) use runtime::open_declared_terminal;
pub(crate) use runtime::{
    open_home_app_with_target, open_self_browser, prime_lxapp_shell_layout, set_shell_owner_app_id,
    update_surface_width,
};

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

/// Whether Windows apps currently render dark. Runners use it to resolve a
/// "follow system" simulated appearance into the icon actually shown.
pub fn windows_system_dark_mode() -> bool {
    theme::is_dark()
}

pub(crate) fn windows_shell_frame_colors() -> (u32, u32, bool) {
    let palette = style::shell_palette();
    (
        palette.window_background,
        palette.frame_button_icon,
        theme::is_dark(),
    )
}
#[cfg(feature = "terminal-runtime")]
pub(crate) use terminal_panel::install_terminal_automation_authority;
pub(crate) use terminal_panel::{
    begin_divider_drag, begin_pane_drag, begin_terminal_selection, close_pane_at,
    divider_orientation_at, end_divider_drag, end_pane_drag, end_terminal_selection,
    pane_drag_handle_at, pane_hover_rect, scroll_pane_at, update_divider_drag, update_pane_drag,
    update_terminal_selection,
};

#[cfg(feature = "terminal-runtime")]
pub(crate) use terminal_grid::{TerminalImageHit, image_hit_at as terminal_image_hit_at};
#[cfg(feature = "terminal-runtime")]
pub(crate) use terminal_image_preview::show as show_terminal_image_preview;

#[cfg(feature = "terminal-runtime")]
pub(crate) fn terminal_preview_image(
    hit: TerminalImageHit,
) -> Option<terminal_grid::TerminalPreviewImage> {
    terminal_grid::preview_image(hit)
}

/// Registers the Windows SDK default shell.
///
/// Must run before the first window is created so hosts get the custom
/// borderless frame and shell event routing.
pub(crate) fn install() {
    chrome::install();
    runtime::install();
}
