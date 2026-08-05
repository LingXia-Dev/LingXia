//! Host-facing color scheme.
//!
//! Field names match the Windows Terminal scheme JSON, the format the
//! common scheme collections all publish, so a host can hand a scheme
//! through verbatim instead of inventing a conversion. Cell colors are
//! resolved inside the engine; cursor and selection are carried for the
//! host, which draws them.

use serde::{Deserialize, Serialize};

use crate::alacritty_vt::ThemeColors;

/// A color the scheme could not express.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalThemeError {
    pub field: &'static str,
    pub value: String,
}

impl std::fmt::Display for TerminalThemeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid color for `{}`: {:?} (expected #RRGGBB)",
            self.field, self.value
        )
    }
}

impl std::error::Error for TerminalThemeError {}

/// An ANSI color scheme.
///
/// Every color is `#RRGGBB` (the leading `#` optional). The 16 ANSI
/// entries and the two default colors are required; cursor and
/// selection are optional because many published schemes omit them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalTheme {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub background: String,
    pub foreground: String,
    /// Host-drawn; falls back to the foreground when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_color: Option<String>,
    /// Host-drawn; falls back to inverting fg/bg when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_foreground: Option<String>,
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    /// Windows Terminal names ANSI 5 "purple"; `magenta` is accepted
    /// because kitty/alacritty exports use that name.
    #[serde(alias = "magenta")]
    pub purple: String,
    pub cyan: String,
    pub white: String,
    pub bright_black: String,
    pub bright_red: String,
    pub bright_green: String,
    pub bright_yellow: String,
    pub bright_blue: String,
    #[serde(alias = "brightMagenta")]
    pub bright_purple: String,
    pub bright_cyan: String,
    pub bright_white: String,
}

impl Default for TerminalTheme {
    fn default() -> Self {
        Self {
            name: Some("LingXia Dark".to_string()),
            background: "#282c34".to_string(),
            foreground: "#ffffff".to_string(),
            cursor_color: None,
            selection_background: None,
            selection_foreground: None,
            black: "#1d1f21".to_string(),
            red: "#cc6666".to_string(),
            green: "#b5bd68".to_string(),
            yellow: "#f0c674".to_string(),
            blue: "#81a2be".to_string(),
            purple: "#b294bb".to_string(),
            cyan: "#8abeb7".to_string(),
            white: "#c5c8c6".to_string(),
            bright_black: "#666666".to_string(),
            bright_red: "#d54e53".to_string(),
            bright_green: "#b9ca4a".to_string(),
            bright_yellow: "#e7c547".to_string(),
            bright_blue: "#7aa6da".to_string(),
            bright_purple: "#c397d8".to_string(),
            bright_cyan: "#70c0b1".to_string(),
            bright_white: "#eaeaea".to_string(),
        }
    }
}

impl TerminalTheme {
    /// Parse a scheme from JSON. Extra keys (`name`, per-app extensions)
    /// are ignored, so a whole scheme file can be passed through.
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|err| err.to_string())
    }

    /// Resolve into the engine's palette, expanding the 16 ANSI colors
    /// into the xterm 256-color cube.
    pub fn to_colors(&self) -> Result<ThemeColors, TerminalThemeError> {
        let ansi16 = [
            parse_field("black", &self.black)?,
            parse_field("red", &self.red)?,
            parse_field("green", &self.green)?,
            parse_field("yellow", &self.yellow)?,
            parse_field("blue", &self.blue)?,
            parse_field("purple", &self.purple)?,
            parse_field("cyan", &self.cyan)?,
            parse_field("white", &self.white)?,
            parse_field("brightBlack", &self.bright_black)?,
            parse_field("brightRed", &self.bright_red)?,
            parse_field("brightGreen", &self.bright_green)?,
            parse_field("brightYellow", &self.bright_yellow)?,
            parse_field("brightBlue", &self.bright_blue)?,
            parse_field("brightPurple", &self.bright_purple)?,
            parse_field("brightCyan", &self.bright_cyan)?,
            parse_field("brightWhite", &self.bright_white)?,
        ];
        Ok(ThemeColors::from_ansi16(
            parse_field("foreground", &self.foreground)?,
            parse_field("background", &self.background)?,
            ansi16,
        ))
    }

    /// Cursor color, falling back to the foreground.
    pub fn cursor_rgb(&self) -> Result<[u8; 3], TerminalThemeError> {
        match self.cursor_color.as_deref() {
            Some(value) => parse_field("cursorColor", value),
            None => parse_field("foreground", &self.foreground),
        }
    }

    /// Selection colors as `(background, foreground)`, falling back to
    /// the scheme's inverted defaults.
    pub fn selection_rgb(&self) -> Result<([u8; 3], [u8; 3]), TerminalThemeError> {
        let background = match self.selection_background.as_deref() {
            Some(value) => parse_field("selectionBackground", value)?,
            None => parse_field("foreground", &self.foreground)?,
        };
        let foreground = match self.selection_foreground.as_deref() {
            Some(value) => parse_field("selectionForeground", value)?,
            None => parse_field("background", &self.background)?,
        };
        Ok((background, foreground))
    }
}

fn parse_field(field: &'static str, value: &str) -> Result<[u8; 3], TerminalThemeError> {
    parse_hex_rgb(value).ok_or_else(|| TerminalThemeError {
        field,
        value: value.to_string(),
    })
}

/// Parse `#RRGGBB` / `RRGGBB` into RGB bytes.
pub fn parse_hex_rgb(value: &str) -> Option<[u8; 3]> {
    let hex = value.trim();
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let rgb = u32::from_str_radix(hex, 16).ok()?;
    Some([
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Windows Terminal scheme as published, pasted verbatim.
    // `r##` because the JSON contains `"#` at every color.
    const ONE_HALF_DARK: &str = r##"{
        "name": "One Half Dark",
        "black": "#282c34", "red": "#e06c75", "green": "#98c379",
        "yellow": "#e5c07b", "blue": "#61afef", "purple": "#c678dd",
        "cyan": "#56b6c2", "white": "#dcdfe4",
        "brightBlack": "#5a6374", "brightRed": "#e06c75",
        "brightGreen": "#98c379", "brightYellow": "#e5c07b",
        "brightBlue": "#61afef", "brightPurple": "#c678dd",
        "brightCyan": "#56b6c2", "brightWhite": "#dcdfe4",
        "background": "#282c34", "foreground": "#dcdfe4",
        "cursorColor": "#ffffff", "selectionBackground": "#474b52"
    }"##;

    #[test]
    fn parses_a_published_windows_terminal_scheme() {
        let theme = TerminalTheme::from_json(ONE_HALF_DARK).expect("scheme parses");
        assert_eq!(theme.name.as_deref(), Some("One Half Dark"));
        let colors = theme.to_colors().expect("colors resolve");
        assert_eq!(colors.bg, [0x28, 0x2c, 0x34]);
        assert_eq!(colors.palette[1], [0xe0, 0x6c, 0x75], "ANSI red");
        assert_eq!(colors.palette[13], [0xc6, 0x78, 0xdd], "bright purple");
        // The 16 base colors expand into the xterm cube and gray ramp.
        assert_eq!(colors.palette[196], [0xff, 0x00, 0x00]);
        assert_eq!(theme.cursor_rgb().unwrap(), [0xff, 0xff, 0xff]);
        assert_eq!(theme.selection_rgb().unwrap().0, [0x47, 0x4b, 0x52]);
    }

    #[test]
    fn accepts_magenta_aliases_from_other_exports() {
        let json = ONE_HALF_DARK
            .replace("\"purple\"", "\"magenta\"")
            .replace("\"brightPurple\"", "\"brightMagenta\"");
        let theme = TerminalTheme::from_json(&json).expect("aliases parse");
        assert_eq!(theme.purple, "#c678dd");
        assert_eq!(theme.bright_purple, "#c678dd");
    }

    #[test]
    fn missing_optionals_fall_back_to_the_scheme() {
        let theme = TerminalTheme::default();
        assert_eq!(theme.cursor_rgb().unwrap(), [0xff, 0xff, 0xff]);
        let (background, foreground) = theme.selection_rgb().unwrap();
        assert_eq!(background, [0xff, 0xff, 0xff], "inverted default");
        assert_eq!(foreground, [0x28, 0x2c, 0x34]);
    }

    #[test]
    fn bad_colors_name_their_field() {
        let theme = TerminalTheme {
            bright_cyan: "not-a-color".to_string(),
            ..TerminalTheme::default()
        };
        let err = theme.to_colors().expect_err("rejected");
        assert_eq!(err.field, "brightCyan");
        assert!(err.to_string().contains("not-a-color"), "{err}");

        // A missing required color is a parse error, not a silent default.
        assert!(TerminalTheme::from_json(r##"{"background":"#000000"}"##).is_err());
    }

    #[test]
    fn hex_parsing_is_strict() {
        assert_eq!(parse_hex_rgb(" #FF8000 "), Some([0xff, 0x80, 0x00]));
        assert_eq!(parse_hex_rgb("ff8000"), Some([0xff, 0x80, 0x00]));
        assert_eq!(parse_hex_rgb("#f80"), None, "3-digit form not supported");
        assert_eq!(parse_hex_rgb("#gggggg"), None);
        assert_eq!(parse_hex_rgb("#ff800000"), None, "alpha not supported");
    }
}
