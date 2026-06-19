//! Shell top bar, address capsule, navigation bar, and caption buttons.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Mutex;

use crate::{WindowsDesignIcon, draw_windows_design_icon_with_color};

use super::*;

/// Whether the layout carries a visible browser address bar (which then
/// owns the top bar; the lxapp navigation bar yields).
pub(super) fn address_bar_visible(layout: &WindowsShellWindowLayout) -> bool {
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
    /// The app-menu button at the window's leading edge (left of the sidebar
    /// toggle). Hidden in the compact sidebar rail so the rail is only the
    /// expand affordance.
    pub(super) app_icon: Option<RECT>,
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
    layout: &WindowsShellWindowLayout,
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

    // Whether this product shows a sidebar (and therefore a sidebar toggle):
    // only left/right tab bars get one.
    let tabbar = layout.tab_bar.as_ref();
    let has_sidebar_toggle = tabbar.is_some_and(|tabbar| {
        tabbar.visible
            && matches!(
                tabbar.position,
                WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
            )
    });
    let compact_sidebar = tabbar.is_some_and(|tabbar| tabbar.collapsed || tabbar.icon_rail);

    // App-menu button: always shown at the window's leading edge. When a
    // sidebar exists it shares the sidebar header's leading edge with the
    // toggle (which moves to its right); otherwise it anchors the top bar.
    let app_icon_left = if has_sidebar_toggle {
        client.left + TOP_BAR_PADDING
    } else {
        top_bar.left + TOP_BAR_PADDING
    };
    let app_icon = (!compact_sidebar).then(|| square_button(app_icon_left));

    // Sidebar toggle: sits just right of the app-menu button. It is
    // intentionally independent of the collapsed flag (it must stay
    // clickable to re-expand a collapsed sidebar) and shares the leading
    // edge: inside the sidebar column while the sidebar is expanded, over
    // the top bar's leading edge while it is collapsed (`navbar_buttons_left`
    // shifts the lxapp navbar buttons clear of it).
    let sidebar_toggle = has_sidebar_toggle.then(|| {
        if compact_sidebar {
            let left = match tabbar.map(|tabbar| tabbar.position) {
                Some(WindowsShellTabBarPosition::Right) => {
                    client.right - SHELL_SIDEBAR_RAIL_WIDTH
                        + (SHELL_SIDEBAR_RAIL_WIDTH - TOP_BAR_BUTTON_SIZE).max(0) / 2
                }
                _ => client.left + (SHELL_SIDEBAR_RAIL_WIDTH - TOP_BAR_BUTTON_SIZE).max(0) / 2,
            };
            square_button(left)
        } else {
            let app_right = app_icon.map(|rect| rect.right).unwrap_or(app_icon_left);
            square_button(app_right + TOP_BAR_BUTTON_GAP)
        }
    });
    let mut left_edge = top_bar.left + TOP_BAR_PADDING;
    if let Some(app_icon) = app_icon {
        left_edge = left_edge.max(app_icon.right + TOP_BAR_BUTTON_GAP);
    }
    if let Some(toggle) = &sidebar_toggle {
        left_edge = left_edge.max(toggle.right + TOP_BAR_BUTTON_GAP);
    }

    let mut controls = TopBarControls {
        app_icon,
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
pub(super) fn draw_top_bar_controls(
    hdc: HDC,
    state: &WindowsChromeState,
    rects: &ChromeRects,
    layout: &WindowsShellWindowLayout,
) {
    let controls = top_bar_controls(state.client, rects.top_bar, layout);
    if let Some(app_icon) = controls.app_icon {
        draw_app_menu_icon(hdc, app_icon, &layout.app_icon_path);
    }
    if let Some(toggle) = controls.sidebar_toggle {
        let icon = layout
            .tab_bar
            .as_ref()
            .map(|tabbar| {
                if tabbar.collapsed || tabbar.icon_rail {
                    WindowsDesignIcon::SidebarExpand
                } else {
                    WindowsDesignIcon::SidebarCollapse
                }
            })
            .unwrap_or(WindowsDesignIcon::SidebarCollapse);
        draw_design_icon_button(hdc, toggle, icon, SHELL_FRAME_BUTTON_ICON, 18);
    }
    if let Some(back) = controls.nav_back {
        draw_design_icon_button(
            hdc,
            back,
            WindowsDesignIcon::Back,
            SHELL_FRAME_BUTTON_ICON,
            18,
        );
    }
    if let Some(forward) = controls.nav_forward {
        draw_design_icon_button(
            hdc,
            forward,
            WindowsDesignIcon::Forward,
            SHELL_FRAME_BUTTON_ICON,
            18,
        );
    }
    if let Some(reload) = controls.nav_reload {
        draw_design_icon_button(
            hdc,
            reload,
            WindowsDesignIcon::BrowserRefresh,
            SHELL_FRAME_BUTTON_ICON,
            18,
        );
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
    navbar: &WindowsShellNavigationBarLayout,
) {
    fill_rect(hdc, rect, navbar.background_color);
    draw_bottom_border(hdc, rect, 0xe6e6e6);

    let text_color = navbar.text_color;
    let mut left_controls_width = 0;

    if navbar.show_back_button {
        let back_rect = nav_button_rect(rect, buttons_left, 0);
        draw_design_icon_button(hdc, back_rect, WindowsDesignIcon::Back, text_color, 22);
        left_controls_width = back_rect.right - rect.left;
    }
    if navbar.show_home_button {
        let home_rect = nav_button_rect(
            rect,
            buttons_left,
            if navbar.show_back_button { 1 } else { 0 },
        );
        draw_design_icon_button(hdc, home_rect, WindowsDesignIcon::Home, text_color, 22);
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

/// Draws the app-menu button: the app's own (clean) icon when it declares
/// one — the brand mark at the window's leading edge, like Arc — else a
/// subtle monochrome glyph matching the rest of the caption row. Clicking the
/// button opens the About/Exit menu.
fn draw_app_menu_icon(hdc: HDC, rect: RECT, icon_path: &str) {
    if !icon_path.trim().is_empty() {
        let icon_rect = centered_square(rect, 18);
        let size = rect_width(&icon_rect).max(1) as u32;
        if draw_icon_from_path(hdc, icon_path, icon_rect, size) {
            return;
        }
    }
    draw_frame_button_glyph(hdc, GLYPH_APP_MENU, rect, SHELL_FRAME_BUTTON_ICON);
}

fn draw_design_icon_button(hdc: HDC, rect: RECT, icon: WindowsDesignIcon, rgb: u32, size: i32) {
    let icon_rect = centered_square(rect, size);
    if !draw_windows_design_icon_with_color(hdc, icon, icon_rect, rgb) {
        let fallback = match icon {
            WindowsDesignIcon::Back => Some(GLYPH_NAV_BACK),
            WindowsDesignIcon::Forward => Some(GLYPH_NAV_FORWARD),
            WindowsDesignIcon::BrowserRefresh => Some(GLYPH_NAV_RELOAD),
            WindowsDesignIcon::Home => Some(GLYPH_NAV_HOME),
            WindowsDesignIcon::SidebarCollapse => Some(GLYPH_SIDEBAR_TOGGLE),
            WindowsDesignIcon::SidebarExpand => Some(GLYPH_PANEL_EXPAND),
            _ => None,
        };
        if let Some(glyph) = fallback {
            draw_frame_button_glyph(hdc, glyph, rect, rgb);
        }
    }
}

fn centered_square(rect: RECT, size: i32) -> RECT {
    let width = rect_width(&rect);
    let height = rect_height(&rect);
    let left = rect.left + (width - size).max(0) / 2;
    let top = rect.top + (height - size).max(0) / 2;
    normalize_rect(RECT {
        left,
        top,
        right: left + size.min(width.max(1)),
        bottom: top + size.min(height.max(1)),
    })
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
    layout: &WindowsShellWindowLayout,
    navbar_rect: RECT,
) -> i32 {
    let controls = top_bar_controls(client, top_bar, layout);
    // Clear the leading-edge app-menu button, when visible, and the sidebar
    // toggle. Both are off in the sidebar column when a sidebar is expanded,
    // so the `max` is a no-op there.
    let mut left = navbar_rect.left + 8;
    if let Some(app_icon) = controls.app_icon {
        left = left.max(app_icon.right + TOP_BAR_BUTTON_GAP);
    }
    if let Some(toggle) = controls.sidebar_toggle {
        left = left.max(toggle.right + TOP_BAR_BUTTON_GAP);
    }
    left
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
