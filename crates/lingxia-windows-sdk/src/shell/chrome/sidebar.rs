//! Sidebar and tab bar chrome.

use crate::WindowsDesignIcon;

use super::*;

mod auxiliary;
mod panel_activator;
pub(super) use auxiliary::*;
pub(super) use panel_activator::*;

/// Phone bottom tab bar: 49px item strip plus a lower safe-area hit region.
const BOTTOM_TAB_ICON_SIZE: i32 = 22;
const BOTTOM_TAB_ITEM_HEIGHT: i32 = 49;
const BOTTOM_TAB_ICON_TOP: i32 = 5;
const BOTTOM_TAB_LABEL_TOP_GAP: i32 = 1;

pub(super) fn draw_tab_bar(
    hdc: HDC,
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    cursor: Option<(i32, i32)>,
) {
    if matches!(
        tabbar.position,
        WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
    ) {
        draw_sidebar_tab_bar(hdc, rect, tabbar, cursor);
        return;
    }

    if !tabbar.background_transparent {
        fill_rect(hdc, rect, tabbar.background_color);
        draw_tabbar_border(hdc, rect, tabbar);
    }

    let count = tabbar.items.len();
    if count == 0 {
        return;
    }

    for (index, item) in tabbar.items.iter().enumerate() {
        let item_rect = tab_item_rect(rect, tabbar.position, count, index);
        let selected = tabbar.selected_index == index as i32;
        let color = if selected {
            tabbar.selected_color
        } else {
            tabbar.color
        };

        // Phone tab cell: the lxapp's pre-tinted icon stacked over its label,
        // both centered. The bundle ships separate normal/selected icons, so the
        // PNG is drawn as-is and only the label tracks `selected_color`.
        let icon_path = if selected && !item.selected_icon_path.trim().is_empty() {
            item.selected_icon_path.as_str()
        } else {
            item.icon_path.as_str()
        };
        let item_top = item_rect.top;
        let item_bottom = (item_rect.top + BOTTOM_TAB_ITEM_HEIGHT).min(item_rect.bottom);
        let center_x = (item_rect.left + item_rect.right) / 2;
        let icon_top = item_top + BOTTOM_TAB_ICON_TOP;
        let icon_rect = RECT {
            left: center_x - BOTTOM_TAB_ICON_SIZE / 2,
            top: icon_top,
            right: center_x + BOTTOM_TAB_ICON_SIZE / 2,
            bottom: icon_top + BOTTOM_TAB_ICON_SIZE,
        };
        let drew_icon = !icon_path.trim().is_empty()
            && draw_icon_from_path(hdc, icon_path, icon_rect, BOTTOM_TAB_ICON_SIZE as u32);

        // Icon-less bars keep the label vertically centred; otherwise it sits
        // just under the icon.
        let label_rect = RECT {
            left: item_rect.left,
            top: if drew_icon {
                icon_rect.bottom + BOTTOM_TAB_LABEL_TOP_GAP
            } else {
                item_top + 6
            },
            right: item_rect.right,
            bottom: item_bottom - 2,
        };
        if tabbar.background_transparent {
            draw_text_antialiased(hdc, &item.text, label_rect, color, DT_CENTER);
        } else {
            draw_text(hdc, &item.text, label_rect, color, DT_CENTER);
        }

        let badge_anchor = if drew_icon { icon_rect } else { item_rect };
        if let Some(badge) = item.badge.as_ref().filter(|badge| !badge.is_empty()) {
            draw_badge(hdc, badge_anchor, badge);
        } else if item.has_red_dot {
            draw_red_dot(hdc, badge_anchor);
        }
    }
}

pub(super) fn draw_sidebar_tab_bar(
    hdc: HDC,
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    cursor: Option<(i32, i32)>,
) {
    if rect_width(&rect) == 0 {
        return;
    }
    fill_rect(hdc, rect, shell_palette().sidebar_background);

    // Icon-only rail: first-level entries only, centered in a compact column.
    if tabbar.collapsed || tabbar.icon_rail {
        draw_sidebar_rail(hdc, rect, tabbar, cursor);
        return;
    }

    let title = if tabbar.app_name.trim().is_empty() {
        "LXAPP".to_string()
    } else {
        tabbar.app_name.to_ascii_uppercase()
    };
    let group_top = sidebar_group_top(rect, tabbar);
    let group_bottom = sidebar_group_bottom(rect, tabbar);
    let chevron_rect = sidebar_group_chevron_rect(rect, tabbar);
    // The lxapp's own icon (via the app-info API) leads the group header.
    let icon_top = group_top + (group_bottom - group_top - SIDEBAR_ICON_SIZE).max(0) / 2;
    let icon_rect = RECT {
        left: rect.left + SIDEBAR_ITEM_INSET + 2,
        top: icon_top,
        right: rect.left + SIDEBAR_ITEM_INSET + 2 + SIDEBAR_ICON_SIZE,
        bottom: icon_top + SIDEBAR_ICON_SIZE,
    };
    draw_icon_or_default(
        hdc,
        &tabbar.app_icon_path,
        icon_rect,
        SIDEBAR_ICON_SIZE as u32,
    );
    let header_rect = RECT {
        left: icon_rect.right + 8,
        top: group_top,
        right: chevron_rect.left - 4,
        bottom: group_bottom,
    };
    draw_text(
        hdc,
        &title,
        header_rect,
        shell_palette().sidebar_header_text,
        DT_LEFT,
    );
    let chevron = if tabbar.items_collapsed {
        GLYPH_CHEVRON_RIGHT
    } else {
        GLYPH_CHEVRON_DOWN
    };
    draw_hover_wash(hdc, chevron_rect, 4, cursor);
    draw_frame_button_glyph(
        hdc,
        chevron,
        chevron_rect,
        shell_palette().sidebar_header_text,
    );

    if !tabbar.items_collapsed {
        draw_sidebar_items(hdc, rect, tabbar, cursor);
    }

    draw_sidebar_auxiliary_section(hdc, rect, tabbar, cursor);

    let footer_top = rect.bottom - SIDEBAR_FOOTER_HEIGHT;
    draw_top_border(
        hdc,
        RECT {
            left: rect.left + SIDEBAR_ITEM_INSET,
            top: footer_top,
            right: rect.right - SIDEBAR_ITEM_INSET,
            bottom: rect.bottom,
        },
        shell_palette().divider,
    );
    for (action_id, action_rect) in sidebar_header_action_rects(rect, tabbar) {
        let Some(action) = tabbar
            .header_actions
            .iter()
            .find(|action| action.id == action_id)
        else {
            continue;
        };
        draw_hover_wash(hdc, action_rect, 4, cursor);
        draw_sidebar_header_action(hdc, &action.id, &action.glyph, action_rect);
    }
}

fn draw_sidebar_header_action(hdc: HDC, action_id: &str, fallback_glyph: &str, rect: RECT) {
    let icon = match action_id {
        "settings" => Some(WindowsDesignIcon::Settings),
        "downloads" => Some(WindowsDesignIcon::Downloads),
        _ => None,
    };
    // Settings/downloads are secondary chrome actions: drawn muted (like
    // macOS `secondaryLabelColor`) so they don't compete with content or the
    // primary caption buttons.
    if let Some(icon) = icon {
        draw_design_icon_button_with_fallback(
            hdc,
            rect,
            icon,
            shell_palette().text_muted,
            18,
            Some(fallback_glyph),
        );
        return;
    }
    draw_frame_button_glyph(hdc, fallback_glyph, rect, shell_palette().text_muted);
}

/// Draws the lxapp item rows plus the macOS-parity connector line: a thin
/// vertical line along the items' leading edge linking them, drawn first so
/// it sits behind the item cards and accent bars.
pub(super) fn draw_sidebar_items(
    hdc: HDC,
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    cursor: Option<(i32, i32)>,
) {
    if !tabbar.items.is_empty() {
        let first = sidebar_item_rect(rect, tabbar, 0);
        let last = sidebar_item_rect(rect, tabbar, tabbar.items.len() - 1);
        fill_rect(
            hdc,
            RECT {
                left: first.left + 7,
                top: first.top + 8,
                right: first.left + 8,
                bottom: (last.bottom - 8).max(first.top + 8),
            },
            shell_palette().divider,
        );
    }

    for (index, item) in tabbar.items.iter().enumerate() {
        let item_rect = sidebar_item_rect(rect, tabbar, index);
        let selected = tabbar.selected_index == index as i32;
        if selected {
            // White item card on the gray sidebar, accent bar on white.
            fill_round_rect_aa(hdc, item_rect, 8, shell_palette().panel_background);
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
        } else {
            draw_hover_wash(hdc, item_rect, 8, cursor);
        }

        let label_rect = RECT {
            left: item_rect.left + 42,
            top: item_rect.top,
            right: item_rect.right - 8,
            bottom: item_rect.bottom,
        };
        let text_color = if selected {
            shell_palette().text_primary
        } else {
            shell_palette().text_muted
        };
        let icon_path = if selected && !item.selected_icon_path.trim().is_empty() {
            &item.selected_icon_path
        } else {
            &item.icon_path
        };
        let icon_rect = centered_icon_rect(
            RECT {
                left: item_rect.left + 18,
                top: item_rect.top,
                right: item_rect.left + 18 + SIDEBAR_ICON_SIZE,
                bottom: item_rect.bottom,
            },
            SIDEBAR_ICON_SIZE,
        );
        draw_icon_or_default(hdc, icon_path, icon_rect, SIDEBAR_ICON_SIZE as u32);
        draw_text(hdc, &item.text, label_rect, text_color, DT_LEFT);

        if let Some(badge) = item.badge.as_ref().filter(|badge| !badge.is_empty()) {
            draw_badge(hdc, item_rect, badge);
        } else if item.has_red_dot {
            draw_red_dot(hdc, item_rect);
        }
    }
}

fn draw_sidebar_rail(
    hdc: HDC,
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    cursor: Option<(i32, i32)>,
) {
    let app_rect = sidebar_rail_item_rect(rect, 0);
    fill_round_rect_aa(hdc, app_rect, 8, shell_palette().panel_background);
    draw_hover_wash(hdc, app_rect, 8, cursor);
    let app_icon_rect = centered_icon_rect(app_rect, SIDEBAR_RAIL_ICON_SIZE);
    draw_icon_or_default(
        hdc,
        &tabbar.app_icon_path,
        app_icon_rect,
        SIDEBAR_RAIL_ICON_SIZE as u32,
    );

    for (index, item) in tabbar.auxiliary_items.iter().enumerate() {
        let item_rect = sidebar_rail_item_rect(rect, 1 + index);
        if item.active {
            fill_round_rect_aa(hdc, item_rect, 8, shell_palette().panel_background);
        }
        draw_hover_wash(hdc, item_rect, 8, cursor);
        let icon_rect = centered_icon_rect(item_rect, SIDEBAR_RAIL_ICON_SIZE);
        let drew = match item.icon_png.as_deref() {
            Some(png) => draw_icon_from_png_bytes(hdc, &item.id, png, icon_rect),
            None => draw_icon_or_default(
                hdc,
                &item.icon_path,
                icon_rect,
                SIDEBAR_RAIL_ICON_SIZE as u32,
            ),
        };
        if !drew {
            draw_default_app_icon(hdc, icon_rect);
        }
    }

    // The new-tab "+" stays reachable while collapsed, mirroring the expanded
    // auxiliary section (full browser environment only).
    if tabbar.show_auxiliary_add {
        let add_rect = sidebar_rail_add_rect(rect, tabbar);
        draw_hover_wash(hdc, add_rect, 8, cursor);
        draw_frame_button_glyph(hdc, GLYPH_ADD, add_rect, shell_palette().text_muted);
    }

    // The collapse/expand toggle (same `SidebarExpand` design icon the top bar
    // uses when expanded) pinned to the bottom of the rail, so a collapsed rail
    // is never a dead end.
    let expand_rect = sidebar_rail_expand_rect(rect);
    draw_hover_wash(hdc, expand_rect, 8, cursor);
    draw_design_icon_button(
        hdc,
        expand_rect,
        WindowsDesignIcon::SidebarExpand,
        shell_palette().text_muted,
        18,
    );
}

/// The collapse/expand toggle cell, pinned to the bottom of an icon rail.
pub(super) fn sidebar_rail_expand_rect(rect: RECT) -> RECT {
    let cell = SIDEBAR_RAIL_ITEM_SIZE;
    let left = rect.left + (rect_width(&rect) - cell).max(0) / 2;
    let bottom = rect.bottom - SIDEBAR_ITEM_GAP;
    normalize_rect(RECT {
        left,
        top: bottom - cell,
        right: left + cell,
        bottom,
    })
}

/// The new-tab "+" cell, one slot past the app icon and auxiliary items.
pub(super) fn sidebar_rail_add_rect(rect: RECT, tabbar: &WindowsShellTabBarLayout) -> RECT {
    sidebar_rail_item_rect(rect, 1 + tabbar.auxiliary_items.len())
}

pub(super) fn sidebar_rail_item_rect(rect: RECT, index: usize) -> RECT {
    let cell = SIDEBAR_RAIL_ITEM_SIZE;
    let top = rect.top
        + SHELL_TOP_BAR_HEIGHT
        + SIDEBAR_ITEM_GAP
        + index as i32 * (cell + SIDEBAR_ITEM_GAP);
    let left = rect.left + (rect_width(&rect) - cell).max(0) / 2;
    normalize_rect(RECT {
        left,
        top,
        right: left + cell,
        bottom: top + cell,
    })
}

/// Chevron hit/draw rect at the trailing edge of the sidebar group header
/// row (the lxapp name).
fn sidebar_group_top(rect: RECT, tabbar: &WindowsShellTabBarLayout) -> i32 {
    rect.top + SHELL_TOP_BAR_HEIGHT + sidebar_pinned_grid_height(rect, tabbar)
}

pub(in crate::shell::chrome) fn sidebar_group_bottom(
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
) -> i32 {
    sidebar_group_top(rect, tabbar) + SIDEBAR_ITEM_HEIGHT
}

pub(super) fn sidebar_group_chevron_rect(rect: RECT, tabbar: &WindowsShellTabBarLayout) -> RECT {
    let group_top = sidebar_group_top(rect, tabbar);
    let group_bottom = sidebar_group_bottom(rect, tabbar);
    let top = group_top + (group_bottom - group_top - SIDEBAR_CHEVRON_SIZE).max(0) / 2;
    normalize_rect(RECT {
        left: rect.right - SIDEBAR_ITEM_INSET - SIDEBAR_CHEVRON_SIZE,
        top,
        right: rect.right - SIDEBAR_ITEM_INSET,
        bottom: top + SIDEBAR_CHEVRON_SIZE,
    })
}

/// Sidebar action buttons (settings/downloads) in the top caption strip,
/// hidden while the sidebar is collapsed. Right-aligned at the column's
/// trailing edge (flush with the chevron below) so the strip reads as two
/// groups - window controls leading, sidebar actions trailing - instead of
/// four packed icons. Actions that would reach the leading buttons drop.
pub(super) fn sidebar_header_action_rects(
    sidebar_rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
) -> Vec<(String, RECT)> {
    if tabbar.header_actions.is_empty() || tabbar.collapsed {
        return Vec::new();
    }
    let top = sidebar_rect.top + (SHELL_TOP_BAR_HEIGHT - SIDEBAR_HEADER_ACTION_SIZE).max(0) / 2;
    // Right edge of the leading app-menu + toggle buttons.
    let leading_limit = sidebar_rect.left
        + TOP_BAR_PADDING
        + 2 * TOP_BAR_BUTTON_SIZE
        + TOP_BAR_BUTTON_GAP
        + SIDEBAR_HEADER_ACTION_GAP;
    let mut right = sidebar_rect.right - SIDEBAR_ITEM_INSET;
    let mut out = Vec::with_capacity(tabbar.header_actions.len());
    // Reverse order from the trailing edge keeps the declared left-to-right
    // reading order.
    for action in tabbar.header_actions.iter().rev() {
        let left = right - SIDEBAR_HEADER_ACTION_SIZE;
        if left < leading_limit {
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
        right = left - SIDEBAR_HEADER_ACTION_GAP;
    }
    out
}

pub(super) fn draw_tabbar_border(hdc: HDC, rect: RECT, tabbar: &WindowsShellTabBarLayout) {
    match tabbar.position {
        WindowsShellTabBarPosition::Bottom => draw_top_border(hdc, rect, tabbar.border_color),
        WindowsShellTabBarPosition::Left => draw_right_border(hdc, rect, tabbar.border_color),
        WindowsShellTabBarPosition::Right => draw_left_border(hdc, rect, tabbar.border_color),
    }
}

pub(super) fn tab_item_rect(
    rect: RECT,
    position: WindowsShellTabBarPosition,
    count: usize,
    index: usize,
) -> RECT {
    let count_i32 = count.max(1) as i32;
    let index_i32 = index as i32;
    match position {
        WindowsShellTabBarPosition::Bottom => {
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
        WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right => {
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

pub(super) fn sidebar_item_rect(
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    index: usize,
) -> RECT {
    let top = rect.top
        + SHELL_TOP_BAR_HEIGHT
        + sidebar_pinned_grid_height(rect, tabbar)
        + SIDEBAR_ITEM_HEIGHT
        + SIDEBAR_PARENT_CHILD_GAP
        + index as i32 * (SIDEBAR_CHILD_ITEM_HEIGHT + SIDEBAR_CHILD_ITEM_GAP);
    normalize_rect(RECT {
        left: rect.left + SIDEBAR_ITEM_INSET,
        top,
        right: rect.right - SIDEBAR_ITEM_INSET,
        bottom: top + SIDEBAR_CHILD_ITEM_HEIGHT,
    })
}
