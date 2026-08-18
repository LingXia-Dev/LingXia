use crate::config::LingXiaConfig;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::lxapp_package::resolve_lxapp_package;

pub(crate) const APP_ID: &str = "app.lingxia.browser";
const DEFAULT_PACKAGE: &str = "@lingxia/browser-shell-webui";

pub(super) struct BrowserShellWebUiSource {
    pub(super) bundle_dir: PathBuf,
    pub(super) build: bool,
}

pub(super) fn resolve_browser_shell_webui_dir(
    project_root: &Path,
    config: &LingXiaConfig,
) -> Result<BrowserShellWebUiSource> {
    let webui = config
        .browser
        .as_ref()
        .and_then(|browser| browser.webui.as_ref());
    if let Some(path) = webui
        .and_then(|webui| webui.path.as_deref())
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        return Ok(BrowserShellWebUiSource {
            bundle_dir: project_root.join(path),
            build: true,
        });
    }

    if let Some(package) = webui
        .and_then(|webui| webui.package.as_deref())
        .map(str::trim)
        .filter(|package| !package.is_empty())
    {
        let version = webui
            .and_then(|webui| webui.version.as_deref())
            .map(str::trim)
            .filter(|version| !version.is_empty())
            .map(str::to_string)
            .unwrap_or_else(crate::versions::npm_compat_range);
        return Ok(BrowserShellWebUiSource {
            bundle_dir: resolve_lxapp_package(
                project_root,
                package,
                &version,
                "browser-shell-webui",
                "browser.webui",
            )?,
            build: false,
        });
    }

    // Latest published package on this CLI's major.minor line.
    let version = crate::versions::npm_compat_range();
    Ok(BrowserShellWebUiSource {
        bundle_dir: resolve_lxapp_package(
            project_root,
            DEFAULT_PACKAGE,
            &version,
            "browser-shell-webui",
            "browser.webui",
        )
        .with_context(|| {
            format!(
                "Failed to resolve default browser webui package {}@{}. Set `browser.webui.path` to point at a local checkout, or `browser.webui.package`/`version` to pin a fork.",
                DEFAULT_PACKAGE,
                version
            )
        })?,
        build: false,
    })
}
