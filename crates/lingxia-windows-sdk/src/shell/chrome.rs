//! Shell window chrome: chrome rect computation, product chrome drawing
//! orchestration, and hit-testing.
//!
//! Moved out of `lingxia-webview` so the webview crate stays generic; this
//! file is pure product policy registered through the
//! [`WindowsChromeRenderer`] seam.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(feature = "browser-runtime")]
use lingxia_windows_contract::post_to_window_thread;
use lingxia_windows_contract::{
    WindowsAsidePanelTab, WindowsChromeAttachedLayout, WindowsChromeCommand, WindowsChromeHit,
    WindowsChromePanel, WindowsChromePanelLayout, WindowsChromePanelLayoutInput,
    WindowsChromeRenderer, WindowsChromeState, WindowsFrameButton, WindowsHostPanelContent,
    WindowsPanelPosition, WindowsWindowLayout, aside_panel_tabs, set_windows_chrome_renderer,
};
use serde_json::json;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, DEFAULT_CHARSET, DEFAULT_PITCH, DT_CENTER,
    DT_END_ELLIPSIS, DT_LEFT, DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawTextW, FF_SWISS,
    GetTextFaceW, HDC, HFONT, HGDIOBJ, OUT_DEFAULT_PRECIS, RestoreDC, SaveDC, SelectObject,
    SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging;
use windows::core::{PCWSTR, w};

use super::style::*;

mod drawing;
mod icons;
mod layout;
mod native_panel;
mod sidebar;
mod top_bar;
pub(super) use drawing::*;
pub use layout::*;
use native_panel::*;
use sidebar::*;
#[cfg(feature = "browser-runtime")]
pub use top_bar::begin_address_edit;
use top_bar::*;

/// More (horizontal ellipsis): the app-menu button at the leading edge.
/// Drawn as a subtle monochrome glyph so it sits cohesively in the same
/// caption row as the toggle and sidebar header actions (Arc-style) rather
/// than as a clashing full-color app icon.
pub(super) const GLYPH_APP_MENU: &str = "\u{e712}";

/// ChevronDown: sidebar group header while the group is expanded.
pub(super) const GLYPH_CHEVRON_DOWN: &str = "\u{e70d}";

/// ChevronRight: sidebar group header while the group is collapsed.
pub(super) const GLYPH_CHEVRON_RIGHT: &str = "\u{e76c}";

/// Side length of the square top-bar buttons (sidebar toggle, back/
/// forward/reload).
pub(super) const TOP_BAR_BUTTON_SIZE: i32 = 26;

pub(super) const TOP_BAR_BUTTON_GAP: i32 = 2;

pub(super) const TOP_BAR_PADDING: i32 = 6;

/// Maximum width of the centered URL capsule.
pub(super) const ADDRESS_CAPSULE_MAX_WIDTH: i32 = 520;

pub(super) const ADDRESS_CAPSULE_HEIGHT: i32 = 24;

/// Gap between the nav-button cluster and the URL capsule.
pub(super) const ADDRESS_CAPSULE_NAV_GAP: i32 = 8;

/// Side length of the sidebar group-header chevron hit area.
pub(super) const SIDEBAR_CHEVRON_SIZE: i32 = 18;

/// Side length of the sidebar header action buttons (settings/downloads),
/// and the gap between them.
pub(super) const SIDEBAR_HEADER_ACTION_SIZE: i32 = 22;
pub(super) const SIDEBAR_HEADER_ACTION_GAP: i32 = 4;

pub(super) const SHELL_SIDEBAR_WIDTH: i32 = 180;

/// Width of the icon-only rail (the macOS first-collapse state).
pub(super) const SHELL_SIDEBAR_RAIL_WIDTH: i32 = 44;

/// Column width for a sidebar in its current state: 0 only for auto-hidden
/// legacy/full-hidden state, the rail width when collapsed to icons, else the
/// expanded width.
pub(super) fn sidebar_column_width(tabbar: &WindowsShellTabBarLayout) -> i32 {
    if tabbar.collapsed || tabbar.icon_rail {
        SHELL_SIDEBAR_RAIL_WIDTH
    } else {
        tabbar.dimension.max(SHELL_SIDEBAR_WIDTH)
    }
}

pub(super) const SIDEBAR_HEADER_HEIGHT: i32 = 66;

pub(super) const SIDEBAR_ITEM_HEIGHT: i32 = 34;

pub(super) const SIDEBAR_ITEM_GAP: i32 = 4;

pub(super) const SIDEBAR_ITEM_INSET: i32 = 10;

pub(super) const SIDEBAR_FOOTER_HEIGHT: i32 = 40;

/// Vertical padding above and below the browser-section separator line.
pub(super) const SIDEBAR_BROWSER_SECTION_GAP: i32 = 8;

/// Width of the close-glyph hit area at the trailing edge of a browser row.
pub(super) const SIDEBAR_BROWSER_CLOSE_SIZE: i32 = 22;

/// Close glyph for browser tab rows (multiplication X).
pub(super) const GLYPH_TAB_CLOSE: &str = "\u{2715}";

pub(super) const SIDEBAR_ICON_SIZE: i32 = 16;

pub(super) const SIDEBAR_RAIL_ITEM_SIZE: i32 = 34;

pub(super) const SIDEBAR_RAIL_ICON_SIZE: i32 = 18;

pub(super) const SIDEBAR_TABBAR_POPUP_WIDTH: i32 = 220;
pub(super) const SIDEBAR_TABBAR_POPUP_PADDING: i32 = 8;
/// Corner radius of the collapsed-rail tabbar popup card; the host masks the
/// layered popup window to this same rounding.
pub(crate) const SIDEBAR_TABBAR_POPUP_RADIUS: i32 = 10;

/// Edge length of the favicon drawn on a sidebar browser-tab row.
pub(super) const SIDEBAR_FAVICON_SIZE: i32 = 16;

/// Gap between a browser row's favicon and its title text.
pub(super) const SIDEBAR_FAVICON_TEXT_GAP: i32 = 6;

pub(super) const PANEL_ACTIVATOR_SIZE: i32 = 28;

pub(super) const PANEL_ACTIVATOR_ICON_SIZE: i32 = 16;

pub(super) const PANEL_ACTIVATOR_GAP: i32 = 4;

pub(super) const PANEL_ACTIVATOR_MARGIN: i32 = 6;

pub(super) const BROWSER_PANEL_HEADER_PADDING: i32 = 8;
pub(super) const BROWSER_PANEL_BUTTON_SIZE: i32 = 28;
pub(super) const BROWSER_PANEL_BUTTON_GAP: i32 = 4;

pub(super) const SHELL_TEXT_POINT_SIZE: i32 = 9;

pub(super) const SHELL_TEXT_WEIGHT: i32 = 400;

pub(super) const ATTACHED_PANEL_WIDTH: i32 = 380;

pub(super) const ATTACHED_PANEL_BOTTOM_HEIGHT: i32 = 280;

pub(super) const ATTACHED_PANEL_MIN_SIZE: i32 = 160;

pub(super) const ATTACHED_PANEL_MAX_SIZE: i32 = 700;

pub(super) const ATTACHED_PANEL_HANDLE_SIZE: i32 = 5;

pub(super) const ATTACHED_MAIN_MIN_WIDTH: i32 = 320;

pub(super) const ATTACHED_MAIN_MIN_HEIGHT: i32 = 240;

pub(super) mod command_id {
    pub(super) const TAB_BAR_CLICK: &str = "tabbar.click";
    pub(super) const PANEL_ACTIVATOR_CLICK: &str = "panel-activator.click";
    pub(super) const NAVIGATION_BACK: &str = "navigation.back";
    pub(super) const NAVIGATION_HOME: &str = "navigation.home";
    pub(super) const BROWSER_NEW_TAB: &str = "browser.new-tab";
    pub(super) const BROWSER_TAB_CLICK: &str = "browser.tab.click";
    pub(super) const BROWSER_TAB_CLOSE: &str = "browser.tab.close";
    pub(super) const SIDEBAR_AUXILIARY_CONTEXT_MENU: &str = "sidebar.auxiliary.context-menu";
    pub(super) const BROWSER_PANEL_CLOSE: &str = "browser-panel.close";
    pub(super) const BROWSER_PANEL_NAV_BACK: &str = "browser-panel.nav.back";
    pub(super) const BROWSER_PANEL_NAV_FORWARD: &str = "browser-panel.nav.forward";
    pub(super) const BROWSER_PANEL_NAV_RELOAD: &str = "browser-panel.nav.reload";
    pub(super) const BROWSER_PANEL_ADDRESS_BAR: &str = "browser-panel.address-bar";
    pub(super) const ASIDE_PANEL_TAB_CLICK: &str = "aside-panel.tab.click";
    pub(super) const ASIDE_PANEL_TAB_CLOSE: &str = "aside-panel.tab.close";
    pub(super) const ASIDE_PANEL_CLOSE_ALL: &str = "aside-panel.close-all";
    pub(super) const ASIDE_PANEL_NAV_BACK: &str = "aside-panel.nav.back";
    pub(super) const ASIDE_PANEL_NAV_FORWARD: &str = "aside-panel.nav.forward";
    pub(super) const ASIDE_PANEL_NAV_RELOAD: &str = "aside-panel.nav.reload";
    pub(super) const NATIVE_PANEL_TAB_CLICK: &str = "native-panel.tab.click";
    pub(super) const NATIVE_PANEL_TAB_CLOSE: &str = "native-panel.tab.close";
    pub(super) const NATIVE_PANEL_NEW_TAB: &str = "native-panel.new-tab";
    pub(super) const NATIVE_PANEL_MAXIMIZE: &str = "native-panel.maximize";
    pub(super) const NATIVE_PANEL_TAB_RENAME: &str = "native-panel.tab.rename";
    pub(super) const NATIVE_PANEL_RIGHT_CLICK: &str = "native-panel.right-click";
    pub(super) const NATIVE_PANEL_PANE_FOCUS: &str = "native-panel.pane-focus";
    pub(super) const BROWSER_NAV_BACK: &str = "browser.nav.back";
    pub(super) const BROWSER_NAV_FORWARD: &str = "browser.nav.forward";
    pub(super) const BROWSER_NAV_RELOAD: &str = "browser.nav.reload";
    pub(super) const BROWSER_ADDRESS_BAR: &str = "browser.address-bar";
    pub(super) const BROWSER_CLOSE: &str = "browser.close";
    pub(super) const SIDEBAR_TOGGLE: &str = "sidebar.toggle";
    pub(super) const SIDEBAR_GROUP_TOGGLE: &str = "sidebar.group.toggle";
    pub(super) const SIDEBAR_ACTION: &str = "sidebar.action";
    pub(super) const APP_MENU_CLICK: &str = "app-menu.click";
}

#[derive(Debug, Clone)]
pub(crate) struct CollapsedSidebarTabbarPopup {
    pub(crate) anchor: RECT,
    pub(crate) popup: RECT,
    pub(crate) tabbar: WindowsShellTabBarLayout,
}

pub(super) fn chrome_command(
    id: impl Into<String>,
    payload: serde_json::Value,
) -> WindowsChromeHit {
    WindowsChromeHit::Command(WindowsChromeCommand::new(id).with_payload(payload))
}

pub(super) fn chrome_command_with_context(
    id: impl Into<String>,
    payload: serde_json::Value,
    context_id: impl Into<String>,
    context_payload: serde_json::Value,
) -> WindowsChromeHit {
    WindowsChromeHit::CommandWithContext {
        command: WindowsChromeCommand::new(id).with_payload(payload),
        context_menu: WindowsChromeCommand::new(context_id)
            .with_payload(context_payload)
            .with_screen_position(),
    }
}

/// The shell's window chrome renderer, registered into `lingxia-webview`.
struct ShellChromeRenderer;

impl WindowsChromeRenderer for ShellChromeRenderer {
    fn content_rect(&self, client: RECT, layout: &WindowsWindowLayout) -> RECT {
        let Some(layout) = shell_layout(layout) else {
            return client;
        };
        compute_chrome_rects(client, layout).content
    }

    fn panel_corner_radius(&self) -> i32 {
        SHELL_PANEL_RADIUS
    }

    fn attached_layout(
        &self,
        client: RECT,
        layout: &WindowsWindowLayout,
        panels: &[WindowsChromePanelLayoutInput],
    ) -> Option<WindowsChromeAttachedLayout> {
        let layout = shell_layout(layout)?;
        Some(compute_attached_layout(client, layout, panels))
    }

    fn paint(&self, hdc: HDC, state: &WindowsChromeState) {
        // An active inline text edit (e.g. a terminal tab rename) is a real
        // EDIT child window; the hosts do not use WS_CLIPCHILDREN, so its
        // rect is clipped out to keep chrome repaints from drawing over it.
        let saved = unsafe { SaveDC(hdc) };
        super::text_input::exclude_active_inline_edit(hdc, state.hwnd);
        if let Some(layout) = shell_layout(&state.layout) {
            draw_window_chrome(hdc, state, layout);
        }
        unsafe {
            let _ = RestoreDC(hdc, saved);
        }
    }

    fn paint_region(&self, hdc: HDC, state: &WindowsChromeState, invalid: RECT) {
        if paint_native_panel_region(hdc, state, invalid) {
            return;
        }
        self.paint(hdc, state);
    }

    fn hit_test(&self, state: &WindowsChromeState, point: (i32, i32)) -> Option<WindowsChromeHit> {
        let layout = shell_layout(&state.layout)?;
        chrome_hit_test(state, layout, point)
    }

    fn frame_button_rect(
        &self,
        state: &WindowsChromeState,
        button: WindowsFrameButton,
    ) -> Option<RECT> {
        window_frame_button_rects(state.client)
            .into_iter()
            .find(|(candidate, _)| *candidate == button)
            .map(|(_, rect)| rect)
    }

    fn hover_rect(&self, state: &WindowsChromeState, point: (i32, i32)) -> Option<RECT> {
        let layout = shell_layout(&state.layout)?;
        chrome_hover_rect(state, layout, point)
    }
}

/// Bounding rect of the hover-highlightable element under `point`. Mirrors
/// `chrome_hit_test`'s geometry (minus caption/frame buttons, which track
/// hover separately) so invalidation and painting agree on what lights up.
fn chrome_hover_rect(
    state: &WindowsChromeState,
    layout: &WindowsShellWindowLayout,
    point: (i32, i32),
) -> Option<RECT> {
    let client = state.client;
    let rects = chrome_rects_for_state(state, layout);

    let controls = top_bar_controls(client, rects.top_bar, layout);
    let top_bar_buttons = [
        (!layout.suppress_window_controls)
            .then_some(controls.app_icon)
            .flatten(),
        controls.sidebar_toggle,
        controls.nav_back,
        controls.nav_forward,
        controls.nav_reload,
        controls.browser_close,
    ];
    for rect in top_bar_buttons.into_iter().flatten() {
        if rect_contains(&rect, point) {
            return Some(rect);
        }
    }

    if !address_bar_visible(layout)
        && let (Some(navbar), Some(navbar_rect)) = (&layout.navigation_bar, rects.navigation_bar)
        && rect_contains(&navbar_rect, point)
    {
        let buttons_left = navbar_buttons_left(client, rects.top_bar, layout, navbar_rect);
        if navbar.show_back_button {
            let rect = nav_button_rect(navbar_rect, buttons_left, 0);
            if rect_contains(&rect, point) {
                return Some(rect);
            }
        }
        if navbar.show_home_button {
            let index = if navbar.show_back_button { 1 } else { 0 };
            let rect = nav_button_rect(navbar_rect, buttons_left, index);
            if rect_contains(&rect, point) {
                return Some(rect);
            }
        }
        return None;
    }

    for (_, rect) in panel_activator_rects(client, &rects, layout) {
        if rect_contains(&rect, point) {
            return Some(rect);
        }
    }

    let (tabbar, tabbar_rect) = match (&layout.tab_bar, rects.tab_bar) {
        (Some(tabbar), Some(rect)) if rect_contains(&rect, point) => (tabbar, rect),
        _ => return None,
    };
    if !matches!(
        tabbar.position,
        WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
    ) {
        return None;
    }

    if tabbar.collapsed || tabbar.icon_rail {
        let expand = sidebar_rail_expand_rect(tabbar_rect);
        if rect_contains(&expand, point) {
            return Some(expand);
        }
        for index in 0..=tabbar.auxiliary_items.len() {
            let rect = sidebar_rail_item_rect(tabbar_rect, index);
            if rect_contains(&rect, point) {
                return Some(rect);
            }
        }
        if tabbar.show_auxiliary_add {
            let rect = sidebar_rail_add_rect(tabbar_rect, tabbar);
            if rect_contains(&rect, point) {
                return Some(rect);
            }
        }
        return None;
    }

    let chevron = sidebar_group_chevron_rect(tabbar_rect);
    if rect_contains(&chevron, point) {
        return Some(chevron);
    }
    for (_, rect) in sidebar_header_action_rects(tabbar_rect, tabbar) {
        if rect_contains(&rect, point) {
            return Some(rect);
        }
    }
    if !tabbar.items_collapsed {
        for index in 0..tabbar.items.len() {
            let rect = sidebar_item_rect(tabbar_rect, index);
            if rect_contains(&rect, point) {
                return Some(rect);
            }
        }
    }
    if let Some(auxiliary) = sidebar_auxiliary_rects(tabbar_rect, tabbar) {
        for rect in &auxiliary.items {
            if rect_contains(rect, point) {
                return Some(*rect);
            }
        }
        if let Some(add) = auxiliary.add
            && rect_contains(&add, point)
        {
            return Some(add);
        }
    }
    None
}

pub(super) fn install() {
    set_windows_chrome_renderer(Arc::new(ShellChromeRenderer));
}

fn shell_layout(layout: &WindowsWindowLayout) -> Option<&WindowsShellWindowLayout> {
    layout.downcast_ref::<WindowsShellWindowLayout>()
}

pub(crate) fn shell_chrome_dirty_rects(
    client: RECT,
    old_layout: &WindowsWindowLayout,
    new_layout: &WindowsWindowLayout,
) -> Option<Vec<RECT>> {
    let old_layout = shell_layout(old_layout)?;
    let new_layout = shell_layout(new_layout)?;
    if old_layout == new_layout {
        return Some(Vec::new());
    }

    let old_rects = compute_chrome_rects(client, old_layout);
    let new_rects = compute_chrome_rects(client, new_layout);
    if old_rects.content != new_rects.content
        || old_rects.panel != new_rects.panel
        || old_rects.tab_bar != new_rects.tab_bar
    {
        return None;
    }

    let mut dirty = Vec::new();
    if old_layout.navigation_bar != new_layout.navigation_bar
        || old_layout.address_bar != new_layout.address_bar
    {
        push_dirty_rect(&mut dirty, new_rects.top_bar, client);
    }

    let tabbar_dirty = tabbar_dirty_rects(
        client,
        new_rects.tab_bar,
        old_layout.tab_bar.as_ref(),
        new_layout.tab_bar.as_ref(),
    )?;
    dirty.extend(tabbar_dirty);
    dirty.extend(panel_activator_dirty_rects(
        client, &old_rects, old_layout, new_layout,
    ));

    Some(dirty)
}

fn tabbar_dirty_rects(
    client: RECT,
    tabbar_rect: Option<RECT>,
    old_tabbar: Option<&WindowsShellTabBarLayout>,
    new_tabbar: Option<&WindowsShellTabBarLayout>,
) -> Option<Vec<RECT>> {
    match (old_tabbar, new_tabbar) {
        (None, None) => Some(Vec::new()),
        (Some(_), None) | (None, Some(_)) => None,
        (Some(old_tabbar), Some(new_tabbar)) => {
            if old_tabbar == new_tabbar {
                return Some(Vec::new());
            }
            let rect = tabbar_rect?;
            if tabbar_requires_full_repaint(old_tabbar, new_tabbar) {
                return Some(vec![clip_dirty_rect(rect, client)?]);
            }

            let mut dirty = Vec::new();
            push_tabbar_selected_rects(&mut dirty, client, rect, old_tabbar, new_tabbar);
            push_sidebar_auxiliary_dirty_rects(&mut dirty, client, rect, old_tabbar, new_tabbar);
            Some(dirty)
        }
    }
}

fn tabbar_requires_full_repaint(
    old_tabbar: &WindowsShellTabBarLayout,
    new_tabbar: &WindowsShellTabBarLayout,
) -> bool {
    old_tabbar.visible != new_tabbar.visible
        || old_tabbar.position != new_tabbar.position
        || old_tabbar.dimension != new_tabbar.dimension
        || old_tabbar.app_name != new_tabbar.app_name
        || old_tabbar.group_id != new_tabbar.group_id
        || old_tabbar.color != new_tabbar.color
        || old_tabbar.background_color != new_tabbar.background_color
        || old_tabbar.background_transparent != new_tabbar.background_transparent
        || old_tabbar.border_color != new_tabbar.border_color
        || old_tabbar.items != new_tabbar.items
        || old_tabbar.collapsed != new_tabbar.collapsed
        || old_tabbar.icon_rail != new_tabbar.icon_rail
        || old_tabbar.items_collapsed != new_tabbar.items_collapsed
        || old_tabbar.show_auxiliary_add != new_tabbar.show_auxiliary_add
        || old_tabbar.header_actions != new_tabbar.header_actions
        || !same_auxiliary_row_slots(old_tabbar, new_tabbar)
}

fn same_auxiliary_row_slots(
    old_tabbar: &WindowsShellTabBarLayout,
    new_tabbar: &WindowsShellTabBarLayout,
) -> bool {
    old_tabbar.auxiliary_items.len() == new_tabbar.auxiliary_items.len()
        && old_tabbar
            .auxiliary_items
            .iter()
            .zip(&new_tabbar.auxiliary_items)
            .all(|(old_item, new_item)| old_item.id == new_item.id)
}

fn push_tabbar_selected_rects(
    dirty: &mut Vec<RECT>,
    client: RECT,
    rect: RECT,
    old_tabbar: &WindowsShellTabBarLayout,
    new_tabbar: &WindowsShellTabBarLayout,
) {
    if old_tabbar.selected_index == new_tabbar.selected_index
        && old_tabbar.selected_color == new_tabbar.selected_color
    {
        return;
    }
    for index in [old_tabbar.selected_index, new_tabbar.selected_index] {
        if index < 0 || index as usize >= new_tabbar.items.len() {
            continue;
        }
        let item_rect = if matches!(
            new_tabbar.position,
            WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
        ) {
            sidebar_item_rect(rect, index as usize)
        } else {
            tab_item_rect(
                rect,
                new_tabbar.position,
                new_tabbar.items.len(),
                index as usize,
            )
        };
        push_dirty_rect(dirty, item_rect, client);
    }
}

fn push_sidebar_auxiliary_dirty_rects(
    dirty: &mut Vec<RECT>,
    client: RECT,
    rect: RECT,
    old_tabbar: &WindowsShellTabBarLayout,
    new_tabbar: &WindowsShellTabBarLayout,
) {
    if !matches!(
        new_tabbar.position,
        WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
    ) {
        return;
    }
    let Some(auxiliary) = sidebar_auxiliary_rects(rect, new_tabbar) else {
        return;
    };
    for (index, (old_item, new_item)) in old_tabbar
        .auxiliary_items
        .iter()
        .zip(&new_tabbar.auxiliary_items)
        .enumerate()
    {
        if old_item.active == new_item.active
            && old_item.title == new_item.title
            && old_item.icon_png == new_item.icon_png
            && old_tabbar.selected_color == new_tabbar.selected_color
        {
            continue;
        }
        if let Some(item_rect) = auxiliary.items.get(index).copied() {
            push_dirty_rect(dirty, item_rect, client);
        }
    }
}

fn panel_activator_dirty_rects(
    client: RECT,
    rects: &ChromeRects,
    old_layout: &WindowsShellWindowLayout,
    new_layout: &WindowsShellWindowLayout,
) -> Vec<RECT> {
    if old_layout.panel_activators == new_layout.panel_activators {
        return Vec::new();
    }

    let mut dirty = Vec::new();
    for (_, rect) in panel_activator_rects(client, rects, old_layout)
        .into_iter()
        .chain(panel_activator_rects(client, rects, new_layout))
    {
        push_dirty_rect(&mut dirty, rect, client);
    }
    dirty
}

fn push_dirty_rect(dirty: &mut Vec<RECT>, rect: RECT, client: RECT) {
    let Some(rect) = clip_dirty_rect(rect, client) else {
        return;
    };
    if !dirty.contains(&rect) {
        dirty.push(rect);
    }
}

fn clip_dirty_rect(rect: RECT, client: RECT) -> Option<RECT> {
    let rect = normalize_rect(RECT {
        left: (rect.left - 2).max(client.left),
        top: (rect.top - 2).max(client.top),
        right: (rect.right + 2).min(client.right),
        bottom: (rect.bottom + 2).min(client.bottom),
    });
    if rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        None
    } else {
        Some(rect)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ChromeRects {
    pub(super) content: RECT,
    pub(super) panel: RECT,
    pub(super) top_bar: RECT,
    pub(super) navigation_bar: Option<RECT>,
    pub(super) tab_bar: Option<RECT>,
}

pub(super) fn compute_chrome_rects(client: RECT, layout: &WindowsShellWindowLayout) -> ChromeRects {
    // Reserve the device frame's status-bar strip at the very top; the nav bar +
    // content stack below it (the device frame's status-bar overlay paints the
    // strip's time/signal). 0 for un-framed windows, so the browser shell is
    // unchanged.
    let top_inset = layout.top_inset.max(0);
    let navbar_visible = layout
        .navigation_bar
        .as_ref()
        .is_some_and(|navbar| navbar.visible && navbar.height > 0);
    let mut content = client;
    content.top += top_inset;
    let mut top_bar_left = client.left;
    let mut top_bar_right = client.right;
    let tab_bar = layout
        .tab_bar
        .as_ref()
        .filter(|tabbar| {
            tabbar.visible
                && tabbar.dimension > 0
                && (!tabbar.items.is_empty()
                    || !tabbar.auxiliary_items.is_empty()
                    || tabbar.show_auxiliary_add
                    || !tabbar.header_actions.is_empty())
        })
        .map(|tabbar| match tabbar.position {
            WindowsShellTabBarPosition::Left => {
                // A collapsed sidebar keeps the side-card layout (insets,
                // top bar) at width 0; the top-bar toggle re-expands it.
                let width = sidebar_column_width(tabbar);
                let right = (content.left + width).min(content.right);
                let rect = RECT {
                    left: content.left,
                    top: content.top,
                    right,
                    bottom: content.bottom,
                };
                content.left = right + SHELL_PANEL_PADDING;
                top_bar_left = content.left;
                top_bar_right = content.right;
                content.top = content.top + SHELL_TOP_BAR_HEIGHT + SHELL_PANEL_PADDING;
                content.right -= SHELL_PANEL_PADDING;
                content.bottom -= SHELL_PANEL_PADDING;
                rect
            }
            WindowsShellTabBarPosition::Right => {
                let width = sidebar_column_width(tabbar);
                let left = (content.right - width).max(content.left);
                let rect = RECT {
                    left,
                    top: content.top,
                    right: content.right,
                    bottom: content.bottom,
                };
                content.right = left - SHELL_PANEL_PADDING;
                top_bar_left = content.left + SHELL_PANEL_PADDING;
                top_bar_right = content.right;
                content.top = content.top + SHELL_TOP_BAR_HEIGHT + SHELL_PANEL_PADDING;
                content.left += SHELL_PANEL_PADDING;
                content.bottom -= SHELL_PANEL_PADDING;
                rect
            }
            WindowsShellTabBarPosition::Bottom => {
                let top = (content.bottom - tabbar.dimension).max(content.top);
                let rect = RECT {
                    left: content.left,
                    top,
                    right: content.right,
                    bottom: content.bottom,
                };
                if !tabbar.background_transparent {
                    content.bottom = top;
                }
                rect
            }
        });

    if !matches!(
        layout.tab_bar.as_ref().map(|tabbar| tabbar.position),
        Some(WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right)
    ) {
        // The browser shell (no device frame) always reserves its top bar. A
        // device frame reserves the nav-bar row only when the page shows a nav
        // bar: a plain framed page keeps top_inset for the status bar and no
        // nav row, and an immersive (custom-nav) framed page has top_inset 0
        // AND no nav bar, so it must NOT reserve the row — content bleeds up to
        // the top edge under the transparent status-bar overlay. Distinguish the
        // immersive frame (top_inset 0 + device frame) from the browser shell
        // (top_inset 0, no device frame) via suppress_window_controls.
        if (top_inset == 0 && !layout.suppress_window_controls) || navbar_visible {
            content.top += SHELL_TOP_BAR_HEIGHT;
        }
        top_bar_left = content.left;
        top_bar_right = content.right;
    }

    content = normalize_rect(content);
    let panel = content;
    let top_bar = normalize_rect(RECT {
        left: top_bar_left,
        top: client.top + top_inset,
        right: top_bar_right,
        bottom: (client.top + top_inset + SHELL_TOP_BAR_HEIGHT).min(client.bottom),
    });

    let navigation_bar = layout
        .navigation_bar
        .as_ref()
        .filter(|navbar| navbar.visible && navbar.height > 0)
        .map(|_| top_bar);

    ChromeRects {
        content: normalize_rect(content),
        panel: normalize_rect(panel),
        top_bar,
        navigation_bar: navigation_bar.map(normalize_rect),
        tab_bar: tab_bar.map(normalize_rect),
    }
}

pub(crate) fn transparent_tabbar_overlay_rect(
    client: RECT,
    layout: &WindowsWindowLayout,
) -> Option<RECT> {
    let layout = shell_layout(layout)?;
    let tabbar = layout.tab_bar.as_ref()?;
    if !tabbar.visible
        || !tabbar.background_transparent
        || !matches!(tabbar.position, WindowsShellTabBarPosition::Bottom)
    {
        return None;
    }
    compute_chrome_rects(client, layout).tab_bar
}

pub(crate) fn collapsed_sidebar_tabbar_popup(
    client: RECT,
    layout: &WindowsWindowLayout,
    point: (i32, i32),
) -> Option<CollapsedSidebarTabbarPopup> {
    let layout = shell_layout(layout)?;
    let tabbar = layout.tab_bar.as_ref()?;
    if tabbar.items.is_empty()
        || !matches!(
            tabbar.position,
            WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
        )
        || !(tabbar.collapsed || tabbar.icon_rail)
    {
        return None;
    }
    let tabbar_rect = compute_chrome_rects(client, layout).tab_bar?;
    let anchor = sidebar_rail_item_rect(tabbar_rect, 0);
    if !rect_contains(&anchor, point) {
        return None;
    }
    let (width, height) = collapsed_sidebar_tabbar_popup_size(tabbar);
    let top = anchor.top.min(client.bottom - height).max(client.top);
    let left = match tabbar.position {
        WindowsShellTabBarPosition::Left => tabbar_rect.right - 2,
        WindowsShellTabBarPosition::Right => tabbar_rect.left - width + 2,
        WindowsShellTabBarPosition::Bottom => return None,
    };
    Some(CollapsedSidebarTabbarPopup {
        anchor,
        popup: normalize_rect(RECT {
            left,
            top,
            right: left + width,
            bottom: top + height,
        }),
        tabbar: tabbar.clone(),
    })
}

pub(crate) fn collapsed_sidebar_tabbar_popup_size(tabbar: &WindowsShellTabBarLayout) -> (i32, i32) {
    let rows = tabbar.items.len().max(1) as i32;
    (
        SIDEBAR_TABBAR_POPUP_WIDTH,
        SIDEBAR_TABBAR_POPUP_PADDING * 2
            + rows * SIDEBAR_ITEM_HEIGHT
            + (rows - 1).max(0) * SIDEBAR_ITEM_GAP,
    )
}

pub(crate) fn collapsed_sidebar_tabbar_popup_hit(
    tabbar: &WindowsShellTabBarLayout,
    point: (i32, i32),
) -> Option<usize> {
    let bounds = normalize_rect(RECT {
        left: 0,
        top: 0,
        right: SIDEBAR_TABBAR_POPUP_WIDTH,
        bottom: collapsed_sidebar_tabbar_popup_size(tabbar).1,
    });
    let item_bounds = collapsed_sidebar_tabbar_popup_item_bounds(bounds);
    (0..tabbar.items.len())
        .find(|&index| rect_contains(&sidebar_item_rect(item_bounds, index), point))
}

pub(crate) fn collapsed_sidebar_tabbar_click_command(index: usize) -> WindowsChromeCommand {
    WindowsChromeCommand::new(command_id::TAB_BAR_CLICK).with_payload(json!({ "index": index }))
}

pub(crate) fn paint_collapsed_sidebar_tabbar_popup(
    hdc: HDC,
    tabbar: &WindowsShellTabBarLayout,
    width: i32,
    height: i32,
) {
    let bounds = normalize_rect(RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    });
    let mut popup_tabbar = tabbar.clone();
    popup_tabbar.collapsed = false;
    popup_tabbar.icon_rail = false;
    popup_tabbar.items_collapsed = false;
    popup_tabbar.dimension = width;
    popup_tabbar.header_actions.clear();
    popup_tabbar.auxiliary_items.clear();
    popup_tabbar.show_auxiliary_add = false;
    // The host alpha-masks the layered popup to the rounded shape; fill the
    // full bounds and draw only the hairline outline here.
    fill_rect(hdc, bounds, shell_palette().sidebar_background);
    draw_sidebar_items(
        hdc,
        collapsed_sidebar_tabbar_popup_item_bounds(bounds),
        &popup_tabbar,
        None,
    );
    stroke_round_rect_aa(
        hdc,
        bounds,
        SIDEBAR_TABBAR_POPUP_RADIUS,
        shell_palette().divider,
    );
}

fn collapsed_sidebar_tabbar_popup_item_bounds(bounds: RECT) -> RECT {
    normalize_rect(RECT {
        left: bounds.left,
        top: bounds.top + SIDEBAR_TABBAR_POPUP_PADDING - SIDEBAR_HEADER_HEIGHT,
        right: bounds.right,
        bottom: bounds.bottom,
    })
}

pub(crate) fn paint_transparent_tabbar_overlay(
    hdc: HDC,
    layout: &WindowsWindowLayout,
    width: i32,
    height: i32,
) {
    let Some(layout) = shell_layout(layout) else {
        return;
    };
    let Some(tabbar) = layout.tab_bar.as_ref() else {
        return;
    };
    if !tabbar.visible
        || !tabbar.background_transparent
        || !matches!(tabbar.position, WindowsShellTabBarPosition::Bottom)
    {
        return;
    }
    draw_tab_bar(
        hdc,
        RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        },
        tabbar,
        None,
    );
}

fn compute_attached_layout(
    client: RECT,
    layout: &WindowsShellWindowLayout,
    panels: &[WindowsChromePanelLayoutInput],
) -> WindowsChromeAttachedLayout {
    let mut main = compute_chrome_rects(client, layout).content;
    let mut out = Vec::new();

    let mut ordered = panels.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| attached_panel_order(left).cmp(&attached_panel_order(right)));

    if let Some(&maximized) = ordered.iter().find(|panel| panel.docked && panel.maximized) {
        out.push(WindowsChromePanelLayout {
            panel_id: maximized.panel_id.clone(),
            webtag_key: maximized.webtag_key.clone(),
            rect: shell_maximized_panel_rect(main),
            header_rect: None,
            resize_handle: None,
        });
        main.bottom = main.top;
        return WindowsChromeAttachedLayout { main, panels: out };
    }

    for panel in ordered {
        let (rect, resize_handle) = match panel.position {
            WindowsPanelPosition::Left => {
                let width = attached_panel_size(panel, main, ATTACHED_PANEL_WIDTH);
                let rect = RECT {
                    left: main.left,
                    top: main.top,
                    right: (main.left + width).min(main.right),
                    bottom: main.bottom,
                };
                let handle_width = SHELL_PANEL_PADDING.max(ATTACHED_PANEL_HANDLE_SIZE);
                let handle = normalize_rect(RECT {
                    left: rect.right,
                    top: rect.top,
                    right: (rect.right + handle_width).min(main.right),
                    bottom: rect.bottom,
                });
                main.left = handle.right.min(main.right);
                (rect, Some(handle))
            }
            WindowsPanelPosition::Right => {
                let width = attached_panel_size(panel, main, ATTACHED_PANEL_WIDTH);
                let rect = RECT {
                    left: (main.right - width).max(main.left),
                    top: main.top,
                    right: main.right,
                    bottom: main.bottom,
                };
                let handle_width = SHELL_PANEL_PADDING.max(ATTACHED_PANEL_HANDLE_SIZE);
                let handle = normalize_rect(RECT {
                    left: (rect.left - handle_width).max(main.left),
                    top: rect.top,
                    right: rect.left,
                    bottom: rect.bottom,
                });
                main.right = handle.left.max(main.left);
                (rect, Some(handle))
            }
            WindowsPanelPosition::Top if panel.docked => {
                let height = attached_panel_size(panel, main, ATTACHED_PANEL_BOTTOM_HEIGHT);
                let rect = RECT {
                    left: main.left,
                    top: main.top,
                    right: main.right,
                    bottom: (main.top + height).min(main.bottom),
                };
                let handle = normalize_rect(RECT {
                    left: rect.left,
                    top: (rect.bottom - ATTACHED_PANEL_HANDLE_SIZE).max(rect.top),
                    right: rect.right,
                    bottom: rect.bottom,
                });
                main.top = rect.bottom.min(main.bottom);
                (rect, Some(handle))
            }
            WindowsPanelPosition::Top => {
                let height = attached_panel_size(panel, main, ATTACHED_PANEL_BOTTOM_HEIGHT);
                let rect = RECT {
                    left: main.left,
                    top: main.top,
                    right: main.right,
                    bottom: (main.top + height).min(main.bottom),
                };
                let handle_height = SHELL_PANEL_PADDING.max(ATTACHED_PANEL_HANDLE_SIZE);
                let handle = normalize_rect(RECT {
                    left: rect.left,
                    top: rect.bottom,
                    right: rect.right,
                    bottom: (rect.bottom + handle_height).min(main.bottom),
                });
                main.top = handle.bottom.min(main.bottom);
                (rect, Some(handle))
            }
            WindowsPanelPosition::Bottom if panel.docked => {
                let height = attached_panel_size(panel, main, ATTACHED_PANEL_BOTTOM_HEIGHT);
                let rect = RECT {
                    left: main.left,
                    top: (main.bottom - height).max(main.top),
                    right: main.right,
                    bottom: main.bottom,
                };
                let handle = normalize_rect(RECT {
                    left: rect.left,
                    top: rect.top,
                    right: rect.right,
                    bottom: (rect.top + ATTACHED_PANEL_HANDLE_SIZE).min(rect.bottom),
                });
                main.bottom = rect.top.max(main.top);
                (rect, Some(handle))
            }
            WindowsPanelPosition::Bottom => {
                let height = attached_panel_size(panel, main, ATTACHED_PANEL_BOTTOM_HEIGHT);
                let rect = RECT {
                    left: main.left,
                    top: (main.bottom - height).max(main.top),
                    right: main.right,
                    bottom: main.bottom,
                };
                let handle_height = SHELL_PANEL_PADDING.max(ATTACHED_PANEL_HANDLE_SIZE);
                let handle = normalize_rect(RECT {
                    left: rect.left,
                    top: (rect.top - handle_height).max(main.top),
                    right: rect.right,
                    bottom: rect.top,
                });
                main.bottom = handle.top.max(main.top);
                (rect, Some(handle))
            }
        };

        // A browser aside hoists its chrome (address bar, or the aside tab
        // strip) into the shared top band so it lands on the same baseline as
        // the main lxapp navbar (which is clipped to `attached.main`). The
        // webview then fills the whole panel rect, top-aligned with the main
        // content card. Side panels only: terminal asides (top/bottom) keep
        // their own in-panel header.
        let header_rect = (matches!(
            panel.position,
            WindowsPanelPosition::Left | WindowsPanelPosition::Right
        ) && (panel.webtag_key.starts_with("app.lingxia.browser:")
            || !aside_panel_tabs(&panel.panel_id).is_empty()))
        .then(|| browser_panel_band_header_rect(client, rect));

        out.push(WindowsChromePanelLayout {
            panel_id: panel.panel_id.clone(),
            webtag_key: panel.webtag_key.clone(),
            rect: normalize_rect(rect),
            header_rect,
            resize_handle,
        });
    }

    WindowsChromeAttachedLayout {
        main: normalize_rect(main),
        panels: out,
    }
}

/// The top-band slice (aligned with the main navbar baseline) over a side
/// panel's column. The trailing edge reserves room for the window frame
/// buttons so the address bar's controls never run under the caption.
fn browser_panel_band_header_rect(client: RECT, panel_rect: RECT) -> RECT {
    let right = panel_rect
        .right
        .min(client.right - window_frame_buttons_width() - TOP_BAR_PADDING);
    normalize_rect(RECT {
        left: panel_rect.left,
        top: client.top,
        right,
        bottom: (client.top + SHELL_TOP_BAR_HEIGHT).min(client.bottom),
    })
}

fn attached_panel_order(panel: &WindowsChromePanelLayoutInput) -> (u8, u8, &str) {
    (
        match panel.position {
            WindowsPanelPosition::Left | WindowsPanelPosition::Right => 0,
            WindowsPanelPosition::Top | WindowsPanelPosition::Bottom => 1,
        },
        match panel.position {
            WindowsPanelPosition::Left => 0,
            WindowsPanelPosition::Right => 1,
            WindowsPanelPosition::Top => 2,
            WindowsPanelPosition::Bottom => 3,
        },
        panel.panel_id.as_str(),
    )
}

fn attached_panel_size(
    panel: &WindowsChromePanelLayoutInput,
    content: RECT,
    default_size: i32,
) -> i32 {
    let requested = panel.requested_size.unwrap_or(default_size).max(1);
    let available = match panel.position {
        WindowsPanelPosition::Top | WindowsPanelPosition::Bottom => rect_height(&content),
        WindowsPanelPosition::Left | WindowsPanelPosition::Right => rect_width(&content),
    };
    if available <= 0 {
        return 0;
    }

    let max_with_main = match panel.position {
        WindowsPanelPosition::Top | WindowsPanelPosition::Bottom => {
            available - SHELL_PANEL_PADDING - ATTACHED_MAIN_MIN_HEIGHT
        }
        WindowsPanelPosition::Left | WindowsPanelPosition::Right => {
            available - SHELL_PANEL_PADDING - ATTACHED_MAIN_MIN_WIDTH
        }
    };
    let max_size = if max_with_main > 0 {
        max_with_main
    } else {
        available / 2
    }
    .min(ATTACHED_PANEL_MAX_SIZE)
    .min(available)
    .max(1);
    let min_size = ATTACHED_PANEL_MIN_SIZE.min(max_size);
    requested.clamp(min_size, max_size)
}

fn shell_maximized_panel_rect(content: RECT) -> RECT {
    normalize_rect(content)
}

/// Chrome rects for a concrete window state: when the host has attached
/// panels, the navigation bar is clipped to the main content card.
pub(super) fn chrome_rects_for_state(
    state: &WindowsChromeState,
    layout: &WindowsShellWindowLayout,
) -> ChromeRects {
    let mut rects = compute_chrome_rects(state.client, layout);
    if rects.navigation_bar.is_some()
        && let Some(attached) = &state.attached
    {
        rects.navigation_bar = Some(normalize_rect(RECT {
            left: attached.main.left,
            top: rects.top_bar.top,
            right: attached.main.right,
            bottom: rects.top_bar.bottom,
        }));
    }
    rects
}

pub(super) fn draw_window_chrome(
    hdc: HDC,
    state: &WindowsChromeState,
    layout: &WindowsShellWindowLayout,
) {
    let client = state.client;
    let rects = chrome_rects_for_state(state, layout);

    fill_rect(hdc, client, shell_palette().window_background);
    draw_shell_top_bar(hdc, &rects);
    draw_content_cards(hdc, state, &rects);

    // The address bar owns the top bar while a browser surface is
    // presented; the lxapp navigation bar yields.
    if !address_bar_visible(layout)
        && let (Some(navbar), Some(navbar_rect)) = (&layout.navigation_bar, rects.navigation_bar)
    {
        let buttons_left = navbar_buttons_left(client, rects.top_bar, layout, navbar_rect);
        draw_navigation_bar(
            hdc,
            navbar_rect,
            buttons_left,
            navbar,
            layout.suppress_window_controls,
            state.cursor,
        );
    }
    if let (Some(tabbar), Some(tabbar_rect)) = (&layout.tab_bar, rects.tab_bar) {
        draw_tab_bar(hdc, tabbar_rect, tabbar, state.cursor);
    }
    // Painted after the navigation bar: the navbar fills the whole top bar
    // with its own background, and the toggle/address controls sit on top.
    draw_top_bar_controls(hdc, state, &rects, layout);
    draw_panel_activators(hdc, client, &rects, layout, state.cursor);
    draw_maximized_native_panels(hdc, state);
    // A device-framed window's caption lives on the simulator toolbar, not the
    // shell screen.
    if !layout.suppress_window_controls {
        draw_window_frame_buttons(hdc, state);
    }
}

fn paint_native_panel_region(hdc: HDC, state: &WindowsChromeState, invalid: RECT) -> bool {
    if rect_width(&invalid) == 0 || rect_height(&invalid) == 0 {
        return false;
    }
    let Some(attached) = &state.attached else {
        return false;
    };
    let Some(panel) = attached.panels.iter().find(|panel| {
        panel.host_content.is_some()
            && rect_contains_rect(&panel.rect, &invalid)
            && rects_intersect(&panel.rect, &invalid)
    }) else {
        return false;
    };
    draw_native_panel_content(hdc, state.hwnd, panel);
    true
}

pub(super) fn chrome_hit_test(
    state: &WindowsChromeState,
    layout: &WindowsShellWindowLayout,
    point: (i32, i32),
) -> Option<WindowsChromeHit> {
    let client = state.client;
    let rects = chrome_rects_for_state(state, layout);

    // Device-framed screens have no shell caption / app-menu (the simulator
    // toolbar owns them), so skip those hit regions entirely.
    if !layout.suppress_window_controls
        && let Some((button, _)) = window_frame_button_rects(client)
            .into_iter()
            .find(|(_, rect)| rect_contains(rect, point))
    {
        return Some(WindowsChromeHit::FrameButton(button));
    }

    if let Some(hit) = maximized_native_panel_hit(state, point) {
        return Some(hit);
    }

    let controls = top_bar_controls(client, rects.top_bar, layout);
    if !layout.suppress_window_controls
        && let Some(app_icon) = controls.app_icon
        && rect_contains(&app_icon, point)
    {
        // Carries the click's screen position so the runtime can anchor the
        // About/Exit popup menu under the icon.
        return Some(WindowsChromeHit::Command(
            WindowsChromeCommand::new(command_id::APP_MENU_CLICK).with_screen_position(),
        ));
    }
    if let Some(toggle) = controls.sidebar_toggle
        && rect_contains(&toggle, point)
    {
        return Some(chrome_command(command_id::SIDEBAR_TOGGLE, json!({})));
    }
    if let Some(back) = controls.nav_back
        && rect_contains(&back, point)
    {
        return Some(chrome_command(command_id::BROWSER_NAV_BACK, json!({})));
    }
    if let Some(forward) = controls.nav_forward
        && rect_contains(&forward, point)
    {
        return Some(chrome_command(command_id::BROWSER_NAV_FORWARD, json!({})));
    }
    if let Some(reload) = controls.nav_reload
        && rect_contains(&reload, point)
    {
        return Some(chrome_command(command_id::BROWSER_NAV_RELOAD, json!({})));
    }
    if let Some(address) = controls.address
        && rect_contains(&address, point)
    {
        return Some(chrome_command(command_id::BROWSER_ADDRESS_BAR, json!({})));
    }
    if let Some(close) = controls.browser_close
        && rect_contains(&close, point)
    {
        return Some(chrome_command(command_id::BROWSER_CLOSE, json!({})));
    }

    if !address_bar_visible(layout)
        && let (Some(navbar), Some(navbar_rect)) = (&layout.navigation_bar, rects.navigation_bar)
        && rect_contains(&navbar_rect, point)
    {
        let buttons_left = navbar_buttons_left(client, rects.top_bar, layout, navbar_rect);
        if navbar.show_back_button
            && rect_contains(&nav_button_rect(navbar_rect, buttons_left, 0), point)
        {
            return Some(chrome_command(command_id::NAVIGATION_BACK, json!({})));
        }
        let home_index = if navbar.show_back_button { 1 } else { 0 };
        if navbar.show_home_button
            && rect_contains(
                &nav_button_rect(navbar_rect, buttons_left, home_index),
                point,
            )
        {
            return Some(chrome_command(command_id::NAVIGATION_HOME, json!({})));
        }
        return Some(WindowsChromeHit::Caption);
    }

    if let Some(hit) = browser_panel_hit_test(state, point) {
        return Some(hit);
    }

    for (panel_id, rect) in panel_activator_rects(client, &rects, layout) {
        if rect_contains(&rect, point) {
            return Some(chrome_command(
                command_id::PANEL_ACTIVATOR_CLICK,
                json!({ "panel_id": panel_id }),
            ));
        }
    }

    if let (Some(tabbar), Some(tabbar_rect)) = (&layout.tab_bar, rects.tab_bar)
        && rect_contains(&tabbar_rect, point)
    {
        let sidebar = matches!(
            tabbar.position,
            WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
        );
        if sidebar && (tabbar.collapsed || tabbar.icon_rail) {
            if rect_contains(&sidebar_rail_expand_rect(tabbar_rect), point) {
                return Some(chrome_command(command_id::SIDEBAR_TOGGLE, json!({})));
            }
            if rect_contains(&sidebar_rail_item_rect(tabbar_rect, 0), point) {
                let index = tabbar.selected_index.max(0) as usize;
                return Some(chrome_command(
                    command_id::TAB_BAR_CLICK,
                    json!({ "index": index }),
                ));
            }
            for (index, item) in tabbar.auxiliary_items.iter().enumerate() {
                if rect_contains(&sidebar_rail_item_rect(tabbar_rect, 1 + index), point) {
                    let payload = json!({ "tab_id": item.id.clone() });
                    return Some(chrome_command_with_context(
                        command_id::BROWSER_TAB_CLICK,
                        payload.clone(),
                        command_id::SIDEBAR_AUXILIARY_CONTEXT_MENU,
                        payload,
                    ));
                }
            }
            if tabbar.show_auxiliary_add
                && rect_contains(&sidebar_rail_add_rect(tabbar_rect, tabbar), point)
            {
                return Some(chrome_command(command_id::BROWSER_NEW_TAB, json!({})));
            }
            return Some(WindowsChromeHit::Chrome);
        }
        if sidebar {
            if rect_contains(&sidebar_group_chevron_rect(tabbar_rect), point) {
                return Some(chrome_command(
                    command_id::SIDEBAR_GROUP_TOGGLE,
                    json!({ "group": tabbar.group_id.clone() }),
                ));
            }
            for (action_id, action_rect) in sidebar_header_action_rects(tabbar_rect, tabbar) {
                if rect_contains(&action_rect, point) {
                    return Some(chrome_command(
                        command_id::SIDEBAR_ACTION,
                        json!({ "action_id": action_id }),
                    ));
                }
            }
        }
        if !(sidebar && tabbar.items_collapsed) {
            for index in 0..tabbar.items.len() {
                let item_rect = if sidebar {
                    sidebar_item_rect(tabbar_rect, index)
                } else {
                    tab_item_rect(tabbar_rect, tabbar.position, tabbar.items.len(), index)
                };
                if rect_contains(&item_rect, point) {
                    return Some(chrome_command(
                        command_id::TAB_BAR_CLICK,
                        json!({ "index": index }),
                    ));
                }
            }
        }
        if sidebar && let Some(hit) = sidebar_auxiliary_hit_test(tabbar_rect, tabbar, point) {
            return Some(hit);
        }
        return Some(WindowsChromeHit::Chrome);
    }

    if let Some(hit) = native_panel_hit(state, point) {
        return Some(hit);
    }

    if rect_contains(&rects.top_bar, point) {
        return Some(WindowsChromeHit::Caption);
    }

    None
}

fn maximized_native_panel_hit(
    state: &WindowsChromeState,
    point: (i32, i32),
) -> Option<WindowsChromeHit> {
    native_panel_hit_by(state, point, panel_is_maximized)
}

fn native_panel_hit(state: &WindowsChromeState, point: (i32, i32)) -> Option<WindowsChromeHit> {
    native_panel_hit_by(state, point, |_| true)
}

fn native_panel_hit_by(
    state: &WindowsChromeState,
    point: (i32, i32),
    include: impl Fn(&WindowsChromePanel) -> bool,
) -> Option<WindowsChromeHit> {
    let attached = state.attached.as_ref()?;
    for panel in &attached.panels {
        if panel.host_content.is_some() && include(panel) && rect_contains(&panel.rect, point) {
            // Header elements (tabs, new-tab, maximize) win over the
            // generic focus hit; the rest of the panel focuses it.
            if let Some(hit) = terminal_header_hit_test(panel, point) {
                return Some(hit);
            }
            return Some(WindowsChromeHit::Focusable {
                id: panel.panel_id.clone(),
                context_menu: Some(
                    WindowsChromeCommand::new(command_id::NATIVE_PANEL_RIGHT_CLICK)
                        .with_payload(json!({ "panel_id": panel.panel_id.clone() }))
                        .with_screen_position(),
                ),
                // Left-clicking the body focuses the pane under the cursor.
                click_command: Some(
                    WindowsChromeCommand::new(command_id::NATIVE_PANEL_PANE_FOCUS)
                        .with_payload(json!({ "panel_id": panel.panel_id.clone() }))
                        .with_screen_position(),
                ),
            });
        }
    }
    None
}

fn browser_panel_hit_test(
    state: &WindowsChromeState,
    point: (i32, i32),
) -> Option<WindowsChromeHit> {
    let attached = state.attached.as_ref()?;
    for panel in &attached.panels {
        if !browser_panel_header_visible(panel)
            || !rect_contains(&browser_panel_header_rect(panel), point)
        {
            continue;
        }
        let tabs = panel_aside_tabs(panel);
        if !tabs.is_empty() {
            return Some(aside_panel_header_hit(panel, &tabs, point));
        }
        for (command, rect) in browser_panel_nav_button_rects(panel) {
            if rect_contains(&rect, point) {
                return Some(chrome_command(
                    command,
                    json!({ "webtag_key": panel.webtag_key.clone() }),
                ));
            }
        }
        if rect_contains(&browser_panel_close_rect(panel), point) {
            return Some(chrome_command(
                command_id::BROWSER_PANEL_CLOSE,
                json!({ "panel_id": panel.panel_id.clone() }),
            ));
        }
        if rect_contains(&browser_panel_address_rect(panel), point) {
            return Some(chrome_command(
                command_id::BROWSER_PANEL_ADDRESS_BAR,
                json!({ "webtag_key": panel.webtag_key.clone() }),
            ));
        }
        return Some(WindowsChromeHit::Chrome);
    }
    None
}

/// The aside browser panel's API-only chrome: nav cluster, tab strip, and
/// the close-all button. No address bar and no new-tab affordance (tabs are
/// opened only through `lx.openSurface`).
fn aside_panel_header_hit(
    panel: &WindowsChromePanel,
    tabs: &[WindowsAsidePanelTab],
    point: (i32, i32),
) -> WindowsChromeHit {
    for (command, rect) in aside_panel_nav_button_rects(panel) {
        if rect_contains(&rect, point) {
            return chrome_command(command, json!({ "panel_id": panel.panel_id.clone() }));
        }
    }
    if rect_contains(&browser_panel_close_rect(panel), point) {
        return chrome_command(
            command_id::ASIDE_PANEL_CLOSE_ALL,
            json!({ "panel_id": panel.panel_id.clone() }),
        );
    }
    for (tab, rect) in tabs.iter().zip(aside_panel_tab_rects(panel, tabs.len())) {
        if let Some(close) = aside_panel_tab_close_rect(rect)
            && rect_contains(&close, point)
        {
            return chrome_command(
                command_id::ASIDE_PANEL_TAB_CLOSE,
                json!({ "surface_id": tab.surface_id.clone() }),
            );
        }
        if rect_contains(&rect, point) {
            return chrome_command(
                command_id::ASIDE_PANEL_TAB_CLICK,
                json!({ "surface_id": tab.surface_id.clone() }),
            );
        }
    }
    WindowsChromeHit::Chrome
}

fn panel_aside_tabs(panel: &WindowsChromePanel) -> Vec<WindowsAsidePanelTab> {
    if panel.host_content.is_some() {
        return Vec::new();
    }
    aside_panel_tabs(&panel.panel_id)
}

fn browser_panel_header_visible(panel: &WindowsChromePanel) -> bool {
    panel.host_content.is_none()
        && (panel.webtag_key.starts_with("app.lingxia.browser:")
            || !aside_panel_tabs(&panel.panel_id).is_empty())
}

fn browser_panel_header_rect(panel: &WindowsChromePanel) -> RECT {
    // The address bar lives in the shared top band (computed at layout time);
    // an empty rect makes the draw/hit-test paths no-op for panels without one.
    panel.header_rect.map(normalize_rect).unwrap_or_default()
}

fn browser_panel_close_rect(panel: &WindowsChromePanel) -> RECT {
    let header = browser_panel_header_rect(panel);
    normalize_rect(RECT {
        left: (header.right - BROWSER_PANEL_BUTTON_SIZE - BROWSER_PANEL_HEADER_PADDING)
            .max(header.left),
        top: header.top + (rect_height(&header) - BROWSER_PANEL_BUTTON_SIZE) / 2,
        right: header.right - BROWSER_PANEL_HEADER_PADDING,
        bottom: header.top + (rect_height(&header) + BROWSER_PANEL_BUTTON_SIZE) / 2,
    })
}

fn browser_panel_nav_button_rects(panel: &WindowsChromePanel) -> [(&'static str, RECT); 3] {
    let header = browser_panel_header_rect(panel);
    let top = header.top + (rect_height(&header) - BROWSER_PANEL_BUTTON_SIZE) / 2;
    let mut left = header.left + BROWSER_PANEL_HEADER_PADDING;
    let mut next = || {
        let rect = normalize_rect(RECT {
            left,
            top,
            right: left + BROWSER_PANEL_BUTTON_SIZE,
            bottom: top + BROWSER_PANEL_BUTTON_SIZE,
        });
        left += BROWSER_PANEL_BUTTON_SIZE + BROWSER_PANEL_BUTTON_GAP;
        rect
    };
    [
        (command_id::BROWSER_PANEL_NAV_BACK, next()),
        (command_id::BROWSER_PANEL_NAV_FORWARD, next()),
        (command_id::BROWSER_PANEL_NAV_RELOAD, next()),
    ]
}

/// Same nav-cluster geometry, aside-panel command ids (routed to the surface
/// layer rather than the in-app browser).
fn aside_panel_nav_button_rects(panel: &WindowsChromePanel) -> [(&'static str, RECT); 3] {
    let [(_, back), (_, forward), (_, reload)] = browser_panel_nav_button_rects(panel);
    [
        (command_id::ASIDE_PANEL_NAV_BACK, back),
        (command_id::ASIDE_PANEL_NAV_FORWARD, forward),
        (command_id::ASIDE_PANEL_NAV_RELOAD, reload),
    ]
}

const ASIDE_PANEL_TAB_MAX_WIDTH: i32 = 190;
const ASIDE_PANEL_TAB_GAP: i32 = 4;
const ASIDE_PANEL_TAB_CLOSE_WIDTH: i32 = 20;
const ASIDE_PANEL_TAB_INSET: i32 = 5;

/// Tab rects of the aside panel's strip, index-aligned with the registered
/// tabs: equal widths (capped) between the nav cluster and close-all.
fn aside_panel_tab_rects(panel: &WindowsChromePanel, count: usize) -> Vec<RECT> {
    if count == 0 {
        return Vec::new();
    }
    let header = browser_panel_header_rect(panel);
    let nav_right = aside_panel_nav_button_rects(panel)[2].1.right;
    let left_edge = nav_right + BROWSER_PANEL_HEADER_PADDING;
    let right_edge = browser_panel_close_rect(panel).left - BROWSER_PANEL_HEADER_PADDING;
    let count_i32 = count as i32;
    let avail = (right_edge - left_edge - (count_i32 - 1) * ASIDE_PANEL_TAB_GAP).max(0);
    let width = (avail / count_i32).clamp(24, ASIDE_PANEL_TAB_MAX_WIDTH);
    let mut out = Vec::with_capacity(count);
    let mut left = left_edge;
    for _ in 0..count {
        out.push(normalize_rect(RECT {
            left,
            top: header.top + ASIDE_PANEL_TAB_INSET,
            right: (left + width).min(right_edge),
            bottom: header.bottom - ASIDE_PANEL_TAB_INSET,
        }));
        left += width + ASIDE_PANEL_TAB_GAP;
    }
    out
}

/// Close-glyph rect at a tab's trailing edge; dropped on tabs too narrow to
/// keep a readable title next to it.
fn aside_panel_tab_close_rect(tab: RECT) -> Option<RECT> {
    (rect_width(&tab) >= 3 * ASIDE_PANEL_TAB_CLOSE_WIDTH).then(|| {
        normalize_rect(RECT {
            left: tab.right - ASIDE_PANEL_TAB_CLOSE_WIDTH,
            top: tab.top,
            right: tab.right,
            bottom: tab.bottom,
        })
    })
}

/// The URL capsule rect inside a browser aside's header (between the nav
/// cluster and the close button). Shared by the painter and hit-test so the
/// inline editor lands exactly on the painted pill.
fn browser_panel_address_rect(panel: &WindowsChromePanel) -> RECT {
    let header = browser_panel_header_rect(panel);
    let close = browser_panel_close_rect(panel);
    let address_left = browser_panel_nav_button_rects(panel)
        .last()
        .map(|(_, rect)| rect.right + BROWSER_PANEL_HEADER_PADDING)
        .unwrap_or(header.left + BROWSER_PANEL_HEADER_PADDING);
    normalize_rect(RECT {
        left: address_left,
        top: header.top + 8,
        right: close.left - BROWSER_PANEL_HEADER_PADDING,
        bottom: header.bottom - 8,
    })
}

/// Last painted URL-capsule rect for a browser aside, keyed by host window +
/// webtag, so a click can start an inline edit over the exact pill (mirrors the
/// main top bar's `ADDRESS_CAPSULE_RECTS`).
static PANEL_ADDRESS_RECTS: OnceLock<Mutex<HashMap<(isize, String), RECT>>> = OnceLock::new();

fn remember_panel_address_rect(hwnd: HWND, webtag_key: &str, rect: Option<RECT>) {
    let map = PANEL_ADDRESS_RECTS.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut map) = map.lock() else {
        return;
    };
    let key = (hwnd.0 as isize, webtag_key.to_string());
    match rect {
        Some(rect) => {
            map.insert(key, rect);
        }
        None => {
            map.remove(&key);
        }
    }
}

/// Starts an inline URL edit over a browser aside's address capsule, prefilled
/// with `initial_text`. Returns `false` when no capsule was painted for
/// `(window, webtag_key)`. Mirrors [`top_bar::begin_address_edit`] for the aside.
#[cfg(feature = "browser-runtime")]
pub fn begin_panel_address_edit(
    window: isize,
    webtag_key: &str,
    initial_text: &str,
    on_commit: super::text_input::InlineEditCommit,
) -> bool {
    let rect = PANEL_ADDRESS_RECTS
        .get()
        .and_then(|map| map.lock().ok())
        .and_then(|map| map.get(&(window, webtag_key.to_string())).copied());
    let Some(rect) = rect else {
        return false;
    };
    let edit_rect = inset_rect(rect, 10, 1);
    if rect_width(&edit_rect) == 0 || rect_height(&edit_rect) == 0 {
        return false;
    }
    let initial = initial_text.to_string();
    post_to_window_thread(
        window,
        Box::new(move || {
            super::text_input::begin_inline_edit(
                HWND(window as *mut core::ffi::c_void),
                edit_rect,
                &initial,
                on_commit,
            );
        }),
    )
}

fn draw_browser_panel_header(hdc: HDC, hwnd: HWND, panel: &WindowsChromePanel) {
    let header = browser_panel_header_rect(panel);
    if rect_width(&header) == 0 || rect_height(&header) == 0 {
        remember_panel_address_rect(hwnd, &panel.webtag_key, None);
        return;
    }
    let pal = shell_palette();

    fill_rect(hdc, header, pal.panel_background);
    fill_rect(
        hdc,
        RECT {
            left: header.left,
            top: header.bottom - 1,
            right: header.right,
            bottom: header.bottom,
        },
        pal.divider,
    );

    let tabs = panel_aside_tabs(panel);
    if !tabs.is_empty() {
        remember_panel_address_rect(hwnd, &panel.webtag_key, None);
        draw_aside_panel_header(hdc, panel, &tabs);
        return;
    }

    let close = browser_panel_close_rect(panel);
    for (command, rect) in browser_panel_nav_button_rects(panel) {
        let glyph = match command {
            command_id::BROWSER_PANEL_NAV_BACK => GLYPH_NAV_BACK,
            command_id::BROWSER_PANEL_NAV_FORWARD => GLYPH_NAV_FORWARD,
            command_id::BROWSER_PANEL_NAV_RELOAD => GLYPH_NAV_RELOAD,
            _ => "",
        };
        draw_frame_button_glyph(hdc, glyph, rect, pal.text_muted);
    }

    let address = browser_panel_address_rect(panel);
    let address_visible = rect_width(&address) > 0 && rect_height(&address) > 0;
    if address_visible {
        fill_round_rect_aa(hdc, address, 10, pal.control_surface);
        draw_text(
            hdc,
            browser_panel_title(panel).as_str(),
            inset_rect(address, 10, 0),
            pal.text_muted,
            DT_LEFT,
        );
    }
    // Record the painted pill so a click can open the inline editor over it.
    remember_panel_address_rect(hwnd, &panel.webtag_key, address_visible.then_some(address));

    draw_frame_button_glyph(hdc, GLYPH_CLOSE, close, pal.text_muted);
}

fn browser_panel_title(panel: &WindowsChromePanel) -> String {
    let title = panel.title.trim();
    if title.is_empty() {
        "Browser".to_string()
    } else {
        title.to_string()
    }
}

/// The aside browser panel's API-only chrome row: back/forward/reload, the
/// title tab strip, and close-all. No address bar and no "+" - tabs come
/// only from `lx.openSurface`.
fn draw_aside_panel_header(hdc: HDC, panel: &WindowsChromePanel, tabs: &[WindowsAsidePanelTab]) {
    let pal = shell_palette();
    for (command, rect) in aside_panel_nav_button_rects(panel) {
        let glyph = match command {
            command_id::ASIDE_PANEL_NAV_BACK => GLYPH_NAV_BACK,
            command_id::ASIDE_PANEL_NAV_FORWARD => GLYPH_NAV_FORWARD,
            command_id::ASIDE_PANEL_NAV_RELOAD => GLYPH_NAV_RELOAD,
            _ => "",
        };
        draw_frame_button_glyph(hdc, glyph, rect, pal.text_muted);
    }

    for (tab, rect) in tabs.iter().zip(aside_panel_tab_rects(panel, tabs.len())) {
        if tab.active {
            fill_round_rect_aa(hdc, rect, 6, pal.control_surface);
        }
        let close = aside_panel_tab_close_rect(rect);
        let title_rect = normalize_rect(RECT {
            left: rect.left + 8,
            top: rect.top,
            right: close.map(|close| close.left).unwrap_or(rect.right - 6),
            bottom: rect.bottom,
        });
        let text_color = if tab.active {
            pal.text_primary
        } else {
            pal.text_muted
        };
        draw_text(hdc, &tab.title, title_rect, text_color, DT_LEFT);
        if let Some(close) = close {
            draw_text(hdc, GLYPH_TAB_CLOSE, close, pal.text_muted, DT_CENTER);
        }
    }

    draw_frame_button_glyph(
        hdc,
        GLYPH_CLOSE,
        browser_panel_close_rect(panel),
        pal.text_muted,
    );
}

pub(super) fn draw_content_cards(hdc: HDC, state: &WindowsChromeState, rects: &ChromeRects) {
    if let Some(attached) = &state.attached {
        draw_content_card(hdc, attached.main);
        // A docked panel sits flush under the main card; square the card's
        // bottom corners so the shared seam has no notches.
        if attached.panels.iter().any(|panel| panel.docked) {
            square_card_bottom_corners(hdc, attached.main);
        }
        for panel in &attached.panels {
            // Interactive panels paint their own full-bleed card.
            if panel.host_content.is_some() {
                continue;
            }
            draw_content_card(hdc, panel.rect);
            if browser_panel_header_visible(panel) {
                draw_browser_panel_header(hdc, state.hwnd, panel);
            }
        }
        for panel in &attached.panels {
            // Maximized native panels are drawn as an overlay later in
            // `draw_window_chrome`, above sidebar/tabbar shell chrome.
            if panel.host_content.is_some() && !panel_is_maximized(panel) {
                draw_native_panel_content(hdc, state.hwnd, panel);
            }
        }
        // Attached cards are painted as plain filled rounded rects; the
        // rectangular WebView2 child overlays the corners, so they currently
        // read as square.
        return;
    }

    draw_content_card(hdc, rects.panel);
}

/// Overpaints the bottom rounded corners of a card with its own fill so
/// the bottom edge becomes square (used above a flush docked panel).
pub(super) fn square_card_bottom_corners(hdc: HDC, rect: RECT) {
    if rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: (rect.bottom - SHELL_PANEL_RADIUS).max(rect.top),
            right: rect.right,
            bottom: rect.bottom,
        },
        shell_palette().panel_background,
    );
}

pub(super) fn draw_content_card(hdc: HDC, rect: RECT) {
    if rect_width(&rect) > 0 && rect_height(&rect) > 0 {
        // White card on the gray window background: the arc must be
        // anti-aliased (and match the corner-cap radius of webview cards).
        fill_round_rect_aa(
            hdc,
            rect,
            SHELL_PANEL_RADIUS,
            shell_palette().panel_background,
        );
    }
}

pub(super) fn inset_rect(rect: RECT, dx: i32, dy: i32) -> RECT {
    normalize_rect(RECT {
        left: rect.left + dx,
        top: rect.top + dy,
        right: rect.right - dx,
        bottom: rect.bottom - dy,
    })
}

pub(super) fn normalize_rect(mut rect: RECT) -> RECT {
    if rect.right < rect.left {
        rect.right = rect.left;
    }
    if rect.bottom < rect.top {
        rect.bottom = rect.top;
    }
    rect
}

pub(super) fn rects_intersect(a: &RECT, b: &RECT) -> bool {
    a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
}

pub(super) fn rect_contains_rect(outer: &RECT, inner: &RECT) -> bool {
    inner.left >= outer.left
        && inner.top >= outer.top
        && inner.right <= outer.right
        && inner.bottom <= outer.bottom
}

pub(super) fn rect_width(rect: &RECT) -> i32 {
    (rect.right - rect.left).max(0)
}

pub(super) fn rect_height(rect: &RECT) -> i32 {
    (rect.bottom - rect.top).max(0)
}

pub(super) fn rect_contains(rect: &RECT, point: (i32, i32)) -> bool {
    point.0 >= rect.left && point.0 < rect.right && point.1 >= rect.top && point.1 < rect.bottom
}
