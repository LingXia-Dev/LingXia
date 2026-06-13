//! Strongly typed Windows shell chrome layout.

use std::sync::Arc;

use lingxia_webview::platform::windows::lingxia_host::WindowsPanelPosition;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
    pub icon_png: Option<Arc<Vec<u8>>>,
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
    pub group_id: String,
    pub color: u32,
    pub selected_color: u32,
    pub background_color: u32,
    pub border_color: u32,
    pub selected_index: i32,
    pub items: Vec<WindowsShellTabBarItemLayout>,
    pub collapsed: bool,
    pub items_collapsed: bool,
    pub auxiliary_items: Vec<WindowsShellAuxiliaryItemLayout>,
    pub show_auxiliary_add: bool,
    pub header_actions: Vec<WindowsShellHeaderActionLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WindowsShellAddressBarLayout {
    pub visible: bool,
    pub url_text: String,
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
}
