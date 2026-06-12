//! Sidebar browser-tab rows.

use super::*;

/// Geometry of the sidebar browser section: separator line, one row rect
/// per browser tab (rows that would collide with the footer are dropped),
/// and the "New Tab" row.
pub(in crate::windows::chrome) struct SidebarBrowserRects {
    pub(super) separator: RECT,
    /// Row rects aligned index-for-index with `tabbar.browser_tabs`
    /// (possibly truncated when rows run out of vertical space).
    pub(super) tabs: Vec<RECT>,
    pub(super) new_tab: Option<RECT>,
}

pub(in crate::windows::chrome) fn sidebar_browser_rects(
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

pub(in crate::windows::chrome) fn sidebar_browser_hit_test(
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

pub(in crate::windows::chrome) fn sidebar_browser_close_rect(item_rect: RECT) -> RECT {
    normalize_rect(RECT {
        left: item_rect.right - SIDEBAR_BROWSER_CLOSE_SIZE,
        top: item_rect.top,
        right: item_rect.right,
        bottom: item_rect.bottom,
    })
}

pub(in crate::windows::chrome) fn draw_sidebar_browser_section(
    hdc: HDC,
    rect: RECT,
    tabbar: &WindowsTabBarLayout,
) {
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
