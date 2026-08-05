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

/// Built-in schemes. Kept few and deliberately ours: every published scheme
/// carries its own author and license, and vendoring a few hundred of them is
/// a licensing obligation, not a feature. At least one light and one dark are
/// required for `mode: system` to work untouched.
const BUILT_IN: &[(&str, &str)] = &[
    ("lingxia-dark", LINGXIA_DARK),
    ("lingxia-light", LINGXIA_LIGHT),
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

const LINGXIA_LIGHT: &str = r##"{
  "name": "LingXia Light",
  "background": "#fafafa", "foreground": "#383a42",
  "cursorColor": "#383a42", "selectionBackground": "#d0d0d0",
  "black": "#383a42", "red": "#e45649", "green": "#50a14f", "yellow": "#c18401",
  "blue": "#0184bc", "purple": "#a626a4", "cyan": "#0997b3", "white": "#fafafa",
  "brightBlack": "#4f525e", "brightRed": "#e06c75", "brightGreen": "#98c379",
  "brightYellow": "#e5c07b", "brightBlue": "#61afef", "brightPurple": "#c678dd",
  "brightCyan": "#56b6c2", "brightWhite": "#ffffff"
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
    fn unknown_names_resolve_to_nothing() {
        let store = ThemeStore::new(Path::new("/nonexistent"));
        assert!(store.get("no-such-theme").is_none());
    }
}
