struct RunnerDevtoolAddon;

impl lingxia::HostAddon for RunnerDevtoolAddon {
    // Cloud provider (lx.cloud/auth + update/fingerprint/push). Must register in this
    // hook — the logic context is built before `start_services`. Injected via
    // `--with-provider cloud`.
    //
    // We bring the cloud client up, so we configure its endpoint here: a user can
    // point the runner at a server / identity via ~/.lingxia/runner/config.toml
    // without rebuilding. Unset fields fall back to the app config.
    #[cfg(feature = "cloud")]
    fn install_logic_extensions(&self) {
        let mut options = lingxia_cloud_client::CloudOptions::default();
        if let Some(home) = std::env::var_os("HOME") {
            let overrides = parse_runner_config(std::path::Path::new(&home));
            if let Some(server) = overrides.lingxia_server {
                options = options.lingxia_server(server);
            }
            if let Some(id) = overrides.lingxia_id {
                options = options.lingxia_id(id);
            }
        }
        // Mock the LingXiao functions service from a local JS dir, when `lingxia
        // dev` points us at the lxapp's `mock/functions`. The mock vs real default
        // is the cloud client's own LINGXIAO_MOCK env; this only supplies the dir.
        if let Some(dir) = std::env::var_os("LINGXIAO_MOCK_DIR").filter(|d| !d.is_empty()) {
            options = options.lingxiao_mock(std::path::PathBuf::from(dir));
        }
        if let Err(err) = lingxia_cloud_client::init(options) {
            eprintln!("[cloud] provider init failed: {err}");
        }
    }

    fn start_services(&self) {
        // The Runner is a dev/test harness: grant lx.automation() to every
        // lxapp it launches so test scripts need not declare the privilege.
        lingxia::set_automation_auto_grant(true);
        lingxia_devtool::start_devtool_bridge_from_env();
    }
}

#[cfg(feature = "cloud")]
#[derive(Default)]
struct RunnerOverrides {
    lingxia_server: Option<String>,
    lingxia_id: Option<String>,
}

/// Parse overrides from `<home>/.lingxia/runner/config.toml`, if present.
#[cfg(feature = "cloud")]
fn parse_runner_config(home: &std::path::Path) -> RunnerOverrides {
    parse_runner_config_for_env(home, runner_env_from_env())
}

#[cfg(feature = "cloud")]
fn parse_runner_config_for_env(home: &std::path::Path, env: RunnerEnv) -> RunnerOverrides {
    let path = home.join(".lingxia/runner/config.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return RunnerOverrides::default();
    };
    let Ok(value) = toml::from_str::<toml::Value>(&text) else {
        return RunnerOverrides::default();
    };
    let env_table = value.get(env.table_name()).and_then(toml::Value::as_table);
    RunnerOverrides {
        lingxia_server: table_str_field(env_table, "lingxiaServer")
            .or_else(|| str_field(&value, "lingxiaServer")),
        lingxia_id: table_str_field(env_table, "lingxiaId")
            .or_else(|| str_field(&value, "lingxiaId")),
    }
}

#[cfg(feature = "cloud")]
#[derive(Clone, Copy)]
enum RunnerEnv {
    Developer,
    Preview,
    Release,
}

#[cfg(feature = "cloud")]
impl RunnerEnv {
    fn table_name(self) -> &'static str {
        match self {
            Self::Developer => "developer",
            Self::Preview => "preview",
            Self::Release => "release",
        }
    }
}

#[cfg(feature = "cloud")]
fn runner_env_from_env() -> RunnerEnv {
    match std::env::var("LINGXIA_RUNNER_ENV")
        .as_deref()
        .map(str::trim)
    {
        Ok("preview") => RunnerEnv::Preview,
        Ok("release") => RunnerEnv::Release,
        // "developer"/"dev", unset, or anything unrecognized
        _ => RunnerEnv::Developer,
    }
}

#[cfg(feature = "cloud")]
fn str_field(value: &toml::Value, key: &str) -> Option<String> {
    let s = value.get(key)?.as_str()?.trim();
    (!s.is_empty()).then(|| s.to_string())
}

#[cfg(feature = "cloud")]
fn table_str_field(
    table: Option<&toml::map::Map<String, toml::Value>>,
    key: &str,
) -> Option<String> {
    let s = table?.get(key)?.as_str()?.trim();
    (!s.is_empty()).then(|| s.to_string())
}

#[unsafe(no_mangle)]
pub extern "C" fn lingxia_register_host_addon() {
    lingxia::register_host_addon(Box::new(RunnerDevtoolAddon));
}

#[cfg(all(test, feature = "cloud"))]
mod tests {
    use super::{RunnerEnv, parse_runner_config, parse_runner_config_for_env};

    #[test]
    fn parses_server_and_id() {
        let dir = std::env::temp_dir().join(format!("lx-runner-{}", std::process::id()));
        let runner = dir.join(".lingxia/runner");
        std::fs::create_dir_all(&runner).unwrap();
        std::fs::write(
            runner.join("config.toml"),
            "lingxiaServer = \"https://staging.example.com\"\nlingxiaId = \"app-123\"\n",
        )
        .unwrap();
        let o = parse_runner_config(&dir);
        assert_eq!(
            o.lingxia_server.as_deref(),
            Some("https://staging.example.com")
        );
        assert_eq!(o.lingxia_id.as_deref(), Some("app-123"));
        std::fs::remove_dir_all(&dir).ok();
        // Missing file -> no overrides.
        let empty = parse_runner_config(std::path::Path::new("/no/such/home"));
        assert!(empty.lingxia_server.is_none() && empty.lingxia_id.is_none());
    }

    #[test]
    fn env_table_overrides_top_level_values() {
        let dir = std::env::temp_dir().join(format!("lx-runner-env-{}", std::process::id()));
        let runner = dir.join(".lingxia/runner");
        std::fs::create_dir_all(&runner).unwrap();
        std::fs::write(
            runner.join("config.toml"),
            r#"lingxiaServer = "https://default.example.com"
lingxiaId = "default-id"

[preview]
lingxiaServer = "https://preview.example.com"
"#,
        )
        .unwrap();
        let o = parse_runner_config_for_env(&dir, RunnerEnv::Preview);
        assert_eq!(
            o.lingxia_server.as_deref(),
            Some("https://preview.example.com")
        );
        assert_eq!(o.lingxia_id.as_deref(), Some("default-id"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
