use crate::config::LingXiaConfig;
use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};

use super::lxapp_package::resolve_lxapp_package;

pub(crate) const APP_ID: &str = "app.lingxia.browser";

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

    // There is no default. The browser shell webui is a prebuilt lxapp that
    // lives in this repo and is deliberately not published to npm, so a host
    // enabling the browser has to say where its copy is. Saying so here is the
    // whole error: the alternative was a default that pointed at a package the
    // registry has never had, which failed at `npm pack` with nothing to act on.
    Err(anyhow!(
        "capabilities.browser is on, but browser.webui is not set.\n\n\
         The webui is the browser's face — newtab, settings, downloads — so it \
         ships as a starting point you own rather than a package you depend on. \
         Take a copy:\n\n  \
         lingxia browser-shell eject\n\n\
         then point the host at it:\n\n  \
         browser:\n    webui:\n      path: browser-shell-webui\n\n\
         `browser.webui.package`/`version` still works for a fork you publish \
         yourself."
    ))
}
