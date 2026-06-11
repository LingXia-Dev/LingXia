//! Shell window chrome: chrome rect computation, all GDI drawing (top bar,
//! tab bar, sidebar, navigation bar, panels, text, colors), and hit-testing.
//!
//! Moved out of `lingxia-webview` so the webview crate stays generic; this
//! file is pure product policy registered through the
//! [`WindowsChromeRenderer`] seam.

use std::ffi::c_void;
use std::sync::{Arc, OnceLock};

use lingxia_webview::platform::windows::{
    WindowsChromeHit, WindowsChromePanel, WindowsChromeRenderer, WindowsChromeState,
    WindowsFrameButton, WindowsNativePanelContent, WindowsNativePanelKind,
    WindowsNavigationBarLayout, WindowsTabBarLayout, WindowsTabBarPosition, WindowsWindowLayout,
    cached_png_icon_handle, set_windows_chrome_renderer,
};
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateSolidBrush, DEFAULT_CHARSET,
    DEFAULT_PITCH, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_SINGLELINE, DT_VCENTER, DeleteObject,
    DrawTextW, FF_SWISS, FillRect, GetDeviceCaps, GetStockObject, GetTextFaceW, HDC, HFONT,
    HGDIOBJ, LOGPIXELSY, NULL_PEN, OUT_DEFAULT_PRECIS, RestoreDC, RoundRect, SaveDC, SelectObject,
    SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::Graphics::GdiPlus;
use windows::Win32::UI::WindowsAndMessaging::{self, HICON};
use windows::core::{PCWSTR, w};

pub(super) const SHELL_PANEL_PADDING: i32 = 6;

pub(super) const SHELL_PANEL_RADIUS: i32 = 14;

pub(super) const SHELL_WINDOW_BACKGROUND: u32 = 0xe7e8eb;

pub(super) const SHELL_PANEL_BACKGROUND: u32 = 0xffffff;

pub(super) const SHELL_SIDEBAR_BACKGROUND: u32 = 0xe7e8eb;

pub(super) const SHELL_TEXT_PRIMARY: u32 = 0x111827;

pub(super) const SHELL_TEXT_MUTED: u32 = 0x667085;

pub(super) const SHELL_ACCENT: u32 = 0x1677ff;

pub(super) const SHELL_DIVIDER: u32 = 0xd6d9de;

pub(super) const SHELL_BADGE_RED: u32 = 0xff3b30;

pub(super) const SHELL_FRAME_BUTTON_ICON: u32 = 0x1f2937;

/// System red of the Win11 close button when hovered (#C42B1C).
pub(super) const SHELL_CLOSE_HOVER: u32 = 0xc42b1c;

/// Slightly darker close-button red while pressed.
pub(super) const SHELL_CLOSE_PRESSED: u32 = 0xb22a1b;

/// Black-overlay strength (percent) for hovered minimize/maximize buttons
/// (Win11 light theme: ~6% black).
pub(super) const FRAME_BUTTON_HOVER_OVERLAY: u32 = 6;

/// Black-overlay strength (percent) for pressed minimize/maximize buttons.
pub(super) const FRAME_BUTTON_PRESSED_OVERLAY: u32 = 9;

pub(super) const SHELL_TERMINAL_TEXT: u32 = 0xe5e7eb;

// ---- Terminal dock chrome (mirrors the macOS TabRailView/WorkspaceView
// UX: header tab strip over a full-bleed terminal surface). ----

/// Height of the terminal panel header (tab strip + maximize) row.
pub(super) const TERMINAL_HEADER_HEIGHT: i32 = 34;

/// Fallback terminal surface background (#282C34, `lxTerminalBackground`)
/// used until a snapshot reports its own background color.
pub(super) const TERMINAL_SURFACE_BACKGROUND: u32 = 0x282c34;

/// Header background: slightly lighter than the terminal surface so the
/// strip reads as chrome while the active tab flows into the surface.
pub(super) const TERMINAL_HEADER_BACKGROUND: u32 = 0x343a46;

pub(super) const TERMINAL_HEADER_TEXT: u32 = 0xe8eaf0;

pub(super) const TERMINAL_HEADER_TEXT_MUTED: u32 = 0x9aa3b2;

/// Maximum width of one header tab; tabs shrink evenly below this.
pub(super) const TERMINAL_TAB_MAX_WIDTH: i32 = 190;

pub(super) const TERMINAL_TAB_GAP: i32 = 4;

/// Top inset of tabs inside the header; doubles as the draggable divider
/// thickness of a docked panel (`ATTACHED_PANEL_HANDLE_SIZE` in
/// lingxia-webview), so tab clicks never collide with resize drags.
pub(super) const TERMINAL_TAB_TOP_INSET: i32 = 5;

/// Side length of the square header buttons (new tab, maximize).
pub(super) const TERMINAL_HEADER_BUTTON_SIZE: i32 = 22;

/// Width of the close-glyph hit area inside the active tab.
pub(super) const TERMINAL_TAB_CLOSE_WIDTH: i32 = 20;

pub(super) const TERMINAL_HEADER_PADDING: i32 = 8;

/// Segoe Fluent Icons "Add" glyph for the new-tab button.
pub(super) const GLYPH_ADD: &str = "\u{e710}";

pub(super) const SHELL_SIDEBAR_HEADER_TEXT: u32 = 0x4f5661;

pub(super) const SHELL_TAB_SELECTED_BACKGROUND: u32 = 0xf3f7ff;

pub(super) const SHELL_TOP_BAR_HEIGHT: i32 = 38;

/// Win11 caption-button width (every Win11 app uses 46px-wide buttons
/// flush against the top-right window edge).
pub(super) const WINDOW_BUTTON_WIDTH: i32 = 46;

/// Caption glyph size: 10pt Segoe Fluent Icons, like the system frame.
pub(super) const WINDOW_BUTTON_GLYPH_POINT_SIZE: i32 = 10;

/// Caption glyphs (Segoe Fluent Icons / Segoe MDL2 Assets codepoints).
pub(super) const GLYPH_MINIMIZE: &str = "\u{e921}";

pub(super) const GLYPH_MAXIMIZE: &str = "\u{e922}";

pub(super) const GLYPH_RESTORE: &str = "\u{e923}";

pub(super) const GLYPH_CLOSE: &str = "\u{e8bb}";

pub(super) const SHELL_SIDEBAR_WIDTH: i32 = 180;

pub(super) const SIDEBAR_HEADER_HEIGHT: i32 = 66;

pub(super) const SIDEBAR_ITEM_HEIGHT: i32 = 34;

pub(super) const SIDEBAR_ITEM_GAP: i32 = 4;

pub(super) const SIDEBAR_ITEM_INSET: i32 = 10;

pub(super) const SIDEBAR_FOOTER_HEIGHT: i32 = 46;

/// Vertical padding above and below the browser-section separator line.
pub(super) const SIDEBAR_BROWSER_SECTION_GAP: i32 = 8;

/// Width of the close-glyph hit area at the trailing edge of a browser row.
pub(super) const SIDEBAR_BROWSER_CLOSE_SIZE: i32 = 22;

/// Close glyph for browser tab rows (multiplication X).
pub(super) const GLYPH_TAB_CLOSE: &str = "\u{2715}";

pub(super) const SIDEBAR_ICON_SIZE: i32 = 16;

pub(super) const PANEL_ACTIVATOR_SIZE: i32 = 28;

pub(super) const PANEL_ACTIVATOR_ICON_SIZE: i32 = 16;

pub(super) const PANEL_ACTIVATOR_GAP: i32 = 4;

pub(super) const PANEL_ACTIVATOR_MARGIN: i32 = 6;

pub(super) const SHELL_TEXT_POINT_SIZE: i32 = 9;

pub(super) const SHELL_TEXT_WEIGHT: i32 = 400;

/// The shell's window chrome renderer, registered into `lingxia-webview`.
struct ShellChromeRenderer;

impl WindowsChromeRenderer for ShellChromeRenderer {
    fn content_rect(&self, client: RECT, layout: &WindowsWindowLayout) -> RECT {
        compute_chrome_rects(client, layout).content
    }

    fn panel_gap(&self) -> i32 {
        SHELL_PANEL_PADDING
    }

    fn panel_corner_radius(&self) -> i32 {
        SHELL_PANEL_RADIUS
    }

    fn card_corner_color(&self) -> Option<COLORREF> {
        // Attached cards are rounded by lingxia-webview's per-pixel-alpha
        // corner-cap overlays; this is the chrome background they blend
        // the card corners into.
        Some(rgb_to_colorref(SHELL_WINDOW_BACKGROUND))
    }

    fn maximized_panel_rect(&self, client: RECT, _layout: &WindowsWindowLayout) -> RECT {
        // A maximized panel takes the whole app area below the caption
        // strip — covering the sidebar — so only the frame buttons stay
        // reachable while maximized.
        RECT {
            left: client.left,
            top: (client.top + SHELL_TOP_BAR_HEIGHT).min(client.bottom),
            right: client.right,
            bottom: client.bottom,
        }
    }

    fn paint(&self, hdc: HDC, state: &WindowsChromeState) {
        // An active inline text edit (e.g. a terminal tab rename) is a real
        // EDIT child window; the hosts do not use WS_CLIPCHILDREN, so its
        // rect is clipped out to keep chrome repaints from drawing over it.
        let saved = unsafe { SaveDC(hdc) };
        super::text_input::exclude_active_inline_edit(hdc, state.hwnd);
        draw_window_chrome(hdc, state);
        unsafe {
            let _ = RestoreDC(hdc, saved);
        }
    }

    fn hit_test(&self, state: &WindowsChromeState, point: (i32, i32)) -> Option<WindowsChromeHit> {
        chrome_hit_test(state, point)
    }

    fn frame_button_rect(
        &self,
        state: &WindowsChromeState,
        button: WindowsFrameButton,
    ) -> Option<RECT> {
        window_frame_button_rects(state.client)
            .into_iter()
            .find(|(candidate, _)| *candidate == button)
            .map(|(_, rect)| rect)
    }
}

pub(super) fn install() {
    set_windows_chrome_renderer(Arc::new(ShellChromeRenderer));
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ChromeRects {
    pub(super) content: RECT,
    pub(super) panel: RECT,
    pub(super) top_bar: RECT,
    pub(super) navigation_bar: Option<RECT>,
    pub(super) tab_bar: Option<RECT>,
}

pub(super) fn compute_chrome_rects(client: RECT, layout: &WindowsWindowLayout) -> ChromeRects {
    let mut content = client;
    let mut top_bar_left = client.left;
    let mut top_bar_right = client.right;
    let tab_bar = layout
        .tab_bar
        .as_ref()
        .filter(|tabbar| tabbar.visible && !tabbar.items.is_empty() && tabbar.dimension > 0)
        .map(|tabbar| match tabbar.position {
            WindowsTabBarPosition::Left => {
                let width = tabbar.dimension.max(SHELL_SIDEBAR_WIDTH);
                let right = (content.left + width).min(content.right);
                let rect = RECT {
                    left: content.left,
                    top: content.top,
                    right,
                    bottom: content.bottom,
                };
                content.left = right + SHELL_PANEL_PADDING;
                top_bar_left = content.left;
                top_bar_right = content.right;
                content.top = content.top + SHELL_TOP_BAR_HEIGHT + SHELL_PANEL_PADDING;
                content.right -= SHELL_PANEL_PADDING;
                content.bottom -= SHELL_PANEL_PADDING;
                rect
            }
            WindowsTabBarPosition::Right => {
                let width = tabbar.dimension.max(SHELL_SIDEBAR_WIDTH);
                let left = (content.right - width).max(content.left);
                let rect = RECT {
                    left,
                    top: content.top,
                    right: content.right,
                    bottom: content.bottom,
                };
                content.right = left - SHELL_PANEL_PADDING;
                top_bar_left = content.left + SHELL_PANEL_PADDING;
                top_bar_right = content.right;
                content.top = content.top + SHELL_TOP_BAR_HEIGHT + SHELL_PANEL_PADDING;
                content.left += SHELL_PANEL_PADDING;
                content.bottom -= SHELL_PANEL_PADDING;
                rect
            }
            WindowsTabBarPosition::Bottom => {
                let top = (content.bottom - tabbar.dimension).max(content.top);
                let rect = RECT {
                    left: content.left,
                    top,
                    right: content.right,
                    bottom: content.bottom,
                };
                content.bottom = top;
                rect
            }
        });

    if !matches!(
        layout.tab_bar.as_ref().map(|tabbar| tabbar.position),
        Some(WindowsTabBarPosition::Left | WindowsTabBarPosition::Right)
    ) {
        content.top += SHELL_TOP_BAR_HEIGHT;
        top_bar_left = content.left;
        top_bar_right = content.right;
    }

    content = normalize_rect(content);
    let panel = content;
    let top_bar = normalize_rect(RECT {
        left: top_bar_left,
        top: client.top,
        right: top_bar_right,
        bottom: (client.top + SHELL_TOP_BAR_HEIGHT).min(client.bottom),
    });

    let navigation_bar = layout
        .navigation_bar
        .as_ref()
        .filter(|navbar| navbar.visible && navbar.height > 0)
        .map(|_| top_bar);

    ChromeRects {
        content: normalize_rect(content),
        panel: normalize_rect(panel),
        top_bar,
        navigation_bar: navigation_bar.map(normalize_rect),
        tab_bar: tab_bar.map(normalize_rect),
    }
}

/// Chrome rects for a concrete window state: when the host has attached
/// panels, the navigation bar is clipped to the main content card.
pub(super) fn chrome_rects_for_state(state: &WindowsChromeState) -> ChromeRects {
    let mut rects = compute_chrome_rects(state.client, &state.layout);
    if rects.navigation_bar.is_some()
        && let Some(attached) = &state.attached
    {
        rects.navigation_bar = Some(normalize_rect(RECT {
            left: attached.main.left,
            top: rects.top_bar.top,
            right: attached.main.right,
            bottom: rects.top_bar.bottom,
        }));
    }
    rects
}

pub(super) fn draw_window_chrome(hdc: HDC, state: &WindowsChromeState) {
    let client = state.client;
    let layout = &state.layout;
    let rects = chrome_rects_for_state(state);

    fill_rect(hdc, client, SHELL_WINDOW_BACKGROUND);
    draw_shell_top_bar(hdc, &rects);
    draw_content_cards(hdc, state, &rects);

    if let (Some(navbar), Some(navbar_rect)) = (&layout.navigation_bar, rects.navigation_bar) {
        draw_navigation_bar(hdc, navbar_rect, navbar);
    }
    if let (Some(tabbar), Some(tabbar_rect)) = (&layout.tab_bar, rects.tab_bar) {
        draw_tab_bar(hdc, tabbar_rect, tabbar);
    }
    draw_panel_activators(hdc, client, &rects, layout);
    draw_maximized_native_panels(hdc, state);
    draw_window_frame_buttons(hdc, state);
}

pub(super) fn chrome_hit_test(
    state: &WindowsChromeState,
    point: (i32, i32),
) -> Option<WindowsChromeHit> {
    let client = state.client;
    let layout = &state.layout;
    let rects = chrome_rects_for_state(state);

    if let Some((button, _)) = window_frame_button_rects(client)
        .into_iter()
        .find(|(_, rect)| rect_contains(rect, point))
    {
        return Some(WindowsChromeHit::FrameButton(button));
    }

    if let Some(attached) = &state.attached {
        for panel in &attached.panels {
            if panel
                .native
                .as_ref()
                .is_some_and(|native| native.kind == WindowsNativePanelKind::Terminal)
                && rect_contains(&panel.rect, point)
            {
                // Header elements (tabs, new-tab, maximize) win over the
                // generic focus hit; the rest of the panel focuses it.
                if let Some(hit) = terminal_header_hit_test(panel, point) {
                    return Some(hit);
                }
                return Some(WindowsChromeHit::NativePanel {
                    panel_id: panel.panel_id.clone(),
                });
            }
        }
    }

    if let (Some(navbar), Some(navbar_rect)) = (&layout.navigation_bar, rects.navigation_bar)
        && rect_contains(&navbar_rect, point)
    {
        if navbar.show_back_button && rect_contains(&nav_button_rect(navbar_rect, 0), point) {
            return Some(WindowsChromeHit::NavigationBack);
        }
        let home_index = if navbar.show_back_button { 1 } else { 0 };
        if navbar.show_home_button
            && rect_contains(&nav_button_rect(navbar_rect, home_index), point)
        {
            return Some(WindowsChromeHit::NavigationHome);
        }
        return Some(WindowsChromeHit::Caption);
    }

    for (panel_id, rect) in panel_activator_rects(client, &rects, layout) {
        if rect_contains(&rect, point) {
            return Some(WindowsChromeHit::PanelActivator { panel_id });
        }
    }

    if let (Some(tabbar), Some(tabbar_rect)) = (&layout.tab_bar, rects.tab_bar)
        && rect_contains(&tabbar_rect, point)
    {
        let sidebar = matches!(
            tabbar.position,
            WindowsTabBarPosition::Left | WindowsTabBarPosition::Right
        );
        for index in 0..tabbar.items.len() {
            let item_rect = if sidebar {
                sidebar_item_rect(tabbar_rect, index)
            } else {
                tab_item_rect(tabbar_rect, tabbar.position, tabbar.items.len(), index)
            };
            if rect_contains(&item_rect, point) {
                return Some(WindowsChromeHit::TabBarItem { index });
            }
        }
        if sidebar && let Some(hit) = sidebar_browser_hit_test(tabbar_rect, tabbar, point) {
            return Some(hit);
        }
        return Some(WindowsChromeHit::Chrome);
    }

    if rect_contains(&rects.top_bar, point) {
        return Some(WindowsChromeHit::Caption);
    }

    None
}

pub(super) fn draw_content_cards(hdc: HDC, state: &WindowsChromeState, rects: &ChromeRects) {
    if let Some(attached) = &state.attached {
        draw_content_card(hdc, attached.main);
        // A docked panel sits flush under the main card; square the card's
        // bottom corners so the shared seam has no notches. (The webview
        // layer hides the card's bottom corner caps for the same reason.)
        if attached.panels.iter().any(|panel| panel.docked) {
            square_card_bottom_corners(hdc, attached.main);
        }
        for panel in &attached.panels {
            // Terminal panels paint their own full-bleed dark card.
            if panel
                .native
                .as_ref()
                .is_some_and(|native| native.kind == WindowsNativePanelKind::Terminal)
            {
                continue;
            }
            draw_content_card(hdc, panel.rect);
        }
        for panel in &attached.panels {
            // Maximized panels cover the sidebar and are repainted after
            // the sidebar pass in `draw_window_chrome`.
            if panel.native.is_some() && !panel_is_maximized(panel) {
                draw_native_panel_content(hdc, state.hwnd, panel);
            }
        }
        // Attached card corners are rounded by lingxia-webview's layered
        // corner-cap overlays (see `card_corner_color` above); the cards
        // themselves are plain filled rounded rects.
        return;
    }

    draw_content_card(hdc, rects.panel);
}

fn panel_is_maximized(panel: &WindowsChromePanel) -> bool {
    panel
        .native
        .as_ref()
        .is_some_and(|native| native.maximized)
}

/// Painted after the sidebar/tab-bar pass: a maximized native panel covers
/// the sidebar, so it must draw above everything except the frame buttons.
fn draw_maximized_native_panels(hdc: HDC, state: &WindowsChromeState) {
    let Some(attached) = &state.attached else {
        return;
    };
    for panel in &attached.panels {
        if panel.native.is_some() && panel_is_maximized(panel) {
            draw_native_panel_content(hdc, state.hwnd, panel);
        }
    }
}

/// Overpaints the bottom rounded corners of a card with its own fill so
/// the bottom edge becomes square (used above a flush docked panel).
pub(super) fn square_card_bottom_corners(hdc: HDC, rect: RECT) {
    if rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: (rect.bottom - SHELL_PANEL_RADIUS).max(rect.top),
            right: rect.right,
            bottom: rect.bottom,
        },
        SHELL_PANEL_BACKGROUND,
    );
}

pub(super) fn draw_content_card(hdc: HDC, rect: RECT) {
    if rect_width(&rect) > 0 && rect_height(&rect) > 0 {
        // White card on the gray window background: the arc must be
        // anti-aliased (and match the corner-cap radius of webview cards).
        fill_round_rect_aa(hdc, rect, SHELL_PANEL_RADIUS, SHELL_PANEL_BACKGROUND);
    }
}

pub(super) fn draw_native_panel_content(hdc: HDC, hwnd: HWND, panel: &WindowsChromePanel) {
    let Some(native) = &panel.native else {
        return;
    };
    if native.kind == WindowsNativePanelKind::Terminal {
        draw_terminal_panel_content(hdc, hwnd, panel, native);
        return;
    }

    let content = inset_rect(panel.rect, 22, 22);
    let title_rect = RECT {
        left: content.left,
        top: content.top,
        right: content.right,
        bottom: content.top + 26,
    };
    let body_rect = RECT {
        left: content.left,
        top: title_rect.bottom + 10,
        right: content.right,
        bottom: title_rect.bottom + 42,
    };
    draw_text(
        hdc,
        native.title.as_deref().unwrap_or("Panel"),
        title_rect,
        SHELL_TEXT_PRIMARY,
        DT_LEFT,
    );
    draw_text(
        hdc,
        native.body.as_deref().unwrap_or_default(),
        body_rect,
        SHELL_TEXT_MUTED,
        DT_LEFT,
    );
}

/// Header geometry of one terminal panel tab.
pub(super) struct TerminalHeaderTab {
    pub(super) tab_id: u64,
    pub(super) active: bool,
    /// Full clickable tab rect.
    pub(super) rect: RECT,
    /// Title area inside the tab (the inline rename editor covers it).
    pub(super) title: RECT,
    /// Close glyph rect; `Some` only on the active tab.
    pub(super) close: Option<RECT>,
}

/// Computed header geometry of a terminal panel: tab strip, new-tab
/// button, and the right-aligned maximize/restore toggle. Shared between
/// drawing and hit-testing so both always agree.
pub(super) struct TerminalHeaderRects {
    pub(super) header: RECT,
    pub(super) tabs: Vec<TerminalHeaderTab>,
    pub(super) new_tab: Option<RECT>,
    pub(super) maximize: Option<RECT>,
}

pub(super) fn terminal_header_rects(
    rect: RECT,
    native: &WindowsNativePanelContent,
) -> TerminalHeaderRects {
    let header = normalize_rect(RECT {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: (rect.top + TERMINAL_HEADER_HEIGHT).min(rect.bottom),
    });
    let button_top = header.top + (rect_height(&header) - TERMINAL_HEADER_BUTTON_SIZE).max(0) / 2;
    let square_button = |left: i32| {
        normalize_rect(RECT {
            left,
            top: button_top,
            right: left + TERMINAL_HEADER_BUTTON_SIZE,
            bottom: button_top + TERMINAL_HEADER_BUTTON_SIZE,
        })
    };

    let maximize_left = header.right - TERMINAL_HEADER_PADDING - TERMINAL_HEADER_BUTTON_SIZE;
    let maximize = (maximize_left > header.left).then(|| square_button(maximize_left));
    let tabs_right_limit = maximize
        .map(|rect| rect.left - TERMINAL_TAB_GAP)
        .unwrap_or(header.right - TERMINAL_HEADER_PADDING);

    let mut tabs = Vec::with_capacity(native.tabs.len());
    let mut left = header.left + TERMINAL_HEADER_PADDING;
    let count = native.tabs.len() as i32;
    if count > 0 {
        // Reserve room for the new-tab button after the last tab, then
        // split the rest evenly (capped at the max tab width).
        let avail = (tabs_right_limit
            - left
            - (TERMINAL_HEADER_BUTTON_SIZE + TERMINAL_TAB_GAP)
            - (count - 1) * TERMINAL_TAB_GAP)
            .max(0);
        let tab_width = (avail / count).min(TERMINAL_TAB_MAX_WIDTH).max(24);
        for item in &native.tabs {
            let tab_rect = normalize_rect(RECT {
                left,
                top: header.top + TERMINAL_TAB_TOP_INSET,
                right: (left + tab_width).min(tabs_right_limit),
                bottom: header.bottom,
            });
            let close = (item.active && rect_width(&tab_rect) >= 3 * TERMINAL_TAB_CLOSE_WIDTH)
                .then(|| {
                    normalize_rect(RECT {
                        left: tab_rect.right - TERMINAL_TAB_CLOSE_WIDTH,
                        top: tab_rect.top,
                        right: tab_rect.right,
                        bottom: tab_rect.bottom,
                    })
                });
            let title = normalize_rect(RECT {
                left: tab_rect.left + 10,
                top: tab_rect.top,
                right: close.map(|close| close.left).unwrap_or(tab_rect.right - 6),
                bottom: tab_rect.bottom,
            });
            tabs.push(TerminalHeaderTab {
                tab_id: item.id,
                active: item.active,
                rect: tab_rect,
                title,
                close,
            });
            left = tab_rect.right + TERMINAL_TAB_GAP;
        }
    }

    let new_tab =
        (left + TERMINAL_HEADER_BUTTON_SIZE <= tabs_right_limit).then(|| square_button(left));

    TerminalHeaderRects {
        header,
        tabs,
        new_tab,
        maximize,
    }
}

/// Maps a point inside a terminal panel's header to its interactive
/// elements; `None` for the header background and the terminal body.
pub(super) fn terminal_header_hit_test(
    panel: &WindowsChromePanel,
    point: (i32, i32),
) -> Option<WindowsChromeHit> {
    let native = panel.native.as_ref()?;
    let rects = terminal_header_rects(panel.rect, native);
    if !rect_contains(&rects.header, point) {
        return None;
    }
    if let Some(maximize) = rects.maximize
        && rect_contains(&maximize, point)
    {
        return Some(WindowsChromeHit::NativePanelMaximize {
            panel_id: panel.panel_id.clone(),
        });
    }
    if let Some(new_tab) = rects.new_tab
        && rect_contains(&new_tab, point)
    {
        return Some(WindowsChromeHit::NativePanelNewTab {
            panel_id: panel.panel_id.clone(),
        });
    }
    for tab in &rects.tabs {
        if let Some(close) = tab.close
            && rect_contains(&close, point)
        {
            return Some(WindowsChromeHit::NativePanelTabClose {
                panel_id: panel.panel_id.clone(),
                tab_id: tab.tab_id,
            });
        }
        if rect_contains(&tab.rect, point) {
            return Some(WindowsChromeHit::NativePanelTab {
                panel_id: panel.panel_id.clone(),
                tab_id: tab.tab_id,
            });
        }
    }
    None
}

/// Draws a terminal panel as a compact dock: full-bleed surface card, a
/// 34px header strip (tabs + new-tab + maximize), and the cell grid below.
/// Docked panels keep square top corners (flush seam with the main card);
/// while maximized the panel is the whole content area and is rounded all
/// around.
pub(super) fn draw_terminal_panel_content(
    hdc: HDC,
    hwnd: HWND,
    panel: &WindowsChromePanel,
    native: &WindowsNativePanelContent,
) {
    let rect = panel.rect;
    if rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }
    let surface = super::terminal_grid::panel_surface_background(&panel.panel_id)
        .unwrap_or(TERMINAL_SURFACE_BACKGROUND);
    let square_top = panel.docked && !native.maximized;

    // Surface card: dark terminal surface on the light window background —
    // the rounded corners (bottom while docked; all four when maximized or
    // floating) need anti-aliasing. Docked panels then square their top
    // corners with the overpaint below.
    fill_round_rect_aa(hdc, rect, SHELL_PANEL_RADIUS, surface);
    if square_top {
        fill_rect(
            hdc,
            RECT {
                left: rect.left,
                top: rect.top,
                right: rect.right,
                bottom: (rect.top + SHELL_PANEL_RADIUS).min(rect.bottom),
            },
            surface,
        );
    }

    // Header strip: bottom corners always square (it joins the surface);
    // top corners follow the card's corner shape.
    let header_rects = terminal_header_rects(rect, native);
    let header = header_rects.header;
    if square_top {
        fill_rect(hdc, header, TERMINAL_HEADER_BACKGROUND);
    } else {
        fill_round_rect_aa(hdc, header, SHELL_PANEL_RADIUS, TERMINAL_HEADER_BACKGROUND);
        fill_rect(
            hdc,
            RECT {
                left: header.left,
                top: header.top + rect_height(&header) / 2,
                right: header.right,
                bottom: header.bottom,
            },
            TERMINAL_HEADER_BACKGROUND,
        );
    }

    for tab in &header_rects.tabs {
        if tab.active {
            // The active tab flows into the surface below it: surface
            // fill, rounded on top, square at the header's bottom edge.
            // Surface-on-header contrast — anti-alias the pill arc.
            fill_round_rect_aa(hdc, tab.rect, 10, surface);
            fill_rect(
                hdc,
                RECT {
                    left: tab.rect.left,
                    top: tab.rect.top + rect_height(&tab.rect) / 2,
                    right: tab.rect.right,
                    bottom: tab.rect.bottom,
                },
                surface,
            );
        }
        let title = native
            .tabs
            .iter()
            .find(|item| item.id == tab.tab_id)
            .map(|item| item.title.as_str())
            .unwrap_or_default();
        let color = if tab.active {
            TERMINAL_HEADER_TEXT
        } else {
            TERMINAL_HEADER_TEXT_MUTED
        };
        draw_text(hdc, title, tab.title, color, DT_LEFT);
        if let Some(close) = tab.close {
            draw_text(hdc, GLYPH_TAB_CLOSE, close, TERMINAL_HEADER_TEXT_MUTED, DT_CENTER);
        }
    }
    if header_rects.tabs.is_empty() {
        // Pre-session states (starting, runtime unavailable): plain title.
        let title_rect = normalize_rect(RECT {
            left: header.left + TERMINAL_HEADER_PADDING + 4,
            top: header.top,
            right: header_rects
                .new_tab
                .map(|rect| rect.left)
                .unwrap_or(header.right - TERMINAL_HEADER_PADDING),
            bottom: header.bottom,
        });
        draw_text(
            hdc,
            native.title.as_deref().unwrap_or("Terminal"),
            title_rect,
            TERMINAL_HEADER_TEXT,
            DT_LEFT,
        );
    }
    if let Some(new_tab) = header_rects.new_tab {
        draw_frame_button_glyph(hdc, GLYPH_ADD, new_tab, TERMINAL_HEADER_TEXT_MUTED);
    }
    if let Some(maximize) = header_rects.maximize {
        let glyph = if native.maximized {
            GLYPH_RESTORE
        } else {
            GLYPH_MAXIMIZE
        };
        draw_frame_button_glyph(hdc, glyph, maximize, TERMINAL_HEADER_TEXT_MUTED);
    }

    // Record the painted tab-title rects so the facade can start an inline
    // rename (EDIT child) over the double-clicked title.
    super::terminal_grid::set_panel_tab_title_rects(
        &panel.panel_id,
        hwnd.0 as isize,
        header_rects
            .tabs
            .iter()
            .map(|tab| (tab.tab_id, tab.title))
            .collect(),
    );

    // Terminal body below the header.
    let body = normalize_rect(RECT {
        left: rect.left,
        top: header.bottom,
        right: rect.right,
        bottom: rect.bottom,
    });
    if rect_width(&body) == 0 || rect_height(&body) == 0 {
        return;
    }

    // Live sessions are drawn as a cell grid from the snapshot store; the
    // body-text path below remains for pre-session states ("Starting
    // terminal...", runtime-unavailable, failures).
    if super::terminal_grid::draw_panel_grid(hdc, &panel.panel_id, body) {
        return;
    }

    let text_rect = inset_rect(body, 12, 10);
    let line_height = logical_font_height(hdc, 10).max(13);
    let max_lines = (rect_height(&text_rect) / line_height).max(1) as usize;
    let body = native
        .body
        .as_deref()
        .filter(|body| !body.trim().is_empty())
        .unwrap_or("Starting terminal...");

    unsafe {
        let font = CreateFontW(
            -logical_font_height(hdc, 10),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32 | FF_SWISS.0 as u32,
            w!("Cascadia Mono"),
        );
        let old_font = if font.is_invalid() {
            HGDIOBJ::default()
        } else {
            SelectObject(hdc, HGDIOBJ(font.0))
        };
        let _ = SetBkMode(hdc, TRANSPARENT);
        let _ = SetTextColor(hdc, rgb_to_colorref(SHELL_TERMINAL_TEXT));
        for (line_index, line) in body.lines().take(max_lines).enumerate() {
            let top = text_rect.top + (line_index as i32 * line_height);
            let mut line_rect = RECT {
                left: text_rect.left,
                top,
                right: text_rect.right,
                bottom: (top + line_height).min(text_rect.bottom),
            };
            if rect_height(&line_rect) <= 0 {
                break;
            }
            let mut wide: Vec<u16> = line.encode_utf16().collect();
            let _ = DrawTextW(
                hdc,
                &mut wide,
                &mut line_rect,
                DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS,
            );
        }
        if !old_font.is_invalid() {
            let _ = SelectObject(hdc, old_font);
        }
        if !font.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
    }
}

pub(super) fn draw_shell_top_bar(hdc: HDC, rects: &ChromeRects) {
    fill_rect(hdc, rects.top_bar, SHELL_WINDOW_BACKGROUND);
    draw_bottom_border(hdc, rects.top_bar, SHELL_DIVIDER);
}

pub(super) fn draw_navigation_bar(hdc: HDC, rect: RECT, navbar: &WindowsNavigationBarLayout) {
    fill_rect(hdc, rect, navbar.background_color);
    draw_bottom_border(hdc, rect, 0xe6e6e6);

    let text_color = navbar.text_color;
    let mut left_controls_width = 0;

    if navbar.show_back_button {
        let back_rect = nav_button_rect(rect, 0);
        draw_text(hdc, "<", back_rect, text_color, DT_CENTER);
        left_controls_width = back_rect.right - rect.left;
    }
    if navbar.show_home_button {
        let home_rect = nav_button_rect(rect, if navbar.show_back_button { 1 } else { 0 });
        draw_text(hdc, "Home", home_rect, text_color, DT_CENTER);
        left_controls_width = home_rect.right - rect.left;
    }

    if !navbar.title.trim().is_empty() {
        let title_inset = (left_controls_width + 8).max(window_frame_buttons_width() + 8);
        let title_rect = normalize_rect(RECT {
            left: rect.left + title_inset,
            top: rect.top,
            right: rect.right - title_inset,
            bottom: rect.bottom,
        });
        draw_text(hdc, &navbar.title, title_rect, text_color, DT_CENTER);
    }
}

/// Draws the Win11-style caption buttons: 46px-wide cells flush against the
/// top-right edge, Segoe Fluent Icons glyphs (restore glyph while zoomed),
/// and system hover/pressed states — the close button turns system red with
/// a white glyph; minimize/maximize get a subtle black overlay.
pub(super) fn draw_window_frame_buttons(hdc: HDC, state: &WindowsChromeState) {
    for (button, rect) in window_frame_button_rects(state.client) {
        let hovered = state.frame_button_hover == Some(button);
        let pressed_here = state.frame_button_pressed == Some(button);
        // Pressed visual needs the cursor on the button; hovering a button
        // while another button's click is in flight shows no highlight.
        let show_pressed = hovered && pressed_here;
        let show_hover =
            hovered && (state.frame_button_pressed.is_none() || pressed_here) && !show_pressed;

        let background = if button == WindowsFrameButton::Close {
            if show_pressed {
                Some(SHELL_CLOSE_PRESSED)
            } else if show_hover {
                Some(SHELL_CLOSE_HOVER)
            } else {
                None
            }
        } else if show_pressed {
            Some(darken_rgb(
                SHELL_WINDOW_BACKGROUND,
                FRAME_BUTTON_PRESSED_OVERLAY,
            ))
        } else if show_hover {
            Some(darken_rgb(SHELL_WINDOW_BACKGROUND, FRAME_BUTTON_HOVER_OVERLAY))
        } else {
            None
        };
        if let Some(background) = background {
            fill_rect(hdc, rect, background);
        }

        let glyph = match button {
            WindowsFrameButton::Minimize => GLYPH_MINIMIZE,
            WindowsFrameButton::Maximize => {
                if unsafe { WindowsAndMessaging::IsZoomed(state.hwnd).as_bool() } {
                    GLYPH_RESTORE
                } else {
                    GLYPH_MAXIMIZE
                }
            }
            WindowsFrameButton::Close => GLYPH_CLOSE,
        };
        let glyph_color = if button == WindowsFrameButton::Close && (show_hover || show_pressed) {
            0xffffff
        } else {
            SHELL_FRAME_BUTTON_ICON
        };
        draw_frame_button_glyph(hdc, glyph, rect, glyph_color);
    }
}

/// Blends `percent`% black into an `0xRRGGBB` color.
pub(super) fn darken_rgb(rgb: u32, percent: u32) -> u32 {
    let blend = |channel: u32| channel * (100 - percent) / 100;
    (blend((rgb >> 16) & 0xff) << 16) | (blend((rgb >> 8) & 0xff) << 8) | blend(rgb & 0xff)
}

pub(super) fn draw_frame_button_glyph(hdc: HDC, glyph: &str, rect: RECT, rgb: u32) {
    let mut wide: Vec<u16> = glyph.encode_utf16().collect();
    let mut rect = rect;
    unsafe {
        let font = create_caption_icon_font(hdc);
        let old_font = if font.is_invalid() {
            HGDIOBJ::default()
        } else {
            SelectObject(hdc, HGDIOBJ(font.0))
        };
        let _ = SetBkMode(hdc, TRANSPARENT);
        let _ = SetTextColor(hdc, rgb_to_colorref(rgb));
        let _ = DrawTextW(hdc, &mut wide, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
        if !old_font.is_invalid() {
            let _ = SelectObject(hdc, old_font);
        }
        if !font.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
    }
}

/// Caption icon font: Segoe Fluent Icons (Win11), falling back to Segoe
/// MDL2 Assets (Win10). The GDI font mapper silently substitutes missing
/// faces, so each candidate is verified via `GetTextFaceW` before its
/// private-use glyphs are trusted.
pub(super) fn create_caption_icon_font(hdc: HDC) -> HFONT {
    let height = -logical_font_height(hdc, WINDOW_BUTTON_GLYPH_POINT_SIZE);
    for face in ["Segoe Fluent Icons", "Segoe MDL2 Assets"] {
        let face_wide: Vec<u16> = face.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let font = CreateFontW(
                height,
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                DEFAULT_PITCH.0 as u32 | FF_SWISS.0 as u32,
                PCWSTR(face_wide.as_ptr()),
            );
            if font.is_invalid() {
                continue;
            }
            let old_font = SelectObject(hdc, HGDIOBJ(font.0));
            let mut resolved = [0u16; 64];
            let copied = GetTextFaceW(hdc, Some(&mut resolved)).max(0) as usize;
            if !old_font.is_invalid() {
                let _ = SelectObject(hdc, old_font);
            }
            let resolved_len = resolved
                .iter()
                .position(|&unit| unit == 0)
                .unwrap_or(copied.min(resolved.len()));
            let resolved = String::from_utf16_lossy(&resolved[..resolved_len]);
            if resolved.eq_ignore_ascii_case(face) {
                return font;
            }
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
    }
    HFONT::default()
}

pub(super) fn window_frame_buttons_width() -> i32 {
    WINDOW_BUTTON_WIDTH * 3
}

pub(super) fn window_frame_button_rects(client: RECT) -> [(WindowsFrameButton, RECT); 3] {
    let top = client.top;
    let bottom = (client.top + SHELL_TOP_BAR_HEIGHT).min(client.bottom);
    let close = RECT {
        left: client.right - WINDOW_BUTTON_WIDTH,
        top,
        right: client.right,
        bottom,
    };
    let maximize = RECT {
        left: close.left - WINDOW_BUTTON_WIDTH,
        top,
        right: close.left,
        bottom,
    };
    let minimize = RECT {
        left: maximize.left - WINDOW_BUTTON_WIDTH,
        top,
        right: maximize.left,
        bottom,
    };
    [
        (WindowsFrameButton::Minimize, normalize_rect(minimize)),
        (WindowsFrameButton::Maximize, normalize_rect(maximize)),
        (WindowsFrameButton::Close, normalize_rect(close)),
    ]
}

pub(super) fn nav_button_rect(navbar: RECT, index: i32) -> RECT {
    let width = 44;
    RECT {
        left: navbar.left + 8 + index * width,
        top: navbar.top,
        right: navbar.left + 8 + (index + 1) * width,
        bottom: navbar.bottom,
    }
}

pub(super) fn draw_tab_bar(hdc: HDC, rect: RECT, tabbar: &WindowsTabBarLayout) {
    if matches!(
        tabbar.position,
        WindowsTabBarPosition::Left | WindowsTabBarPosition::Right
    ) {
        draw_sidebar_tab_bar(hdc, rect, tabbar);
        return;
    }

    fill_rect(hdc, rect, tabbar.background_color);
    draw_tabbar_border(hdc, rect, tabbar);

    let count = tabbar.items.len();
    if count == 0 {
        return;
    }

    for (index, item) in tabbar.items.iter().enumerate() {
        let item_rect = tab_item_rect(rect, tabbar.position, count, index);
        let selected = tabbar.selected_index == index as i32;
        if selected {
            fill_rect(
                hdc,
                inset_rect(item_rect, 4, 5),
                SHELL_TAB_SELECTED_BACKGROUND,
            );
        }

        let text_color = if selected {
            tabbar.selected_color
        } else {
            tabbar.color
        };
        let mut label_rect = inset_rect(item_rect, 6, 4);
        if matches!(tabbar.position, WindowsTabBarPosition::Bottom) {
            label_rect.top += 6;
        }
        draw_text(hdc, &item.text, label_rect, text_color, DT_CENTER);

        if let Some(badge) = item.badge.as_ref().filter(|badge| !badge.is_empty()) {
            draw_badge(hdc, item_rect, badge);
        } else if item.has_red_dot {
            draw_red_dot(hdc, item_rect);
        }
    }
}

pub(super) fn draw_sidebar_tab_bar(hdc: HDC, rect: RECT, tabbar: &WindowsTabBarLayout) {
    fill_rect(hdc, rect, SHELL_SIDEBAR_BACKGROUND);

    let title = if tabbar.app_name.trim().is_empty() {
        "LXAPP".to_string()
    } else {
        tabbar.app_name.to_ascii_uppercase()
    };
    let header_rect = RECT {
        left: rect.left + SIDEBAR_ITEM_INSET + 2,
        top: rect.top + 22,
        right: rect.right - SIDEBAR_ITEM_INSET,
        bottom: rect.top + SIDEBAR_HEADER_HEIGHT,
    };
    draw_text(hdc, &title, header_rect, SHELL_SIDEBAR_HEADER_TEXT, DT_LEFT);

    for (index, item) in tabbar.items.iter().enumerate() {
        let item_rect = sidebar_item_rect(rect, index);
        let selected = tabbar.selected_index == index as i32;
        if selected {
            // White item card on the gray sidebar, accent bar on white.
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

        let label_rect = RECT {
            left: item_rect.left + 42,
            top: item_rect.top,
            right: item_rect.right - 8,
            bottom: item_rect.bottom,
        };
        let text_color = if selected {
            SHELL_TEXT_PRIMARY
        } else {
            SHELL_TEXT_MUTED
        };
        let icon_path = if selected && !item.selected_icon_path.trim().is_empty() {
            &item.selected_icon_path
        } else {
            &item.icon_path
        };
        if !icon_path.trim().is_empty() {
            let icon_rect = centered_icon_rect(
                RECT {
                    left: item_rect.left + 18,
                    top: item_rect.top,
                    right: item_rect.left + 18 + SIDEBAR_ICON_SIZE,
                    bottom: item_rect.bottom,
                },
                SIDEBAR_ICON_SIZE,
            );
            if !draw_icon_from_path(hdc, icon_path, icon_rect, SIDEBAR_ICON_SIZE as u32) {
                draw_text(hdc, "?", icon_rect, text_color, DT_CENTER);
            }
        }
        draw_text(hdc, &item.text, label_rect, text_color, DT_LEFT);

        if let Some(badge) = item.badge.as_ref().filter(|badge| !badge.is_empty()) {
            draw_badge(hdc, item_rect, badge);
        } else if item.has_red_dot {
            draw_red_dot(hdc, item_rect);
        }
    }

    draw_sidebar_browser_section(hdc, rect, tabbar);

    let footer_top = rect.bottom - SIDEBAR_FOOTER_HEIGHT;
    draw_top_border(
        hdc,
        RECT {
            left: rect.left + SIDEBAR_ITEM_INSET,
            top: footer_top,
            right: rect.right - SIDEBAR_ITEM_INSET,
            bottom: rect.bottom,
        },
        SHELL_DIVIDER,
    );
}

pub(super) fn draw_tabbar_border(hdc: HDC, rect: RECT, tabbar: &WindowsTabBarLayout) {
    match tabbar.position {
        WindowsTabBarPosition::Bottom => draw_top_border(hdc, rect, tabbar.border_color),
        WindowsTabBarPosition::Left => draw_right_border(hdc, rect, tabbar.border_color),
        WindowsTabBarPosition::Right => draw_left_border(hdc, rect, tabbar.border_color),
    }
}

pub(super) fn tab_item_rect(
    rect: RECT,
    position: WindowsTabBarPosition,
    count: usize,
    index: usize,
) -> RECT {
    let count_i32 = count.max(1) as i32;
    let index_i32 = index as i32;
    match position {
        WindowsTabBarPosition::Bottom => {
            let width = (rect_width(&rect) / count_i32).max(1);
            let left = rect.left + width * index_i32;
            RECT {
                left,
                top: rect.top,
                right: if index + 1 == count {
                    rect.right
                } else {
                    left + width
                },
                bottom: rect.bottom,
            }
        }
        WindowsTabBarPosition::Left | WindowsTabBarPosition::Right => {
            let height = (rect_height(&rect) / count_i32).max(1);
            let top = rect.top + height * index_i32;
            RECT {
                left: rect.left,
                top,
                right: rect.right,
                bottom: if index + 1 == count {
                    rect.bottom
                } else {
                    top + height
                },
            }
        }
    }
}

pub(super) fn sidebar_item_rect(rect: RECT, index: usize) -> RECT {
    let top =
        rect.top + SIDEBAR_HEADER_HEIGHT + index as i32 * (SIDEBAR_ITEM_HEIGHT + SIDEBAR_ITEM_GAP);
    normalize_rect(RECT {
        left: rect.left + SIDEBAR_ITEM_INSET,
        top,
        right: rect.right - SIDEBAR_ITEM_INSET,
        bottom: top + SIDEBAR_ITEM_HEIGHT,
    })
}

/// Geometry of the sidebar browser section: separator line, one row rect
/// per browser tab (rows that would collide with the footer are dropped),
/// and the "New Tab" row.
pub(super) struct SidebarBrowserRects {
    pub(super) separator: RECT,
    /// Row rects aligned index-for-index with `tabbar.browser_tabs`
    /// (possibly truncated when rows run out of vertical space).
    pub(super) tabs: Vec<RECT>,
    pub(super) new_tab: Option<RECT>,
}

pub(super) fn sidebar_browser_rects(
    rect: RECT,
    tabbar: &WindowsTabBarLayout,
) -> Option<SidebarBrowserRects> {
    if tabbar.browser_tabs.is_empty() && !tabbar.show_browser_new_tab {
        return None;
    }
    let footer_top = rect.bottom - SIDEBAR_FOOTER_HEIGHT;
    let items_bottom = rect.top
        + SIDEBAR_HEADER_HEIGHT
        + tabbar.items.len() as i32 * (SIDEBAR_ITEM_HEIGHT + SIDEBAR_ITEM_GAP);
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

pub(super) fn sidebar_browser_hit_test(
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

pub(super) fn sidebar_browser_close_rect(item_rect: RECT) -> RECT {
    normalize_rect(RECT {
        left: item_rect.right - SIDEBAR_BROWSER_CLOSE_SIZE,
        top: item_rect.top,
        right: item_rect.right,
        bottom: item_rect.bottom,
    })
}

pub(super) fn draw_sidebar_browser_section(hdc: HDC, rect: RECT, tabbar: &WindowsTabBarLayout) {
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
        let label_rect = normalize_rect(RECT {
            left: item_rect.left + 16,
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
        draw_text(hdc, GLYPH_TAB_CLOSE, close_rect, SHELL_TEXT_MUTED, DT_CENTER);
    }

    if let Some(new_tab_rect) = browser.new_tab {
        let label_rect = normalize_rect(RECT {
            left: new_tab_rect.left + 16,
            top: new_tab_rect.top,
            right: new_tab_rect.right - 8,
            bottom: new_tab_rect.bottom,
        });
        draw_text(hdc, "+  New Tab", label_rect, SHELL_TEXT_MUTED, DT_LEFT);
    }
}

pub(super) fn panel_activator_rects(
    client: RECT,
    rects: &ChromeRects,
    layout: &WindowsWindowLayout,
) -> Vec<(String, RECT)> {
    if layout.panel_activators.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(layout.panel_activators.len());

    if let (Some(tabbar), Some(tabbar_rect)) = (&layout.tab_bar, rects.tab_bar)
        && matches!(
            tabbar.position,
            WindowsTabBarPosition::Left | WindowsTabBarPosition::Right
        )
    {
        let footer_top = tabbar_rect.bottom - SIDEBAR_FOOTER_HEIGHT;
        let top = footer_top + (SIDEBAR_FOOTER_HEIGHT - PANEL_ACTIVATOR_SIZE) / 2;
        let mut right = tabbar_rect.right - PANEL_ACTIVATOR_MARGIN;
        for activator in &layout.panel_activators {
            let left = right - PANEL_ACTIVATOR_SIZE;
            if left < tabbar_rect.left + PANEL_ACTIVATOR_MARGIN {
                break;
            }
            out.push((
                activator.id.clone(),
                normalize_rect(RECT {
                    left,
                    top,
                    right,
                    bottom: top + PANEL_ACTIVATOR_SIZE,
                }),
            ));
            right = left - PANEL_ACTIVATOR_GAP;
        }
        return out;
    }

    let bottom_limit = rects
        .tab_bar
        .map(|tabbar| tabbar.top)
        .unwrap_or(client.bottom);
    let left = rects.panel.left + PANEL_ACTIVATOR_MARGIN;
    let mut bottom = bottom_limit - PANEL_ACTIVATOR_MARGIN;

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

pub(super) fn inset_rect(rect: RECT, dx: i32, dy: i32) -> RECT {
    normalize_rect(RECT {
        left: rect.left + dx,
        top: rect.top + dy,
        right: rect.right - dx,
        bottom: rect.bottom - dy,
    })
}

pub(super) fn draw_panel_activators(
    hdc: HDC,
    client: RECT,
    rects: &ChromeRects,
    layout: &WindowsWindowLayout,
) {
    for (panel_id, rect) in panel_activator_rects(client, rects, layout) {
        let active = layout
            .panel_activators
            .iter()
            .find(|item| item.id == panel_id)
            .is_some_and(|item| item.active);
        let activator = layout
            .panel_activators
            .iter()
            .find(|item| item.id == panel_id);
        let text = activator
            .map(|item| panel_activator_label(&item.label))
            .unwrap_or_else(|| panel_activator_label(&panel_id));
        let text_color = if active {
            SHELL_ACCENT
        } else {
            SHELL_TEXT_MUTED
        };

        if active {
            // White activator pill on the gray sidebar footer.
            fill_round_rect_aa(hdc, rect, 6, 0xffffff);
            fill_round_rect_aa(
                hdc,
                RECT {
                    left: rect.left + 3,
                    top: rect.bottom - 5,
                    right: rect.right - 3,
                    bottom: rect.bottom - 3,
                },
                2,
                SHELL_ACCENT,
            );
        }
        let icon_rect = centered_icon_rect(rect, PANEL_ACTIVATOR_ICON_SIZE);
        let icon_drawn = activator
            .filter(|item| !item.icon_path.trim().is_empty())
            .is_some_and(|item| {
                draw_icon_from_path(
                    hdc,
                    &item.icon_path,
                    icon_rect,
                    PANEL_ACTIVATOR_ICON_SIZE as u32,
                )
            });
        if !icon_drawn {
            draw_text(hdc, &text, rect, text_color, DT_CENTER);
        }
    }
}

pub(super) fn panel_activator_label(label: &str) -> String {
    let mut out = String::new();
    for ch in label.chars().filter(|ch| ch.is_ascii_alphanumeric()) {
        out.push(ch.to_ascii_uppercase());
        if out.len() == 2 {
            break;
        }
    }
    if out.is_empty() { "?".to_string() } else { out }
}

/// Chrome text font ("Segoe UI" at the shell text size/weight) sized for
/// `hdc`'s DPI. The caller owns the returned font and deletes it after use.
pub(super) fn create_chrome_text_font(hdc: HDC) -> HFONT {
    unsafe {
        CreateFontW(
            -logical_font_height(hdc, SHELL_TEXT_POINT_SIZE),
            0,
            0,
            0,
            SHELL_TEXT_WEIGHT,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32 | FF_SWISS.0 as u32,
            w!("Segoe UI"),
        )
    }
}

pub(super) fn draw_text(
    hdc: HDC,
    text: &str,
    rect: RECT,
    rgb: u32,
    horizontal: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    if text.is_empty() || rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }

    let mut wide: Vec<u16> = text.encode_utf16().collect();
    let mut rect = rect;
    let font = create_chrome_text_font(hdc);
    unsafe {
        let old_font = if font.is_invalid() {
            HGDIOBJ::default()
        } else {
            SelectObject(hdc, HGDIOBJ(font.0))
        };
        let _ = SetBkMode(hdc, TRANSPARENT);
        let _ = SetTextColor(hdc, rgb_to_colorref(rgb));
        let _ = DrawTextW(
            hdc,
            &mut wide,
            &mut rect,
            horizontal | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        );
        if !old_font.is_invalid() {
            let _ = SelectObject(hdc, old_font);
        }
        if !font.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
    }
}

pub(super) fn logical_font_height(hdc: HDC, point_size: i32) -> i32 {
    let dpi_y = unsafe { GetDeviceCaps(Some(hdc), LOGPIXELSY) };
    let dpi_y = if dpi_y > 0 { dpi_y } else { 96 };
    (point_size * dpi_y + 36) / 72
}

pub(super) fn draw_badge(hdc: HDC, item_rect: RECT, badge: &str) {
    let badge_rect = RECT {
        left: item_rect.right - 30,
        top: item_rect.top + 7,
        right: item_rect.right - 8,
        bottom: item_rect.top + 25,
    };
    fill_rect(hdc, badge_rect, SHELL_BADGE_RED);
    draw_text(hdc, badge, badge_rect, 0xffffff, DT_CENTER);
}

pub(super) fn draw_red_dot(hdc: HDC, item_rect: RECT) {
    let dot_rect = RECT {
        left: item_rect.right - 18,
        top: item_rect.top + 9,
        right: item_rect.right - 10,
        bottom: item_rect.top + 17,
    };
    fill_rect(hdc, dot_rect, SHELL_BADGE_RED);
}

pub(super) fn draw_top_border(hdc: HDC, rect: RECT, rgb: u32) {
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.top + 1,
        },
        rgb,
    );
}

pub(super) fn draw_bottom_border(hdc: HDC, rect: RECT, rgb: u32) {
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: rect.bottom - 1,
            right: rect.right,
            bottom: rect.bottom,
        },
        rgb,
    );
}

pub(super) fn draw_left_border(hdc: HDC, rect: RECT, rgb: u32) {
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: rect.top,
            right: rect.left + 1,
            bottom: rect.bottom,
        },
        rgb,
    );
}

pub(super) fn draw_right_border(hdc: HDC, rect: RECT, rgb: u32) {
    fill_rect(
        hdc,
        RECT {
            left: rect.right - 1,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        },
        rgb,
    );
}

pub(super) fn fill_rect(hdc: HDC, rect: RECT, rgb: u32) {
    if rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }
    unsafe {
        let brush = CreateSolidBrush(rgb_to_colorref(rgb));
        if brush.is_invalid() {
            return;
        }
        let _ = FillRect(hdc, &rect, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

/// Starts GDI+ once for the process (the shell paints chrome until exit,
/// so the library is never shut down). Returns `false` when startup failed;
/// rounded fills then fall back to aliased GDI `RoundRect`.
fn ensure_gdiplus_started() -> bool {
    static STARTED: OnceLock<bool> = OnceLock::new();
    *STARTED.get_or_init(|| {
        let input = GdiPlus::GdiplusStartupInput {
            GdiplusVersion: 1,
            ..Default::default()
        };
        let mut token = 0usize;
        let status = unsafe { GdiPlus::GdiplusStartup(&mut token, &input, std::ptr::null_mut()) };
        if status != GdiPlus::Ok {
            log::warn!(
                "GdiplusStartup failed ({}); rounded chrome falls back to aliased GDI",
                status.0
            );
        }
        status == GdiPlus::Ok
    })
}

/// Fills a rounded rectangle with an anti-aliased GDI+ path. `radius` is
/// the true corner radius (matching the corner-cap overlays, not GDI
/// `RoundRect`'s ellipse-diameter semantics), clamped to the rect. Used for
/// every rounded shape the chrome paints over a contrasting background —
/// plain GDI fills alias the arc into a hard staircase.
pub(super) fn fill_round_rect_aa(hdc: HDC, rect: RECT, radius: i32, rgb: u32) {
    let width = rect_width(&rect);
    let height = rect_height(&rect);
    if width == 0 || height == 0 {
        return;
    }
    let radius = radius.clamp(0, (width / 2).min(height / 2));
    if radius == 0 {
        fill_rect(hdc, rect, rgb);
        return;
    }
    if !ensure_gdiplus_started() {
        fill_round_rect_gdi(hdc, rect, rgb, radius * 2);
        return;
    }
    unsafe {
        let mut graphics: *mut GdiPlus::GpGraphics = std::ptr::null_mut();
        if GdiPlus::GdipCreateFromHDC(hdc, &mut graphics) != GdiPlus::Ok || graphics.is_null() {
            fill_round_rect_gdi(hdc, rect, rgb, radius * 2);
            return;
        }
        let _ = GdiPlus::GdipSetSmoothingMode(graphics, GdiPlus::SmoothingModeAntiAlias);
        let mut path: *mut GdiPlus::GpPath = std::ptr::null_mut();
        if GdiPlus::GdipCreatePath(GdiPlus::FillModeAlternate, &mut path) == GdiPlus::Ok
            && !path.is_null()
        {
            let (left, top) = (rect.left as f32, rect.top as f32);
            let (right, bottom) = (rect.right as f32, rect.bottom as f32);
            let diameter = (radius * 2) as f32;
            // Quarter arcs at the four corners; GDI+ connects consecutive
            // figure segments (and the close) with straight edges.
            let _ = GdiPlus::GdipAddPathArc(path, left, top, diameter, diameter, 180.0, 90.0);
            let _ =
                GdiPlus::GdipAddPathArc(path, right - diameter, top, diameter, diameter, 270.0, 90.0);
            let _ = GdiPlus::GdipAddPathArc(
                path,
                right - diameter,
                bottom - diameter,
                diameter,
                diameter,
                0.0,
                90.0,
            );
            let _ =
                GdiPlus::GdipAddPathArc(path, left, bottom - diameter, diameter, diameter, 90.0, 90.0);
            let _ = GdiPlus::GdipClosePathFigure(path);
            let mut brush: *mut GdiPlus::GpSolidFill = std::ptr::null_mut();
            if GdiPlus::GdipCreateSolidFill(0xff00_0000 | rgb, &mut brush) == GdiPlus::Ok
                && !brush.is_null()
            {
                let _ = GdiPlus::GdipFillPath(graphics, brush.cast(), path);
                let _ = GdiPlus::GdipDeleteBrush(brush.cast());
            }
            let _ = GdiPlus::GdipDeletePath(path);
        }
        let _ = GdiPlus::GdipDeleteGraphics(graphics);
    }
}

/// Aliased GDI rounded fill, kept only as the fallback when GDI+ is
/// unavailable. `corner_diameter` follows `RoundRect`'s ellipse semantics
/// (twice the corner radius).
fn fill_round_rect_gdi(hdc: HDC, rect: RECT, rgb: u32, corner_diameter: i32) {
    if rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }
    unsafe {
        let brush = CreateSolidBrush(rgb_to_colorref(rgb));
        if brush.is_invalid() {
            return;
        }
        let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
        let pen = GetStockObject(NULL_PEN);
        let old_pen = SelectObject(hdc, pen);
        let _ = RoundRect(
            hdc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            corner_diameter,
            corner_diameter,
        );
        if !old_pen.is_invalid() {
            let _ = SelectObject(hdc, old_pen);
        }
        if !old_brush.is_invalid() {
            let _ = SelectObject(hdc, old_brush);
        }
        let _ = DeleteObject(HGDIOBJ(brush.0));
    }
}

pub(super) fn centered_icon_rect(rect: RECT, size: i32) -> RECT {
    let left = rect.left + (rect_width(&rect) - size).max(0) / 2;
    let top = rect.top + (rect_height(&rect) - size).max(0) / 2;
    normalize_rect(RECT {
        left,
        top,
        right: left + size,
        bottom: top + size,
    })
}

pub(super) fn draw_icon_from_path(hdc: HDC, path: &str, rect: RECT, size: u32) -> bool {
    let Some(handle) = cached_png_icon_handle(path, size) else {
        return false;
    };
    unsafe {
        WindowsAndMessaging::DrawIconEx(
            hdc,
            rect.left,
            rect.top,
            HICON(handle as *mut c_void),
            rect_width(&rect),
            rect_height(&rect),
            0,
            None,
            WindowsAndMessaging::DI_NORMAL,
        )
        .is_ok()
    }
}

pub(super) fn rgb_to_colorref(rgb: u32) -> COLORREF {
    let r = (rgb >> 16) & 0xff;
    let g = (rgb >> 8) & 0xff;
    let b = rgb & 0xff;
    COLORREF(r | (g << 8) | (b << 16))
}

pub(super) fn normalize_rect(mut rect: RECT) -> RECT {
    if rect.right < rect.left {
        rect.right = rect.left;
    }
    if rect.bottom < rect.top {
        rect.bottom = rect.top;
    }
    rect
}

pub(super) fn rect_width(rect: &RECT) -> i32 {
    (rect.right - rect.left).max(0)
}

pub(super) fn rect_height(rect: &RECT) -> i32 {
    (rect.bottom - rect.top).max(0)
}

pub(super) fn rect_contains(rect: &RECT, point: (i32, i32)) -> bool {
    point.0 >= rect.left && point.0 < rect.right && point.1 >= rect.top && point.1 < rect.bottom
}
