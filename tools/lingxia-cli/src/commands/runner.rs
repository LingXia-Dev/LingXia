use anyhow::{Context, Result, anyhow, bail};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const LINGXIA_DIR: &str = ".lingxia";
const RUNNER_DIR: &str = "runner";
const CONFIG_FILE: &str = "config.toml";

#[derive(Subcommand)]
pub enum RunnerAction {
    /// Replace the Runner identity and its environment server URLs
    ///
    /// Environment URLs omitted from this command are removed from the saved
    /// configuration, so a new identity never inherits the previous identity's
    /// endpoints.
    Set {
        /// Cloud identity supplied by the Runner host
        #[arg(value_name = "LINGXIA_ID")]
        lingxia_id: String,

        /// Developer environment server URL
        #[arg(long, value_name = "URL")]
        developer: Option<String>,

        /// Preview environment server URL
        #[arg(long, value_name = "URL")]
        preview: Option<String>,

        /// Release environment server URL
        #[arg(long, value_name = "URL")]
        release: Option<String>,
    },

    /// Remove the persisted Runner cloud configuration
    Clear,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EnvironmentServers {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    developer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    release: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
enum ServerConfig {
    Shared(String),
    PerEnvironment(EnvironmentServers),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadRunnerConfig {
    lingxia_id: String,
    lingxia_server: ServerConfig,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WriteRunnerConfig {
    lingxia_id: String,
    lingxia_server: EnvironmentServers,
}

pub fn execute(action: Option<RunnerAction>) -> Result<()> {
    let path = config_path()?;
    match action {
        None => println!("{}", render_config(&path)?),
        Some(RunnerAction::Set {
            lingxia_id,
            developer,
            preview,
            release,
        }) => {
            let config = prepare_config(lingxia_id, developer, preview, release)?;
            save_config(&path, &config)?;
            println!("✓ Saved Runner config to {}", path.display());
            println!("{}", render_config(&path)?);
        }
        Some(RunnerAction::Clear) => {
            if clear_config(&path)? {
                println!("✓ Cleared Runner config at {}", path.display());
            } else {
                println!("Runner config is already clear: {}", path.display());
            }
        }
    }
    Ok(())
}

fn config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(LINGXIA_DIR).join(RUNNER_DIR).join(CONFIG_FILE))
}

fn prepare_config(
    lingxia_id: String,
    developer: Option<String>,
    preview: Option<String>,
    release: Option<String>,
) -> Result<WriteRunnerConfig> {
    let lingxia_id = lingxia_id.trim();
    if lingxia_id.is_empty() {
        bail!("LINGXIA_ID must not be empty");
    }

    let servers = EnvironmentServers {
        developer: clean_server(developer, "--developer")?,
        preview: clean_server(preview, "--preview")?,
        release: clean_server(release, "--release")?,
    };
    if servers == EnvironmentServers::default() {
        bail!("Provide at least one of --developer, --preview, or --release");
    }

    Ok(WriteRunnerConfig {
        lingxia_id: lingxia_id.to_string(),
        lingxia_server: servers,
    })
}

fn clean_server(server: Option<String>, flag: &str) -> Result<Option<String>> {
    let Some(server) = server else {
        return Ok(None);
    };
    let server = server.trim();
    if server.is_empty() {
        bail!("{flag} must not be empty");
    }
    if !(server.starts_with("http://") || server.starts_with("https://")) {
        bail!("{flag} must be an http(s) URL (got '{server}')");
    }
    Ok(Some(server.to_string()))
}

fn save_config(path: &Path, config: &WriteRunnerConfig) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Runner config path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("Failed to create {}", parent.display()))?;
    let text = toml::to_string_pretty(config).context("Failed to serialize Runner config")?;
    let mut temp = tempfile::Builder::new()
        .prefix(".runner-config-")
        .tempfile_in(parent)
        .with_context(|| format!("Failed to create temporary file in {}", parent.display()))?;
    temp.write_all(text.as_bytes()).with_context(|| {
        format!(
            "Failed to write temporary Runner config for {}",
            path.display()
        )
    })?;
    temp.as_file().sync_all().with_context(|| {
        format!(
            "Failed to sync temporary Runner config for {}",
            path.display()
        )
    })?;
    set_private_file_mode(temp.path());
    temp.persist(path)
        .map_err(|err| anyhow!("Failed to replace {}: {}", path.display(), err.error))?;
    Ok(())
}

fn render_config(path: &Path) -> Result<String> {
    if !path.exists() {
        return Ok(format!(
            "LingXia Runner is not configured.\n\
             Configure it with:\n  lingxia runner set <LINGXIA_ID> --developer <URL>\n\
             Config: {}",
            path.display()
        ));
    }
    let text =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let config: ReadRunnerConfig =
        toml::from_str(&text).with_context(|| format!("Failed to parse {}", path.display()))?;
    let id = config.lingxia_id.trim();
    if id.is_empty() {
        bail!("{}.lingxiaId must not be empty", path.display());
    }
    let servers = match config.lingxia_server {
        ServerConfig::Shared(server) => EnvironmentServers {
            developer: Some(server.clone()),
            preview: Some(server.clone()),
            release: Some(server),
        },
        ServerConfig::PerEnvironment(servers) => servers,
    };
    Ok(format!(
        "LingXia ID: {id}\n\
         developer:  {}\n\
         preview:    {}\n\
         release:    {}\n\
         Config: {}",
        display_server(servers.developer.as_deref()),
        display_server(servers.preview.as_deref()),
        display_server(servers.release.as_deref()),
        path.display()
    ))
}

fn display_server(server: Option<&str>) -> &str {
    server
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("—")
}

fn clear_config(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    fs::remove_file(path).with_context(|| format!("Failed to remove {}", path.display()))?;
    Ok(true)
}

#[cfg(unix)]
fn set_private_file_mode(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn set_private_file_mode(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> WriteRunnerConfig {
        prepare_config(
            "com.example.app".into(),
            Some("http://127.0.0.1:8787".into()),
            None,
            Some("https://api.example.com".into()),
        )
        .unwrap()
    }

    #[test]
    fn writes_the_runtime_config_shape() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("runner/config.toml");
        save_config(&path, &sample_config()).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("lingxiaId = \"com.example.app\""));
        assert!(text.contains("[lingxiaServer]"));
        assert!(text.contains("developer = \"http://127.0.0.1:8787\""));
        assert!(text.contains("release = \"https://api.example.com\""));
        assert!(!text.contains("preview ="));
    }

    #[test]
    fn replacing_config_drops_omitted_environments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        save_config(&path, &sample_config()).unwrap();
        let replacement = prepare_config(
            "com.example.next".into(),
            None,
            Some("https://preview.example.com".into()),
            None,
        )
        .unwrap();
        save_config(&path, &replacement).unwrap();
        let text = fs::read_to_string(path).unwrap();
        assert!(text.contains("lingxiaId = \"com.example.next\""));
        assert!(text.contains("preview = \"https://preview.example.com\""));
        assert!(!text.contains("developer ="));
        assert!(!text.contains("release ="));
    }

    #[test]
    fn validates_identity_and_urls() {
        assert!(prepare_config(" ".into(), Some("https://x".into()), None, None).is_err());
        assert!(prepare_config("app".into(), None, None, None).is_err());
        assert!(prepare_config("app".into(), Some("api.example.com".into()), None, None).is_err());
    }

    #[test]
    fn renders_missing_and_configured_states() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let missing = render_config(&path).unwrap();
        assert!(missing.contains("LingXia Runner is not configured"));
        save_config(&path, &sample_config()).unwrap();
        let rendered = render_config(&path).unwrap();
        assert!(rendered.contains("LingXia ID: com.example.app"));
        assert!(rendered.contains("preview:    —"));
    }

    #[test]
    fn clear_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        assert!(!clear_config(&path).unwrap());
        save_config(&path, &sample_config()).unwrap();
        assert!(clear_config(&path).unwrap());
        assert!(!path.exists());
        assert!(!clear_config(&path).unwrap());
    }
}
