//! Sidebar, tab bar, browser rows, and panel activator chrome.

use super::*;

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
        draw_text(
            hdc,
            GLYPH_TAB_CLOSE,
            close_rect,
            SHELL_TEXT_MUTED,
            DT_CENTER,
        );
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
