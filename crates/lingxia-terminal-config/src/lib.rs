//! Terminal configuration: schema, storage and resolution.
//!
//! This layer exists because the terminal surface is implemented by the
//! framework, not by the product embedding it. If configuration were owned by
//! the platform SDKs it would be written twice — Swift for Apple, Rust for
//! Windows — with two schemas and two hot-reload paths that drift. Reading
//! and merging configuration is pure data work with no platform content, so
//! it lives here once and the engine (`lingxia-terminal`) keeps touching no
//! files at all.
//!
//! Three layers, lowest precedence first:
//!
//! 1. framework defaults (this crate),
//! 2. product defaults (`lingxia.yaml`, compiled into the app),
//! 3. user overrides (`terminal.json` in the app's state directory).

pub mod cli;
mod font;
mod theme;
mod watch;

pub use font::{BoldStyle, FontConfig, InstalledFont, ResolvedFont, resolve as resolve_font};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
pub use theme::{CursorConfig, CursorStyle, ThemeConfig, ThemeMode, ThemeSource, ThemeStore};
pub use watch::{ConfigWatcher, watch};

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
#[serde(rename_all = "camelCase", default)]
pub struct TerminalConfig {
    pub font: FontConfig,
    pub theme: ThemeConfig,
}

impl TerminalConfig {
    pub fn path(app_data_dir: &Path) -> PathBuf {
        lingxia_app_context::app_state_file(app_data_dir, CONFIG_FILE)
    }

    /// Load the user's configuration on top of the product defaults.
    ///
    /// A missing file is not an error — it is the common case. A malformed
    /// one is reported, and the caller still gets a usable configuration.
    pub fn load(
        app_data_dir: &Path,
        product_defaults: &serde_json::Value,
    ) -> (Self, Option<ConfigError>) {
        let path = Self::path(app_data_dir);
        let user = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(value) => value,
                Err(error) => {
                    return (
                        Self::from_defaults(product_defaults),
                        Some(ConfigError::Invalid {
                            path,
                            reason: error.to_string(),
                        }),
                    );
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => serde_json::Value::Null,
            Err(error) => {
                return (
                    Self::from_defaults(product_defaults),
                    Some(ConfigError::Io(error)),
                );
            }
        };

        let mut merged = product_defaults.clone();
        merge(&mut merged, &user);
        match serde_json::from_value::<Self>(merged) {
            Ok(config) => (config, None),
            Err(error) => (
                Self::from_defaults(product_defaults),
                Some(ConfigError::Invalid {
                    path,
                    reason: error.to_string(),
                }),
            ),
        }
    }

    fn from_defaults(product_defaults: &serde_json::Value) -> Self {
        serde_json::from_value(product_defaults.clone()).unwrap_or_default()
    }

    /// Write the user's configuration.
    ///
    /// Written to a sibling temporary file and renamed, so a crash mid-write
    /// cannot leave a half-written config that fails to parse on next launch.
    pub fn save(&self, app_data_dir: &Path) -> Result<(), ConfigError> {
        let path = Self::path(app_data_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|error| ConfigError::Invalid {
            path: path.clone(),
            reason: error.to_string(),
        })?;
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, text.as_bytes())?;
        std::fs::rename(&temporary, &path)?;
        Ok(())
    }
}

/// Recursively overlay `overlay` onto `base`. Objects merge key by key;
/// everything else replaces, so a user's `font.family` list wholly replaces
/// the product's rather than appending to it.
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

    fn defaults() -> serde_json::Value {
        serde_json::json!({
            "font": { "family": ["Product Mono"], "size": 14.0 },
            "theme": { "dark": "product-dark" }
        })
    }

    #[test]
    fn missing_file_yields_the_product_defaults() {
        let dir = tempfile::tempdir().expect("temp dir");
        let (config, error) = TerminalConfig::load(dir.path(), &defaults());
        assert!(error.is_none(), "a missing file is the common case");
        assert_eq!(config.font.family, vec!["Product Mono".to_string()]);
        assert_eq!(config.font.size, 14.0);
        assert_eq!(config.theme.dark, "product-dark");
        // Untouched fields still come from the framework defaults.
        assert!(config.font.ligatures);
    }

    #[test]
    fn user_file_overlays_field_by_field() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = TerminalConfig::path(dir.path());
        std::fs::create_dir_all(path.parent().expect("state dir")).expect("mkdir");
        std::fs::write(&path, r#"{"font":{"size":16.0,"ligatures":false}}"#).expect("write");

        let (config, error) = TerminalConfig::load(dir.path(), &defaults());
        assert!(error.is_none());
        assert_eq!(config.font.size, 16.0, "user wins");
        assert!(!config.font.ligatures);
        assert_eq!(
            config.font.family,
            vec!["Product Mono".to_string()],
            "untouched keys keep the product default"
        );
        assert_eq!(config.theme.dark, "product-dark");
    }

    #[test]
    fn a_family_list_replaces_rather_than_appends() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = TerminalConfig::path(dir.path());
        std::fs::create_dir_all(path.parent().expect("state dir")).expect("mkdir");
        std::fs::write(&path, r#"{"font":{"family":["Only This"]}}"#).expect("write");

        let (config, _) = TerminalConfig::load(dir.path(), &defaults());
        assert_eq!(config.font.family, vec!["Only This".to_string()]);
    }

    #[test]
    fn a_broken_file_reports_and_still_returns_a_usable_config() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = TerminalConfig::path(dir.path());
        std::fs::create_dir_all(path.parent().expect("state dir")).expect("mkdir");
        std::fs::write(&path, "{ not json").expect("write");

        let (config, error) = TerminalConfig::load(dir.path(), &defaults());
        let error = error.expect("the failure is surfaced, not swallowed");
        assert!(matches!(error, ConfigError::Invalid { .. }), "{error}");
        assert_eq!(
            config.font.size, 14.0,
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

        let (loaded, error) = TerminalConfig::load(dir.path(), &serde_json::json!({}));
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
}
