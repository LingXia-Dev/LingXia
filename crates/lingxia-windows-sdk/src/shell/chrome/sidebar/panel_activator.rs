//! Activator flow geometry and painting.

use std::ops::Range;

use super::*;

const ACTIVATOR_CELL_MIN_WIDTH: i32 = 72;
const ACTIVATOR_CELL_PADDING: i32 = 8;
const ACTIVATOR_ICON_TEXT_GAP: i32 = 8;
const ACTIVATOR_ASCII_ADVANCE: i32 = 7;
const ACTIVATOR_WIDE_ADVANCE: i32 = 13;
const ACTIVATOR_SEPARATOR_HEIGHT: i32 = 1;

fn preferred_cell_width(label: &str, available: i32) -> i32 {
    let text = label.chars().fold(0, |width, ch| {
        width
            + if ch.is_ascii() {
                ACTIVATOR_ASCII_ADVANCE
            } else {
                ACTIVATOR_WIDE_ADVANCE
            }
    });
    (2 * ACTIVATOR_CELL_PADDING + PANEL_ACTIVATOR_ICON_SIZE + ACTIVATOR_ICON_TEXT_GAP + text)
        .clamp(ACTIVATOR_CELL_MIN_WIDTH.min(available), available.max(1))
}

fn activator_rows(
    width: i32,
    activators: &[WindowsShellPanelActivatorLayout],
) -> Vec<Range<usize>> {
    let available = (width - 2 * PANEL_ACTIVATOR_MARGIN).max(1);
    let mut rows = Vec::new();
    let mut row_start = 0;
    let mut used = 0;
    for (index, activator) in activators.iter().enumerate() {
        let preferred = preferred_cell_width(&activator.label, available);
        let next = if index == row_start {
            preferred
        } else {
            used + PANEL_ACTIVATOR_GAP + preferred
        };
        if index > row_start && next > available {
            rows.push(row_start..index);
            row_start = index;
            used = preferred;
        } else {
            used = next;
        }
    }
    if row_start < activators.len() {
        rows.push(row_start..activators.len());
    }
    rows
}

pub(in crate::shell::chrome) fn panel_activator_footer_height_for_width(
    width: i32,
    activators: &[WindowsShellPanelActivatorLayout],
) -> i32 {
    let rows = activator_rows(width, activators)
        .len()
        .min(PANEL_ACTIVATOR_MAX_ROWS) as i32;
    if rows == 0 {
        0
    } else {
        ACTIVATOR_SEPARATOR_HEIGHT
            + 2 * PANEL_ACTIVATOR_MARGIN
            + rows * PANEL_ACTIVATOR_SIZE
            + (rows - 1) * PANEL_ACTIVATOR_GAP
    }
}

fn expanded_activator_rects(
    tabbar_rect: RECT,
    tabbar: &WindowsShellTabBarLayout,
    activators: &[WindowsShellPanelActivatorLayout],
) -> Vec<(String, RECT)> {
    let rows = activator_rows(rect_width(&tabbar_rect), activators);
    let footer_top = tabbar_rect.bottom - tabbar.activator_footer_height;
    let available = (rect_width(&tabbar_rect) - 2 * PANEL_ACTIVATOR_MARGIN).max(1);
    let mut top = footer_top + ACTIVATOR_SEPARATOR_HEIGHT + PANEL_ACTIVATOR_MARGIN;
    let mut out = Vec::with_capacity(activators.len());

    for row in rows.into_iter().take(PANEL_ACTIVATOR_MAX_ROWS) {
        let items = &activators[row.clone()];
        let preferred = items
            .iter()
            .map(|item| preferred_cell_width(&item.label, available))
            .collect::<Vec<_>>();
        let gaps = (items.len().saturating_sub(1) as i32) * PANEL_ACTIVATOR_GAP;
        let preferred_total = preferred.iter().sum::<i32>() + gaps;
        let extra = (available - preferred_total).max(0);
        let total_weight = items
            .iter()
            .map(|item| item.weight.max(1) as u64)
            .sum::<u64>()
            .max(1);
        let mut left = tabbar_rect.left + PANEL_ACTIVATOR_MARGIN;
        let row_right = tabbar_rect.right - PANEL_ACTIVATOR_MARGIN;
        let mut allocated_extra = 0;
        for (offset, item) in items.iter().enumerate() {
            let is_last = offset + 1 == items.len();
            let item_extra = if is_last {
                extra - allocated_extra
            } else {
                let share = ((extra as u64 * item.weight.max(1) as u64) / total_weight) as i32;
                allocated_extra += share;
                share
            };
            let right = if is_last {
                row_right
            } else {
                (left + preferred[offset] + item_extra).min(row_right)
            };
            out.push((
                item.id.clone(),
                normalize_rect(RECT {
                    left,
                    top,
                    right,
                    bottom: top + PANEL_ACTIVATOR_SIZE,
                }),
            ));
            left = right + PANEL_ACTIVATOR_GAP;
        }
        top += PANEL_ACTIVATOR_SIZE + PANEL_ACTIVATOR_GAP;
    }
    out
}

fn rail_activator_rects(
    tabbar_rect: RECT,
    activators: &[WindowsShellPanelActivatorLayout],
) -> Vec<(String, RECT)> {
    let count = activators.len().min(PANEL_ACTIVATOR_MAX_ROWS);
    if count == 0 {
        return Vec::new();
    }
    let expand = sidebar_rail_expand_rect(tabbar_rect);
    let total =
        count as i32 * PANEL_ACTIVATOR_SIZE + count.saturating_sub(1) as i32 * PANEL_ACTIVATOR_GAP;
    let mut top =
        (expand.top - PANEL_ACTIVATOR_MARGIN - total).max(tabbar_rect.top + SHELL_TOP_BAR_HEIGHT);
    let left = tabbar_rect.left + (rect_width(&tabbar_rect) - PANEL_ACTIVATOR_SIZE) / 2;
    activators
        .iter()
        .take(count)
        .map(|activator| {
            let rect = normalize_rect(RECT {
                left,
                top,
                right: left + PANEL_ACTIVATOR_SIZE,
                bottom: top + PANEL_ACTIVATOR_SIZE,
            });
            top = rect.bottom + PANEL_ACTIVATOR_GAP;
            (activator.id.clone(), rect)
        })
        .collect()
}

pub(in crate::shell::chrome) fn panel_activator_rects(
    client: RECT,
    rects: &ChromeRects,
    layout: &WindowsShellWindowLayout,
) -> Vec<(String, RECT)> {
    if layout.panel_activators.is_empty() {
        return Vec::new();
    }

    if let (Some(tabbar), Some(tabbar_rect)) = (&layout.tab_bar, rects.tab_bar)
        && matches!(
            tabbar.position,
            WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
        )
    {
        if tabbar.collapsed || tabbar.icon_rail {
            return rail_activator_rects(tabbar_rect, &layout.panel_activators);
        }
        return expanded_activator_rects(tabbar_rect, tabbar, &layout.panel_activators);
    }

    let bottom_limit = rects
        .tab_bar
        .map(|tabbar| tabbar.top)
        .unwrap_or(client.bottom);
    let left = rects.panel.left + PANEL_ACTIVATOR_MARGIN;
    let mut bottom = bottom_limit - PANEL_ACTIVATOR_MARGIN;
    let mut out = Vec::new();
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

pub(in crate::shell::chrome) fn draw_panel_activators(
    hdc: HDC,
    client: RECT,
    rects: &ChromeRects,
    layout: &WindowsShellWindowLayout,
    cursor: Option<(i32, i32)>,
) {
    let icon_only = layout.tab_bar.as_ref().is_some_and(|tabbar| {
        matches!(
            tabbar.position,
            WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
        ) && (tabbar.collapsed || tabbar.icon_rail)
    });
    for (panel_id, rect) in panel_activator_rects(client, rects, layout) {
        let activator = layout
            .panel_activators
            .iter()
            .find(|item| item.id == panel_id);
        let active = activator.is_some_and(|item| item.active);
        let label = activator
            .map(|item| item.label.as_str())
            .unwrap_or(panel_id.as_str());
        let text_color = activator
            .and_then(|item| item.label_color)
            .unwrap_or_else(|| {
                if active {
                    shell_palette().accent
                } else {
                    shell_palette().text_muted
                }
            });

        if !active {
            draw_hover_wash(hdc, rect, 6, cursor);
        } else {
            fill_round_rect_aa(hdc, rect, 6, shell_palette().panel_background);
            let accent = if icon_only {
                RECT {
                    left: rect.left + 4,
                    top: rect.bottom - 4,
                    right: rect.right - 4,
                    bottom: rect.bottom - 2,
                }
            } else {
                RECT {
                    left: rect.left + 2,
                    top: rect.top + 6,
                    right: rect.left + 4,
                    bottom: rect.bottom - 6,
                }
            };
            fill_round_rect_aa(hdc, accent, 2, shell_palette().accent);
        }

        let icon_rect = if icon_only {
            centered_icon_rect(rect, PANEL_ACTIVATOR_ICON_SIZE)
        } else {
            let top = rect.top + (rect_height(&rect) - PANEL_ACTIVATOR_ICON_SIZE) / 2;
            RECT {
                left: rect.left + ACTIVATOR_CELL_PADDING,
                top,
                right: rect.left + ACTIVATOR_CELL_PADDING + PANEL_ACTIVATOR_ICON_SIZE,
                bottom: top + PANEL_ACTIVATOR_ICON_SIZE,
            }
        };
        let icon_path = activator
            .map(|item| item.icon_path.as_str())
            .unwrap_or_default();
        if !draw_icon_or_default(hdc, icon_path, icon_rect, PANEL_ACTIVATOR_ICON_SIZE as u32) {
            draw_text(
                hdc,
                &panel_activator_label(label),
                icon_rect,
                text_color,
                DT_CENTER,
            );
        }
        if !icon_only {
            draw_text(
                hdc,
                label,
                RECT {
                    left: icon_rect.right + ACTIVATOR_ICON_TEXT_GAP,
                    top: rect.top,
                    right: rect.right - ACTIVATOR_CELL_PADDING,
                    bottom: rect.bottom,
                },
                text_color,
                DT_LEFT,
            );
        }
    }
}

pub(in crate::shell::chrome) fn panel_activator_label(label: &str) -> String {
    let mut out = String::new();
    for ch in label.chars().filter(|ch| ch.is_ascii_alphanumeric()) {
        out.push(ch.to_ascii_uppercase());
        if out.len() == 2 {
            break;
        }
    }
    if out.is_empty() { "?".to_string() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, label: &str) -> WindowsShellPanelActivatorLayout {
        WindowsShellPanelActivatorLayout {
            id: id.to_string(),
            label: label.to_string(),
            label_color: None,
            icon_path: String::new(),
            weight: 1_000,
            position: WindowsPanelPosition::Right,
            active: false,
        }
    }

    #[test]
    fn short_activators_share_a_standard_sidebar_row() {
        let items = vec![item("api", "API"), item("nav", "Navigation")];
        assert_eq!(activator_rows(220, &items), vec![0..2]);
    }

    #[test]
    fn long_activators_wrap_as_whole_cells() {
        let items = vec![
            item("first", "A deliberately long activator"),
            item("second", "Another deliberately long activator"),
        ];
        assert_eq!(activator_rows(220, &items), vec![0..1, 1..2]);
    }
}
