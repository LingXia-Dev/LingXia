use crate::github;
use anyhow::{Context, Result, anyhow};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const UPDATE_CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;
const INSTALL_META_NAME: &str = "lingxia-cli-install.json";
const BIN_NAME: &str = "lingxia";

#[derive(Debug, Deserialize, Serialize)]
struct InstallMetadata {
    channel: String,
    repo: String,
    version: String,
    install_path: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct UpdateCheckCache {
    checked_at_unix_secs: u64,
    release_repo: String,
    latest_version: Option<String>,
    latest_tag: Option<String>,
}

#[derive(Debug)]
struct UpdateStatus {
    current_version: Version,
    latest_version: Version,
    latest_tag: String,
    release_repo: String,
    update_available: bool,
}

pub fn maybe_auto_update() {
    let Ok(raw_exe_path) = current_exe_path() else {
        return;
    };
    let Ok(exe_path) = raw_exe_path.canonicalize().with_context(|| {
        format!(
            "Failed to resolve current executable path: {}",
            raw_exe_path.display()
        )
    }) else {
        return;
    };

    if !is_install_sh_install(&exe_path) {
        return;
    }

    let Ok(status) = load_update_status(false) else {
        return;
    };
    if !status.update_available {
        return;
    }

    println!(
        "A newer LingXia CLI is available: {} (current {}). Updating now...",
        status.latest_version, status.current_version
    );
    if let Err(err) = install_update(&exe_path, &status) {
        eprintln!("warning: automatic CLI update failed: {err}");
        eprintln!("Continuing with the current CLI version.");
    }
}

fn install_update(exe_path: &Path, status: &UpdateStatus) -> Result<()> {
    let asset_name = current_platform_asset_name()?;
    println!(
        "Updating LingXia CLI from {} to {}",
        status.current_version, status.latest_version
    );
    let bytes = github::download_release_asset_from_repo(
        &status.release_repo,
        &status.latest_tag,
        &asset_name,
    )
    .with_context(|| {
        format!(
            "Failed to download release asset '{}' from {} ({})",
            asset_name, status.latest_tag, status.release_repo
        )
    })?;

    let parent = exe_path
        .parent()
        .ok_or_else(|| anyhow!("Executable path has no parent: {}", exe_path.display()))?;
    let temp_path = parent.join(format!(".lingxia-update-{}", std::process::id()));

    let install_result = (|| -> Result<()> {
        fs::write(&temp_path, &bytes)
            .with_context(|| format!("Failed to write temp binary: {}", temp_path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o755))
                .with_context(|| format!("Failed to chmod {}", temp_path.display()))?;
        }

        fs::rename(&temp_path, exe_path).with_context(|| {
            format!(
                "Failed to replace current executable\n  From: {}\n  To: {}",
                temp_path.display(),
                exe_path.display()
            )
        })
    })();

    if install_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    install_result?;

    update_install_metadata_version(exe_path, &status.latest_version.to_string())?;
    if let Some(cache_path) = update_cache_path()
        && cache_path.exists()
    {
        let _ = fs::remove_file(cache_path);
    }

    println!("Updated LingXia CLI to {}", status.latest_version);
    Ok(())
}

fn load_update_status(force_refresh: bool) -> Result<UpdateStatus> {
    let release_repo = release_repo_for_current_install();
    let current_version =
        Version::parse(env!("CARGO_PKG_VERSION")).context("Failed to parse current CLI version")?;
    let release = if force_refresh {
        let release = github::latest_cli_release_from_repo(&release_repo)?;
        persist_update_cache(&release_repo, Some(&release.version), Some(&release.tag));
        release
    } else if let Some(cache) = load_update_cache_if_fresh()? {
        match (cache.latest_version, cache.latest_tag) {
            (Some(latest_version), Some(latest_tag)) => github::CliReleaseTag {
                tag: latest_tag,
                version: latest_version,
            },
            _ => {
                return Ok(UpdateStatus {
                    current_version: current_version.clone(),
                    latest_version: current_version,
                    latest_tag: String::new(),
                    release_repo,
                    update_available: false,
                });
            }
        }
    } else {
        match github::latest_cli_release_from_repo(&release_repo) {
            Ok(release) => {
                persist_update_cache(&release_repo, Some(&release.version), Some(&release.tag));
                release
            }
            Err(err) => {
                persist_update_cache(&release_repo, None, None);
                return Err(err);
            }
        }
    };

    let latest_version =
        Version::parse(&release.version).context("Failed to parse latest CLI version")?;
    let update_available = latest_version > current_version;
    Ok(UpdateStatus {
        current_version,
        latest_version,
        latest_tag: release.tag,
        release_repo,
        update_available,
    })
}

fn load_update_cache_if_fresh() -> Result<Option<UpdateCheckCache>> {
    let Some(path) = update_cache_path() else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read update cache: {}", path.display()))?;
    let cache: UpdateCheckCache =
        serde_json::from_str(&text).context("Failed to parse update cache")?;
    if cache.release_repo != release_repo_for_current_install() {
        return Ok(None);
    }
    let age = current_unix_secs().saturating_sub(cache.checked_at_unix_secs);
    if age > UPDATE_CHECK_INTERVAL_SECS {
        return Ok(None);
    }
    Ok(Some(cache))
}

fn persist_update_cache(
    release_repo: &str,
    latest_version: Option<&str>,
    latest_tag: Option<&str>,
) {
    let Some(path) = update_cache_path() else {
        return;
    };
    if let Some(parent) = path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return;
    }
    let cache = UpdateCheckCache {
        checked_at_unix_secs: current_unix_secs(),
        release_repo: release_repo.to_string(),
        latest_version: latest_version.map(str::to_string),
        latest_tag: latest_tag.map(str::to_string),
    };
    if let Ok(payload) = serde_json::to_vec_pretty(&cache) {
        let _ = fs::write(path, payload);
    }
}

fn update_cache_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".lingxia").join("cli").join("update.json"))
}

fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

fn current_exe_path() -> Result<PathBuf> {
    env::current_exe().context("Failed to resolve current executable path")
}

fn is_install_sh_install(exe_path: &Path) -> bool {
    if load_install_metadata(exe_path)
        .filter(|metadata| metadata.channel == "github-release")
        .is_some_and(|metadata| {
            install_path_matches_exe(Path::new(&metadata.install_path), exe_path)
        })
    {
        return true;
    }

    default_install_path()
        .map(|path| install_path_matches_exe(&path, exe_path))
        .unwrap_or(false)
}

fn install_path_matches_exe(install_path: &Path, exe_path: &Path) -> bool {
    let configured_path = install_path
        .canonicalize()
        .unwrap_or_else(|_| install_path.to_path_buf());
    let resolved_exe_path = exe_path
        .canonicalize()
        .unwrap_or_else(|_| exe_path.to_path_buf());
    configured_path == resolved_exe_path
}

fn release_repo_for_current_install() -> String {
    if let Ok(exe_path) = current_exe_path()
        && let Some(metadata) = load_install_metadata(&exe_path)
        && is_install_sh_install(&exe_path)
        && !metadata.repo.trim().is_empty()
    {
        return metadata.repo;
    }

    github::release_repo()
}

fn default_install_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".local").join("bin").join(BIN_NAME))
}

fn load_install_metadata(exe_path: &Path) -> Option<InstallMetadata> {
    let meta_path = exe_path.with_file_name(INSTALL_META_NAME);
    let text = fs::read_to_string(meta_path).ok()?;
    serde_json::from_str(&text).ok()
}

fn update_install_metadata_version(exe_path: &Path, version: &str) -> Result<()> {
    let meta_path = exe_path.with_file_name(INSTALL_META_NAME);
    if !meta_path.exists() {
        return Ok(());
    }
    let text = fs::read_to_string(&meta_path)
        .with_context(|| format!("Failed to read install metadata: {}", meta_path.display()))?;
    let mut metadata: InstallMetadata =
        serde_json::from_str(&text).context("Failed to parse install metadata")?;
    metadata.version = version.to_string();
    let payload =
        serde_json::to_vec_pretty(&metadata).context("Failed to serialize install metadata")?;
    fs::write(&meta_path, payload)
        .with_context(|| format!("Failed to write install metadata: {}", meta_path.display()))?;
    Ok(())
}

fn current_platform_asset_name() -> Result<String> {
    let os = match env::consts::OS {
        "macos" => "darwin",
        "linux" => "linux",
        other => {
            return Err(anyhow!(
                "Automatic CLI update is not supported on this OS yet: {}",
                other
            ));
        }
    };
    let arch = match env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => {
            return Err(anyhow!(
                "Automatic CLI update is not supported on this architecture yet: {}",
                other
            ));
        }
    };
    Ok(format!("lingxia-{os}-{arch}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_platform_asset_name_uses_release_naming() {
        let asset = current_platform_asset_name();
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            assert!(asset.unwrap().starts_with("lingxia-"));
        } else {
            assert!(asset.is_err());
        }
    }

    #[test]
    fn stale_update_cache_is_ignored() {
        let cache = UpdateCheckCache {
            checked_at_unix_secs: current_unix_secs() - UPDATE_CHECK_INTERVAL_SECS - 1,
            release_repo: "LingXia-Dev/LingXia".to_string(),
            latest_version: Some("9.9.9".to_string()),
            latest_tag: Some("lingxia-cli-v9.9.9".to_string()),
        };
        let age = current_unix_secs().saturating_sub(cache.checked_at_unix_secs);
        assert!(age > UPDATE_CHECK_INTERVAL_SECS);
    }
}
