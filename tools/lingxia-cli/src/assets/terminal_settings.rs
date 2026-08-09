use crate::config::LingXiaConfig;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::lxapp_package::resolve_lxapp_package;

pub(crate) const APP_ID: &str = "app.lingxia.terminal-settings";
const DEFAULT_PACKAGE: &str = "@lingxia/terminal-settings";

pub(super) struct TerminalSettingsSource {
    pub(super) bundle_dir: PathBuf,
    pub(super) build: bool,
}

pub(super) fn resolve_terminal_settings_dir(
    project_root: &Path,
    config: &LingXiaConfig,
) -> Result<TerminalSettingsSource> {
    let settings = config
        .terminal
        .as_ref()
        .and_then(|terminal| terminal.settings.as_ref());
    if let Some(path) = settings
        .and_then(|settings| settings.path.as_deref())
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        return Ok(TerminalSettingsSource {
            bundle_dir: project_root.join(path),
            build: true,
        });
    }

    if let Some(package) = settings
        .and_then(|settings| settings.package.as_deref())
        .map(str::trim)
        .filter(|package| !package.is_empty())
    {
        let version = settings
            .and_then(|settings| settings.version.as_deref())
            .unwrap_or(env!("LINGXIA_TERMINAL_SETTINGS_VERSION"));
        return Ok(TerminalSettingsSource {
            bundle_dir: resolve_lxapp_package(
                project_root,
                package,
                version,
                "terminal-settings",
                "terminal.settings",
            )?,
            build: false,
        });
    }

    Ok(TerminalSettingsSource {
        bundle_dir: resolve_lxapp_package(
            project_root,
            DEFAULT_PACKAGE,
            env!("LINGXIA_TERMINAL_SETTINGS_VERSION"),
            "terminal-settings",
            "terminal.settings",
        )
        .with_context(|| {
            format!(
                "Failed to resolve default terminal settings package {}@{}. Set `terminal.settings.path` to a local checkout, or `terminal.settings.package`/`version` to pin a fork.",
                DEFAULT_PACKAGE,
                env!("LINGXIA_TERMINAL_SETTINGS_VERSION")
            )
        })?,
        build: false,
    })
}
