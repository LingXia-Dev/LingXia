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
    GetTextFaceW, HDC, HFONT, HGDIOBJ, IntersectClipRect, OUT_DEFAULT_PRECIS, RestoreDC, SaveDC,
    SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging;
use windows::core::{PCWSTR, w};

use crate::WindowsDesignIcon;

use super::style::*;

mod aside_panel;
mod drawing;
mod icons;
mod layout;
mod native_panel;
mod notice;
mod phone_bar;
mod sidebar;
mod top_bar;
#[cfg(feature = "browser-runtime")]
pub use aside_panel::begin_panel_address_edit;
pub(crate) use aside_panel::*;
pub(super) use drawing::*;
pub use layout::*;
use native_panel::*;
pub(crate) use notice::paint_shell_notice;
pub(crate) use phone_bar::*;
use sidebar::*;
#[cfg(feature = "browser-runtime")]
pub use top_bar::begin_address_edit;
use top_bar::*;

pub(in crate::shell) fn panel_activator_footer_height(
    width: i32,
    activators: &[WindowsShellPanelActivatorLayout],
) -> i32 {
    panel_activator_footer_height_for_width(width, activators)
}

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

/// Side length of the star/pin buttons inside the URL capsule (macOS
/// address-bar parity).
pub(super) const ADDRESS_CAPSULE_BUTTON_SIZE: i32 = 20;

/// Side length of the sidebar group-header chevron hit area.
pub(super) const SIDEBAR_CHEVRON_SIZE: i32 = 18;

/// Side length of the sidebar header action buttons (settings/downloads),
/// and the gap between them.
pub(super) const SIDEBAR_HEADER_ACTION_SIZE: i32 = 22;
pub(super) const SIDEBAR_HEADER_ACTION_GAP: i32 = 4;

pub(super) const SHELL_SIDEBAR_WIDTH: i32 = 184;

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

/// Top-level lxapp/web rows share one vertical rhythm.
pub(super) const SIDEBAR_ITEM_HEIGHT: i32 = 36;
pub(super) const SIDEBAR_ITEM_GAP: i32 = 4;

/// Expanded lxapp children use a compact desktop rhythm; the whole 28px row
/// remains clickable even though the visual selection is deliberately quiet.
pub(super) const SIDEBAR_CHILD_ITEM_HEIGHT: i32 = 28;
pub(super) const SIDEBAR_CHILD_ITEM_GAP: i32 = 0;
pub(super) const SIDEBAR_PARENT_CHILD_GAP: i32 = 1;

/// macOS uses the same 8pt outer inset for lxapp and browser rows. Keeping
/// this shared is what makes both top-level tab types occupy one visual
/// column instead of letting browser tabs drift inward.
pub(super) const SIDEBAR_ITEM_INSET: i32 = 8;

/// Leading padding inside every top-level lxapp/web row. Their 16px icons
/// therefore share the exact same x axis (`8 + 8` from the sidebar edge).
pub(super) const SIDEBAR_TOP_LEVEL_ICON_INSET: i32 = 8;

/// Child navigation sits inside its owning lxapp group. The parent remains a
/// plain section header; only the active leaf receives a selection surface.
pub(super) const SIDEBAR_CHILD_INDENT: i32 = 22;

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
pub(super) const SIDEBAR_FAVICON_TEXT_GAP: i32 = 8;

pub(super) const PANEL_ACTIVATOR_SIZE: i32 = 30;

pub(super) const PANEL_ACTIVATOR_ICON_SIZE: i32 = 16;

pub(super) const PANEL_ACTIVATOR_GAP: i32 = 4;

pub(super) const PANEL_ACTIVATOR_MARGIN: i32 = 6;

pub(super) const PANEL_ACTIVATOR_MAX_ROWS: usize = 5;

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
    pub(super) const BROWSER_TABS_CYCLE: &str = "browser.tabs.cycle";
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
    pub(super) const BROWSER_BOOKMARK_TOGGLE: &str = "browser.bookmark.toggle";
    pub(super) const BROWSER_PIN_TOGGLE: &str = "browser.pin.toggle";
    pub(super) const BROWSER_PAGE_MENU: &str = "browser.page-menu";
    pub(super) const BROWSER_CLOSE: &str = "browser.close";
    pub(super) const SIDEBAR_TOGGLE: &str = "sidebar.toggle";
    pub(super) const SIDEBAR_GROUP_TOGGLE: &str = "sidebar.group.toggle";
    pub(super) const SIDEBAR_ACTION: &str = "sidebar.action";
    pub(super) const SIDEBAR_SCROLL: &str = "sidebar.scroll";
    pub(super) const PANEL_ACTIVATOR_SCROLL: &str = "panel-activator.scroll";
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

    fn mouse_wheel(
        &self,
        state: &WindowsChromeState,
        point: (i32, i32),
        delta: i16,
    ) -> Option<WindowsChromeCommand> {
        let layout = shell_layout(&state.layout)?;
        chrome_mouse_wheel(state, layout, point, delta)
    }
}

fn sidebar_scroll_metrics(
    tabbar_rect: RECT,
    layout: &WindowsShellWindowLayout,
) -> Option<(i32, i32, i32)> {
    let tabbar = layout.tab_bar.as_ref()?;
    if !matches!(
        tabbar.position,
        WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
    ) {
        return None;
    }
    let viewport_bottom =
        sidebar_navigation_viewport_bottom(tabbar_rect, tabbar, &layout.panel_activators)
            .clamp(tabbar_rect.top + SHELL_TOP_BAR_HEIGHT, tabbar_rect.bottom);
    let (offset, max_offset) = clamp_sidebar_scroll(
        tabbar.main_scroll_offset,
        sidebar_content_bottom(tabbar_rect, tabbar),
        viewport_bottom,
    );
    Some((offset, max_offset, viewport_bottom))
}

fn clamp_sidebar_scroll(requested: i32, content_bottom: i32, viewport_bottom: i32) -> (i32, i32) {
    let max_offset = (content_bottom - viewport_bottom).max(0);
    (requested.clamp(0, max_offset), max_offset)
}

fn chrome_mouse_wheel(
    state: &WindowsChromeState,
    layout: &WindowsShellWindowLayout,
    point: (i32, i32),
    delta: i16,
) -> Option<WindowsChromeCommand> {
    if delta == 0 {
        return None;
    }
    let rects = chrome_rects_for_state(state, layout);
    let tabbar_rect = rects.tab_bar?;
    let tabbar = layout.tab_bar.as_ref()?;
    if !matches!(
        tabbar.position,
        WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
    ) || !rect_contains(&tabbar_rect, point)
    {
        return None;
    }

    let activator_max =
        panel_activator_max_scroll_row(tabbar_rect, tabbar, &layout.panel_activators);
    let over_activators = panel_activator_rects(state.client, &rects, layout)
        .iter()
        .any(|(_, rect)| rect_contains(rect, point));
    if activator_max > 0 && over_activators {
        let current = tabbar.activator_scroll_row.min(activator_max);
        let row = if delta > 0 {
            current.saturating_sub(1)
        } else {
            (current + 1).min(activator_max)
        };
        if row != current {
            return Some(
                WindowsChromeCommand::new(command_id::PANEL_ACTIVATOR_SCROLL).with_payload(json!({
                    "group": tabbar.group_id,
                    "row": row,
                })),
            );
        }
        return None;
    }

    let (current, max_offset, viewport_bottom) = sidebar_scroll_metrics(tabbar_rect, layout)?;
    if max_offset == 0
        || point.1 < tabbar_rect.top + SHELL_TOP_BAR_HEIGHT
        || point.1 >= viewport_bottom
    {
        return None;
    }
    let step = SIDEBAR_ITEM_HEIGHT + SIDEBAR_ITEM_GAP;
    let offset = if delta > 0 {
        current.saturating_sub(step)
    } else {
        (current + step).min(max_offset)
    };
    (offset != current).then(|| {
        WindowsChromeCommand::new(command_id::SIDEBAR_SCROLL).with_payload(json!({
            "group": tabbar.group_id,
            "offset": offset,
        }))
    })
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
        controls.bookmark,
        controls.pin,
        controls.page_menu,
    ];
    for rect in top_bar_buttons.into_iter().flatten() {
        if rect_contains(&rect, point) {
            return Some(rect);
        }
    }

    // Phone browser bar: every button lights up on hover.
    if phone_browser_bar_active(client, layout)
        && let Some(address_bar) = &layout.address_bar
    {
        let rects = phone_browser_bar_rects(client, address_bar.aside);
        if rect_contains(&rects.bar, point) {
            let buttons = [
                Some(rects.back),
                Some(rects.forward),
                rects.row_reload,
                rects.address_reload,
                rects.new_tab,
                Some(rects.tabs),
                Some(rects.close),
            ];
            for rect in buttons.into_iter().flatten() {
                if rect_contains(&rect, point) {
                    return Some(rect);
                }
            }
            return None;
        }
    }

    // Aside browser toolbar: nav cluster, tab strip (titles + closes), and
    // close-all light up on hover like the top-bar buttons.
    if let Some(attached) = &state.attached {
        for panel in &attached.panels {
            if !browser_panel_header_visible(panel)
                || !rect_contains(&browser_panel_header_rect(panel), point)
            {
                continue;
            }
            for (_, rect) in aside_panel_nav_button_rects(panel) {
                if rect_contains(&rect, point) {
                    return Some(rect);
                }
            }
            let close_all = browser_panel_close_rect(panel);
            if rect_contains(&close_all, point) {
                return Some(close_all);
            }
            let tabs = panel_aside_tabs(panel);
            for rect in aside_panel_tab_rects(panel, tabs.len()) {
                if let Some(close) = aside_panel_tab_close_rect(rect)
                    && rect_contains(&close, point)
                {
                    return Some(close);
                }
                if rect_contains(&rect, point) {
                    return Some(rect);
                }
            }
            return Some(browser_panel_header_rect(panel));
        }
    }

    if !address_bar_visible(layout)
        && let (Some(navbar), Some(navbar_rect)) = (&layout.navigation_bar, rects.navigation_bar)
        && rect_contains(&navbar_rect, point)
    {
        let buttons_left = navbar_buttons_left(navbar_rect);
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
        let (scroll_offset, _, viewport_bottom) =
            sidebar_scroll_metrics(tabbar_rect, layout).unwrap_or((0, 0, tabbar_rect.bottom));
        let expand = sidebar_rail_expand_rect(tabbar_rect);
        if rect_contains(&expand, point) {
            return Some(expand);
        }
        if point.1 < tabbar_rect.top + SHELL_TOP_BAR_HEIGHT || point.1 >= viewport_bottom {
            return None;
        }
        for index in 0..=tabbar.auxiliary_items.len() {
            let rect = sidebar_rail_item_rect(tabbar_rect, index, scroll_offset);
            if rect_contains(&rect, point) {
                return Some(rect);
            }
        }
        if tabbar.show_auxiliary_add {
            let rect = sidebar_rail_add_rect(tabbar_rect, tabbar, scroll_offset);
            if rect_contains(&rect, point) {
                return Some(rect);
            }
        }
        return None;
    }

    let (scroll_offset, _, viewport_bottom) =
        sidebar_scroll_metrics(tabbar_rect, layout).unwrap_or((0, 0, tabbar_rect.bottom));
    for (_, rect) in sidebar_header_action_rects(tabbar_rect, tabbar) {
        if rect_contains(&rect, point) {
            return Some(rect);
        }
    }
    if point.1 < tabbar_rect.top + SHELL_TOP_BAR_HEIGHT || point.1 >= viewport_bottom {
        return None;
    }
    let chevron = sidebar_group_chevron_rect(tabbar_rect, tabbar, scroll_offset);
    if rect_contains(&chevron, point) {
        return Some(chevron);
    }
    let group = sidebar_group_rect(tabbar_rect, tabbar, scroll_offset);
    if rect_contains(&group, point) {
        return Some(group);
    }
    if !tabbar.items_collapsed {
        for index in 0..tabbar.items.len() {
            let rect = sidebar_item_rect(tabbar_rect, tabbar, index, scroll_offset);
            if rect_contains(&rect, point) {
                return Some(rect);
            }
        }
    }
    if let Some(auxiliary) =
        sidebar_auxiliary_rects(tabbar_rect, tabbar, scroll_offset, viewport_bottom)
    {
        for (_, rect) in &auxiliary.items {
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
        || old_layout.navigation_bar != new_layout.navigation_bar
    {
        return None;
    }

    let mut dirty = Vec::new();
    if old_layout.address_bar != new_layout.address_bar {
        push_dirty_rect(&mut dirty, new_rects.top_bar, client);
        // A phone-width browser paints its address bar (URL, nav state, tab
        // count) in the bottom bar instead of the top band.
        if phone_browser_bar_active(client, new_layout) {
            push_dirty_rect(
                &mut dirty,
                phone_browser_bar_rects(client, phone_bar_is_aside(new_layout)).bar,
                client,
            );
        }
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
        || old_tabbar.group_active != new_tabbar.group_active
        || old_tabbar.group_closable != new_tabbar.group_closable
        || old_tabbar.group_order_index != new_tabbar.group_order_index
        || old_tabbar.color != new_tabbar.color
        || old_tabbar.background_color != new_tabbar.background_color
        || old_tabbar.background_transparent != new_tabbar.background_transparent
        || old_tabbar.border_color != new_tabbar.border_color
        || old_tabbar.items != new_tabbar.items
        || old_tabbar.collapsed != new_tabbar.collapsed
        || old_tabbar.icon_rail != new_tabbar.icon_rail
        || old_tabbar.items_api_hidden != new_tabbar.items_api_hidden
        || old_tabbar.items_collapsed != new_tabbar.items_collapsed
        || old_tabbar.activator_footer_height != new_tabbar.activator_footer_height
        || old_tabbar.main_scroll_offset != new_tabbar.main_scroll_offset
        || old_tabbar.activator_scroll_row != new_tabbar.activator_scroll_row
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
            .all(|(old_item, new_item)| {
                old_item.id == new_item.id && old_item.pinned == new_item.pinned
            })
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
            sidebar_item_rect(
                rect,
                new_tabbar,
                index as usize,
                new_tabbar.main_scroll_offset,
            )
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
    let viewport_bottom = rect.bottom - new_tabbar.activator_footer_height;
    let Some(auxiliary) = sidebar_auxiliary_rects(
        rect,
        new_tabbar,
        new_tabbar.main_scroll_offset,
        viewport_bottom,
    ) else {
        return;
    };
    for (index, (old_item, new_item)) in old_tabbar
        .auxiliary_items
        .iter()
        .zip(&new_tabbar.auxiliary_items)
        .enumerate()
    {
        if old_item == new_item && old_tabbar.selected_color == new_tabbar.selected_color {
            continue;
        }
        if let Some((_, item_rect)) = auxiliary
            .items
            .iter()
            .find(|(item_index, _)| *item_index == index)
        {
            push_dirty_rect(dirty, *item_rect, client);
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
    /// Main/aside workspace below global shell chrome, before aside splits.
    pub(super) workspace: RECT,
    /// Main WebView viewport when no aside layout is attached.
    pub(super) content: RECT,
    /// Unsplit workspace card used by the no-aside painter/activator layout.
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
    let desktop_card = top_inset == 0 && !layout.suppress_window_controls;
    let mut content = client;
    content.top += top_inset;
    let tab_bar = layout
        .tab_bar
        .as_ref()
        .filter(|tabbar| {
            tabbar.visible
                && tabbar.dimension > 0
                && (!tabbar.items.is_empty()
                    || !tabbar.auxiliary_items.is_empty()
                    || tabbar.show_auxiliary_add
                    || !tabbar.header_actions.is_empty()
                    || tabbar.activator_footer_height > 0)
        })
        .map(|tabbar| match tabbar.position {
            WindowsShellTabBarPosition::Left => {
                let width = sidebar_column_width(tabbar);
                let right = (content.left + width).min(content.right);
                let rect = RECT {
                    left: content.left,
                    top: content.top,
                    right,
                    bottom: content.bottom,
                };
                content.left = right;
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
                content.right = left;
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

    let side_tabbar = tab_bar.is_some()
        && matches!(
            layout.tab_bar.as_ref().map(|tabbar| tabbar.position),
            Some(WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right)
        );
    let reserve_top_bar = side_tabbar
        || (top_inset == 0 && !layout.suppress_window_controls)
        || (address_bar_visible(layout) && !phone_browser_bar_active(client, layout));

    // The caption/address row belongs to the shell's base layer. Keep it at
    // the real top of the client so its controls line up with minimize /
    // maximize / close instead of moving down with the content card.
    let top_bar = normalize_rect(RECT {
        left: content.left,
        top: client.top + top_inset,
        right: content.right,
        bottom: if reserve_top_bar {
            (client.top + top_inset + SHELL_TOP_BAR_HEIGHT).min(client.bottom)
        } else {
            client.top + top_inset
        },
    });
    if reserve_top_bar {
        content.top = top_bar.bottom.max(content.top);
    }

    // The WebView workspace is the second layer. Side and bottom clearance
    // keep the card distinct from the shell base; its top edge aligns with
    // the first sidebar row immediately below the caption/address band.
    if desktop_card {
        content = inset_desktop_workspace(content);
    }

    // The phone-frame browser chrome docks at the screen's bottom (the macOS
    // RunnerPhoneBrowserSurface layout); the webview ends above it.
    if phone_browser_bar_active(client, layout) {
        let bar = phone_browser_bar_rects(client, phone_bar_is_aside(layout)).bar;
        content.bottom = bar.top.max(content.top);
    }

    let workspace = normalize_rect(content);
    let panel = workspace;

    let (navigation_bar, content) = split_main_navigation_bar(workspace, layout);

    ChromeRects {
        workspace,
        content: normalize_rect(content),
        panel: normalize_rect(panel),
        top_bar,
        navigation_bar: navigation_bar.map(normalize_rect),
        tab_bar: tab_bar.map(normalize_rect),
    }
}

fn inset_desktop_workspace(rect: RECT) -> RECT {
    normalize_rect(RECT {
        left: rect.left + SHELL_CONTENT_INSET,
        top: rect.top,
        right: rect.right - SHELL_CONTENT_INSET,
        bottom: rect.bottom - SHELL_CONTENT_INSET,
    })
}

/// Reserve the lxapp navigation bar inside the concrete main region. Aside
/// slots are split before this helper is applied, so their viewports never
/// inherit or sit beneath main-owned chrome.
fn split_main_navigation_bar(
    main_region: RECT,
    layout: &WindowsShellWindowLayout,
) -> (Option<RECT>, RECT) {
    let Some(navbar) = layout
        .navigation_bar
        .as_ref()
        .filter(|navbar| navbar.visible && navbar.height > 0)
    else {
        return (None, main_region);
    };
    let height = navbar.height.clamp(0, rect_height(&main_region));
    if height == 0 || rect_width(&main_region) == 0 {
        return (None, main_region);
    }
    let navigation_bar = normalize_rect(RECT {
        left: main_region.left,
        top: main_region.top,
        right: main_region.right,
        bottom: main_region.top + height,
    });
    let content = normalize_rect(RECT {
        top: navigation_bar.bottom,
        ..main_region
    });
    (Some(navigation_bar), content)
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
    let anchor = sidebar_rail_item_rect(tabbar_rect, sidebar_group_rail_index(tabbar), 0);
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
            + rows * SIDEBAR_CHILD_ITEM_HEIGHT
            + (rows - 1).max(0) * SIDEBAR_CHILD_ITEM_GAP,
    )
}

pub(crate) fn collapsed_sidebar_tabbar_popup_hit(
    tabbar: &WindowsShellTabBarLayout,
    point: (i32, i32),
) -> Option<usize> {
    // Hit-test against the same tabbar variant the popup paints; the raw
    // tabbar's auxiliary items would shift row rects by the pinned-grid height.
    let popup_tabbar = collapsed_sidebar_popup_tabbar(tabbar, SIDEBAR_TABBAR_POPUP_WIDTH);
    let bounds = normalize_rect(RECT {
        left: 0,
        top: 0,
        right: SIDEBAR_TABBAR_POPUP_WIDTH,
        bottom: collapsed_sidebar_tabbar_popup_size(&popup_tabbar).1,
    });
    let item_bounds = collapsed_sidebar_tabbar_popup_item_bounds(bounds);
    (0..popup_tabbar.items.len()).find(|&index| {
        rect_contains(
            &sidebar_item_rect(item_bounds, &popup_tabbar, index, 0),
            point,
        )
    })
}

pub(crate) fn collapsed_sidebar_tabbar_click_command(
    group: &str,
    index: usize,
) -> WindowsChromeCommand {
    WindowsChromeCommand::new(command_id::TAB_BAR_CLICK)
        .with_payload(json!({ "group": group, "index": index }))
}

/// The tabbar variant the popup renders: expanded first-level rows only, no
/// header actions or auxiliary section. Paint and hit-test must both use this
/// so their row geometry agrees.
fn collapsed_sidebar_popup_tabbar(
    tabbar: &WindowsShellTabBarLayout,
    width: i32,
) -> WindowsShellTabBarLayout {
    let mut popup_tabbar = tabbar.clone();
    popup_tabbar.collapsed = false;
    popup_tabbar.icon_rail = false;
    popup_tabbar.items_collapsed = false;
    popup_tabbar.dimension = width;
    popup_tabbar.header_actions.clear();
    popup_tabbar.auxiliary_items.clear();
    popup_tabbar.group_order_index = 0;
    popup_tabbar.main_scroll_offset = 0;
    popup_tabbar.show_auxiliary_add = false;
    popup_tabbar
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
    let popup_tabbar = collapsed_sidebar_popup_tabbar(tabbar, width);
    // The host alpha-masks the layered popup to the rounded shape; fill the
    // full bounds and draw only the hairline outline here.
    fill_rect(hdc, bounds, shell_palette().sidebar_background);
    draw_sidebar_items(
        hdc,
        collapsed_sidebar_tabbar_popup_item_bounds(bounds),
        &popup_tabbar,
        None,
        0,
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
        // `sidebar_item_rect` adds the normal shell/header/group offsets.
        // Cancel them so the popup's first child starts at its own padding.
        top: bounds.top + SIDEBAR_TABBAR_POPUP_PADDING
            - SHELL_TOP_BAR_HEIGHT
            - SIDEBAR_ITEM_HEIGHT
            - SIDEBAR_PARENT_CHILD_GAP,
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
        0,
        height,
    );
}

fn compute_attached_layout(
    client: RECT,
    layout: &WindowsShellWindowLayout,
    panels: &[WindowsChromePanelLayoutInput],
) -> WindowsChromeAttachedLayout {
    let mut main_region = compute_chrome_rects(client, layout).workspace;
    let mut out = Vec::new();

    let mut ordered = panels.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| attached_panel_order(left).cmp(&attached_panel_order(right)));

    if let Some(&maximized) = ordered.iter().find(|panel| panel.docked && panel.maximized) {
        out.push(WindowsChromePanelLayout {
            panel_id: maximized.panel_id.clone(),
            webtag_key: maximized.webtag_key.clone(),
            rect: shell_maximized_panel_rect(main_region),
            header_rect: None,
            resize_handle: None,
        });
        main_region.bottom = main_region.top;
        return WindowsChromeAttachedLayout {
            main_region,
            main: main_region,
            panels: out,
        };
    }

    for panel in ordered {
        let (rect, resize_handle) = match panel.position {
            WindowsPanelPosition::Left => {
                let width = attached_panel_size(panel, main_region, ATTACHED_PANEL_WIDTH);
                let rect = RECT {
                    left: main_region.left,
                    top: main_region.top,
                    right: (main_region.left + width).min(main_region.right),
                    bottom: main_region.bottom,
                };
                let handle_width = SHELL_PANEL_GAP.max(ATTACHED_PANEL_HANDLE_SIZE);
                let handle = normalize_rect(RECT {
                    left: rect.right,
                    top: rect.top,
                    right: (rect.right + handle_width).min(main_region.right),
                    bottom: rect.bottom,
                });
                main_region.left = handle.right.min(main_region.right);
                (rect, Some(handle))
            }
            WindowsPanelPosition::Right => {
                let width = attached_panel_size(panel, main_region, ATTACHED_PANEL_WIDTH);
                let rect = RECT {
                    left: (main_region.right - width).max(main_region.left),
                    top: main_region.top,
                    right: main_region.right,
                    bottom: main_region.bottom,
                };
                let handle_width = SHELL_PANEL_GAP.max(ATTACHED_PANEL_HANDLE_SIZE);
                let handle = normalize_rect(RECT {
                    left: (rect.left - handle_width).max(main_region.left),
                    top: rect.top,
                    right: rect.left,
                    bottom: rect.bottom,
                });
                main_region.right = handle.left.max(main_region.left);
                (rect, Some(handle))
            }
            // Docked and floating share the geometry: the gutter between the
            // panel and the content shows the shell background and hosts the
            // resize handle, so the split reads as a divider — not the panel
            // covering the content. (Side panels already separate this way.)
            WindowsPanelPosition::Top => {
                let height = attached_panel_size(panel, main_region, ATTACHED_PANEL_BOTTOM_HEIGHT);
                let rect = RECT {
                    left: main_region.left,
                    top: main_region.top,
                    right: main_region.right,
                    bottom: (main_region.top + height).min(main_region.bottom),
                };
                let handle_height = SHELL_PANEL_GAP.max(ATTACHED_PANEL_HANDLE_SIZE);
                let handle = normalize_rect(RECT {
                    left: rect.left,
                    top: rect.bottom,
                    right: rect.right,
                    bottom: (rect.bottom + handle_height).min(main_region.bottom),
                });
                main_region.top = handle.bottom.min(main_region.bottom);
                (rect, Some(handle))
            }
            WindowsPanelPosition::Bottom => {
                let height = attached_panel_size(panel, main_region, ATTACHED_PANEL_BOTTOM_HEIGHT);
                // Docked panels split the space flat — flush with the content
                // edges, divided from the content by the gutter's hairline —
                // so both regions read as the same layer. A floating panel
                // keeps a bottom margin and draws as a rounded card instead.
                let bottom = if panel.docked {
                    main_region.bottom
                } else {
                    (main_region.bottom - SHELL_PANEL_GAP).max(main_region.top)
                };
                let rect = RECT {
                    left: main_region.left,
                    top: (bottom - height).max(main_region.top),
                    right: main_region.right,
                    bottom,
                };
                let handle_height = SHELL_PANEL_GAP.max(ATTACHED_PANEL_HANDLE_SIZE);
                let handle = normalize_rect(RECT {
                    left: rect.left,
                    top: (rect.top - handle_height).max(main_region.top),
                    right: rect.right,
                    bottom: rect.top,
                });
                main_region.bottom = handle.top.max(main_region.top);
                (rect, Some(handle))
            }
        };

        // A browser aside carries its chrome (nav cluster + tab strip +
        // close) as a toolbar row at the panel's own top, above the webview —
        // the macOS DockedBrowser layout. Side panels only: terminal asides
        // (top/bottom) keep their own native header.
        let header_rect = (matches!(
            panel.position,
            WindowsPanelPosition::Left | WindowsPanelPosition::Right
        ) && (panel.webtag_key.starts_with("app.lingxia.browser:")
            || !aside_panel_tabs(&panel.panel_id).is_empty()))
        .then(|| aside_panel_toolbar_rect(rect));

        out.push(WindowsChromePanelLayout {
            panel_id: panel.panel_id.clone(),
            webtag_key: panel.webtag_key.clone(),
            rect: normalize_rect(rect),
            header_rect,
            resize_handle,
        });
    }

    let main_region = normalize_rect(main_region);
    let (_, main) = split_main_navigation_bar(main_region, layout);
    WindowsChromeAttachedLayout {
        main_region,
        main: normalize_rect(main),
        panels: out,
    }
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
            available - SHELL_PANEL_GAP - ATTACHED_MAIN_MIN_HEIGHT
        }
        WindowsPanelPosition::Left | WindowsPanelPosition::Right => {
            available - SHELL_PANEL_GAP - ATTACHED_MAIN_MIN_WIDTH
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

/// Chrome rects for a concrete window state. Attached panels arbitrate the
/// workspace first; the navigation bar then occupies the top of the resulting
/// main region.
pub(super) fn chrome_rects_for_state(
    state: &WindowsChromeState,
    layout: &WindowsShellWindowLayout,
) -> ChromeRects {
    let mut rects = compute_chrome_rects(state.client, layout);
    if rects.navigation_bar.is_some()
        && let Some(attached) = &state.attached
    {
        rects.navigation_bar = split_main_navigation_bar(attached.main_region, layout).0;
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
    draw_content_cards(hdc, state, &rects);
    draw_shell_top_bar(hdc, &rects);

    // The address bar owns the top bar while a browser surface is
    // presented; the lxapp navigation bar yields.
    if !address_bar_visible(layout)
        && let (Some(navbar), Some(navbar_rect)) = (&layout.navigation_bar, rects.navigation_bar)
    {
        let buttons_left = navbar_buttons_left(navbar_rect);
        let corner_radii =
            workspace_corner_radii(navbar_rect, rects.workspace, SHELL_CONTENT_RADIUS);
        draw_navigation_bar(
            hdc,
            navbar_rect,
            corner_radii,
            buttons_left,
            navbar,
            state.cursor,
        );
    }
    if let (Some(tabbar), Some(tabbar_rect)) = (&layout.tab_bar, rects.tab_bar) {
        let (scroll_offset, _, viewport_bottom) =
            sidebar_scroll_metrics(tabbar_rect, layout).unwrap_or((0, 0, tabbar_rect.bottom));
        draw_tab_bar(
            hdc,
            tabbar_rect,
            tabbar,
            state.cursor,
            scroll_offset,
            viewport_bottom,
        );
    }
    // Global window/address controls remain in the shell-owned top strip;
    // the lxapp navigation bar above is confined to the main region below it.
    draw_top_bar_controls(hdc, state, &rects, layout);
    if phone_browser_bar_active(state.client, layout) {
        draw_phone_browser_bar(hdc, state, layout);
    }
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
    draw_native_panel_content(hdc, state.hwnd, state.client, panel);
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
    // The phone browser bar owns its bottom card; every element maps to the
    // same browser commands the top bar uses.
    if phone_browser_bar_active(client, layout)
        && let Some(address_bar) = &layout.address_bar
    {
        let rects = phone_browser_bar_rects(client, address_bar.aside);
        if rect_contains(&rects.bar, point) {
            if rect_contains(&rects.back, point) {
                return Some(chrome_command(command_id::BROWSER_NAV_BACK, json!({})));
            }
            if rect_contains(&rects.forward, point) {
                return Some(chrome_command(command_id::BROWSER_NAV_FORWARD, json!({})));
            }
            if rects
                .row_reload
                .or(rects.address_reload)
                .is_some_and(|reload| rect_contains(&reload, point))
            {
                return Some(chrome_command(command_id::BROWSER_NAV_RELOAD, json!({})));
            }
            if rects
                .new_tab
                .is_some_and(|new_tab| rect_contains(&new_tab, point))
            {
                return Some(chrome_command(command_id::BROWSER_NEW_TAB, json!({})));
            }
            if rect_contains(&rects.tabs, point) {
                return Some(chrome_command(command_id::BROWSER_TABS_CYCLE, json!({})));
            }
            if rect_contains(&rects.close, point) {
                return Some(chrome_command(command_id::BROWSER_CLOSE, json!({})));
            }
            if rects
                .address
                .is_some_and(|address| rect_contains(&address, point))
            {
                return Some(chrome_command(command_id::BROWSER_ADDRESS_BAR, json!({})));
            }
            return Some(WindowsChromeHit::Chrome);
        }
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
    // The star/pin buttons sit inside the address capsule, so they must win
    // the hit-test over the address edit.
    if let Some(bookmark) = controls.bookmark
        && rect_contains(&bookmark, point)
    {
        return Some(chrome_command(
            command_id::BROWSER_BOOKMARK_TOGGLE,
            json!({}),
        ));
    }
    if let Some(pin) = controls.pin
        && rect_contains(&pin, point)
    {
        return Some(chrome_command(command_id::BROWSER_PIN_TOGGLE, json!({})));
    }
    if let Some(address) = controls.address
        && rect_contains(&address, point)
    {
        return Some(chrome_command(command_id::BROWSER_ADDRESS_BAR, json!({})));
    }
    if let Some(page_menu) = controls.page_menu
        && rect_contains(&page_menu, point)
    {
        return Some(WindowsChromeHit::Command(
            WindowsChromeCommand::new(command_id::BROWSER_PAGE_MENU).with_screen_position(),
        ));
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
        let buttons_left = navbar_buttons_left(navbar_rect);
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
        let disabled = layout
            .panel_activators
            .iter()
            .find(|item| item.id == panel_id)
            .is_some_and(|item| item.disabled);
        if !disabled && rect_contains(&rect, point) {
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
        let (scroll_offset, _, viewport_bottom) = if sidebar {
            sidebar_scroll_metrics(tabbar_rect, layout).unwrap_or((0, 0, tabbar_rect.bottom))
        } else {
            (0, 0, tabbar_rect.bottom)
        };
        let in_sidebar_viewport =
            point.1 >= tabbar_rect.top + SHELL_TOP_BAR_HEIGHT && point.1 < viewport_bottom;
        if sidebar {
            // Header actions remain interactive, while every unused pixel in
            // the sidebar's caption strip must behave like a native caption.
            // Treating the whole tab-bar column as client chrome made the most
            // obvious Arc-style drag affordance inert.
            for (action_id, action_rect) in sidebar_header_action_rects(tabbar_rect, tabbar) {
                if rect_contains(&action_rect, point) {
                    return Some(chrome_command(
                        command_id::SIDEBAR_ACTION,
                        json!({ "action_id": action_id }),
                    ));
                }
            }
            if sidebar_caption_contains(tabbar_rect, point) {
                return Some(WindowsChromeHit::Caption);
            }
        }
        if sidebar && (tabbar.collapsed || tabbar.icon_rail) {
            if rect_contains(&sidebar_rail_expand_rect(tabbar_rect), point) {
                return Some(chrome_command(command_id::SIDEBAR_TOGGLE, json!({})));
            }
            if in_sidebar_viewport
                && rect_contains(
                    &sidebar_rail_item_rect(
                        tabbar_rect,
                        sidebar_group_rail_index(tabbar),
                        scroll_offset,
                    ),
                    point,
                )
            {
                let payload = json!({ "tab_id": format!("lxapp:{}", tabbar.group_id) });
                return Some(chrome_command_with_context(
                    command_id::BROWSER_TAB_CLICK,
                    payload.clone(),
                    command_id::SIDEBAR_AUXILIARY_CONTEXT_MENU,
                    payload,
                ));
            }
            for (index, item) in tabbar.auxiliary_items.iter().enumerate() {
                if in_sidebar_viewport
                    && rect_contains(
                        &sidebar_rail_item_rect(
                            tabbar_rect,
                            sidebar_auxiliary_rail_index(tabbar, index),
                            scroll_offset,
                        ),
                        point,
                    )
                {
                    let payload = json!({ "tab_id": item.id.clone() });
                    return Some(chrome_command_with_context(
                        command_id::BROWSER_TAB_CLICK,
                        payload.clone(),
                        command_id::SIDEBAR_AUXILIARY_CONTEXT_MENU,
                        payload,
                    ));
                }
            }
            if in_sidebar_viewport
                && tabbar.show_auxiliary_add
                && rect_contains(
                    &sidebar_rail_add_rect(tabbar_rect, tabbar, scroll_offset),
                    point,
                )
            {
                return Some(chrome_command(command_id::BROWSER_NEW_TAB, json!({})));
            }
            return Some(WindowsChromeHit::Chrome);
        }
        if sidebar {
            if in_sidebar_viewport
                && tabbar.group_closable
                && rect_contains(
                    &sidebar_group_close_rect(tabbar_rect, tabbar, scroll_offset),
                    point,
                )
            {
                return Some(chrome_command(
                    command_id::BROWSER_TAB_CLOSE,
                    json!({ "tab_id": format!("lxapp:{}", tabbar.group_id) }),
                ));
            }
            if in_sidebar_viewport
                && !tabbar.items_api_hidden
                && !tabbar.items.is_empty()
                && rect_contains(
                    &sidebar_group_chevron_rect(tabbar_rect, tabbar, scroll_offset),
                    point,
                )
            {
                return Some(chrome_command(
                    command_id::SIDEBAR_GROUP_TOGGLE,
                    json!({ "group": tabbar.group_id.clone() }),
                ));
            }
            if in_sidebar_viewport
                && rect_contains(
                    &sidebar_group_rect(tabbar_rect, tabbar, scroll_offset),
                    point,
                )
            {
                let payload = json!({ "tab_id": format!("lxapp:{}", tabbar.group_id) });
                return Some(chrome_command_with_context(
                    command_id::BROWSER_TAB_CLICK,
                    payload.clone(),
                    command_id::SIDEBAR_AUXILIARY_CONTEXT_MENU,
                    payload,
                ));
            }
        }
        if !sidebar || (!tabbar.items_collapsed && in_sidebar_viewport) {
            for index in 0..tabbar.items.len() {
                let item_rect = if sidebar {
                    sidebar_item_rect(tabbar_rect, tabbar, index, scroll_offset)
                } else {
                    tab_item_rect(tabbar_rect, tabbar.position, tabbar.items.len(), index)
                };
                if rect_contains(&item_rect, point) {
                    return Some(chrome_command(
                        command_id::TAB_BAR_CLICK,
                        json!({ "group": tabbar.group_id, "index": index }),
                    ));
                }
            }
        }
        if sidebar
            && in_sidebar_viewport
            && let Some(hit) = sidebar_auxiliary_hit_test(
                tabbar_rect,
                tabbar,
                point,
                scroll_offset,
                viewport_bottom,
            )
        {
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

fn sidebar_caption_contains(rect: RECT, point: (i32, i32)) -> bool {
    rect_contains(
        &RECT {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: (rect.top + SHELL_TOP_BAR_HEIGHT).min(rect.bottom),
        },
        point,
    )
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

pub(super) fn draw_content_cards(hdc: HDC, state: &WindowsChromeState, rects: &ChromeRects) {
    if let Some(attached) = &state.attached {
        // Main and asides share one outer shadow; their resize gutters cut
        // through that wrapper to the first shell layer below.
        draw_content_card(hdc, rects.panel);
        // Each resize gutter exposes the first shell layer, matching the top
        // address-bar band. The centered hairline keeps the split legible on
        // every edge and between multiple asides.
        let pal = shell_palette();
        for panel in &attached.panels {
            let Some(handle) = panel.resize_handle else {
                continue;
            };
            fill_rect(hdc, handle, pal.window_background);
            if rect_width(&handle) <= rect_height(&handle) {
                let mid = handle.left + rect_width(&handle) / 2;
                fill_rect(
                    hdc,
                    RECT {
                        left: mid,
                        top: handle.top,
                        right: mid + 1,
                        bottom: handle.bottom,
                    },
                    pal.divider,
                );
            } else {
                let mid = handle.top + rect_height(&handle) / 2;
                fill_rect(
                    hdc,
                    RECT {
                        left: handle.left,
                        top: mid,
                        right: handle.right,
                        bottom: mid + 1,
                    },
                    pal.divider,
                );
            }
        }
        for panel in &attached.panels {
            if panel.host_content.is_some() {
                continue;
            }
            if browser_panel_header_visible(panel) {
                draw_browser_panel_header(hdc, state.hwnd, panel, state.cursor);
            }
        }
        for panel in &attached.panels {
            // Maximized native panels are drawn as an overlay later in
            // `draw_window_chrome`, above sidebar/tabbar shell chrome.
            if panel.host_content.is_some() && !panel_is_maximized(panel) {
                draw_native_panel_content(hdc, state.hwnd, state.client, panel);
            }
        }
        // The card's arcs show wherever no surface covers them (loading,
        // gutters); composition-hosted webviews clip to the same silhouette
        // via `workspace_corner_radii`, windowed ones still overlay square.
        return;
    }

    draw_content_card(hdc, rects.panel);
}

pub(super) fn draw_content_card(hdc: HDC, rect: RECT) {
    if rect_width(&rect) > 0 && rect_height(&rect) > 0 {
        // macOS uses an 8pt, 15%-opacity shadow on one wrapper around the whole
        // workspace. Layered translucent expansions approximate that blur in
        // GDI+ without creating a solid band around the card.
        for spread in (1..=8).rev() {
            let alpha = if spread <= 2 { 10 } else { 5 };
            fill_round_rect_overlay(
                hdc,
                RECT {
                    left: rect.left - spread,
                    top: rect.top - spread + 2,
                    right: rect.right + spread,
                    bottom: rect.bottom + spread + 2,
                },
                SHELL_CONTENT_RADIUS + spread,
                (alpha as u32) << 24,
            );
        }
        fill_round_rect_aa(
            hdc,
            rect,
            SHELL_CONTENT_RADIUS,
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

/// Per-corner clip radii for a composition-hosted surface,
/// `[top_left, top_right, bottom_right, bottom_left]`, in the same raw client
/// pixels as `set_content_bounds`.
pub(crate) type SurfaceCornerRadii = [i32; 4];

/// Corner radii for a surface rect inside the rounded workspace silhouette.
///
/// `compute_attached_layout` carves every surface from the workspace rect by
/// integer assignment, so a corner belongs to the silhouette iff both of its
/// coordinates are exactly equal — gutters, aside headers, and the navigation
/// bar break coincidence for interior seams by construction. The radius is
/// clamped so tiny surfaces never self-intersect.
pub(crate) fn workspace_corner_radii(
    surface: RECT,
    silhouette: RECT,
    radius: i32,
) -> SurfaceCornerRadii {
    let surface = normalize_rect(surface);
    if rect_width(&surface) == 0 || rect_height(&surface) == 0 {
        return [0; 4];
    }
    let radius = radius
        .min(rect_width(&surface) / 2)
        .min(rect_height(&surface) / 2)
        .max(0);
    let corner = |x: bool, y: bool| if x && y { radius } else { 0 };
    [
        corner(
            surface.left == silhouette.left,
            surface.top == silhouette.top,
        ),
        corner(
            surface.right == silhouette.right,
            surface.top == silhouette.top,
        ),
        corner(
            surface.right == silhouette.right,
            surface.bottom == silhouette.bottom,
        ),
        corner(
            surface.left == silhouette.left,
            surface.bottom == silhouette.bottom,
        ),
    ]
}

/// The workspace silhouette a window layout rounds — the union rect that
/// `compute_attached_layout` carves main and aside surfaces from.
pub(crate) fn workspace_silhouette_rect(
    client: RECT,
    layout: &WindowsWindowLayout,
) -> Option<RECT> {
    let layout = shell_layout(layout)?;
    Some(compute_chrome_rects(client, layout).workspace)
}

#[cfg(test)]
mod scroll_tests {
    use super::{
        SHELL_CONTENT_INSET, SHELL_PANEL_GAP, SHELL_TOP_BAR_HEIGHT, SIDEBAR_ICON_SIZE,
        SIDEBAR_ITEM_HEIGHT, WindowsChromePanelLayoutInput, WindowsPanelPosition,
        WindowsShellAuxiliaryItemLayout, WindowsShellNavigationBarLayout,
        WindowsShellTabBarItemLayout, WindowsShellTabBarLayout, WindowsShellTabBarPosition,
        WindowsShellWindowLayout, clamp_sidebar_scroll, collapsed_sidebar_tabbar_click_command,
        compute_attached_layout, compute_chrome_rects, sidebar_auxiliary_rects,
        sidebar_caption_contains, sidebar_group_rect, sidebar_top_level_icon_rect,
        tabbar_requires_full_repaint,
    };
    use windows::Win32::Foundation::RECT;

    #[test]
    fn sidebar_scroll_is_bounded_by_content_overflow() {
        assert_eq!(clamp_sidebar_scroll(80, 600, 500), (80, 100));
        assert_eq!(clamp_sidebar_scroll(180, 600, 500), (100, 100));
        assert_eq!(clamp_sidebar_scroll(-20, 600, 500), (0, 100));
        assert_eq!(clamp_sidebar_scroll(20, 400, 500), (0, 0));
    }

    #[test]
    fn unused_sidebar_top_bar_stays_draggable() {
        let sidebar = RECT {
            left: 0,
            top: 0,
            right: 220,
            bottom: 768,
        };
        assert!(sidebar_caption_contains(sidebar, (100, 20)));
        assert!(!sidebar_caption_contains(sidebar, (100, 40)));
    }

    #[test]
    fn desktop_top_bar_and_content_card_use_distinct_layers() {
        let tab_bar = WindowsShellTabBarLayout {
            visible: true,
            position: WindowsShellTabBarPosition::Left,
            dimension: 220,
            app_name: "App".to_string(),
            app_icon_path: String::new(),
            group_id: "app".to_string(),
            group_active: true,
            group_closable: false,
            group_order_index: 0,
            color: 0,
            selected_color: 0,
            background_color: 0,
            background_transparent: false,
            border_color: 0,
            selected_index: 0,
            items: vec![WindowsShellTabBarItemLayout {
                page_path: "home".to_string(),
                text: "Home".to_string(),
                icon_path: String::new(),
                selected_icon_path: String::new(),
                badge: None,
                has_red_dot: false,
            }],
            collapsed: false,
            icon_rail: false,
            items_api_hidden: false,
            items_collapsed: false,
            activator_footer_height: 0,
            main_scroll_offset: 0,
            activator_scroll_row: 0,
            auxiliary_items: Vec::new(),
            show_auxiliary_add: false,
            header_actions: Vec::new(),
        };
        let mut layout = WindowsShellWindowLayout {
            tab_bar: Some(tab_bar),
            ..Default::default()
        };
        let client = RECT {
            left: 0,
            top: 0,
            right: 1024,
            bottom: 768,
        };
        let rects = compute_chrome_rects(client, &layout);

        assert_eq!(rects.top_bar.left, 220);
        assert_eq!(rects.top_bar.top, client.top);
        assert_eq!(rects.top_bar.bottom, client.top + SHELL_TOP_BAR_HEIGHT);
        assert_eq!(rects.panel.left, 220 + SHELL_CONTENT_INSET);
        assert_eq!(rects.panel.top, SHELL_TOP_BAR_HEIGHT);
        assert_eq!(client.right - rects.panel.right, SHELL_CONTENT_INSET);
        assert_eq!(client.bottom - rects.panel.bottom, SHELL_CONTENT_INSET);
        assert_eq!(rects.content.top, SHELL_TOP_BAR_HEIGHT);

        layout.navigation_bar = Some(WindowsShellNavigationBarLayout {
            visible: true,
            title: "Page".to_string(),
            background_color: 0,
            text_color: 0,
            show_back_button: false,
            show_home_button: false,
            height: 38,
        });
        let rects = compute_chrome_rects(client, &layout);
        assert_eq!(rects.workspace.top, SHELL_TOP_BAR_HEIGHT);
        assert_eq!(rects.navigation_bar.unwrap().top, SHELL_TOP_BAR_HEIGHT);
    }

    #[test]
    fn attached_aside_has_only_an_internal_split_gap() {
        let layout = WindowsShellWindowLayout::default();
        let client = RECT {
            left: 0,
            top: 0,
            right: 1024,
            bottom: 768,
        };
        let panel = WindowsChromePanelLayoutInput {
            panel_id: "browser-aside".to_string(),
            webtag_key: "app.lingxia.browser:aside".to_string(),
            position: WindowsPanelPosition::Right,
            requested_size: Some(320),
            docked: true,
            maximized: false,
        };

        let attached = compute_attached_layout(client, &layout, &[panel]);
        let aside = &attached.panels[0];

        assert_eq!(attached.main_region.top, SHELL_TOP_BAR_HEIGHT);
        assert_eq!(aside.rect.top, SHELL_TOP_BAR_HEIGHT);
        assert_eq!(aside.rect.right, client.right - SHELL_CONTENT_INSET);
        assert_eq!(aside.rect.bottom, client.bottom - SHELL_CONTENT_INSET);
        assert_eq!(
            aside.rect.left - attached.main_region.right,
            SHELL_PANEL_GAP
        );
        let handle = aside.resize_handle.unwrap();
        assert_eq!(handle.left, attached.main_region.right);
        assert_eq!(handle.right, aside.rect.left);
        assert_eq!(
            attached.main_region.bottom,
            client.bottom - SHELL_CONTENT_INSET
        );
    }

    #[test]
    fn lxapp_and_web_tabs_share_top_level_geometry() {
        let tabbar = WindowsShellTabBarLayout {
            visible: true,
            position: WindowsShellTabBarPosition::Left,
            dimension: 220,
            app_name: "App".to_string(),
            app_icon_path: String::new(),
            group_id: "app".to_string(),
            group_active: true,
            group_closable: false,
            group_order_index: 0,
            color: 0,
            selected_color: 0,
            background_color: 0,
            background_transparent: true,
            border_color: 0,
            selected_index: -1,
            items: Vec::new(),
            collapsed: false,
            icon_rail: false,
            items_api_hidden: false,
            items_collapsed: false,
            activator_footer_height: 0,
            main_scroll_offset: 0,
            activator_scroll_row: 0,
            auxiliary_items: vec![WindowsShellAuxiliaryItemLayout {
                id: "tab-1".to_string(),
                title: "Web".to_string(),
                active: false,
                pinned: false,
                closable: true,
                icon_png: None,
                icon_path: String::new(),
            }],
            show_auxiliary_add: false,
            header_actions: Vec::new(),
        };
        let sidebar = RECT {
            left: 0,
            top: 0,
            right: 220,
            bottom: 768,
        };
        let lxapp = sidebar_group_rect(sidebar, &tabbar, 0);
        let web = sidebar_auxiliary_rects(sidebar, &tabbar, 0, sidebar.bottom)
            .unwrap()
            .items[0]
            .1;

        assert_eq!((lxapp.left, lxapp.right), (web.left, web.right));
        assert_eq!(lxapp.bottom - lxapp.top, SIDEBAR_ITEM_HEIGHT);
        assert_eq!(web.bottom - web.top, SIDEBAR_ITEM_HEIGHT);
        assert_eq!(
            sidebar_top_level_icon_rect(lxapp, SIDEBAR_ICON_SIZE).left,
            sidebar_top_level_icon_rect(web, SIDEBAR_ICON_SIZE).left
        );
    }

    #[test]
    fn pinning_an_existing_auxiliary_id_repaints_its_old_geometry() {
        let old = WindowsShellTabBarLayout {
            visible: true,
            position: WindowsShellTabBarPosition::Left,
            dimension: 220,
            app_name: "App".to_string(),
            app_icon_path: String::new(),
            group_id: "app".to_string(),
            group_active: true,
            group_closable: false,
            group_order_index: 0,
            color: 0,
            selected_color: 0,
            background_color: 0,
            background_transparent: true,
            border_color: 0,
            selected_index: -1,
            items: Vec::new(),
            collapsed: false,
            icon_rail: false,
            items_api_hidden: false,
            items_collapsed: false,
            activator_footer_height: 0,
            main_scroll_offset: 0,
            activator_scroll_row: 0,
            auxiliary_items: vec![WindowsShellAuxiliaryItemLayout {
                id: "lxapp:chat".to_string(),
                title: "Chat".to_string(),
                active: true,
                pinned: false,
                closable: true,
                icon_png: None,
                icon_path: String::new(),
            }],
            show_auxiliary_add: true,
            header_actions: Vec::new(),
        };
        let mut pinned = old.clone();
        pinned.auxiliary_items[0].pinned = true;
        pinned.auxiliary_items[0].closable = false;

        assert!(tabbar_requires_full_repaint(&old, &pinned));
    }

    #[test]
    fn collapsed_tabbar_click_keeps_its_group_owner() {
        let command = collapsed_sidebar_tabbar_click_command("home", 2);

        assert_eq!(command.payload["group"], "home");
        assert_eq!(command.payload["index"], 2);
    }
}

#[cfg(test)]
mod corner_tests {
    use super::{
        WindowsChromePanelLayoutInput, WindowsPanelPosition, WindowsShellNavigationBarLayout,
        WindowsShellWindowLayout, compute_attached_layout, compute_chrome_rects,
        workspace_corner_radii,
    };
    use windows::Win32::Foundation::RECT;

    const R: i32 = super::SHELL_CONTENT_RADIUS;

    fn client() -> RECT {
        RECT {
            left: 0,
            top: 0,
            right: 1024,
            bottom: 768,
        }
    }

    fn panel(id: &str, position: WindowsPanelPosition) -> WindowsChromePanelLayoutInput {
        WindowsChromePanelLayoutInput {
            panel_id: id.to_string(),
            webtag_key: format!("{id}:webtag"),
            position,
            requested_size: Some(240),
            docked: true,
            maximized: false,
        }
    }

    #[test]
    fn main_only_rounds_all_four_workspace_corners() {
        let layout = WindowsShellWindowLayout::default();
        let rects = compute_chrome_rects(client(), &layout);

        assert_eq!(
            workspace_corner_radii(rects.content, rects.workspace, R),
            [R, R, R, R]
        );
    }

    #[test]
    fn navigation_bar_band_owns_main_top_corners() {
        let layout = WindowsShellWindowLayout {
            navigation_bar: Some(WindowsShellNavigationBarLayout {
                visible: true,
                title: "Page".to_string(),
                background_color: 0,
                text_color: 0,
                show_back_button: false,
                show_home_button: false,
                height: 38,
            }),
            ..Default::default()
        };
        let rects = compute_chrome_rects(client(), &layout);
        let navbar = rects.navigation_bar.unwrap();

        assert_eq!(
            workspace_corner_radii(navbar, rects.workspace, R),
            [R, R, 0, 0]
        );
        assert_eq!(
            workspace_corner_radii(rects.content, rects.workspace, R),
            [0, 0, R, R]
        );
    }

    #[test]
    fn right_aside_takes_right_exterior_corners() {
        let layout = WindowsShellWindowLayout::default();
        let attached = compute_attached_layout(
            client(),
            &layout,
            &[panel("aside", WindowsPanelPosition::Right)],
        );
        let silhouette = compute_chrome_rects(client(), &layout).workspace;

        assert_eq!(
            workspace_corner_radii(attached.main, silhouette, R),
            [R, 0, 0, R]
        );
        assert_eq!(
            workspace_corner_radii(attached.panels[0].rect, silhouette, R),
            [0, R, R, 0]
        );
    }

    #[test]
    fn left_aside_mirrors_corner_ownership() {
        let layout = WindowsShellWindowLayout::default();
        let attached = compute_attached_layout(
            client(),
            &layout,
            &[panel("aside", WindowsPanelPosition::Left)],
        );
        let silhouette = compute_chrome_rects(client(), &layout).workspace;

        assert_eq!(
            workspace_corner_radii(attached.main, silhouette, R),
            [0, R, R, 0]
        );
        assert_eq!(
            workspace_corner_radii(attached.panels[0].rect, silhouette, R),
            [R, 0, 0, R]
        );
    }

    #[test]
    fn bottom_aside_owns_full_width_bottom_corners() {
        let layout = WindowsShellWindowLayout::default();
        let attached = compute_attached_layout(
            client(),
            &layout,
            &[panel("terminal", WindowsPanelPosition::Bottom)],
        );
        let silhouette = compute_chrome_rects(client(), &layout).workspace;

        assert_eq!(
            workspace_corner_radii(attached.main, silhouette, R),
            [R, R, 0, 0]
        );
        assert_eq!(
            workspace_corner_radii(attached.panels[0].rect, silhouette, R),
            [0, 0, R, R]
        );
    }

    #[test]
    fn browser_aside_header_leaves_webview_top_square() {
        let layout = WindowsShellWindowLayout::default();
        let browser = WindowsChromePanelLayoutInput {
            panel_id: "browser-aside".to_string(),
            webtag_key: "app.lingxia.browser:aside".to_string(),
            position: WindowsPanelPosition::Right,
            requested_size: Some(320),
            docked: true,
            maximized: false,
        };
        let attached = compute_attached_layout(client(), &layout, &[browser]);
        let silhouette = compute_chrome_rects(client(), &layout).workspace;
        let aside = &attached.panels[0];
        let header = aside.header_rect.unwrap();

        // The header band owns the panel's exterior top corner; the webview
        // below it (top clamped to the header bottom) keeps only the bottom.
        assert_eq!(workspace_corner_radii(header, silhouette, R), [0, R, 0, 0]);
        let webview = RECT {
            top: header.bottom,
            ..aside.rect
        };
        assert_eq!(workspace_corner_radii(webview, silhouette, R), [0, 0, R, 0]);
    }

    #[test]
    fn stacked_side_asides_round_only_the_outermost() {
        let layout = WindowsShellWindowLayout::default();
        let attached = compute_attached_layout(
            client(),
            &layout,
            &[
                panel("a-outer", WindowsPanelPosition::Right),
                panel("b-inner", WindowsPanelPosition::Right),
            ],
        );
        let silhouette = compute_chrome_rects(client(), &layout).workspace;
        let outer = attached
            .panels
            .iter()
            .find(|panel| panel.panel_id == "a-outer")
            .unwrap();
        let inner = attached
            .panels
            .iter()
            .find(|panel| panel.panel_id == "b-inner")
            .unwrap();

        assert_eq!(
            workspace_corner_radii(outer.rect, silhouette, R),
            [0, R, R, 0]
        );
        assert_eq!(workspace_corner_radii(inner.rect, silhouette, R), [0; 4]);
    }

    #[test]
    fn side_plus_bottom_asides_split_bottom_corners() {
        let layout = WindowsShellWindowLayout::default();
        let attached = compute_attached_layout(
            client(),
            &layout,
            &[
                panel("aside", WindowsPanelPosition::Right),
                panel("terminal", WindowsPanelPosition::Bottom),
            ],
        );
        let silhouette = compute_chrome_rects(client(), &layout).workspace;
        let aside = attached
            .panels
            .iter()
            .find(|panel| panel.panel_id == "aside")
            .unwrap();
        let terminal = attached
            .panels
            .iter()
            .find(|panel| panel.panel_id == "terminal")
            .unwrap();

        // The side aside spans the full workspace height, so it owns the
        // right corners; the bottom panel spans only the narrowed main region
        // and owns the remaining bottom-left arc.
        assert_eq!(
            workspace_corner_radii(aside.rect, silhouette, R),
            [0, R, R, 0]
        );
        assert_eq!(
            workspace_corner_radii(terminal.rect, silhouette, R),
            [0, 0, 0, R]
        );
        assert_eq!(
            workspace_corner_radii(attached.main, silhouette, R),
            [R, 0, 0, 0]
        );
    }

    #[test]
    fn maximized_panel_owns_all_corners_and_main_collapses() {
        let layout = WindowsShellWindowLayout::default();
        let mut maximized = panel("terminal", WindowsPanelPosition::Bottom);
        maximized.maximized = true;
        let attached = compute_attached_layout(client(), &layout, &[maximized]);
        let silhouette = compute_chrome_rects(client(), &layout).workspace;

        assert_eq!(
            workspace_corner_radii(attached.panels[0].rect, silhouette, R),
            [R, R, R, R]
        );
        // The collapsed main region is zero-area and must clip square.
        assert_eq!(workspace_corner_radii(attached.main, silhouette, R), [0; 4]);
    }

    #[test]
    fn floating_bottom_panel_never_coincides_with_the_silhouette() {
        let layout = WindowsShellWindowLayout::default();
        let mut floating = panel("panel", WindowsPanelPosition::Bottom);
        floating.docked = false;
        let attached = compute_attached_layout(client(), &layout, &[floating]);
        let silhouette = compute_chrome_rects(client(), &layout).workspace;

        // The floating card keeps a bottom margin, so the coincidence rule
        // yields squares; the caller assigns the free-standing card radius.
        assert_eq!(
            workspace_corner_radii(attached.panels[0].rect, silhouette, R),
            [0; 4]
        );
    }

    #[test]
    fn device_frame_surface_rounds_screen_bottom_corners_only() {
        // Device-frame mode: silhouette = the content window's client rect,
        // radius = the device screen radius; the webview starts below the
        // simulated status-bar strip, whose overlay owns the top arcs.
        let screen = RECT {
            left: 0,
            top: 0,
            right: 393,
            bottom: 852,
        };
        let webview = RECT { top: 54, ..screen };

        assert_eq!(workspace_corner_radii(webview, screen, 54), [0, 0, 54, 54]);
    }

    #[test]
    fn tiny_surface_clamps_radius() {
        let silhouette = RECT {
            left: 0,
            top: 0,
            right: 400,
            bottom: 12,
        };

        assert_eq!(
            workspace_corner_radii(silhouette, silhouette, R),
            [6, 6, 6, 6]
        );
    }

    #[test]
    fn zero_area_surface_has_square_clip() {
        let silhouette = RECT {
            left: 0,
            top: 0,
            right: 400,
            bottom: 300,
        };
        let collapsed = RECT {
            left: 0,
            top: 300,
            right: 400,
            bottom: 300,
        };

        assert_eq!(workspace_corner_radii(collapsed, silhouette, R), [0; 4]);
        assert_eq!(
            workspace_corner_radii(RECT::default(), silhouette, R),
            [0; 4]
        );
    }
}
