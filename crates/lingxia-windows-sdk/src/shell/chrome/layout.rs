//! Strongly typed Windows shell chrome layout.

use std::sync::Arc;

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
    /// Index in `lxapp.json`, which selection and clicks key on. The shell is
    /// given only the items it shows, so this is not the row's position.
    pub index: usize,
    pub page_path: String,
    pub text: String,
    pub icon_path: String,
    pub badge: Option<String>,
    pub has_red_dot: bool,
}

impl WindowsShellTabBarLayout {
    /// First item the bottom strip folds behind "more". Only the compact
    /// bottom shape folds; a sidebar lists everything it is given.
    pub fn bottom_overflow_start(&self) -> Option<usize> {
        if self.position != WindowsShellTabBarPosition::Bottom {
            return None;
        }
        usize::try_from(self.overflow_start_index)
            .ok()
            .filter(|start| *start < self.items.len())
    }

    /// Cells the bottom strip renders: the direct tabs, plus "more" when it
    /// has anything to hold.
    pub fn bottom_slot_count(&self) -> usize {
        self.bottom_overflow_start()
            .map_or(self.items.len(), |direct| direct + 1)
    }

    /// What one bottom-strip cell stands for.
    ///
    /// `BottomSlot::Tab` is the visible-list position (`items.get`), not the
    /// `lxapp.json` declaration index. Clicks must go through
    /// [`Self::click_index_at_slot`] so `showOn` cannot retarget a tab.
    pub fn bottom_slot(&self, slot: usize) -> Option<BottomSlot> {
        match self.bottom_overflow_start() {
            Some(direct) if slot == direct => Some(BottomSlot::More),
            _ => (slot < self.items.len()).then_some(BottomSlot::Tab(slot)),
        }
    }

    /// Declaration index of the item painted in this visible slot.
    /// `TabBarClick` and selection key on this, not on the slot.
    pub fn click_index_at_slot(&self, slot: usize) -> Option<usize> {
        self.items.get(slot).map(|item| item.index)
    }

    /// Visible-list position of a declared item, if this host shows it.
    pub fn visible_slot_for_item(&self, index: i32) -> Option<usize> {
        self.items
            .iter()
            .position(|item| item.index as i32 == index)
    }

    /// Which strip slot an item paints in. Folded items all share the "more"
    /// slot, so a switch between two of them repaints the same cell.
    /// The cell a declared item paints in, or `None` when this host does not
    /// show it. Folded items all share the "more" cell.
    pub fn bottom_slot_for_item(&self, index: i32) -> Option<usize> {
        let slot = self
            .items
            .iter()
            .position(|item| item.index as i32 == index)?;
        Some(match self.bottom_overflow_start() {
            Some(direct) if slot >= direct => direct,
            _ => slot,
        })
    }

    /// Folded badges still have to surface, so "more" aggregates them.
    pub fn overflow_has_notification(&self) -> bool {
        self.bottom_overflow_start().is_some_and(|start| {
            self.items[start..]
                .iter()
                .any(|item| item.has_red_dot || item.badge.is_some())
        })
    }
}

/// One cell of the compact bottom strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottomSlot {
    /// A tab, by its position in the visible `items` list (not the
    /// `lxapp.json` declaration index — map through
    /// [`WindowsShellTabBarLayout::click_index_at_slot`] for clicks).
    Tab(usize),
    /// The overflow affordance, standing in for every folded item.
    More,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsShellAuxiliaryItemLayout {
    pub id: String,
    pub title: String,
    pub active: bool,
    /// Compact pinned lxapp/web shortcut rendered in the sidebar icon grid.
    pub pinned: bool,
    /// Whether the row exposes the trailing close affordance. Pinned bookmark
    /// shortcuts are independent from open tabs and therefore are not closed.
    pub closable: bool,
    pub icon_png: Option<Arc<Vec<u8>>>,
    /// Absolute icon path (PNG or SVG) used when `icon_png` is absent —
    /// e.g. an open lxapp's own icon. Empty falls back to the LingXia mark.
    pub icon_path: String,
    /// The row's lxapp tab pages, for the collapsed rail's hover panel. Only
    /// the expanded group carries items in the tab bar itself; every other
    /// lxapp row brings its own here.
    pub tabs: Option<WindowsShellAuxiliaryTabs>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsShellAuxiliaryTabs {
    /// Target of a page click; routed like the group's own tab bar.
    pub app_id: String,
    pub items: Vec<WindowsShellTabBarItemLayout>,
    pub selected_index: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsShellHeaderActionLayout {
    pub generation: u64,
    pub id: String,
    pub label: String,
    pub icon_path: String,
    pub disabled: bool,
    pub source: WindowsShellSidebarActionSource,
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
    /// Fully qualified main-switch target emitted when the group row is
    /// clicked, closed, or opened via its context menu.
    pub group_target_id: String,
    /// The lxapp group is the selected main. Browser selection is independent,
    /// so presenting a web tab clears this highlight without collapsing it.
    pub group_active: bool,
    /// Home owns the host and cannot be closed from its group header.
    pub group_closable: bool,
    /// Position of the expanded lxapp group among the unpinned top-level
    /// lxapp/web rows. This keeps the active group in the shared tab order.
    pub group_order_index: usize,
    pub color: u32,
    pub selected_color: u32,
    pub background_color: u32,
    /// Desktop sidebar: fill the expanded-item card only when `tabBar.style`
    /// declared a background and the host is light. `#000000` is a valid fill.
    pub paint_items_background: bool,
    pub background_transparent: bool,
    pub border_color: u32,
    pub selected_index: i32,
    pub items: Vec<WindowsShellTabBarItemLayout>,
    /// First item a compact bottom strip folds behind "more", or -1 when every
    /// item has a slot. Sidebar layouts have room for the list and ignore it.
    pub overflow_start_index: i32,
    /// Sidebar fully hidden (width 0).
    pub collapsed: bool,
    /// Sidebar collapsed to an icon-only rail (the macOS first-collapse
    /// state). Ignored when `collapsed` is set.
    pub icon_rail: bool,
    /// Compact desktop keeps the rail expand glyph visible but rejects the
    /// click so a narrow window cannot persist an expanded sidebar choice.
    pub rail_expand_disabled: bool,
    /// The lxapp explicitly hid its tabbar. Desktop keeps the group and the
    /// surrounding sidebar visible, but removes the child rows and disables
    /// the chevron until `visibility: 'auto'` clears this state.
    pub items_api_hidden: bool,
    pub items_collapsed: bool,
    /// Height reserved at the sidebar bottom for adaptive footer-action rows.
    /// Zero when no footer actions are declared.
    pub footer_action_height: i32,
    /// Pixel offset of the scrollable sidebar navigation region.
    pub main_scroll_offset: i32,
    /// First visual footer-action row rendered inside the capped footer/rail.
    pub footer_action_scroll_row: usize,
    pub auxiliary_items: Vec<WindowsShellAuxiliaryItemLayout>,
    pub show_auxiliary_add: bool,
    pub header_actions: Vec<WindowsShellHeaderActionLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WindowsShellAddressBarLayout {
    pub visible: bool,
    /// Whether browser chrome may dismiss back to underlying host content.
    /// Browser-only containers have nothing to return to and hide this action.
    pub dismissible: bool,
    pub url_text: String,
    /// The presented tab is an API-managed aside. Desktop keeps a read-only
    /// address; compact chrome omits the address and user tab creation.
    pub aside: bool,
    /// Session-history availability of the presented tab; the back/forward
    /// buttons dim while their direction is unavailable.
    pub can_go_back: bool,
    pub can_go_forward: bool,
    /// Whether the current page is stored as a bookmark.
    pub bookmarked: bool,
    /// Whether the current page is pinned as a sidebar shortcut.
    pub pinned: bool,
    /// Whether desktop browser chrome exposes the current-page bookmark action.
    pub show_bookmark: bool,
    /// Whether desktop browser chrome exposes the current-page pin action.
    pub show_pin: bool,
    /// Whether desktop browser chrome exposes the generic page overflow menu.
    pub show_page_menu: bool,
    /// Current page is an http(s) website; the capsule's star/pin buttons
    /// only exist then (internal pages cannot be bookmarked, as on macOS).
    pub web: bool,
    /// Open browser-tab count, shown on the phone bar's tabs button.
    pub tab_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowsShellSidebarActionSource {
    Runtime,
    StaticSettings(crate::static_settings::StaticSettingsDestinationKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsShellFooterActionLayout {
    pub generation: u64,
    pub id: String,
    pub label: String,
    pub icon_path: String,
    pub disabled: bool,
    pub source: WindowsShellSidebarActionSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WindowsShellWindowLayout {
    pub navigation_bar: Option<WindowsShellNavigationBarLayout>,
    pub address_bar: Option<WindowsShellAddressBarLayout>,
    pub tab_bar: Option<WindowsShellTabBarLayout>,
    pub footer_actions: Vec<WindowsShellFooterActionLayout>,
    /// The shared surface graph resolved the shell to Compact, so a presented
    /// browser projects its mobile browser controls at the bottom. This is
    /// explicit instead of re-deriving a breakpoint from physical pixels.
    pub compact_browser_chrome: bool,
    /// Hide the window caption buttons and app-menu icon. Set when the window
    /// is wrapped in a simulator device frame (the runner), whose own toolbar
    /// owns the window controls — the framed screen stays chrome-free.
    pub suppress_window_controls: bool,
    /// Pixels reserved at the top for a device frame's simulated status bar, so
    /// the navigation bar + content sit below it. 0 for un-framed windows.
    pub top_inset: i32,
}

#[cfg(test)]
mod click_index_tests {
    use super::{
        WindowsShellTabBarItemLayout, WindowsShellTabBarLayout, WindowsShellTabBarPosition,
    };

    fn item(index: usize) -> WindowsShellTabBarItemLayout {
        WindowsShellTabBarItemLayout {
            index,
            page_path: format!("page-{index}"),
            text: format!("Tab {index}"),
            icon_path: String::new(),
            badge: None,
            has_red_dot: false,
        }
    }

    fn tabbar(items: Vec<WindowsShellTabBarItemLayout>) -> WindowsShellTabBarLayout {
        WindowsShellTabBarLayout {
            visible: true,
            position: WindowsShellTabBarPosition::Left,
            dimension: 184,
            app_name: "App".to_string(),
            app_icon_path: String::new(),
            group_id: "app".to_string(),
            group_target_id: "lxapp:app".to_string(),
            group_active: true,
            group_closable: false,
            group_order_index: 0,
            color: 0,
            selected_color: 0,
            background_color: 0,
            paint_items_background: false,
            background_transparent: false,
            border_color: 0,
            selected_index: 0,
            overflow_start_index: -1,
            items,
            collapsed: false,
            icon_rail: false,
            rail_expand_disabled: false,
            items_api_hidden: false,
            items_collapsed: false,
            footer_action_height: 0,
            main_scroll_offset: 0,
            footer_action_scroll_row: 0,
            auxiliary_items: Vec::new(),
            show_auxiliary_add: false,
            header_actions: Vec::new(),
        }
    }

    #[test]
    fn click_index_uses_declaration_index_when_show_on_drops_a_slot() {
        // Visible items 0 and 2 — declaration 1 was filtered by showOn.
        let bar = tabbar(vec![item(0), item(2)]);
        assert_eq!(bar.click_index_at_slot(0), Some(0));
        assert_eq!(bar.click_index_at_slot(1), Some(2));
        assert_eq!(bar.click_index_at_slot(2), None);
        assert_eq!(bar.visible_slot_for_item(0), Some(0));
        assert_eq!(bar.visible_slot_for_item(1), None);
        assert_eq!(bar.visible_slot_for_item(2), Some(1));
    }
}
