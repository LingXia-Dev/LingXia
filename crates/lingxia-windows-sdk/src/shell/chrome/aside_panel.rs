//! Docked aside/browser panel chrome: the in-panel toolbar (tab strip,
//! smart navigation, address capsule) and its hit-testing.

use super::*;
use crate::dpi::px;

/// Toolbar height of a browser aside panel (matches the macOS DockedBrowser).
pub(super) fn aside_panel_toolbar_height() -> i32 {
    crate::dpi::px(38)
}

/// The toolbar row at a browser aside panel's top edge; the webview fills
/// the panel below it.
pub(super) fn aside_panel_toolbar_rect(panel_rect: RECT) -> RECT {
    normalize_rect(RECT {
        left: panel_rect.left,
        top: panel_rect.top,
        right: panel_rect.right,
        bottom: (panel_rect.top + aside_panel_toolbar_height()).min(panel_rect.bottom),
    })
}

pub(super) fn browser_panel_hit_test(
    state: &WindowsChromeState,
    point: (i32, i32),
) -> Option<WindowsChromeHit> {
    let attached = state.attached.as_ref()?;
    // Panels are laid out docked first, overlays last, and drawn in that
    // order, so an overlay (a maximized slot) is on top. Test it first: its
    // header shares the top row with a docked slot's header underneath.
    for panel in attached.panels.iter().rev() {
        if let Some(hit) = aside_panel_navigation_hit(panel, point) {
            return Some(hit);
        }
        if let Some(hit) = aside_panel_tabbar_hit(panel, point) {
            return Some(hit);
        }
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
        // The aside address capsule is informational; its tabs are API-only.
        if rect_contains(&browser_panel_address_rect(panel), point) {
            return Some(WindowsChromeHit::Chrome);
        }
        return Some(WindowsChromeHit::Chrome);
    }
    None
}

/// Aside-slot chrome hit testing. Browser slots add navigation; every slot
/// shares the tab strip (each tab closes itself) and the collapse control that
/// puts the region away, with no address or new-tab controls in the strip.
pub(super) fn aside_panel_header_hit(
    panel: &WindowsChromePanel,
    tabs: &[WindowsAsidePanelTab],
    point: (i32, i32),
) -> WindowsChromeHit {
    if aside_panel_shows_browser(panel) {
        for (command, rect) in aside_panel_nav_button_rects(panel) {
            if rect_contains(&rect, point) {
                return chrome_command(command, json!({ "panel_id": panel.panel_id.clone() }));
            }
        }
    }
    if rect_contains(&browser_panel_close_rect(panel), point) {
        return chrome_command(
            command_id::ASIDE_PANEL_COLLAPSE,
            json!({ "panel_id": panel.panel_id.clone() }),
        );
    }
    if let Some(maximize) = aside_panel_maximize_rect(panel)
        && rect_contains(&maximize, point)
    {
        return chrome_command(
            command_id::ASIDE_PANEL_MAXIMIZE,
            json!({ "panel_id": panel.panel_id.clone() }),
        );
    }
    let layout = aside_panel_tab_layout(panel, tabs);
    if let Some(overflow) = layout.overflow
        && rect_contains(&overflow, point)
    {
        let hidden = layout.hidden.iter().map(|&index| &tabs[index]);
        return WindowsChromeHit::Command(
            WindowsChromeCommand::new(command_id::ASIDE_PANEL_TAB_OVERFLOW)
                .with_payload(json!({
                    "panel_id": panel.panel_id.clone(),
                    "surface_ids": hidden.clone().map(|tab| tab.surface_id.clone()).collect::<Vec<_>>(),
                    "titles": hidden.map(|tab| tab.title.clone()).collect::<Vec<_>>(),
                }))
                .with_screen_position(),
        );
    }
    let header = tabs.len() == 1;
    for (tab, rect) in tabs.iter().zip(layout.rects) {
        let Some(rect) = rect else {
            continue;
        };
        let payload = json!({
            "panel_id": panel.panel_id.clone(),
            "surface_id": tab.surface_id.clone()
        });
        // The pointer is on the tab here, so the hover-revealed controls
        // are the ones it can hit.
        let controls = aside_panel_tab_controls(rect, tab.active, header, true);
        if let Some(close) = controls.close
            && rect_contains(&close, point)
        {
            return chrome_command(command_id::ASIDE_PANEL_TAB_CLOSE, payload);
        }
        // "···" opens the tab's menu where the button is; the tab body
        // selects on click and opens the same menu on right-click.
        if let Some(more) = controls.more
            && rect_contains(&more, point)
        {
            return WindowsChromeHit::Command(
                WindowsChromeCommand::new(command_id::ASIDE_PANEL_TAB_MENU)
                    .with_payload(payload)
                    .with_screen_position(),
            );
        }
        if rect_contains(&rect, point) {
            return chrome_command_with_context(
                command_id::ASIDE_PANEL_TAB_CLICK,
                payload.clone(),
                command_id::ASIDE_PANEL_TAB_MENU,
                payload,
            );
        }
    }
    WindowsChromeHit::Chrome
}

/// The `(panel_id, surface_id)` of the aside tab a press on `command` lands
/// on: what dragging it onto the sidebar promotes (spec §4.2).
pub(crate) fn aside_tab_drag_source(command: &WindowsChromeCommand) -> Option<(String, String)> {
    if command.id != command_id::ASIDE_PANEL_TAB_CLICK {
        return None;
    }
    let panel_id = command.payload.get("panel_id")?.as_str()?;
    let surface_id = command.payload.get("surface_id")?.as_str()?;
    (panel_id == lingxia_windows_contract::ASIDE_PANEL_ID
        && crate::shell::aside_tab_promotable(surface_id))
    .then(|| (panel_id.to_string(), surface_id.to_string()))
}

/// Maximize control left of the collapse control (spec §4.6). A compact
/// overlay already covers the main, so it has nothing to toggle there.
pub(super) fn aside_panel_maximize_rect(panel: &WindowsChromePanel) -> Option<RECT> {
    let maximized = lingxia_windows_contract::aside_panel_maximized(&panel.panel_id);
    if panel.overlay && !maximized {
        return None;
    }
    let collapse = browser_panel_close_rect(panel);
    Some(normalize_rect(RECT {
        left: collapse.left - browser_panel_button_gap() - rect_width(&collapse),
        top: collapse.top,
        right: collapse.left - browser_panel_button_gap(),
        bottom: collapse.bottom,
    }))
}

/// The lxapp aside's navigation bar, under its tab strip.
pub(super) fn aside_panel_navigation_rect(panel: &WindowsChromePanel) -> Option<RECT> {
    if panel.host_content.is_some() {
        return None;
    }
    crate::window_host::aside_panel_navigation_rect(&panel.panel_id, panel.rect, panel.header_rect)
}

fn aside_panel_navigation_layout(
    panel: &WindowsChromePanel,
) -> Option<(RECT, WindowsShellNavigationBarLayout)> {
    let rect = aside_panel_navigation_rect(panel)?;
    let navigation = lingxia_windows_contract::aside_panel_navigation(&panel.panel_id)?;
    Some((
        rect,
        WindowsShellNavigationBarLayout {
            visible: true,
            title: navigation.title,
            background_color: navigation.background_color,
            text_color: navigation.text_color,
            show_back_button: navigation.show_back_button,
            show_home_button: navigation.show_home_button,
            height: rect_height(&rect),
        },
    ))
}

/// Back sits first; Home follows it, or takes its place at the stack root.
fn aside_panel_navigation_button_rects(
    rect: RECT,
    navigation: &WindowsShellNavigationBarLayout,
) -> Vec<(&'static str, RECT)> {
    let buttons_left = navbar_buttons_left(rect);
    let mut out = Vec::new();
    if navigation.show_back_button {
        out.push((
            command_id::ASIDE_PANEL_NAV_BACK,
            nav_button_rect(rect, buttons_left, 0),
        ));
    }
    if navigation.show_home_button {
        out.push((
            command_id::ASIDE_PANEL_NAV_HOME,
            nav_button_rect(rect, buttons_left, out.len() as i32),
        ));
    }
    out
}

fn aside_panel_navigation_hit(
    panel: &WindowsChromePanel,
    point: (i32, i32),
) -> Option<WindowsChromeHit> {
    let (rect, navigation) = aside_panel_navigation_layout(panel)?;
    if !rect_contains(&rect, point) {
        return None;
    }
    for (command, button) in aside_panel_navigation_button_rects(rect, &navigation) {
        if rect_contains(&button, point) {
            return Some(chrome_command(
                command,
                json!({ "panel_id": panel.panel_id.clone() }),
            ));
        }
    }
    Some(WindowsChromeHit::Chrome)
}

/// Hover target inside an lxapp aside's navigation bar.
pub(super) fn aside_panel_navigation_hover_rect(
    panel: &WindowsChromePanel,
    point: (i32, i32),
) -> Option<RECT> {
    let (rect, navigation) = aside_panel_navigation_layout(panel)?;
    if !rect_contains(&rect, point) {
        return None;
    }
    aside_panel_navigation_button_rects(rect, &navigation)
        .into_iter()
        .map(|(_, button)| button)
        .find(|button| rect_contains(button, point))
        .or(Some(rect))
}

/// Below this the row of page tabs turns into the current tab's pop-up.
fn aside_panel_tabbar_segments_min_width() -> i32 {
    px(240)
}
/// Air either side of a page tab's label; neighbours sit two of these apart.
fn aside_panel_tabbar_label_padding() -> i32 {
    px(8)
}

fn aside_panel_tabbar_layout(
    panel: &WindowsChromePanel,
) -> Option<(RECT, lingxia_windows_contract::WindowsAsidePanelTabBar)> {
    if panel.host_content.is_some() {
        return None;
    }
    let rect = crate::window_host::aside_panel_tabbar_rect(
        &panel.panel_id,
        panel.rect,
        panel.header_rect,
    )?;
    let tabbar = lingxia_windows_contract::aside_panel_tabbar(&panel.panel_id)?;
    Some((rect, tabbar))
}

/// Page tab rects, index-aligned with `tabbar.items`: each fitted to its
/// label, left-aligned. Empty in the pop-up form — a panel narrower than the
/// minimum, or one the labels do not fit (there is no horizontal scroll).
fn aside_panel_tabbar_segment_rects(rect: RECT, items: &[(i32, String)]) -> Vec<RECT> {
    if items.is_empty() || rect_width(&rect) < aside_panel_tabbar_segments_min_width() {
        return Vec::new();
    }
    let padding = aside_panel_tabbar_label_padding();
    let inner = inset_rect(rect, px(4), 0);
    let mut out = Vec::with_capacity(items.len());
    let mut left = inner.left;
    for (_, label) in items {
        let right = left + measure_chrome_text_width(label) + 2 * padding;
        if right > inner.right {
            return Vec::new();
        }
        out.push(normalize_rect(RECT {
            left,
            top: inner.top,
            right,
            bottom: inner.bottom - 1,
        }));
        left = right;
    }
    out
}

fn aside_panel_tabbar_hit(
    panel: &WindowsChromePanel,
    point: (i32, i32),
) -> Option<WindowsChromeHit> {
    let (rect, tabbar) = aside_panel_tabbar_layout(panel)?;
    if !rect_contains(&rect, point) {
        return None;
    }
    let segments = aside_panel_tabbar_segment_rects(rect, &tabbar.items);
    if segments.is_empty() {
        return Some(WindowsChromeHit::Command(
            WindowsChromeCommand::new(command_id::ASIDE_PANEL_TABBAR_MENU)
                .with_payload(json!({ "panel_id": panel.panel_id.clone() }))
                .with_screen_position(),
        ));
    }
    for ((index, _), segment) in tabbar.items.iter().zip(segments) {
        if rect_contains(&segment, point) {
            return Some(chrome_command(
                command_id::ASIDE_PANEL_TABBAR_CLICK,
                json!({ "panel_id": panel.panel_id.clone(), "index": index }),
            ));
        }
    }
    Some(WindowsChromeHit::Chrome)
}

/// Hover target inside an lxapp aside's tabBar row.
pub(super) fn aside_panel_tabbar_hover_rect(
    panel: &WindowsChromePanel,
    point: (i32, i32),
) -> Option<RECT> {
    let (rect, tabbar) = aside_panel_tabbar_layout(panel)?;
    if !rect_contains(&rect, point) {
        return None;
    }
    aside_panel_tabbar_segment_rects(rect, &tabbar.items)
        .into_iter()
        .find(|segment| rect_contains(segment, point))
        .or(Some(rect))
}

/// The lxapp aside's tabBar: page tabs under the navigation bar, the current
/// one in the selected colour over an underline (page chrome, not a second
/// slot strip, so no fill chip); the current tab with a ▾ when they do not fit.
pub(super) fn draw_aside_panel_tabbar(
    hdc: HDC,
    panel: &WindowsChromePanel,
    cursor: Option<(i32, i32)>,
) {
    let Some((rect, tabbar)) = aside_panel_tabbar_layout(panel) else {
        return;
    };
    let pal = shell_palette();
    fill_rect(hdc, rect, pal.panel_background);
    draw_bottom_border(hdc, rect, pal.divider);
    let segments = aside_panel_tabbar_segment_rects(rect, &tabbar.items);
    if segments.is_empty() {
        let label = tabbar
            .items
            .iter()
            .find(|(index, _)| *index == tabbar.selected_index)
            .or(tabbar.items.first())
            .map(|(_, label)| format!("{label}  \u{25BE}"))
            .unwrap_or_default();
        let pill = inset_rect(rect, px(8), px(3));
        draw_hover_wash(hdc, pill, 6, cursor);
        draw_text(
            hdc,
            &label,
            inset_rect(pill, px(8), 0),
            pal.text_primary,
            DT_LEFT,
        );
        return;
    }
    let selected_color = tabbar.selected_color.unwrap_or(pal.accent);
    let idle_color = tabbar.color.unwrap_or(pal.text_muted);
    for ((index, label), segment) in tabbar.items.iter().zip(segments) {
        let selected = *index == tabbar.selected_index;
        let label_rect = inset_rect(segment, aside_panel_tabbar_label_padding(), 0);
        if selected {
            // Clear of the row's bottom edge, where the page starts.
            let bottom = segment.bottom - px(3);
            fill_round_rect_aa(
                hdc,
                RECT {
                    left: label_rect.left,
                    top: bottom - px(2),
                    right: label_rect.right,
                    bottom,
                },
                px(1),
                selected_color,
            );
        } else {
            draw_hover_wash(hdc, inset_rect(segment, px(2), px(3)), 6, cursor);
        }
        let color = if selected { selected_color } else { idle_color };
        draw_text(hdc, label, segment, color, DT_CENTER);
    }
}

/// The lxapp aside's navigation bar: the same bar the main draws, fed the
/// aside page's state and sized to the panel.
pub(super) fn draw_aside_panel_navigation(
    hdc: HDC,
    panel: &WindowsChromePanel,
    cursor: Option<(i32, i32)>,
) {
    let Some((rect, navigation)) = aside_panel_navigation_layout(panel) else {
        return;
    };
    draw_navigation_bar(
        hdc,
        rect,
        [0; 4],
        true,
        navbar_buttons_left(rect),
        &navigation,
        cursor,
    );
}

pub(super) fn panel_aside_tabs(panel: &WindowsChromePanel) -> Vec<WindowsAsidePanelTab> {
    if panel.host_content.is_some() {
        return Vec::new();
    }
    aside_panel_tabs(&panel.panel_id)
}

pub(super) fn browser_panel_header_visible(panel: &WindowsChromePanel) -> bool {
    panel.host_content.is_none()
        && (panel.webtag_key.starts_with("app.lingxia.browser:")
            || !aside_panel_tabs(&panel.panel_id).is_empty())
}

pub(super) fn browser_panel_header_rect(panel: &WindowsChromePanel) -> RECT {
    // The toolbar row at the panel's top (computed at layout time); an empty
    // rect makes the draw/hit-test paths no-op for panels without one.
    panel.header_rect.map(normalize_rect).unwrap_or_default()
}

pub(super) fn browser_panel_close_rect(panel: &WindowsChromePanel) -> RECT {
    let header = browser_panel_header_rect(panel);
    normalize_rect(RECT {
        left: (header.right - browser_panel_button_size() - browser_panel_header_padding())
            .max(header.left),
        top: header.top + (rect_height(&header) - browser_panel_button_size()) / 2,
        right: header.right - browser_panel_header_padding(),
        bottom: header.top + (rect_height(&header) + browser_panel_button_size()) / 2,
    })
}

pub(super) fn browser_panel_nav_button_rects(
    panel: &WindowsChromePanel,
) -> [(&'static str, RECT); 3] {
    let header = browser_panel_header_rect(panel);
    let top = header.top + (rect_height(&header) - browser_panel_button_size()) / 2;
    let mut left = header.left + browser_panel_header_padding();
    let mut next = || {
        let rect = normalize_rect(RECT {
            left,
            top,
            right: left + browser_panel_button_size(),
            bottom: top + browser_panel_button_size(),
        });
        left += browser_panel_button_size() + browser_panel_button_gap();
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
pub(super) fn aside_panel_nav_button_rects(
    panel: &WindowsChromePanel,
) -> [(&'static str, RECT); 3] {
    let [(_, back), (_, forward), (_, reload)] = browser_panel_nav_button_rects(panel);
    [
        (command_id::ASIDE_PANEL_NAV_BACK, back),
        (command_id::ASIDE_PANEL_NAV_FORWARD, forward),
        (command_id::ASIDE_PANEL_NAV_RELOAD, reload),
    ]
}

fn aside_panel_tab_max_width() -> i32 {
    crate::dpi::px(190)
}
/// The narrowest tab: its icon alone. Tabs that would go below it wait
/// behind the overflow button.
fn aside_panel_tab_min_width() -> i32 {
    crate::dpi::px(36)
}
/// From this width a tab keeps room for its "···" next to the title.
fn aside_panel_tab_full_width() -> i32 {
    crate::dpi::px(110)
}
/// From this width a tab shows its title; below it, its icon alone.
fn aside_panel_tab_title_width() -> i32 {
    crate::dpi::px(64)
}
fn aside_panel_tab_overflow_width() -> i32 {
    crate::dpi::px(34)
}
fn aside_panel_tab_icon_size() -> i32 {
    crate::dpi::px(16)
}
/// Air between a tab's leading edge and its icon, and around its title.
fn aside_panel_tab_inset() -> i32 {
    crate::dpi::px(10)
}
fn aside_panel_tab_gap() -> i32 {
    crate::dpi::px(4)
}
/// Air above the tabs. Bridging tabs run flush to the toolbar's bottom edge
/// so the active one merges into the content below (Chrome style); the rest
/// keep the same air below and sit centred in the strip.
fn aside_panel_tab_top_inset() -> i32 {
    crate::dpi::px(6)
}
/// Only the browser's own toolbar sits flush under the strip in the card's
/// fill. Below an lxapp tab is its page (or its declared navigation bar
/// colour), so its active tab stays a chip clear of the seam instead of
/// pretending to merge into it.
fn aside_panel_tabs_bridge(panel: &WindowsChromePanel) -> bool {
    aside_panel_shows_browser(panel)
}

/// Whether the side panel's active tab is a URL tab: then the header carries
/// the browser navigation cluster and the active tab bridges into the page.
pub(super) fn aside_panel_shows_browser(panel: &WindowsChromePanel) -> bool {
    panel.panel_id == lingxia_windows_contract::ASIDE_PANEL_ID
        && panel_aside_tabs(panel)
            .iter()
            .find(|tab| tab.active)
            .is_some_and(|tab| tab.browser)
}
/// Upper-corner radius of the active tab shape.
fn aside_panel_tab_radius() -> i32 {
    crate::dpi::px(8)
}
fn aside_panel_tab_close_size() -> i32 {
    crate::dpi::px(16)
}
/// The "···" menu button left of the close glyph. Its room is always
/// reserved so revealing it on hover never shifts the title.
fn aside_panel_tab_more_size() -> i32 {
    crate::dpi::px(16)
}
/// The aside strip's tabs: a rect per registered tab (index-aligned), None
/// for the tabs that wait behind the overflow button.
pub(super) struct AsideTabLayout {
    pub(super) rects: Vec<Option<RECT>>,
    pub(super) overflow: Option<RECT>,
    pub(super) hidden: Vec<usize>,
}

/// Tabs share the strip between the nav cluster and the slot controls in
/// equal widths (capped), Chrome style; each tab then shows what its width
/// allows (`aside_panel_tab_controls`). When even icon-wide tabs do not fit,
/// the ones that do are shown, the active one always, and the rest wait
/// behind an overflow button that lists them.
pub(super) fn aside_panel_tab_layout(
    panel: &WindowsChromePanel,
    tabs: &[WindowsAsidePanelTab],
) -> AsideTabLayout {
    if tabs.is_empty() {
        return AsideTabLayout {
            rects: Vec::new(),
            overflow: None,
            hidden: Vec::new(),
        };
    }
    let header = browser_panel_header_rect(panel);
    let left_edge = if aside_panel_shows_browser(panel) {
        aside_panel_nav_button_rects(panel)[2].1.right + browser_panel_header_padding()
    } else {
        header.left + browser_panel_header_padding()
    };
    let right_edge = aside_panel_maximize_rect(panel)
        .unwrap_or_else(|| browser_panel_close_rect(panel))
        .left
        - browser_panel_header_padding();
    let avail = (right_edge - left_edge).max(0);
    let gap = aside_panel_tab_gap();
    // n tabs of width w take n*w + (n-1)*gap.
    let width_for = |count: i32, room: i32| {
        ((room - (count - 1) * gap) / count).min(aside_panel_tab_max_width())
    };

    let mut visible: Vec<usize> = (0..tabs.len()).collect();
    if width_for(tabs.len() as i32, avail) < aside_panel_tab_min_width() {
        let room = avail - aside_panel_tab_overflow_width();
        let fits = ((room + gap) / (aside_panel_tab_min_width() + gap)).max(1) as usize;
        visible.truncate(fits);
        if let Some(active) = tabs.iter().position(|tab| tab.active)
            && !visible.contains(&active)
            && let Some(last) = visible.last_mut()
        {
            *last = active;
        }
    }
    let hidden: Vec<usize> = (0..tabs.len())
        .filter(|index| !visible.contains(index))
        .collect();
    let room = if hidden.is_empty() {
        avail
    } else {
        avail - aside_panel_tab_overflow_width()
    };
    let width = width_for(visible.len() as i32, room).max(aside_panel_tab_min_width());
    let top = header.top + aside_panel_tab_top_inset();
    let bottom = if aside_panel_tabs_bridge(panel) {
        header.bottom
    } else {
        header.bottom - aside_panel_tab_top_inset()
    };
    let mut rects = vec![None; tabs.len()];
    let mut left = left_edge;
    for &index in &visible {
        rects[index] = Some(normalize_rect(RECT {
            left,
            top,
            right: (left + width).min(right_edge),
            bottom,
        }));
        left += width + gap;
    }
    let overflow = (!hidden.is_empty()).then(|| {
        let height = px(24);
        let top = top + (bottom - top - height) / 2;
        normalize_rect(RECT {
            left,
            top,
            right: (left + aside_panel_tab_overflow_width() - px(4)).min(right_edge),
            bottom: top + height,
        })
    });
    AsideTabLayout {
        rects,
        overflow,
        hidden,
    }
}

/// What a tab of this width shows: the icon always; the title from
/// `aside_panel_tab_title_width`; the ✕ on a wide tab, else only while the
/// tab is active, hovered or the single-child header (an icon-wide tab: when
/// active and it still fits); the "···" room only on a wide tab, where it is
/// reserved so revealing it on hover never shifts the title. The menu stays
/// on right-click at every width.
pub(super) struct AsideTabControls {
    pub(super) icon: RECT,
    pub(super) title: Option<RECT>,
    pub(super) more: Option<RECT>,
    pub(super) close: Option<RECT>,
}

pub(super) fn aside_panel_tab_controls(
    tab: RECT,
    active: bool,
    header: bool,
    hovered: bool,
) -> AsideTabControls {
    let width = rect_width(&tab);
    let icon_size = aside_panel_tab_icon_size();
    let inset = aside_panel_tab_inset();
    let glyph_rect = |right: i32, size: i32| {
        let top = tab.top + (rect_height(&tab) - size) / 2;
        normalize_rect(RECT {
            left: right - size,
            top,
            right,
            bottom: top + size,
        })
    };
    let close_size = aside_panel_tab_close_size();
    let close_at = glyph_rect(tab.right - px(6), close_size);
    let full = width >= aside_panel_tab_full_width();
    let titled = width >= aside_panel_tab_title_width();
    let icon_and_close = inset + icon_size + px(4) + close_size + px(6);
    let close = if full {
        Some(close_at)
    } else if titled {
        (active || hovered || header).then_some(close_at)
    } else {
        ((active || header) && width >= icon_and_close).then_some(close_at)
    };
    let more = full.then(|| glyph_rect(close_at.left - px(2), aside_panel_tab_more_size()));
    // An icon-wide tab centres its icon when it shows nothing else.
    let icon_left = if titled || close.is_some() {
        tab.left + inset
    } else {
        tab.left + (width - icon_size) / 2
    };
    let icon_top = tab.top + (rect_height(&tab) - icon_size) / 2;
    let icon = normalize_rect(RECT {
        left: icon_left,
        top: icon_top,
        right: icon_left + icon_size,
        bottom: icon_top + icon_size,
    });
    let title = titled.then(|| {
        normalize_rect(RECT {
            left: icon.right + px(6),
            top: tab.top,
            right: more
                .or(close)
                .map(|control| control.left)
                .unwrap_or(tab.right - px(6)),
            bottom: tab.bottom,
        })
    });
    AsideTabControls {
        icon,
        title,
        more,
        close,
    }
}

/// The URL capsule rect inside a browser aside's header (between the nav
/// cluster and the close button). Shared by the painter and hit-test so the
/// inline editor lands exactly on the painted pill.
pub(super) fn browser_panel_address_rect(panel: &WindowsChromePanel) -> RECT {
    let header = browser_panel_header_rect(panel);
    let close = browser_panel_close_rect(panel);
    let address_left = browser_panel_nav_button_rects(panel)
        .last()
        .map(|(_, rect)| rect.right + browser_panel_header_padding())
        .unwrap_or(header.left + browser_panel_header_padding());
    normalize_rect(RECT {
        left: address_left,
        top: header.top + px(8),
        right: close.left - browser_panel_header_padding(),
        bottom: header.bottom - px(8),
    })
}

pub(super) fn draw_browser_panel_header(
    hdc: HDC,
    panel: &WindowsChromePanel,
    cursor: Option<(i32, i32)>,
) {
    let header = browser_panel_header_rect(panel);
    if rect_width(&header) == 0 || rect_height(&header) == 0 {
        return;
    }
    let pal = shell_palette();

    let tabs = panel_aside_tabs(panel);
    if !tabs.is_empty() {
        draw_aside_panel_header(hdc, panel, &tabs, cursor);
        return;
    }

    // The toolbar shares the panel card's fill; only the divider separates
    // it from the webview below.
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

    let close = browser_panel_close_rect(panel);
    for (command, rect) in browser_panel_nav_button_rects(panel) {
        let icon = match command {
            command_id::BROWSER_PANEL_NAV_BACK => WindowsDesignIcon::Back,
            command_id::BROWSER_PANEL_NAV_FORWARD => WindowsDesignIcon::Forward,
            _ => WindowsDesignIcon::BrowserRefresh,
        };
        draw_hover_wash(hdc, rect, 5, cursor);
        draw_design_icon_button(hdc, rect, icon, pal.frame_button_icon, px(16));
    }

    let address = browser_panel_address_rect(panel);
    let address_visible = rect_width(&address) > 0 && rect_height(&address) > 0;
    if address_visible {
        fill_round_rect_aa(hdc, address, 10, pal.address_background);
        draw_text(
            hdc,
            browser_panel_title(panel).as_str(),
            inset_rect(address, 10, 0),
            pal.text_muted,
            DT_LEFT,
        );
    }
    draw_hover_wash(hdc, close, 5, cursor);
    draw_design_icon_button_with_fallback(
        hdc,
        close,
        WindowsDesignIcon::CloseX,
        pal.frame_button_icon,
        14,
        Some(GLYPH_CLOSE),
    );
}

pub(super) fn browser_panel_title(panel: &WindowsChromePanel) -> String {
    let title = panel.title.trim();
    if title.is_empty() {
        "Browser".to_string()
    } else {
        title.to_string()
    }
}

/// Shared aside-slot chrome row: title tabs and the collapse control. The browser slot
/// additionally gets back/forward/reload; lxapp slots start the strip at the
/// leading edge. No slot carries a "+" affordance. A single tab is drawn as
/// the panel's header (icon-less title, "···", ✕) with no chip, wash or
/// divider — there is nothing to pick among — and turns into tabs when a
/// sibling arrives.
pub(super) fn draw_aside_panel_header(
    hdc: HDC,
    panel: &WindowsChromePanel,
    tabs: &[WindowsAsidePanelTab],
    cursor: Option<(i32, i32)>,
) {
    let pal = shell_palette();

    // Chrome-style strip: the tinted cap is painted by the card itself (see
    // `draw_content_cards` — cap and body split at the header seam so each
    // arc is rasterized once). Here only the controls and tabs go on top;
    // the active tab lifts out as a round-topped shape in the card's fill,
    // merging seamlessly into the web content below (browser slot only).

    if aside_panel_shows_browser(panel) {
        let (can_back, can_forward) = crate::shell::runtime::aside_panel_nav_state();
        for (command, rect) in aside_panel_nav_button_rects(panel) {
            let (icon, enabled) = match command {
                command_id::ASIDE_PANEL_NAV_BACK => (WindowsDesignIcon::Back, can_back),
                command_id::ASIDE_PANEL_NAV_FORWARD => (WindowsDesignIcon::Forward, can_forward),
                _ => (WindowsDesignIcon::BrowserRefresh, true),
            };
            // Hover lights only actionable buttons; a disabled direction stays
            // flat and dim.
            let color = if enabled {
                draw_hover_wash(hdc, rect, 5, cursor);
                pal.frame_button_icon
            } else {
                pal.text_muted
            };
            draw_design_icon_button(hdc, rect, icon, color, px(16));
        }
    }

    let bridges = aside_panel_tabs_bridge(panel);
    let header = tabs.len() == 1;
    let layout = aside_panel_tab_layout(panel, tabs);
    for (index, (tab, rect)) in tabs.iter().zip(layout.rects.iter().copied()).enumerate() {
        let Some(rect) = rect else {
            continue;
        };
        if header {
            // A plain header row: no chip, wash or divider.
        } else if tab.active && !bridges {
            // The white card on the grey cap, as the sidebar's selected child.
            fill_round_rect_aa(hdc, rect, px(6), pal.panel_background);
        } else if tab.active {
            // Rounded top, flush bottom: the tab joins the web content.
            fill_round_rect_aa(hdc, rect, aside_panel_tab_radius(), pal.panel_background);
            fill_rect(
                hdc,
                RECT {
                    left: rect.left,
                    top: (rect.bottom - aside_panel_tab_radius()).max(rect.top),
                    right: rect.right,
                    bottom: rect.bottom,
                },
                pal.panel_background,
            );
        } else if index > 0 && !tabs[index - 1].active && layout.rects[index - 1].is_some() {
            // Chrome hides the separator next to the active tab.
            let x = rect.left - (aside_panel_tab_gap() + 1) / 2;
            fill_rect(
                hdc,
                RECT {
                    left: x,
                    top: rect.top + px(8),
                    right: x + 1,
                    bottom: rect.bottom - px(8),
                },
                pal.divider,
            );
        }
        if !tab.active && !header {
            let radius = if bridges {
                aside_panel_tab_radius()
            } else {
                px(6)
            };
            draw_hover_wash(hdc, rect, radius, cursor);
        }
        let hovered = cursor.is_some_and(|point| rect_contains(&rect, point));
        let controls = aside_panel_tab_controls(rect, tab.active, header, hovered);
        draw_aside_panel_tab_icon(hdc, tab, controls.icon);
        if let Some(title_rect) = controls.title {
            let text_color = if tab.active {
                pal.text_primary
            } else {
                pal.text_muted
            };
            draw_text(hdc, &tab.title, title_rect, text_color, DT_LEFT);
        }
        // "···" shows on hover, or always on the header where it is a panel
        // control; its room is reserved either way.
        if let Some(more) = controls.more
            && (header || hovered)
        {
            draw_hover_wash(hdc, more, 5, cursor);
            draw_design_icon_button(
                hdc,
                more,
                WindowsDesignIcon::MoreVertical,
                pal.text_muted,
                px(14),
            );
        }
        if let Some(close) = controls.close {
            draw_hover_wash(hdc, close, 5, cursor);
            draw_text(hdc, GLYPH_TAB_CLOSE, close, pal.text_muted, DT_CENTER);
        }
    }
    if let Some(overflow) = layout.overflow {
        draw_hover_wash(hdc, overflow, 5, cursor);
        draw_text(
            hdc,
            &format!("{} \u{25BE}", layout.hidden.len()),
            overflow,
            pal.text_muted,
            DT_CENTER,
        );
    }

    // Putting the region away is not a close: it keeps every tab alive, so it
    // gets the panel-collapse mark rather than an ✕.
    let collapse = browser_panel_close_rect(panel);
    draw_hover_wash(hdc, collapse, 5, cursor);
    draw_design_icon_button(
        hdc,
        collapse,
        WindowsDesignIcon::AsideCollapse,
        pal.frame_button_icon,
        16,
    );
    if let Some(maximize) = aside_panel_maximize_rect(panel) {
        draw_hover_wash(hdc, maximize, 5, cursor);
        let glyph = if lingxia_windows_contract::aside_panel_maximized(&panel.panel_id) {
            GLYPH_PANEL_SHRINK
        } else {
            GLYPH_PANEL_EXPAND
        };
        draw_frame_button_glyph_grayscale(hdc, glyph, maximize, pal.frame_button_icon);
    }
}

/// A side panel tab's icon: the lxapp's icon (an lxapp child's surface id is
/// its app id), the globe for a page.
fn draw_aside_panel_tab_icon(hdc: HDC, tab: &WindowsAsidePanelTab, rect: RECT) {
    if tab.browser {
        draw_design_icon_button(
            hdc,
            rect,
            WindowsDesignIcon::Globe,
            shell_palette().text_muted,
            aside_panel_tab_icon_size(),
        );
        return;
    }
    let path = lxapp::lxapp_display_icon_path(&tab.surface_id).unwrap_or_default();
    draw_icon_or_default(hdc, &path, rect, aside_panel_tab_icon_size().max(1) as u32);
}
