//! User-level CLI config: `~/.lingxia/cli/config.toml`.
//!
//! Machine-wide defaults for the CLI itself, kept separate from a project's
//! `lingxia.yaml` (project identity) and `store/credentials.toml` (secrets).
//! Today it carries the package upload server so lxapp projects — which have
//! no `lingxia.yaml` — can publish without repeating `--lingxia-server`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::EnvVersion;

const LINGXIA_DIR: &str = ".lingxia";
const CLI_DIR: &str = "cli";
const CONFIG_FILE: &str = "config.toml";

/// The whole `config.toml` (one table per area).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CliConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish: Option<PublishConfig>,
}

/// `[publish]` — defaults for `lingxia publish`. Top-level `token`/`server`
/// are the defaults; a `[publish.<env>]` table overrides them for that env,
/// selected by the package's `--env`/`--channel`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishConfig {
    /// Default bearer token (used when `--token` is omitted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Default upload server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer: Option<EnvPublish>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<EnvPublish>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<EnvPublish>,
}

/// `[publish.<env>]` — token + server for one env (each env is a distinct
/// backend that usually needs its own credentials).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvPublish {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
}

impl PublishConfig {
    fn env(&self, version: EnvVersion) -> Option<&EnvPublish> {
        match version {
            EnvVersion::Developer => self.developer.as_ref(),
            EnvVersion::Preview => self.preview.as_ref(),
            EnvVersion::Release => self.release.as_ref(),
        }
    }

    /// The upload server for `version`: the per-env entry if set, else the
    /// section default. Empty strings are treated as unset.
    pub fn lingxia_server_for(&self, version: EnvVersion) -> Option<&str> {
        clean(
            self.env(version)
                .and_then(|e| e.server.as_deref())
                .or(self.server.as_deref()),
        )
    }

    /// The bearer token for `version`, resolved like the server.
    pub fn token_for(&self, version: EnvVersion) -> Option<&str> {
        clean(
            self.env(version)
                .and_then(|e| e.token.as_deref())
                .or(self.token.as_deref()),
        )
    }
}

fn clean(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

/// Path to `~/.lingxia/cli/config.toml`.
pub fn config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(LINGXIA_DIR).join(CLI_DIR).join(CONFIG_FILE))
}

impl CliConfig {
    /// Load the config file, or an empty config when it does not exist.
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("Failed to parse {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_empty() {
        let cfg = CliConfig::load_from(Path::new("/no/such/config.toml")).unwrap();
        assert!(cfg.publish.is_none());
    }

    #[test]
    fn token_default_applies_to_all_envs() {
        let cfg: CliConfig = toml::from_str(
            r#"
            [publish]
            token = "lx_abc"
        "#,
        )
        .unwrap();
        let publish = cfg.publish.unwrap();
        assert_eq!(publish.token_for(EnvVersion::Developer), Some("lx_abc"));
        assert_eq!(publish.token_for(EnvVersion::Release), Some("lx_abc"));
    }

    #[test]
    fn single_server_applies_to_all_envs() {
        let cfg: CliConfig = toml::from_str(
            r#"
            [publish]
            server = "https://prod.example.com"
        "#,
        )
        .unwrap();
        let publish = cfg.publish.unwrap();
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Developer),
            Some("https://prod.example.com")
        );
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Release),
            Some("https://prod.example.com")
        );
    }

    #[test]
    fn per_env_table_overrides_defaults() {
        let cfg: CliConfig = toml::from_str(
            r#"
            [publish]
            token = "lx_prod"
            server = "https://prod.example.com"

            [publish.developer]
            token = "lx_dev"
            server = "http://localhost:8080"
        "#,
        )
        .unwrap();
        let publish = cfg.publish.unwrap();
        // developer uses its own table
        assert_eq!(publish.token_for(EnvVersion::Developer), Some("lx_dev"));
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Developer),
            Some("http://localhost:8080")
        );
        // release falls back to the section defaults
        assert_eq!(publish.token_for(EnvVersion::Release), Some("lx_prod"));
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Release),
            Some("https://prod.example.com")
        );
    }

    #[test]
    fn empty_string_is_unset() {
        let cfg: CliConfig = toml::from_str(
            r#"
            [publish]
            server = ""
        "#,
        )
        .unwrap();
        assert_eq!(
            cfg.publish.unwrap().lingxia_server_for(EnvVersion::Release),
            None
        );
    }
}
