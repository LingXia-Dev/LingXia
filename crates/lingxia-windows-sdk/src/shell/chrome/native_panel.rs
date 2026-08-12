//! Native panel chrome, including terminal panel headers and body drawing.

use super::*;
use windows::Win32::Graphics::Gdi::{RestoreDC, SaveDC};

/// Restores the DC's saved state (clip region included) on drop, so the
/// rounded-card clip cannot leak past this panel's painting even through the
/// early returns.
struct DcClipGuard {
    hdc: HDC,
    saved: i32,
}

impl DcClipGuard {
    fn save(hdc: HDC) -> Self {
        Self {
            hdc,
            saved: unsafe { SaveDC(hdc) },
        }
    }
}

impl Drop for DcClipGuard {
    fn drop(&mut self) {
        if self.saved != 0 {
            unsafe {
                let _ = RestoreDC(self.hdc, self.saved);
            }
        }
    }
}

pub(super) fn panel_is_maximized(panel: &WindowsChromePanel) -> bool {
    panel
        .host_content
        .as_ref()
        .is_some_and(|native| native.maximized)
}

/// Painted as a shell overlay: an expanded native panel covers the app UI,
/// including sidebar/tabbar chrome. Window frame buttons are drawn afterwards.
pub(super) fn draw_maximized_native_panels(hdc: HDC, state: &WindowsChromeState) {
    let Some(attached) = &state.attached else {
        return;
    };
    for panel in &attached.panels {
        if panel.host_content.is_some() && panel_is_maximized(panel) {
            draw_native_panel_content(hdc, state.hwnd, state.client, panel, state.cursor);
        }
    }
}

pub(super) fn draw_native_panel_content(
    hdc: HDC,
    hwnd: HWND,
    client: RECT,
    panel: &WindowsChromePanel,
    cursor: Option<(i32, i32)>,
) {
    let Some(native) = &panel.host_content else {
        return;
    };
    draw_terminal_panel_content(hdc, hwnd, client, panel, native, cursor);
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
    native: &WindowsHostPanelContent,
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
    let maximize =
        (native.show_maximize && maximize_left > header.left).then(|| square_button(maximize_left));
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
        let tab_width = (avail / count).clamp(24, TERMINAL_TAB_MAX_WIDTH);
        for item in &native.tabs {
            let tab_rect = normalize_rect(RECT {
                left,
                top: header.top + TERMINAL_TAB_TOP_INSET,
                right: (left + tab_width).min(tabs_right_limit),
                bottom: header.bottom,
            });
            // Every tab wide enough gets a close glyph (macOS tab-rail
            // parity); narrow tabs keep the full rect clickable.
            let close = (rect_width(&tab_rect) >= 3 * TERMINAL_TAB_CLOSE_WIDTH).then(|| {
                normalize_rect(RECT {
                    left: tab_rect.right - TERMINAL_TAB_CLOSE_WIDTH,
                    top: tab_rect.top,
                    right: tab_rect.right,
                    bottom: tab_rect.bottom,
                })
            });
            let title = normalize_rect(RECT {
                left: tab_rect.left + 14,
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
    let native = panel.host_content.as_ref()?;
    let rects = terminal_header_rects(panel.rect, native);
    if !rect_contains(&rects.header, point) {
        return None;
    }
    if let Some(maximize) = rects.maximize
        && rect_contains(&maximize, point)
    {
        return Some(chrome_command(
            command_id::NATIVE_PANEL_MAXIMIZE,
            serde_json::json!({ "panel_id": panel.panel_id.clone() }),
        ));
    }
    if let Some(new_tab) = rects.new_tab
        && rect_contains(&new_tab, point)
    {
        return Some(chrome_command(
            command_id::NATIVE_PANEL_NEW_TAB,
            serde_json::json!({ "panel_id": panel.panel_id.clone() }),
        ));
    }
    for tab in &rects.tabs {
        if let Some(close) = tab.close
            && rect_contains(&close, point)
        {
            return Some(chrome_command(
                command_id::NATIVE_PANEL_TAB_CLOSE,
                serde_json::json!({ "panel_id": panel.panel_id.clone(), "tab_id": tab.tab_id }),
            ));
        }
        if rect_contains(&tab.rect, point) {
            let click = WindowsChromeCommand::new(command_id::NATIVE_PANEL_TAB_CLICK)
                .with_payload(serde_json::json!({
                    "panel_id": panel.panel_id.clone(),
                    "tab_id": tab.tab_id
                }))
                .with_focus(panel.panel_id.clone());
            let active = native
                .tabs
                .iter()
                .any(|native_tab| native_tab.id == tab.tab_id && native_tab.active);
            let command = if active {
                click.with_double_click(
                    WindowsChromeCommand::new(command_id::NATIVE_PANEL_TAB_RENAME)
                        .with_payload(serde_json::json!({
                            "panel_id": panel.panel_id.clone(),
                            "tab_id": tab.tab_id
                        }))
                        .with_focus(panel.panel_id.clone()),
                )
            } else {
                click
            };
            return Some(WindowsChromeHit::Command(command));
        }
    }
    None
}

/// Draws a terminal panel as a compact dock: full-bleed surface card, a
/// 34px header strip (tabs + new-tab + maximize), and the cell grid below.
/// Docked, floating, and maximized panels all draw the same rounded card
/// as the webview content.
pub(super) fn draw_terminal_panel_content(
    hdc: HDC,
    hwnd: HWND,
    client: RECT,
    panel: &WindowsChromePanel,
    native: &WindowsHostPanelContent,
    cursor: Option<(i32, i32)>,
) {
    let rect = panel.rect;
    if rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }
    let _ = client;
    // The strip is the terminal's own — its `+` opens another PTY — so it is
    // tinted from the scheme in effect rather than fixed. One rule, shared
    // with the Apple host, so a theme change moves the whole card.
    let chrome = super::super::terminal_grid::surface_chrome();
    let surface = super::super::terminal_panel::focused_session(&panel.panel_id)
        .and_then(super::super::terminal_grid::session_surface_background)
        .unwrap_or(chrome.surface);

    // Card: ONE rounded path, filled in two colors split at the header's
    // bottom edge. Each fill is clipped by a plain horizontal band (straight
    // clip edges alias nothing), so every arc pixel is anti-aliased exactly
    // once in its final color — a second rounded fill over the same arc
    // re-blends it (the old header-over-card fringe), and an aliased rounded
    // clip shows a staircase wherever the inner color differs from the card.
    // Maximized panels keep the same rounded card, drawn over the workspace.
    // Radius and shadow match the webview content cards so the workspace
    // reads as one system of cards.
    let header_rects = terminal_header_rects(rect, native);
    let header = header_rects.header;
    let _clip_guard = DcClipGuard::save(hdc);
    draw_content_card_shadow(hdc, rect);
    fill_round_rect_aa_band(
        hdc,
        rect,
        SHELL_CONTENT_RADIUS,
        chrome.header,
        rect.top,
        header.bottom,
    );
    fill_round_rect_aa_band(
        hdc,
        rect,
        SHELL_CONTENT_RADIUS,
        surface,
        header.bottom,
        rect.bottom,
    );
    // Body drawing below (pane fills, grid) stays clipped to the card's
    // interior so square fills cannot overpaint the bottom arcs. The clip
    // boundary is aliased, but everything drawn inside matches the card's
    // surface color there, so it stays invisible.
    clip_to_round_rect_inside(hdc, rect, SHELL_CONTENT_RADIUS);

    // Hairline under the tab strip; the active tab paints over it, so it
    // visually connects into the surface (macOS rail parity).
    fill_rect(
        hdc,
        RECT {
            left: rect.left,
            top: header.bottom - 1,
            right: rect.right,
            bottom: header.bottom,
        },
        chrome.separator,
    );

    for tab in &header_rects.tabs {
        if tab.active {
            // The active tab flows into the surface below it: surface
            // fill, rounded on top, square at the header's bottom edge
            // (macOS tab-rail shape). A faint inner highlight along the
            // top edge lifts the pill off the darker chrome.
            fill_round_rect_aa_corners(
                hdc,
                tab.rect,
                [TERMINAL_TAB_RADIUS, TERMINAL_TAB_RADIUS, 0, 0],
                surface,
            );
            fill_rect(
                hdc,
                RECT {
                    left: tab.rect.left + TERMINAL_TAB_RADIUS + 2,
                    top: tab.rect.top + 1,
                    right: tab.rect.right - TERMINAL_TAB_RADIUS - 2,
                    bottom: tab.rect.top + 2,
                },
                blend_rgb(chrome.text, surface, 8),
            );
        } else {
            // Hairline separator at the trailing edge of inactive tabs.
            let inset = rect_height(&tab.rect) / 3;
            fill_rect(
                hdc,
                RECT {
                    left: tab.rect.right + TERMINAL_TAB_GAP / 2,
                    top: tab.rect.top + inset,
                    right: tab.rect.right + TERMINAL_TAB_GAP / 2 + 1,
                    bottom: tab.rect.bottom - inset,
                },
                blend_rgb(chrome.text, chrome.header, 8),
            );
        }
        let title = native
            .tabs
            .iter()
            .find(|item| item.id == tab.tab_id)
            .map(|item| item.title.as_str())
            .unwrap_or_default();
        let color = if tab.active {
            chrome.text
        } else {
            chrome.text_muted
        };
        // Grayscale AA throughout the header: ClearType subpixel rendering
        // fringes text against a chrome color it knows nothing about.
        draw_text_antialiased(hdc, title, tab.title, color, DT_LEFT);
        if let Some(close) = tab.close {
            let close_color = if tab.active {
                chrome.text_muted
            } else {
                blend_rgb(chrome.text_muted, chrome.header, 55)
            };
            let size = 12.min(rect_width(&close)).min(rect_height(&close));
            let icon = normalize_rect(RECT {
                left: close.left + (rect_width(&close) - size) / 2,
                top: close.top + (rect_height(&close) - size) / 2,
                right: close.left + (rect_width(&close) - size) / 2 + size,
                bottom: close.top + (rect_height(&close) - size) / 2 + size,
            });
            crate::draw_windows_design_icon_with_color(
                hdc,
                WindowsDesignIcon::CloseX,
                icon,
                close_color,
            );
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
        let fallback_title = lingxia_logic::i18n::t(lingxia_logic::I18nKey::TerminalTitle);
        draw_text_antialiased(
            hdc,
            native.title.as_deref().unwrap_or(&fallback_title),
            title_rect,
            chrome.text,
            DT_LEFT,
        );
    }
    if let Some(new_tab) = header_rects.new_tab {
        draw_frame_button_glyph_grayscale(hdc, GLYPH_ADD, new_tab, chrome.text_muted);
    }
    if let Some(maximize) = header_rects.maximize {
        let glyph = if native.maximized {
            GLYPH_PANEL_SHRINK
        } else {
            GLYPH_PANEL_EXPAND
        };
        draw_frame_button_glyph_grayscale(hdc, glyph, maximize, chrome.text_muted);
    }

    // Record the painted tab-title rects so the facade can start an inline
    // rename (EDIT child) over the double-clicked title.
    super::super::terminal_grid::set_panel_tab_title_rects(
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

    // The body is the renderer's: a composited surface covers whatever GDI
    // drew underneath it, so the two cannot both paint it. Only the card's
    // bottom corners are its to round — the header above keeps the top two.
    super::super::terminal_gpu::present(
        hwnd,
        &panel.panel_id,
        body,
        [0, 0, SHELL_CONTENT_RADIUS, SHELL_CONTENT_RADIUS],
        cursor,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terminal_content(show_maximize: bool) -> WindowsHostPanelContent {
        WindowsHostPanelContent {
            title: Some("Terminal".to_string()),
            body: None,
            tabs: Vec::new(),
            maximized: true,
            show_maximize,
        }
    }

    #[test]
    fn main_workspace_omits_panel_zoom_control() {
        let rect = RECT {
            left: 0,
            top: 0,
            right: 800,
            bottom: 600,
        };

        assert!(
            terminal_header_rects(rect, &terminal_content(true))
                .maximize
                .is_some()
        );
        assert!(
            terminal_header_rects(rect, &terminal_content(false))
                .maximize
                .is_none()
        );
    }
}
