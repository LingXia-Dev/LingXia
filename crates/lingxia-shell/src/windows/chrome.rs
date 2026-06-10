//! Shell window chrome: chrome rect computation, all GDI drawing (top bar,
//! tab bar, sidebar, navigation bar, panels, text, colors), and hit-testing.
//!
//! Moved out of `lingxia-webview` so the webview crate stays generic; this
//! file is pure product policy registered through the
//! [`WindowsChromeRenderer`] seam.

use std::ffi::c_void;
use std::sync::Arc;

use lingxia_webview::platform::windows::{
    WindowsChromeHit, WindowsChromePanel, WindowsChromeRenderer, WindowsChromeState,
    WindowsFrameButton, WindowsNativePanelContent, WindowsNativePanelKind,
    WindowsNavigationBarLayout, WindowsTabBarLayout, WindowsTabBarPosition, WindowsWindowLayout,
    cached_png_icon_handle, set_windows_chrome_renderer,
};
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreatePen, CreateSolidBrush,
    DEFAULT_CHARSET, DEFAULT_PITCH, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_SINGLELINE, DT_VCENTER,
    DeleteObject, DrawTextW, FF_SWISS, FillRect, GetDeviceCaps, GetStockObject, HDC, HGDIOBJ,
    LOGPIXELSY, LineTo, MoveToEx, NULL_PEN, OUT_DEFAULT_PRECIS, PS_SOLID, RoundRect, SelectObject,
    SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{self, HICON};
use windows::core::w;

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

pub(super) const SHELL_TERMINAL_BACKGROUND: u32 = 0x111827;

pub(super) const SHELL_TERMINAL_TEXT: u32 = 0xe5e7eb;

pub(super) const SHELL_SIDEBAR_HEADER_TEXT: u32 = 0x4f5661;

pub(super) const SHELL_TAB_SELECTED_BACKGROUND: u32 = 0xf3f7ff;

pub(super) const SHELL_TOP_BAR_HEIGHT: i32 = 38;

pub(super) const WINDOW_BUTTON_WIDTH: i32 = 46;

pub(super) const WINDOW_BUTTON_ICON_SIZE: i32 = 10;

pub(super) const SHELL_SIDEBAR_WIDTH: i32 = 180;

pub(super) const SIDEBAR_HEADER_HEIGHT: i32 = 66;

pub(super) const SIDEBAR_ITEM_HEIGHT: i32 = 34;

pub(super) const SIDEBAR_ITEM_GAP: i32 = 4;

pub(super) const SIDEBAR_ITEM_INSET: i32 = 10;

pub(super) const SIDEBAR_FOOTER_HEIGHT: i32 = 46;

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

    fn paint(&self, hdc: HDC, state: &WindowsChromeState) {
        draw_window_chrome(hdc, state);
    }

    fn hit_test(&self, state: &WindowsChromeState, point: (i32, i32)) -> Option<WindowsChromeHit> {
        chrome_hit_test(state, point)
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
    draw_window_frame_buttons(hdc, state.hwnd, client);
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
        for index in 0..tabbar.items.len() {
            let item_rect = if matches!(
                tabbar.position,
                WindowsTabBarPosition::Left | WindowsTabBarPosition::Right
            ) {
                sidebar_item_rect(tabbar_rect, index)
            } else {
                tab_item_rect(tabbar_rect, tabbar.position, tabbar.items.len(), index)
            };
            if rect_contains(&item_rect, point) {
                return Some(WindowsChromeHit::TabBarItem { index });
            }
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
        for panel in &attached.panels {
            draw_content_card(hdc, panel.rect);
        }
        for panel in &attached.panels {
            if panel.native.is_some() {
                draw_native_panel_content(hdc, panel);
            }
        }
        return;
    }

    draw_content_card(hdc, rects.panel);
}

pub(super) fn draw_content_card(hdc: HDC, rect: RECT) {
    if rect_width(&rect) > 0 && rect_height(&rect) > 0 {
        fill_round_rect(hdc, rect, SHELL_PANEL_BACKGROUND, SHELL_PANEL_RADIUS);
    }
}

pub(super) fn draw_native_panel_content(hdc: HDC, panel: &WindowsChromePanel) {
    let Some(native) = &panel.native else {
        return;
    };
    if native.kind == WindowsNativePanelKind::Terminal {
        draw_terminal_panel_content(hdc, panel.rect, native);
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

pub(super) fn draw_terminal_panel_content(
    hdc: HDC,
    rect: RECT,
    native: &WindowsNativePanelContent,
) {
    let content = inset_rect(rect, 14, 14);
    let title_rect = RECT {
        left: content.left + 8,
        top: content.top,
        right: content.right,
        bottom: content.top + 24,
    };
    draw_text(
        hdc,
        native.title.as_deref().unwrap_or("Terminal"),
        title_rect,
        SHELL_TEXT_PRIMARY,
        DT_LEFT,
    );

    let terminal_rect = normalize_rect(RECT {
        left: content.left,
        top: title_rect.bottom + 8,
        right: content.right,
        bottom: content.bottom,
    });
    if rect_width(&terminal_rect) == 0 || rect_height(&terminal_rect) == 0 {
        return;
    }

    fill_round_rect(hdc, terminal_rect, SHELL_TERMINAL_BACKGROUND, 8);
    let text_rect = inset_rect(terminal_rect, 12, 10);
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

pub(super) fn draw_window_frame_buttons(hdc: HDC, hwnd: HWND, client: RECT) {
    let text_color = SHELL_FRAME_BUTTON_ICON;
    for (button, rect) in window_frame_button_rects(client) {
        match button {
            WindowsFrameButton::Minimize => {
                let y = rect.top + rect_height(&rect) / 2 + 5;
                draw_line(
                    hdc,
                    rect.left + (rect_width(&rect) - WINDOW_BUTTON_ICON_SIZE) / 2,
                    y,
                    rect.left + (rect_width(&rect) + WINDOW_BUTTON_ICON_SIZE) / 2,
                    y,
                    text_color,
                );
            }
            WindowsFrameButton::Maximize => {
                let size = WINDOW_BUTTON_ICON_SIZE;
                let left = rect.left + (rect_width(&rect) - size) / 2;
                let top = rect.top + (rect_height(&rect) - size) / 2;
                if unsafe { WindowsAndMessaging::IsZoomed(hwnd).as_bool() } {
                    draw_rect_outline(
                        hdc,
                        RECT {
                            left: left + 3,
                            top,
                            right: left + 3 + size - 2,
                            bottom: top + size - 2,
                        },
                        text_color,
                    );
                    draw_rect_outline(
                        hdc,
                        RECT {
                            left,
                            top: top + 3,
                            right: left + size - 2,
                            bottom: top + 3 + size - 2,
                        },
                        text_color,
                    );
                } else {
                    draw_rect_outline(
                        hdc,
                        RECT {
                            left,
                            top,
                            right: left + size,
                            bottom: top + size,
                        },
                        text_color,
                    );
                }
            }
            WindowsFrameButton::Close => {
                let size = WINDOW_BUTTON_ICON_SIZE;
                let left = rect.left + (rect_width(&rect) - size) / 2;
                let top = rect.top + (rect_height(&rect) - size) / 2;
                draw_line(hdc, left, top, left + size, top + size, text_color);
                draw_line(hdc, left + size, top, left, top + size, text_color);
            }
        }
    }
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

pub(super) fn draw_rect_outline(hdc: HDC, rect: RECT, rgb: u32) {
    draw_line(hdc, rect.left, rect.top, rect.right, rect.top, rgb);
    draw_line(hdc, rect.right, rect.top, rect.right, rect.bottom, rgb);
    draw_line(hdc, rect.right, rect.bottom, rect.left, rect.bottom, rgb);
    draw_line(hdc, rect.left, rect.bottom, rect.left, rect.top, rgb);
}

pub(super) fn draw_line(hdc: HDC, x1: i32, y1: i32, x2: i32, y2: i32, rgb: u32) {
    unsafe {
        let pen = CreatePen(PS_SOLID, 1, rgb_to_colorref(rgb));
        if pen.is_invalid() {
            return;
        }
        let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
        let _ = MoveToEx(hdc, x1, y1, None);
        let _ = LineTo(hdc, x2, y2);
        if !old_pen.is_invalid() {
            let _ = SelectObject(hdc, old_pen);
        }
        let _ = DeleteObject(HGDIOBJ(pen.0));
    }
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
            fill_round_rect(hdc, item_rect, 0xffffff, 8);
            fill_round_rect(
                hdc,
                RECT {
                    left: item_rect.left + 6,
                    top: item_rect.top + 9,
                    right: item_rect.left + 10,
                    bottom: item_rect.bottom - 9,
                },
                tabbar.selected_color,
                3,
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
            fill_round_rect(hdc, rect, 0xffffff, 6);
            fill_round_rect(
                hdc,
                RECT {
                    left: rect.left + 3,
                    top: rect.bottom - 5,
                    right: rect.right - 3,
                    bottom: rect.bottom - 3,
                },
                SHELL_ACCENT,
                2,
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
    unsafe {
        let font = CreateFontW(
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
        );
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

pub(super) fn fill_round_rect(hdc: HDC, rect: RECT, rgb: u32, radius: i32) {
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
            radius,
            radius,
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
