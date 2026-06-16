//! Native panel chrome, including terminal panel headers and body drawing.

use super::*;

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
            draw_native_panel_content(hdc, state.hwnd, panel);
        }
    }
}

pub(super) fn draw_native_panel_content(hdc: HDC, hwnd: HWND, panel: &WindowsChromePanel) {
    let Some(native) = &panel.host_content else {
        return;
    };
    draw_terminal_panel_content(hdc, hwnd, panel, native);
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
/// Docked panels keep square top corners (flush seam with the main card);
/// while maximized the panel is the whole content area and is rounded all
/// around.
pub(super) fn draw_terminal_panel_content(
    hdc: HDC,
    hwnd: HWND,
    panel: &WindowsChromePanel,
    native: &WindowsHostPanelContent,
) {
    let rect = panel.rect;
    if rect_width(&rect) == 0 || rect_height(&rect) == 0 {
        return;
    }
    let surface = super::super::terminal_grid::panel_surface_background(&panel.panel_id)
        .unwrap_or(TERMINAL_SURFACE_BACKGROUND);
    let square_top = panel.docked && !native.maximized;

    // Surface card: dark terminal surface on the light window background.
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
            // Surface-on-header contrast: anti-alias the pill arc.
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
            draw_text(
                hdc,
                GLYPH_TAB_CLOSE,
                close,
                TERMINAL_HEADER_TEXT_MUTED,
                DT_CENTER,
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
            GLYPH_PANEL_SHRINK
        } else {
            GLYPH_PANEL_EXPAND
        };
        draw_frame_button_glyph(hdc, glyph, maximize, TERMINAL_HEADER_TEXT_MUTED);
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

    // Live sessions are drawn as a cell grid from the snapshot store; the
    // body-text path below remains for pre-session states ("Starting
    // terminal...", runtime-unavailable, failures).
    if super::super::terminal_grid::draw_panel_grid(hdc, &panel.panel_id, body) {
        return;
    }

    let text_rect = inset_rect(body, 12, 10);
    // Line advance with leading: the glyph cell alone clips descenders
    // when DrawText clamps to the per-line rect.
    let line_height = (logical_font_height(hdc, 10).max(13) * 4 + 2) / 3;
    let max_lines = (rect_height(&text_rect) / line_height).max(1) as usize;
    let snapshot_body = super::super::terminal_grid::panel_snapshot_text(&panel.panel_id);
    let body = snapshot_body
        .as_deref()
        .filter(|body| !body.trim().is_empty())
        .or_else(|| {
            native
                .body
                .as_deref()
                .filter(|body| !body.trim().is_empty())
        })
        .unwrap_or("Terminal session is waiting for output");

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
