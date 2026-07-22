//! Resolves the dev runner's configuration from its environment.
//!
//! Both the macOS and Windows dev runners need the same inputs before handing
//! them to the (privately-injected) provider: the `lingxiaServer`/`lingxiaId`
//! overrides from `~/.lingxia/runner/config.toml`.
//!
//! That resolution is pure LingXia/CLI convention, so it lives here as a small,
//! dependency-light crate returning plain data. Each runner maps the result
//! onto the provider's own option types (the only crate that has them).

use std::path::{Path, PathBuf};

/// Env var (set by `lingxia dev --env`) selecting the runner config table.
const ENV_RUNNER_ENV: &str = "LINGXIA_RUNNER_ENV";
/// Runner config location, relative to the user's home directory.
const RUNNER_CONFIG_REL: &str = ".lingxia/runner/config.toml";

/// The dev runner's resolved configuration.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunnerConfig {
    /// `lingxiaServer` override from the runner config, if set.
    pub lingxia_server: Option<String>,
    /// `lingxiaId` override from the runner config, if set.
    pub lingxia_id: Option<String>,
}

/// Resolve the runner config from the current process environment.
pub fn from_env() -> RunnerConfig {
    let env = runner_env_from_env();
    let (lingxia_server, lingxia_id) = home_dir()
        .map(|home| parse_runner_config(&home.join(RUNNER_CONFIG_REL), env))
        .unwrap_or_default();
    RunnerConfig {
        lingxia_server,
        lingxia_id,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunnerEnv {
    Developer,
    Preview,
    Release,
}

impl RunnerEnv {
    fn table_name(self) -> &'static str {
        match self {
            Self::Developer => "developer",
            Self::Preview => "preview",
            Self::Release => "release",
        }
    }
}

fn runner_env_from_env() -> RunnerEnv {
    match std::env::var(ENV_RUNNER_ENV).as_deref().map(str::trim) {
        Ok("preview") => RunnerEnv::Preview,
        Ok("release") => RunnerEnv::Release,
        // "developer"/"dev", unset, or anything unrecognized
        _ => RunnerEnv::Developer,
    }
}

/// Cross-platform home directory (`USERPROFILE` on Windows, else `HOME`).
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Parse `(lingxiaServer, lingxiaId)` from the runner config at `path`. A
/// missing or unparseable file yields `(None, None)`. Each key is either a
/// scalar (applies to every env) or an env-keyed table (explicit per env, no
/// fallback) — the same shape as `lingxia.yaml`'s `app.lingxiaServer`.
fn parse_runner_config(path: &Path, env: RunnerEnv) -> (Option<String>, Option<String>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (None, None);
    };
    let Ok(value) = toml::from_str::<toml::Value>(&text) else {
        return (None, None);
    };
    (
        env_value(&value, "lingxiaServer", env),
        env_value(&value, "lingxiaId", env),
    )
}

/// Resolve `key` for `env`: a string value applies to every env; a table
/// value is looked up by env name. Empty strings are treated as unset.
fn env_value(root: &toml::Value, key: &str, env: RunnerEnv) -> Option<String> {
    let value = root.get(key)?;
    let s = match value {
        toml::Value::String(s) => s.as_str(),
        toml::Value::Table(per_env) => per_env.get(env.table_name())?.as_str()?,
        _ => return None,
    }
    .trim();
    (!s.is_empty()).then(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_server_and_id() {
        let dir = std::env::temp_dir().join(format!("lx-runner-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(
            &path,
            "lingxiaServer = \"https://staging.example.com\"\nlingxiaId = \"app-123\"\n",
        )
        .unwrap();
        let (server, id) = parse_runner_config(&path, RunnerEnv::Developer);
        assert_eq!(server.as_deref(), Some("https://staging.example.com"));
        assert_eq!(id.as_deref(), Some("app-123"));
        std::fs::remove_dir_all(&dir).ok();
        // Missing file -> no overrides.
        assert_eq!(
            parse_runner_config(Path::new("/no/such/config.toml"), RunnerEnv::Developer),
            (None, None)
        );
    }

    #[test]
    fn per_key_env_maps_are_explicit_per_env() {
        let dir = std::env::temp_dir().join(format!("lx-runner-cfg-env-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(
            &path,
            r#"
lingxiaId = "app-id"

[lingxiaServer]
preview = "https://preview.example.com"
release = "https://api.example.com"
"#,
        )
        .unwrap();

        // Scalar lingxiaId applies to every env; the lingxiaServer map is
        // looked up per env with no fallback for envs it does not list.
        let (server, id) = parse_runner_config(&path, RunnerEnv::Preview);
        assert_eq!(server.as_deref(), Some("https://preview.example.com"));
        assert_eq!(id.as_deref(), Some("app-id"));

        let (server, id) = parse_runner_config(&path, RunnerEnv::Developer);
        assert_eq!(server, None);
        assert_eq!(id.as_deref(), Some("app-id"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
