//! Compact TabBar overflow geometry and painting.

use super::*;

const OVERFLOW_COLUMNS: usize = 5;
const OVERFLOW_PANEL_RADIUS: i32 = 16;
const OVERFLOW_PANEL_PADDING: i32 = 8;
const OVERFLOW_HORIZONTAL_INSET: i32 = 12;
const OVERFLOW_BOTTOM_GAP: i32 = 8;
const OVERFLOW_CELL_HEIGHT: i32 = 64;
const OVERFLOW_ICON_SIZE: i32 = 24;
const OVERFLOW_ICON_CONTAINER: i32 = 32;
const OVERFLOW_ICON_TEXT_GAP: i32 = 4;
const OVERFLOW_INDICATOR_SIZE: i32 = 32;
const OVERFLOW_INDICATOR_MIX_PERCENT: u32 = 20;
/// Matches iOS/macOS overflow labels (`UIFont.systemFont(ofSize: 10, weight: .medium)`).
const OVERFLOW_LABEL_POINT_SIZE: i32 = 10;
const OVERFLOW_LABEL_WEIGHT: i32 = 500;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TabbarOverflowCell {
    pub(crate) index: usize,
    pub(crate) rect: RECT,
    pub(crate) icon: RECT,
    pub(crate) label: RECT,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TabbarOverflowLayout {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) sheet: RECT,
    /// Top of the compact strip. The overlay is transparent here so the tab
    /// bar stays visible and clickable, matching iOS/macOS.
    pub(crate) strip_top: i32,
    pub(crate) cells: Vec<TabbarOverflowCell>,
    pub(crate) tabbar: WindowsShellTabBarLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TabbarOverflowHit {
    Item(usize),
    Sheet,
    Dismiss,
}

pub(crate) const TABBAR_OVERFLOW_PANEL_RADIUS: i32 = OVERFLOW_PANEL_RADIUS;

/// Places the folded items in a five-column grid immediately above the compact
/// strip. The full layout keeps the overlay inside the simulated screen; the
/// window host supplies the screen-sized layered surface and rounded clip.
pub(crate) fn tabbar_overflow_layout(
    width: i32,
    height: i32,
    strip_top: i32,
    tabbar: WindowsShellTabBarLayout,
) -> Option<TabbarOverflowLayout> {
    let start = tabbar.bottom_overflow_start()?;
    let folded = tabbar.items.len().saturating_sub(start);
    if folded == 0 || width <= 0 || height <= 0 {
        return None;
    }

    let rows = folded.div_ceil(OVERFLOW_COLUMNS) as i32;
    let panel_height = OVERFLOW_PANEL_PADDING * 2 + rows * OVERFLOW_CELL_HEIGHT;
    let strip_top = strip_top.clamp(0, height);
    let bottom = (strip_top - OVERFLOW_BOTTOM_GAP).clamp(0, height);
    let inset = OVERFLOW_HORIZONTAL_INSET.min(width / 2);
    let sheet = normalize_rect(RECT {
        left: inset,
        top: (bottom - panel_height).max(0),
        right: width - inset,
        bottom,
    });
    let inner_left = sheet.left + OVERFLOW_PANEL_PADDING;
    let inner_right = sheet.right - OVERFLOW_PANEL_PADDING;
    let inner_width = (inner_right - inner_left).max(0);
    let column_width = inner_width / OVERFLOW_COLUMNS as i32;
    let mut cells = Vec::with_capacity(folded);
    for (offset, index) in (start..tabbar.items.len()).enumerate() {
        let row = offset / OVERFLOW_COLUMNS;
        let column = offset % OVERFLOW_COLUMNS;
        let left = inner_left + column as i32 * column_width;
        let right = if column + 1 == OVERFLOW_COLUMNS {
            inner_right
        } else {
            inner_left + (column as i32 + 1) * column_width
        };
        let rect = normalize_rect(RECT {
            left,
            top: sheet.top + OVERFLOW_PANEL_PADDING + row as i32 * OVERFLOW_CELL_HEIGHT,
            right,
            bottom: sheet.top + OVERFLOW_PANEL_PADDING + (row as i32 + 1) * OVERFLOW_CELL_HEIGHT,
        });
        let center_x = (rect.left + rect.right) / 2;
        // iOS: 32pt well + 24pt icon, so the 32pt selection plate sits fully
        // inside the cell instead of being clipped at the card's top edge.
        let icon_box_top = rect.top + OVERFLOW_PANEL_PADDING;
        let icon_inset = (OVERFLOW_ICON_CONTAINER - OVERFLOW_ICON_SIZE) / 2;
        let icon = normalize_rect(RECT {
            left: center_x - OVERFLOW_ICON_SIZE / 2,
            top: icon_box_top + icon_inset,
            right: center_x + OVERFLOW_ICON_SIZE / 2,
            bottom: icon_box_top + icon_inset + OVERFLOW_ICON_SIZE,
        });
        let label = normalize_rect(RECT {
            left: rect.left + 4,
            top: icon.bottom + OVERFLOW_ICON_TEXT_GAP,
            right: rect.right - 4,
            bottom: rect.bottom - 2,
        });
        cells.push(TabbarOverflowCell {
            index,
            rect,
            icon,
            label,
        });
    }

    Some(TabbarOverflowLayout {
        width,
        height,
        sheet,
        strip_top,
        cells,
        tabbar,
    })
}

pub(crate) fn tabbar_overflow_hit(
    layout: &TabbarOverflowLayout,
    point: (i32, i32),
) -> TabbarOverflowHit {
    if let Some(cell) = layout
        .cells
        .iter()
        .find(|cell| rect_contains(&cell.rect, point))
    {
        return TabbarOverflowHit::Item(cell.index);
    }
    if rect_contains(&layout.sheet, point) {
        TabbarOverflowHit::Sheet
    } else {
        TabbarOverflowHit::Dismiss
    }
}

/// The page floor the host declared, as the chrome's `0xRRGGBB`.
///
/// `None` when the host declares nothing, leaving the caller on its own system
/// colour rather than guessing a page colour on the app's behalf.
fn declared_page_background() -> Option<u32> {
    let dark = crate::shell::theme::chrome_is_dark();
    let declared = lingxia_app_context::page_background_color(dark)?;
    let hex = declared.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

/// Paints the sliding panel and icons. Labels are returned as runs so the host
/// can composite them after geometric alpha — GDI text on this layered DIB
/// leaves gray fringes because the alpha pass cannot recover coverage from RGB.
pub(crate) fn paint_tabbar_overflow(
    hdc: HDC,
    layout: &TabbarOverflowLayout,
    panel_offset: i32,
) -> Vec<LayeredTextRun> {
    fill_rect(
        hdc,
        RECT {
            left: 0,
            top: 0,
            right: layout.width,
            bottom: layout.height,
        },
        0x000000,
    );

    let translate = |rect: RECT| RECT {
        left: rect.left,
        top: rect.top + panel_offset,
        right: rect.right,
        bottom: rect.bottom + panel_offset,
    };
    let sheet = translate(layout.sheet);
    let palette = shell_palette();
    // A transparent bar lets the page through, so the panel's plate has to be
    // the page's own colour. `panel_background` is shell chrome and follows the
    // system, so a light page under a dark system produced a dark panel sitting
    // over a light strip.
    let surface = if layout.tabbar.background_transparent || layout.tabbar.background_color == 0 {
        declared_page_background().unwrap_or(palette.panel_background)
    } else {
        layout.tabbar.background_color
    };
    fill_round_rect_aa_corners(hdc, sheet, [OVERFLOW_PANEL_RADIUS; 4], surface);

    let font_height = logical_font_height(hdc, OVERFLOW_LABEL_POINT_SIZE);
    let mut text_runs = Vec::with_capacity(layout.cells.len());
    for cell in &layout.cells {
        let Some(item) = layout.tabbar.items.get(cell.index) else {
            continue;
        };
        let icon = translate(cell.icon);
        let selected = layout.tabbar.selected_index == item.index as i32;
        let color = if selected {
            layout.tabbar.selected_color
        } else {
            layout.tabbar.color
        };
        if selected {
            let center_x = (icon.left + icon.right) / 2;
            let center_y = (icon.top + icon.bottom) / 2;
            fill_round_rect_aa(
                hdc,
                RECT {
                    left: center_x - OVERFLOW_INDICATOR_SIZE / 2,
                    top: center_y - OVERFLOW_INDICATOR_SIZE / 2,
                    right: center_x + OVERFLOW_INDICATOR_SIZE / 2,
                    bottom: center_y + OVERFLOW_INDICATOR_SIZE / 2,
                },
                OVERFLOW_INDICATOR_SIZE / 2,
                blend_rgb(
                    layout.tabbar.selected_color,
                    surface,
                    OVERFLOW_INDICATOR_MIX_PERCENT,
                ),
            );
        }
        let drew_icon = !item.icon_path.trim().is_empty()
            && draw_icon_from_path(hdc, &item.icon_path, icon, OVERFLOW_ICON_SIZE as u32);
        if !drew_icon {
            fill_round_rect_aa(hdc, icon, OVERFLOW_ICON_SIZE / 2, color);
        }
        if let Some(badge) = item.badge.as_deref().filter(|badge| !badge.is_empty()) {
            draw_badge(hdc, icon, badge);
        } else if item.has_red_dot {
            draw_red_dot(hdc, icon);
        }

        let label = if item.text.trim().is_empty() {
            item.page_path.as_str()
        } else {
            item.text.as_str()
        };
        if !label.is_empty() {
            text_runs.push(LayeredTextRun {
                rect: translate(cell.label),
                color,
                text: label.to_string(),
                font_height,
                font_weight: OVERFLOW_LABEL_WEIGHT,
            });
        }
    }
    text_runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabbar(count: usize, selected_index: i32) -> WindowsShellTabBarLayout {
        WindowsShellTabBarLayout {
            visible: true,
            position: WindowsShellTabBarPosition::Bottom,
            dimension: 49,
            app_name: "App".to_string(),
            app_icon_path: String::new(),
            group_id: "app".to_string(),
            group_target_id: "lxapp:app".to_string(),
            group_active: true,
            group_closable: false,
            group_order_index: 0,
            color: 0x666666,
            selected_color: 0x1677ff,
            background_color: 0,
            background_transparent: true,
            border_color: 0,
            selected_index,
            items: (0..count)
                .map(|index| WindowsShellTabBarItemLayout {
                    index,
                    page_path: format!("pages/p{index}/index"),
                    text: format!("P{index}"),
                    icon_path: String::new(),
                    badge: None,
                    has_red_dot: false,
                })
                .collect(),
            overflow_start_index: if count > 5 { 4 } else { -1 },
            collapsed: false,
            icon_rail: false,
            items_api_hidden: false,
            items_collapsed: false,
            footer_action_height: 0,
            main_scroll_offset: 0,
            footer_action_scroll_row: 0,
            auxiliary_items: Vec::new(),
            show_auxiliary_add: false,
            header_actions: Vec::new(),
        }
    }

    #[test]
    fn six_items_create_one_fixed_column_row_above_the_strip() {
        let layout = tabbar_overflow_layout(393, 852, 803, tabbar(6, 0)).unwrap();
        assert_eq!(
            layout.sheet,
            RECT {
                left: 12,
                top: 715,
                right: 381,
                bottom: 795
            }
        );
        assert_eq!(layout.strip_top, 803);
        assert_eq!(layout.cells.len(), 2);
        assert_eq!(layout.cells[0].index, 4);
        assert_eq!(layout.cells[1].index, 5);
        // Five equal columns fill the inset plate; a short row stays left-aligned.
        assert_eq!(layout.cells[0].rect.left, 20);
        assert_eq!(layout.cells[1].rect.left, 90);
        assert_eq!(
            layout.cells[0].rect.right - layout.cells[0].rect.left,
            layout.cells[1].rect.right - layout.cells[1].rect.left
        );
        assert!(layout.cells[0].rect.right - layout.cells[0].rect.left >= 70);
    }

    #[test]
    fn five_character_labels_keep_a_wide_enough_cell() {
        let layout = tabbar_overflow_layout(393, 852, 803, tabbar(6, 0)).unwrap();
        let width = layout.cells[0].label.right - layout.cells[0].label.left;
        // 10pt CJK is ~10–12px wide; five glyphs need ~60px plus padding.
        assert!(
            width >= 62,
            "label slot {width}px is too narrow for five glyphs"
        );
    }

    #[test]
    fn ten_items_wrap_the_last_folded_item_to_a_second_row() {
        let layout = tabbar_overflow_layout(400, 800, 751, tabbar(10, 9)).unwrap();
        assert_eq!(layout.cells.len(), 6);
        assert_eq!(layout.cells[5].index, 9);
        assert!(layout.cells[5].rect.top > layout.cells[0].rect.top);
        assert_eq!(layout.sheet.bottom, 743);
        assert_eq!(layout.strip_top, 751);
    }

    #[test]
    fn hit_testing_distinguishes_items_sheet_padding_and_scrim() {
        let layout = tabbar_overflow_layout(400, 800, 751, tabbar(6, 0)).unwrap();
        let first = &layout.cells[0];
        assert_eq!(
            tabbar_overflow_hit(
                &layout,
                (
                    (first.rect.left + first.rect.right) / 2,
                    (first.rect.top + first.rect.bottom) / 2
                )
            ),
            TabbarOverflowHit::Item(4)
        );
        assert_eq!(
            tabbar_overflow_hit(&layout, (layout.sheet.right - 2, layout.sheet.top + 2)),
            TabbarOverflowHit::Sheet
        );
        assert_eq!(
            tabbar_overflow_hit(&layout, (20, 20)),
            TabbarOverflowHit::Dismiss
        );
    }

    #[test]
    fn folded_labels_sit_under_the_icon_inside_the_cell() {
        let layout = tabbar_overflow_layout(393, 852, 803, tabbar(6, 0)).unwrap();
        for cell in &layout.cells {
            assert!(cell.label.top >= cell.icon.bottom);
            assert!(cell.label.bottom <= cell.rect.bottom);
            assert!(cell.label.right > cell.label.left);
            assert!(cell.label.bottom > cell.label.top);
        }
    }

    #[test]
    fn selected_indicator_stays_inside_the_overflow_card() {
        // iOS: 32pt well + 24pt icon, plate centred on the well. A 36pt plate
        // at icon_top+4 used to clip the card's top edge.
        let layout = tabbar_overflow_layout(393, 852, 803, tabbar(6, 4)).unwrap();
        let cell = &layout.cells[0];
        let center_y = (cell.icon.top + cell.icon.bottom) / 2;
        let plate_top = center_y - OVERFLOW_INDICATOR_SIZE / 2;
        let plate_bottom = center_y + OVERFLOW_INDICATOR_SIZE / 2;
        assert!(
            plate_top >= layout.sheet.top,
            "plate top {plate_top} clips sheet top {}",
            layout.sheet.top
        );
        assert!(plate_top >= cell.rect.top);
        assert!(plate_bottom <= cell.rect.bottom);
        assert!(plate_bottom <= layout.sheet.bottom);
    }
}
