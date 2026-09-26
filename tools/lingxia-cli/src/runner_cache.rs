//! Runner acquisition + caching for end-user installs.
//!
//! `lingxia dev` launches a pre-built "LingXia Runner" from
//! `~/.lingxia/runner/<version>/`. End users who installed only the CLI (via
//! `install.sh` / `install.ps1`) don't have it, so this module downloads the
//! per-platform runner zip published to the `lingxia-cli-v<version>` GitHub
//! release, verifies its SHA-256 against `SHASUMS256-<version>.txt`, and unpacks
//! it into the cache. The unpacked app's existence marks the version installed,
//! so repeat launches short-circuit with no network (mirrors `sdk_cache.rs`).
//!
//! The version alone does not name a build: every commit between two releases
//! reports the same one. So each install records what it is in
//! [`BUILD_STAMP`] beside the app — a release asset of its version, or a local
//! build of a commit (`install-local-runner.sh` / `.ps1`) — and
//! [`ensure_matching_runner`] runs only a Runner of this CLI's own build.

use crate::github;
use crate::sdk_cache::{sha256_hex, shasum_for};
use anyhow::{Context, Result, anyhow, bail};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// macOS app bundle name as it was before `lingxia package` began naming
/// artifacts from the project. Only a fallback for reporting a path that does
/// not exist — never used to find an installed bundle; see [`runner_path`].
#[cfg(not(target_os = "windows"))]
const RUNNER_APP_NAME_FALLBACK: &str = "LingXiaRunner.app";
/// Windows runner exe stem (matches `commands/dev.rs` `RUNNER_WINDOWS_BIN_NAME`).
#[cfg(target_os = "windows")]
const RUNNER_WINDOWS_BIN_NAME: &str = "lingxia-runner";

/// Root of the runner cache: `<state root>/runner`.
fn runner_root() -> Result<PathBuf> {
    Ok(crate::state_root::lingxia_dir()?.join("runner"))
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
        // The zip names the bundle; we do not. `lingxia package` names its
        // artifacts from the project, which turned "LingXia Runner.app" into
        // "LingXiaRunner.app" and left this lookup pointing at nothing —
        // `lingxia dev` then reported the install as incomplete. A version dir
        // holds exactly one bundle, so read the name off disk.
        fs::read_dir(dir)
            .ok()
            .and_then(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("app"))
            })
            .unwrap_or_else(|| dir.join(RUNNER_APP_NAME_FALLBACK))
    }
}

/// Beside the Runner in its version dir: which build it is.
pub const BUILD_STAMP: &str = "runner-build.json";

/// What an installed Runner is, from its [`BUILD_STAMP`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RunnerBuild {
    pub version: String,
    /// The commit a local build was made from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// The published release asset of `version`.
    #[serde(default)]
    pub release: bool,
}

impl RunnerBuild {
    fn describe(&self) -> String {
        match (&self.commit, self.release) {
            (Some(commit), _) => format!("{} built from {}", self.version, short(commit)),
            (None, true) => format!("{} from its release", self.version),
            (None, false) => self.version.clone(),
        }
    }
}

/// The build of this CLI a Runner must match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliBuild {
    pub version: String,
    /// `None` when the CLI was built outside a git checkout: then only the
    /// version can be checked.
    pub commit: Option<String>,
    /// Built by the release pipeline: its Runner is its version's release asset.
    pub release: bool,
}

impl CliBuild {
    pub fn current() -> Self {
        let commit = env!("LINGXIA_COMMIT_HASH");
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            commit: (commit != "unknown" && !commit.is_empty()).then(|| commit.to_string()),
            release: !env!("LINGXIA_RELEASE_BUILD").is_empty(),
        }
    }

    /// Whether `runner` (its stamp; `None` when it has none) is this build's.
    pub fn matches(&self, runner: Option<&RunnerBuild>) -> bool {
        let Some(commit) = &self.commit else {
            // Nothing to tell builds apart by: the version dir decides.
            return true;
        };
        let Some(runner) = runner.filter(|runner| runner.version == self.version) else {
            return false;
        };
        match &runner.commit {
            Some(theirs) => same_commit(theirs, commit),
            None => runner.release && self.release,
        }
    }

    fn describe(&self) -> String {
        match &self.commit {
            Some(commit) => format!("{} ({})", self.version, short(commit)),
            None => self.version.clone(),
        }
    }
}

fn same_commit(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim().to_ascii_lowercase(), b.trim().to_ascii_lowercase());
    !a.is_empty() && !b.is_empty() && (a.starts_with(&b) || b.starts_with(&a))
}

fn short(commit: &str) -> &str {
    &commit[..commit.len().min(9)]
}

fn read_stamp(dir: &Path) -> Option<RunnerBuild> {
    let text = fs::read_to_string(dir.join(BUILD_STAMP)).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_stamp(dir: &Path, build: &RunnerBuild) -> Result<()> {
    let path = dir.join(BUILD_STAMP);
    let text = serde_json::to_string_pretty(build)?;
    fs::write(&path, text).with_context(|| format!("Failed to write {}", path.display()))
}

/// The script that builds and installs a Runner from this CLI's checkout.
fn local_install_command() -> String {
    let script = if cfg!(target_os = "windows") {
        "tools/lingxia-runner/windows/install-local-runner.ps1"
    } else {
        "tools/lingxia-runner/macos/install-local-runner.sh"
    };
    // This CLI's own checkout, when it is still where it was built.
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match checkout.join(script).canonicalize() {
        Ok(path) => path.display().to_string(),
        Err(_) => script.to_string(),
    }
}

/// Why `found` (the stamp; `None` if there is none) is not `cli`'s Runner,
/// and the fix — one line.
fn mismatch_message(cli: &CliBuild, found: Option<&RunnerBuild>) -> String {
    let what = match found {
        Some(build) => format!("the installed Runner is {}, not", build.describe()),
        None => {
            "the installed Runner does not record its build, so it cannot be matched to".to_string()
        }
    };
    format!(
        "{what} this CLI's build {}: rebuild it from this checkout with {} (or {}=1)",
        cli.describe(),
        local_install_command(),
        lingxia_control_protocol::dev_session::compat::ALLOW_SKEW_ENV
    )
}

/// The Runner of this CLI's build, installed if it can be: a release CLI
/// fetches its release asset (replacing a cached Runner of another build,
/// and saying so); a CLI built from a checkout needs a Runner built from the
/// same commit, and fails naming the script that builds one.
/// `LINGXIA_ALLOW_SKEW=1` runs the installed Runner anyway, with a warning.
pub fn ensure_matching_runner() -> Result<PathBuf> {
    let cli = CliBuild::current();
    let dir = runner_root()?.join(&cli.version);
    let path = runner_path(&dir);
    let installed = path.exists().then(|| read_stamp(&dir));
    match &installed {
        Some(stamp) if cli.matches(stamp.as_ref()) => return Ok(path),
        Some(stamp) if !cli.release => {
            let message = mismatch_message(&cli, stamp.as_ref());
            if lingxia_control_protocol::dev_session::compat::skew_allowed() {
                eprintln!("warning: {message}");
                return Ok(path);
            }
            bail!("{message}");
        }
        Some(stamp) => eprintln!(
            "Replacing the cached Runner ({}) with this CLI's release Runner {}...",
            stamp
                .as_ref()
                .map_or_else(|| "build not recorded".to_string(), RunnerBuild::describe),
            cli.version
        ),
        None if cli.commit.is_some() && !cli.release => bail!(
            "LingXia Runner {} is not installed for this CLI build {}: build it from this \
             checkout with {}",
            cli.version,
            cli.describe(),
            local_install_command()
        ),
        None => {}
    }
    ensure_runner(&cli.version, installed.is_some())
}

/// `lingxia doctor --project`: the installed Runner of this CLI's version, and
/// whether `lingxia dev` runs it (`Err`: the one-line reason and fix).
pub fn installed_report() -> (String, std::result::Result<(), String>) {
    let cli = CliBuild::current();
    let Ok(root) = runner_root() else {
        return ("unknown".into(), Ok(()));
    };
    let dir = root.join(&cli.version);
    if !runner_path(&dir).exists() {
        return if cli.commit.is_some() && !cli.release {
            (
                "not installed".into(),
                Err(format!(
                    "no Runner for this CLI build {}: build it from this checkout with {}",
                    cli.describe(),
                    local_install_command()
                )),
            )
        } else {
            ("not installed (`lingxia dev` fetches it)".into(), Ok(()))
        };
    }
    let stamp = read_stamp(&dir);
    let label = stamp.as_ref().map_or_else(
        || format!("{} (build not recorded)", cli.version),
        RunnerBuild::describe,
    );
    // A release CLI replaces another build by itself on the next `lingxia dev`.
    if cli.matches(stamp.as_ref()) || cli.release {
        (label, Ok(()))
    } else {
        (label, Err(mismatch_message(&cli, stamp.as_ref())))
    }
}

/// Ensure the runner for `version` is installed under
/// `~/.lingxia/runner/<version>/` and return its path. On a cache hit (the app
/// is present) returns immediately with no network, unless `force`.
pub fn ensure_runner(version: &str, force: bool) -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    crate::platform::windows::ensure_supported_host()?;

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
    write_stamp(
        &dir,
        &RunnerBuild {
            version: version.to_string(),
            commit: None,
            release: true,
        },
    )?;

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
/// (macOS) or the exe (Windows) at the top level, no strip.
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

    fn cli(commit: Option<&str>, release: bool) -> CliBuild {
        CliBuild {
            version: "0.19.0".into(),
            commit: commit.map(str::to_string),
            release,
        }
    }

    fn local(commit: &str) -> RunnerBuild {
        RunnerBuild {
            version: "0.19.0".into(),
            commit: Some(commit.into()),
            release: false,
        }
    }

    fn release() -> RunnerBuild {
        RunnerBuild {
            version: "0.19.0".into(),
            commit: None,
            release: true,
        }
    }

    #[test]
    fn a_checkout_build_runs_only_a_runner_of_its_commit() {
        let dev = cli(Some("abcdef0123456789"), false);
        assert!(dev.matches(Some(&local("abcdef0"))));
        assert!(!dev.matches(Some(&local("1234567"))));
        // Same version, older commit: the stale Runner this check exists for.
        assert!(!dev.matches(None));
        assert!(!dev.matches(Some(&release())));
        let other_version = RunnerBuild {
            version: "0.18.0".into(),
            ..local("abcdef0")
        };
        assert!(!dev.matches(Some(&other_version)));
    }

    #[test]
    fn a_release_build_runs_its_release_runner() {
        let shipped = cli(Some("abcdef0123456789"), true);
        assert!(shipped.matches(Some(&release())));
        assert!(shipped.matches(Some(&local("abcdef0"))));
        assert!(!shipped.matches(None));
        assert!(!shipped.matches(Some(&local("1234567"))));
        // Without a commit only the version dir tells.
        assert!(cli(None, false).matches(None));
    }

    #[test]
    fn a_mismatch_is_one_line_with_the_fix() {
        let dev = cli(Some("abcdef0123456789"), false);
        let message = mismatch_message(&dev, Some(&local("1234567890")));
        assert!(!message.contains('\n'), "{message}");
        assert!(
            message.starts_with(
                "the installed Runner is 0.19.0 built from 123456789, not this CLI's build \
                 0.19.0 (abcdef012): rebuild it from this checkout with "
            ),
            "{message}"
        );
        assert!(message.contains("install-local-runner"), "{message}");
        assert!(message.ends_with("(or LINGXIA_ALLOW_SKEW=1)"), "{message}");
        assert!(mismatch_message(&dev, None).starts_with(
            "the installed Runner does not record its build, so it cannot be matched to this \
                 CLI's build 0.19.0 (abcdef012)"
        ));
    }

    #[test]
    fn the_stamp_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_stamp(dir.path()), None);
        write_stamp(dir.path(), &local("abcdef0")).unwrap();
        assert_eq!(read_stamp(dir.path()), Some(local("abcdef0")));
        // What the install scripts write.
        fs::write(
            dir.path().join(BUILD_STAMP),
            r#"{"version":"0.19.0","commit":"abcdef0"}"#,
        )
        .unwrap();
        assert_eq!(read_stamp(dir.path()), Some(local("abcdef0")));
    }

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
