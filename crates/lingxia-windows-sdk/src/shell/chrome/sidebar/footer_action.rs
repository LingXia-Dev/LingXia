//! Footer sidebar action flow geometry and painting.

use std::ops::Range;

use super::*;

fn footer_action_cell_min_width() -> i32 {
    crate::dpi::px(72)
}
fn footer_action_cell_padding() -> i32 {
    crate::dpi::px(8)
}
fn footer_action_icon_text_gap() -> i32 {
    crate::dpi::px(8)
}
fn footer_action_separator_height() -> i32 {
    crate::dpi::px(1)
}

fn preferred_cell_width(label: &str, available: i32) -> i32 {
    let text = measure_chrome_text_width(label);
    (2 * footer_action_cell_padding()
        + footer_action_icon_size()
        + footer_action_icon_text_gap()
        + text)
        .clamp(
            footer_action_cell_min_width().min(available),
            available.max(1),
        )
}

fn footer_action_rows(
    width: i32,
    footer_actions: &[WindowsShellFooterActionLayout],
) -> Vec<Range<usize>> {
    let available = (width - 2 * footer_action_margin()).max(1);
    let minimum = footer_action_cell_min_width().min(available);
    let mut rows = Vec::new();
    let mut row_start = 0;
    let mut used = 0;
    for index in 0..footer_actions.len() {
        let next = if index == row_start {
            minimum
        } else {
            used + footer_action_gap() + minimum
        };
        if index > row_start && next > available {
            rows.push(row_start..index);
            row_start = index;
            used = minimum;
        } else {
            used = next;
        }
    }
    if row_start < footer_actions.len() {
        rows.push(row_start..footer_actions.len());
    }
    rows
}

fn fitted_cell_widths(preferred: &[i32], available: i32) -> Vec<i32> {
    if preferred.is_empty() {
        return Vec::new();
    }

    let count = preferred.len() as i32;
    let gaps = (count - 1) * footer_action_gap();
    let target = (available - gaps).max(count);
    let minimum = footer_action_cell_min_width().min(target / count).max(1);
    let mut widths = preferred
        .iter()
        .map(|width| (*width).clamp(minimum, target))
        .collect::<Vec<_>>();
    let mut delta = target - widths.iter().sum::<i32>();

    while delta < 0 {
        let shrinkable = widths.iter().filter(|width| **width > minimum).count() as i32;
        debug_assert!(shrinkable > 0, "row minimums must fit the available width");
        if shrinkable == 0 {
            break;
        }
        let share = ((-delta) + shrinkable - 1) / shrinkable;
        for width in &mut widths {
            let shrink = (*width - minimum).max(0).min(share).min(-delta);
            *width -= shrink;
            delta += shrink;
            if delta == 0 {
                break;
            }
        }
    }

    if delta > 0 {
        let share = delta / count;
        let remainder = delta % count;
        for (index, width) in widths.iter_mut().enumerate() {
            *width += share + i32::from((index as i32) < remainder);
        }
    }

    debug_assert_eq!(widths.iter().sum::<i32>(), target);
    widths
}

fn capped_row_window(total: usize, requested_start: usize) -> Range<usize> {
    let max_start = total.saturating_sub(FOOTER_ACTION_MAX_ROWS);
    let start = requested_start.min(max_start);
    start..(start + FOOTER_ACTION_MAX_ROWS).min(total)
}

pub(in crate::shell::chrome) fn panel_footer_action_height_for_width(
    width: i32,
    footer_actions: &[WindowsShellFooterActionLayout],
) -> i32 {
    let rows = footer_action_rows(width, footer_actions)
        .len()
        .min(FOOTER_ACTION_MAX_ROWS) as i32;
    if rows == 0 {
        0
    } else {
        footer_action_separator_height()
            + 2 * footer_action_margin()
            + rows * footer_action_size()
            + (rows - 1) * footer_action_gap()
    }
}

fn expanded_footer_action_rects(
    tabbar_rect: RECT,
    footer_action_height: i32,
    footer_action_scroll_row: usize,
    footer_actions: &[WindowsShellFooterActionLayout],
) -> Vec<(String, RECT)> {
    let rows = footer_action_rows(rect_width(&tabbar_rect), footer_actions);
    let footer_top = tabbar_rect.bottom - footer_action_height;
    let available = (rect_width(&tabbar_rect) - 2 * footer_action_margin()).max(1);
    let mut top = footer_top + footer_action_separator_height() + footer_action_margin();
    let mut out = Vec::with_capacity(footer_actions.len());

    let visible_rows = capped_row_window(rows.len(), footer_action_scroll_row);
    for row in &rows[visible_rows] {
        let items = &footer_actions[row.clone()];
        let preferred = items
            .iter()
            .map(|item| preferred_cell_width(&item.label, available))
            .collect::<Vec<_>>();
        let widths = fitted_cell_widths(&preferred, available);
        let mut left = tabbar_rect.left + footer_action_margin();
        let row_right = tabbar_rect.right - footer_action_margin();
        for (offset, item) in items.iter().enumerate() {
            let is_last = offset + 1 == items.len();
            let right = if is_last {
                row_right
            } else {
                (left + widths[offset]).min(row_right)
            };
            out.push((
                item.id.clone(),
                normalize_rect(RECT {
                    left,
                    top,
                    right,
                    bottom: top + footer_action_size(),
                }),
            ));
            left = right + footer_action_gap();
        }
        top += footer_action_size() + footer_action_gap();
    }
    out
}

fn rail_footer_action_rects(
    tabbar_rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    footer_actions: &[WindowsShellFooterActionLayout],
) -> Vec<(String, RECT)> {
    let visible = capped_row_window(footer_actions.len(), tabbar.footer_action_scroll_row);
    let count = visible.len();
    if count == 0 {
        return Vec::new();
    }
    let expand = sidebar_rail_expand_rect(tabbar_rect);
    let total =
        count as i32 * footer_action_size() + count.saturating_sub(1) as i32 * footer_action_gap();
    let mut top =
        (expand.top - footer_action_margin() - total).max(tabbar_rect.top + shell_top_bar_height());
    let left = tabbar_rect.left + (rect_width(&tabbar_rect) - footer_action_size()) / 2;
    footer_actions
        .iter()
        .skip(visible.start)
        .take(count)
        .map(|footer_action| {
            let rect = normalize_rect(RECT {
                left,
                top,
                right: left + footer_action_size(),
                bottom: top + footer_action_size(),
            });
            top = rect.bottom + footer_action_gap();
            (footer_action.id.clone(), rect)
        })
        .collect()
}

pub(in crate::shell::chrome) fn footer_action_rects(
    client: RECT,
    rects: &ChromeRects,
    layout: &WindowsShellWindowLayout,
) -> Vec<(String, RECT)> {
    if layout.footer_actions.is_empty() {
        return Vec::new();
    }

    if let (Some(tabbar), Some(tabbar_rect)) = (&layout.tab_bar, rects.tab_bar)
        && matches!(
            tabbar.position,
            WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
        )
    {
        if tabbar.collapsed || tabbar.icon_rail {
            return rail_footer_action_rects(tabbar_rect, tabbar, &layout.footer_actions);
        }
        return expanded_footer_action_rects(
            tabbar_rect,
            tabbar.footer_action_height,
            tabbar.footer_action_scroll_row,
            &layout.footer_actions,
        );
    }

    let bottom_limit = rects
        .tab_bar
        .map(|tabbar| tabbar.top)
        .unwrap_or(client.bottom);
    let left = rects.panel.left + footer_action_margin();
    let mut bottom = bottom_limit - footer_action_margin();
    let mut out = Vec::new();
    for footer_action in &layout.footer_actions {
        let top = bottom - footer_action_size();
        if top < client.top + footer_action_margin() {
            break;
        }
        out.push((
            footer_action.id.clone(),
            normalize_rect(RECT {
                left,
                top,
                right: left + footer_action_size(),
                bottom,
            }),
        ));
        bottom = top - footer_action_gap();
    }
    out
}

pub(in crate::shell::chrome) fn footer_action_max_scroll_row(
    tabbar_rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    footer_actions: &[WindowsShellFooterActionLayout],
) -> usize {
    let rows = if tabbar.collapsed || tabbar.icon_rail {
        footer_actions.len()
    } else {
        footer_action_rows(rect_width(&tabbar_rect), footer_actions).len()
    };
    rows.saturating_sub(FOOTER_ACTION_MAX_ROWS)
}

pub(in crate::shell::chrome) fn sidebar_navigation_viewport_bottom(
    tabbar_rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    footer_actions: &[WindowsShellFooterActionLayout],
) -> i32 {
    if tabbar.collapsed || tabbar.icon_rail {
        return rail_footer_action_rects(tabbar_rect, tabbar, footer_actions)
            .first()
            .map(|(_, rect)| rect.top - footer_action_margin())
            .unwrap_or_else(|| sidebar_rail_expand_rect(tabbar_rect).top - footer_action_margin());
    }
    tabbar_rect.bottom - tabbar.footer_action_height
}

pub(in crate::shell::chrome) fn draw_footer_actions(
    hdc: HDC,
    client: RECT,
    rects: &ChromeRects,
    layout: &WindowsShellWindowLayout,
    cursor: Option<(i32, i32)>,
) {
    let palette = shell_palette();
    let icon_only = layout.tab_bar.as_ref().is_some_and(|tabbar| {
        matches!(
            tabbar.position,
            WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
        ) && (tabbar.collapsed || tabbar.icon_rail)
    });
    for (panel_id, rect) in footer_action_rects(client, rects, layout) {
        let footer_action = layout
            .footer_actions
            .iter()
            .find(|item| item.id == panel_id);
        let disabled = footer_action.is_some_and(|item| item.disabled);
        let label = footer_action
            .map(|item| item.label.as_str())
            .unwrap_or(panel_id.as_str());
        let text_color = palette.text_muted;

        if !disabled {
            draw_hover_wash(hdc, rect, 6, cursor);
        }

        let icon_rect = if icon_only {
            centered_icon_rect(rect, footer_action_icon_size())
        } else {
            let top = rect.top + (rect_height(&rect) - footer_action_icon_size()) / 2;
            RECT {
                left: rect.left + footer_action_cell_padding(),
                top,
                right: rect.left + footer_action_cell_padding() + footer_action_icon_size(),
                bottom: top + footer_action_icon_size(),
            }
        };
        let icon_path = footer_action
            .map(|item| item.icon_path.as_str())
            .unwrap_or_default();
        let _ = draw_icon_from_path(hdc, icon_path, icon_rect, footer_action_icon_size() as u32);
        if !icon_only {
            draw_text(
                hdc,
                label,
                RECT {
                    left: icon_rect.right + footer_action_icon_text_gap(),
                    top: rect.top,
                    right: rect.right - footer_action_cell_padding(),
                    bottom: rect.bottom,
                },
                text_color,
                DT_LEFT,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, label: &str) -> WindowsShellFooterActionLayout {
        WindowsShellFooterActionLayout {
            generation: 1,
            id: id.to_string(),
            label: label.to_string(),
            icon_path: String::new(),
            disabled: false,
            source: WindowsShellSidebarActionSource::Runtime,
        }
    }

    #[test]
    fn short_footer_actions_share_a_standard_sidebar_row() {
        let items = vec![item("api", "API"), item("chat", "Chat")];
        assert_eq!(footer_action_rows(184, &items), vec![0..2]);
    }

    #[test]
    fn label_metrics_do_not_change_row_topology() {
        let items = vec![
            item("first", "A deliberately long footer_action"),
            item("second", "这是一个会被截断的很长标签"),
        ];
        assert_eq!(footer_action_rows(184, &items), vec![0..2]);
    }

    #[test]
    fn fitted_widths_preserve_minimums_across_font_metric_ranges() {
        let available = 184 - 2 * footer_action_margin();
        for preferred in [vec![72, 86], vec![92, 126], vec![available, available]] {
            let widths = fitted_cell_widths(&preferred, available);
            assert_eq!(widths.len(), 2);
            assert!(widths.iter().all(|width| *width >= 72));
            assert_eq!(widths.iter().sum::<i32>() + footer_action_gap(), available);
        }
    }

    #[test]
    fn showcase_footer_fixture_gates_render_and_hit_test_rects() {
        let items = vec![
            item("chat", "chat"),
            item("terminal", "Terminal"),
            item("ping", "Ping"),
        ];
        let tabbar_rect = RECT {
            left: 0,
            top: 0,
            right: 184,
            bottom: 600,
        };
        let height = panel_footer_action_height_for_width(184, &items);
        let rects = expanded_footer_action_rects(tabbar_rect, height, 0, &items);

        assert_eq!(footer_action_rows(184, &items), vec![0..2, 2..3]);
        assert_eq!(
            height,
            footer_action_separator_height()
                + 2 * footer_action_margin()
                + 2 * footer_action_size()
                + footer_action_gap()
        );
        assert_eq!(
            rects.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
            vec!["chat", "terminal", "ping"]
        );

        let chat = rects[0].1;
        let terminal = rects[1].1;
        let ping = rects[2].1;
        assert_eq!(chat.top, terminal.top);
        assert_eq!(chat.right + footer_action_gap(), terminal.left);
        assert!(rect_width(&chat) >= footer_action_cell_min_width());
        assert!(rect_width(&terminal) >= footer_action_cell_min_width());
        assert_eq!(chat.left, tabbar_rect.left + footer_action_margin());
        assert_eq!(terminal.right, tabbar_rect.right - footer_action_margin());
        assert_eq!(ping.top, chat.bottom + footer_action_gap());
        assert_eq!(ping.left, tabbar_rect.left + footer_action_margin());
        assert_eq!(ping.right, tabbar_rect.right - footer_action_margin());
        assert_eq!(ping.bottom, tabbar_rect.bottom - footer_action_margin());
    }

    #[test]
    fn capped_footer_window_keeps_overflow_rows_reachable() {
        assert_eq!(capped_row_window(8, 0), 0..5);
        assert_eq!(capped_row_window(8, 2), 2..7);
        assert_eq!(capped_row_window(8, usize::MAX), 3..8);
        assert_eq!(capped_row_window(3, 1), 0..3);
    }
}
