//! Sidebar auxiliary rows.

use super::*;

const PINNED_SHORTCUT_SIZE: i32 = 34;
const PINNED_SHORTCUT_ICON_SIZE: i32 = 20;

fn pinned_count(tabbar: &WindowsShellTabBarLayout) -> usize {
    tabbar
        .auxiliary_items
        .iter()
        .take_while(|item| item.pinned)
        .count()
}

pub(in crate::shell::chrome) fn sidebar_pinned_grid_height(
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
) -> i32 {
    let count = pinned_count(tabbar);
    if count == 0 {
        return 0;
    }
    let grid_width = (rect_width(&rect) - 2 * SIDEBAR_ITEM_INSET).max(PINNED_SHORTCUT_SIZE);
    let columns = ((grid_width + SIDEBAR_ITEM_GAP) / (PINNED_SHORTCUT_SIZE + SIDEBAR_ITEM_GAP))
        .max(1) as usize;
    let stride = PINNED_SHORTCUT_SIZE + SIDEBAR_ITEM_GAP;
    // `sidebar_auxiliary_rects` drops rows that would cross the footer;
    // reserve height only for rows that actually render, or the sections
    // below would shift as if the dropped rows existed.
    let available = (rect.bottom - SIDEBAR_FOOTER_HEIGHT) - (rect.top + SHELL_TOP_BAR_HEIGHT);
    let fitting_rows = if available < PINNED_SHORTCUT_SIZE {
        0
    } else {
        (available - PINNED_SHORTCUT_SIZE) / stride + 1
    };
    // The grid's row stride already leaves `SIDEBAR_ITEM_GAP` after its last
    // row. Do not add browser-section padding here: pins and lxapp groups are
    // adjacent primary navigation sections, not separator-delimited groups.
    (count.div_ceil(columns) as i32).min(fitting_rows) * stride
}

/// Geometry of the sidebar auxiliary section: separator line, one row rect
/// per auxiliary item (rows that would collide with the footer are dropped),
/// and the add row.
pub(in crate::shell::chrome) struct SidebarAuxiliaryRects {
    pub(super) separator: RECT,
    /// Row rects aligned index-for-index with `tabbar.auxiliary_items`
    /// (possibly truncated when rows run out of vertical space).
    pub(in crate::shell::chrome) items: Vec<RECT>,
    pub(in crate::shell::chrome) add: Option<RECT>,
}

pub(in crate::shell::chrome) fn sidebar_auxiliary_rects(
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
) -> Option<SidebarAuxiliaryRects> {
    if tabbar.auxiliary_items.is_empty() && !tabbar.show_auxiliary_add {
        return None;
    }
    let footer_top = rect.bottom - SIDEBAR_FOOTER_HEIGHT;
    // A collapsed items group hides its rows; the auxiliary section moves up
    // directly under the group header.
    let pinned_height = sidebar_pinned_grid_height(rect, tabbar);
    let items_height = if tabbar.items_collapsed {
        0
    } else {
        tabbar.items.len() as i32 * (SIDEBAR_ITEM_HEIGHT + SIDEBAR_ITEM_GAP)
    };
    let items_bottom = rect.top + SIDEBAR_HEADER_HEIGHT + pinned_height + items_height;
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

    let mut items = Vec::with_capacity(tabbar.auxiliary_items.len());
    let pinned_count = pinned_count(tabbar);
    if pinned_count > 0 {
        let grid_left = rect.left + SIDEBAR_ITEM_INSET;
        let grid_width = (rect.right - SIDEBAR_ITEM_INSET - grid_left).max(PINNED_SHORTCUT_SIZE);
        let columns = ((grid_width + SIDEBAR_ITEM_GAP) / (PINNED_SHORTCUT_SIZE + SIDEBAR_ITEM_GAP))
            .max(1) as usize;
        // Pinned websites are global shortcuts, not children of the current
        // lxapp group. They sit immediately below the caption controls and
        // above the lxapp header/navigation, matching macOS.
        let grid_top = rect.top + SHELL_TOP_BAR_HEIGHT;
        for index in 0..pinned_count {
            let row = index / columns;
            let column = index % columns;
            let left = grid_left + column as i32 * (PINNED_SHORTCUT_SIZE + SIDEBAR_ITEM_GAP);
            let top = grid_top + row as i32 * (PINNED_SHORTCUT_SIZE + SIDEBAR_ITEM_GAP);
            let pinned_rect = normalize_rect(RECT {
                left,
                top,
                right: left + PINNED_SHORTCUT_SIZE,
                bottom: top + PINNED_SHORTCUT_SIZE,
            });
            if pinned_rect.bottom > footer_top {
                return Some(SidebarAuxiliaryRects {
                    separator,
                    items,
                    add: None,
                });
            }
            items.push(pinned_rect);
        }
    }
    for _ in tabbar.auxiliary_items.iter().skip(pinned_count) {
        match row(&mut top) {
            Some(rect) => items.push(rect),
            None => break,
        }
    }
    let add = if tabbar.show_auxiliary_add {
        row(&mut top)
    } else {
        None
    };

    Some(SidebarAuxiliaryRects {
        separator,
        items,
        add,
    })
}

pub(in crate::shell::chrome) fn sidebar_auxiliary_hit_test(
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    point: (i32, i32),
) -> Option<WindowsChromeHit> {
    let auxiliary = sidebar_auxiliary_rects(rect, tabbar)?;
    for (item, item_rect) in tabbar.auxiliary_items.iter().zip(&auxiliary.items) {
        if rect_contains(item_rect, point) {
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
            command_id::BROWSER_NEW_TAB,
            serde_json::json!({}),
        ));
    }
    None
}

pub(in crate::shell::chrome) fn sidebar_auxiliary_close_rect(item_rect: RECT) -> RECT {
    normalize_rect(RECT {
        left: item_rect.right - SIDEBAR_BROWSER_CLOSE_SIZE,
        top: item_rect.top,
        right: item_rect.right,
        bottom: item_rect.bottom,
    })
}

pub(in crate::shell::chrome) fn draw_sidebar_auxiliary_section(
    hdc: HDC,
    rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    cursor: Option<(i32, i32)>,
) {
    let Some(auxiliary) = sidebar_auxiliary_rects(rect, tabbar) else {
        return;
    };

    let has_regular_rows =
        tabbar.auxiliary_items.iter().any(|item| !item.pinned) || tabbar.show_auxiliary_add;
    if has_regular_rows {
        fill_rect(hdc, auxiliary.separator, shell_palette().divider);
    }

    for (item, item_rect) in tabbar.auxiliary_items.iter().zip(&auxiliary.items) {
        let item_rect = *item_rect;
        if item.pinned {
            if item.active {
                fill_round_rect_aa(hdc, item_rect, 8, shell_palette().panel_background);
                fill_round_rect_aa(
                    hdc,
                    RECT {
                        left: item_rect.left + 9,
                        top: item_rect.bottom - 4,
                        right: item_rect.right - 9,
                        bottom: item_rect.bottom - 1,
                    },
                    2,
                    tabbar.selected_color,
                );
            } else {
                draw_hover_wash(hdc, item_rect, 8, cursor);
            }
            let left =
                item_rect.left + (rect_width(&item_rect) - PINNED_SHORTCUT_ICON_SIZE).max(0) / 2;
            let top =
                item_rect.top + (rect_height(&item_rect) - PINNED_SHORTCUT_ICON_SIZE).max(0) / 2;
            let icon_rect = normalize_rect(RECT {
                left,
                top,
                right: left + PINNED_SHORTCUT_ICON_SIZE,
                bottom: top + PINNED_SHORTCUT_ICON_SIZE,
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
                        PINNED_SHORTCUT_ICON_SIZE as u32,
                    ));
            if !drawn {
                draw_design_icon_button(
                    hdc,
                    item_rect,
                    WindowsDesignIcon::Globe,
                    shell_palette().text_muted,
                    PINNED_SHORTCUT_ICON_SIZE,
                );
            }
            continue;
        }
        if item.active {
            // White row card on the gray sidebar, accent bar on white.
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

        let close_rect = sidebar_auxiliary_close_rect(item_rect);
        // 16px icon left of the title: the page favicon when supplied, else
        // the default LingXia mark (internal pages like Downloads/Settings
        // report no favicon, mirroring the macOS bundled fallback).
        let mut label_left = item_rect.left + 16;
        let icon_top = item_rect.top + (rect_height(&item_rect) - SIDEBAR_FAVICON_SIZE) / 2;
        let icon_rect = normalize_rect(RECT {
            left: label_left,
            top: icon_top,
            right: label_left + SIDEBAR_FAVICON_SIZE,
            bottom: icon_top + SIDEBAR_FAVICON_SIZE,
        });
        let icon_drawn = match item.icon_png.as_deref() {
            Some(png) => draw_icon_from_png_bytes(hdc, &item.id, png, icon_rect),
            None => draw_icon_or_default(
                hdc,
                &item.icon_path,
                icon_rect,
                SIDEBAR_FAVICON_SIZE.max(1) as u32,
            ),
        };
        if icon_drawn {
            label_left = icon_rect.right + SIDEBAR_FAVICON_TEXT_GAP;
        }
        let label_rect = normalize_rect(RECT {
            left: label_left,
            top: item_rect.top,
            right: if item.closable {
                close_rect.left - 2
            } else {
                item_rect.right - 8
            },
            bottom: item_rect.bottom,
        });
        let text_color = if item.active {
            shell_palette().text_primary
        } else {
            shell_palette().text_muted
        };
        draw_text(hdc, &item.title, label_rect, text_color, DT_LEFT);
        if item.closable {
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
