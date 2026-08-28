//! Sidebar and tab bar chrome.

use crate::WindowsDesignIcon;

use super::*;

mod auxiliary;
mod footer_action;
pub(super) use auxiliary::*;
pub(super) use footer_action::*;

/// Phone bottom tab bar: 49px item strip plus a lower safe-area hit region.
const BOTTOM_TAB_ICON_SIZE: i32 = 22;
const BOTTOM_TAB_ITEM_HEIGHT: i32 = 49;
const BOTTOM_TAB_ICON_TOP: i32 = 5;
const BOTTOM_TAB_LABEL_TOP_GAP: i32 = 1;
/// Circle behind a selected single-icon tab, standing in for the selected
/// artwork it does not have. Matches the mobile hosts.
const ACTIVE_INDICATOR_SIZE: i32 = 36;
/// How far the indicator sits from the bar toward the selected colour. GDI has
/// no alpha here, so the tint is mixed against the plate instead.
const ACTIVE_INDICATOR_MIX_PERCENT: u32 = 20;

pub(super) fn draw_tab_bar(
    hdc: HDC,
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    cursor: Option<(i32, i32)>,
    scroll_offset: i32,
    viewport_bottom: i32,
) {
    draw_tab_bar_inner(
        hdc,
        rect,
        tabbar,
        cursor,
        scroll_offset,
        viewport_bottom,
        None,
    );
}

pub(super) fn draw_tab_bar_with_layered_text(
    hdc: HDC,
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    cursor: Option<(i32, i32)>,
    scroll_offset: i32,
    viewport_bottom: i32,
    text_runs: &mut Vec<LayeredTextRun>,
) {
    draw_tab_bar_inner(
        hdc,
        rect,
        tabbar,
        cursor,
        scroll_offset,
        viewport_bottom,
        Some(text_runs),
    );
}

fn draw_tab_bar_inner(
    hdc: HDC,
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    cursor: Option<(i32, i32)>,
    scroll_offset: i32,
    viewport_bottom: i32,
    mut text_runs: Option<&mut Vec<LayeredTextRun>>,
) {
    if matches!(
        tabbar.position,
        WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
    ) {
        draw_sidebar_tab_bar(hdc, rect, tabbar, cursor, scroll_offset, viewport_bottom);
        return;
    }

    if !tabbar.background_transparent {
        fill_rect(hdc, rect, tabbar.background_color);
        draw_tabbar_border(hdc, rect, tabbar);
    }

    // Past the strip's capacity the last cell becomes "more" and stands in for
    // every folded item, so slots and items stop being the same list.
    let count = tabbar.bottom_slot_count();
    if count == 0 {
        return;
    }
    let overflow_start = tabbar.bottom_overflow_start();

    for slot in 0..count {
        let item_rect = tab_item_rect(rect, tabbar.position, count, slot);
        let item = tabbar.items.get(slot);
        let is_more = overflow_start.is_some_and(|start| slot == start);
        let selected = if is_more {
            overflow_start.is_some_and(|start| tabbar.selected_index >= start as i32)
        } else {
            tabbar.selected_index == slot as i32
        };
        let color = if selected {
            tabbar.selected_color
        } else {
            tabbar.color
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

        let drew_icon = if is_more {
            // The overflow glyph is shell chrome, so it tints with the strip
            // instead of coming from the lxapp's bundle.
            draw_design_icon_button(
                hdc,
                icon_rect,
                WindowsDesignIcon::PageMenu,
                color,
                BOTTOM_TAB_ICON_SIZE,
            );
            true
        } else {
            let Some(item) = item else { continue };
            // An item with only one icon has no swap to signal selection, so
            // the strip marks it with an indicator behind the artwork.
            if selected && !item.has_selected_icon {
                draw_active_indicator(hdc, icon_rect, tabbar);
            }
            // Phone tab cell: the lxapp's pre-tinted icon stacked over its
            // label, both centered. A bundle that ships separate normal and
            // selected icons has its PNG drawn as-is.
            let icon_path = if selected && !item.selected_icon_path.trim().is_empty() {
                item.selected_icon_path.as_str()
            } else {
                item.icon_path.as_str()
            };
            !icon_path.trim().is_empty()
                && draw_icon_from_path(hdc, icon_path, icon_rect, BOTTOM_TAB_ICON_SIZE as u32)
        };

        let label = if is_more {
            lingxia_logic::i18n::t(lingxia_logic::I18nKey::TabbarMore)
        } else {
            match item {
                Some(item) => item.text.clone(),
                None => continue,
            }
        };

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
            if let Some(text_runs) = text_runs.as_deref_mut() {
                text_runs.push(LayeredTextRun {
                    rect: label_rect,
                    color,
                    text: label.clone(),
                    font_height: logical_font_height(hdc, SHELL_TEXT_POINT_SIZE),
                    font_weight: SHELL_TEXT_WEIGHT,
                });
            } else {
                draw_text_antialiased(hdc, &label, label_rect, color, DT_CENTER);
            }
        } else {
            draw_text(hdc, &label, label_rect, color, DT_CENTER);
        }

        let badge_anchor = if drew_icon { icon_rect } else { item_rect };
        if is_more {
            // Folded badges still have to surface, so "more" aggregates them.
            if tabbar.overflow_has_notification() {
                draw_red_dot(hdc, badge_anchor);
            }
        } else if let Some(item) = item {
            if let Some(badge) = item.badge.as_ref().filter(|badge| !badge.is_empty()) {
                draw_badge(hdc, badge_anchor, badge);
            } else if item.has_red_dot {
                draw_red_dot(hdc, badge_anchor);
            }
        }
    }
}

/// Rounded plate behind the icon of a selected single-icon item, matching the
/// active indicator the mobile hosts draw.
fn draw_active_indicator(hdc: HDC, icon_rect: RECT, tabbar: &WindowsShellTabBarLayout) {
    let center_x = (icon_rect.left + icon_rect.right) / 2;
    let center_y = (icon_rect.top + icon_rect.bottom) / 2;
    let plate = RECT {
        left: center_x - ACTIVE_INDICATOR_SIZE / 2,
        top: center_y - ACTIVE_INDICATOR_SIZE / 2,
        right: center_x + ACTIVE_INDICATOR_SIZE / 2,
        bottom: center_y + ACTIVE_INDICATOR_SIZE / 2,
    };
    // An immersive bar paints no plate of its own, so mix against the shell.
    let behind = if tabbar.background_transparent {
        shell_palette().sidebar_background
    } else {
        tabbar.background_color
    };
    fill_round_rect_aa(
        hdc,
        plate,
        ACTIVE_INDICATOR_SIZE / 2,
        blend_rgb(tabbar.selected_color, behind, ACTIVE_INDICATOR_MIX_PERCENT),
    );
}

pub(super) fn draw_sidebar_tab_bar(
    hdc: HDC,
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    cursor: Option<(i32, i32)>,
    scroll_offset: i32,
    viewport_bottom: i32,
) {
    if rect_width(&rect) == 0 {
        return;
    }
    // The app's tabbar background styles its expanded child strip on macOS;
    // it does not replace the whole desktop sidebar surface.
    fill_rect(hdc, rect, shell_palette().sidebar_background);

    // Icon-only rail: first-level entries only, centered in a compact column.
    if tabbar.collapsed || tabbar.icon_rail {
        draw_sidebar_rail(hdc, rect, tabbar, cursor, scroll_offset, viewport_bottom);
        return;
    }

    let saved = unsafe { SaveDC(hdc) };
    unsafe {
        let _ = IntersectClipRect(
            hdc,
            rect.left,
            rect.top + SHELL_TOP_BAR_HEIGHT,
            rect.right,
            viewport_bottom,
        );
    }

    let title = if tabbar.app_name.trim().is_empty() {
        "LxApp".to_string()
    } else {
        tabbar.app_name.clone()
    };
    let group_rect = sidebar_group_rect(rect, tabbar, scroll_offset);
    let chevron_rect = sidebar_group_chevron_rect(rect, tabbar, scroll_offset);
    let close_rect = sidebar_group_close_rect(rect, tabbar, scroll_offset);
    let menu_rect = sidebar_group_menu_rect(rect, tabbar, scroll_offset);
    // The active lxapp is a top-level tab just like a web tab. macOS gives the
    // group header a quiet wash while its selected child carries the stronger
    // page-level card, preserving both levels without stacking white pills.
    if tabbar.group_active {
        fill_round_rect_aa(hdc, group_rect, 6, shell_palette().group_active_background);
    } else {
        draw_hover_wash(hdc, group_rect, 6, cursor);
    }
    // The lxapp's own icon (via the app-info API) leads the group header.
    let icon_rect = sidebar_top_level_icon_rect(group_rect, SIDEBAR_ICON_SIZE);
    draw_icon_or_default(
        hdc,
        &tabbar.app_icon_path,
        icon_rect,
        SIDEBAR_ICON_SIZE as u32,
    );
    let show_chevron = !tabbar.items_api_hidden && !tabbar.items.is_empty();
    let header_rect = sidebar_group_title_rect(rect, tabbar, scroll_offset);
    draw_text(
        hdc,
        &title,
        header_rect,
        shell_palette().text_primary,
        DT_LEFT,
    );
    if show_chevron {
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
    }
    if tabbar.group_closable && rect_contains(&group_rect, cursor.unwrap_or((-1, -1))) {
        draw_hover_wash(hdc, close_rect, 4, cursor);
        draw_text(
            hdc,
            GLYPH_TAB_CLOSE,
            close_rect,
            shell_palette().text_muted,
            DT_CENTER,
        );
    }
    if rect_contains(&group_rect, cursor.unwrap_or((-1, -1))) {
        draw_hover_wash(hdc, menu_rect, 4, cursor);
        draw_design_icon_button(
            hdc,
            menu_rect,
            WindowsDesignIcon::PageMenu,
            shell_palette().text_muted,
            16,
        );
    }
    if !tabbar.items_collapsed {
        draw_sidebar_items(hdc, rect, tabbar, cursor, scroll_offset);
    }

    draw_sidebar_auxiliary_section(hdc, rect, tabbar, cursor, scroll_offset, viewport_bottom);
    unsafe {
        let _ = RestoreDC(hdc, saved);
    }

    if tabbar.footer_action_height > 0 {
        let footer_top = rect.bottom - tabbar.footer_action_height;
        // The footer is host chrome, so its separator follows the shell theme
        // instead of inheriting the lxapp tabbar's page-owned border color.
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
    }
    for (action_id, action_rect) in sidebar_header_action_rects(rect, tabbar) {
        let Some(action) = tabbar
            .header_actions
            .iter()
            .find(|action| action.id == action_id)
        else {
            continue;
        };
        if !action.disabled {
            draw_hover_wash(hdc, action_rect, 6, cursor);
        }
        let icon_rect = centered_icon_rect(action_rect, 16);
        let _ = draw_icon_from_path(hdc, &action.icon_path, icon_rect, 16);
    }
}

/// Shared icon box for top-level lxapp and web tabs. Keeping this geometry in
/// one function prevents their draw paths from silently acquiring different
/// leading padding again.
pub(super) fn sidebar_top_level_icon_rect(item_rect: RECT, icon_size: i32) -> RECT {
    let top = item_rect.top + (rect_height(&item_rect) - icon_size).max(0) / 2;
    normalize_rect(RECT {
        left: item_rect.left + SIDEBAR_TOP_LEVEL_ICON_INSET,
        top,
        right: item_rect.left + SIDEBAR_TOP_LEVEL_ICON_INSET + icon_size,
        bottom: top + icon_size,
    })
}

/// Draws the indented lxapp leaves plus a parent-child guide. The guide stays
/// outside the selected pill, so the group remains legible without another
/// enclosing card.
pub(super) fn draw_sidebar_items(
    hdc: HDC,
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    cursor: Option<(i32, i32)>,
    scroll_offset: i32,
) {
    if !tabbar.background_transparent && !tabbar.items.is_empty() {
        let last = sidebar_item_rect(rect, tabbar, tabbar.items.len() - 1, scroll_offset);
        fill_round_rect_aa(
            hdc,
            RECT {
                left: rect.left + SIDEBAR_ITEM_INSET,
                top: sidebar_group_bottom(rect, tabbar, scroll_offset),
                right: rect.right - SIDEBAR_ITEM_INSET,
                bottom: last.bottom,
            },
            6,
            tabbar.background_color,
        );
    }

    if !tabbar.items.is_empty() {
        let last = sidebar_item_rect(rect, tabbar, tabbar.items.len() - 1, scroll_offset);
        // Same 12pt attribution axis as macOS (group inset + 12).
        let guide_x = rect.left + SIDEBAR_ITEM_INSET + 12;
        fill_rect(
            hdc,
            RECT {
                left: guide_x,
                top: sidebar_group_bottom(rect, tabbar, scroll_offset) - 2,
                right: guide_x + 1,
                bottom: (last.bottom - 7)
                    .max(sidebar_group_bottom(rect, tabbar, scroll_offset) - 2),
            },
            shell_palette().divider,
        );
    }

    for (index, item) in tabbar.items.iter().enumerate() {
        let item_rect = sidebar_item_rect(rect, tabbar, index, scroll_offset);
        let selected = tabbar.selected_index == index as i32;
        if selected {
            // A flat selection avoids the horizontal shadow residue that was
            // especially visible while the old and new rows repainted during
            // tab switches. The accent guide already carries active emphasis.
            fill_round_rect_aa(hdc, item_rect, 5, shell_palette().selection_background);
            let guide_x = rect.left + SIDEBAR_ITEM_INSET + 12;
            fill_round_rect_aa(
                hdc,
                RECT {
                    left: guide_x - 1,
                    top: item_rect.top + 6,
                    right: guide_x + 2,
                    bottom: item_rect.bottom - 6,
                },
                2,
                tabbar.selected_color,
            );
        } else {
            draw_hover_wash(hdc, item_rect, 6, cursor);
        }

        let label_rect = RECT {
            left: item_rect.left + 32,
            top: item_rect.top,
            right: item_rect.right - 8,
            bottom: item_rect.bottom,
        };
        let text_color = if selected {
            tabbar.selected_color
        } else {
            tabbar.color
        };
        let icon_path = if selected && !item.selected_icon_path.trim().is_empty() {
            &item.selected_icon_path
        } else {
            &item.icon_path
        };
        let icon_rect = centered_icon_rect(
            RECT {
                left: item_rect.left + 8,
                top: item_rect.top,
                right: item_rect.left + 8 + SIDEBAR_ICON_SIZE,
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
    scroll_offset: i32,
    viewport_bottom: i32,
) {
    let saved = unsafe { SaveDC(hdc) };
    unsafe {
        let _ = IntersectClipRect(
            hdc,
            rect.left,
            rect.top + SHELL_TOP_BAR_HEIGHT,
            rect.right,
            viewport_bottom,
        );
    }
    if let Some(divider) = sidebar_rail_pinned_divider_rect(rect, tabbar, scroll_offset) {
        fill_rect(hdc, divider, shell_palette().divider);
    }
    let app_rect = sidebar_rail_item_rect(rect, sidebar_group_rail_index(tabbar), scroll_offset);
    if tabbar.group_active {
        fill_round_rect_aa(hdc, app_rect, 6, shell_palette().selection_background);
    }
    draw_hover_wash(hdc, app_rect, 6, cursor);
    let app_icon_rect = centered_icon_rect(app_rect, SIDEBAR_RAIL_ICON_SIZE);
    draw_icon_or_default(
        hdc,
        &tabbar.app_icon_path,
        app_icon_rect,
        SIDEBAR_RAIL_ICON_SIZE as u32,
    );
    if tabbar.group_active
        && tabbar.group_closable
        && rect_contains(&app_rect, cursor.unwrap_or((-1, -1)))
    {
        draw_sidebar_rail_close(hdc, app_rect);
    }

    for (index, item) in tabbar.auxiliary_items.iter().enumerate() {
        let item_rect = sidebar_rail_item_rect(
            rect,
            sidebar_auxiliary_rail_index(tabbar, index),
            scroll_offset,
        );
        if item.active {
            fill_round_rect_aa(hdc, item_rect, 6, shell_palette().selection_background);
        }
        draw_hover_wash(hdc, item_rect, 6, cursor);
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
        if item.active && item.closable && rect_contains(&item_rect, cursor.unwrap_or((-1, -1))) {
            draw_sidebar_rail_close(hdc, item_rect);
        }
    }

    // The new-tab "+" stays reachable while collapsed, mirroring the expanded
    // auxiliary section (full browser environment only).
    if tabbar.show_auxiliary_add {
        let add_rect = sidebar_rail_add_rect(rect, tabbar, scroll_offset);
        draw_hover_wash(hdc, add_rect, 8, cursor);
        draw_frame_button_glyph(hdc, GLYPH_ADD, add_rect, shell_palette().text_muted);
    }

    unsafe {
        let _ = RestoreDC(hdc, saved);
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

fn draw_sidebar_rail_close(hdc: HDC, item_rect: RECT) {
    let close_rect = sidebar_rail_close_rect(item_rect);
    fill_round_rect_aa(hdc, close_rect, 6, shell_palette().control_surface);
    draw_text(
        hdc,
        GLYPH_TAB_CLOSE,
        close_rect,
        shell_palette().text_primary,
        DT_CENTER,
    );
}

/// Direct-close target overlaid on a closable collapsed-rail switcher.
pub(super) fn sidebar_rail_close_rect(item_rect: RECT) -> RECT {
    const SIZE: i32 = 24;
    let left = item_rect.left + (rect_width(&item_rect) - SIZE).max(0) / 2;
    let top = item_rect.top + (rect_height(&item_rect) - SIZE).max(0) / 2;
    normalize_rect(RECT {
        left,
        top,
        right: left + SIZE,
        bottom: top + SIZE,
    })
}

/// Separator between user-owned pins and live workspace switchers.
pub(super) fn sidebar_rail_pinned_divider_rect(
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    scroll_offset: i32,
) -> Option<RECT> {
    let pinned = sidebar_pinned_count(tabbar);
    let has_open_items = !tabbar.group_target_id.trim().is_empty()
        || tabbar.auxiliary_items.len().saturating_sub(pinned) > 0;
    if pinned == 0 || !has_open_items {
        return None;
    }

    let last_pin = sidebar_rail_item_rect(rect, pinned - 1, scroll_offset);
    let first_open = sidebar_rail_item_rect(rect, pinned, scroll_offset);
    let width = 22.min(rect_width(&rect));
    let left = rect.left + (rect_width(&rect) - width).max(0) / 2;
    let top = (last_pin.bottom + first_open.top) / 2;
    Some(normalize_rect(RECT {
        left,
        top,
        right: left + width,
        bottom: top + 1,
    }))
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
pub(super) fn sidebar_rail_add_rect(
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    scroll_offset: i32,
) -> RECT {
    sidebar_rail_item_rect(rect, 1 + tabbar.auxiliary_items.len(), scroll_offset)
}

pub(super) fn sidebar_group_rail_index(tabbar: &WindowsShellTabBarLayout) -> usize {
    let pinned = sidebar_pinned_count(tabbar);
    pinned
        + tabbar
            .group_order_index
            .min(tabbar.auxiliary_items.len().saturating_sub(pinned))
}

pub(super) fn sidebar_auxiliary_rail_index(
    tabbar: &WindowsShellTabBarLayout,
    auxiliary_index: usize,
) -> usize {
    let pinned = sidebar_pinned_count(tabbar);
    if auxiliary_index < pinned {
        return auxiliary_index;
    }
    let unpinned_index = auxiliary_index - pinned;
    pinned + unpinned_index + usize::from(unpinned_index >= tabbar.group_order_index)
}

pub(super) fn sidebar_rail_item_rect(rect: RECT, index: usize, scroll_offset: i32) -> RECT {
    let cell = SIDEBAR_RAIL_ITEM_SIZE;
    let top = rect.top
        + SHELL_TOP_BAR_HEIGHT
        + SIDEBAR_ITEM_GAP
        + index as i32 * (cell + SIDEBAR_ITEM_GAP)
        - scroll_offset;
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
fn sidebar_group_top(rect: RECT, tabbar: &WindowsShellTabBarLayout, scroll_offset: i32) -> i32 {
    let pinned = sidebar_pinned_count(tabbar);
    let unpinned = tabbar.auxiliary_items.len().saturating_sub(pinned);
    rect.top
        + SHELL_TOP_BAR_HEIGHT
        + sidebar_pinned_grid_height(rect, tabbar)
        + tabbar.group_order_index.min(unpinned) as i32 * (SIDEBAR_ITEM_HEIGHT + SIDEBAR_ITEM_GAP)
        - scroll_offset
}

pub(in crate::shell::chrome) fn sidebar_group_bottom(
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    scroll_offset: i32,
) -> i32 {
    sidebar_group_top(rect, tabbar, scroll_offset) + SIDEBAR_ITEM_HEIGHT
}

pub(super) fn sidebar_group_rect(
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    scroll_offset: i32,
) -> RECT {
    normalize_rect(RECT {
        left: rect.left + SIDEBAR_ITEM_INSET,
        top: sidebar_group_top(rect, tabbar, scroll_offset),
        right: rect.right - SIDEBAR_ITEM_INSET,
        bottom: sidebar_group_bottom(rect, tabbar, scroll_offset),
    })
}

pub(in crate::shell::chrome) fn sidebar_group_title_rect(
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    scroll_offset: i32,
) -> RECT {
    let group_rect = sidebar_group_rect(rect, tabbar, scroll_offset);
    let icon_rect = sidebar_top_level_icon_rect(group_rect, SIDEBAR_ICON_SIZE);
    let show_chevron = !tabbar.items_api_hidden && !tabbar.items.is_empty();
    let right = if tabbar.group_closable {
        sidebar_group_close_rect(rect, tabbar, scroll_offset).left - 4
    } else if show_chevron {
        sidebar_group_chevron_rect(rect, tabbar, scroll_offset).left - 4
    } else {
        rect.right - SIDEBAR_ITEM_INSET
    };
    normalize_rect(RECT {
        left: icon_rect.right + 8,
        top: group_rect.top,
        right,
        bottom: group_rect.bottom,
    })
}

pub(super) fn sidebar_group_chevron_rect(
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    scroll_offset: i32,
) -> RECT {
    let group_top = sidebar_group_top(rect, tabbar, scroll_offset);
    let group_bottom = sidebar_group_bottom(rect, tabbar, scroll_offset);
    let top = group_top + (group_bottom - group_top - SIDEBAR_CHEVRON_SIZE).max(0) / 2;
    normalize_rect(RECT {
        left: rect.right - SIDEBAR_ITEM_INSET - SIDEBAR_CHEVRON_SIZE,
        top,
        right: rect.right - SIDEBAR_ITEM_INSET,
        bottom: top + SIDEBAR_CHEVRON_SIZE,
    })
}

pub(super) fn sidebar_group_close_rect(
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    scroll_offset: i32,
) -> RECT {
    let chevron = sidebar_group_chevron_rect(rect, tabbar, scroll_offset);
    normalize_rect(RECT {
        left: chevron.left - SIDEBAR_BROWSER_CLOSE_SIZE,
        top: sidebar_group_top(rect, tabbar, scroll_offset),
        right: chevron.left,
        bottom: sidebar_group_bottom(rect, tabbar, scroll_offset),
    })
}

pub(super) fn sidebar_group_menu_rect(
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    scroll_offset: i32,
) -> RECT {
    let trailing = if tabbar.group_closable {
        sidebar_group_close_rect(rect, tabbar, scroll_offset).left
    } else {
        sidebar_group_chevron_rect(rect, tabbar, scroll_offset).left
    };
    normalize_rect(RECT {
        left: trailing - SIDEBAR_BROWSER_CLOSE_SIZE,
        top: sidebar_group_top(rect, tabbar, scroll_offset),
        right: trailing,
        bottom: sidebar_group_bottom(rect, tabbar, scroll_offset),
    })
}

/// Width the leading controls take before the action strip can start.
///
/// Header actions only draw while the sidebar is expanded, so the collapse
/// toggle is always there; the app-menu button beside it exists only in the
/// product shell — a runner-style build has no menu worth showing and draws
/// none. Reserving for it regardless leaves a visibly empty slot the header
/// then refuses to use.
fn header_leading_reserve() -> i32 {
    let app_menu = if cfg!(feature = "browser-shell") {
        TOP_BAR_BUTTON_SIZE + TOP_BAR_BUTTON_GAP
    } else {
        0
    };
    TOP_BAR_PADDING + app_menu + TOP_BAR_BUTTON_SIZE + SIDEBAR_HEADER_ACTION_GAP
}

/// How many header actions a strip of `available` pixels can seat. The last
/// one needs no trailing gap, so the run is `n * SIZE + (n - 1) * GAP`.
///
/// `lingxia_shell::MAX_HEADER_SIDEBAR_ACTIONS` is the contract an lxapp is held
/// to at declaration time; this is what the window can actually show right now.
/// At `SHELL_SIDEBAR_WIDTH` the two agree — widening the buttons or the leading
/// controls without revisiting the limit would let an app declare an action
/// that never draws.
fn header_action_capacity(available: i32) -> usize {
    if available < SIDEBAR_HEADER_ACTION_SIZE {
        return 0;
    }
    let stride = SIDEBAR_HEADER_ACTION_SIZE + SIDEBAR_HEADER_ACTION_GAP;
    ((available + SIDEBAR_HEADER_ACTION_GAP) / stride).max(0) as usize
}

/// Sidebar action buttons in the top caption strip,
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
    let leading_limit = sidebar_rect.left + header_leading_reserve();
    let mut right = sidebar_rect.right - SIDEBAR_ITEM_INSET;
    // Draw the ones that fit rather than measuring the whole set and giving up
    // on it: a sidebar one icon too narrow would otherwise lose the buttons
    // that did fit, which reads as the header having lost them all.
    let shown = tabbar
        .header_actions
        .len()
        .min(header_action_capacity(right - leading_limit));
    if shown == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(shown);
    // Reverse order from the trailing edge keeps the declared left-to-right
    // reading order; an overflow drops the last declared, not the first.
    for action in tabbar.header_actions[..shown].iter().rev() {
        let left = right - SIDEBAR_HEADER_ACTION_SIZE;
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
    scroll_offset: i32,
) -> RECT {
    let top = sidebar_group_top(rect, tabbar, scroll_offset)
        + SIDEBAR_ITEM_HEIGHT
        + SIDEBAR_PARENT_CHILD_GAP
        + index as i32 * (SIDEBAR_CHILD_ITEM_HEIGHT + SIDEBAR_CHILD_ITEM_GAP);
    normalize_rect(RECT {
        left: rect.left + SIDEBAR_ITEM_INSET + SIDEBAR_CHILD_INDENT,
        top,
        right: rect.right - SIDEBAR_ITEM_INSET,
        bottom: top + SIDEBAR_CHILD_ITEM_HEIGHT,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lingxia_shell::MAX_HEADER_SIDEBAR_ACTIONS;

    /// Space the strip has left once the leading controls took theirs.
    fn available_at(sidebar_width: i32) -> i32 {
        sidebar_width - SIDEBAR_ITEM_INSET - header_leading_reserve()
    }

    /// The declaration limit is only honest if the standard sidebar can seat
    /// every action it allows. Widening the buttons or the leading controls
    /// without revisiting the limit trips this, instead of a user finding a
    /// declared action that never draws.
    #[test]
    fn the_contract_limit_fits_the_standard_sidebar() {
        assert!(
            header_action_capacity(available_at(SHELL_SIDEBAR_WIDTH)) >= MAX_HEADER_SIDEBAR_ACTIONS,
            "the standard sidebar must seat every action the contract allows"
        );
    }

    /// Dropping the app-menu button hands its slot to the actions rather than
    /// leaving a gap where it would have been.
    #[test]
    fn the_reserve_tracks_the_buttons_that_exist() {
        let expected = if cfg!(feature = "browser-shell") {
            TOP_BAR_PADDING
                + 2 * TOP_BAR_BUTTON_SIZE
                + TOP_BAR_BUTTON_GAP
                + SIDEBAR_HEADER_ACTION_GAP
        } else {
            TOP_BAR_PADDING + TOP_BAR_BUTTON_SIZE + SIDEBAR_HEADER_ACTION_GAP
        };
        assert_eq!(header_leading_reserve(), expected);
    }

    #[test]
    fn capacity_counts_one_action_at_a_time() {
        let stride = SIDEBAR_HEADER_ACTION_SIZE + SIDEBAR_HEADER_ACTION_GAP;
        assert_eq!(header_action_capacity(SIDEBAR_HEADER_ACTION_SIZE - 1), 0);
        assert_eq!(header_action_capacity(SIDEBAR_HEADER_ACTION_SIZE), 1);
        assert_eq!(
            header_action_capacity(stride + SIDEBAR_HEADER_ACTION_SIZE),
            2
        );
    }

    /// A strip too narrow for the whole set keeps what it can draw. Coming back
    /// empty is what made one action too many look like losing them all.
    #[test]
    fn a_narrow_strip_still_seats_what_it_can() {
        // One button's worth, so the strip is short of the contract limit
        // however that limit is later set.
        let narrow = SIDEBAR_HEADER_ACTION_SIZE;
        assert_eq!(header_action_capacity(narrow), 1);
        assert!(header_action_capacity(narrow) < MAX_HEADER_SIDEBAR_ACTIONS);
        assert_eq!(header_action_capacity(0), 0);
    }
}
