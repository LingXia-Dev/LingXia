//! Shell top bar, address capsule, navigation bar, and caption buttons.

use std::collections::HashMap;
use std::sync::Mutex;

use super::*;

/// Whether the layout carries a visible browser address bar (which then
/// owns the top bar; the lxapp navigation bar yields).
pub(super) fn address_bar_visible(layout: &WindowsWindowLayout) -> bool {
    layout
        .address_bar
        .as_ref()
        .is_some_and(|address_bar| address_bar.visible)
}

/// Interactive controls of the shell top bar: the sidebar toggle at the
/// leading edge and while a browser surface is presented the back/
/// forward/reload cluster left of the centered URL capsule. Shared between
/// drawing and hit-testing so both always agree.
pub(super) struct TopBarControls {
    pub(super) sidebar_toggle: Option<RECT>,
    pub(super) nav_back: Option<RECT>,
    pub(super) nav_forward: Option<RECT>,
    pub(super) nav_reload: Option<RECT>,
    /// The URL capsule (also the inline address-edit anchor).
    pub(super) address: Option<RECT>,
}

pub(super) fn top_bar_controls(
    client: RECT,
    top_bar: RECT,
    layout: &WindowsWindowLayout,
) -> TopBarControls {
    let button_top = top_bar.top + (rect_height(&top_bar) - TOP_BAR_BUTTON_SIZE).max(0) / 2;
    let square_button = |left: i32| {
        normalize_rect(RECT {
            left,
            top: button_top,
            right: left + TOP_BAR_BUTTON_SIZE,
            bottom: button_top + TOP_BAR_BUTTON_SIZE,
        })
    };

    // Sidebar toggle: only products with a sidebar tab bar get one. It is
    // intentionally independent of the collapsed flag (it must stay
    // clickable to re-expand a collapsed sidebar) and sits at the window's
    // leading edge: inside the sidebar column while the sidebar is
    // expanded, over the top bar's leading edge while it is collapsed
    // (`navbar_buttons_left` shifts the lxapp navbar buttons clear of it).
    let sidebar_toggle = layout
        .tab_bar
        .as_ref()
        .filter(|tabbar| {
            tabbar.visible
                && matches!(
                    tabbar.position,
                    WindowsTabBarPosition::Left | WindowsTabBarPosition::Right
                )
        })
        .map(|_| square_button(client.left + TOP_BAR_PADDING));
    let mut left_edge = top_bar.left + TOP_BAR_PADDING;
    if let Some(toggle) = &sidebar_toggle {
        left_edge = left_edge.max(toggle.right + TOP_BAR_BUTTON_GAP);
    }

    let mut controls = TopBarControls {
        sidebar_toggle,
        nav_back: None,
        nav_forward: None,
        nav_reload: None,
        address: None,
    };
    if !address_bar_visible(layout) {
        return controls;
    }

    // The frame buttons own the client's trailing edge; everything between
    // the toggle and them is available to the address section.
    let right_edge = (client.right - window_frame_buttons_width() - TOP_BAR_PADDING)
        .min(top_bar.right - TOP_BAR_PADDING);
    let nav_width = 3 * TOP_BAR_BUTTON_SIZE + 2 * TOP_BAR_BUTTON_GAP;
    let capsule_space = right_edge - left_edge - nav_width - ADDRESS_CAPSULE_NAV_GAP;
    if capsule_space < 48 {
        return controls;
    }

    // The nav cluster anchors at the content (webview region) leading edge,
    // like Arc; the capsule centers in the space that remains.
    let nav_left = left_edge;
    controls.nav_back = Some(square_button(nav_left));
    controls.nav_forward = Some(square_button(
        nav_left + TOP_BAR_BUTTON_SIZE + TOP_BAR_BUTTON_GAP,
    ));
    controls.nav_reload = Some(square_button(
        nav_left + 2 * (TOP_BAR_BUTTON_SIZE + TOP_BAR_BUTTON_GAP),
    ));

    let capsule_min_left = nav_left + nav_width + ADDRESS_CAPSULE_NAV_GAP;
    let capsule_width = capsule_space.min(ADDRESS_CAPSULE_MAX_WIDTH);
    let capsule_height = ADDRESS_CAPSULE_HEIGHT.min(rect_height(&top_bar));
    let capsule_top = top_bar.top + (rect_height(&top_bar) - capsule_height).max(0) / 2;
    // Center the capsule between the nav cluster and the frame buttons,
    // clamped so it never runs under either.
    let centered_left = (capsule_min_left + right_edge - capsule_width) / 2;
    let capsule_left = centered_left
        .max(capsule_min_left)
        .min(right_edge - capsule_width);
    controls.address = Some(normalize_rect(RECT {
        left: capsule_left,
        top: capsule_top,
        right: capsule_left + capsule_width,
        bottom: capsule_top + capsule_height,
    }));
    controls
}

/// Last painted URL-capsule rect per host window, so the facade can start
/// an inline address edit (EDIT child) over the capsule; same pattern as
/// the terminal tab-title rects in `terminal_grid`.
static ADDRESS_CAPSULE_RECTS: OnceLock<Mutex<HashMap<isize, RECT>>> = OnceLock::new();

fn remember_address_capsule_rect(hwnd: HWND, rect: Option<RECT>) {
    let rects = ADDRESS_CAPSULE_RECTS.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut rects) = rects.lock() else {
        return;
    };
    match rect {
        Some(rect) => {
            rects.insert(hwnd.0 as isize, rect);
        }
        None => {
            rects.remove(&(hwnd.0 as isize));
        }
    }
}

/// Starts an inline URL edit over the address capsule last painted in
/// `window`'s top bar, prefilled with `initial_text` (selected). Safe to
/// call from any thread; the editor is marshalled onto the window's UI
/// thread (see [`super::super::text_input`] for lifecycle). `on_commit`
/// receives the submitted text on Enter/focus loss; Esc cancels. Returns
/// `false` when no address capsule has been painted for `window`.
pub fn begin_address_edit(
    window: isize,
    initial_text: &str,
    on_commit: super::super::text_input::InlineEditCommit,
) -> bool {
    let capsule = ADDRESS_CAPSULE_RECTS
        .get()
        .and_then(|rects| rects.lock().ok())
        .and_then(|rects| rects.get(&window).copied());
    let Some(capsule) = capsule else {
        return false;
    };
    // The editor sits inside the capsule (white EDIT on the white capsule
    // fill), inset past the rounded ends.
    let edit_rect = inset_rect(capsule, ADDRESS_CAPSULE_HEIGHT / 2, 4);
    if rect_width(&edit_rect) == 0 || rect_height(&edit_rect) == 0 {
        return false;
    }
    let initial = initial_text.to_string();
    post_to_window_thread(
        window,
        Box::new(move || {
            super::super::text_input::begin_inline_edit(
                HWND(window as *mut c_void),
                edit_rect,
                &initial,
                on_commit,
            );
        }),
    )
}

pub(super) fn draw_shell_top_bar(hdc: HDC, rects: &ChromeRects) {
    fill_rect(hdc, rects.top_bar, SHELL_WINDOW_BACKGROUND);
    draw_bottom_border(hdc, rects.top_bar, SHELL_DIVIDER);
}

/// Draws the interactive top-bar controls (sidebar toggle, browser nav
/// buttons, URL capsule) and records the capsule rect for the inline
/// address editor. Painted after the navigation bar, which fills the top
/// bar with its own background.
pub(super) fn draw_top_bar_controls(hdc: HDC, state: &WindowsChromeState, rects: &ChromeRects) {
    let layout = &state.layout;
    let controls = top_bar_controls(state.client, rects.top_bar, layout);
    if let Some(toggle) = controls.sidebar_toggle {
        draw_frame_button_glyph(hdc, GLYPH_SIDEBAR_TOGGLE, toggle, SHELL_FRAME_BUTTON_ICON);
    }
    if let Some(back) = controls.nav_back {
        draw_frame_button_glyph(hdc, GLYPH_NAV_BACK, back, SHELL_FRAME_BUTTON_ICON);
    }
    if let Some(forward) = controls.nav_forward {
        draw_frame_button_glyph(hdc, GLYPH_NAV_FORWARD, forward, SHELL_FRAME_BUTTON_ICON);
    }
    if let Some(reload) = controls.nav_reload {
        draw_frame_button_glyph(hdc, GLYPH_NAV_RELOAD, reload, SHELL_FRAME_BUTTON_ICON);
    }
    if let Some(address) = controls.address {
        // White capsule on the gray caption strip; anti-alias the arc.
        fill_round_rect_aa(
            hdc,
            address,
            rect_height(&address) / 2,
            SHELL_PANEL_BACKGROUND,
        );
        let text = layout
            .address_bar
            .as_ref()
            .map(|address_bar| address_bar.url_text.as_str())
            .unwrap_or_default();
        draw_text(
            hdc,
            text,
            inset_rect(address, ADDRESS_CAPSULE_HEIGHT / 2, 0),
            SHELL_TEXT_PRIMARY,
            DT_CENTER,
        );
    }
    remember_address_capsule_rect(state.hwnd, controls.address);
}

pub(super) fn draw_navigation_bar(
    hdc: HDC,
    rect: RECT,
    buttons_left: i32,
    navbar: &WindowsNavigationBarLayout,
) {
    fill_rect(hdc, rect, navbar.background_color);
    draw_bottom_border(hdc, rect, 0xe6e6e6);

    let text_color = navbar.text_color;
    let mut left_controls_width = 0;

    if navbar.show_back_button {
        let back_rect = nav_button_rect(rect, buttons_left, 0);
        draw_frame_button_glyph(hdc, GLYPH_NAV_BACK, back_rect, text_color);
        left_controls_width = back_rect.right - rect.left;
    }
    if navbar.show_home_button {
        let home_rect = nav_button_rect(
            rect,
            buttons_left,
            if navbar.show_back_button { 1 } else { 0 },
        );
        draw_frame_button_glyph(hdc, GLYPH_NAV_HOME, home_rect, text_color);
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
/// and system hover/pressed states; the close button turns system red with
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
            Some(darken_rgb(
                SHELL_WINDOW_BACKGROUND,
                FRAME_BUTTON_HOVER_OVERLAY,
            ))
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
        let _ = DrawTextW(
            hdc,
            &mut wide,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
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

/// Leading x of the navigation bar's back/home buttons: just right of the
/// top-bar sidebar toggle when one is shown (so they never overlap), else
/// the navbar's own inset.
pub(super) fn navbar_buttons_left(
    client: RECT,
    top_bar: RECT,
    layout: &WindowsWindowLayout,
    navbar_rect: RECT,
) -> i32 {
    match top_bar_controls(client, top_bar, layout).sidebar_toggle {
        Some(toggle) => (navbar_rect.left + 8).max(toggle.right + TOP_BAR_BUTTON_GAP),
        None => navbar_rect.left + 8,
    }
}

pub(super) fn nav_button_rect(navbar: RECT, buttons_left: i32, index: i32) -> RECT {
    let width = 44;
    RECT {
        left: buttons_left + index * width,
        top: navbar.top,
        right: buttons_left + (index + 1) * width,
        bottom: navbar.bottom,
    }
}
