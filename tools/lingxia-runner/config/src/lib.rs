//! Resolves the dev runner's configuration from its environment.
//!
//! Both the macOS and Windows dev runners need the same inputs before handing
//! them to the (privately-injected) provider: the `lingxiaServer`/`lingxiaId`
//! overrides from `~/.lingxia/runner/config.toml`, the local function-mock
//! directory that `lingxia dev` points at, and the per-function mock/live
//! routing from the running lxapp's `functions.json`.
//!
//! That resolution is pure LingXia/CLI convention — env var names, file paths,
//! and the `functions.json` schema — so it lives here as a small, dependency-
//! light crate returning plain data. Each runner maps the result onto the
//! provider's own option/routing types (the only crate that has them).

use std::path::{Path, PathBuf};

/// Env var (set by `lingxia dev`) pointing at the transpiled LingXiao mock dir.
const ENV_MOCK_DIR: &str = "LINGXIAO_MOCK_DIR";
/// Env var (set by `lingxia dev`) pointing at the running lxapp's directory.
const ENV_LXAPP_PATH: &str = "LINGXIA_LXAPP_PATH";
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
    /// Local LingXiao mock, present only when `lingxia dev` enabled it; `None`
    /// means call the real service.
    pub mock: Option<RunnerMock>,
}

/// A local LingXiao mock plus the per-function routing that selects, name by
/// name, whether to hit the mock or the live service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunnerMock {
    /// Directory of transpiled mock `.js` (each registers via `lx.fn`).
    pub dir: PathBuf,
    pub routing: RunnerRouting,
}

/// Per-function mock/live routing. `default_live` applies to any function
/// without an `overrides` entry; `true` = live service, `false` = mock.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunnerRouting {
    pub default_live: bool,
    /// `(function name, is_live)` overrides.
    pub overrides: Vec<(String, bool)>,
}

/// Resolve the runner config from the current process environment.
pub fn from_env() -> RunnerConfig {
    let env = runner_env_from_env();
    let (lingxia_server, lingxia_id) = home_dir()
        .map(|home| parse_runner_config(&home.join(RUNNER_CONFIG_REL), env))
        .unwrap_or_default();
    let mock = std::env::var_os(ENV_MOCK_DIR)
        .filter(|dir| !dir.is_empty())
        .map(|dir| RunnerMock {
            dir: PathBuf::from(dir),
            routing: routing_from_env(),
        });
    RunnerConfig {
        lingxia_server,
        lingxia_id,
        mock,
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
        Ok("developer") | Ok("dev") | _ => RunnerEnv::Developer,
    }
}

/// Cross-platform home directory (`USERPROFILE` on Windows, else `HOME`).
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Parse `(lingxiaServer, lingxiaId)` from the runner config at `path`. A
/// missing or unparseable file yields `(None, None)`. Top-level values are
/// defaults; `[developer]`, `[preview]`, and `[release]` override per env.
fn parse_runner_config(path: &Path, env: RunnerEnv) -> (Option<String>, Option<String>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (None, None);
    };
    let Ok(value) = toml::from_str::<toml::Value>(&text) else {
        return (None, None);
    };
    let env_table = value.get(env.table_name()).and_then(toml::Value::as_table);
    (
        table_str_field(env_table, "lingxiaServer").or_else(|| str_field(&value, "lingxiaServer")),
        table_str_field(env_table, "lingxiaId").or_else(|| str_field(&value, "lingxiaId")),
    )
}

fn str_field(value: &toml::Value, key: &str) -> Option<String> {
    let s = value.get(key)?.as_str()?.trim();
    (!s.is_empty()).then(|| s.to_string())
}

fn table_str_field(
    table: Option<&toml::map::Map<String, toml::Value>>,
    key: &str,
) -> Option<String> {
    let s = table?.get(key)?.as_str()?.trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// Read routing from `<LINGXIA_LXAPP_PATH>/functions.json`. Missing/invalid →
/// all-mock default.
fn routing_from_env() -> RunnerRouting {
    let Some(lxapp) = std::env::var_os(ENV_LXAPP_PATH) else {
        return RunnerRouting::default();
    };
    let Ok(text) = std::fs::read_to_string(Path::new(&lxapp).join("functions.json")) else {
        return RunnerRouting::default();
    };
    parse_routing(&text)
}

/// Parse the `dev` section of a `functions.json` document into routing. Absent
/// fields and unknown provider strings fall back to mock.
fn parse_routing(functions_json: &str) -> RunnerRouting {
    let is_live = |s: &str| s.eq_ignore_ascii_case("live");
    let Ok(config) = serde_json::from_str::<serde_json::Value>(functions_json) else {
        return RunnerRouting::default();
    };
    let dev = config.get("dev");
    let default_live = dev
        .and_then(|d| d.get("default"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(is_live);
    let overrides = dev
        .and_then(|d| d.get("overrides"))
        .and_then(serde_json::Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(name, value)| value.as_str().map(|v| (name.clone(), is_live(v))))
                .collect()
        })
        .unwrap_or_default();
    RunnerRouting {
        default_live,
        overrides,
    }
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
    fn env_table_overrides_top_level_runner_config() {
        let dir = std::env::temp_dir().join(format!("lx-runner-cfg-env-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(
            &path,
            r#"
lingxiaServer = "https://default.example.com"
lingxiaId = "default-id"

[preview]
lingxiaServer = "https://preview.example.com"

[release]
lingxiaId = "release-id"
"#,
        )
        .unwrap();

        let (server, id) = parse_runner_config(&path, RunnerEnv::Preview);
        assert_eq!(server.as_deref(), Some("https://preview.example.com"));
        assert_eq!(id.as_deref(), Some("default-id"));

        let (server, id) = parse_runner_config(&path, RunnerEnv::Release);
        assert_eq!(server.as_deref(), Some("https://default.example.com"));
        assert_eq!(id.as_deref(), Some("release-id"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parses_routing_default_and_overrides() {
        let json = r#"{ "worker": "./server",
            "dev": { "default": "live", "overrides": { "hello": "mock", "charge": "live" } } }"#;
        let r = parse_routing(json);
        assert!(r.default_live);
        let mut overrides = r.overrides;
        overrides.sort();
        assert_eq!(
            overrides,
            vec![("charge".to_string(), true), ("hello".to_string(), false)]
        );
    }

    #[test]
    fn missing_dev_section_or_invalid_is_all_mock() {
        let r = parse_routing(r#"{ "worker": "./server" }"#);
        assert!(!r.default_live);
        assert!(r.overrides.is_empty());
        assert_eq!(parse_routing("not json"), RunnerRouting::default());
    }
}
