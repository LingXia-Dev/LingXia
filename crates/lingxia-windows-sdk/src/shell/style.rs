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
}

fn composite_argb(argb: u32, background: u32) -> u32 {
    let alpha = argb >> 24;
    let foreground = argb & 0x00ff_ffff;
    let blend = |shift: u32| {
        let foreground = (foreground >> shift) & 0xffu32;
        let background = (background >> shift) & 0xffu32;
        ((foreground * alpha + background * (255 - alpha) + 127) / 255) << shift
    };
    blend(16) | blend(8) | blend(0)
}

/// Opaque equivalents of the WinUI common theme resources. The custom GDI
/// renderer cannot consume XAML brushes, so translucent Fluent tokens are
/// composited over `SolidBackgroundFillColorBase` before painting.
fn fluent_shell_palette(dark: bool, accent: u32) -> ShellPalette {
    let (
        window_background,
        panel_fill,
        selection_fill,
        text_primary_fill,
        text_muted_fill,
        divider_fill,
        control_fill,
    ) = if dark {
        (
            0x202020,
            0x0dff_ffff,
            0x0fff_ffff,
            0xffff_ffff,
            0xc5ff_ffff,
            0x15ff_ffff,
            0x0fff_ffff,
        )
    } else {
        (
            0xf3f3f3,
            0xb3ff_ffff,
            0x0900_0000,
            0xe400_0000,
            0x9e00_0000,
            0x0f00_0000,
            0xb3ff_ffff,
        )
    };
    let panel_background = composite_argb(panel_fill, window_background);
    let selection_background = composite_argb(selection_fill, window_background);
    let text_primary = composite_argb(text_primary_fill, window_background);
    let text_muted = composite_argb(text_muted_fill, window_background);
    let divider = composite_argb(divider_fill, window_background);
    let control_surface = composite_argb(control_fill, window_background);
    ShellPalette {
        window_background,
        panel_background,
        selection_background,
        group_active_background: selection_background,
        sidebar_background: window_background,
        text_primary,
        text_muted,
        accent,
        divider,
        control_surface,
        address_background: control_surface,
        frame_button_icon: text_primary,
        sidebar_header_text: text_muted,
    }
}

fn high_contrast_shell_palette(colors: super::theme::SystemColors) -> ShellPalette {
    ShellPalette {
        window_background: colors.window,
        panel_background: colors.window,
        selection_background: colors.control,
        group_active_background: colors.control,
        sidebar_background: colors.window,
        text_primary: colors.window_text,
        text_muted: colors.gray_text,
        accent: colors.highlight,
        divider: colors.window_text,
        control_surface: colors.control,
        address_background: colors.control,
        frame_button_icon: colors.window_text,
        sidebar_header_text: colors.window_text,
    }
}

fn apply_theme_style(
    mut palette: ShellPalette,
    style: Option<&lingxia_app_context::ThemeStyle>,
) -> ShellPalette {
    let Some(style) = style else {
        return palette;
    };
    if let Some(color) = style.window_background_color {
        palette.window_background = color.rgb();
        palette.sidebar_background = color.rgb();
    }
    if let Some(color) = style.surface_background_color {
        palette.panel_background = color.rgb();
        palette.control_surface = color.rgb();
        palette.address_background = color.rgb();
    }
    if let Some(color) = style.foreground_color {
        palette.text_primary = color.rgb();
        palette.frame_button_icon = color.rgb();
    }
    if let Some(color) = style.muted_foreground_color {
        palette.text_muted = color.rgb();
        palette.sidebar_header_text = color.rgb();
    }
    if let Some(color) = style.accent_color {
        palette.accent = color.rgb();
    }
    if let Some(color) = style.separator_color {
        palette.divider = color.rgb();
    }
    if let Some(color) = style.selection_background_color {
        palette.selection_background = color.rgb();
        palette.group_active_background = color.rgb();
    }
    palette
}

fn palette_for(
    dark: bool,
    accent: u32,
    style: Option<&lingxia_app_context::ThemeStyle>,
) -> ShellPalette {
    apply_theme_style(fluent_shell_palette(dark, accent), style)
}

/// The active palette for the current system theme. Values are cached atomics,
/// so call sites can read it per-draw without querying system APIs.
pub(super) fn shell_palette() -> ShellPalette {
    if super::theme::is_high_contrast() {
        return high_contrast_shell_palette(super::theme::system_colors());
    }
    let dark = super::theme::is_dark();
    let style = lingxia_app_context::theme().and_then(|theme| theme.style(dark));
    palette_for(dark, super::theme::system_accent(), style)
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

/// Height of the terminal panel header (tab strip + maximize) row.
pub(super) const TERMINAL_HEADER_HEIGHT: i32 = 34;

// The terminal card's colors — surface, header, separator and both text
// weights — are not constants: they are derived from the scheme in effect by
// `lingxia_terminal_config::runtime::current_chrome()`, which the Apple host
// reads too. Fixing them here left a hardcoded dark strip attached to a light
// terminal every time someone changed a theme.

/// Corner radius of the active tab's top-rounded pill; shared with the
/// macOS tab rail so both platforms draw the same shape.
pub(super) const TERMINAL_TAB_RADIUS: i32 = 8;

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

#[cfg(test)]
mod tests {
    use super::{high_contrast_shell_palette, palette_for};
    use crate::shell::theme::SystemColors;
    use lingxia_app_context::{ThemeColor, ThemeStyle};

    #[test]
    fn app_theme_maps_semantic_roles_into_windows_shell() {
        let color = |value| Some(ThemeColor::parse(value).unwrap());
        let style = ThemeStyle {
            window_background_color: color("#101112"),
            surface_background_color: color("#202122"),
            foreground_color: color("#F0F1F2"),
            muted_foreground_color: color("#A0A1A2"),
            accent_color: color("#304FFE"),
            separator_color: color("#303132"),
            selection_background_color: color("#404142"),
            // Roles the Windows shell does not consume (the page floor is for
            // chrome that borders lxapp pages) stay default, so a new role
            // does not break this exhaustive mapping test.
            ..ThemeStyle::default()
        };

        let palette = palette_for(true, 0xABCDEF, Some(&style));

        assert_eq!(palette.window_background, 0x101112);
        assert_eq!(palette.sidebar_background, 0x101112);
        assert_eq!(palette.panel_background, 0x202122);
        assert_eq!(palette.control_surface, 0x202122);
        assert_eq!(palette.address_background, 0x202122);
        assert_eq!(palette.text_primary, 0xF0F1F2);
        assert_eq!(palette.frame_button_icon, 0xF0F1F2);
        assert_eq!(palette.text_muted, 0xA0A1A2);
        assert_eq!(palette.sidebar_header_text, 0xA0A1A2);
        assert_eq!(palette.accent, 0x304FFE);
        assert_eq!(palette.divider, 0x303132);
        assert_eq!(palette.selection_background, 0x404142);
        assert_eq!(palette.group_active_background, 0x404142);
    }

    #[test]
    fn partial_theme_keeps_fluent_defaults_for_omitted_roles() {
        let style = ThemeStyle {
            accent_color: Some(ThemeColor::parse("#304FFE").unwrap()),
            ..ThemeStyle::default()
        };

        let palette = palette_for(false, 0xABCDEF, Some(&style));

        assert_eq!(palette.window_background, 0xF3F3F3);
        assert_eq!(palette.panel_background, 0xFBFBFB);
        assert_eq!(palette.text_primary, 0x1A1A1A);
        assert_eq!(palette.text_muted, 0x5C5C5C);
        assert_eq!(palette.accent, 0x304FFE);
        assert_eq!(palette.divider, 0xE5E5E5);
        assert_eq!(palette.selection_background, 0xEAEAEA);
    }

    #[test]
    fn absent_dark_theme_uses_fluent_dark_defaults() {
        let palette = palette_for(true, 0xABCDEF, None);

        assert_eq!(palette.window_background, 0x202020);
        assert_eq!(palette.panel_background, 0x2B2B2B);
        assert_eq!(palette.text_primary, 0xFFFFFF);
        assert_eq!(palette.text_muted, 0xCCCCCC);
        assert_eq!(palette.divider, 0x323232);
        assert_eq!(palette.selection_background, 0x2D2D2D);
        assert_eq!(palette.accent, 0xABCDEF);
    }

    #[test]
    fn contrast_theme_uses_system_colors() {
        let palette = high_contrast_shell_palette(SystemColors {
            window: 0x010203,
            window_text: 0x111213,
            gray_text: 0x212223,
            highlight: 0x313233,
            control: 0x414243,
        });

        assert_eq!(palette.window_background, 0x010203);
        assert_eq!(palette.panel_background, 0x010203);
        assert_eq!(palette.text_primary, 0x111213);
        assert_eq!(palette.text_muted, 0x212223);
        assert_eq!(palette.accent, 0x313233);
        assert_eq!(palette.control_surface, 0x414243);
    }
}
