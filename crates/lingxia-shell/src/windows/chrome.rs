//! Shell window chrome: chrome rect computation, all GDI drawing (top bar,
//! tab bar, sidebar, navigation bar, panels, text, colors), and hit-testing.
//!
//! Moved out of `lingxia-webview` so the webview crate stays generic; this
//! file is pure product policy registered through the
//! [`WindowsChromeRenderer`] seam.

use std::ffi::c_void;
use std::sync::{Arc, OnceLock};

use lingxia_webview::platform::windows::{
    WindowsChromeHit, WindowsChromePanel, WindowsChromeRenderer, WindowsChromeState,
    WindowsFrameButton, WindowsNativePanelContent, WindowsNativePanelKind,
    WindowsNavigationBarLayout, WindowsTabBarLayout, WindowsTabBarPosition, WindowsWindowLayout,
    cached_png_bytes_icon_handle, cached_png_icon_handle, post_to_window_thread,
    set_windows_chrome_renderer,
};
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateSolidBrush, DEFAULT_CHARSET,
    DEFAULT_PITCH, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_SINGLELINE, DT_VCENTER, DeleteObject,
    DrawTextW, FF_SWISS, FillRect, GetDeviceCaps, GetStockObject, GetTextFaceW, HDC, HFONT,
    HGDIOBJ, LOGPIXELSY, NULL_PEN, OUT_DEFAULT_PRECIS, RestoreDC, RoundRect, SaveDC, SelectObject,
    SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::Graphics::GdiPlus;
use windows::Win32::UI::WindowsAndMessaging::{self, HICON};
use windows::core::{PCWSTR, w};

use super::style::*;

mod top_bar;
mod native_panel;
pub use top_bar::begin_address_edit;
use native_panel::*;
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
            && rect_contains(&nav_button_rect(navbar_rect, buttons_left, home_index), point)
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

pub(super) fn draw_tab_bar(hdc: HDC, rect: RECT, tabbar: &WindowsTabBarLayout) {
    if matches!(
        tabbar.position,
        WindowsTabBarPosition::Left | WindowsTabBarPosition::Right
    ) {
        draw_sidebar_tab_bar(hdc, rect, tabbar);
        return;
    }

    fill_rect(hdc, rect, tabbar.background_color);
    draw_tabbar_border(hdc, rect, tabbar);

    let count = tabbar.items.len();
    if count == 0 {
        return;
    }

    for (index, item) in tabbar.items.iter().enumerate() {
        let item_rect = tab_item_rect(rect, tabbar.position, count, index);
        let selected = tabbar.selected_index == index as i32;
        if selected {
            fill_rect(
                hdc,
                inset_rect(item_rect, 4, 5),
                SHELL_TAB_SELECTED_BACKGROUND,
            );
        }

        let text_color = if selected {
            tabbar.selected_color
        } else {
            tabbar.color
        };
        let mut label_rect = inset_rect(item_rect, 6, 4);
        if matches!(tabbar.position, WindowsTabBarPosition::Bottom) {
            label_rect.top += 6;
        }
        draw_text(hdc, &item.text, label_rect, text_color, DT_CENTER);

        if let Some(badge) = item.badge.as_ref().filter(|badge| !badge.is_empty()) {
            draw_badge(hdc, item_rect, badge);
        } else if item.has_red_dot {
            draw_red_dot(hdc, item_rect);
        }
    }
}

pub(super) fn draw_sidebar_tab_bar(hdc: HDC, rect: RECT, tabbar: &WindowsTabBarLayout) {
    if rect_width(&rect) == 0 {
        return;
    }
    fill_rect(hdc, rect, SHELL_SIDEBAR_BACKGROUND);

    let title = if tabbar.app_name.trim().is_empty() {
        "LXAPP".to_string()
    } else {
        tabbar.app_name.to_ascii_uppercase()
    };
    let chevron_rect = sidebar_group_chevron_rect(rect);
    let header_rect = RECT {
        left: rect.left + SIDEBAR_ITEM_INSET + 2,
        top: rect.top + 22,
        right: chevron_rect.left - 4,
        bottom: rect.top + SIDEBAR_HEADER_HEIGHT,
    };
    draw_text(hdc, &title, header_rect, SHELL_SIDEBAR_HEADER_TEXT, DT_LEFT);
    let chevron = if tabbar.items_collapsed {
        GLYPH_CHEVRON_RIGHT
    } else {
        GLYPH_CHEVRON_DOWN
    };
    draw_frame_button_glyph(hdc, chevron, chevron_rect, SHELL_SIDEBAR_HEADER_TEXT);

    if !tabbar.items_collapsed {
        draw_sidebar_items(hdc, rect, tabbar);
    }

    draw_sidebar_browser_section(hdc, rect, tabbar);

    let footer_top = rect.bottom - SIDEBAR_FOOTER_HEIGHT;
    draw_top_border(
        hdc,
        RECT {
            left: rect.left + SIDEBAR_ITEM_INSET,
            top: footer_top,
            right: rect.right - SIDEBAR_ITEM_INSET,
            bottom: rect.bottom,
        },
        SHELL_DIVIDER,
    );
    for (action_id, action_rect) in sidebar_header_action_rects(rect, tabbar) {
        let glyph = tabbar
            .header_actions
            .iter()
            .find(|action| action.id == action_id)
            .map(|action| action.glyph.as_str())
            .unwrap_or_default();
        draw_frame_button_glyph(hdc, glyph, action_rect, SHELL_TEXT_MUTED);
    }
}

/// Draws the lxapp item rows plus the macOS-parity connector line: a thin
/// vertical line along the items' leading edge linking them, drawn first so
/// it sits behind the item cards and accent bars.
fn draw_sidebar_items(hdc: HDC, rect: RECT, tabbar: &WindowsTabBarLayout) {
    if !tabbar.items.is_empty() {
        let first = sidebar_item_rect(rect, 0);
        let last = sidebar_item_rect(rect, tabbar.items.len() - 1);
        fill_rect(
            hdc,
            RECT {
                left: first.left + 7,
                top: first.top + 8,
                right: first.left + 8,
                bottom: (last.bottom - 8).max(first.top + 8),
            },
            SHELL_DIVIDER,
        );
    }

    for (index, item) in tabbar.items.iter().enumerate() {
        let item_rect = sidebar_item_rect(rect, index);
        let selected = tabbar.selected_index == index as i32;
        if selected {
            // White item card on the gray sidebar, accent bar on white.
            fill_round_rect_aa(hdc, item_rect, 8, 0xffffff);
            fill_round_rect_aa(
                hdc,
                RECT {
                    left: item_rect.left + 6,
                    top: item_rect.top + 9,
                    right: item_rect.left + 10,
                    bottom: item_rect.bottom - 9,
                },
                3,
                tabbar.selected_color,
            );
        }

        let label_rect = RECT {
            left: item_rect.left + 42,
            top: item_rect.top,
            right: item_rect.right - 8,
            bottom: item_rect.bottom,
        };
        let text_color = if selected {
            SHELL_TEXT_PRIMARY
        } else {
            SHELL_TEXT_MUTED
        };
        let icon_path = if selected && !item.selected_icon_path.trim().is_empty() {
            &item.selected_icon_path
        } else {
            &item.icon_path
        };
        if !icon_path.trim().is_empty() {
            let icon_rect = centered_icon_rect(
                RECT {
                    left: item_rect.left + 18,
                    top: item_rect.top,
                    right: item_rect.left + 18 + SIDEBAR_ICON_SIZE,
                    bottom: item_rect.bottom,
                },
                SIDEBAR_ICON_SIZE,
            );
            if !draw_icon_from_path(hdc, icon_path, icon_rect, SIDEBAR_ICON_SIZE as u32) {
                draw_text(hdc, "?", icon_rect, text_color, DT_CENTER);
            }
        }
        draw_text(hdc, &item.text, label_rect, text_color, DT_LEFT);

        if let Some(badge) = item.badge.as_ref().filter(|badge| !badge.is_empty()) {
            draw_badge(hdc, item_rect, badge);
        } else if item.has_red_dot {
            draw_red_dot(hdc, item_rect);
        }
    }
}

/// Chevron hit/draw rect at the trailing edge of the sidebar group header
/// row (the lxapp name).
pub(super) fn sidebar_group_chevron_rect(rect: RECT) -> RECT {
    let top = rect.top + 22 + (SIDEBAR_HEADER_HEIGHT - 22 - SIDEBAR_CHEVRON_SIZE).max(0) / 2;
    normalize_rect(RECT {
        left: rect.right - SIDEBAR_ITEM_INSET - SIDEBAR_CHEVRON_SIZE,
        top,
        right: rect.right - SIDEBAR_ITEM_INSET,
        bottom: top + SIDEBAR_CHEVRON_SIZE,
    })
}

/// Sidebar action buttons (settings/downloads) in the top caption strip,
/// immediately right of the sidebar toggle. They belong to the sidebar:
/// hidden while it is collapsed (only the toggle remains), and clamped to
/// the sidebar column so they never reach the lxapp navbar region.
/// `sidebar_rect` is the sidebar column rect (its top-left is the window
/// origin; the caption strip sits inside its top).
pub(super) fn sidebar_header_action_rects(
    sidebar_rect: RECT,
    tabbar: &WindowsTabBarLayout,
) -> Vec<(String, RECT)> {
    if tabbar.header_actions.is_empty() || tabbar.collapsed {
        return Vec::new();
    }
    let top = sidebar_rect.top + (SHELL_TOP_BAR_HEIGHT - SIDEBAR_HEADER_ACTION_SIZE).max(0) / 2;
    // Start right of the sidebar toggle at the window's leading edge.
    let mut left =
        sidebar_rect.left + TOP_BAR_PADDING + TOP_BAR_BUTTON_SIZE + SIDEBAR_HEADER_ACTION_GAP;
    let mut out = Vec::with_capacity(tabbar.header_actions.len());
    for action in &tabbar.header_actions {
        let right = left + SIDEBAR_HEADER_ACTION_SIZE;
        if right > sidebar_rect.left + tabbar.dimension - SIDEBAR_ITEM_INSET {
            break;
        }
        out.push((
            action.id.clone(),
            normalize_rect(RECT {
                left,
                top,
                right,
                bottom: top + SIDEBAR_HEADER_ACTION_SIZE,
            }),
        ));
        left = right + SIDEBAR_HEADER_ACTION_GAP;
    }
    out
}

pub(super) fn draw_tabbar_border(hdc: HDC, rect: RECT, tabbar: &WindowsTabBarLayout) {
    match tabbar.position {
        WindowsTabBarPosition::Bottom => draw_top_border(hdc, rect, tabbar.border_color),
        WindowsTabBarPosition::Left => draw_right_border(hdc, rect, tabbar.border_color),
        WindowsTabBarPosition::Right => draw_left_border(hdc, rect, tabbar.border_color),
    }
}

pub(super) fn tab_item_rect(
    rect: RECT,
    position: WindowsTabBarPosition,
    count: usize,
    index: usize,
) -> RECT {
    let count_i32 = count.max(1) as i32;
    let index_i32 = index as i32;
    match position {
        WindowsTabBarPosition::Bottom => {
            let width = (rect_width(&rect) / count_i32).max(1);
            let left = rect.left + width * index_i32;
            RECT {
                left,
                top: rect.top,
                right: if index + 1 == count {
                    rect.right
                } else {
                    left + width
                },
                bottom: rect.bottom,
            }
        }
        WindowsTabBarPosition::Left | WindowsTabBarPosition::Right => {
            let height = (rect_height(&rect) / count_i32).max(1);
            let top = rect.top + height * index_i32;
            RECT {
                left: rect.left,
                top,
                right: rect.right,
                bottom: if index + 1 == count {
                    rect.bottom
                } else {
                    top + height
                },
            }
        }
    }
}

pub(super) fn sidebar_item_rect(rect: RECT, index: usize) -> RECT {
    let top =
        rect.top + SIDEBAR_HEADER_HEIGHT + index as i32 * (SIDEBAR_ITEM_HEIGHT + SIDEBAR_ITEM_GAP);
    normalize_rect(RECT {
        left: rect.left + SIDEBAR_ITEM_INSET,
        top,
        right: rect.right - SIDEBAR_ITEM_INSET,
        bottom: top + SIDEBAR_ITEM_HEIGHT,
    })
}

/// Geometry of the sidebar browser section: separator line, one row rect
/// per browser tab (rows that would collide with the footer are dropped),
/// and the "New Tab" row.
pub(super) struct SidebarBrowserRects {
    pub(super) separator: RECT,
    /// Row rects aligned index-for-index with `tabbar.browser_tabs`
    /// (possibly truncated when rows run out of vertical space).
    pub(super) tabs: Vec<RECT>,
    pub(super) new_tab: Option<RECT>,
}

pub(super) fn sidebar_browser_rects(
    rect: RECT,
    tabbar: &WindowsTabBarLayout,
) -> Option<SidebarBrowserRects> {
    if tabbar.browser_tabs.is_empty() && !tabbar.show_browser_new_tab {
        return None;
    }
    let footer_top = rect.bottom - SIDEBAR_FOOTER_HEIGHT;
    // A collapsed items group hides its rows; the browser section moves up
    // directly under the group header.
    let items_height = if tabbar.items_collapsed {
        0
    } else {
        tabbar.items.len() as i32 * (SIDEBAR_ITEM_HEIGHT + SIDEBAR_ITEM_GAP)
    };
    let items_bottom = rect.top + SIDEBAR_HEADER_HEIGHT + items_height;
    let mut top = items_bottom + SIDEBAR_BROWSER_SECTION_GAP;
    let separator = normalize_rect(RECT {
        left: rect.left + SIDEBAR_ITEM_INSET,
        top,
        right: rect.right - SIDEBAR_ITEM_INSET,
        bottom: top + 1,
    });
    top += 1 + SIDEBAR_BROWSER_SECTION_GAP;

    let row = |top: &mut i32| -> Option<RECT> {
        let bottom = *top + SIDEBAR_ITEM_HEIGHT;
        if bottom > footer_top {
            return None;
        }
        let rect = normalize_rect(RECT {
            left: rect.left + SIDEBAR_ITEM_INSET,
            top: *top,
            right: rect.right - SIDEBAR_ITEM_INSET,
            bottom,
        });
        *top = bottom + SIDEBAR_ITEM_GAP;
        Some(rect)
    };

    let mut tabs = Vec::with_capacity(tabbar.browser_tabs.len());
    for _ in &tabbar.browser_tabs {
        match row(&mut top) {
            Some(rect) => tabs.push(rect),
            None => break,
        }
    }
    let new_tab = if tabbar.show_browser_new_tab {
        row(&mut top)
    } else {
        None
    };

    Some(SidebarBrowserRects {
        separator,
        tabs,
        new_tab,
    })
}

pub(super) fn sidebar_browser_hit_test(
    rect: RECT,
    tabbar: &WindowsTabBarLayout,
    point: (i32, i32),
) -> Option<WindowsChromeHit> {
    let browser = sidebar_browser_rects(rect, tabbar)?;
    for (item, item_rect) in tabbar.browser_tabs.iter().zip(&browser.tabs) {
        if rect_contains(item_rect, point) {
            if rect_contains(&sidebar_browser_close_rect(*item_rect), point) {
                return Some(WindowsChromeHit::BrowserTabClose {
                    tab_id: item.tab_id.clone(),
                });
            }
            return Some(WindowsChromeHit::BrowserTab {
                tab_id: item.tab_id.clone(),
            });
        }
    }
    if let Some(new_tab_rect) = browser.new_tab
        && rect_contains(&new_tab_rect, point)
    {
        return Some(WindowsChromeHit::BrowserNewTab);
    }
    None
}

pub(super) fn sidebar_browser_close_rect(item_rect: RECT) -> RECT {
    normalize_rect(RECT {
        left: item_rect.right - SIDEBAR_BROWSER_CLOSE_SIZE,
        top: item_rect.top,
        right: item_rect.right,
        bottom: item_rect.bottom,
    })
}

pub(super) fn draw_sidebar_browser_section(hdc: HDC, rect: RECT, tabbar: &WindowsTabBarLayout) {
    let Some(browser) = sidebar_browser_rects(rect, tabbar) else {
        return;
    };

    fill_rect(hdc, browser.separator, SHELL_DIVIDER);

    for (item, item_rect) in tabbar.browser_tabs.iter().zip(&browser.tabs) {
        let item_rect = *item_rect;
        if item.active {
            // White row card on the gray sidebar, accent bar on white.
            fill_round_rect_aa(hdc, item_rect, 8, 0xffffff);
            fill_round_rect_aa(
                hdc,
                RECT {
                    left: item_rect.left + 6,
                    top: item_rect.top + 9,
                    right: item_rect.left + 10,
                    bottom: item_rect.bottom - 9,
                },
                3,
                tabbar.selected_color,
            );
        }

        let close_rect = sidebar_browser_close_rect(item_rect);
        // 16px favicon left of the title when the tab reported one;
        // text-only row otherwise (the title keeps its original left edge).
        let mut label_left = item_rect.left + 16;
        if let Some(png) = item.favicon_png.as_deref() {
            let icon_top = item_rect.top + (rect_height(&item_rect) - SIDEBAR_FAVICON_SIZE) / 2;
            let icon_rect = normalize_rect(RECT {
                left: label_left,
                top: icon_top,
                right: label_left + SIDEBAR_FAVICON_SIZE,
                bottom: icon_top + SIDEBAR_FAVICON_SIZE,
            });
            if draw_icon_from_png_bytes(hdc, &item.tab_id, png, icon_rect) {
                label_left = icon_rect.right + SIDEBAR_FAVICON_TEXT_GAP;
            }
        }
        let label_rect = normalize_rect(RECT {
            left: label_left,
            top: item_rect.top,
            right: close_rect.left - 2,
            bottom: item_rect.bottom,
        });
        let text_color = if item.active {
            SHELL_TEXT_PRIMARY
        } else {
            SHELL_TEXT_MUTED
        };
        draw_text(hdc, &item.title, label_rect, text_color, DT_LEFT);
        draw_text(hdc, GLYPH_TAB_CLOSE, close_rect, SHELL_TEXT_MUTED, DT_CENTER);
    }

    if let Some(new_tab_rect) = browser.new_tab {
        // Arc-style new-tab row: a centered "+" glyph only, no label.
        draw_frame_button_glyph(hdc, GLYPH_ADD, new_tab_rect, SHELL_TEXT_MUTED);
    }
}

pub(super) fn panel_activator_rects(
    client: RECT,
    rects: &ChromeRects,
    layout: &WindowsWindowLayout,
) -> Vec<(String, RECT)> {
    if layout.panel_activators.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(layout.panel_activators.len());

    if let (Some(tabbar), Some(tabbar_rect)) = (&layout.tab_bar, rects.tab_bar)
        && matches!(
            tabbar.position,
            WindowsTabBarPosition::Left | WindowsTabBarPosition::Right
        )
    {
        let footer_top = tabbar_rect.bottom - SIDEBAR_FOOTER_HEIGHT;
        let top = footer_top + (SIDEBAR_FOOTER_HEIGHT - PANEL_ACTIVATOR_SIZE) / 2;
        let mut right = tabbar_rect.right - PANEL_ACTIVATOR_MARGIN;
        for activator in &layout.panel_activators {
            let left = right - PANEL_ACTIVATOR_SIZE;
            if left < tabbar_rect.left + PANEL_ACTIVATOR_MARGIN {
                break;
            }
            out.push((
                activator.id.clone(),
                normalize_rect(RECT {
                    left,
                    top,
                    right,
                    bottom: top + PANEL_ACTIVATOR_SIZE,
                }),
            ));
            right = left - PANEL_ACTIVATOR_GAP;
        }
        return out;
    }

    let bottom_limit = rects
        .tab_bar
        .map(|tabbar| tabbar.top)
        .unwrap_or(client.bottom);
    let left = rects.panel.left + PANEL_ACTIVATOR_MARGIN;
    let mut bottom = bottom_limit - PANEL_ACTIVATOR_MARGIN;

    for activator in &layout.panel_activators {
        let top = bottom - PANEL_ACTIVATOR_SIZE;
        if top < client.top + PANEL_ACTIVATOR_MARGIN {
            break;
        }
        out.push((
            activator.id.clone(),
            normalize_rect(RECT {
                left,
                top,
                right: left + PANEL_ACTIVATOR_SIZE,
                bottom,
            }),
        ));
        bottom = top - PANEL_ACTIVATOR_GAP;
    }

    out
}

pub(super) fn inset_rect(rect: RECT, dx: i32, dy: i32) -> RECT {
    normalize_rect(RECT {
        left: rect.left + dx,
        top: rect.top + dy,
        right: rect.right - dx,
        bottom: rect.bottom - dy,
    })
}

pub(super) fn draw_panel_activators(
    hdc: HDC,
    client: RECT,
    rects: &ChromeRects,
    layout: &WindowsWindowLayout,
) {
    for (panel_id, rect) in panel_activator_rects(client, rects, layout) {
        let active = layout
            .panel_activators
            .iter()
            .find(|item| item.id == panel_id)
            .is_some_and(|item| item.active);
        let activator = layout
            .panel_activators
            .iter()
            .find(|item| item.id == panel_id);
        let text = activator
            .map(|item| panel_activator_label(&item.label))
            .unwrap_or_else(|| panel_activator_label(&panel_id));
        let text_color = if active {
            SHELL_ACCENT
        } else {
            SHELL_TEXT_MUTED
        };

        if active {
            // White activator pill on the gray sidebar footer.
            fill_round_rect_aa(hdc, rect, 6, 0xffffff);
            fill_round_rect_aa(
                hdc,
                RECT {
                    left: rect.left + 3,
                    top: rect.bottom - 5,
                    right: rect.right - 3,
                    bottom: rect.bottom - 3,
                },
                2,
                SHELL_ACCENT,
            );
        }
        let icon_rect = centered_icon_rect(rect, PANEL_ACTIVATOR_ICON_SIZE);
        let icon_drawn = activator
            .filter(|item| !item.icon_path.trim().is_empty())
            .is_some_and(|item| {
                draw_icon_from_path(
                    hdc,
                    &item.icon_path,
                    icon_rect,
                    PANEL_ACTIVATOR_ICON_SIZE as u32,
                )
            });
        if !icon_drawn {
            draw_text(hdc, &text, rect, text_color, DT_CENTER);
        }
    }
}

pub(super) fn panel_activator_label(label: &str) -> String {
    let mut out = String::new();
    for ch in label.chars().filter(|ch| ch.is_ascii_alphanumeric()) {
        out.push(ch.to_ascii_uppercase());
        if out.len() == 2 {
            break;
        }
    }
    if out.is_empty() { "?".to_string() } else { out }
}

/// Chrome text font ("Segoe UI" at the shell text size/weight) sized for
/// `hdc`'s DPI. The caller owns the returned font and deletes it after use.
pub(super) fn create_chrome_text_font(hdc: HDC) -> HFONT {
    unsafe {
        CreateFontW(
            -logical_font_height(hdc, SHELL_TEXT_POINT_SIZE),
            0,
            0,
            0,
            SHELL_TEXT_WEIGHT,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32 | FF_SWISS.0 as u32,
            w!("Segoe UI"),
        )
    }
}

pub(super) fn draw_text(
    hdc: HDC,
    text: &str,
    rect: RECT,
    rgb: u32,
    horizontal: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    if text.is_empty() || rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }

    let mut wide: Vec<u16> = text.encode_utf16().collect();
    let mut rect = rect;
    let font = create_chrome_text_font(hdc);
    unsafe {
        let old_font = if font.is_invalid() {
            HGDIOBJ::default()
        } else {
            SelectObject(hdc, HGDIOBJ(font.0))
        };
        let _ = SetBkMode(hdc, TRANSPARENT);
        let _ = SetTextColor(hdc, rgb_to_colorref(rgb));
        let _ = DrawTextW(
            hdc,
            &mut wide,
            &mut rect,
            horizontal | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        );
        if !old_font.is_invalid() {
            let _ = SelectObject(hdc, old_font);
        }
        if !font.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
    }
}

pub(super) fn logical_font_height(hdc: HDC, point_size: i32) -> i32 {
    let dpi_y = unsafe { GetDeviceCaps(Some(hdc), LOGPIXELSY) };
    let dpi_y = if dpi_y > 0 { dpi_y } else { 96 };
    (point_size * dpi_y + 36) / 72
}

pub(super) fn draw_badge(hdc: HDC, item_rect: RECT, badge: &str) {
    let badge_rect = RECT {
        left: item_rect.right - 30,
        top: item_rect.top + 7,
        right: item_rect.right - 8,
        bottom: item_rect.top + 25,
    };
    fill_rect(hdc, badge_rect, SHELL_BADGE_RED);
    draw_text(hdc, badge, badge_rect, 0xffffff, DT_CENTER);
}

pub(super) fn draw_red_dot(hdc: HDC, item_rect: RECT) {
    let dot_rect = RECT {
        left: item_rect.right - 18,
        top: item_rect.top + 9,
        right: item_rect.right - 10,
        bottom: item_rect.top + 17,
    };
    fill_rect(hdc, dot_rect, SHELL_BADGE_RED);
}

pub(super) fn draw_top_border(hdc: HDC, rect: RECT, rgb: u32) {
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.top + 1,
        },
        rgb,
    );
}

pub(super) fn draw_bottom_border(hdc: HDC, rect: RECT, rgb: u32) {
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: rect.bottom - 1,
            right: rect.right,
            bottom: rect.bottom,
        },
        rgb,
    );
}

pub(super) fn draw_left_border(hdc: HDC, rect: RECT, rgb: u32) {
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: rect.top,
            right: rect.left + 1,
            bottom: rect.bottom,
        },
        rgb,
    );
}

pub(super) fn draw_right_border(hdc: HDC, rect: RECT, rgb: u32) {
    fill_rect(
        hdc,
        RECT {
            left: rect.right - 1,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        },
        rgb,
    );
}

pub(super) fn fill_rect(hdc: HDC, rect: RECT, rgb: u32) {
    if rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }
    unsafe {
        let brush = CreateSolidBrush(rgb_to_colorref(rgb));
        if brush.is_invalid() {
            return;
        }
        let _ = FillRect(hdc, &rect, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

/// Starts GDI+ once for the process (the shell paints chrome until exit,
/// so the library is never shut down). Returns `false` when startup failed;
/// rounded fills then fall back to aliased GDI `RoundRect`.
fn ensure_gdiplus_started() -> bool {
    static STARTED: OnceLock<bool> = OnceLock::new();
    *STARTED.get_or_init(|| {
        let input = GdiPlus::GdiplusStartupInput {
            GdiplusVersion: 1,
            ..Default::default()
        };
        let mut token = 0usize;
        let status = unsafe { GdiPlus::GdiplusStartup(&mut token, &input, std::ptr::null_mut()) };
        if status != GdiPlus::Ok {
            log::warn!(
                "GdiplusStartup failed ({}); rounded chrome falls back to aliased GDI",
                status.0
            );
        }
        status == GdiPlus::Ok
    })
}

/// Fills a rounded rectangle with an anti-aliased GDI+ path. `radius` is
/// the true corner radius (matching the corner-cap overlays, not GDI
/// `RoundRect`'s ellipse-diameter semantics), clamped to the rect. Used for
/// every rounded shape the chrome paints over a contrasting background —
/// plain GDI fills alias the arc into a hard staircase.
pub(super) fn fill_round_rect_aa(hdc: HDC, rect: RECT, radius: i32, rgb: u32) {
    let width = rect_width(&rect);
    let height = rect_height(&rect);
    if width == 0 || height == 0 {
        return;
    }
    let radius = radius.clamp(0, (width / 2).min(height / 2));
    if radius == 0 {
        fill_rect(hdc, rect, rgb);
        return;
    }
    if !ensure_gdiplus_started() {
        fill_round_rect_gdi(hdc, rect, rgb, radius * 2);
        return;
    }
    unsafe {
        let mut graphics: *mut GdiPlus::GpGraphics = std::ptr::null_mut();
        if GdiPlus::GdipCreateFromHDC(hdc, &mut graphics) != GdiPlus::Ok || graphics.is_null() {
            fill_round_rect_gdi(hdc, rect, rgb, radius * 2);
            return;
        }
        let _ = GdiPlus::GdipSetSmoothingMode(graphics, GdiPlus::SmoothingModeAntiAlias);
        let mut path: *mut GdiPlus::GpPath = std::ptr::null_mut();
        if GdiPlus::GdipCreatePath(GdiPlus::FillModeAlternate, &mut path) == GdiPlus::Ok
            && !path.is_null()
        {
            let (left, top) = (rect.left as f32, rect.top as f32);
            let (right, bottom) = (rect.right as f32, rect.bottom as f32);
            let diameter = (radius * 2) as f32;
            // Quarter arcs at the four corners; GDI+ connects consecutive
            // figure segments (and the close) with straight edges.
            let _ = GdiPlus::GdipAddPathArc(path, left, top, diameter, diameter, 180.0, 90.0);
            let _ =
                GdiPlus::GdipAddPathArc(path, right - diameter, top, diameter, diameter, 270.0, 90.0);
            let _ = GdiPlus::GdipAddPathArc(
                path,
                right - diameter,
                bottom - diameter,
                diameter,
                diameter,
                0.0,
                90.0,
            );
            let _ =
                GdiPlus::GdipAddPathArc(path, left, bottom - diameter, diameter, diameter, 90.0, 90.0);
            let _ = GdiPlus::GdipClosePathFigure(path);
            let mut brush: *mut GdiPlus::GpSolidFill = std::ptr::null_mut();
            if GdiPlus::GdipCreateSolidFill(0xff00_0000 | rgb, &mut brush) == GdiPlus::Ok
                && !brush.is_null()
            {
                let _ = GdiPlus::GdipFillPath(graphics, brush.cast(), path);
                let _ = GdiPlus::GdipDeleteBrush(brush.cast());
            }
            let _ = GdiPlus::GdipDeletePath(path);
        }
        let _ = GdiPlus::GdipDeleteGraphics(graphics);
    }
}

/// Aliased GDI rounded fill, kept only as the fallback when GDI+ is
/// unavailable. `corner_diameter` follows `RoundRect`'s ellipse semantics
/// (twice the corner radius).
fn fill_round_rect_gdi(hdc: HDC, rect: RECT, rgb: u32, corner_diameter: i32) {
    if rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }
    unsafe {
        let brush = CreateSolidBrush(rgb_to_colorref(rgb));
        if brush.is_invalid() {
            return;
        }
        let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
        let pen = GetStockObject(NULL_PEN);
        let old_pen = SelectObject(hdc, pen);
        let _ = RoundRect(
            hdc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            corner_diameter,
            corner_diameter,
        );
        if !old_pen.is_invalid() {
            let _ = SelectObject(hdc, old_pen);
        }
        if !old_brush.is_invalid() {
            let _ = SelectObject(hdc, old_brush);
        }
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

pub(super) fn centered_icon_rect(rect: RECT, size: i32) -> RECT {
    let left = rect.left + (rect_width(&rect) - size).max(0) / 2;
    let top = rect.top + (rect_height(&rect) - size).max(0) / 2;
    normalize_rect(RECT {
        left,
        top,
        right: left + size,
        bottom: top + size,
    })
}

pub(super) fn draw_icon_from_path(hdc: HDC, path: &str, rect: RECT, size: u32) -> bool {
    let Some(handle) = cached_png_icon_handle(path, size) else {
        return false;
    };
    draw_icon_handle(hdc, handle, rect)
}

/// Draws a PNG supplied as in-memory bytes (e.g. a tab favicon) into
/// `rect`, decoding through the id-keyed icon cache in `lingxia-webview`.
/// Returns `false` when the bytes cannot be decoded.
pub(super) fn draw_icon_from_png_bytes(hdc: HDC, cache_key: &str, png: &[u8], rect: RECT) -> bool {
    let Some(handle) = cached_png_bytes_icon_handle(cache_key, png, rect_width(&rect).max(1) as u32)
    else {
        return false;
    };
    draw_icon_handle(hdc, handle, rect)
}

fn draw_icon_handle(hdc: HDC, handle: isize, rect: RECT) -> bool {
    unsafe {
        WindowsAndMessaging::DrawIconEx(
            hdc,
            rect.left,
            rect.top,
            HICON(handle as *mut c_void),
            rect_width(&rect),
            rect_height(&rect),
            0,
            None,
            WindowsAndMessaging::DI_NORMAL,
        )
        .is_ok()
    }
}

pub(super) fn rgb_to_colorref(rgb: u32) -> COLORREF {
    let r = (rgb >> 16) & 0xff;
    let g = (rgb >> 8) & 0xff;
    let b = rgb & 0xff;
    COLORREF(r | (g << 8) | (b << 16))
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
