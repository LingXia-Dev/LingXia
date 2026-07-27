/// Side and bottom clearance around the elevated desktop content card. Its
/// top edge stays flush with the first sidebar row below the caption band.
pub(super) const SHELL_CONTENT_INSET: i32 = 12;

/// Separation belongs only between independently resizable main/aside panes.
pub(super) const SHELL_PANEL_GAP: i32 = 8;

/// Radius of the elevated content wrapper and its shadow. Windowed WebView2
/// content remains rectangular, but the wrapper is still visible while a
/// surface is loading and around native/attached regions.
pub(super) const SHELL_CONTENT_RADIUS: i32 = 10;

pub(super) const SHELL_PANEL_RADIUS: i32 = 14;

pub(super) const SHELL_BADGE_RED: u32 = 0xff3b30;

/// Themed shell palette, derived at paint time from the Win11 light/dark
/// setting and the system accent (see [`super::theme`]). All fields are
/// `0xRRGGBB` - the format `rgb_to_colorref` expects.
#[derive(Clone, Copy)]
pub(super) struct ShellPalette {
    pub window_background: u32,
    pub panel_background: u32,
    /// Quiet selection wash used by sidebar rows. Keeping this distinct from
    /// the content-card white avoids stacking bright cards inside the rail.
    pub selection_background: u32,
    /// Low-contrast wash for an active top-level lxapp group. Its selected
    /// tabbar child remains the stronger white card, matching macOS hierarchy.
    pub group_active_background: u32,
    pub sidebar_background: u32,
    pub text_primary: u32,
    pub text_muted: u32,
    pub accent: u32,
    pub divider: u32,
    /// Inset control surface (phone URL pill / input field) that reads against
    /// a `panel_background` card.
    pub control_surface: u32,
    /// Quiet desktop address capsule on the shell's first layer.
    pub address_background: u32,
    pub frame_button_icon: u32,
    pub sidebar_header_text: u32,
    pub sidebar_selected_text: u32,
}

/// The active palette for the current system theme. Cheap (two atomic reads +
/// a literal), so call sites can read it per-draw without caching.
pub(super) fn shell_palette() -> ShellPalette {
    let accent = super::theme::system_accent();
    let dark = super::windows_tabbar_effective_dark_mode();
    let mut palette = if dark {
        ShellPalette {
            window_background: 0x202020,
            panel_background: 0x2b2b2b,
            selection_background: 0x34333a,
            group_active_background: 0x343434,
            sidebar_background: 0x202020,
            text_primary: 0xf3f3f3,
            text_muted: 0x9aa0a6,
            accent,
            divider: 0x383838,
            control_surface: 0x3a3a3a,
            address_background: 0x2a2a2a,
            frame_button_icon: 0xe6e6e6,
            sidebar_header_text: 0xb0b4ba,
            sidebar_selected_text: 0xf3f3f3,
        }
    } else {
        ShellPalette {
            window_background: 0xdad6e4,
            panel_background: 0xffffff,
            selection_background: 0xf7f5fb,
            group_active_background: 0xcfccd6,
            sidebar_background: 0xdad6e4,
            text_primary: 0x111827,
            text_muted: 0x667085,
            accent,
            divider: 0xc7c2d2,
            control_surface: 0xf3f4f6,
            address_background: 0xe5e2ec,
            frame_button_icon: 0x1f2937,
            sidebar_header_text: 0x4f5661,
            sidebar_selected_text: 0x111827,
        }
    };
    if let Some(theme) = lingxia_app_context::app_config()
        .and_then(|config| config.shell_theme.as_ref())
        .map(|theme| if dark { &theme.dark } else { &theme.light })
    {
        let color = |value: &Option<String>| value.as_deref().and_then(parse_rgb);
        palette.window_background =
            color(&theme.window_background_color).unwrap_or(palette.window_background);
        palette.panel_background =
            color(&theme.surface_background_color).unwrap_or(palette.panel_background);
        palette.text_primary = color(&theme.foreground_color).unwrap_or(palette.text_primary);
        palette.text_muted = color(&theme.muted_foreground_color).unwrap_or(palette.text_muted);
        palette.accent = color(&theme.accent_color).unwrap_or(palette.accent);
        palette.divider = color(&theme.separator_color).unwrap_or(palette.divider);
        palette.selection_background =
            color(&theme.selection_background_color).unwrap_or(palette.selection_background);
        palette.sidebar_background =
            color(&theme.sidebar_background_color).unwrap_or(palette.sidebar_background);
        palette.sidebar_header_text = color(&theme.sidebar_foreground_color)
            .or_else(|| color(&theme.muted_foreground_color))
            .unwrap_or(palette.sidebar_header_text);
        palette.group_active_background = color(&theme.sidebar_selected_background_color)
            .unwrap_or(palette.group_active_background);
        palette.sidebar_selected_text = color(&theme.sidebar_selected_foreground_color)
            .unwrap_or(palette.sidebar_selected_text);
        palette.control_surface =
            color(&theme.surface_background_color).unwrap_or(palette.control_surface);
        palette.address_background =
            color(&theme.selection_background_color).unwrap_or(palette.address_background);
        palette.frame_button_icon =
            color(&theme.foreground_color).unwrap_or(palette.frame_button_icon);
    }
    palette
}

fn parse_rgb(value: &str) -> Option<u32> {
    let value = value.trim().strip_prefix('#')?;
    match value.len() {
        6 => u32::from_str_radix(value, 16).ok(),
        8 => u32::from_str_radix(&value[2..], 16).ok(),
        _ => None,
    }
}

/// Hover wash (`0xAARRGGBB`) for interactive chrome; an alpha overlay reads
/// correctly on any surface, including colored lxapp navigation bars.
pub(super) fn hover_overlay() -> u32 {
    if super::windows_tabbar_effective_dark_mode() {
        0x28ffffff
    } else {
        0x1f000000
    }
}

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

/// Height of the terminal panel header (tab strip + maximize) row.
pub(super) const TERMINAL_HEADER_HEIGHT: i32 = 34;

/// Fallback terminal surface background (#282C34, `lxTerminalBackground`)
/// used until a snapshot reports its own background color.
pub(super) const TERMINAL_SURFACE_BACKGROUND: u32 = 0x282c34;

/// Header background: darker than the terminal surface so the strip reads
/// as recessed chrome while the active tab flows into the surface. Matches
/// the macOS terminal rail (`lxTerminalChrome`).
pub(super) const TERMINAL_HEADER_BACKGROUND: u32 = 0x21252b;

pub(super) const TERMINAL_HEADER_TEXT: u32 = 0xe8eaf0;

pub(super) const TERMINAL_HEADER_TEXT_MUTED: u32 = 0x9aa3b2;

/// Corner radius of the active tab's top-rounded pill; shared with the
/// macOS tab rail so both platforms draw the same shape.
pub(super) const TERMINAL_TAB_RADIUS: i32 = 8;

/// Marker-dot accent of the active tab (macOS tab rail mint).
pub(super) const TERMINAL_TAB_ACCENT: u32 = 0xaecfbb;

/// Diameter of the per-tab marker dot.
pub(super) const TERMINAL_TAB_DOT_SIZE: i32 = 6;

/// Minimum tab width that still draws the marker dot + inset title.
pub(super) const TERMINAL_TAB_DOT_MIN_WIDTH: i32 = 56;

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

/// Compact Arc-style caption strip.
pub(super) const SHELL_TOP_BAR_HEIGHT: i32 = 32;

/// Win11 caption-button width.
pub(super) const WINDOW_BUTTON_WIDTH: i32 = 46;

/// Caption glyph size.
pub(super) const WINDOW_BUTTON_GLYPH_POINT_SIZE: i32 = 9;

pub(super) const GLYPH_MINIMIZE: &str = "\u{e921}";

pub(super) const GLYPH_MAXIMIZE: &str = "\u{e922}";

pub(super) const GLYPH_RESTORE: &str = "\u{e923}";

pub(super) const GLYPH_CLOSE: &str = "\u{e8bb}";

pub(super) const GLYPH_PANEL_EXPAND: &str = "\u{e740}";
pub(super) const GLYPH_PANEL_SHRINK: &str = "\u{e73f}";

pub(super) const GLYPH_NAV_BACK: &str = "\u{e72b}";

pub(super) const GLYPH_NAV_FORWARD: &str = "\u{e72a}";

pub(super) const GLYPH_NAV_RELOAD: &str = "\u{e72c}";

pub(super) const GLYPH_NAV_HOME: &str = "\u{e80f}";
