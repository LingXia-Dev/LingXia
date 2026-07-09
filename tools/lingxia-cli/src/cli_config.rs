//! User-level CLI config: `~/.lingxia/cli/config.toml`.
//!
//! Machine-wide defaults for the CLI itself, kept separate from a project's
//! `lingxia.yaml` (project identity) and `store/credentials.toml` (secrets).
//! Today it carries the package upload server so lxapp projects — which have
//! no `lingxia.yaml` — can publish without repeating `--lingxia-server`.
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

const LINGXIA_DIR: &str = ".lingxia";
const CLI_DIR: &str = "cli";
const CONFIG_FILE: &str = "config.toml";

/// The whole `config.toml` (one table per area).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CliConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish: Option<PublishConfig>,
}

/// `[publish]` — defaults for `lingxia publish`, selected by the package's
/// `--env`/`--channel`. Each value is a scalar (all envs) or an env map:
///
/// ```toml
/// [publish]
/// token = "lx_tok"                          # every env
///
/// [publish.lingxiaServer]                   # per env
/// developer = "http://localhost:8080"
/// release = "https://prod.example.com"
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishConfig {
    /// Bearer token (used when `--token` is omitted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<EnvValue>,
    /// Upload server. Named like everywhere else (`app.lingxiaServer` in
    /// lxapp.json/lingxia.yaml, runner config, `--lingxia-server`); `server`
    /// is accepted for configs written before the rename.
    #[serde(default, alias = "server", skip_serializing_if = "Option::is_none")]
    pub lingxia_server: Option<EnvValue>,
    /// Legacy pre-v0.11 `[publish.<env>]` tables (token + server per env).
    /// Read-only for compatibility: they win for their env, and `save`
    /// rewrites the file without them.
    #[serde(default, skip_serializing)]
    developer: Option<LegacyEnvPublish>,
    #[serde(default, skip_serializing)]
    preview: Option<LegacyEnvPublish>,
    #[serde(default, skip_serializing)]
    release: Option<LegacyEnvPublish>,
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

/// Legacy `[publish.<env>]` table (token + server for one env).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyEnvPublish {
    #[serde(default)]
    token: Option<String>,
    #[serde(default, alias = "server")]
    lingxia_server: Option<String>,
}

impl PublishConfig {
    fn legacy_env(&self, version: EnvVersion) -> Option<&LegacyEnvPublish> {
        match version {
            EnvVersion::Developer => self.developer.as_ref(),
            EnvVersion::Preview => self.preview.as_ref(),
            EnvVersion::Release => self.release.as_ref(),
        }
    }

    /// The upload server for `version`. A legacy `[publish.<env>]` entry
    /// keeps its old precedence (env table over default); otherwise the
    /// scalar-or-map value resolves directly. Empty strings are unset.
    pub fn lingxia_server_for(&self, version: EnvVersion) -> Option<&str> {
        clean(
            self.legacy_env(version)
                .and_then(|e| e.lingxia_server.as_deref())
                .or_else(|| {
                    self.lingxia_server
                        .as_ref()
                        .and_then(|v| v.for_env(version))
                }),
        )
    }

    /// The bearer token for `version`, resolved like the server.
    pub fn token_for(&self, version: EnvVersion) -> Option<&str> {
        clean(
            self.legacy_env(version)
                .and_then(|e| e.token.as_deref())
                .or_else(|| self.token.as_ref().and_then(|v| v.for_env(version))),
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

    /// Set the publish server and/or token. A `None` env writes a scalar
    /// (every env); an env scopes to that env's map entry, materializing a
    /// previous scalar so the other envs keep their value. `None` arguments
    /// are left unchanged. Legacy `[publish.<env>]` tables are folded into
    /// the new shape first so they can't shadow the value being written.
    pub fn set_publish(
        &mut self,
        env: Option<EnvVersion>,
        server: Option<String>,
        token: Option<String>,
    ) {
        let publish = self.publish.get_or_insert_with(PublishConfig::default);
        publish.fold_legacy_tables();
        match env {
            None => {
                if let Some(server) = server {
                    publish.lingxia_server = Some(EnvValue::Single(server));
                }
                if let Some(token) = token {
                    publish.token = Some(EnvValue::Single(token));
                }
            }
            Some(env) => {
                if let Some(server) = server {
                    EnvValue::set_env_or_new(&mut publish.lingxia_server, env, server);
                }
                if let Some(token) = token {
                    EnvValue::set_env_or_new(&mut publish.token, env, token);
                }
            }
        }
    }
}

impl PublishConfig {
    /// Migrate legacy `[publish.<env>]` tables into the scalar-or-map values,
    /// preserving each env's resolved result, then drop the tables.
    fn fold_legacy_tables(&mut self) {
        if self.developer.is_none() && self.preview.is_none() && self.release.is_none() {
            return;
        }
        for env in [
            EnvVersion::Developer,
            EnvVersion::Preview,
            EnvVersion::Release,
        ] {
            if let Some(server) = self
                .legacy_env(env)
                .and_then(|e| clean(e.lingxia_server.as_deref()))
                .map(str::to_string)
            {
                EnvValue::set_env_or_new(&mut self.lingxia_server, env, server);
            }
            if let Some(token) = self
                .legacy_env(env)
                .and_then(|e| clean(e.token.as_deref()))
                .map(str::to_string)
            {
                EnvValue::set_env_or_new(&mut self.token, env, token);
            }
        }
        self.developer = None;
        self.preview = None;
        self.release = None;
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
    fn set_publish_writes_scalar_when_env_is_none() {
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
    }

    #[test]
    fn set_publish_scopes_to_env_and_preserves_others() {
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
        // developer edit must not clobber the release entry.
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
        // preview was never configured and stays unset.
        assert_eq!(publish.token_for(EnvVersion::Preview), None);
    }

    #[test]
    fn scoping_an_env_materializes_a_scalar_for_the_others() {
        let mut cfg = CliConfig::default();
        cfg.set_publish(None, Some("https://api.example.com".to_string()), None);
        cfg.set_publish(
            Some(EnvVersion::Developer),
            Some("http://localhost:8080".to_string()),
            None,
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
    fn scalar_applies_to_all_envs() {
        let cfg: CliConfig = toml::from_str(
            r#"
            [publish]
            token = "lx_abc"
            lingxiaServer = "https://prod.example.com"
        "#,
        )
        .unwrap();
        let publish = cfg.publish.unwrap();
        assert_eq!(publish.token_for(EnvVersion::Developer), Some("lx_abc"));
        assert_eq!(publish.token_for(EnvVersion::Release), Some("lx_abc"));
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Developer),
            Some("https://prod.example.com")
        );
    }

    #[test]
    fn env_map_is_explicit_per_env() {
        let cfg: CliConfig = toml::from_str(
            r#"
            [publish.token]
            developer = "lx_dev"
            release = "lx_prod"

            [publish.lingxiaServer]
            developer = "http://localhost:8080"
            release = "https://prod.example.com"
        "#,
        )
        .unwrap();
        let publish = cfg.publish.unwrap();
        assert_eq!(publish.token_for(EnvVersion::Developer), Some("lx_dev"));
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Release),
            Some("https://prod.example.com")
        );
        // An env the map does not list is unconfigured — no fallback.
        assert_eq!(publish.token_for(EnvVersion::Preview), None);
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

    #[test]
    fn legacy_server_key_and_env_tables_still_parse() {
        // The shape shipped in v0.10: top-level defaults + [publish.<env>]
        // tables, with the server keyed as `server`.
        let cfg: CliConfig = toml::from_str(
            r#"
            [publish]
            token = "lx_default"
            server = "https://prod.example.com"

            [publish.developer]
            token = "lx_dev"
            server = "http://localhost:8080"
        "#,
        )
        .unwrap();
        let publish = cfg.publish.unwrap();
        assert_eq!(publish.token_for(EnvVersion::Developer), Some("lx_dev"));
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Developer),
            Some("http://localhost:8080")
        );
        // Envs without a legacy table fall back to the old defaults.
        assert_eq!(publish.token_for(EnvVersion::Release), Some("lx_default"));
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Release),
            Some("https://prod.example.com")
        );
    }

    #[test]
    fn saving_migrates_legacy_tables_to_env_maps() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cli").join("config.toml");
        let mut cfg: CliConfig = toml::from_str(
            r#"
            [publish]
            server = "https://prod.example.com"

            [publish.developer]
            token = "lx_dev"
            server = "http://localhost:8080"
        "#,
        )
        .unwrap();
        cfg.set_publish(Some(EnvVersion::Release), None, Some("lx_prod".to_string()));
        cfg.save_to(&path).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("[publish.developer]"));
        assert!(!text.contains("server ="));

        let loaded = CliConfig::load_from(&path).unwrap();
        let publish = loaded.publish.unwrap();
        assert_eq!(publish.token_for(EnvVersion::Developer), Some("lx_dev"));
        assert_eq!(publish.token_for(EnvVersion::Release), Some("lx_prod"));
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Developer),
            Some("http://localhost:8080")
        );
        assert_eq!(
            publish.lingxia_server_for(EnvVersion::Release),
            Some("https://prod.example.com")
        );
    }
}
