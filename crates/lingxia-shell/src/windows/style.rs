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
