use crate::config::{LingXiaConfig, LxappSourceConfig};
use anyhow::Result;
use std::path::{Path, PathBuf};

use super::lxapp_package::resolve_lxapp_package;

/// An lxapp with a reserved appId that the host supplies under its own `lingxia.yaml` key.
pub(crate) struct SystemLxapp {
    pub(crate) app_id: &'static str,
    /// Where it is configured: `browser.webui`.
    pub(crate) key: &'static str,
    /// Names its build cache and npm staging.
    pub(super) kind: &'static str,
    source: fn(&LingXiaConfig) -> Option<&LxappSourceConfig>,
}

pub(crate) const BROWSER_WEBUI: SystemLxapp = SystemLxapp {
    app_id: "app.lingxia.browser",
    key: "browser.webui",
    kind: "browser-shell-webui",
    source: |config| config.browser.as_ref()?.webui.as_ref(),
};

pub(crate) const TERMINAL_SETTINGS: SystemLxapp = SystemLxapp {
    app_id: "app.lingxia.terminal-settings",
    key: "terminal.settings",
    kind: "terminal-settings",
    source: |config| config.terminal.as_ref()?.settings.as_ref(),
};

pub(crate) fn system_lxapp(app_id: &str) -> Option<&'static SystemLxapp> {
    [&BROWSER_WEBUI, &TERMINAL_SETTINGS]
        .into_iter()
        .find(|system| system.app_id == app_id)
}

pub(super) struct SystemLxappSource {
    pub(super) bundle_dir: PathBuf,
    pub(super) build: bool,
}

fn non_empty(value: Option<&String>) -> Option<&str> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
}

/// The configured copy, or `None` when the host supplies none.
pub(super) fn resolve_system_lxapp(
    system: &SystemLxapp,
    project_root: &Path,
    config: &LingXiaConfig,
) -> Result<Option<SystemLxappSource>> {
    let Some(source) = (system.source)(config) else {
        return Ok(None);
    };
    if let Some(path) = non_empty(source.path.as_ref()) {
        return Ok(Some(SystemLxappSource {
            bundle_dir: project_root.join(path),
            build: true,
        }));
    }
    let Some(package) = non_empty(source.package.as_ref()) else {
        return Ok(None);
    };
    let version = non_empty(source.version.as_ref())
        .map(str::to_string)
        .unwrap_or_else(crate::versions::npm_compat_range);
    Ok(Some(SystemLxappSource {
        bundle_dir: resolve_lxapp_package(
            project_root,
            package,
            &version,
            system.kind,
            system.key,
        )?,
        build: false,
    }))
}
