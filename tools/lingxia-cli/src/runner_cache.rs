//! Runner acquisition + caching for end-user installs.
//!
//! `lingxia dev` launches a pre-built "LingXia Runner" from
//! `~/.lingxia/runner/<version>/`. End users who installed only the CLI (via
//! `install.sh` / `install.ps1`) don't have it, so this module downloads the
//! per-platform runner zip published to the `lingxia-cli-v<version>` GitHub
//! release, verifies its SHA-256 against `SHASUMS256-<version>.txt`, and unpacks
//! it into the cache. The unpacked app's existence marks the version installed,
//! so repeat launches short-circuit with no network (mirrors `sdk_cache.rs`).

use crate::github;
use crate::sdk_cache::{sha256_hex, shasum_for};
use anyhow::{Context, Result, anyhow, bail};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// macOS app bundle name (matches `commands/dev.rs` `RUNNER_APP_NAME`).
#[cfg(not(target_os = "windows"))]
const RUNNER_APP_NAME: &str = "LingXia Runner.app";
/// Windows runner exe stem (matches `commands/dev.rs` `RUNNER_WINDOWS_BIN_NAME`).
#[cfg(target_os = "windows")]
const RUNNER_WINDOWS_BIN_NAME: &str = "lingxia-runner";

/// Root of the runner cache: `~/.lingxia/runner`.
fn runner_root() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .ok_or_else(|| anyhow!("Could not resolve home directory for the runner cache"))?
        .join(".lingxia")
        .join("runner"))
}

/// GitHub release tag carrying the runner (same release as the CLI binary).
fn release_tag(version: &str) -> String {
    format!("lingxia-cli-v{version}")
}

/// Platform runner asset name published by `scripts/release/runner.sh`.
fn asset_name(version: &str) -> Result<String> {
    #[cfg(target_os = "windows")]
    {
        // Only an x64 Windows runner is published (runs under emulation on arm64).
        let _ = version;
        Ok("lingxia-runner-windows-x64.zip".to_string())
    }
    #[cfg(target_os = "macos")]
    {
        let arch = match std::env::consts::ARCH {
            "aarch64" => "arm64",
            "x86_64" => "x64",
            other => bail!("unsupported macOS architecture '{other}' for the runner"),
        };
        Ok(format!("lingxia-runner-{version}-macos-{arch}.zip"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = version;
        bail!("The LingXia Runner is only available on macOS and Windows")
    }
}

/// The resolved runner path inside a version dir: the `.app` on macOS, the
/// `.exe` on Windows.
fn runner_path(dir: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dir.join(format!("{RUNNER_WINDOWS_BIN_NAME}.exe"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        dir.join(RUNNER_APP_NAME)
    }
}

/// Ensure the runner for `version` is installed under
/// `~/.lingxia/runner/<version>/` and return its path. On a cache hit (the app
/// is present) returns immediately with no network, unless `force`.
pub fn ensure_runner(version: &str, force: bool) -> Result<PathBuf> {
    let dir = runner_root()?.join(version);
    let path = runner_path(&dir);
    // Published by an atomic rename below (and by install-local-runner.sh's
    // atomic mv), so the app only ever appears fully-formed — its existence
    // alone means this version is installed. No separate ready-marker needed.
    if !force && path.exists() {
        return Ok(path);
    }

    let asset = asset_name(version)?;
    let tag = release_tag(version);
    let repo = github::release_repo();

    let asset_bytes = github::download_release_asset_from_repo(&repo, &tag, &asset)
        .with_context(|| format!("Failed to download runner '{asset}' from {repo} ({tag})"))?;

    let shasums_name = format!("SHASUMS256-{version}.txt");
    let shasums_bytes = github::download_release_asset_from_repo(&repo, &tag, &shasums_name)
        .with_context(|| format!("Failed to download '{shasums_name}' from {repo} ({tag})"))?;
    let shasums_text = String::from_utf8_lossy(&shasums_bytes);

    let expected = shasum_for(&shasums_text, &asset)
        .ok_or_else(|| anyhow!("{shasums_name} has no entry for '{asset}'"))?;
    let actual = sha256_hex(&asset_bytes);
    if actual != expected {
        bail!(
            "SHA-256 verification failed for runner '{asset}'\n  Expected: {expected}\n  Actual:   {actual}"
        );
    }

    // Unpack into a temp dir, then atomically rename into place.
    let tmp = dir.with_extension("tmp");
    if tmp.exists() {
        fs::remove_dir_all(&tmp)
            .with_context(|| format!("Failed to clean stale temp dir {}", tmp.display()))?;
    }
    fs::create_dir_all(&tmp)
        .with_context(|| format!("Failed to create temp dir {}", tmp.display()))?;

    if let Err(err) = extract_zip(&asset_bytes, &tmp) {
        let _ = fs::remove_dir_all(&tmp);
        return Err(err);
    }

    if dir.exists() {
        fs::remove_dir_all(&dir)
            .with_context(|| format!("Failed to remove stale runner dir {}", dir.display()))?;
    }
    if let Some(parent) = dir.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create runner parent {}", parent.display()))?;
    }
    fs::rename(&tmp, &dir).with_context(|| {
        format!(
            "Failed to publish runner cache\n  From: {}\n  To: {}",
            tmp.display(),
            dir.display()
        )
    })?;

    let path = runner_path(&dir);
    if !path.exists() {
        bail!("Runner install incomplete (missing {})", path.display());
    }

    prune_other_versions(version);
    Ok(path)
}

/// Best-effort removal of other `~/.lingxia/runner/<version>/` dirs so the cache
/// keeps just the current runner.
fn prune_other_versions(keep: &str) {
    let Ok(root) = runner_root() else {
        return;
    };
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == keep || name.starts_with('.') {
            continue;
        }
        if entry.path().is_dir() {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// Extract a zip preserving paths, Unix modes, and symlinks (a notarized `.app`
/// bundle contains symlinks; mangling them breaks its code signature). Skips
/// macOS archive junk (`__MACOSX/`, `.DS_Store`). The runner zip has the `.app`
/// (macOS) or the exe + `VERSION` (Windows) at the top level — no strip.
fn extract_zip(bytes: &[u8], out_dir: &Path) -> Result<()> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(reader).context("Failed to read downloaded runner zip")?;
    let mut written = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .with_context(|| format!("Failed to read zip entry #{index}"))?;
        let Some(enclosed) = entry.enclosed_name() else {
            continue; // skip unsafe/absolute paths
        };
        let rel = enclosed.to_string_lossy().replace('\\', "/");
        if rel.is_empty()
            || rel == "__MACOSX"
            || rel.starts_with("__MACOSX/")
            || rel.ends_with(".DS_Store")
        {
            continue;
        }

        let out_path = out_dir.join(&rel);
        if entry.is_dir() {
            fs::create_dir_all(&out_path)
                .with_context(|| format!("Failed to create {}", out_path.display()))?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }

        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .with_context(|| format!("Failed to read zip entry {rel}"))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = entry.unix_mode().unwrap_or(0o644);
            // S_IFLNK: the entry data is the link target.
            if mode & 0o170000 == 0o120000 {
                let target = String::from_utf8_lossy(&data);
                let _ = fs::remove_file(&out_path);
                std::os::unix::fs::symlink(target.as_ref(), &out_path)
                    .with_context(|| format!("Failed to create symlink {}", out_path.display()))?;
                written += 1;
                continue;
            }
            fs::write(&out_path, &data)
                .with_context(|| format!("Failed to write {}", out_path.display()))?;
            let _ = fs::set_permissions(&out_path, fs::Permissions::from_mode(mode));
        }
        #[cfg(not(unix))]
        {
            fs::write(&out_path, &data)
                .with_context(|| format!("Failed to write {}", out_path.display()))?;
        }
        written += 1;
    }
    if written == 0 {
        bail!("Runner zip contained no files (unexpected archive layout)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_tag_matches_cli_release() {
        assert_eq!(release_tag("0.10.0"), "lingxia-cli-v0.10.0");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_asset_name_uses_arch_suffix() {
        let name = asset_name("0.10.0").unwrap();
        assert!(
            name == "lingxia-runner-0.10.0-macos-x64.zip"
                || name == "lingxia-runner-0.10.0-macos-arm64.zip",
            "unexpected asset name: {name}"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_asset_name_is_x64() {
        assert_eq!(
            asset_name("0.10.0").unwrap(),
            "lingxia-runner-windows-x64.zip"
        );
    }
}
