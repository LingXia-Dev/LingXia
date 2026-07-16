/// Clear space around the elevated desktop content card. The top caption /
/// address row and the sidebar stay on the shell's base layer; the WebView
/// card is inset from both so it reads as a separate surface.
pub(super) const SHELL_CONTENT_INSET: i32 = 12;

/// Separation belongs only between independently resizable main/aside panes.
pub(super) const SHELL_PANEL_GAP: i32 = 6;

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
    /// Resting surface for persistent panel activators. It stays quieter than
    /// active/hover states while keeping the controls discoverable.
    pub activator_background: u32,
    pub sidebar_background: u32,
    pub text_primary: u32,
    pub text_muted: u32,
    pub accent: u32,
    pub divider: u32,
    /// Inset control surface (URL pill / input field) that must read against a
    /// `panel_background` card.
    pub control_surface: u32,
    pub frame_button_icon: u32,
    pub sidebar_header_text: u32,
}

/// The active palette for the current system theme. Cheap (two atomic reads +
/// a literal), so call sites can read it per-draw without caching.
pub(super) fn shell_palette() -> ShellPalette {
    let accent = super::theme::system_accent();
    if super::theme::is_dark() {
        ShellPalette {
            window_background: 0x202020,
            panel_background: 0x2b2b2b,
            selection_background: 0x34333a,
            group_active_background: 0x343434,
            activator_background: 0x272727,
            sidebar_background: 0x202020,
            text_primary: 0xf3f3f3,
            text_muted: 0x9aa0a6,
            accent,
            divider: 0x383838,
            control_surface: 0x3a3a3a,
            frame_button_icon: 0xe6e6e6,
            sidebar_header_text: 0xb0b4ba,
        }
    } else {
        ShellPalette {
            window_background: 0xdad6e4,
            panel_background: 0xffffff,
            selection_background: 0xf7f5fb,
            group_active_background: 0xcfccd6,
            activator_background: 0xe5e2ec,
            sidebar_background: 0xdad6e4,
            text_primary: 0x111827,
            text_muted: 0x667085,
            accent,
            divider: 0xc7c2d2,
            control_surface: 0xf3f4f6,
            frame_button_icon: 0x1f2937,
            sidebar_header_text: 0x4f5661,
        }
    }
}

/// Hover wash (`0xAARRGGBB`) for interactive chrome; an alpha overlay reads
/// correctly on any surface, including colored lxapp navigation bars.
pub(super) fn hover_overlay() -> u32 {
    if super::theme::is_dark() {
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
