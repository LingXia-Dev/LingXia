//! Docked aside/browser panel chrome: the in-panel toolbar (tab strip,
//! smart navigation, address capsule) and its hit-testing.

use super::*;

/// Toolbar height of a browser aside panel (matches the macOS DockedBrowser).
pub(super) const ASIDE_PANEL_TOOLBAR_HEIGHT: i32 = 38;

/// The toolbar row at a browser aside panel's top edge; the webview fills
/// the panel below it.
pub(super) fn aside_panel_toolbar_rect(panel_rect: RECT) -> RECT {
    normalize_rect(RECT {
        left: panel_rect.left,
        top: panel_rect.top,
        right: panel_rect.right,
        bottom: (panel_rect.top + ASIDE_PANEL_TOOLBAR_HEIGHT).min(panel_rect.bottom),
    })
}

pub(super) fn browser_panel_hit_test(
    state: &WindowsChromeState,
    point: (i32, i32),
) -> Option<WindowsChromeHit> {
    let attached = state.attached.as_ref()?;
    for panel in &attached.panels {
        if !browser_panel_header_visible(panel)
            || !rect_contains(&browser_panel_header_rect(panel), point)
        {
            continue;
        }
        let tabs = panel_aside_tabs(panel);
        if !tabs.is_empty() {
            return Some(aside_panel_header_hit(panel, &tabs, point));
        }
        for (command, rect) in browser_panel_nav_button_rects(panel) {
            if rect_contains(&rect, point) {
                return Some(chrome_command(
                    command,
                    json!({ "webtag_key": panel.webtag_key.clone() }),
                ));
            }
        }
        if rect_contains(&browser_panel_close_rect(panel), point) {
            return Some(chrome_command(
                command_id::BROWSER_PANEL_CLOSE,
                json!({ "panel_id": panel.panel_id.clone() }),
            ));
        }
        if rect_contains(&browser_panel_address_rect(panel), point) {
            return Some(chrome_command(
                command_id::BROWSER_PANEL_ADDRESS_BAR,
                json!({ "webtag_key": panel.webtag_key.clone() }),
            ));
        }
        return Some(WindowsChromeHit::Chrome);
    }
    None
}

/// Aside-slot chrome hit testing. Browser slots add navigation; every slot
/// shares the tab strip and close-all affordance, with no address or new-tab
/// controls in the strip itself.
pub(super) fn aside_panel_header_hit(
    panel: &WindowsChromePanel,
    tabs: &[WindowsAsidePanelTab],
    point: (i32, i32),
) -> WindowsChromeHit {
    if panel.panel_id == lingxia_windows_contract::ASIDE_BROWSER_PANEL_ID {
        for (command, rect) in aside_panel_nav_button_rects(panel) {
            if rect_contains(&rect, point) {
                return chrome_command(command, json!({ "panel_id": panel.panel_id.clone() }));
            }
        }
    }
    if rect_contains(&browser_panel_close_rect(panel), point) {
        return chrome_command(
            command_id::ASIDE_PANEL_CLOSE_ALL,
            json!({ "panel_id": panel.panel_id.clone() }),
        );
    }
    for (tab, rect) in tabs.iter().zip(aside_panel_tab_rects(panel, tabs.len())) {
        if let Some(close) = aside_panel_tab_close_rect(rect)
            && rect_contains(&close, point)
        {
            return chrome_command(
                command_id::ASIDE_PANEL_TAB_CLOSE,
                json!({
                    "panel_id": panel.panel_id.clone(),
                    "surface_id": tab.surface_id.clone()
                }),
            );
        }
        if rect_contains(&rect, point) {
            return chrome_command(
                command_id::ASIDE_PANEL_TAB_CLICK,
                json!({
                    "panel_id": panel.panel_id.clone(),
                    "surface_id": tab.surface_id.clone()
                }),
            );
        }
    }
    WindowsChromeHit::Chrome
}

pub(super) fn panel_aside_tabs(panel: &WindowsChromePanel) -> Vec<WindowsAsidePanelTab> {
    if panel.host_content.is_some() {
        return Vec::new();
    }
    aside_panel_tabs(&panel.panel_id)
}

pub(super) fn browser_panel_header_visible(panel: &WindowsChromePanel) -> bool {
    panel.host_content.is_none()
        && (panel.webtag_key.starts_with("app.lingxia.browser:")
            || !aside_panel_tabs(&panel.panel_id).is_empty())
}

pub(super) fn browser_panel_header_rect(panel: &WindowsChromePanel) -> RECT {
    // The toolbar row at the panel's top (computed at layout time); an empty
    // rect makes the draw/hit-test paths no-op for panels without one.
    panel.header_rect.map(normalize_rect).unwrap_or_default()
}

pub(super) fn browser_panel_close_rect(panel: &WindowsChromePanel) -> RECT {
    let header = browser_panel_header_rect(panel);
    normalize_rect(RECT {
        left: (header.right - BROWSER_PANEL_BUTTON_SIZE - BROWSER_PANEL_HEADER_PADDING)
            .max(header.left),
        top: header.top + (rect_height(&header) - BROWSER_PANEL_BUTTON_SIZE) / 2,
        right: header.right - BROWSER_PANEL_HEADER_PADDING,
        bottom: header.top + (rect_height(&header) + BROWSER_PANEL_BUTTON_SIZE) / 2,
    })
}

pub(super) fn browser_panel_nav_button_rects(
    panel: &WindowsChromePanel,
) -> [(&'static str, RECT); 3] {
    let header = browser_panel_header_rect(panel);
    let top = header.top + (rect_height(&header) - BROWSER_PANEL_BUTTON_SIZE) / 2;
    let mut left = header.left + BROWSER_PANEL_HEADER_PADDING;
    let mut next = || {
        let rect = normalize_rect(RECT {
            left,
            top,
            right: left + BROWSER_PANEL_BUTTON_SIZE,
            bottom: top + BROWSER_PANEL_BUTTON_SIZE,
        });
        left += BROWSER_PANEL_BUTTON_SIZE + BROWSER_PANEL_BUTTON_GAP;
        rect
    };
    [
        (command_id::BROWSER_PANEL_NAV_BACK, next()),
        (command_id::BROWSER_PANEL_NAV_FORWARD, next()),
        (command_id::BROWSER_PANEL_NAV_RELOAD, next()),
    ]
}

/// Same nav-cluster geometry, aside-panel command ids (routed to the surface
/// layer rather than the in-app browser).
pub(super) fn aside_panel_nav_button_rects(
    panel: &WindowsChromePanel,
) -> [(&'static str, RECT); 3] {
    let [(_, back), (_, forward), (_, reload)] = browser_panel_nav_button_rects(panel);
    [
        (command_id::ASIDE_PANEL_NAV_BACK, back),
        (command_id::ASIDE_PANEL_NAV_FORWARD, forward),
        (command_id::ASIDE_PANEL_NAV_RELOAD, reload),
    ]
}

const ASIDE_PANEL_TAB_MAX_WIDTH: i32 = 190;
const ASIDE_PANEL_TAB_GAP: i32 = 4;
const ASIDE_PANEL_TAB_CLOSE_WIDTH: i32 = 20;
/// Air above the tabs; they run flush to the toolbar's bottom edge so the
/// active tab merges into the content below (Chrome style).
const ASIDE_PANEL_TAB_TOP_INSET: i32 = 6;
/// Upper-corner radius of the active tab shape.
const ASIDE_PANEL_TAB_RADIUS: i32 = 8;

/// Tab rects of the aside panel's strip, index-aligned with the registered
/// tabs: equal widths (capped) between the nav cluster and close-all.
pub(super) fn aside_panel_tab_rects(panel: &WindowsChromePanel, count: usize) -> Vec<RECT> {
    if count == 0 {
        return Vec::new();
    }
    let header = browser_panel_header_rect(panel);
    let left_edge = if panel.panel_id == lingxia_windows_contract::ASIDE_BROWSER_PANEL_ID {
        aside_panel_nav_button_rects(panel)[2].1.right + BROWSER_PANEL_HEADER_PADDING
    } else {
        header.left + BROWSER_PANEL_HEADER_PADDING
    };
    let right_edge = browser_panel_close_rect(panel).left - BROWSER_PANEL_HEADER_PADDING;
    let count_i32 = count as i32;
    let avail = (right_edge - left_edge - (count_i32 - 1) * ASIDE_PANEL_TAB_GAP).max(0);
    let width = (avail / count_i32).clamp(24, ASIDE_PANEL_TAB_MAX_WIDTH);
    let mut out = Vec::with_capacity(count);
    let mut left = left_edge;
    for _ in 0..count {
        out.push(normalize_rect(RECT {
            left,
            top: header.top + ASIDE_PANEL_TAB_TOP_INSET,
            right: (left + width).min(right_edge),
            bottom: header.bottom,
        }));
        left += width + ASIDE_PANEL_TAB_GAP;
    }
    out
}

/// Close-glyph rect at a tab's trailing edge; dropped on tabs too narrow to
/// keep a readable title next to it.
pub(super) fn aside_panel_tab_close_rect(tab: RECT) -> Option<RECT> {
    (rect_width(&tab) >= 3 * ASIDE_PANEL_TAB_CLOSE_WIDTH).then(|| {
        normalize_rect(RECT {
            left: tab.right - ASIDE_PANEL_TAB_CLOSE_WIDTH,
            top: tab.top,
            right: tab.right,
            bottom: tab.bottom,
        })
    })
}

/// The URL capsule rect inside a browser aside's header (between the nav
/// cluster and the close button). Shared by the painter and hit-test so the
/// inline editor lands exactly on the painted pill.
pub(super) fn browser_panel_address_rect(panel: &WindowsChromePanel) -> RECT {
    let header = browser_panel_header_rect(panel);
    let close = browser_panel_close_rect(panel);
    let address_left = browser_panel_nav_button_rects(panel)
        .last()
        .map(|(_, rect)| rect.right + BROWSER_PANEL_HEADER_PADDING)
        .unwrap_or(header.left + BROWSER_PANEL_HEADER_PADDING);
    normalize_rect(RECT {
        left: address_left,
        top: header.top + 8,
        right: close.left - BROWSER_PANEL_HEADER_PADDING,
        bottom: header.bottom - 8,
    })
}

/// Last painted URL-capsule rect for a browser aside, keyed by host window +
/// webtag, so a click can start an inline edit over the exact pill (mirrors the
/// main top bar's `ADDRESS_CAPSULE_RECTS`).
static PANEL_ADDRESS_RECTS: OnceLock<Mutex<HashMap<(isize, String), RECT>>> = OnceLock::new();

pub(super) fn remember_panel_address_rect(hwnd: HWND, webtag_key: &str, rect: Option<RECT>) {
    let map = PANEL_ADDRESS_RECTS.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut map) = map.lock() else {
        return;
    };
    let key = (hwnd.0 as isize, webtag_key.to_string());
    match rect {
        Some(rect) => {
            map.insert(key, rect);
        }
        None => {
            map.remove(&key);
        }
    }
}

/// Starts an inline URL edit over a browser aside's address capsule, prefilled
/// with `initial_text`. Returns `false` when no capsule was painted for
/// `(window, webtag_key)`. Mirrors [`top_bar::begin_address_edit`] for the aside.
#[cfg(feature = "browser-runtime")]
pub fn begin_panel_address_edit(
    window: isize,
    webtag_key: &str,
    initial_text: &str,
    on_commit: crate::shell::text_input::InlineEditCommit,
) -> bool {
    let rect = PANEL_ADDRESS_RECTS
        .get()
        .and_then(|map| map.lock().ok())
        .and_then(|map| map.get(&(window, webtag_key.to_string())).copied());
    let Some(rect) = rect else {
        return false;
    };
    let edit_rect = inset_rect(rect, 10, 1);
    if rect_width(&edit_rect) == 0 || rect_height(&edit_rect) == 0 {
        return false;
    }
    let initial = initial_text.to_string();
    post_to_window_thread(
        window,
        Box::new(move || {
            crate::shell::text_input::begin_inline_edit(
                HWND(window as *mut core::ffi::c_void),
                edit_rect,
                &initial,
                on_commit,
            );
        }),
    )
}

pub(super) fn draw_browser_panel_header(
    hdc: HDC,
    hwnd: HWND,
    panel: &WindowsChromePanel,
    cursor: Option<(i32, i32)>,
) {
    let header = browser_panel_header_rect(panel);
    if rect_width(&header) == 0 || rect_height(&header) == 0 {
        remember_panel_address_rect(hwnd, &panel.webtag_key, None);
        return;
    }
    let pal = shell_palette();

    let tabs = panel_aside_tabs(panel);
    if !tabs.is_empty() {
        remember_panel_address_rect(hwnd, &panel.webtag_key, None);
        draw_aside_panel_header(hdc, panel, &tabs, cursor);
        return;
    }

    // The toolbar shares the panel card's fill; only the divider separates
    // it from the webview below.
    fill_rect(
        hdc,
        RECT {
            left: header.left,
            top: header.bottom - 1,
            right: header.right,
            bottom: header.bottom,
        },
        pal.divider,
    );

    let close = browser_panel_close_rect(panel);
    for (command, rect) in browser_panel_nav_button_rects(panel) {
        let icon = match command {
            command_id::BROWSER_PANEL_NAV_BACK => WindowsDesignIcon::Back,
            command_id::BROWSER_PANEL_NAV_FORWARD => WindowsDesignIcon::Forward,
            _ => WindowsDesignIcon::BrowserRefresh,
        };
        draw_hover_wash(hdc, rect, 5, cursor);
        draw_design_icon_button(hdc, rect, icon, pal.frame_button_icon, 16);
    }

    let address = browser_panel_address_rect(panel);
    let address_visible = rect_width(&address) > 0 && rect_height(&address) > 0;
    if address_visible {
        fill_round_rect_aa(hdc, address, 10, pal.control_surface);
        draw_text(
            hdc,
            browser_panel_title(panel).as_str(),
            inset_rect(address, 10, 0),
            pal.text_muted,
            DT_LEFT,
        );
    }
    // Record the painted pill so a click can open the inline editor over it.
    remember_panel_address_rect(hwnd, &panel.webtag_key, address_visible.then_some(address));

    draw_hover_wash(hdc, close, 5, cursor);
    draw_design_icon_button_with_fallback(
        hdc,
        close,
        WindowsDesignIcon::CloseX,
        pal.frame_button_icon,
        14,
        Some(GLYPH_CLOSE),
    );
}

pub(super) fn browser_panel_title(panel: &WindowsChromePanel) -> String {
    let title = panel.title.trim();
    if title.is_empty() {
        "Browser".to_string()
    } else {
        title.to_string()
    }
}

/// Shared aside-slot chrome row: title tabs and close-all. The browser slot
/// additionally gets back/forward/reload; lxapp slots start the strip at the
/// leading edge. No slot carries a "+" affordance.
pub(super) fn draw_aside_panel_header(
    hdc: HDC,
    panel: &WindowsChromePanel,
    tabs: &[WindowsAsidePanelTab],
    cursor: Option<(i32, i32)>,
) {
    let pal = shell_palette();
    let header = browser_panel_header_rect(panel);

    // Chrome-style strip: a tinted bar the active tab lifts out of as a
    // round-topped card merging into the content below. The strip keeps the
    // panel card's top rounding; its bottom corners are squared.
    fill_round_rect_aa(hdc, header, SHELL_PANEL_RADIUS, pal.window_background);
    fill_rect(
        hdc,
        RECT {
            left: header.left,
            top: (header.bottom - SHELL_PANEL_RADIUS).max(header.top),
            right: header.right,
            bottom: header.bottom,
        },
        pal.window_background,
    );

    if panel.panel_id == lingxia_windows_contract::ASIDE_BROWSER_PANEL_ID {
        let (can_back, can_forward) = crate::shell::runtime::aside_panel_nav_state();
        for (command, rect) in aside_panel_nav_button_rects(panel) {
            let (icon, enabled) = match command {
                command_id::ASIDE_PANEL_NAV_BACK => (WindowsDesignIcon::Back, can_back),
                command_id::ASIDE_PANEL_NAV_FORWARD => (WindowsDesignIcon::Forward, can_forward),
                _ => (WindowsDesignIcon::BrowserRefresh, true),
            };
            // Hover lights only actionable buttons; a disabled direction stays
            // flat and dim.
            let color = if enabled {
                draw_hover_wash(hdc, rect, 5, cursor);
                pal.frame_button_icon
            } else {
                pal.text_muted
            };
            draw_design_icon_button(hdc, rect, icon, color, 16);
        }
    }

    let rects = aside_panel_tab_rects(panel, tabs.len());
    for (index, (tab, rect)) in tabs.iter().zip(rects.iter().copied()).enumerate() {
        if tab.active {
            // Rounded top, flush bottom: the tab joins the web content.
            fill_round_rect_aa(hdc, rect, ASIDE_PANEL_TAB_RADIUS, pal.panel_background);
            fill_rect(
                hdc,
                RECT {
                    left: rect.left,
                    top: (rect.bottom - ASIDE_PANEL_TAB_RADIUS).max(rect.top),
                    right: rect.right,
                    bottom: rect.bottom,
                },
                pal.panel_background,
            );
        } else if index > 0 && !tabs[index - 1].active {
            // Chrome hides the separator next to the active tab.
            let x = rect.left - (ASIDE_PANEL_TAB_GAP + 1) / 2;
            fill_rect(
                hdc,
                RECT {
                    left: x,
                    top: rect.top + 8,
                    right: x + 1,
                    bottom: rect.bottom - 8,
                },
                pal.divider,
            );
        }
        if !tab.active {
            draw_hover_wash(hdc, rect, ASIDE_PANEL_TAB_RADIUS, cursor);
        }
        let close = aside_panel_tab_close_rect(rect);
        let title_rect = normalize_rect(RECT {
            left: rect.left + 10,
            top: rect.top,
            right: close.map(|close| close.left).unwrap_or(rect.right - 6),
            bottom: rect.bottom,
        });
        let text_color = if tab.active {
            pal.text_primary
        } else {
            pal.text_muted
        };
        draw_text(hdc, &tab.title, title_rect, text_color, DT_LEFT);
        if let Some(close) = close {
            draw_hover_wash(hdc, close, 5, cursor);
            draw_text(hdc, GLYPH_TAB_CLOSE, close, pal.text_muted, DT_CENTER);
        }
    }

    let close_all = browser_panel_close_rect(panel);
    draw_hover_wash(hdc, close_all, 5, cursor);
    draw_design_icon_button_with_fallback(
        hdc,
        close_all,
        WindowsDesignIcon::CloseX,
        pal.frame_button_icon,
        14,
        Some(GLYPH_CLOSE),
    );
}
