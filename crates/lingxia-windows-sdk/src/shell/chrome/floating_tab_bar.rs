//! A touch host's tabs in Regular: a pill floating at the leading bottom corner
//! of the main region, drawn by a per-pixel-alpha overlay above the page.

use crate::dpi::px;

use super::*;

fn cell_width() -> i32 {
    px(60)
}

fn bar_height() -> i32 {
    px(60)
}

fn bar_padding() -> i32 {
    px(6)
}

fn bar_margin() -> i32 {
    px(12)
}

/// The pill's fill. The overlay makes it slightly translucent, so the page
/// shows through faintly; GDI cannot blur what lies under another window.
pub(super) fn floating_tab_bar_fill() -> u32 {
    shell_palette().panel_background
}

/// Translucency the overlay applies to the pill, 0–255. An immersive bar lets
/// more of the page through, but never so much that labels lose their plate.
const FLOATING_TAB_BAR_ALPHA: u32 = 240;
const FLOATING_TAB_BAR_IMMERSIVE_ALPHA: u32 = 217;

/// The pill inside the strip it keeps clear: at the leading edge, a margin up
/// from the bottom.
pub(super) fn floating_tab_bar_rect(strip: RECT, tabbar: &WindowsShellTabBarLayout) -> RECT {
    let width = tabbar.items.len() as i32 * cell_width() + 2 * bar_padding();
    let bottom = (strip.bottom - bar_margin()).max(strip.top);
    let left = strip.left + bar_margin();
    normalize_rect(RECT {
        left,
        top: (bottom - bar_height()).max(strip.top),
        right: (left + width).min(strip.right),
        bottom,
    })
}

pub(super) fn floating_tab_cell_rect(bar: RECT, slot: usize) -> RECT {
    let left = bar.left + bar_padding() + slot as i32 * cell_width();
    RECT {
        left,
        top: bar.top,
        right: left + cell_width(),
        bottom: bar.bottom,
    }
}

/// Declaration index of the tab under `point`, in the same space as `bar`.
pub(super) fn floating_tab_index_at(
    bar: RECT,
    tabbar: &WindowsShellTabBarLayout,
    point: (i32, i32),
) -> Option<usize> {
    (0..tabbar.items.len())
        .find(|slot| rect_contains(&floating_tab_cell_rect(bar, *slot), point))
        .and_then(|slot| tabbar.click_index_at_slot(slot))
}

/// Paints the pill, opaque, into a surface of its own size; the overlay rounds
/// and fades it afterwards.
pub(super) fn draw_floating_tab_bar(hdc: HDC, bar: RECT, tabbar: &WindowsShellTabBarLayout) {
    fill_rect(hdc, bar, floating_tab_bar_fill());
    stroke_round_rect_aa(hdc, bar, rect_height(&bar) / 2, shell_palette().divider);
    // Labels draw straight onto the opaque fill, whatever the page's bar style.
    let painted = WindowsShellTabBarLayout {
        background_transparent: false,
        ..tabbar.clone()
    };
    let item_top = bar.top + (rect_height(&bar) - px(49)) / 2;
    for (slot, item) in tabbar.items.iter().enumerate() {
        let cell = floating_tab_cell_rect(bar, slot);
        if cell.right > bar.right {
            break;
        }
        draw_tab_cell(
            hdc,
            cell,
            item_top,
            &painted,
            BottomSlot::Tab(slot),
            tabbar.selected_index == item.index as i32,
            None,
        );
    }
}

/// The floating bar's pill in client coordinates, while one shows.
pub(crate) fn floating_tabbar_overlay_rect(
    client: RECT,
    layout: &WindowsWindowLayout,
) -> Option<RECT> {
    let layout = shell_layout(layout)?;
    layout
        .touch_tab_bar
        .as_ref()
        .filter(|tabbar| tabbar.floating)?;
    compute_chrome_rects(client, layout).touch_tab_bar
}

/// Paints the floating bar into a `width × height` overlay surface and returns
/// the alpha to apply to it; `None` when the layout has none.
pub(crate) fn paint_floating_tabbar_overlay(
    hdc: HDC,
    layout: &WindowsWindowLayout,
    width: i32,
    height: i32,
) -> Option<u32> {
    let tabbar = shell_layout(layout)
        .and_then(|layout| layout.touch_tab_bar.as_ref())
        .filter(|tabbar| tabbar.visible && tabbar.floating)?;
    draw_floating_tab_bar(
        hdc,
        RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        },
        tabbar,
    );
    Some(if tabbar.background_transparent {
        FLOATING_TAB_BAR_IMMERSIVE_ALPHA
    } else {
        FLOATING_TAB_BAR_ALPHA
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_tabs_beside_an_app_rail_stay_left_of_a_fold_crease() {
        // Mate X5 open: crease at 392 dp; the app rail takes 56 dp.
        let mut tabbar = super::super::tabbar_overflow::tests::tabbar(5, 0);
        tabbar.floating = true;
        let strip = RECT {
            left: 56,
            top: 784,
            right: 784,
            bottom: 868,
        };
        let bar = floating_tab_bar_rect(strip, &tabbar);
        assert!(bar.right < 392, "{bar:?}");
        assert_eq!(
            floating_tab_index_at(bar, &tabbar, (bar.right - 10, bar.top + 20)),
            Some(4)
        );
    }
}
