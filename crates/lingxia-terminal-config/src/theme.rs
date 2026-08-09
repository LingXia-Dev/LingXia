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
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
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
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeDetails {
    pub name: String,
    pub source: ThemeSource,
    pub scheme: TerminalTheme,
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

    /// Every selectable theme with its resolved colors for settings swatches.
    pub fn list_with_schemes(&self) -> Vec<ThemeDetails> {
        self.list()
            .into_iter()
            .filter_map(|entry| {
                self.get(&entry.name).map(|scheme| ThemeDetails {
                    name: entry.name,
                    source: entry.source,
                    scheme,
                })
            })
            .collect()
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
        validate_theme_name(name)?;
        std::fs::create_dir_all(&self.directory)?;
        let path = self.directory.join(format!("{name}.json"));
        let text = serde_json::to_string_pretty(theme)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, text)?;
        std::fs::rename(&temporary, &path)?;
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

fn validate_theme_name(name: &str) -> std::io::Result<()> {
    let trimmed = name.trim();
    let invalid = trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.len() > 96
        || trimmed.ends_with('.')
        || trimmed.ends_with(' ')
        || trimmed.chars().any(|ch| {
            ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
        });
    if invalid {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "theme name is empty or contains filesystem-reserved characters",
        ));
    }
    Ok(())
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
///
/// `white` and `brightWhite` are greys here rather than white. Programs assume
/// a dark terminal, so ANSI 7 and 15 are what they reach for when they mean
/// "ordinary text" — PowerShell colors every command *argument* with ANSI 7.
/// Spelling them literally on a light scheme makes that text disappear.
const LINGXIA_LIGHT: &str = r##"{
  "name": "LingXia Light",
  "background": "#fafafa", "foreground": "#2b2d33",
  "cursorColor": "#2b2d33", "selectionBackground": "#cfd6e4",
  "black": "#383a42", "red": "#c02128", "green": "#1f7a3d", "yellow": "#8a6100",
  "blue": "#0060b0", "purple": "#8b1a89", "cyan": "#00707f", "white": "#5a636e",
  "brightBlack": "#5c6370", "brightRed": "#961218", "brightGreen": "#155c2c",
  "brightYellow": "#6b4b00", "brightBlue": "#004a89", "brightPurple": "#6b1069",
  "brightCyan": "#005661", "brightWhite": "#3f4650"
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

/// Colors for the chrome around a terminal, derived from the terminal's own.
///
/// The tab strip belongs to the terminal, not to the app: the `+` on it opens
/// another PTY, and every terminal that has one tints it with the active
/// scheme. Deriving it here rather than fixing it per platform means a theme
/// change moves the whole pane, instead of leaving a hardcoded stripe
/// attached to a repainted grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceChrome {
    /// The pane body — the terminal's own background.
    pub surface: u32,
    /// The tab strip behind it.
    pub header: u32,
    /// Hairline between the two.
    pub separator: u32,
    pub text: u32,
    pub text_muted: u32,
}

impl Default for SurfaceChrome {
    fn default() -> Self {
        Self::derive(&TerminalTheme::default())
    }
}

impl SurfaceChrome {
    pub fn derive(theme: &TerminalTheme) -> Self {
        let surface = rgb(&theme.background).unwrap_or(0x282c34);
        let text = rgb(&theme.foreground).unwrap_or(0xffffff);
        // The strip reads as further back than the body, which means darker —
        // under a light scheme too, where a slightly grey bar is what every
        // light terminal theme does. A background with nothing left to darken
        // lifts instead, so pure black still shows an edge.
        let header = if luminance(surface) < 0.06 {
            mix(surface, 0xffffff, 0.10)
        } else {
            mix(surface, 0x000000, 0.22)
        };
        Self {
            surface,
            header,
            separator: mix(header, text, 0.12),
            text,
            text_muted: mix(text, header, 0.45),
        }
    }
}

fn rgb(color: &str) -> Option<u32> {
    let [r, g, b] = lingxia_terminal::parse_hex_rgb(color)?;
    Some((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b))
}

/// Perceived brightness, 0..1 — the sRGB luma weights, which is what decides
/// whether a scheme reads as light or dark.
fn luminance(color: u32) -> f32 {
    let channel = |shift: u32| ((color >> shift) & 0xff) as f32 / 255.0;
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

fn mix(color: u32, towards: u32, amount: f32) -> u32 {
    let amount = amount.clamp(0.0, 1.0);
    let blend = |shift: u32| {
        let from = ((color >> shift) & 0xff) as f32;
        let to = ((towards >> shift) & 0xff) as f32;
        ((from + (to - from) * amount).round() as u32).min(0xff) << shift
    };
    blend(16) | blend(8) | blend(0)
}

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
    fn listing_carries_schemes_and_import_names_cannot_escape() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = ThemeStore::new(dir.path());
        let themes = store.list_with_schemes();
        assert!(themes.iter().all(|entry| entry.scheme.to_colors().is_ok()));
        assert!(
            store
                .import("../outside", &TerminalTheme::default())
                .is_err()
        );
        assert!(store.import("bad:name", &TerminalTheme::default()).is_err());
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
    /// `white`/`brightWhite` are in the set on *every* scheme, light ones
    /// included: programs assume a dark terminal, so ANSI 7 and 15 are what
    /// they reach for when they mean "ordinary text". Only `black` and
    /// `brightBlack` are exempt — being the dim end of the ramp is their job,
    /// and nothing prints body text in them.
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
            "white",
            "brightWhite",
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

    fn chrome(background: &str, foreground: &str) -> SurfaceChrome {
        SurfaceChrome::derive(&TerminalTheme {
            background: background.to_string(),
            foreground: foreground.to_string(),
            ..TerminalTheme::default()
        })
    }

    /// The whole point: chrome tracks the scheme, so a light theme cannot end
    /// up with a dark strip stapled to it.
    #[test]
    fn chrome_follows_the_scheme_into_the_light() {
        let dark = chrome("#282c34", "#ffffff");
        let light = chrome("#fafafa", "#383a42");
        assert!(luminance(dark.header) < 0.5, "dark scheme, dark strip");
        assert!(luminance(light.header) > 0.5, "light scheme, light strip");
        assert_eq!(dark.surface, 0x282c34);
        assert_eq!(light.surface, 0xfafafa);
    }

    /// The strip must be distinguishable from the body at both extremes —
    /// darkening pure black would leave no edge at all.
    #[test]
    fn the_strip_is_always_visible_against_the_body() {
        for (background, foreground) in [
            ("#000000", "#ffffff"),
            ("#282c34", "#ffffff"),
            ("#fafafa", "#383a42"),
            ("#ffffff", "#000000"),
        ] {
            let chrome = chrome(background, foreground);
            assert_ne!(
                chrome.header, chrome.surface,
                "{background} left no edge between strip and body"
            );
            assert_ne!(chrome.text_muted, chrome.text, "{background}");
        }
    }

    #[test]
    fn an_unparseable_color_falls_back_rather_than_panicking() {
        let chrome = chrome("not a color", "#ffffff");
        assert_eq!(chrome.surface, 0x282c34);
    }
}
