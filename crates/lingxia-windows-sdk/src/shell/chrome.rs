//! Shell window chrome: chrome rect computation, product chrome drawing
//! orchestration, and hit-testing.
//!
//! Moved out of `lingxia-webview` so the webview crate stays generic; this
//! file is pure product policy registered through the
//! [`WindowsChromeRenderer`] seam.

use std::sync::{Arc, OnceLock};

use lingxia_windows_host::post_to_window_thread;
use lingxia_windows_host::{
    WindowsChromeAttachedLayout, WindowsChromeCommand, WindowsChromeHit, WindowsChromePanel,
    WindowsChromePanelLayout, WindowsChromePanelLayoutInput, WindowsChromeRenderer,
    WindowsChromeState, WindowsFrameButton, WindowsHostPanelContent, WindowsPanelPosition,
    WindowsWindowLayout, set_windows_chrome_renderer,
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
pub use top_bar::begin_address_edit;
use top_bar::*;

/// GlobalNavButton (hamburger): the closest Fluent match to Arc's
/// sidebar collapse/expand toggle.
pub(super) const GLYPH_SIDEBAR_TOGGLE: &str = "\u{e700}";

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

pub(super) const SIDEBAR_HEADER_HEIGHT: i32 = 66;

pub(super) const SIDEBAR_ITEM_HEIGHT: i32 = 34;

pub(super) const SIDEBAR_ITEM_GAP: i32 = 4;

pub(super) const SIDEBAR_ITEM_INSET: i32 = 10;

pub(super) const SIDEBAR_FOOTER_HEIGHT: i32 = 46;

/// Vertical padding above and below the browser-section separator line.
pub(super) const SIDEBAR_BROWSER_SECTION_GAP: i32 = 8;

/// Width of the close-glyph hit area at the trailing edge of a browser row.
pub(super) const SIDEBAR_BROWSER_CLOSE_SIZE: i32 = 22;

/// Close glyph for browser tab rows (multiplication X).
pub(super) const GLYPH_TAB_CLOSE: &str = "\u{2715}";

pub(super) const SIDEBAR_ICON_SIZE: i32 = 16;

/// Edge length of the favicon drawn on a sidebar browser-tab row.
pub(super) const SIDEBAR_FAVICON_SIZE: i32 = 16;

/// Gap between a browser row's favicon and its title text.
pub(super) const SIDEBAR_FAVICON_TEXT_GAP: i32 = 6;

pub(super) const PANEL_ACTIVATOR_SIZE: i32 = 28;

pub(super) const PANEL_ACTIVATOR_ICON_SIZE: i32 = 16;

pub(super) const PANEL_ACTIVATOR_GAP: i32 = 4;

pub(super) const PANEL_ACTIVATOR_MARGIN: i32 = 6;

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
    pub(super) const SIDEBAR_TOGGLE: &str = "sidebar.toggle";
    pub(super) const SIDEBAR_GROUP_TOGGLE: &str = "sidebar.group.toggle";
    pub(super) const SIDEBAR_ACTION: &str = "sidebar.action";
}

pub(super) fn chrome_command(
    id: impl Into<String>,
    payload: serde_json::Value,
) -> WindowsChromeHit {
    WindowsChromeHit::Command(WindowsChromeCommand::new(id).with_payload(payload))
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
        || old_tabbar.border_color != new_tabbar.border_color
        || old_tabbar.items != new_tabbar.items
        || old_tabbar.collapsed != new_tabbar.collapsed
        || old_tabbar.items_collapsed != new_tabbar.items_collapsed
        || old_tabbar.show_auxiliary_add != new_tabbar.show_auxiliary_add
        || old_tabbar.header_actions != new_tabbar.header_actions
        || !same_auxiliary_rows(old_tabbar, new_tabbar)
}

fn same_auxiliary_rows(
    old_tabbar: &WindowsShellTabBarLayout,
    new_tabbar: &WindowsShellTabBarLayout,
) -> bool {
    old_tabbar.auxiliary_items.len() == new_tabbar.auxiliary_items.len()
        && old_tabbar
            .auxiliary_items
            .iter()
            .zip(&new_tabbar.auxiliary_items)
            .all(|(old_item, new_item)| {
                old_item.id == new_item.id
                    && old_item.title == new_item.title
                    && old_item.icon_png == new_item.icon_png
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

    let activator_rects = panel_activator_rects(client, rects, new_layout);
    let mut dirty = Vec::new();
    for (index, (old_activator, new_activator)) in old_layout
        .panel_activators
        .iter()
        .zip(&new_layout.panel_activators)
        .enumerate()
    {
        if old_activator == new_activator {
            continue;
        }
        if let Some((_, rect)) = activator_rects.get(index) {
            push_dirty_rect(&mut dirty, *rect, client);
        }
    }
    if old_layout.panel_activators.len() != new_layout.panel_activators.len() {
        for (_, rect) in activator_rects {
            push_dirty_rect(&mut dirty, rect, client);
        }
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
    let mut content = client;
    let mut top_bar_left = client.left;
    let mut top_bar_right = client.right;
    let tab_bar = layout
        .tab_bar
        .as_ref()
        .filter(|tabbar| tabbar.visible && !tabbar.items.is_empty() && tabbar.dimension > 0)
        .map(|tabbar| match tabbar.position {
            WindowsShellTabBarPosition::Left => {
                // A collapsed sidebar keeps the side-card layout (insets,
                // top bar) at width 0; the top-bar toggle re-expands it.
                let width = if tabbar.collapsed {
                    0
                } else {
                    tabbar.dimension.max(SHELL_SIDEBAR_WIDTH)
                };
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
                let width = if tabbar.collapsed {
                    0
                } else {
                    tabbar.dimension.max(SHELL_SIDEBAR_WIDTH)
                };
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
                content.bottom = top;
                rect
            }
        });

    if !matches!(
        layout.tab_bar.as_ref().map(|tabbar| tabbar.position),
        Some(WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right)
    ) {
        content.top += SHELL_TOP_BAR_HEIGHT;
        top_bar_left = content.left;
        top_bar_right = content.right;
    }

    content = normalize_rect(content);
    let panel = content;
    let top_bar = normalize_rect(RECT {
        left: top_bar_left,
        top: client.top,
        right: top_bar_right,
        bottom: (client.top + SHELL_TOP_BAR_HEIGHT).min(client.bottom),
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

fn compute_attached_layout(
    client: RECT,
    layout: &WindowsShellWindowLayout,
    panels: &[WindowsChromePanelLayoutInput],
) -> WindowsChromeAttachedLayout {
    let mut main = compute_chrome_rects(client, layout).content;
    let mut out = Vec::new();

    if let Some(maximized) = panels.iter().find(|panel| panel.docked && panel.maximized) {
        out.push(WindowsChromePanelLayout {
            panel_id: maximized.panel_id.clone(),
            webtag_key: maximized.webtag_key.clone(),
            rect: shell_maximized_panel_rect(client),
            resize_handle: None,
        });
        main.bottom = main.top;
        return WindowsChromeAttachedLayout { main, panels: out };
    }

    let mut ordered = panels.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|panel| match panel.position {
        WindowsPanelPosition::Left | WindowsPanelPosition::Right => 0,
        WindowsPanelPosition::Bottom => 1,
    });

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

        out.push(WindowsChromePanelLayout {
            panel_id: panel.panel_id.clone(),
            webtag_key: panel.webtag_key.clone(),
            rect: normalize_rect(rect),
            resize_handle,
        });
    }

    WindowsChromeAttachedLayout {
        main: normalize_rect(main),
        panels: out,
    }
}

fn attached_panel_size(
    panel: &WindowsChromePanelLayoutInput,
    content: RECT,
    default_size: i32,
) -> i32 {
    let requested = panel.requested_size.unwrap_or(default_size).max(1);
    let available = match panel.position {
        WindowsPanelPosition::Bottom => rect_height(&content),
        WindowsPanelPosition::Left | WindowsPanelPosition::Right => rect_width(&content),
    };
    if available <= 0 {
        return 0;
    }

    let max_with_main = match panel.position {
        WindowsPanelPosition::Bottom => available - SHELL_PANEL_PADDING - ATTACHED_MAIN_MIN_HEIGHT,
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

fn shell_maximized_panel_rect(client: RECT) -> RECT {
    normalize_rect(RECT {
        left: client.left,
        top: (client.top + SHELL_TOP_BAR_HEIGHT).min(client.bottom),
        right: client.right,
        bottom: client.bottom,
    })
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

    fill_rect(hdc, client, SHELL_WINDOW_BACKGROUND);
    draw_shell_top_bar(hdc, &rects);
    draw_content_cards(hdc, state, &rects);

    // The address bar owns the top bar while a browser surface is
    // presented; the lxapp navigation bar yields.
    if !address_bar_visible(layout)
        && let (Some(navbar), Some(navbar_rect)) = (&layout.navigation_bar, rects.navigation_bar)
    {
        let buttons_left = navbar_buttons_left(client, rects.top_bar, layout, navbar_rect);
        draw_navigation_bar(hdc, navbar_rect, buttons_left, navbar);
    }
    if let (Some(tabbar), Some(tabbar_rect)) = (&layout.tab_bar, rects.tab_bar) {
        draw_tab_bar(hdc, tabbar_rect, tabbar);
    }
    // Painted after the navigation bar: the navbar fills the whole top bar
    // with its own background, and the toggle/address controls sit on top.
    draw_top_bar_controls(hdc, state, &rects, layout);
    draw_panel_activators(hdc, client, &rects, layout);
    draw_maximized_native_panels(hdc, state);
    draw_window_frame_buttons(hdc, state);
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

    if let Some((button, _)) = window_frame_button_rects(client)
        .into_iter()
        .find(|(_, rect)| rect_contains(rect, point))
    {
        return Some(WindowsChromeHit::FrameButton(button));
    }

    if let Some(hit) = maximized_native_panel_hit(state, point) {
        return Some(hit);
    }

    let controls = top_bar_controls(client, rects.top_bar, layout);
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
        SHELL_PANEL_BACKGROUND,
    );
}

pub(super) fn draw_content_card(hdc: HDC, rect: RECT) {
    if rect_width(&rect) > 0 && rect_height(&rect) > 0 {
        // White card on the gray window background: the arc must be
        // anti-aliased (and match the corner-cap radius of webview cards).
        fill_round_rect_aa(hdc, rect, SHELL_PANEL_RADIUS, SHELL_PANEL_BACKGROUND);
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
