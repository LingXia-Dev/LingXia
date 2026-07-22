//! Shell top bar, address capsule, navigation bar, and caption buttons.

use std::collections::HashMap;
#[cfg(feature = "browser-runtime")]
use std::ffi::c_void;
use std::sync::Mutex;

use crate::{WindowsDesignIcon, draw_windows_design_icon_with_color};

use super::*;

/// Whether the layout carries a visible browser address bar (which then
/// owns the top bar; the lxapp navigation bar yields).
/// Width the floating device capsule keeps occupied at a framed screen's
/// top-right, which the top-bar controls must stay clear of.
fn device_capsule_reserve() -> i32 {
    #[cfg(feature = "device-frame")]
    {
        crate::device_frame::capsule_reserve_width()
    }
    #[cfg(not(feature = "device-frame"))]
    {
        0
    }
}

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
    /// Current-page bookmark/pin toggles and overflow page menu.
    pub(super) bookmark: Option<RECT>,
    pub(super) pin: Option<RECT>,
    pub(super) page_menu: Option<RECT>,
    /// Dismisses the presented browser tab back to the lxapp. Only on
    /// device-framed screens (no caption buttons), mirroring the macOS
    /// phone browser's close button.
    pub(super) browser_close: Option<RECT>,
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

    // App-menu button at the window's leading edge. When a sidebar exists it
    // shares the sidebar header's leading edge with the toggle (which moves
    // to its right); otherwise it anchors the top bar. Only the product shell
    // (`browser-shell`) has a real app menu (About/Exit); a runner-style
    // build would offer a lone "Exit" that just duplicates the window close,
    // so it gets no button at all.
    let app_icon_left = if has_sidebar_toggle {
        client.left + TOP_BAR_PADDING
    } else {
        top_bar.left + TOP_BAR_PADDING
    };
    let app_icon =
        (cfg!(feature = "browser-shell") && !compact_sidebar).then(|| square_button(app_icon_left));

    // Sidebar toggle: sits just right of the app-menu button (or takes its
    // slot when there is none). It is
    // The collapse toggle lives in the sidebar header while the sidebar is
    // expanded. Once collapsed to a rail, the rail draws the *same* toggle
    // icon pinned to its bottom (see `draw_sidebar_rail`), so the top bar
    // shows none here — otherwise the rail would carry two expand affordances.
    let sidebar_toggle = (has_sidebar_toggle && !compact_sidebar).then(|| {
        let left = app_icon
            .map(|rect| rect.right + TOP_BAR_BUTTON_GAP)
            .unwrap_or(app_icon_left);
        square_button(left)
    });
    let mut left_edge = top_bar.left + TOP_BAR_PADDING;
    // The app-menu slot is skipped on a device-framed screen (the icon is not
    // drawn there — its menu lives on the frame's capsule), freeing the
    // leading edge for the browser controls.
    if !layout.suppress_window_controls
        && let Some(app_icon) = app_icon
    {
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
        bookmark: None,
        pin: None,
        page_menu: None,
        browser_close: None,
    };
    // On the phone frame the browser chrome is the bottom bar; the top bar
    // carries no browser controls.
    if !address_bar_visible(layout) || super::phone_browser_bar_active(client, layout) {
        return controls;
    }

    // The frame buttons own the client's trailing edge; everything between
    // the toggle and them is available to the address section.
    let mut right_edge = (client.right - window_frame_buttons_width() - TOP_BAR_PADDING)
        .min(top_bar.right - TOP_BAR_PADDING);
    // A device-framed screen has no caption buttons: the presented browser
    // leads with a close button instead (Safari-view style), and the trailing
    // edge stays clear of the floating device capsule.
    if layout.suppress_window_controls {
        let close = square_button(left_edge);
        controls.browser_close = Some(close);
        left_edge = close.right + TOP_BAR_BUTTON_GAP;
        right_edge = top_bar.right - TOP_BAR_PADDING - device_capsule_reserve();
    }
    let aside = layout
        .address_bar
        .as_ref()
        .is_some_and(|address_bar| address_bar.aside);
    // The ••• page menu hugs the capsule's trailing edge (macOS groups the
    // page controls with the address bar, not the window edge); reserve its
    // slot here, place the button once the capsule rect is known.
    if !aside {
        right_edge -= TOP_BAR_BUTTON_SIZE + ADDRESS_CAPSULE_NAV_GAP;
    }
    let nav_width = 3 * TOP_BAR_BUTTON_SIZE + 2 * TOP_BAR_BUTTON_GAP;
    let capsule_space = right_edge - left_edge - nav_width - ADDRESS_CAPSULE_NAV_GAP;
    if capsule_space < 48 {
        if !aside {
            controls.page_menu = Some(square_button(right_edge + ADDRESS_CAPSULE_NAV_GAP));
        }
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

    // The capsule sits right after the nav cluster, keeping the reload
    // button and the address text together. It remains visible but read-only
    // for API-managed aside tabs.
    let capsule_left = nav_left + nav_width + ADDRESS_CAPSULE_NAV_GAP;
    let capsule_width = capsule_space.min(ADDRESS_CAPSULE_MAX_WIDTH);
    let capsule_height = ADDRESS_CAPSULE_HEIGHT.min(rect_height(&top_bar));
    let capsule_top = top_bar.top + (rect_height(&top_bar) - capsule_height).max(0) / 2;
    let capsule = normalize_rect(RECT {
        left: capsule_left,
        top: capsule_top,
        right: (capsule_left + capsule_width).min(right_edge),
        bottom: capsule_top + capsule_height,
    });
    controls.address = Some(capsule);
    if !aside {
        controls.page_menu = Some(square_button(capsule.right + ADDRESS_CAPSULE_NAV_GAP));
    }
    // Star/pin live inside the capsule's trailing edge, like the macOS
    // address bar; internal pages have neither (they cannot be bookmarked).
    let web = layout
        .address_bar
        .as_ref()
        .is_some_and(|address_bar| address_bar.web);
    if !aside && web && rect_width(&capsule) >= 4 * ADDRESS_CAPSULE_BUTTON_SIZE {
        let button_top =
            capsule.top + (rect_height(&capsule) - ADDRESS_CAPSULE_BUTTON_SIZE).max(0) / 2;
        let capsule_button = |right: i32| RECT {
            left: right - ADDRESS_CAPSULE_BUTTON_SIZE,
            top: button_top,
            right,
            bottom: button_top + ADDRESS_CAPSULE_BUTTON_SIZE,
        };
        let pin = capsule_button(capsule.right - 5);
        let bookmark = capsule_button(pin.left - 2);
        controls.pin = Some(pin);
        controls.bookmark = Some(bookmark);
    }
    controls
}

/// Last painted URL address rect per host window, so the facade can start
/// an inline address edit (EDIT child) over the address bar; same pattern as
/// the terminal tab-title rects in `terminal_grid`.
static ADDRESS_CAPSULE_RECTS: OnceLock<Mutex<HashMap<isize, RECT>>> = OnceLock::new();

pub(super) fn remember_address_capsule_rect(hwnd: HWND, rect: Option<RECT>) {
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

/// Starts an inline URL edit over the address bar last painted in
/// `window`'s top bar, prefilled with `initial_text` (selected). Safe to
/// call from any thread; the editor is marshalled onto the window's UI
/// thread (see [`super::super::text_input`] for lifecycle). `on_commit`
/// receives the submitted text on Enter/focus loss; Esc cancels. Returns
/// `false` when no address capsule has been painted for `window`.
#[cfg(feature = "browser-runtime")]
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
    // The editor sits inside the address bar fill, inset enough to match the
    // painted URL text.
    let edit_rect = inset_rect(capsule, 12, 4);
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
    fill_rect(hdc, rects.top_bar, shell_palette().window_background);
}

/// Draws the interactive top-bar controls (sidebar toggle, browser nav
/// buttons, URL address bar) and records the address rect for the inline
/// address editor. Painted after the navigation bar, which fills the top
/// bar with its own background.
pub(super) fn draw_top_bar_controls(
    hdc: HDC,
    state: &WindowsChromeState,
    rects: &ChromeRects,
    layout: &WindowsShellWindowLayout,
) {
    let controls = top_bar_controls(state.client, rects.top_bar, layout);
    let cursor = state.cursor;
    // The leading app-menu button is a window control; a device-framed screen
    // gets it from the simulator toolbar instead.
    if !layout.suppress_window_controls
        && let Some(app_icon) = controls.app_icon
    {
        draw_hover_wash(hdc, app_icon, 5, cursor);
        draw_app_menu_icon(hdc, app_icon);
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
        // Muted like the sidebar header actions - it's a secondary control,
        // not a primary caption button.
        draw_hover_wash(hdc, toggle, 5, cursor);
        draw_design_icon_button(hdc, toggle, icon, shell_palette().text_muted, 18);
    }
    // Back/forward dim while the presented tab has no history in that
    // direction (smart nav, mirroring the macOS browser chrome).
    let (can_back, can_forward) = layout
        .address_bar
        .as_ref()
        .map(|address_bar| (address_bar.can_go_back, address_bar.can_go_forward))
        .unwrap_or((true, true));
    if let Some(back) = controls.nav_back {
        draw_hover_wash(hdc, back, 5, cursor);
        let color = if can_back {
            shell_palette().frame_button_icon
        } else {
            shell_palette().text_muted
        };
        draw_design_icon_button(hdc, back, WindowsDesignIcon::Back, color, 18);
    }
    if let Some(forward) = controls.nav_forward {
        draw_hover_wash(hdc, forward, 5, cursor);
        let color = if can_forward {
            shell_palette().frame_button_icon
        } else {
            shell_palette().text_muted
        };
        draw_design_icon_button(hdc, forward, WindowsDesignIcon::Forward, color, 18);
    }
    if let Some(reload) = controls.nav_reload {
        draw_hover_wash(hdc, reload, 5, cursor);
        draw_design_icon_button(
            hdc,
            reload,
            WindowsDesignIcon::BrowserRefresh,
            shell_palette().frame_button_icon,
            18,
        );
    }
    if let Some(close) = controls.browser_close {
        draw_hover_wash(hdc, close, 5, cursor);
        draw_frame_button_glyph(hdc, GLYPH_CLOSE, close, shell_palette().frame_button_icon);
    }
    if let Some(address) = controls.address {
        fill_round_rect_aa(
            hdc,
            address,
            rect_height(&address) / 2,
            shell_palette().address_background,
        );
        let text = layout
            .address_bar
            .as_ref()
            .map(|address_bar| address_bar.url_text.as_str())
            .unwrap_or_default();
        // Left-aligned like a browser address bar, so the URL reads next to
        // the nav cluster instead of floating in the capsule's middle. The
        // text yields to the trailing star/pin buttons when present.
        let text_right = controls
            .bookmark
            .map(|bookmark| bookmark.left - 4)
            .unwrap_or(address.right - 12);
        draw_text(
            hdc,
            text,
            normalize_rect(RECT {
                left: address.left + 12,
                top: address.top,
                right: text_right,
                bottom: address.bottom,
            }),
            shell_palette().text_primary,
            DT_LEFT,
        );
    }
    // Star/pin inside the capsule: accent-tinted while active, muted
    // otherwise (macOS address-bar styling).
    if let Some(bookmark) = controls.bookmark {
        draw_hover_wash(hdc, bookmark, 5, cursor);
        let filled = layout
            .address_bar
            .as_ref()
            .is_some_and(|address_bar| address_bar.bookmarked);
        draw_design_icon_button(
            hdc,
            bookmark,
            if filled {
                WindowsDesignIcon::BookmarkFilled
            } else {
                WindowsDesignIcon::Bookmark
            },
            if filled {
                shell_palette().accent
            } else {
                shell_palette().text_muted
            },
            14,
        );
    }
    if let Some(pin) = controls.pin {
        draw_hover_wash(hdc, pin, 5, cursor);
        let filled = layout
            .address_bar
            .as_ref()
            .is_some_and(|address_bar| address_bar.pinned);
        draw_design_icon_button(
            hdc,
            pin,
            if filled {
                WindowsDesignIcon::PinFilled
            } else {
                WindowsDesignIcon::Pin
            },
            if filled {
                shell_palette().accent
            } else {
                shell_palette().text_muted
            },
            14,
        );
    }
    if let Some(page_menu) = controls.page_menu {
        draw_hover_wash(hdc, page_menu, 5, cursor);
        draw_design_icon_button(
            hdc,
            page_menu,
            WindowsDesignIcon::PageMenu,
            shell_palette().frame_button_icon,
            18,
        );
    }
    // The inline URL editor overlays the capsule but must not cover the
    // trailing star/pin buttons.
    let edit_rect = controls.address.map(|address| match controls.bookmark {
        Some(bookmark) => RECT {
            right: bookmark.left - 4,
            ..address
        },
        None => address,
    });
    let editable = layout
        .address_bar
        .as_ref()
        .is_some_and(|address_bar| !address_bar.aside);
    remember_address_capsule_rect(state.hwnd, editable.then_some(edit_rect).flatten());
}

/// Rendered size of the back/home navigation glyphs.
const NAV_ICON_SIZE: i32 = 22;

/// Leading inset of the navigation bar's back/home button from the screen edge.
const NAV_LEADING_MARGIN: i32 = 8;

pub(super) fn draw_navigation_bar(
    hdc: HDC,
    rect: RECT,
    corner_radii: [i32; 4],
    desktop_card: bool,
    buttons_left: i32,
    navbar: &WindowsShellNavigationBarLayout,
    cursor: Option<(i32, i32)>,
) {
    // The band owns whichever workspace-silhouette corners it touches; a
    // square fill in the page's bar color would poke past the card's arcs.
    fill_round_rect_aa_corners(hdc, rect, corner_radii, navbar.background_color);
    // Mobile/framed form fuses the bar with the page — no divider seam.
    if desktop_card {
        draw_bottom_border(hdc, rect, shell_palette().divider);
    }

    let text_color = navbar.text_color;
    let mut left_controls_width = 0;

    if navbar.show_back_button {
        let back_rect = nav_button_rect(rect, buttons_left, 0);
        // Left-align the chevron near the leading edge instead of centering it
        // in the 44px tap target, so it sits close to the screen edge. The tap
        // target keeps its full width for title clearance below.
        let slot = leading_icon_slot(back_rect);
        if cursor.is_some_and(|point| rect_contains(&back_rect, point)) {
            fill_round_rect_overlay(hdc, nav_hover_rect(slot, back_rect), 5, hover_overlay());
        }
        draw_design_icon_button(
            hdc,
            slot,
            WindowsDesignIcon::Back,
            text_color,
            NAV_ICON_SIZE,
        );
        left_controls_width = back_rect.right - rect.left;
    }
    if navbar.show_home_button {
        let home_rect = nav_button_rect(
            rect,
            buttons_left,
            if navbar.show_back_button { 1 } else { 0 },
        );
        let slot = leading_icon_slot(home_rect);
        if cursor.is_some_and(|point| rect_contains(&home_rect, point)) {
            fill_round_rect_overlay(hdc, nav_hover_rect(slot, home_rect), 5, hover_overlay());
        }
        draw_design_icon_button(
            hdc,
            slot,
            WindowsDesignIcon::Home,
            text_color,
            NAV_ICON_SIZE,
        );
        left_controls_width = home_rect.right - rect.left;
    }

    if !navbar.title.trim().is_empty() {
        let title_inset = left_controls_width + 8;
        let title_rect = normalize_rect(RECT {
            left: rect.left + title_inset,
            top: rect.top,
            right: rect.right - title_inset,
            bottom: rect.bottom,
        });
        draw_text(hdc, &navbar.title, title_rect, text_color, DT_CENTER);
    }
}

/// A leading-edge, left-aligned slot (the icon's own width) at the start of a
/// navigation icon's wider tap-target rect, so the chevron sits right at the
/// screen edge rather than floating in the middle of the 44px button.
fn leading_icon_slot(button: RECT) -> RECT {
    let slot = NAV_ICON_SIZE;
    normalize_rect(RECT {
        left: button.left,
        top: button.top,
        right: (button.left + slot).min(button.right),
        bottom: button.bottom,
    })
}

/// The hover wash square for a navbar button: sized like a top-bar button,
/// centered on the drawn glyph slot rather than the wider tap target.
fn nav_hover_rect(slot: RECT, button: RECT) -> RECT {
    let size = TOP_BAR_BUTTON_SIZE;
    let center_x = (slot.left + slot.right) / 2;
    let center_y = (button.top + button.bottom) / 2;
    normalize_rect(RECT {
        left: center_x - size / 2,
        top: center_y - size / 2,
        right: center_x + size / 2,
        bottom: center_y + size / 2,
    })
}

/// Draws the app-menu button at the window's leading edge, like Arc: the
/// LingXia brand mark (the bare vessel glyph on transparency,
/// `<asset_dir>/icons/lingxia.png`) rather than the app's launcher icon, whose
/// full plate reads as a white box in the caption row. Falls back to a subtle
/// monochrome glyph matching the rest of the caption row before the asset dir
/// is known. Clicking the button opens the About/Exit menu.
fn draw_app_menu_icon(hdc: HDC, rect: RECT) {
    let icon_rect = centered_square(rect, 18);
    if draw_default_app_icon(hdc, icon_rect) {
        return;
    }
    draw_frame_button_glyph(hdc, GLYPH_APP_MENU, rect, shell_palette().frame_button_icon);
}

pub(super) fn draw_design_icon_button(
    hdc: HDC,
    rect: RECT,
    icon: WindowsDesignIcon,
    rgb: u32,
    size: i32,
) {
    draw_design_icon_button_with_fallback(hdc, rect, icon, rgb, size, None);
}

pub(super) fn draw_design_icon_button_with_fallback(
    hdc: HDC,
    rect: RECT,
    icon: WindowsDesignIcon,
    rgb: u32,
    size: i32,
    fallback: Option<&str>,
) {
    let icon_rect = centered_square(rect, size);
    if !draw_windows_design_icon_with_color(hdc, icon, icon_rect, rgb) {
        let fallback = fallback.or(match icon {
            WindowsDesignIcon::Back => Some(GLYPH_NAV_BACK),
            WindowsDesignIcon::Forward => Some(GLYPH_NAV_FORWARD),
            WindowsDesignIcon::BrowserRefresh => Some(GLYPH_NAV_RELOAD),
            WindowsDesignIcon::Home => Some(GLYPH_NAV_HOME),
            _ => None,
        });
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
                shell_palette().window_background,
                FRAME_BUTTON_PRESSED_OVERLAY,
            ))
        } else if show_hover {
            Some(darken_rgb(
                shell_palette().window_background,
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
            shell_palette().frame_button_icon
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
        let font = caption_icon_font(hdc);
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
    }
}

/// Caption icon font: Segoe Fluent Icons (Win11), falling back to Segoe
/// MDL2 Assets (Win10). The GDI font mapper silently substitutes missing
/// faces, so each candidate is verified via `GetTextFaceW` before its
/// private-use glyphs are trusted. The probe runs once per DPI height;
/// the resolved font is a shared cache entry - do not delete.
pub(super) fn caption_icon_font(hdc: HDC) -> HFONT {
    let height = logical_font_height(hdc, WINDOW_BUTTON_GLYPH_POINT_SIZE);
    cached_font_with("caption-icon", height, 400, CLEARTYPE_QUALITY, || {
        create_caption_icon_font(hdc, -height)
    })
}

fn create_caption_icon_font(hdc: HDC, height: i32) -> HFONT {
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

/// Leading x of the main-owned navigation bar's back/home buttons.
pub(super) fn navbar_buttons_left(navbar_rect: RECT) -> i32 {
    navbar_rect.left + NAV_LEADING_MARGIN
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
