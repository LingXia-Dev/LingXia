//! Host chrome colours for a GPU page: the bars and canvas around it take the roles of the
//! palette the page renders with, so chrome and page are one surface in every look.
use crate::theme::Palette;
use lingxia_theme::Role;

/// RGBA colours the host paints its chrome with while this page is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostChrome {
    /// Window and page floor: behind the surface, in a pull-to-refresh strip, during transitions.
    pub canvas: [u8; 4],
    /// Title bar and tab bar background.
    pub bar: [u8; 4],
    /// Their separator.
    pub bar_line: [u8; 4],
    /// Titles and icons on the bars.
    pub content: [u8; 4],
    /// Unselected tab items.
    pub content_muted: [u8; 4],
    /// The selected tab item.
    pub accent: [u8; 4],
    /// System bar glyphs go light: the colours behind them are dark.
    pub dark: bool,
}

impl HostChrome {
    pub fn from_palette(palette: &Palette) -> HostChrome {
        let bar = palette.role(Role::Surface);
        HostChrome {
            canvas: palette.role(Role::Canvas),
            bar,
            bar_line: palette.role(Role::Line),
            content: palette.role(Role::Text),
            content_muted: palette.role(Role::TextMuted),
            accent: palette.role(Role::Accent),
            dark: luminance(bar).max(luminance(palette.role(Role::Canvas))) < 0.5,
        }
    }

    /// The chrome for an environment the host has applied its pushes to: its palette push,
    /// or the built-in palette for its look, the same palette the page renders with.
    pub fn from_environment(env: &serde_json::Value) -> HostChrome {
        let dark = env["theme"] == "dark";
        let high_contrast = env["highContrast"] == true;
        let palette = env
            .get("palette")
            .and_then(|pushed| Palette::decode(pushed, dark).ok())
            .unwrap_or_else(|| Palette::builtin_for(dark, high_contrast));
        HostChrome::from_palette(&palette)
    }
}

/// Relative luminance (sRGB, alpha ignored).
fn luminance([r, g, b, _]: [u8; 4]) -> f32 {
    let channel = |c: u8| {
        let c = f32::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_follows_the_page_palette_in_every_look() {
        for (dark, high_contrast) in [(false, false), (true, false), (false, true), (true, true)] {
            let palette = Palette::builtin_for(dark, high_contrast);
            let env = serde_json::json!({
                "theme": if dark { "dark" } else { "light" },
                "highContrast": high_contrast,
            });
            let chrome = HostChrome::from_environment(&env);
            assert_eq!(
                chrome.canvas,
                palette.role(Role::Canvas),
                "canvas is the page floor"
            );
            assert_eq!(chrome.bar, palette.role(Role::Surface));
            assert_eq!(chrome.accent, palette.role(Role::Accent));
            assert_eq!(chrome.dark, dark, "glyphs follow the look");
        }
    }

    #[test]
    fn a_pushed_palette_wins_over_the_builtin_one() {
        let mut colors: Vec<String> = Palette::builtin(false)
            .0
            .iter()
            .map(|[r, g, b, _]| format!("#{r:02x}{g:02x}{b:02x}"))
            .collect();
        colors[Role::Canvas as usize] = "#123456".into();
        let env = serde_json::json!({
            "theme": "light",
            "palette": {"revision": 1, "colors": colors},
        });
        assert_eq!(
            HostChrome::from_environment(&env).canvas,
            [0x12, 0x34, 0x56, 0xff]
        );
    }
}
