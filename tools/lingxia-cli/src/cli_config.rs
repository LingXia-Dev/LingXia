//! User-level CLI config: `~/.lingxia/cli/config.toml`.
//!
//! Machine-wide defaults for the CLI itself, kept separate from a project's
//! `lingxia.yaml` (project identity) and the credential wallet (secrets).
//! Today it carries the package upload server so lxapp projects — which have
//! no `lingxia.yaml` — can publish without repeating `--lingxia-server`.
//! Publish tokens are NOT config: they live in the wallet, keyed by the
//! canonical server URL + env.
//!
//! Env-dependent values follow the same shape as `lingxia.yaml`'s
//! `app.lingxiaServer` and the runner config: a value is either a scalar
//! (applies to every env) or an env-keyed map (explicit per env, no fallback
//! between the two forms).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::EnvVersion;

const CLI_DIR: &str = "cli";
const CONFIG_FILE: &str = "config.toml";

/// The whole `config.toml` (one table per area).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CliConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish: Option<PublishConfig>,
}

/// `[publish]` — defaults for `lingxia publish`, selected by the package's
/// `--env`/`--channel`:
///
/// ```toml
/// [publish.lingxiaServer]                   # per env
/// developer = "http://localhost:8080"
/// release = "https://prod.example.com"
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishConfig {
    /// Upload server. Named like everywhere else (`app.lingxiaServer` in
    /// lxapp.json/lingxia.yaml, runner config, `--lingxia-server`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lingxia_server: Option<EnvValue>,
}

/// A config value that is either one scalar for every env or an explicit
/// per-env map — the same shape as `lingxia.yaml`'s `app.lingxiaServer`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnvValue {
    Single(String),
    PerEnv(PerEnv),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PerEnv {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
}

impl EnvValue {
    /// The value that applies to `version`, or `None` when the map form does
    /// not list that env. `Single` applies to every env.
    fn for_env(&self, version: EnvVersion) -> Option<&str> {
        match self {
            EnvValue::Single(value) => Some(value.as_str()),
            EnvValue::PerEnv(per) => match version {
                EnvVersion::Developer => per.developer.as_deref(),
                EnvVersion::Preview => per.preview.as_deref(),
                EnvVersion::Release => per.release.as_deref(),
            },
        }
    }

    /// Set the value for `version`, converting a `Single` into an explicit
    /// map that keeps the old scalar for the other envs (so scoping one env
    /// never silently changes the rest).
    fn set_env(&mut self, version: EnvVersion, value: String) {
        let mut per = match self {
            EnvValue::Single(old) => PerEnv {
                developer: Some(old.clone()),
                preview: Some(old.clone()),
                release: Some(old.clone()),
            },
            EnvValue::PerEnv(per) => per.clone(),
        };
        match version {
            EnvVersion::Developer => per.developer = Some(value),
            EnvVersion::Preview => per.preview = Some(value),
            EnvVersion::Release => per.release = Some(value),
        }
        *self = EnvValue::PerEnv(per);
    }

    fn set_env_or_new(slot: &mut Option<EnvValue>, version: EnvVersion, value: String) {
        match slot {
            Some(existing) => existing.set_env(version, value),
            None => {
                let mut fresh = EnvValue::PerEnv(PerEnv::default());
                fresh.set_env(version, value);
                *slot = Some(fresh);
            }
        }
    }
}

impl PublishConfig {
    /// The upload server for `version`. Empty strings are unset.
    pub fn lingxia_server_for(&self, version: EnvVersion) -> Option<&str> {
        clean(
            self.lingxia_server
                .as_ref()
                .and_then(|v| v.for_env(version)),
        )
    }
}

fn clean(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

/// Path to `~/.lingxia/cli/config.toml`.
pub fn config_path() -> Result<PathBuf> {
    Ok(crate::state_root::lingxia_dir()?
        .join(CLI_DIR)
        .join(CONFIG_FILE))
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

    /// Write the config to `~/.lingxia/cli/config.toml` (mode 0600), creating the
    /// parent directory. Any hand-added TOML comments are lost — this file is
    /// CLI-managed.
    pub fn save(&self) -> Result<()> {
        self.save_to(&config_path()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        let text =
            toml::to_string_pretty(self).context("Failed to serialize CLI config to TOML")?;
        fs::write(path, &text).with_context(|| format!("Failed to write {}", path.display()))?;
        set_secret_file_mode(path);
        Ok(())
    }

    /// Set the publish server default. A `None` env writes a scalar (every
    /// env); an env scopes to that env's map entry, materializing a previous
    /// scalar so the other envs keep their value.
    pub fn set_publish_server(&mut self, env: Option<EnvVersion>, server: String) {
        let publish = self.publish.get_or_insert_with(PublishConfig::default);
        match env {
            None => publish.lingxia_server = Some(EnvValue::Single(server)),
            Some(env) => EnvValue::set_env_or_new(&mut publish.lingxia_server, env, server),
        }
    }
}

#[cfg(unix)]
fn set_secret_file_mode(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn set_secret_file_mode(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_empty() {
        let cfg = CliConfig::load_from(Path::new("/no/such/config.toml")).unwrap();
        assert!(cfg.publish.is_none());
    }

    #[test]
    fn set_server_writes_scalar_when_env_is_none() {
        let mut cfg = CliConfig::default();
        cfg.set_publish_server(None, "https://api.example.com".to_string());
        let publish = cfg.publish.unwrap();
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Release),
            Some("https://api.example.com")
        );
    }

    #[test]
    fn scoping_an_env_materializes_a_scalar_for_the_others() {
        let mut cfg = CliConfig::default();
        cfg.set_publish_server(None, "https://api.example.com".to_string());
        cfg.set_publish_server(
            Some(EnvVersion::Developer),
            "http://localhost:8080".to_string(),
        );
        let publish = cfg.publish.unwrap();
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Developer),
            Some("http://localhost:8080")
        );
        // The other envs keep the previous scalar explicitly.
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Release),
            Some("https://api.example.com")
        );
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Preview),
            Some("https://api.example.com")
        );
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli").join("config.toml");
        let mut cfg = CliConfig::default();
        cfg.set_publish_server(
            Some(EnvVersion::Preview),
            "https://preview.example.com".to_string(),
        );
        cfg.save_to(&path).unwrap();

        let loaded = CliConfig::load_from(&path).unwrap();
        let publish = loaded.publish.unwrap();
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Preview),
            Some("https://preview.example.com")
        );
    }

    #[test]
    fn env_map_is_explicit_per_env() {
        let cfg: CliConfig = toml::from_str(
            r#"
            [publish.lingxiaServer]
            developer = "http://localhost:8080"
            release = "https://prod.example.com"
        "#,
        )
        .unwrap();
        let publish = cfg.publish.unwrap();
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Release),
            Some("https://prod.example.com")
        );
        // An env the map does not list is unconfigured — no fallback.
        assert_eq!(publish.lingxia_server_for(EnvVersion::Preview), None);
    }

    #[test]
    fn empty_string_is_unset() {
        let cfg: CliConfig = toml::from_str(
            r#"
            [publish]
            lingxiaServer = ""
        "#,
        )
        .unwrap();
        assert_eq!(
            cfg.publish.unwrap().lingxia_server_for(EnvVersion::Release),
            None
        );
    }
}
