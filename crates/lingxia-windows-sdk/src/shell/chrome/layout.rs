//! Strongly typed Windows shell chrome layout.

use std::sync::Arc;

use lingxia_windows_contract::WindowsPanelPosition;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum WindowsShellTabBarPosition {
    #[default]
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsShellNavigationBarLayout {
    pub visible: bool,
    pub title: String,
    pub background_color: u32,
    pub text_color: u32,
    pub show_back_button: bool,
    pub show_home_button: bool,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsShellTabBarItemLayout {
    pub page_path: String,
    pub text: String,
    pub icon_path: String,
    pub selected_icon_path: String,
    pub badge: Option<String>,
    pub has_red_dot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsShellAuxiliaryItemLayout {
    pub id: String,
    pub title: String,
    pub active: bool,
    /// Compact pinned-site shortcut rendered in the sidebar icon grid.
    pub pinned: bool,
    /// Whether the row exposes the trailing close affordance. Pinned bookmark
    /// shortcuts are independent from open tabs and therefore are not closed.
    pub closable: bool,
    pub icon_png: Option<Arc<Vec<u8>>>,
    /// Absolute icon path (PNG or SVG) used when `icon_png` is absent —
    /// e.g. an open lxapp's own icon. Empty falls back to the LingXia mark.
    pub icon_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsShellHeaderActionLayout {
    pub id: String,
    pub glyph: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsShellTabBarLayout {
    pub visible: bool,
    pub position: WindowsShellTabBarPosition,
    pub dimension: i32,
    pub app_name: String,
    /// Absolute path to the lxapp's own icon (resolved via the app-info API),
    /// shown in the group header and the icon rail. Empty falls back to the
    /// bundled LingXia mark.
    pub app_icon_path: String,
    pub group_id: String,
    pub color: u32,
    pub selected_color: u32,
    pub background_color: u32,
    pub background_transparent: bool,
    pub border_color: u32,
    pub selected_index: i32,
    pub items: Vec<WindowsShellTabBarItemLayout>,
    /// Sidebar fully hidden (width 0).
    pub collapsed: bool,
    /// Sidebar collapsed to an icon-only rail (the macOS first-collapse
    /// state). Ignored when `collapsed` is set.
    pub icon_rail: bool,
    pub items_collapsed: bool,
    pub auxiliary_items: Vec<WindowsShellAuxiliaryItemLayout>,
    pub show_auxiliary_add: bool,
    pub header_actions: Vec<WindowsShellHeaderActionLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WindowsShellAddressBarLayout {
    pub visible: bool,
    pub url_text: String,
    /// The presented tab is an aside (a compact-degraded URL aside): the top
    /// bar keeps the browser controls but draws no address capsule.
    pub aside: bool,
    /// Session-history availability of the presented tab; the back/forward
    /// buttons dim while their direction is unavailable.
    pub can_go_back: bool,
    pub can_go_forward: bool,
    /// Whether the current page is stored as a bookmark.
    pub bookmarked: bool,
    /// Whether the current page is pinned as a sidebar shortcut.
    pub pinned: bool,
    /// Current page is an http(s) website; the capsule's star/pin buttons
    /// only exist then (internal pages cannot be bookmarked, as on macOS).
    pub web: bool,
    /// Open browser-tab count, shown on the phone bar's tabs button.
    pub tab_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsShellPanelActivatorLayout {
    pub id: String,
    pub label: String,
    pub icon_path: String,
    pub position: WindowsPanelPosition,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WindowsShellWindowLayout {
    pub navigation_bar: Option<WindowsShellNavigationBarLayout>,
    pub address_bar: Option<WindowsShellAddressBarLayout>,
    pub tab_bar: Option<WindowsShellTabBarLayout>,
    pub panel_activators: Vec<WindowsShellPanelActivatorLayout>,
    /// Hide the window caption buttons and app-menu icon. Set when the window
    /// is wrapped in a simulator device frame (the runner), whose own toolbar
    /// owns the window controls — the framed screen stays chrome-free.
    pub suppress_window_controls: bool,
    /// Pixels reserved at the top for a device frame's simulated status bar, so
    /// the navigation bar + content sit below it. 0 for un-framed windows.
    pub top_inset: i32,
}
