//! `mocks/` in a dev session: bundle each watched lxapp's `mocks/index.ts`,
//! validate its `mocks/config.json`, and push both to the runtime
//! (`session.network.mock.load`) at session start, on every runtime
//! (re)connect, and on every save under `mocks/` — without rebuilding or
//! restarting the app. An invalid save keeps the last valid mocks.

use crate::lxapp::{MocksBundle, build_mocks};
use anyhow::{Result, anyhow};
use lingxia_control_protocol::mock::{self, CONFIG_FILE, MockConfig, MockMode};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static BASELINE: OnceLock<Option<MockMode>> = OnceLock::new();

/// Record `lingxia dev --mock` for this process's session.
pub(super) fn set_baseline(mode: Option<MockMode>) {
    let _ = BASELINE.set(mode);
}

/// The session baseline, `None` when `--mock` was not given.
pub(super) fn baseline() -> Option<MockMode> {
    BASELINE.get().copied().flatten()
}

/// One lxapp's mocks as last validated.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct AppMocks {
    pub app_id: String,
    pub root: PathBuf,
    pub bundle: MocksBundle,
    /// `mocks/config.json` as written; `None` when absent.
    pub config: Option<Value>,
    /// The same, validated.
    pub parsed: Option<MockConfig>,
}

impl AppMocks {
    /// `session.network.mock.load` params.
    pub fn load_params(&self, baseline: Option<MockMode>) -> Value {
        json!({
            "appid": self.app_id,
            "source": self.bundle.source,
            "keys": self.bundle.keys,
            "config": self.config,
            "baseline": baseline,
        })
    }

    /// `all — from mocks/config.json · 14 handlers` (the live layer is not
    /// set at start).
    pub fn summary(&self, baseline: Option<MockMode>) -> String {
        let handlers = self.bundle.keys.len();
        format!(
            "{} · {handlers} handler{}",
            mock::describe_selection(self.parsed.as_ref(), baseline, &[]),
            if handlers == 1 { "" } else { "s" }
        )
    }
}

/// Bundle and validate an lxapp's mocks. `None` when it has no
/// `mocks/index.ts`; an error names the file and what is wrong.
pub(super) fn load_app(app_id: &str, root: &Path) -> Result<Option<AppMocks>> {
    let config_path = root.join(CONFIG_FILE);
    let config = if config_path.is_file() {
        let text = std::fs::read_to_string(&config_path)
            .map_err(|err| anyhow!("{CONFIG_FILE}: cannot read it: {err}"))?;
        Some(serde_json::from_str::<Value>(&text).map_err(|err| {
            anyhow!(
                "{CONFIG_FILE} is not valid JSON (line {}, column {}): {err}",
                err.line(),
                err.column()
            )
        })?)
    } else {
        None
    };
    let Some(bundle) = build_mocks(root)? else {
        if config.is_some() {
            return Err(anyhow!("{CONFIG_FILE} needs mocks/index.ts"));
        }
        return Ok(None);
    };
    let parsed = config
        .as_ref()
        .map(|value| MockConfig::parse(value, Some(&bundle.keys)))
        .transpose()
        .map_err(|err| anyhow!(err))?;
    Ok(Some(AppMocks {
        app_id: app_id.to_string(),
        root: root.to_path_buf(),
        bundle,
        config,
        parsed,
    }))
}

/// Whether `path` (a saved file) lies under `root/mocks/`.
pub(super) fn is_mocks_path(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root)
        .ok()
        .and_then(|relative| relative.components().next())
        .is_some_and(|first| first.as_os_str() == mock::MOCKS_DIR)
}

/// The banner line when an lxapp has no `mocks/` at all.
pub(super) fn no_mocks_line(baseline: Option<MockMode>) -> String {
    match baseline {
        Some(mode) => format!("{mode} — from lingxia dev --mock (no mocks/: nothing can answer)"),
        None => "none (no mocks/)".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn lxapp(root: &Path) {
        write(
            root,
            "lxapp.json",
            r#"{ "appId": "demo.app", "appName": "Demo", "version": "1.0.0", "pages": [] }"#,
        );
    }

    #[test]
    fn an_lxapp_without_mocks_has_none_and_a_lone_config_is_an_error() {
        let temp = tempfile::tempdir().unwrap();
        lxapp(temp.path());
        assert_eq!(load_app("demo.app", temp.path()).unwrap(), None);
        write(temp.path(), "mocks/config.json", r#"{ "mock": "all" }"#);
        assert_eq!(
            load_app("demo.app", temp.path()).unwrap_err().to_string(),
            "mocks/config.json needs mocks/index.ts"
        );
    }

    #[test]
    fn mocks_load_with_their_config_and_describe_the_selection() {
        let temp = tempfile::tempdir().unwrap();
        lxapp(temp.path());
        write(
            temp.path(),
            "mocks/index.ts",
            "export default { 'GET **/qoe/*': { json: {} }, 'POST **/sessions': () => ({ status: 204 }) };\n",
        );
        let loaded = load_app("demo.app", temp.path()).unwrap().unwrap();
        assert_eq!(loaded.parsed, None);
        assert_eq!(
            loaded.summary(None),
            "none — default (no mocks/config.json) · 2 handlers"
        );
        write(
            temp.path(),
            "mocks/config.json",
            r#"{ "mock": "none", "overrides": ["GET **/qoe/*"] }"#,
        );
        let loaded = load_app("demo.app", temp.path()).unwrap().unwrap();
        assert_eq!(
            loaded.summary(Some(MockMode::All)),
            "all — from lingxia dev --mock (mocks/config.json: none, 1 mock override — not in \
             effect) · 2 handlers"
        );
        let params = loaded.load_params(Some(MockMode::All));
        assert_eq!(params["keys"], json!(["GET **/qoe/*", "POST **/sessions"]));
        assert_eq!(params["baseline"], "all");
        assert_eq!(params["config"]["mock"], "none");

        write(
            temp.path(),
            "mocks/config.json",
            r#"{ "mock": "none", "overrides": ["GET **/other"] }"#,
        );
        assert_eq!(
            load_app("demo.app", temp.path()).unwrap_err().to_string(),
            "mocks/config.json: overrides[0] \"GET **/other\" matches no handler in mocks/index.ts"
        );
        write(temp.path(), "mocks/config.json", "{ mock: all }");
        let err = load_app("demo.app", temp.path()).unwrap_err().to_string();
        assert!(
            err.starts_with("mocks/config.json is not valid JSON"),
            "{err}"
        );
    }

    #[test]
    fn saves_under_mocks_are_told_apart() {
        let root = Path::new("/p/app");
        assert!(is_mocks_path(root, Path::new("/p/app/mocks/index.ts")));
        assert!(is_mocks_path(
            root,
            Path::new("/p/app/mocks/sub/fixtures.ts")
        ));
        assert!(!is_mocks_path(root, Path::new("/p/app/pages/mocks.ts")));
        assert!(!is_mocks_path(root, Path::new("/p/app/mocksx/index.ts")));
    }
}
