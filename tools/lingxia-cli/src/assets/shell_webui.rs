use crate::config::LingXiaConfig;
use anyhow::{Context, Result, anyhow};
use flate2::read::GzDecoder;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::Archive;

pub(crate) const APP_ID: &str = "app.lingxia.browser";
const DEFAULT_PACKAGE: &str = "@lingxia/shell-webui";

pub(super) struct ShellWebUiSource {
    pub(super) bundle_dir: PathBuf,
    pub(super) build: bool,
}

pub(super) fn resolve_shell_webui_dir(
    project_root: &Path,
    config: &LingXiaConfig,
) -> Result<ShellWebUiSource> {
    let webui = config.shell.as_ref().and_then(|shell| shell.webui.as_ref());
    if let Some(path) = webui
        .and_then(|webui| webui.path.as_deref())
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        return Ok(ShellWebUiSource {
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
            .unwrap_or(env!("LINGXIA_SHELL_WEBUI_VERSION"));
        return Ok(ShellWebUiSource {
            bundle_dir: resolve_lxapp_package(project_root, package, version)?,
            build: false,
        });
    }

    // Fall back to the SDK's default npm package, pinned to the SDK package set.
    Ok(ShellWebUiSource {
        bundle_dir: resolve_lxapp_package(
            project_root,
            DEFAULT_PACKAGE,
            env!("LINGXIA_SHELL_WEBUI_VERSION"),
        )
        .with_context(|| {
            format!(
                "Failed to resolve default shell webui package {}@{}. Set `shell.webui.path` to point at a local checkout, or `shell.webui.package`/`version` to pin a fork.",
                DEFAULT_PACKAGE,
                env!("LINGXIA_SHELL_WEBUI_VERSION")
            )
        })?,
        build: false,
    })
}

#[derive(Deserialize)]
struct NpmPackResult {
    filename: String,
}

pub(super) fn resolve_lxapp_package(
    project_root: &Path,
    package: &str,
    version: &str,
) -> Result<PathBuf> {
    let package = package.trim();
    let version = version.trim();
    if package.is_empty() {
        return Err(anyhow!("shell.webui.package must not be empty"));
    }
    if version.is_empty() {
        return Err(anyhow!("shell.webui.version must not be empty"));
    }

    let package_dir = project_root
        .join(".lingxia")
        .join("shell-webui")
        .join(sanitize_package_name(package))
        .join(version)
        .join("package");
    if package_dir.join("lxapp.json").exists() {
        return Ok(package_dir);
    }

    let cache_dir = package_dir
        .parent()
        .ok_or_else(|| anyhow!("Invalid shell webui cache path: {}", package_dir.display()))?;
    fs::create_dir_all(cache_dir)?;

    let temp_dir = tempfile::Builder::new()
        .prefix("shell-webui-")
        .tempdir_in(cache_dir)
        .with_context(|| format!("Failed to create temp dir in {}", cache_dir.display()))?;
    let spec = format!("{package}@{version}");
    let output = Command::new(crate::npm::command())
        .arg("pack")
        .arg("--json")
        .arg(&spec)
        .arg("--pack-destination")
        .arg(temp_dir.path())
        .current_dir(project_root)
        .output()
        .with_context(|| format!("Failed to run npm pack {spec}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "npm pack {spec} failed with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let packed: Vec<NpmPackResult> =
        serde_json::from_slice(&output.stdout).context("Failed to parse npm pack --json output")?;
    let tarball = packed
        .first()
        .map(|info| temp_dir.path().join(&info.filename))
        .ok_or_else(|| anyhow!("npm pack {spec} returned no tarball"))?;
    if !tarball.is_file() {
        return Err(anyhow!(
            "npm pack {spec} did not create expected tarball: {}",
            tarball.display()
        ));
    }

    let extract_dir = temp_dir.path().join("extract");
    fs::create_dir_all(&extract_dir)?;
    let tar_gz = fs::File::open(&tarball)
        .with_context(|| format!("Failed to open {}", tarball.display()))?;
    Archive::new(GzDecoder::new(tar_gz))
        .unpack(&extract_dir)
        .with_context(|| format!("Failed to unpack {}", tarball.display()))?;

    let unpacked_package = extract_dir.join("package");
    if !unpacked_package.join("lxapp.json").exists() {
        return Err(anyhow!(
            "lxapp package {spec} must contain lxapp.json at package root"
        ));
    }
    if !unpacked_package.join("dist").is_dir() {
        return Err(anyhow!("lxapp package {spec} must contain prebuilt dist/"));
    }

    if package_dir.exists() {
        fs::remove_dir_all(&package_dir)
            .with_context(|| format!("Failed to remove {}", package_dir.display()))?;
    }
    fs::rename(&unpacked_package, &package_dir).with_context(|| {
        format!(
            "Failed to move shell webui package into cache: {}",
            package_dir.display()
        )
    })?;

    Ok(package_dir)
}

fn sanitize_package_name(package: &str) -> String {
    package
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
