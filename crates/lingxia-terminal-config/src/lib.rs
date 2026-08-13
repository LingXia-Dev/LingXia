//! Terminal configuration: schema, storage and resolution.
//!
//! This layer exists because the terminal surface is implemented by the
//! framework, not by the product embedding it. If configuration were owned by
//! the platform SDKs it would be written twice — Swift for Apple, Rust for
//! Windows — with two schemas and two application paths that drift. Reading
//! and merging configuration is pure data work with no platform content, so
//! it lives here once and the engine (`lingxia-terminal`) keeps touching no
//! files at all.
//!
//! Two layers, lowest precedence first:
//!
//! 1. framework defaults (this crate),
//! 2. user overrides (`terminal.json` in the app's state directory).

mod font;
pub mod runtime;
mod theme;
#[cfg(target_os = "windows")]
pub mod windows;

pub use font::{FontConfig, InstalledFont, ResolvedFont, resolve as resolve_font};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
pub use theme::{
    SurfaceChrome, ThemeConfig, ThemeDetails, ThemeMode, ThemeSource, ThemeStore, parse_scheme,
};

/// Host-bundled control lxapp allowed to manage terminal settings.
pub const SETTINGS_APP_ID: &str = "app.lingxia.terminal-settings";

/// File name inside the app's state directory.
const CONFIG_FILE: &str = "terminal.json";

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    /// The file exists but could not be understood. The caller keeps running
    /// on defaults; a terminal must never be bricked by its config.
    Invalid {
        path: PathBuf,
        reason: String,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "terminal config I/O error: {error}"),
            Self::Invalid { path, reason } => {
                write!(f, "invalid terminal config at {}: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// The resolved terminal configuration.
///
/// Every field has a default, so a partial file is valid: users write only
/// what they want to change.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct TerminalConfig {
    pub font: FontConfig,
    pub theme: ThemeConfig,
}

/// The persisted configuration layers, resolved without exposing their
/// host filesystem location to JavaScript.
#[derive(Debug)]
pub struct ConfigLayers {
    /// Framework defaults.
    pub defaults: TerminalConfig,
    /// Valid user-authored fields from `terminal.json`, or an empty object when
    /// the file is absent or invalid.
    pub overrides: serde_json::Value,
    /// Configuration in effect after all valid layers are merged.
    pub value: TerminalConfig,
    /// A malformed or unreadable user file never prevents the terminal from
    /// opening, but settings can surface the warning.
    pub warning: Option<ConfigError>,
}

impl TerminalConfig {
    pub fn path(app_data_dir: &Path) -> PathBuf {
        lingxia_app_context::app_state_file(app_data_dir, CONFIG_FILE)
    }

    /// Load the user's configuration on top of the framework defaults.
    ///
    /// A missing file is not an error — it is the common case. A malformed
    /// one is reported, and the caller still gets a usable configuration.
    pub fn load(app_data_dir: &Path) -> (Self, Option<ConfigError>) {
        let layers = Self::load_layers(app_data_dir);
        (layers.value, layers.warning)
    }

    /// Load every configuration layer for settings and diagnostics.
    pub fn load_layers(app_data_dir: &Path) -> ConfigLayers {
        let path = Self::path(app_data_dir);
        let defaults = Self::default();
        let user = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(value) if value.is_object() => value,
                Ok(_) => {
                    return ConfigLayers {
                        defaults: defaults.clone(),
                        overrides: serde_json::json!({}),
                        value: defaults,
                        warning: Some(ConfigError::Invalid {
                            path,
                            reason: "terminal config must be an object".to_string(),
                        }),
                    };
                }
                Err(error) => {
                    return ConfigLayers {
                        defaults: defaults.clone(),
                        overrides: serde_json::json!({}),
                        value: defaults,
                        warning: Some(ConfigError::Invalid {
                            path,
                            reason: error.to_string(),
                        }),
                    };
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
            Err(error) => {
                return ConfigLayers {
                    defaults: defaults.clone(),
                    overrides: serde_json::json!({}),
                    value: defaults,
                    warning: Some(ConfigError::Io(error)),
                };
            }
        };

        let mut merged = serde_json::to_value(&defaults).unwrap_or_else(|_| serde_json::json!({}));
        merge(&mut merged, &user);
        match serde_json::from_value::<Self>(merged) {
            Ok(config) => match config.validate() {
                Ok(()) => ConfigLayers {
                    defaults,
                    overrides: user,
                    value: config,
                    warning: None,
                },
                Err(reason) => ConfigLayers {
                    defaults: defaults.clone(),
                    overrides: serde_json::json!({}),
                    value: defaults,
                    warning: Some(ConfigError::Invalid { path, reason }),
                },
            },
            Err(error) => ConfigLayers {
                defaults: defaults.clone(),
                overrides: serde_json::json!({}),
                value: defaults,
                warning: Some(ConfigError::Invalid {
                    path,
                    reason: error.to_string(),
                }),
            },
        }
    }

    /// Merge a partial config object onto this resolved configuration.
    pub fn with_overlay(&self, overlay: &serde_json::Value) -> Result<Self, String> {
        if !overlay.is_object() {
            return Err("terminal config overlay must be an object".to_string());
        }
        let mut merged = serde_json::to_value(self).map_err(|error| error.to_string())?;
        merge(&mut merged, overlay);
        let config: Self = serde_json::from_value(merged).map_err(|error| error.to_string())?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.font.family.is_empty() || self.font.family.iter().any(|name| name.trim().is_empty())
        {
            return Err("font.family must contain at least one non-empty family".to_string());
        }
        if !self.font.size.is_finite() || !(4.0..=96.0).contains(&self.font.size) {
            return Err("font.size must be between 4 and 96".to_string());
        }
        if !self.font.line_height.is_finite() || !(0.5..=3.0).contains(&self.font.line_height) {
            return Err("font.lineHeight must be between 0.5 and 3".to_string());
        }
        if self.theme.light.trim().is_empty() || self.theme.dark.trim().is_empty() {
            return Err("theme.light and theme.dark must name a theme".to_string());
        }
        Ok(())
    }

    /// Write the user's configuration as what it overrides, and nothing more.
    ///
    /// Writing every field would freeze today's defaults into the file. So
    /// only the fields that differ from the framework defaults are written,
    /// and a configuration that differs in nothing removes the file — which is
    /// what makes `reset` a real reset.
    ///
    /// Written to a sibling temporary file and renamed, so a crash mid-write
    /// cannot leave a half-written config that fails to parse on next launch.
    pub fn save(&self, app_data_dir: &Path) -> Result<(), ConfigError> {
        let path = Self::path(app_data_dir);
        self.validate().map_err(|reason| ConfigError::Invalid {
            path: path.clone(),
            reason,
        })?;
        let invalid = |error: serde_json::Error| ConfigError::Invalid {
            path: path.clone(),
            reason: error.to_string(),
        };
        let base = serde_json::to_value(Self::default()).map_err(invalid)?;
        let mine = serde_json::to_value(self).map_err(invalid)?;
        let overrides = difference(&mine, &base);

        if overrides.as_object().is_none_or(serde_json::Map::is_empty) {
            return match std::fs::remove_file(&path) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
                _ => Ok(()),
            };
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(&overrides).map_err(invalid)?;
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, text.as_bytes())?;
        std::fs::rename(&temporary, &path)?;
        Ok(())
    }
}

/// What `value` says that `base` does not. Objects recurse; everything else,
/// arrays included, compares whole — the same rule [`merge`] applies going the
/// other way, so a diff then a merge is the identity.
fn difference(value: &serde_json::Value, base: &serde_json::Value) -> serde_json::Value {
    let empty = || serde_json::Value::Object(serde_json::Map::new());
    match (value, base) {
        (serde_json::Value::Object(value), serde_json::Value::Object(base)) => {
            let mut out = serde_json::Map::new();
            for (key, item) in value {
                match base.get(key) {
                    Some(other) => {
                        let nested = difference(item, other);
                        if !nested.as_object().is_some_and(serde_json::Map::is_empty) {
                            out.insert(key.clone(), nested);
                        }
                    }
                    None => {
                        out.insert(key.clone(), item.clone());
                    }
                }
            }
            serde_json::Value::Object(out)
        }
        _ if value == base => empty(),
        _ => value.clone(),
    }
}

/// Recursively overlay `overlay` onto `base`. Objects merge key by key;
/// everything else replaces, so a user's `font.family` list wholly replaces
/// the default rather than appending to it.
fn merge(base: &mut serde_json::Value, overlay: &serde_json::Value) {
    match (base, overlay) {
        (serde_json::Value::Object(base), serde_json::Value::Object(overlay)) => {
            for (key, value) in overlay {
                merge(
                    base.entry(key.clone()).or_insert(serde_json::Value::Null),
                    value,
                );
            }
        }
        (base, overlay) if !overlay.is_null() => *base = overlay.clone(),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_framework_defaults() {
        let dir = tempfile::tempdir().expect("temp dir");
        let (config, error) = TerminalConfig::load(dir.path());
        assert!(error.is_none(), "a missing file is the common case");
        assert_eq!(config, TerminalConfig::default());
    }

    #[test]
    fn user_file_overlays_field_by_field() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = TerminalConfig::path(dir.path());
        std::fs::create_dir_all(path.parent().expect("state dir")).expect("mkdir");
        std::fs::write(&path, r#"{"font":{"size":16.0,"ligatures":false}}"#).expect("write");

        let (config, error) = TerminalConfig::load(dir.path());
        assert!(error.is_none());
        assert_eq!(config.font.size, 16.0, "user wins");
        assert!(!config.font.ligatures);
        assert_eq!(
            config.font.family,
            TerminalConfig::default().font.family,
            "untouched keys keep the framework default"
        );
        assert_eq!(config.theme.dark, TerminalConfig::default().theme.dark);
    }

    #[test]
    fn settings_layers_preserve_only_valid_user_overrides() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = TerminalConfig::path(dir.path());
        std::fs::create_dir_all(path.parent().expect("state dir")).expect("mkdir");
        std::fs::write(&path, r#"{"font":{"size":16.0}}"#).expect("write");

        let layers = TerminalConfig::load_layers(dir.path());
        assert!(layers.warning.is_none());
        assert_eq!(layers.defaults, TerminalConfig::default());
        assert_eq!(
            layers.overrides,
            serde_json::json!({"font": {"size": 16.0}})
        );
        assert_eq!(layers.value.font.size, 16.0);
    }

    #[test]
    fn invalid_settings_layers_hide_the_bad_payload() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = TerminalConfig::path(dir.path());
        std::fs::create_dir_all(path.parent().expect("state dir")).expect("mkdir");
        std::fs::write(&path, r#"{"font":{"size":2.0}}"#).expect("write");

        let layers = TerminalConfig::load_layers(dir.path());
        assert!(matches!(layers.warning, Some(ConfigError::Invalid { .. })));
        assert_eq!(layers.overrides, serde_json::json!({}));
        assert_eq!(layers.value, layers.defaults);
    }

    #[test]
    fn a_family_list_replaces_rather_than_appends() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = TerminalConfig::path(dir.path());
        std::fs::create_dir_all(path.parent().expect("state dir")).expect("mkdir");
        std::fs::write(&path, r#"{"font":{"family":["Only This"]}}"#).expect("write");

        let (config, _) = TerminalConfig::load(dir.path());
        assert_eq!(config.font.family, vec!["Only This".to_string()]);
    }

    #[test]
    fn a_broken_file_reports_and_still_returns_a_usable_config() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = TerminalConfig::path(dir.path());
        std::fs::create_dir_all(path.parent().expect("state dir")).expect("mkdir");
        std::fs::write(&path, "{ not json").expect("write");

        let (config, error) = TerminalConfig::load(dir.path());
        let error = error.expect("the failure is surfaced, not swallowed");
        assert!(matches!(error, ConfigError::Invalid { .. }), "{error}");
        assert_eq!(
            config.font.size,
            TerminalConfig::default().font.size,
            "a broken file must never brick the terminal"
        );
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut config = TerminalConfig::default();
        config.font.size = 15.5;
        config.theme.light = "paper".to_string();
        config.save(dir.path()).expect("save");

        let (loaded, error) = TerminalConfig::load(dir.path());
        assert!(error.is_none());
        assert_eq!(loaded.font.size, 15.5);
        assert_eq!(loaded.theme.light, "paper");
        // The temporary file used for the atomic write is gone.
        assert!(
            !TerminalConfig::path(dir.path())
                .with_extension("json.tmp")
                .exists()
        );
    }

    /// The file must say what the user changed, not what the defaults happened
    /// to be the day they changed it.
    #[test]
    fn only_the_overrides_are_written() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut config = TerminalConfig::default();
        config.font.size = 15.5;
        config.save(dir.path()).expect("save");

        let text = std::fs::read_to_string(TerminalConfig::path(dir.path())).expect("read");
        let written: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_eq!(written, serde_json::json!({ "font": { "size": 15.5 } }));
    }

    /// A configuration that overrides nothing leaves no file, which is what
    /// lets a later default reach someone who once changed something back.
    #[test]
    fn saving_the_defaults_removes_the_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut config = TerminalConfig::default();
        config.font.size = 15.5;
        config.save(dir.path()).expect("save");
        assert!(TerminalConfig::path(dir.path()).exists());

        TerminalConfig::default().save(dir.path()).expect("save");
        assert!(!TerminalConfig::path(dir.path()).exists());
    }

    #[test]
    fn partial_overlay_is_typed_and_range_checked() {
        let config = TerminalConfig::default()
            .with_overlay(&serde_json::json!({"font": {"size": 15.5}}))
            .expect("valid overlay");
        assert_eq!(config.font.size, 15.5);
        assert_eq!(config.theme, ThemeConfig::default());

        for overlay in [
            serde_json::json!({"font": {"size": 2}}),
            serde_json::json!({"font": {"unknown": true}}),
            serde_json::json!({"theme": {"unknown": true}}),
        ] {
            assert!(
                TerminalConfig::default().with_overlay(&overlay).is_err(),
                "overlay should be rejected: {overlay}"
            );
        }
    }
}
