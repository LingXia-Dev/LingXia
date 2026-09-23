//! Sidebar auxiliary rows.

use super::*;
use crate::dpi::px;

fn pinned_shortcut_size() -> i32 {
    crate::dpi::px(36)
}
fn pinned_shortcut_icon_size() -> i32 {
    crate::dpi::px(20)
}
fn pinned_shortcut_gap() -> i32 {
    crate::dpi::px(5)
}
const PINNED_SHORTCUT_COLUMNS: usize = 4;

pub(in crate::shell::chrome) fn sidebar_pinned_count(tabbar: &WindowsShellTabBarLayout) -> usize {
    tabbar
        .auxiliary_items
        .iter()
        .take_while(|item| item.pinned)
        .count()
}

pub(in crate::shell::chrome) fn sidebar_pinned_grid_height(
    _rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
) -> i32 {
    let count = sidebar_pinned_count(tabbar);
    if count == 0 {
        return 0;
    }
    pinned_grid_height(count)
}

fn pinned_grid_height(count: usize) -> i32 {
    let stride = pinned_shortcut_size() + pinned_shortcut_gap();
    count.div_ceil(PINNED_SHORTCUT_COLUMNS) as i32 * stride
}

/// Geometry of the sidebar auxiliary section: separator line, one row rect
/// per auxiliary item (rows that would collide with the footer are dropped),
/// and the add row.
pub(in crate::shell::chrome) struct SidebarAuxiliaryRects {
    /// Visible row rects paired with their index in `tabbar.auxiliary_items`.
    pub(in crate::shell::chrome) items: Vec<(usize, RECT)>,
    pub(in crate::shell::chrome) add: Option<RECT>,
}

pub(in crate::shell::chrome) fn sidebar_auxiliary_rects(
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    scroll_offset: i32,
    viewport_bottom: i32,
) -> Option<SidebarAuxiliaryRects> {
    if tabbar.auxiliary_items.is_empty() && !tabbar.show_auxiliary_add {
        return None;
    }
    let viewport_top = rect.top + shell_top_bar_height();
    // A collapsed items group hides its rows; the auxiliary section moves up
    // directly under the group header.
    let pinned_height = sidebar_pinned_grid_height(rect, tabbar);
    let items_height = if tabbar.items_collapsed || tabbar.items.is_empty() {
        0
    } else {
        sidebar_parent_child_gap()
            + tabbar.items.len() as i32 * sidebar_child_item_height()
            + (tabbar.items.len() as i32 - 1) * sidebar_child_item_gap()
    };
    let top_level_start = rect.top + shell_top_bar_height() + pinned_height;
    let row = |top: i32| -> RECT {
        normalize_rect(RECT {
            left: rect.left + sidebar_item_inset(),
            top: top - scroll_offset,
            right: rect.right - sidebar_item_inset(),
            bottom: top - scroll_offset + sidebar_item_height(),
        })
    };
    let visible = |rect: RECT| rect.bottom > viewport_top && rect.top < viewport_bottom;

    let mut items = Vec::with_capacity(tabbar.auxiliary_items.len());
    let pinned_count = sidebar_pinned_count(tabbar);
    if pinned_count > 0 {
        // Leading-aligned on the shared icon axis (not centered): the first
        // tile's icon lines up with the top-level rows, and the grid no
        // longer drifts when the sidebar is resized.
        let grid_left = rect.left + sidebar_icon_axis() - pinned_shortcut_size() / 2;
        // Pins are global shortcuts, not children of the current
        // lxapp group. They sit immediately below the caption controls and
        // above the lxapp header/navigation, matching macOS.
        let grid_top = rect.top + shell_top_bar_height() - scroll_offset;
        for index in 0..pinned_count {
            let row = index / PINNED_SHORTCUT_COLUMNS;
            let column = index % PINNED_SHORTCUT_COLUMNS;
            let left = grid_left + column as i32 * (pinned_shortcut_size() + pinned_shortcut_gap());
            let top = grid_top + row as i32 * (pinned_shortcut_size() + pinned_shortcut_gap());
            let pinned_rect = normalize_rect(RECT {
                left,
                top,
                right: left + pinned_shortcut_size(),
                bottom: top + pinned_shortcut_size(),
            });
            if visible(pinned_rect) {
                items.push((index, pinned_rect));
            }
        }
    }
    let unpinned_count = tabbar.auxiliary_items.len().saturating_sub(pinned_count);
    let group_index = tabbar.group_order_index.min(unpinned_count);
    let top_level_stride = sidebar_item_height() + sidebar_item_gap();
    let group_top = top_level_start + group_index as i32 * top_level_stride;
    for index in 0..unpinned_count {
        let top = if index < group_index {
            top_level_start + index as i32 * top_level_stride
        } else {
            group_top
                + sidebar_item_height()
                + items_height
                + sidebar_item_gap()
                + (index - group_index) as i32 * top_level_stride
        };
        let item_rect = row(top);
        if visible(item_rect) {
            items.push((pinned_count + index, item_rect));
        }
    }
    let add = if tabbar.show_auxiliary_add {
        let top = group_top
            + sidebar_item_height()
            + items_height
            + sidebar_item_gap()
            + unpinned_count.saturating_sub(group_index) as i32 * top_level_stride;
        let add = row(top);
        visible(add).then_some(add)
    } else {
        None
    };

    Some(SidebarAuxiliaryRects { items, add })
}

pub(in crate::shell::chrome) fn sidebar_content_bottom(
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
) -> i32 {
    if tabbar.collapsed || tabbar.icon_rail {
        let count = 1 + tabbar.auxiliary_items.len() + usize::from(tabbar.show_auxiliary_add);
        return rect.top
            + shell_top_bar_height()
            + sidebar_item_gap()
            + count as i32 * (sidebar_rail_item_size() + sidebar_item_gap());
    }
    let pinned_count = sidebar_pinned_count(tabbar);
    let pinned_height = sidebar_pinned_grid_height(rect, tabbar);
    let unpinned_count = tabbar.auxiliary_items.len().saturating_sub(pinned_count);
    let group_index = tabbar.group_order_index.min(unpinned_count);
    let stride = sidebar_item_height() + sidebar_item_gap();
    let start = rect.top + shell_top_bar_height() + pinned_height;
    let group_top = start + group_index as i32 * stride;
    let items_height = if tabbar.items_collapsed || tabbar.items.is_empty() {
        0
    } else {
        sidebar_parent_child_gap()
            + tabbar.items.len() as i32 * sidebar_child_item_height()
            + (tabbar.items.len() as i32 - 1) * sidebar_child_item_gap()
    };
    let mut bottom = group_top + sidebar_item_height() + items_height;
    for index in 0..unpinned_count {
        let top = if index < group_index {
            start + index as i32 * stride
        } else {
            group_top
                + sidebar_item_height()
                + items_height
                + sidebar_item_gap()
                + (index - group_index) as i32 * stride
        };
        bottom = bottom.max(top + sidebar_item_height());
    }
    if tabbar.show_auxiliary_add {
        let top = group_top
            + sidebar_item_height()
            + items_height
            + sidebar_item_gap()
            + unpinned_count.saturating_sub(group_index) as i32 * stride;
        bottom = bottom.max(top + sidebar_item_height());
    }
    bottom.max(rect.top + shell_top_bar_height() + pinned_height)
}

pub(in crate::shell::chrome) fn sidebar_auxiliary_hit_test(
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    point: (i32, i32),
    scroll_offset: i32,
    viewport_bottom: i32,
) -> Option<WindowsChromeHit> {
    let auxiliary = sidebar_auxiliary_rects(rect, tabbar, scroll_offset, viewport_bottom)?;
    for (index, item_rect) in &auxiliary.items {
        let item = tabbar.auxiliary_items.get(*index)?;
        if rect_contains(item_rect, point) {
            if !item.pinned && rect_contains(&sidebar_auxiliary_menu_rect(*item_rect, item), point)
            {
                return Some(WindowsChromeHit::Command(
                    WindowsChromeCommand::new(command_id::SIDEBAR_AUXILIARY_CONTEXT_MENU)
                        .with_payload(serde_json::json!({ "tab_id": item.id.clone() }))
                        .with_screen_position(),
                ));
            }
            if item.closable && rect_contains(&sidebar_auxiliary_close_rect(*item_rect), point) {
                return Some(chrome_command(
                    command_id::BROWSER_TAB_CLOSE,
                    serde_json::json!({ "tab_id": item.id.clone() }),
                ));
            }
            let payload = serde_json::json!({ "tab_id": item.id.clone() });
            return Some(chrome_command_with_context(
                command_id::BROWSER_TAB_CLICK,
                payload.clone(),
                command_id::SIDEBAR_AUXILIARY_CONTEXT_MENU,
                payload,
            ));
        }
    }
    if let Some(add_rect) = auxiliary.add
        && rect_contains(&add_rect, point)
    {
        return Some(chrome_command(
            command_id::MAIN_WORKSPACE_ADD,
            serde_json::json!({}),
        ));
    }
    None
}

pub(in crate::shell::chrome) fn sidebar_auxiliary_close_rect(item_rect: RECT) -> RECT {
    normalize_rect(RECT {
        left: item_rect.right - sidebar_browser_close_size(),
        top: item_rect.top,
        right: item_rect.right,
        bottom: item_rect.bottom,
    })
}

pub(in crate::shell::chrome) fn sidebar_auxiliary_menu_rect(
    item_rect: RECT,
    item: &WindowsShellAuxiliaryItemLayout,
) -> RECT {
    let trailing = if item.closable {
        sidebar_auxiliary_close_rect(item_rect).left
    } else {
        item_rect.right
    };
    normalize_rect(RECT {
        left: trailing - sidebar_browser_close_size(),
        top: item_rect.top,
        right: trailing,
        bottom: item_rect.bottom,
    })
}

pub(in crate::shell::chrome) fn draw_sidebar_auxiliary_section(
    hdc: HDC,
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    cursor: Option<(i32, i32)>,
    scroll_offset: i32,
    viewport_bottom: i32,
) {
    let Some(auxiliary) = sidebar_auxiliary_rects(rect, tabbar, scroll_offset, viewport_bottom)
    else {
        return;
    };

    for (index, item_rect) in &auxiliary.items {
        let Some(item) = tabbar.auxiliary_items.get(*index) else {
            continue;
        };
        let item_rect = *item_rect;
        if item.pinned {
            if item.active {
                fill_round_rect_aa(hdc, item_rect, 6, shell_palette().selection_background);
            } else {
                draw_hover_wash(hdc, item_rect, 6, cursor);
            }
            let left =
                item_rect.left + (rect_width(&item_rect) - pinned_shortcut_icon_size()).max(0) / 2;
            let top =
                item_rect.top + (rect_height(&item_rect) - pinned_shortcut_icon_size()).max(0) / 2;
            let icon_rect = normalize_rect(RECT {
                left,
                top,
                right: left + pinned_shortcut_icon_size(),
                bottom: top + pinned_shortcut_icon_size(),
            });
            // Live-tab favicon bytes first, then the bookmark's cached
            // favicon file (`icon_path`), then the generic globe.
            let drawn = item
                .icon_png
                .as_deref()
                .is_some_and(|png| draw_icon_from_png_bytes(hdc, &item.id, png, icon_rect))
                || (!item.icon_path.trim().is_empty()
                    && draw_icon_from_path(
                        hdc,
                        &item.icon_path,
                        icon_rect,
                        pinned_shortcut_icon_size() as u32,
                    ));
            if !drawn {
                if item.id.starts_with("lxapp:") || item.id.starts_with("pin:lxapp:") {
                    draw_default_app_icon(hdc, icon_rect);
                } else {
                    draw_design_icon_button(
                        hdc,
                        item_rect,
                        WindowsDesignIcon::Globe,
                        shell_palette().text_muted,
                        pinned_shortcut_icon_size(),
                    );
                }
            }
            continue;
        }
        if item.active {
            fill_round_rect_aa(hdc, item_rect, 6, shell_palette().selection_background);
        } else {
            draw_hover_wash(hdc, item_rect, 6, cursor);
        }

        let close_rect = sidebar_auxiliary_close_rect(item_rect);
        let menu_rect = sidebar_auxiliary_menu_rect(item_rect, item);
        // 16px icon left of the title: the page favicon when supplied, else
        // the host app icon (internal pages like Downloads/Settings report
        // no favicon; the LingXia mark is only the last-resort fallback).
        // Top-level browser tabs share the lxapp header's outer row and icon
        // axis. Only lxapp page items are indented beneath their parent.
        let mut label_left = item_rect.left + sidebar_top_level_icon_inset();
        let icon_rect = sidebar_top_level_icon_rect(item_rect, sidebar_favicon_size());
        let icon_drawn = match item.icon_png.as_deref() {
            Some(png) => draw_icon_from_png_bytes(hdc, &item.id, png, icon_rect),
            None => draw_icon_or_default(
                hdc,
                &item.icon_path,
                icon_rect,
                sidebar_favicon_size().max(1) as u32,
            ),
        };
        if icon_drawn {
            label_left = icon_rect.right + sidebar_favicon_text_gap();
        }
        let label_rect = sidebar_auxiliary_title_rect(item_rect, item, label_left);
        let text_color = if item.active {
            shell_palette().text_primary
        } else {
            shell_palette().text_muted
        };
        draw_text(hdc, &item.title, label_rect, text_color, DT_LEFT);
        if rect_contains(&item_rect, cursor.unwrap_or((-1, -1))) {
            draw_hover_wash(hdc, menu_rect, 4, cursor);
            draw_design_icon_button(
                hdc,
                menu_rect,
                WindowsDesignIcon::PageMenu,
                shell_palette().text_muted,
                16,
            );
        }
        if item.closable && rect_contains(&item_rect, cursor.unwrap_or((-1, -1))) {
            draw_hover_wash(hdc, close_rect, 4, cursor);
            draw_text(
                hdc,
                GLYPH_TAB_CLOSE,
                close_rect,
                shell_palette().text_muted,
                DT_CENTER,
            );
        }
    }

    if let Some(add_rect) = auxiliary.add {
        // Add row: a centered "+" glyph only, no label.
        draw_hover_wash(hdc, add_rect, 8, cursor);
        draw_frame_button_glyph(hdc, GLYPH_ADD, add_rect, shell_palette().text_muted);
    }
}

pub(in crate::shell::chrome) fn sidebar_auxiliary_title_rect(
    item_rect: RECT,
    item: &WindowsShellAuxiliaryItemLayout,
    label_left: i32,
) -> RECT {
    normalize_rect(RECT {
        left: label_left,
        top: item_rect.top,
        right: if item.pinned {
            item_rect.right - px(8)
        } else {
            sidebar_auxiliary_menu_rect(item_rect, item).left - px(2)
        },
        bottom: item_rect.bottom,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabbar_with_auxiliary_item(
        item: WindowsShellAuxiliaryItemLayout,
    ) -> WindowsShellTabBarLayout {
        WindowsShellTabBarLayout {
            visible: true,
            position: WindowsShellTabBarPosition::Left,
            dimension: 184,
            app_name: "Home".to_string(),
            app_icon_path: String::new(),
            group_id: "home".to_string(),
            group_target_id: "surface:home".to_string(),
            group_active: false,
            group_closable: false,
            group_order_index: 0,
            color: 0,
            selected_color: 0,
            background_color: 0,
            paint_items_background: false,
            background_transparent: true,
            border_color: 0,
            selected_index: -1,
            overflow_start_index: -1,
            items: Vec::new(),
            collapsed: false,
            icon_rail: false,
            rail_expand_disabled: false,
            items_api_hidden: false,
            items_collapsed: false,
            footer_action_height: 0,
            main_scroll_offset: 0,
            footer_action_scroll_row: 0,
            auxiliary_items: vec![item],
            show_auxiliary_add: false,
            header_actions: Vec::new(),
        }
    }

    #[test]
    fn fixed_pin_grid_has_four_columns_and_two_bounded_rows() {
        assert_eq!(pinned_grid_height(1), 41);
        assert_eq!(pinned_grid_height(4), 41);
        assert_eq!(pinned_grid_height(8), 82);
    }

    #[test]
    fn workspace_menu_sits_before_close_without_overlapping_it() {
        let item_rect = RECT {
            left: 8,
            top: 100,
            right: 176,
            bottom: 136,
        };
        let item = WindowsShellAuxiliaryItemLayout {
            id: "surface:chat".to_string(),
            title: "Chat".to_string(),
            active: true,
            pinned: false,
            closable: true,
            icon_png: None,
            icon_path: String::new(),
            tabs: None,
        };

        let menu = sidebar_auxiliary_menu_rect(item_rect, &item);
        let close = sidebar_auxiliary_close_rect(item_rect);
        let title = sidebar_auxiliary_title_rect(item_rect, &item, 40);
        assert_eq!(menu.right, close.left);
        assert_eq!(title.right, menu.left - 2);
        assert_eq!(menu.right - menu.left, sidebar_browser_close_size());
        assert_eq!(close.right - close.left, sidebar_browser_close_size());
    }

    #[test]
    fn pinned_tile_right_click_uses_the_lxapp_context_menu() {
        let tabbar = tabbar_with_auxiliary_item(WindowsShellAuxiliaryItemLayout {
            id: "pin:lxapp:chat".to_string(),
            title: "Chat".to_string(),
            active: true,
            pinned: true,
            closable: false,
            icon_png: None,
            icon_path: String::new(),
            tabs: None,
        });
        let sidebar = RECT {
            left: 0,
            top: 0,
            right: 184,
            bottom: 500,
        };
        let rows = sidebar_auxiliary_rects(sidebar, &tabbar, 0, sidebar.bottom).unwrap();
        let pin = rows.items[0].1;
        assert_eq!(pin.left, sidebar_icon_axis() - pinned_shortcut_size() / 2);
        assert_eq!(pin.top, shell_top_bar_height());
        let point = ((pin.left + pin.right) / 2, (pin.top + pin.bottom) / 2);

        let Some(WindowsChromeHit::CommandWithContext {
            command,
            context_menu,
        }) = sidebar_auxiliary_hit_test(sidebar, &tabbar, point, 0, sidebar.bottom)
        else {
            panic!("pinned tile did not produce a click plus context menu");
        };
        assert_eq!(command.id, command_id::BROWSER_TAB_CLICK);
        assert_eq!(context_menu.id, command_id::SIDEBAR_AUXILIARY_CONTEXT_MENU);
        assert_eq!(
            context_menu.payload,
            serde_json::json!({ "tab_id": "pin:lxapp:chat" })
        );
        assert!(context_menu.include_screen_position);
    }

    #[test]
    fn workspace_ellipsis_opens_its_context_menu_on_left_click() {
        let tabbar = tabbar_with_auxiliary_item(WindowsShellAuxiliaryItemLayout {
            id: "surface:chat".to_string(),
            title: "Chat".to_string(),
            active: true,
            pinned: false,
            closable: true,
            icon_png: None,
            icon_path: String::new(),
            tabs: None,
        });
        let sidebar = RECT {
            left: 0,
            top: 0,
            right: 184,
            bottom: 500,
        };
        let rows = sidebar_auxiliary_rects(sidebar, &tabbar, 0, sidebar.bottom).unwrap();
        let row = rows.items[0].1;
        let menu = sidebar_auxiliary_menu_rect(row, &tabbar.auxiliary_items[0]);
        let point = ((menu.left + menu.right) / 2, (menu.top + menu.bottom) / 2);

        let Some(WindowsChromeHit::Command(command)) =
            sidebar_auxiliary_hit_test(sidebar, &tabbar, point, 0, sidebar.bottom)
        else {
            panic!("workspace ellipsis did not produce a direct menu command");
        };
        assert_eq!(command.id, command_id::SIDEBAR_AUXILIARY_CONTEXT_MENU);
        assert_eq!(
            command.payload,
            serde_json::json!({ "tab_id": "surface:chat" })
        );
        assert!(command.include_screen_position);
    }

    #[test]
    fn sidebar_add_uses_the_active_main_workspace_command() {
        let mut tabbar = tabbar_with_auxiliary_item(WindowsShellAuxiliaryItemLayout {
            id: "surface:chat".to_string(),
            title: "Chat".to_string(),
            active: true,
            pinned: false,
            closable: true,
            icon_png: None,
            icon_path: String::new(),
            tabs: None,
        });
        tabbar.show_auxiliary_add = true;
        let sidebar = RECT {
            left: 0,
            top: 0,
            right: 184,
            bottom: 500,
        };
        let rows = sidebar_auxiliary_rects(sidebar, &tabbar, 0, sidebar.bottom).unwrap();
        let add = rows.add.expect("workspace add row should be visible");
        let point = ((add.left + add.right) / 2, (add.top + add.bottom) / 2);

        let Some(WindowsChromeHit::Command(command)) =
            sidebar_auxiliary_hit_test(sidebar, &tabbar, point, 0, sidebar.bottom)
        else {
            panic!("workspace add row did not produce a command");
        };
        assert_eq!(command.id, command_id::MAIN_WORKSPACE_ADD);
    }
}
