//! Phone-width browser chrome: the bottom browser bar and the tab-switcher
//! sheet, active when a device-framed screen presents a browser tab at a
//! compact width.

use super::*;

/// Compact (phone-width) breakpoint, matching the surface arbiter's Compact
/// size class.
const PHONE_BROWSER_MAX_WIDTH: i32 = 600;
/// Bottom browser-bar metrics: an edge-to-edge bottom sheet (rounded top
/// corners only, flush with the screen sides and bottom, like the iOS
/// browser bar) with an address-pill row (self tabs only) above the action
/// row.
const PHONE_BAR_HEIGHT_SELF: i32 = 96;
const PHONE_BAR_HEIGHT_ASIDE: i32 = 56;
const PHONE_BAR_MARGIN: i32 = 0;
const PHONE_BAR_BOTTOM_GAP: i32 = 0;
const PHONE_BAR_RADIUS: i32 = 16;
const PHONE_BAR_BUTTON: i32 = 38;
const PHONE_BAR_BUTTON_GAP: i32 = 4;
const PHONE_BAR_EDGE: i32 = 6;
const PHONE_ADDRESS_HEIGHT: i32 = 34;

/// Whether the presented browser tab renders the phone bottom bar: a
/// device-framed screen at a compact width.
pub(super) fn phone_browser_bar_active(client: RECT, layout: &WindowsShellWindowLayout) -> bool {
    layout.suppress_window_controls
        && address_bar_visible(layout)
        && rect_width(&client) < PHONE_BROWSER_MAX_WIDTH
}

pub(super) fn phone_bar_is_aside(layout: &WindowsShellWindowLayout) -> bool {
    layout
        .address_bar
        .as_ref()
        .is_some_and(|address_bar| address_bar.aside)
}

/// Interactive geometry of the phone browser bar. Self tabs get the address
/// pill row and a new-tab button; API-driven asides drop both and put the
/// reload into the action row.
pub(super) struct PhoneBarRects {
    pub(super) bar: RECT,
    pub(super) address: Option<RECT>,
    pub(super) address_reload: Option<RECT>,
    pub(super) back: RECT,
    pub(super) forward: RECT,
    pub(super) row_reload: Option<RECT>,
    pub(super) new_tab: Option<RECT>,
    pub(super) tabs: RECT,
    pub(super) close: RECT,
}

pub(super) fn phone_browser_bar_rects(client: RECT, aside: bool) -> PhoneBarRects {
    let height = if aside {
        PHONE_BAR_HEIGHT_ASIDE
    } else {
        PHONE_BAR_HEIGHT_SELF
    };
    let bar = normalize_rect(RECT {
        left: client.left + PHONE_BAR_MARGIN,
        top: (client.bottom - PHONE_BAR_BOTTOM_GAP - height).max(client.top),
        right: client.right - PHONE_BAR_MARGIN,
        bottom: client.bottom - PHONE_BAR_BOTTOM_GAP,
    });

    let (address, address_reload, action_top) = if aside {
        (
            None,
            None,
            bar.top + (rect_height(&bar) - PHONE_BAR_BUTTON).max(0) / 2,
        )
    } else {
        let pill = normalize_rect(RECT {
            left: bar.left + 6,
            top: bar.top + 8,
            right: bar.right - 6,
            bottom: bar.top + 8 + PHONE_ADDRESS_HEIGHT,
        });
        let reload = normalize_rect(RECT {
            left: pill.right - PHONE_ADDRESS_HEIGHT,
            top: pill.top + 2,
            right: pill.right - 2,
            bottom: pill.bottom - 2,
        });
        (Some(pill), Some(reload), pill.bottom + 4)
    };

    let button = |left: i32| {
        normalize_rect(RECT {
            left,
            top: action_top,
            right: left + PHONE_BAR_BUTTON,
            bottom: action_top + PHONE_BAR_BUTTON,
        })
    };
    let back = button(bar.left + PHONE_BAR_EDGE);
    let forward = button(back.right + PHONE_BAR_BUTTON_GAP);
    let row_reload = aside.then(|| button(forward.right + PHONE_BAR_BUTTON_GAP));

    let close = button(bar.right - PHONE_BAR_EDGE - PHONE_BAR_BUTTON);
    let tabs = button(close.left - PHONE_BAR_BUTTON_GAP - PHONE_BAR_BUTTON);
    let new_tab = (!aside).then(|| button(tabs.left - PHONE_BAR_BUTTON_GAP - PHONE_BAR_BUTTON));

    PhoneBarRects {
        bar,
        address,
        address_reload,
        back,
        forward,
        row_reload,
        new_tab,
        tabs,
        close,
    }
}

/// Paints the phone browser bar: a floating bottom card with the address
/// pill (self tabs only), the nav cluster, and new-tab/tabs/close.
pub(super) fn draw_phone_browser_bar(
    hdc: HDC,
    state: &WindowsChromeState,
    layout: &WindowsShellWindowLayout,
) {
    let Some(address_bar) = layout.address_bar.as_ref() else {
        return;
    };
    let pal = shell_palette();
    let cursor = state.cursor;
    let rects = phone_browser_bar_rects(state.client, address_bar.aside);

    // Rounded top corners only: extend the fill below the client bottom so
    // the lower arcs are clipped away and the sheet sits flush with the
    // screen bottom.
    let sheet = RECT {
        left: rects.bar.left,
        top: rects.bar.top,
        right: rects.bar.right,
        bottom: rects.bar.bottom + PHONE_BAR_RADIUS,
    };
    fill_round_rect_aa(hdc, sheet, PHONE_BAR_RADIUS, pal.panel_background);

    if let Some(pill) = rects.address {
        fill_round_rect_aa(hdc, pill, rect_height(&pill) / 2, pal.control_surface);
        let text_rect = normalize_rect(RECT {
            left: pill.left + 12,
            top: pill.top,
            right: rects
                .address_reload
                .map(|reload| reload.left - 4)
                .unwrap_or(pill.right - 8),
            bottom: pill.bottom,
        });
        draw_text(
            hdc,
            &address_bar.url_text,
            text_rect,
            pal.text_primary,
            DT_LEFT,
        );
        // The pill is the inline URL-edit anchor, like the top-bar capsule.
        remember_address_capsule_rect(state.hwnd, Some(pill));
        if let Some(reload) = rects.address_reload {
            draw_hover_wash(hdc, reload, rect_height(&reload) / 2, cursor);
            draw_design_icon_button(
                hdc,
                reload,
                WindowsDesignIcon::BrowserRefresh,
                pal.text_muted,
                14,
            );
        }
    } else {
        remember_address_capsule_rect(state.hwnd, None);
    }

    let nav = [
        (rects.back, WindowsDesignIcon::Back, address_bar.can_go_back),
        (
            rects.forward,
            WindowsDesignIcon::Forward,
            address_bar.can_go_forward,
        ),
    ];
    for (rect, icon, enabled) in nav {
        let color = if enabled {
            draw_hover_wash(hdc, rect, 5, cursor);
            pal.frame_button_icon
        } else {
            pal.text_muted
        };
        draw_design_icon_button(hdc, rect, icon, color, 18);
    }
    if let Some(reload) = rects.row_reload {
        draw_hover_wash(hdc, reload, 5, cursor);
        draw_design_icon_button(
            hdc,
            reload,
            WindowsDesignIcon::BrowserRefresh,
            pal.frame_button_icon,
            18,
        );
    }
    if let Some(new_tab) = rects.new_tab {
        draw_hover_wash(hdc, new_tab, 5, cursor);
        draw_frame_button_glyph(hdc, GLYPH_ADD, new_tab, pal.frame_button_icon);
    }

    // Tabs button: the shared browser-tabs icon with the open-tab count
    // overlaid, matching the iOS/HarmonyOS browsers.
    draw_hover_wash(hdc, rects.tabs, 5, cursor);
    draw_design_icon_button(
        hdc,
        rects.tabs,
        WindowsDesignIcon::BrowserTabs,
        pal.frame_button_icon,
        20,
    );
    let count = address_bar.tab_count.min(99).to_string();
    let icon_box = inset_rect(
        rects.tabs,
        (rect_width(&rects.tabs) - 20) / 2,
        (rect_height(&rects.tabs) - 20) / 2,
    );
    draw_text(hdc, &count, icon_box, pal.text_primary, DT_CENTER);

    draw_hover_wash(hdc, rects.close, 5, cursor);
    draw_design_icon_button_with_fallback(
        hdc,
        rects.close,
        WindowsDesignIcon::CloseX,
        pal.frame_button_icon,
        16,
        Some(GLYPH_CLOSE),
    );
}

/// The phone tab-switcher bottom sheet (the macOS runner's in-frame sheet):
/// a dimmed backdrop with a rounded-top panel listing every open tab.
pub(crate) struct PhoneTabSwitcherLayout {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) sheet: RECT,
    pub(crate) rows: Vec<PhoneTabSwitcherRow>,
}

pub(crate) struct PhoneTabSwitcherRow {
    pub(crate) tab_id: String,
    pub(crate) title: String,
    pub(crate) active: bool,
    pub(crate) rect: RECT,
    pub(crate) close: RECT,
}

pub(crate) enum PhoneTabSwitcherHit {
    Row(String),
    Close(String),
    Sheet,
    Dismiss,
}

pub(crate) const PHONE_SWITCHER_SHEET_RADIUS: i32 = 18;
const PHONE_SWITCHER_ROW_HEIGHT: i32 = 40;
const PHONE_SWITCHER_TITLE_HEIGHT: i32 = 36;

/// Lays the sheet over a `width`x`height` phone client; `tabs` are
/// `(tab_id, title, active)` in display order.
pub(crate) fn phone_tab_switcher_layout(
    width: i32,
    height: i32,
    tabs: &[(String, String, bool)],
) -> PhoneTabSwitcherLayout {
    let content = PHONE_SWITCHER_TITLE_HEIGHT + tabs.len() as i32 * PHONE_SWITCHER_ROW_HEIGHT + 16;
    let sheet_height = content
        .min(height * 3 / 5)
        .max(PHONE_SWITCHER_TITLE_HEIGHT + 16);
    let sheet = RECT {
        left: 0,
        top: (height - sheet_height).max(0),
        right: width,
        bottom: height,
    };
    let mut rows = Vec::with_capacity(tabs.len());
    let mut top = sheet.top + PHONE_SWITCHER_TITLE_HEIGHT;
    for (tab_id, title, active) in tabs {
        if top + PHONE_SWITCHER_ROW_HEIGHT > sheet.bottom - 8 {
            break;
        }
        let rect = RECT {
            left: sheet.left + 8,
            top,
            right: sheet.right - 8,
            bottom: top + PHONE_SWITCHER_ROW_HEIGHT,
        };
        let close = RECT {
            left: rect.right - 36,
            top: rect.top + (PHONE_SWITCHER_ROW_HEIGHT - 28) / 2,
            right: rect.right - 8,
            bottom: rect.top + (PHONE_SWITCHER_ROW_HEIGHT + 28) / 2,
        };
        rows.push(PhoneTabSwitcherRow {
            tab_id: tab_id.clone(),
            title: title.clone(),
            active: *active,
            rect,
            close,
        });
        top += PHONE_SWITCHER_ROW_HEIGHT;
    }
    PhoneTabSwitcherLayout {
        width,
        height,
        sheet,
        rows,
    }
}

/// Paints the switcher into a layered-window surface: black backdrop (the
/// alpha mask turns it into the dim), a white sheet, and the tab rows.
pub(crate) fn paint_phone_tab_switcher(hdc: HDC, layout: &PhoneTabSwitcherLayout) {
    let pal = shell_palette();
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
    fill_round_rect_aa(
        hdc,
        layout.sheet,
        PHONE_SWITCHER_SHEET_RADIUS,
        pal.panel_background,
    );
    // Square the sheet's bottom corners (it sits flush with the screen edge).
    fill_rect(
        hdc,
        RECT {
            left: layout.sheet.left,
            top: (layout.sheet.bottom - PHONE_SWITCHER_SHEET_RADIUS).max(layout.sheet.top),
            right: layout.sheet.right,
            bottom: layout.sheet.bottom,
        },
        pal.panel_background,
    );
    draw_text(
        hdc,
        "Tabs",
        RECT {
            left: layout.sheet.left + 16,
            top: layout.sheet.top + 6,
            right: layout.sheet.right - 16,
            bottom: layout.sheet.top + PHONE_SWITCHER_TITLE_HEIGHT,
        },
        pal.text_primary,
        DT_LEFT,
    );
    for row in &layout.rows {
        if row.active {
            fill_round_rect_aa(hdc, row.rect, 8, pal.control_surface);
        }
        let title_rect = RECT {
            left: row.rect.left + 10,
            top: row.rect.top,
            right: row.close.left - 8,
            bottom: row.rect.bottom,
        };
        let color = if row.active {
            pal.text_primary
        } else {
            pal.text_muted
        };
        draw_text(hdc, &row.title, title_rect, color, DT_LEFT);
        draw_text(hdc, GLYPH_TAB_CLOSE, row.close, pal.text_muted, DT_CENTER);
    }
}

pub(crate) fn phone_tab_switcher_hit(
    layout: &PhoneTabSwitcherLayout,
    point: (i32, i32),
) -> PhoneTabSwitcherHit {
    for row in &layout.rows {
        if rect_contains(&row.close, point) {
            return PhoneTabSwitcherHit::Close(row.tab_id.clone());
        }
        if rect_contains(&row.rect, point) {
            return PhoneTabSwitcherHit::Row(row.tab_id.clone());
        }
    }
    if rect_contains(&layout.sheet, point) {
        PhoneTabSwitcherHit::Sheet
    } else {
        PhoneTabSwitcherHit::Dismiss
    }
}

/// Chrome commands the switcher rows dispatch back to the shell runtime
/// (the sidebar's tab click/close semantics).
pub(crate) fn phone_tab_click_command(tab_id: &str) -> WindowsChromeCommand {
    WindowsChromeCommand::new(command_id::BROWSER_TAB_CLICK)
        .with_payload(json!({ "tab_id": tab_id }))
}

pub(crate) fn phone_tab_close_command(tab_id: &str) -> WindowsChromeCommand {
    WindowsChromeCommand::new(command_id::BROWSER_TAB_CLOSE)
        .with_payload(json!({ "tab_id": tab_id }))
}
