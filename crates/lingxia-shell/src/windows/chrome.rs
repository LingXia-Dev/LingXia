//! Shell window chrome: chrome rect computation, product chrome drawing
//! orchestration, and hit-testing.
//!
//! Moved out of `lingxia-webview` so the webview crate stays generic; this
//! file is pure product policy registered through the
//! [`WindowsChromeRenderer`] seam.

use std::sync::{Arc, OnceLock};

use lingxia_webview::platform::windows::{
    WindowsChromeHit, WindowsChromePanel, WindowsChromeRenderer, WindowsChromeState,
    WindowsFrameButton, WindowsNativePanelContent, WindowsNativePanelKind,
    WindowsNavigationBarLayout, WindowsTabBarLayout, WindowsTabBarPosition, WindowsWindowLayout,
    post_to_window_thread, set_windows_chrome_renderer,
};
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
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
mod native_panel;
mod sidebar;
mod top_bar;
pub(super) use drawing::*;
use native_panel::*;
use sidebar::*;
pub use top_bar::begin_address_edit;
use top_bar::*;

/// GlobalNavButton (hamburger) — the closest Fluent match to Arc's
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

/// The shell's window chrome renderer, registered into `lingxia-webview`.
struct ShellChromeRenderer;

impl WindowsChromeRenderer for ShellChromeRenderer {
    fn content_rect(&self, client: RECT, layout: &WindowsWindowLayout) -> RECT {
        compute_chrome_rects(client, layout).content
    }

    fn panel_gap(&self) -> i32 {
        SHELL_PANEL_PADDING
    }

    fn panel_corner_radius(&self) -> i32 {
        SHELL_PANEL_RADIUS
    }

    fn card_corner_color(&self) -> Option<COLORREF> {
        // Attached cards are rounded by lingxia-webview's per-pixel-alpha
        // corner-cap overlays; this is the chrome background they blend
        // the card corners into.
        Some(rgb_to_colorref(SHELL_WINDOW_BACKGROUND))
    }

    fn maximized_panel_rect(&self, client: RECT, _layout: &WindowsWindowLayout) -> RECT {
        // A maximized panel takes the whole app area below the caption
        // strip — covering the sidebar — so only the frame buttons stay
        // reachable while maximized.
        RECT {
            left: client.left,
            top: (client.top + SHELL_TOP_BAR_HEIGHT).min(client.bottom),
            right: client.right,
            bottom: client.bottom,
        }
    }

    fn paint(&self, hdc: HDC, state: &WindowsChromeState) {
        // An active inline text edit (e.g. a terminal tab rename) is a real
        // EDIT child window; the hosts do not use WS_CLIPCHILDREN, so its
        // rect is clipped out to keep chrome repaints from drawing over it.
        let saved = unsafe { SaveDC(hdc) };
        super::text_input::exclude_active_inline_edit(hdc, state.hwnd);
        draw_window_chrome(hdc, state);
        unsafe {
            let _ = RestoreDC(hdc, saved);
        }
    }

    fn hit_test(&self, state: &WindowsChromeState, point: (i32, i32)) -> Option<WindowsChromeHit> {
        chrome_hit_test(state, point)
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

#[derive(Debug, Clone, Copy)]
pub(super) struct ChromeRects {
    pub(super) content: RECT,
    pub(super) panel: RECT,
    pub(super) top_bar: RECT,
    pub(super) navigation_bar: Option<RECT>,
    pub(super) tab_bar: Option<RECT>,
}

pub(super) fn compute_chrome_rects(client: RECT, layout: &WindowsWindowLayout) -> ChromeRects {
    let mut content = client;
    let mut top_bar_left = client.left;
    let mut top_bar_right = client.right;
    let tab_bar = layout
        .tab_bar
        .as_ref()
        .filter(|tabbar| tabbar.visible && !tabbar.items.is_empty() && tabbar.dimension > 0)
        .map(|tabbar| match tabbar.position {
            WindowsTabBarPosition::Left => {
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
            WindowsTabBarPosition::Right => {
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
            WindowsTabBarPosition::Bottom => {
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
        Some(WindowsTabBarPosition::Left | WindowsTabBarPosition::Right)
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

/// Chrome rects for a concrete window state: when the host has attached
/// panels, the navigation bar is clipped to the main content card.
pub(super) fn chrome_rects_for_state(state: &WindowsChromeState) -> ChromeRects {
    let mut rects = compute_chrome_rects(state.client, &state.layout);
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

pub(super) fn draw_window_chrome(hdc: HDC, state: &WindowsChromeState) {
    let client = state.client;
    let layout = &state.layout;
    let rects = chrome_rects_for_state(state);

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
    draw_top_bar_controls(hdc, state, &rects);
    draw_panel_activators(hdc, client, &rects, layout);
    draw_maximized_native_panels(hdc, state);
    draw_window_frame_buttons(hdc, state);
}

pub(super) fn chrome_hit_test(
    state: &WindowsChromeState,
    point: (i32, i32),
) -> Option<WindowsChromeHit> {
    let client = state.client;
    let layout = &state.layout;
    let rects = chrome_rects_for_state(state);

    if let Some((button, _)) = window_frame_button_rects(client)
        .into_iter()
        .find(|(_, rect)| rect_contains(rect, point))
    {
        return Some(WindowsChromeHit::FrameButton(button));
    }

    if let Some(attached) = &state.attached {
        for panel in &attached.panels {
            if panel
                .native
                .as_ref()
                .is_some_and(|native| native.kind == WindowsNativePanelKind::Terminal)
                && rect_contains(&panel.rect, point)
            {
                // Header elements (tabs, new-tab, maximize) win over the
                // generic focus hit; the rest of the panel focuses it.
                if let Some(hit) = terminal_header_hit_test(panel, point) {
                    return Some(hit);
                }
                return Some(WindowsChromeHit::NativePanel {
                    panel_id: panel.panel_id.clone(),
                });
            }
        }
    }

    let controls = top_bar_controls(client, rects.top_bar, layout);
    if let Some(toggle) = controls.sidebar_toggle
        && rect_contains(&toggle, point)
    {
        return Some(WindowsChromeHit::SidebarToggle);
    }
    if let Some(back) = controls.nav_back
        && rect_contains(&back, point)
    {
        return Some(WindowsChromeHit::BrowserNavBack);
    }
    if let Some(forward) = controls.nav_forward
        && rect_contains(&forward, point)
    {
        return Some(WindowsChromeHit::BrowserNavForward);
    }
    if let Some(reload) = controls.nav_reload
        && rect_contains(&reload, point)
    {
        return Some(WindowsChromeHit::BrowserNavReload);
    }
    if let Some(address) = controls.address
        && rect_contains(&address, point)
    {
        return Some(WindowsChromeHit::BrowserAddressBar);
    }

    if !address_bar_visible(layout)
        && let (Some(navbar), Some(navbar_rect)) = (&layout.navigation_bar, rects.navigation_bar)
        && rect_contains(&navbar_rect, point)
    {
        let buttons_left = navbar_buttons_left(client, rects.top_bar, layout, navbar_rect);
        if navbar.show_back_button
            && rect_contains(&nav_button_rect(navbar_rect, buttons_left, 0), point)
        {
            return Some(WindowsChromeHit::NavigationBack);
        }
        let home_index = if navbar.show_back_button { 1 } else { 0 };
        if navbar.show_home_button
            && rect_contains(
                &nav_button_rect(navbar_rect, buttons_left, home_index),
                point,
            )
        {
            return Some(WindowsChromeHit::NavigationHome);
        }
        return Some(WindowsChromeHit::Caption);
    }

    for (panel_id, rect) in panel_activator_rects(client, &rects, layout) {
        if rect_contains(&rect, point) {
            return Some(WindowsChromeHit::PanelActivator { panel_id });
        }
    }

    if let (Some(tabbar), Some(tabbar_rect)) = (&layout.tab_bar, rects.tab_bar)
        && rect_contains(&tabbar_rect, point)
    {
        let sidebar = matches!(
            tabbar.position,
            WindowsTabBarPosition::Left | WindowsTabBarPosition::Right
        );
        if sidebar {
            if rect_contains(&sidebar_group_chevron_rect(tabbar_rect), point) {
                return Some(WindowsChromeHit::SidebarGroupToggle {
                    group: tabbar.group_id.clone(),
                });
            }
            for (action_id, action_rect) in sidebar_header_action_rects(tabbar_rect, tabbar) {
                if rect_contains(&action_rect, point) {
                    return Some(WindowsChromeHit::SidebarAction { action_id });
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
                    return Some(WindowsChromeHit::TabBarItem { index });
                }
            }
        }
        if sidebar && let Some(hit) = sidebar_browser_hit_test(tabbar_rect, tabbar, point) {
            return Some(hit);
        }
        return Some(WindowsChromeHit::Chrome);
    }

    if rect_contains(&rects.top_bar, point) {
        return Some(WindowsChromeHit::Caption);
    }

    None
}

pub(super) fn draw_content_cards(hdc: HDC, state: &WindowsChromeState, rects: &ChromeRects) {
    if let Some(attached) = &state.attached {
        draw_content_card(hdc, attached.main);
        // A docked panel sits flush under the main card; square the card's
        // bottom corners so the shared seam has no notches. (The webview
        // layer hides the card's bottom corner caps for the same reason.)
        if attached.panels.iter().any(|panel| panel.docked) {
            square_card_bottom_corners(hdc, attached.main);
        }
        for panel in &attached.panels {
            // Terminal panels paint their own full-bleed dark card.
            if panel
                .native
                .as_ref()
                .is_some_and(|native| native.kind == WindowsNativePanelKind::Terminal)
            {
                continue;
            }
            draw_content_card(hdc, panel.rect);
        }
        for panel in &attached.panels {
            // Maximized panels cover the sidebar and are repainted after
            // the sidebar pass in `draw_window_chrome`.
            if panel.native.is_some() && !panel_is_maximized(panel) {
                draw_native_panel_content(hdc, state.hwnd, panel);
            }
        }
        // Attached card corners are rounded by lingxia-webview's layered
        // corner-cap overlays (see `card_corner_color` above); the cards
        // themselves are plain filled rounded rects.
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

pub(super) fn rect_width(rect: &RECT) -> i32 {
    (rect.right - rect.left).max(0)
}

pub(super) fn rect_height(rect: &RECT) -> i32 {
    (rect.bottom - rect.top).max(0)
}

pub(super) fn rect_contains(rect: &RECT, point: (i32, i32)) -> bool {
    point.0 >= rect.left && point.0 < rect.right && point.1 >= rect.top && point.1 < rect.bottom
}
