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
/// Timeout for the lightweight npm registry version check (seconds).
const SKILL_REGISTRY_TIMEOUT_SECS: u64 = 5;
/// npm registry endpoint for the latest published `@lingxia/skill` version.
const SKILL_LATEST_URL: &str = "https://registry.npmjs.org/@lingxia/skill/latest";
/// Skill manifest under the home dir (the global `--user` install) recording the
/// installed skill version. The skill body lives globally, not per project.
const SKILL_MANIFEST_REL_PATH: &str = ".claude/skills/lingxia/skill-manifest.json";
const UPDATE_ERROR_LOG_REL_PATH: &str = ".lingxia/cli/update-error.log";

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
    /// Latest published `@lingxia/skill` version (cached in the same 24h window).
    #[serde(default)]
    latest_skill_version: Option<String>,
}

#[derive(Debug)]
struct UpdateStatus {
    current_version: Version,
    latest_version: Version,
    latest_tag: String,
    release_repo: String,
    update_available: bool,
    latest_skill_version: Option<String>,
}

pub fn maybe_auto_update() {
    notify_deferred_update_failure();

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

    if status.update_available {
        println!(
            "Updating LingXia CLI {} -> {}...",
            status.current_version, status.latest_version
        );
        if let Err(err) = install_update(&exe_path, &status) {
            eprintln!("warning: automatic CLI update failed: {err}");
            eprintln!("Continuing with the current CLI version.");
        }
    }

    notify_outdated_skill(status.latest_skill_version.as_deref());
}

/// If an installed project skill is older than the latest published one, print a
/// one-line notice (best effort). No skill installed → no nag.
fn notify_outdated_skill(latest_skill_version: Option<&str>) {
    let Some(latest) = latest_skill_version else {
        return;
    };
    let Some(installed) = installed_skill_version() else {
        return;
    };
    let (Ok(latest_v), Ok(installed_v)) = (Version::parse(latest), Version::parse(&installed))
    else {
        return;
    };
    if latest_v > installed_v {
        println!(
            "A newer @lingxia/skill is available ({installed} -> {latest}). \
             Run `npx @lingxia/skill install` to update."
        );
    }
}

/// Installed skill version from the global `~/.claude/skills/lingxia/
/// skill-manifest.json`, if present.
fn installed_skill_version() -> Option<String> {
    let manifest = dirs::home_dir()?.join(SKILL_MANIFEST_REL_PATH);
    let text = fs::read_to_string(manifest).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value.get("version")?.as_str().map(str::to_string)
}

/// Latest published `@lingxia/skill` version from the npm registry (best effort;
/// any network/parse error yields `None`).
fn fetch_latest_skill_version() -> Option<String> {
    let agent = crate::http_client::create_agent(SKILL_REGISTRY_TIMEOUT_SECS);
    let mut response = agent
        .get(SKILL_LATEST_URL)
        .header("User-Agent", "lingxia-cli")
        .header("Accept", "application/json")
        .call()
        .ok()?;
    if response.status().as_u16() != 200 {
        return None;
    }
    let body = response.body_mut().read_to_string().ok()?;
    let value: serde_json::Value = serde_json::from_str(&body).ok()?;
    value.get("version")?.as_str().map(str::to_string)
}

fn install_update(exe_path: &Path, status: &UpdateStatus) -> Result<()> {
    let asset_name = current_platform_asset_name()?;
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

    let cache_path = update_cache_path();
    let install_result = (|| -> Result<BinaryReplace> {
        fs::write(&temp_path, &bytes)
            .with_context(|| format!("Failed to write temp binary: {}", temp_path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o755))
                .with_context(|| format!("Failed to chmod {}", temp_path.display()))?;
        }

        replace_current_binary(
            &temp_path,
            exe_path,
            &status.latest_version.to_string(),
            cache_path.as_deref(),
        )
    })();

    if install_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    let replace = install_result?;

    match replace {
        #[cfg(not(target_os = "windows"))]
        BinaryReplace::Complete => {
            update_install_metadata_version(exe_path, &status.latest_version.to_string())?;
            if let Some(cache_path) = cache_path
                && cache_path.exists()
            {
                let _ = fs::remove_file(cache_path);
            }
            println!("Updated LingXia CLI to {}", status.latest_version);
        }
        #[cfg(target_os = "windows")]
        BinaryReplace::Deferred => {
            println!(
                "Staged LingXia CLI update to {}; it will be installed after this command exits.",
                status.latest_version
            );
            if let Some(log_path) = update_error_log_path() {
                println!(
                    "If the background update fails, details will be written to {}",
                    log_path.display()
                );
            }
        }
    }

    // The CLI, lxdev, and the runner ship from one release, so update them
    // together. Both are best-effort: a failure (e.g. offline, or an older
    // release without the asset) must not fail the CLI update.

    // lxdev: the devtools binary installed alongside the CLI.
    update_sibling_binary(parent, "lxdev", &status.release_repo, &status.latest_tag);

    // Runner: `lingxia dev` re-provisions if this misses.
    let runner_version = status.latest_version.to_string();
    if let Err(err) = crate::runner_cache::ensure_runner(&runner_version, true) {
        eprintln!("warning: failed to update the LingXia Runner to {runner_version}: {err}");
        eprintln!("It will be fetched on the next `lingxia dev`.");
    }
    Ok(())
}

/// Best-effort refresh of a sibling release binary (e.g. `lxdev`) next to the
/// CLI. Downloads the platform asset and atomically swaps it; warns and returns
/// on any failure so the CLI update itself is never blocked. Reached only on
/// platforms where auto-update runs (see `current_platform_asset_name`).
fn update_sibling_binary(dir: &Path, name: &str, repo: &str, tag: &str) {
    let suffix = match platform_suffix() {
        Ok(suffix) => suffix,
        Err(_) => return,
    };
    let asset = format!("{name}-{suffix}");
    let bytes = match github::download_release_asset_from_repo(repo, tag, &asset) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("warning: failed to update {name}: {err}");
            return;
        }
    };
    let dest = dir.join(binary_file_name(name));
    let temp_path = dir.join(format!(".{name}-update-{}", std::process::id()));
    let result = (|| -> Result<BinaryReplace> {
        fs::write(&temp_path, &bytes)
            .with_context(|| format!("Failed to write temp binary: {}", temp_path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o755))
                .with_context(|| format!("Failed to chmod {}", temp_path.display()))?;
        }
        replace_sibling_binary(&temp_path, &dest, name)
    })();
    if let Err(err) = result {
        let _ = fs::remove_file(&temp_path);
        eprintln!("warning: failed to update {name}: {err}");
    }
}

enum BinaryReplace {
    #[cfg(not(target_os = "windows"))]
    Complete,
    #[cfg(target_os = "windows")]
    Deferred,
}

#[cfg(not(target_os = "windows"))]
fn replace_current_binary(
    temp_path: &Path,
    exe_path: &Path,
    _version: &str,
    _cache_path: Option<&Path>,
) -> Result<BinaryReplace> {
    fs::rename(temp_path, exe_path).with_context(|| {
        format!(
            "Failed to replace current executable\n  From: {}\n  To: {}",
            temp_path.display(),
            exe_path.display()
        )
    })?;
    Ok(BinaryReplace::Complete)
}

#[cfg(target_os = "windows")]
fn replace_current_binary(
    temp_path: &Path,
    exe_path: &Path,
    version: &str,
    cache_path: Option<&Path>,
) -> Result<BinaryReplace> {
    use std::os::windows::process::CommandExt;

    // Windows keeps the running .exe locked, so the CLI cannot atomically
    // overwrite itself in-process. Hand the final swap to a detached helper
    // that waits for this process to exit. Metadata/cache changes live in the
    // helper too, after Move-Item succeeds, so a failed deferred swap never
    // records the old binary as updated.
    let pid = std::process::id();
    let metadata_path = exe_path.with_file_name(INSTALL_META_NAME);
    let cache_cleanup = cache_path
        .map(|path| {
            format!(
                "if (Test-Path -LiteralPath {}) {{ Remove-Item -LiteralPath {} -Force; }}",
                ps_single_quote(path),
                ps_single_quote(path),
            )
        })
        .unwrap_or_default();
    let log_path = update_error_log_path()
        .unwrap_or_else(|| exe_path.with_file_name(".lingxia-update-error.log"));
    let script = format!(
        "$ErrorActionPreference='Stop'; \
         $log={}; \
         try {{ \
           Wait-Process -Id {pid} -ErrorAction SilentlyContinue; \
           Move-Item -LiteralPath {} -Destination {} -Force; \
         }} catch {{ \
           $parent = [System.IO.Path]::GetDirectoryName($log); \
           if ($parent) {{ New-Item -ItemType Directory -Path $parent -Force | Out-Null; }} \
           $message = (Get-Date -Format o) + ' LingXia CLI deferred update failed: ' + $_.Exception.Message; \
           $utf8 = [System.Text.UTF8Encoding]::new($false); \
           [System.IO.File]::WriteAllText($log, $message, $utf8); \
           exit 1; \
         }}; \
         try {{ \
           if (Test-Path -LiteralPath {}) {{ \
             $metadata = Get-Content -Raw -LiteralPath {} | ConvertFrom-Json; \
             $metadata.version = {}; \
             $json = $metadata | ConvertTo-Json -Depth 4; \
             $utf8 = [System.Text.UTF8Encoding]::new($false); \
             [System.IO.File]::WriteAllText({}, $json, $utf8); \
           }} \
         }} catch {{}}; \
         try {{ {} }} catch {{}}; \
         try {{ if (Test-Path -LiteralPath $log) {{ Remove-Item -LiteralPath $log -Force; }} }} catch {{}}",
        ps_single_quote(&log_path),
        ps_single_quote(temp_path),
        ps_single_quote(exe_path),
        ps_single_quote(&metadata_path),
        ps_single_quote(&metadata_path),
        ps_single_quote_str(version),
        ps_single_quote(&metadata_path),
        cache_cleanup,
    );
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .context("Failed to schedule deferred Windows CLI replacement")?;
    Ok(BinaryReplace::Deferred)
}

#[cfg(not(target_os = "windows"))]
fn replace_sibling_binary(temp_path: &Path, dest: &Path, _name: &str) -> Result<BinaryReplace> {
    fs::rename(temp_path, dest).with_context(|| format!("Failed to replace {}", dest.display()))?;
    Ok(BinaryReplace::Complete)
}

#[cfg(target_os = "windows")]
fn replace_sibling_binary(temp_path: &Path, dest: &Path, name: &str) -> Result<BinaryReplace> {
    match fs::rename(temp_path, dest) {
        Ok(()) => Ok(BinaryReplace::Deferred),
        Err(rename_err) => {
            schedule_deferred_sibling_replace(temp_path, dest, name).with_context(|| {
                format!(
                    "Failed to replace {} now ({rename_err}); also failed to stage deferred replacement",
                    dest.display()
                )
            })?;
            eprintln!("warning: {name}.exe is in use; staged update after it exits");
            Ok(BinaryReplace::Deferred)
        }
    }
}

#[cfg(target_os = "windows")]
fn schedule_deferred_sibling_replace(temp_path: &Path, dest: &Path, name: &str) -> Result<()> {
    use std::os::windows::process::CommandExt;

    let log_path =
        update_error_log_path().unwrap_or_else(|| dest.with_file_name(".lingxia-update-error.log"));
    let script = format!(
        "$ErrorActionPreference='Stop'; \
         $log={}; \
         try {{ \
           Wait-Process -Name {} -ErrorAction SilentlyContinue; \
           Move-Item -LiteralPath {} -Destination {} -Force; \
           if (Test-Path -LiteralPath $log) {{ Remove-Item -LiteralPath $log -Force; }} \
         }} catch {{ \
           $parent = [System.IO.Path]::GetDirectoryName($log); \
           if ($parent) {{ New-Item -ItemType Directory -Path $parent -Force | Out-Null; }} \
           $message = (Get-Date -Format o) + ' LingXia deferred update failed for {}: ' + $_.Exception.Message; \
           $utf8 = [System.Text.UTF8Encoding]::new($false); \
           [System.IO.File]::WriteAllText($log, $message, $utf8); \
           exit 1; \
         }}",
        ps_single_quote(&log_path),
        ps_single_quote_str(name),
        ps_single_quote(temp_path),
        ps_single_quote(dest),
        ps_single_quote_str(name),
    );
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .context("Failed to schedule deferred sibling replacement")?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn ps_single_quote(path: &Path) -> String {
    ps_single_quote_str(&path.display().to_string())
}

#[cfg(target_os = "windows")]
fn ps_single_quote_str(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn load_update_status(force_refresh: bool) -> Result<UpdateStatus> {
    let release_repo = release_repo_for_current_install();
    let current_version =
        Version::parse(env!("CARGO_PKG_VERSION")).context("Failed to parse current CLI version")?;

    let fresh_cache = if force_refresh {
        None
    } else {
        load_update_cache_if_fresh()?
    };

    // CLI release + latest skill version share the same 24h cache window.
    let (release, latest_skill_version) = match fresh_cache {
        Some(cache) => {
            let latest_skill_version = cache.latest_skill_version;
            match (cache.latest_version, cache.latest_tag) {
                (Some(latest_version), Some(latest_tag)) => (
                    github::CliReleaseTag {
                        tag: latest_tag,
                        version: latest_version,
                    },
                    latest_skill_version,
                ),
                _ => {
                    return Ok(UpdateStatus {
                        current_version: current_version.clone(),
                        latest_version: current_version,
                        latest_tag: String::new(),
                        release_repo,
                        update_available: false,
                        latest_skill_version,
                    });
                }
            }
        }
        None => {
            let latest_skill_version = fetch_latest_skill_version();
            match github::latest_cli_release_from_repo(&release_repo) {
                Ok(release) => {
                    persist_update_cache(
                        &release_repo,
                        Some(&release.version),
                        Some(&release.tag),
                        latest_skill_version.as_deref(),
                    );
                    (release, latest_skill_version)
                }
                Err(err) => {
                    persist_update_cache(
                        &release_repo,
                        None,
                        None,
                        latest_skill_version.as_deref(),
                    );
                    return Err(err);
                }
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
        latest_skill_version,
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
    latest_skill_version: Option<&str>,
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
        latest_skill_version: latest_skill_version.map(str::to_string),
    };
    if let Ok(payload) = serde_json::to_vec_pretty(&cache) {
        let _ = fs::write(path, payload);
    }
}

fn update_cache_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".lingxia").join("cli").join("update.json"))
}

fn update_error_log_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(UPDATE_ERROR_LOG_REL_PATH))
}

fn notify_deferred_update_failure() {
    let Some(path) = update_error_log_path() else {
        return;
    };
    let Ok(message) = fs::read_to_string(&path) else {
        return;
    };
    let message = message.trim();
    if !message.is_empty() {
        eprintln!("warning: previous LingXia CLI update failed: {message}");
    }
    let _ = fs::remove_file(path);
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
    Some(
        home.join(".local")
            .join("bin")
            .join(binary_file_name(BIN_NAME)),
    )
}

fn load_install_metadata(exe_path: &Path) -> Option<InstallMetadata> {
    let meta_path = exe_path.with_file_name(INSTALL_META_NAME);
    let text = fs::read_to_string(meta_path).ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(not(target_os = "windows"))]
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
    Ok(format!("lingxia-{}", platform_suffix()?))
}

fn binary_file_name(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// The `<os>-<arch>` suffix shared by every release binary asset
/// (`lingxia-<suffix>`, `lxdev-<suffix>`). Errors on platforms where
/// auto-update isn't supported yet.
fn platform_suffix() -> Result<String> {
    let os = match env::consts::OS {
        "macos" => "darwin",
        "linux" => "linux",
        "windows" => "windows",
        other => {
            return Err(anyhow!(
                "Automatic CLI update is not supported on this OS yet: {}",
                other
            ));
        }
    };
    // Release assets use raw Rust target_arch names (e.g. x86_64, aarch64).
    let arch = match env::consts::ARCH {
        "x86_64" | "aarch64" => env::consts::ARCH,
        other => {
            return Err(anyhow!(
                "Automatic CLI update is not supported on this architecture yet: {}",
                other
            ));
        }
    };
    let ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };
    Ok(format!("{os}-{arch}{ext}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_platform_asset_name_uses_release_naming() {
        let asset = current_platform_asset_name();
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") || cfg!(target_os = "windows") {
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
            latest_skill_version: Some("9.9.9".to_string()),
        };
        let age = current_unix_secs().saturating_sub(cache.checked_at_unix_secs);
        assert!(age > UPDATE_CHECK_INTERVAL_SECS);
    }
}
