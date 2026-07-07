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

    /// Set the publish server and/or token for `env`. A `None` env writes the
    /// top-level defaults; an env writes the matching `[publish.<env>]` table.
    /// `None` arguments are left unchanged.
    pub fn set_publish(
        &mut self,
        env: Option<EnvVersion>,
        server: Option<String>,
        token: Option<String>,
    ) {
        let publish = self.publish.get_or_insert_with(PublishConfig::default);
        let target = match env {
            None => EnvPublishMut {
                server: &mut publish.server,
                token: &mut publish.token,
            },
            Some(EnvVersion::Developer) => publish
                .developer
                .get_or_insert_with(EnvPublish::default)
                .as_mut(),
            Some(EnvVersion::Preview) => publish
                .preview
                .get_or_insert_with(EnvPublish::default)
                .as_mut(),
            Some(EnvVersion::Release) => publish
                .release
                .get_or_insert_with(EnvPublish::default)
                .as_mut(),
        };
        if let Some(server) = server {
            *target.server = Some(server);
        }
        if let Some(token) = token {
            *target.token = Some(token);
        }
    }
}

/// Mutable view over an env's server/token, so `set_publish` handles the
/// top-level defaults and the per-env tables uniformly.
struct EnvPublishMut<'a> {
    server: &'a mut Option<String>,
    token: &'a mut Option<String>,
}

impl EnvPublish {
    fn as_mut(&mut self) -> EnvPublishMut<'_> {
        EnvPublishMut {
            server: &mut self.server,
            token: &mut self.token,
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
    fn set_publish_writes_top_level_defaults_when_env_is_none() {
        let mut cfg = CliConfig::default();
        cfg.set_publish(
            None,
            Some("https://api.example.com".to_string()),
            Some("lx_tok".to_string()),
        );
        let publish = cfg.publish.unwrap();
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Release),
            Some("https://api.example.com")
        );
        assert_eq!(publish.token_for(EnvVersion::Developer), Some("lx_tok"));
        assert!(publish.developer.is_none());
    }

    #[test]
    fn set_publish_scopes_to_env_table_and_preserves_others() {
        let mut cfg = CliConfig::default();
        cfg.set_publish(
            Some(EnvVersion::Release),
            Some("https://prod.example.com".to_string()),
            Some("lx_prod".to_string()),
        );
        cfg.set_publish(
            Some(EnvVersion::Developer),
            Some("http://localhost:8080".to_string()),
            Some("lx_dev".to_string()),
        );
        let publish = cfg.publish.unwrap();
        // developer edit must not clobber the release table.
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Release),
            Some("https://prod.example.com")
        );
        assert_eq!(publish.token_for(EnvVersion::Release), Some("lx_prod"));
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Developer),
            Some("http://localhost:8080")
        );
        assert_eq!(publish.token_for(EnvVersion::Developer), Some("lx_dev"));
    }

    #[test]
    fn set_publish_leaves_unspecified_field_unchanged() {
        let mut cfg = CliConfig::default();
        cfg.set_publish(None, Some("https://a.example.com".to_string()), None);
        cfg.set_publish(None, None, Some("lx_tok".to_string()));
        let publish = cfg.publish.unwrap();
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Release),
            Some("https://a.example.com")
        );
        assert_eq!(publish.token_for(EnvVersion::Release), Some("lx_tok"));
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli").join("config.toml");
        let mut cfg = CliConfig::default();
        cfg.set_publish(
            Some(EnvVersion::Preview),
            Some("https://preview.example.com".to_string()),
            Some("lx_preview".to_string()),
        );
        cfg.save_to(&path).unwrap();

        let loaded = CliConfig::load_from(&path).unwrap();
        let publish = loaded.publish.unwrap();
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Preview),
            Some("https://preview.example.com")
        );
        assert_eq!(publish.token_for(EnvVersion::Preview), Some("lx_preview"));
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
