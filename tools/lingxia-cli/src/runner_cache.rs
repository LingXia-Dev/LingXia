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
//! The version directory is the cache key. A Runner of this CLI's version runs
//! when it speaks a dev protocol this CLI still speaks — a release asset of
//! that version included. The protocol is the one its [`BUILD_STAMP`] records,
//! or, for a stamp without one, the constant committed at its commit or
//! release tag.

use crate::github;
use crate::sdk_cache::{sha256_hex, shasum_for};
use anyhow::{Context, Result, anyhow, bail};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use lingxia_control_protocol::dev_session::compat;

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
    /// Dev-session protocol this Runner speaks. Absent on installs from before
    /// the field existed; a same-version install without it still runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<u32>,
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

/// Why `found` is not a Runner this CLI can drive, and the fix — one line.
fn mismatch_message(cli: &CliBuild, found: &RunnerBuild, protocol: Option<u32>) -> String {
    let script = local_install_command();
    if found.version != cli.version {
        return format!(
            "the installed Runner is {}, which is not this CLI's version {}: rebuild it from \
             this checkout with {script}",
            found.describe(),
            cli.version
        );
    }
    format!(
        "the installed Runner ({}) speaks dev protocol {}; this CLI speaks {}. Rebuild it from \
         this checkout with {script}",
        found.describe(),
        protocol.map_or_else(|| "(unrecorded)".to_string(), |p| p.to_string()),
        compat::protocol_span()
    )
}

/// This CLI's checkout, when it is still where it was built.
fn checkout_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The dev protocol a Runner speaks. A stamp without one (an older install,
/// or a checkout's download of the release asset) is looked up at the
/// Runner's commit, or its version's release tag, in this CLI's checkout.
/// `None` when nothing says; the Runner's `hello` still decides at runtime.
fn runner_protocol(cli: &CliBuild, runner: &RunnerBuild) -> Option<u32> {
    if runner.protocol.is_some() || cli.release {
        return runner.protocol;
    }
    let rev = match &runner.commit {
        Some(commit) => commit.clone(),
        None if runner.release => format!("lingxia-cli-v{}", runner.version),
        None => return None,
    };
    protocol_at(&checkout_dir(), &rev)
}

/// `DEV_SESSION_PROTOCOL_VERSION` as committed at `rev`. Reads the object, not
/// the working tree, so a checkout that moved since this CLI was built does
/// not change the answer.
fn protocol_at(repo: &Path, rev: &str) -> Option<u32> {
    let output = Command::new("git")
        .arg("show")
        .arg(format!("{rev}:crates/lingxia-control-protocol/src/lib.rs"))
        .current_dir(repo)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_protocol_constant(&String::from_utf8_lossy(&output.stdout))
}

fn parse_protocol_constant(source: &str) -> Option<u32> {
    source
        .lines()
        .find(|line| {
            line.trim_start()
                .starts_with("pub const DEV_SESSION_PROTOCOL_VERSION: u32")
        })?
        .split_once('=')?
        .1
        .trim()
        .trim_end_matches(';')
        .trim()
        .parse()
        .ok()
}

/// A checkout CLI may run a Runner of its version built elsewhere, but its
/// own runtime changes are not in it. Say so, without failing.
fn foreign_build_note(cli: &CliBuild, runner: Option<&RunnerBuild>) -> Option<String> {
    let ours = cli.commit.as_deref().filter(|_| !cli.release)?;
    let theirs = runner.and_then(|runner| runner.commit.as_deref());
    if theirs.is_some_and(|theirs| same_commit(theirs, ours)) {
        return None;
    }
    let what = runner.map_or_else(|| cli.version.clone(), RunnerBuild::describe);
    Some(format!(
        "note: running Runner {what}, not this checkout ({}); runtime changes since are not in \
         it — rebuild with {} to test them",
        short(ours),
        local_install_command()
    ))
}

/// Beside the Runner: the CLI/Runner pair [`foreign_build_note`] last named.
const NOTED: &str = ".foreign-build-noted";

fn note_key(cli: &CliBuild, runner: Option<&RunnerBuild>) -> String {
    let runner = runner.map_or_else(
        || "unrecorded".to_string(),
        |runner| {
            runner
                .commit
                .clone()
                .unwrap_or_else(|| format!("release {}", runner.version))
        },
    );
    format!("{} {runner}", cli.commit.as_deref().unwrap_or(""))
}

/// Whether `key` is new for this Runner; records it. The note is for the
/// first launch of a pair, not every one.
fn claim_note(dir: &Path, key: &str) -> bool {
    let path = dir.join(NOTED);
    if fs::read_to_string(&path).is_ok_and(|seen| seen.trim() == key) {
        return false;
    }
    let _ = fs::write(&path, key);
    true
}

fn same_commit(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim().to_ascii_lowercase(), b.trim().to_ascii_lowercase());
    !a.is_empty() && !b.is_empty() && (a.starts_with(&b) || b.starts_with(&a))
}

/// What `lingxia dev` does with the installed Runner.
enum Verdict {
    Run,
    /// A release CLI fetches its own release Runner over this one.
    Replace,
    /// A checkout CLI needs a Runner built from the checkout.
    Rebuild(String),
}

fn verdict(cli: &CliBuild, stamp: Option<&RunnerBuild>) -> Verdict {
    let Some(build) = stamp else {
        return Verdict::Run;
    };
    let protocol = runner_protocol(cli, build);
    let fits = build.version == cli.version && protocol.is_none_or(compat::speaks_dev_protocol);
    match (fits, cli.release) {
        (true, _) => Verdict::Run,
        (false, true) => Verdict::Replace,
        (false, false) => Verdict::Rebuild(mismatch_message(cli, build, protocol)),
    }
}

/// `LINGXIA_SKIP_COMPAT=1` runs the Runner anyway; its `hello` still decides.
fn refuse_or_warn(message: String, path: PathBuf) -> Result<PathBuf> {
    if compat::skip_requested() {
        crate::compat::warn_skipped(&[message]);
        Ok(path)
    } else {
        Err(anyhow!("{message}"))
    }
}

/// The Runner of this CLI's version: any build of that version whose dev
/// protocol this CLI speaks, the release asset included. A release CLI
/// replaces one it cannot drive; a checkout CLI names the script that builds
/// one from the checkout.
pub fn ensure_matching_runner() -> Result<PathBuf> {
    let cli = CliBuild::current();
    let dir = runner_root()?.join(&cli.version);
    let path = if runner_path(&dir).exists() {
        let stamp = read_stamp(&dir);
        match verdict(&cli, stamp.as_ref()) {
            Verdict::Run => runner_path(&dir),
            Verdict::Replace => {
                eprintln!(
                    "Replacing the cached Runner ({}) with this CLI's release Runner {}...",
                    stamp
                        .as_ref()
                        .map_or_else(|| "build not recorded".to_string(), RunnerBuild::describe),
                    cli.version
                );
                ensure_runner(&cli.version, true)?
            }
            Verdict::Rebuild(message) => refuse_or_warn(message, runner_path(&dir))?,
        }
    } else {
        let path = ensure_runner(&cli.version, false)?;
        match verdict(&cli, read_stamp(&dir).as_ref()) {
            Verdict::Rebuild(message) => refuse_or_warn(message, path)?,
            _ => path,
        }
    };
    let stamp = read_stamp(&dir);
    if let Some(note) = foreign_build_note(&cli, stamp.as_ref())
        && claim_note(&dir, &note_key(&cli, stamp.as_ref()))
    {
        eprintln!("{note}");
    }
    Ok(path)
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
        return ("not installed (`lingxia dev` fetches it)".into(), Ok(()));
    }
    let stamp = read_stamp(&dir);
    let label = stamp.as_ref().map_or_else(
        || format!("{} (build not recorded)", cli.version),
        RunnerBuild::describe,
    );
    match verdict(&cli, stamp.as_ref()) {
        Verdict::Run => (label, Ok(())),
        Verdict::Replace => (format!("{label} (`lingxia dev` replaces it)"), Ok(())),
        Verdict::Rebuild(message) => (label, Err(message)),
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
    let release_cli = CliBuild::current().release;
    write_stamp(
        &dir,
        &RunnerBuild {
            version: version.to_string(),
            commit: None,
            release: true,
            // A checkout CLI's own constant need not be the asset's; it leaves
            // the field empty and reads the release tag instead.
            protocol: release_cli
                .then_some(lingxia_control_protocol::dev_session::DEV_SESSION_PROTOCOL_VERSION),
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
            protocol: None,
        }
    }

    fn release() -> RunnerBuild {
        RunnerBuild {
            version: "0.19.0".into(),
            commit: None,
            release: true,
            protocol: None,
        }
    }

    fn speaking(mut build: RunnerBuild, protocol: u32) -> RunnerBuild {
        build.protocol = Some(protocol);
        build
    }

    const CURRENT: u32 = lingxia_control_protocol::dev_session::DEV_SESSION_PROTOCOL_VERSION;

    #[test]
    fn a_checkout_build_runs_any_runner_of_its_version_and_protocol() {
        let dev = cli(Some("abcdef0123456789"), false);
        for runner in [
            speaking(release(), CURRENT),
            speaking(local("1234567"), CURRENT),
        ] {
            assert!(matches!(verdict(&dev, Some(&runner)), Verdict::Run));
        }
        assert!(matches!(verdict(&dev, None), Verdict::Run));
        let Verdict::Rebuild(message) = verdict(&dev, Some(&speaking(release(), 1))) else {
            panic!("an old protocol must be rebuilt");
        };
        assert!(message.contains("dev protocol 1"), "{message}");
        assert!(message.contains("install-local-runner"), "{message}");
        assert!(!message.contains('\n'), "{message}");
        let other_version = RunnerBuild {
            version: "0.18.0".into(),
            ..speaking(local("abcdef0"), CURRENT)
        };
        assert!(matches!(
            verdict(&dev, Some(&other_version)),
            Verdict::Rebuild(_)
        ));
    }

    #[test]
    fn a_release_build_replaces_what_it_cannot_drive() {
        let shipped = cli(Some("abcdef0123456789"), true);
        assert!(matches!(
            verdict(&shipped, Some(&speaking(release(), CURRENT))),
            Verdict::Run
        ));
        // No checkout to look an unrecorded protocol up in: the hello decides.
        assert!(matches!(verdict(&shipped, Some(&release())), Verdict::Run));
        assert!(matches!(
            verdict(&shipped, Some(&speaking(local("1234567"), 1))),
            Verdict::Replace
        ));
    }

    #[test]
    fn an_unrecorded_protocol_is_read_from_the_runner_commit() {
        assert_eq!(
            parse_protocol_constant("    pub const DEV_SESSION_PROTOCOL_VERSION: u32 = 3;\n"),
            Some(3)
        );
        assert_eq!(parse_protocol_constant("const X: u32 = 1;"), None);
        // This checkout's HEAD; the test runs from a git checkout.
        if let Some(protocol) = protocol_at(&checkout_dir(), "HEAD") {
            assert!(protocol >= 1);
        }
        assert_eq!(protocol_at(&checkout_dir(), "no-such-rev-xyz"), None);
    }

    #[test]
    fn a_runner_from_elsewhere_is_named_not_refused() {
        let dev = cli(Some("abcdef0123456789"), false);
        assert!(foreign_build_note(&dev, Some(&local("abcdef0"))).is_none());
        let note = foreign_build_note(&dev, Some(&release())).unwrap();
        assert!(note.contains("not this checkout (abcdef012)"), "{note}");
        assert!(foreign_build_note(&dev, Some(&local("1234567"))).is_some());
        assert!(foreign_build_note(&cli(Some("abcdef0"), true), Some(&release())).is_none());
        assert!(foreign_build_note(&cli(None, false), Some(&release())).is_none());
    }

    #[test]
    fn the_foreign_build_note_is_shown_once_per_pair() {
        let dir = tempfile::tempdir().unwrap();
        let dev = cli(Some("abcdef0123456789"), false);
        let key = note_key(&dev, Some(&release()));
        assert!(claim_note(dir.path(), &key));
        assert!(!claim_note(dir.path(), &key));
        let other = note_key(&cli(Some("1234567890"), false), Some(&release()));
        assert!(claim_note(dir.path(), &other));
        assert_ne!(key, note_key(&dev, Some(&local("1234567"))));
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
