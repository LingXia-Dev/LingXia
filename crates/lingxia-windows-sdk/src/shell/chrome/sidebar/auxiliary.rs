//! Sidebar auxiliary rows.

use super::*;

/// Geometry of the sidebar auxiliary section: separator line, one row rect
/// per auxiliary item (rows that would collide with the footer are dropped),
/// and the add row.
pub(in crate::shell::chrome) struct SidebarAuxiliaryRects {
    pub(super) separator: RECT,
    /// Row rects aligned index-for-index with `tabbar.auxiliary_items`
    /// (possibly truncated when rows run out of vertical space).
    pub(in crate::shell::chrome) items: Vec<RECT>,
    pub(super) add: Option<RECT>,
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

    let mut items = Vec::with_capacity(tabbar.auxiliary_items.len());
    for _ in &tabbar.auxiliary_items {
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
            if rect_contains(&sidebar_auxiliary_close_rect(*item_rect), point) {
                return Some(chrome_command(
                    command_id::BROWSER_TAB_CLOSE,
                    serde_json::json!({ "tab_id": item.id.clone() }),
                ));
            }
            return Some(chrome_command(
                command_id::BROWSER_TAB_CLICK,
                serde_json::json!({ "tab_id": item.id.clone() }),
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
) {
    let Some(auxiliary) = sidebar_auxiliary_rects(rect, tabbar) else {
        return;
    };

    fill_rect(hdc, auxiliary.separator, SHELL_DIVIDER);

    for (item, item_rect) in tabbar.auxiliary_items.iter().zip(&auxiliary.items) {
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
            None => draw_default_app_icon(hdc, icon_rect),
        };
        if icon_drawn {
            label_left = icon_rect.right + SIDEBAR_FAVICON_TEXT_GAP;
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

    if let Some(add_rect) = auxiliary.add {
        // Add row: a centered "+" glyph only, no label.
        draw_frame_button_glyph(hdc, GLYPH_ADD, add_rect, SHELL_TEXT_MUTED);
    }
}
