//! Theme selection and lookup.
//!
//! A handful of schemes are built in so the terminal is usable out of the box,
//! and anything else is imported into the app's `themes/` directory — one
//! scheme per file, the convention every other terminal already uses. Shipping
//! hundreds would mean carrying each scheme's own license and turning the
//! picker into a haystack; shipping one would make importing everybody's first
//! chore.

use lingxia_terminal::TerminalTheme;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Directory of imported themes, inside the app's state directory.
const THEME_DIR: &str = "themes";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
    /// Follow the OS appearance.
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CursorStyle {
    #[default]
    Block,
    Bar,
    Underline,
    BlockHollow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CursorConfig {
    pub style: CursorStyle,
    pub blink: bool,
}

impl Default for CursorConfig {
    fn default() -> Self {
        Self {
            style: CursorStyle::Block,
            blink: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ThemeConfig {
    pub mode: ThemeMode,
    /// Theme used in light appearance; both are named so following the system
    /// works without further configuration.
    pub light: String,
    pub dark: String,
    /// Background opacity, 0.0–1.0.
    pub opacity: f32,
    pub cursor: CursorConfig,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            mode: ThemeMode::System,
            light: "lingxia-light".to_string(),
            dark: "lingxia-dark".to_string(),
            opacity: 1.0,
            cursor: CursorConfig::default(),
        }
    }
}

impl ThemeConfig {
    /// The theme name for the current appearance.
    pub fn selected(&self, system_is_dark: bool) -> &str {
        match self.mode {
            ThemeMode::Light => &self.light,
            ThemeMode::Dark => &self.dark,
            ThemeMode::System if system_is_dark => &self.dark,
            ThemeMode::System => &self.light,
        }
    }
}

/// Where a theme came from, which is what `term theme --list` shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeSource {
    BuiltIn,
    Imported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeEntry {
    pub name: String,
    pub source: ThemeSource,
}

/// Themes available to an app: the built-ins plus whatever was imported.
pub struct ThemeStore {
    directory: PathBuf,
}

impl ThemeStore {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            directory: lingxia_app_context::app_state_file(app_data_dir, THEME_DIR),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Every selectable theme, imported ones first: a user who imports a name
    /// that collides with a built-in means to override it.
    pub fn list(&self) -> Vec<ThemeEntry> {
        let mut entries: Vec<ThemeEntry> = self
            .imported_names()
            .into_iter()
            .map(|name| ThemeEntry {
                name,
                source: ThemeSource::Imported,
            })
            .collect();
        for (name, _) in BUILT_IN {
            if !entries.iter().any(|entry| entry.name == *name) {
                entries.push(ThemeEntry {
                    name: name.to_string(),
                    source: ThemeSource::BuiltIn,
                });
            }
        }
        entries
    }

    /// Resolve a theme by name, imported taking precedence over built-in.
    pub fn get(&self, name: &str) -> Option<TerminalTheme> {
        if let Some(theme) = self.read_imported(name) {
            return Some(theme);
        }
        BUILT_IN
            .iter()
            .find(|(built_in, _)| *built_in == name)
            .and_then(|(_, json)| TerminalTheme::from_json(json).ok())
    }

    /// Store an imported scheme under `name`.
    pub fn import(&self, name: &str, theme: &TerminalTheme) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(&self.directory)?;
        let path = self.directory.join(format!("{name}.json"));
        let text = serde_json::to_string_pretty(theme)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        std::fs::write(&path, text)?;
        Ok(path)
    }

    fn imported_names(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.directory) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    return None;
                }
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .map(ToOwned::to_owned)
            })
            .collect();
        names.sort();
        names
    }

    fn read_imported(&self, name: &str) -> Option<TerminalTheme> {
        let path = self.directory.join(format!("{name}.json"));
        let text = std::fs::read_to_string(path).ok()?;
        TerminalTheme::from_json(&text).ok()
    }
}

/// Parse a scheme from a file.
///
/// Two shapes cover what people actually have. The JSON is this crate's own —
/// the Windows Terminal scheme shape, which every published collection emits.
/// The other is the `name: value` text shared by Xresources and kitty, where
/// only the key spelling differs and a colon is optional.
pub fn parse_scheme(text: &str) -> Result<TerminalTheme, String> {
    if let Ok(theme) = TerminalTheme::from_json(text) {
        return Ok(theme);
    }
    let mut fields: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('!') || line.starts_with("//") {
            continue;
        }
        // `*.color0: #073642`, `URxvt*background: #fff`, `color0 #073642`.
        let (key, value) = match line.split_once(':') {
            Some((key, value)) => (key, value),
            None => line
                .split_once(char::is_whitespace)
                .ok_or_else(String::new)?,
        };
        let key = key.rsplit(['*', '.']).next().unwrap_or(key).trim();
        let value = value.split_whitespace().next().unwrap_or("");
        if value.starts_with('#') && !key.is_empty() {
            fields.insert(key.to_ascii_lowercase(), value.to_string());
        }
    }

    let take = |names: &[&str]| -> Option<String> {
        names.iter().find_map(|name| fields.get(*name).cloned())
    };
    let color = |index: usize| take(&[&format!("color{index}")]);
    let mut theme = TerminalTheme {
        background: take(&["background"]).ok_or("no background color")?,
        foreground: take(&["foreground"]).ok_or("no foreground color")?,
        cursor_color: take(&["cursorcolor", "cursor"]),
        selection_background: take(&["selection_background", "selectionbackground"]),
        selection_foreground: take(&["selection_foreground", "selectionforeground"]),
        ..TerminalTheme::default()
    };
    let slots = [
        &mut theme.black,
        &mut theme.red,
        &mut theme.green,
        &mut theme.yellow,
        &mut theme.blue,
        &mut theme.purple,
        &mut theme.cyan,
        &mut theme.white,
        &mut theme.bright_black,
        &mut theme.bright_red,
        &mut theme.bright_green,
        &mut theme.bright_yellow,
        &mut theme.bright_blue,
        &mut theme.bright_purple,
        &mut theme.bright_cyan,
        &mut theme.bright_white,
    ];
    let mut found = 0;
    for (index, slot) in slots.into_iter().enumerate() {
        if let Some(value) = color(index) {
            *slot = value;
            found += 1;
        }
    }
    if found < 8 {
        return Err(format!("only {found} of the 16 ANSI colors were present"));
    }
    theme.name = None;
    Ok(theme)
}

/// Built-in schemes, all written for this project.
///
/// Deliberately not the famous ones: every published scheme is someone's work
/// under someone's license, and redistributing a handful of them means
/// carrying and honouring each of those — a legal obligation, not a feature.
/// Users who want a well-known scheme import it in one command, which is both
/// compliant and unlimited in choice.
///
/// At least one light and one dark are required for `mode: system` to work
/// untouched; dim and high-contrast cover the two rooms people actually
/// complain about.
const BUILT_IN: &[(&str, &str)] = &[
    ("lingxia-dark", LINGXIA_DARK),
    ("lingxia-light", LINGXIA_LIGHT),
    ("lingxia-dim", LINGXIA_DIM),
    ("lingxia-contrast", LINGXIA_CONTRAST),
];

const LINGXIA_DARK: &str = r##"{
  "name": "LingXia Dark",
  "background": "#282c34", "foreground": "#e6e6e6",
  "cursorColor": "#e6e6e6", "selectionBackground": "#3e4451",
  "black": "#1d1f21", "red": "#cc6666", "green": "#b5bd68", "yellow": "#f0c674",
  "blue": "#81a2be", "purple": "#b294bb", "cyan": "#8abeb7", "white": "#c5c8c6",
  "brightBlack": "#666666", "brightRed": "#d54e53", "brightGreen": "#b9ca4a",
  "brightYellow": "#e7c547", "brightBlue": "#7aa6da", "brightPurple": "#c397d8",
  "brightCyan": "#70c0b1", "brightWhite": "#eaeaea"
}"##;

/// On a light background "bright" has to mean *deeper*, not lighter. The
/// obvious light scheme takes a dark scheme's bright ramp as-is, and then
/// every error a shell prints — bright red — arrives as pale salmon on white.
const LINGXIA_LIGHT: &str = r##"{
  "name": "LingXia Light",
  "background": "#fafafa", "foreground": "#2b2d33",
  "cursorColor": "#2b2d33", "selectionBackground": "#cfd6e4",
  "black": "#383a42", "red": "#c02128", "green": "#1f7a3d", "yellow": "#8a6100",
  "blue": "#0060b0", "purple": "#8b1a89", "cyan": "#00707f", "white": "#f0f0f0",
  "brightBlack": "#5c6370", "brightRed": "#961218", "brightGreen": "#155c2c",
  "brightYellow": "#6b4b00", "brightBlue": "#004a89", "brightPurple": "#6b1069",
  "brightCyan": "#005661", "brightWhite": "#ffffff"
}"##;

/// Muted, for long sessions and dim rooms: the same hues as the dark scheme
/// with the saturation taken out, so nothing on screen is the brightest thing
/// in the room.
const LINGXIA_DIM: &str = r##"{
  "name": "LingXia Dim",
  "background": "#1b1e24", "foreground": "#b9c0cc",
  "cursorColor": "#b9c0cc", "selectionBackground": "#2f3540",
  "black": "#20242c", "red": "#a4626a", "green": "#8f9f74", "yellow": "#bfa76a",
  "blue": "#6f8ba4", "purple": "#9481a4", "cyan": "#749f9a", "white": "#a8b0bd",
  "brightBlack": "#525a68", "brightRed": "#b87179", "brightGreen": "#9db083",
  "brightYellow": "#cfb87a", "brightBlue": "#7f9db8", "brightPurple": "#a691b8",
  "brightCyan": "#83b0aa", "brightWhite": "#ccd3de"
}"##;

/// High contrast, for accessibility and bright rooms. Foreground and
/// background sit far apart on purpose, and every ANSI color is chosen to
/// stay legible against the background rather than to look subtle.
const LINGXIA_CONTRAST: &str = r##"{
  "name": "LingXia High Contrast",
  "background": "#000000", "foreground": "#ffffff",
  "cursorColor": "#ffffff", "selectionBackground": "#3a3a3a",
  "black": "#000000", "red": "#ff5f5f", "green": "#5fff5f", "yellow": "#ffff5f",
  "blue": "#5fafff", "purple": "#ff5fff", "cyan": "#5fffff", "white": "#e4e4e4",
  "brightBlack": "#767676", "brightRed": "#ff8787", "brightGreen": "#87ff87",
  "brightYellow": "#ffff87", "brightBlue": "#87d7ff", "brightPurple": "#ff87ff",
  "brightCyan": "#87ffff", "brightWhite": "#ffffff"
}"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_themes_parse_and_cover_both_appearances() {
        let store = ThemeStore::new(Path::new("/nonexistent"));
        let defaults = ThemeConfig::default();
        for name in [defaults.light.as_str(), defaults.dark.as_str()] {
            let theme = store
                .get(name)
                .unwrap_or_else(|| panic!("built-in {name} is selectable"));
            theme
                .to_colors()
                .unwrap_or_else(|error| panic!("built-in {name} has valid colors: {error}"));
        }
    }

    #[test]
    fn mode_selects_by_appearance() {
        let config = ThemeConfig::default();
        assert_eq!(config.selected(true), "lingxia-dark");
        assert_eq!(config.selected(false), "lingxia-light");

        let pinned = ThemeConfig {
            mode: ThemeMode::Light,
            ..ThemeConfig::default()
        };
        assert_eq!(
            pinned.selected(true),
            "lingxia-light",
            "pinned beats system"
        );
    }

    #[test]
    fn imported_themes_are_listed_and_override_built_ins() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = ThemeStore::new(dir.path());
        let theme = TerminalTheme {
            background: "#123456".to_string(),
            ..TerminalTheme::default()
        };
        store.import("lingxia-dark", &theme).expect("import");
        store.import("solarized", &theme).expect("import");

        let listed = store.list();
        assert_eq!(
            listed
                .iter()
                .find(|entry| entry.name == "lingxia-dark")
                .map(|entry| entry.source),
            Some(ThemeSource::Imported),
            "an imported name shadows the built-in"
        );
        assert!(listed.iter().any(|entry| entry.name == "solarized"));
        assert!(listed.iter().any(|entry| entry.name == "lingxia-light"));

        assert_eq!(
            store.get("lingxia-dark").expect("resolves").background,
            "#123456"
        );
    }

    #[test]
    fn every_built_in_scheme_is_valid() {
        let store = ThemeStore::new(Path::new("/nonexistent"));
        for entry in store.list() {
            let theme = store
                .get(&entry.name)
                .unwrap_or_else(|| panic!("{} is selectable", entry.name));
            theme
                .to_colors()
                .unwrap_or_else(|error| panic!("{} has valid colors: {error}", entry.name));
        }
    }

    #[test]
    fn xresources_and_kitty_text_import() {
        let xresources = "\
! a comment
*.background: #002b36
*.foreground: #839496
*.color0:  #073642
*.color1:  #dc322f
*.color2:  #859900
*.color3:  #b58900
*.color4:  #268bd2
*.color5:  #d33682
*.color6:  #2aa198
*.color7:  #eee8d5
";
        let theme = parse_scheme(xresources).expect("xresources parses");
        assert_eq!(theme.background, "#002b36");
        assert_eq!(theme.red, "#dc322f");
        // Slots the file did not mention keep a sane default rather than
        // rendering as nothing.
        assert_eq!(theme.bright_white, TerminalTheme::default().bright_white);

        let kitty = xresources.replace("*.", "").replace(':', " ");
        assert_eq!(
            parse_scheme(&kitty).expect("kitty parses").background,
            "#002b36",
            "the same text without colons is kitty's spelling"
        );
    }

    #[test]
    fn text_without_enough_colors_is_rejected() {
        let error = parse_scheme("*.background: #000000\n*.foreground: #ffffff\n")
            .expect_err("not a scheme");
        assert!(error.contains("ANSI colors"), "{error}");
        assert!(parse_scheme("hello").is_err());
    }

    #[test]
    fn unknown_names_resolve_to_nothing() {
        let store = ThemeStore::new(Path::new("/nonexistent"));
        assert!(store.get("no-such-theme").is_none());
    }

    /// WCAG relative luminance, which is what a contrast ratio is built from.
    fn relative_luminance(color: &str) -> f32 {
        let rgb = lingxia_terminal::parse_hex_rgb(color).expect("hex");
        let channel = |value: u8| {
            let value = f32::from(value) / 255.0;
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2])
    }

    fn contrast(a: &str, b: &str) -> f32 {
        let (a, b) = (relative_luminance(a), relative_luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// Every color a program actually prints text in has to be legible against
    /// the scheme's own background.
    ///
    /// The light scheme shipped with a dark scheme's bright ramp, so every
    /// error a shell printed — bright red — arrived as pale salmon on white.
    /// `black`/`white` and their bright forms are exempt: sitting at the
    /// background's own end of the ramp is their job.
    #[test]
    fn every_built_in_color_is_legible_on_its_own_background() {
        let chromatic = [
            "red",
            "green",
            "yellow",
            "blue",
            "purple",
            "cyan",
            "brightRed",
            "brightGreen",
            "brightYellow",
            "brightBlue",
            "brightPurple",
            "brightCyan",
            "foreground",
        ];
        for (name, source) in BUILT_IN {
            let theme: serde_json::Value = serde_json::from_str(source).expect(name);
            let background = theme["background"].as_str().expect("background");
            for key in chromatic {
                let color = theme[key]
                    .as_str()
                    .unwrap_or_else(|| panic!("{name}.{key}"));
                let ratio = contrast(color, background);
                assert!(
                    ratio >= 3.2,
                    "{name}.{key} ({color}) is {ratio:.2}:1 on {background}"
                );
            }
        }
    }

}
