//! `lingxia.lock`: the LingXia source a project builds against.
//!
//! Identity is a git source plus a commit; semver only says which
//! compatibility line that commit is on. Official releases are the case
//! where the git source is the official repository and the ref is a release
//! tag, whose SDKs come from the GitHub release. A fork, or an unreleased
//! commit, builds its SDKs from that commit's source (cached per commit).
//!
//! `lingxia pin` is the only writer of the lock (and of the managed
//! `[patch.crates-io]` entries in the root `Cargo.toml`); nothing rewrites
//! tracked files implicitly.

pub mod cargo;
pub mod overrides;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const LOCK_FILE: &str = "lingxia.lock";
pub const OFFICIAL_GIT: &str = "https://github.com/LingXia-Dev/LingXia.git";
const LOCK_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Lock {
    pub version: u32,
    pub lingxia: Pin,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    pub git: String,
    /// What was asked for (tag, branch, or sha); `commit` is what it named.
    #[serde(rename = "ref")]
    pub reference: String,
    pub commit: String,
    /// The pinned commit's workspace version: the compatibility line and the
    /// version its artifacts are named with.
    pub workspace_version: String,
    pub cli_version: String,
    pub pinned_at: String,
}

impl Pin {
    /// An official release: the official repository at a release tag. Its
    /// crates come from crates.io and its SDKs from the GitHub release.
    pub fn is_official_release(&self) -> bool {
        same_git(&self.git, OFFICIAL_GIT) && is_release_tag(&self.reference)
    }

    pub fn short_commit(&self) -> &str {
        &self.commit[..self.commit.len().min(12)]
    }
}

/// Release tags the publish workflow creates; all of one release's tags
/// name one commit.
pub fn is_release_tag(reference: &str) -> bool {
    let tag = reference.strip_prefix("refs/tags/").unwrap_or(reference);
    tag.starts_with("lingxia-")
        && tag.contains("-v")
        && tag.rsplit("-v").next().is_some_and(is_semver)
}

fn is_semver(value: &str) -> bool {
    let parts: Vec<_> = value.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.parse::<u64>().is_ok())
}

/// The release tag `lingxia new` and a bare `lingxia pin` default to.
pub fn default_release_ref() -> String {
    format!("lingxia-crates-v{}", env!("LINGXIA_RUST_CRATE_VERSION"))
}

fn same_git(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.trim()
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .to_ascii_lowercase()
    };
    norm(a) == norm(b)
}

pub fn lock_path(root: &Path) -> PathBuf {
    root.join(LOCK_FILE)
}

pub fn read(root: &Path) -> Result<Option<Lock>> {
    let path = lock_path(root);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).with_context(|| format!("read {}", path.display())),
    };
    let lock: Lock = serde_json::from_str(&text)
        .with_context(|| format!("{} is not a valid lock; run `lingxia pin`", path.display()))?;
    if lock.version != LOCK_VERSION {
        bail!(
            "{} has lock version {}; this CLI reads version {LOCK_VERSION}",
            path.display(),
            lock.version
        );
    }
    if !is_full_sha(&lock.lingxia.commit) {
        bail!("{} has no full commit; run `lingxia pin`", path.display());
    }
    Ok(Some(lock))
}

pub fn write(root: &Path, pin: Pin) -> Result<()> {
    let lock = Lock {
        version: LOCK_VERSION,
        lingxia: pin,
    };
    let text = serde_json::to_string_pretty(&lock)? + "\n";
    let path = lock_path(root);
    fs::write(&path, text).with_context(|| format!("write {}", path.display()))
}

pub fn is_full_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

// ---------------------------------------------------------------------------
// Resolving refs

fn git(args: &[&str]) -> Command {
    let mut command = Command::new("git");
    // Private forks authenticate through the user's git credential helpers.
    command.args(args).env("GIT_TERMINAL_PROMPT", "0");
    command
}

fn git_output(mut command: Command, what: &str) -> Result<String> {
    let output = command
        .output()
        .with_context(|| format!("{what}: is git installed and on PATH?"))?;
    if !output.status.success() {
        bail!(
            "{what} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The commit `reference` names in `git_url`: a full sha as is, otherwise
/// a tag (peeled to its commit) or a branch.
pub fn resolve_ref(git_url: &str, reference: &str) -> Result<String> {
    if is_full_sha(reference) {
        return Ok(reference.to_ascii_lowercase());
    }
    let listing = git_output(
        git(&[
            "ls-remote",
            git_url,
            reference,
            &format!("{reference}^{{}}"),
        ]),
        &format!("git ls-remote {git_url} {reference}"),
    )?;
    resolve_from_listing(&listing, reference)
        .ok_or_else(|| anyhow!("{git_url} has no tag or branch named {reference}"))
}

/// Picks the commit from `git ls-remote` output: a peeled tag (`^{}`) wins
/// over the annotated tag object it peels.
fn resolve_from_listing(listing: &str, reference: &str) -> Option<String> {
    let mut direct = None;
    let short = reference
        .strip_prefix("refs/tags/")
        .or_else(|| reference.strip_prefix("refs/heads/"))
        .unwrap_or(reference);
    for line in listing.lines() {
        let mut parts = line.split_whitespace();
        let (Some(sha), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        if name == format!("refs/tags/{short}^{{}}") {
            return Some(sha.to_string());
        }
        if name == format!("refs/tags/{short}")
            || name == format!("refs/heads/{short}")
            || name == reference
        {
            direct.get_or_insert_with(|| sha.to_string());
        }
    }
    direct
}

/// A release tag of the official repository that names `commit`, so a
/// release pinned by sha is still a release.
pub fn release_tag_for(git_url: &str, commit: &str) -> Option<String> {
    if !same_git(git_url, OFFICIAL_GIT) {
        return None;
    }
    let listing = git_output(
        git(&[
            "ls-remote",
            "--tags",
            git_url,
            "refs/tags/lingxia-crates-v*",
        ]),
        "git ls-remote --tags",
    )
    .ok()?;
    listing.lines().find_map(|line| {
        let (sha, name) = line.split_once(char::is_whitespace)?;
        let tag = name
            .trim()
            .strip_prefix("refs/tags/")?
            .strip_suffix("^{}")?;
        (sha == commit && is_release_tag(tag)).then(|| tag.to_string())
    })
}

// ---------------------------------------------------------------------------
// Source checkouts

/// `<project>/.lingxia/lingxia-src`: one shallow checkout of the pinned
/// commit, where its versions are read and its SDKs built. A new pin moves it
/// in place and keeps `target/`, so the next SDK build is incremental; it
/// lives in the project so CI caches it with `.lingxia/`.
pub fn source_dir(project_root: &Path) -> PathBuf {
    project_root.join(".lingxia").join("lingxia-src")
}

const SOURCE_READY: &str = ".lingxia-source-ready";
/// Held by whoever moves or builds the checkout.
pub(crate) const SOURCE_LOCK: &str = ".lingxia-build.lock";

/// Exclusive hold on a checkout. Opened without truncating: Windows refuses
/// to truncate a file another process holds locked, instead of waiting.
pub(crate) fn lock_source(checkout: &Path) -> Result<fs::File> {
    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(checkout.join(SOURCE_LOCK))
        .with_context(|| format!("lock {}", checkout.display()))?;
    file.lock()
        .with_context(|| format!("lock {}", checkout.display()))?;
    Ok(file)
}

pub(crate) fn source_at(dest: &Path, commit: &str) -> bool {
    fs::read_to_string(dest.join(SOURCE_READY)).is_ok_and(|ready| ready.trim() == commit)
}

/// `reference` (the lock's tag or branch) is the fallback for servers that
/// will not serve an unadvertised commit by sha.
pub fn ensure_source(
    project_root: &Path,
    git_url: &str,
    commit: &str,
    reference: Option<&str>,
) -> Result<PathBuf> {
    let dest = source_dir(project_root);
    if source_at(&dest, commit) {
        return Ok(dest);
    }
    fs::create_dir_all(&dest).with_context(|| format!("create {}", dest.display()))?;
    let _lock = lock_source(&dest)?;
    if source_at(&dest, commit) {
        return Ok(dest);
    }
    let _ = fs::remove_file(dest.join(SOURCE_READY));
    let dir = dest.to_string_lossy().into_owned();
    let run = |args: &[&str], what: &str| -> Result<()> {
        let mut command = git(&["-C", &dir]);
        command.args(args);
        git_output(command, what).map(|_| ())
    };
    if !dest.join(".git").exists() {
        run(&["init", "-q"], "git init")?;
    }
    // A re-pin may name another fork.
    let _ = run(&["remote", "remove", "origin"], "git remote remove");
    run(&["remote", "add", "origin", git_url], "git remote add")?;
    let fetch = |what: &str| {
        run(
            &[
                "fetch",
                "-q",
                "--depth",
                "1",
                "--filter=blob:none",
                "origin",
                what,
            ],
            &format!("fetch {what} from {git_url}"),
        )
    };
    if let Err(by_sha) = fetch(commit) {
        let reference = reference.filter(|r| !is_full_sha(r));
        let Some(reference) = reference else {
            return Err(by_sha).with_context(|| format!("is {commit} pushed to {git_url}?"));
        };
        fetch(reference).with_context(|| format!("is {commit} pushed to {git_url}?"))?;
        let mut head = git(&["-C", &dir]);
        head.args(["rev-parse", "FETCH_HEAD"]);
        let fetched = git_output(head, "git rev-parse")?;
        if fetched.trim() != commit {
            bail!(
                "{git_url} serves {reference} at {}, not the locked {commit}; run `lingxia pin` to move the lock",
                fetched.trim()
            );
        }
    }
    run(
        &["checkout", "-q", "--force", "--detach", "FETCH_HEAD"],
        "git checkout",
    )?;
    // Untracked files of the last commit go; ignored ones stay, as in any
    // checkout switched with git: build scripts write generated sources
    // there (`lingxia-sdk/apple/Sources/generated`) and rerun only when
    // their inputs change, so deleting them under a kept `target/` loses
    // them. `node_modules` goes: the lockfile may differ.
    run(
        &["clean", "-ffdq", "-e", &format!("/{SOURCE_LOCK}")],
        "git clean",
    )?;
    // The release script skips regenerating Swift bridges while this file
    // is newer than the core crate, which a switch does not guarantee (it
    // never looks at lingxia-platform). Without it, cargo decides.
    let _ = fs::remove_file(dest.join("lingxia-sdk/apple/Sources/generated/SwiftBridgeCore.h"));
    let modules = dest.join("packages").join("node_modules");
    if modules.exists() {
        fs::remove_dir_all(&modules).with_context(|| format!("remove {}", modules.display()))?;
    }
    fs::write(dest.join(SOURCE_READY), commit)?;
    Ok(dest)
}

/// What `lingxia upgrade` makes of a pin, against this CLI's release.
pub enum AgainstRelease {
    /// The release already carries the pinned commit: the pin has done its job.
    Superseded {
        release: String,
    },
    Kept {
        reason: String,
    },
}

pub fn against_release(pinned: &Pin) -> Result<AgainstRelease> {
    let release = default_release_ref();
    if !same_git(&pinned.git, OFFICIAL_GIT) {
        return Ok(AgainstRelease::Kept {
            reason: format!("it comes from {}, not the official repository", pinned.git),
        });
    }
    if pinned.is_official_release() {
        let cli = semver::Version::parse(env!("LINGXIA_RUST_CRATE_VERSION"))?;
        let pinned_version = semver::Version::parse(&pinned.workspace_version)?;
        return Ok(if pinned_version <= cli {
            AgainstRelease::Superseded { release }
        } else {
            AgainstRelease::Kept {
                reason: format!("{} is newer than {release}", pinned.reference),
            }
        });
    }
    Ok(if commit_in_release(&pinned.commit, &release)? {
        AgainstRelease::Superseded { release }
    } else {
        AgainstRelease::Kept {
            reason: format!("not in {release} yet"),
        }
    })
}

/// Whether the release tag's history holds `commit`. Fetches commits only
/// (no trees or files) into a scratch repository.
fn commit_in_release(commit: &str, release: &str) -> Result<bool> {
    let scratch = tempfile::tempdir()?;
    let dir = scratch.path().to_string_lossy().into_owned();
    git_output(git(&["init", "-q", "--bare", &dir]), "git init")?;
    let tag = format!("refs/tags/{release}");
    git_output(
        git(&[
            "-C",
            &dir,
            "fetch",
            "-q",
            "--filter=tree:0",
            OFFICIAL_GIT,
            &format!("{tag}:{tag}"),
        ]),
        &format!("fetch {release}"),
    )?;
    let status = git(&["-C", &dir, "merge-base", "--is-ancestor", commit, &tag])
        .status()
        .context("run git merge-base")?;
    Ok(status.success())
}

/// Undo a pin: the lock, the manifest's managed lines, the checkout and its
/// SDKs, a local source, and the pinned skill. The project then follows the
/// CLI's release line like one never pinned.
pub fn clear(project_root: &Path) -> Result<()> {
    let lock = project_root.join(LOCK_FILE);
    if lock.exists() {
        fs::remove_file(&lock).with_context(|| format!("remove {}", lock.display()))?;
    }
    let manifest = project_root.join("Cargo.toml");
    if let Ok(content) = fs::read_to_string(&manifest) {
        let (rewritten, _) = cargo::rewrite(&content, &[]);
        if rewritten != content {
            fs::write(&manifest, rewritten)
                .with_context(|| format!("write {}", manifest.display()))?;
        }
    }
    overrides::clear(project_root)?;
    for dir in [
        source_dir(project_root),
        project_root.join(".lingxia").join("source-sdk"),
        project_root.join(".agents").join("skills").join("lingxia"),
    ] {
        if dir.exists() {
            fs::remove_dir_all(&dir).with_context(|| format!("remove {}", dir.display()))?;
        }
    }
    // The Claude link into the skill above.
    let _ = fs::remove_file(project_root.join(".claude").join("skills").join("lingxia"));
    Ok(())
}

/// Put the lock's commit at `.lingxia/lingxia-src`, where the manifest's
/// managed path patches point, unless a local source stands in for it.
pub fn prepare_source(project_root: &Path) -> Result<()> {
    if overrides::source(project_root)?.is_some() {
        return Ok(());
    }
    let Some(lock) = read(project_root)? else {
        return Ok(());
    };
    let pinned = lock.lingxia;
    if pinned.is_official_release() {
        return Ok(());
    }
    ensure_source(
        project_root,
        &pinned.git,
        &pinned.commit,
        Some(&pinned.reference),
    )
    .map(|_| ())
}

/// The pin covers native code only: crates and platform SDKs. Page JS
/// (`@lingxia/bridge`, `@lingxia/polyfills`) is embedded in the CLI and npm
/// packages come from the registry, both on the release line. A pinned
/// commit whose page JS differs from this CLI's would build an app whose
/// pages run another bridge than its native side expects, so builds stop.
pub fn ensure_js_matches_cli(project_root: &Path) -> Result<()> {
    let Some(root) = crate::commands::project_upgrade::find_project_root_from(project_root) else {
        return Ok(());
    };
    let packages = match overrides::source(&root)? {
        Some(source) => source.join("packages"),
        None => {
            let Some(lock) = read(&root)? else {
                return Ok(());
            };
            if lock.lingxia.is_official_release() {
                return Ok(());
            }
            let pinned = lock.lingxia;
            ensure_source(&root, &pinned.git, &pinned.commit, Some(&pinned.reference))?
                .join("packages")
        }
    };
    let pinned = crate::embedded_js_digest::digest(&packages)
        .with_context(|| format!("read the page JS in {}", packages.display()))?;
    if pinned != env!("LINGXIA_EMBEDDED_JS_DIGEST") {
        bail!(
            "the pinned LingXia source changes the page bridge or polyfills (packages/lingxia-bridge, packages/lingxia-polyfills), which this CLI embeds from its own release.\n  A pin covers native code (crates, SDKs); page JS changes ship in a release. Use the CLI release that carries them, or pin a commit whose bridge matches this CLI."
        );
    }
    Ok(())
}

/// Keep a pinned project's skill on its pinned commit. Runs on every
/// command, so it only uses a checkout already on disk; `lingxia pin`
/// fetches one.
pub fn sync_project_skill_if_pinned() {
    let Some(root) = crate::commands::project_upgrade::find_project_root() else {
        return;
    };
    let Ok(Some(lock)) = read(&root) else {
        return;
    };
    let checkout = source_dir(&root);
    if !source_at(&checkout, &lock.lingxia.commit) {
        return;
    }
    match crate::commands::skill::sync_project_skill(
        &root,
        &checkout.join("docs").join("skill"),
        false,
    ) {
        Ok(true) => eprintln!(
            "Updated the project's LingXia skill to {} (lingxia.lock).",
            lock.lingxia.short_commit()
        ),
        Ok(false) => {}
        Err(err) => eprintln!("warning: could not sync the project's LingXia skill: {err}"),
    }
}

/// `[workspace.package] version` (or `[package] version`) of a manifest.
pub fn manifest_version(manifest: &Path) -> Result<String> {
    let text =
        fs::read_to_string(manifest).with_context(|| format!("read {}", manifest.display()))?;
    let value: toml::Table =
        toml::from_str(&text).with_context(|| format!("parse {}", manifest.display()))?;
    let version = value
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("version"))
        .or_else(|| value.get("package").and_then(|p| p.get("version")))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| anyhow!("{} has no version", manifest.display()))?;
    Ok(version.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peeled_tags_win_over_annotated_tag_objects() {
        let listing = "\
7f890f8af0000000000000000000000000000000\trefs/tags/lingxia-crates-v0.27.0
80ba623470000000000000000000000000000000\trefs/tags/lingxia-crates-v0.27.0^{}
";
        assert_eq!(
            resolve_from_listing(listing, "lingxia-crates-v0.27.0").as_deref(),
            Some("80ba623470000000000000000000000000000000")
        );
        let branch = "aaaa000000000000000000000000000000000000\trefs/heads/main\n";
        assert_eq!(
            resolve_from_listing(branch, "main").as_deref(),
            Some("aaaa000000000000000000000000000000000000")
        );
        assert_eq!(resolve_from_listing(branch, "other"), None);
    }

    #[test]
    fn official_release_needs_the_official_repo_and_a_release_tag() {
        let pin = |git: &str, reference: &str| Pin {
            git: git.to_string(),
            reference: reference.to_string(),
            commit: "a".repeat(40),
            workspace_version: "0.27.0".to_string(),
            cli_version: "0.27.0".to_string(),
            pinned_at: String::new(),
        };
        assert!(
            pin(
                "https://github.com/LingXia-Dev/LingXia",
                "lingxia-crates-v0.27.0"
            )
            .is_official_release()
        );
        assert!(
            !pin(
                "https://github.com/someone/LingXia.git",
                "lingxia-crates-v0.27.0"
            )
            .is_official_release()
        );
        assert!(!pin(OFFICIAL_GIT, "main").is_official_release());
        assert!(!pin(OFFICIAL_GIT, "lingxia-crates-v0.27").is_official_release());
    }

    #[test]
    fn lock_round_trips_and_rejects_short_commits() {
        let dir = tempfile::tempdir().unwrap();
        let pin = Pin {
            git: OFFICIAL_GIT.to_string(),
            reference: "main".to_string(),
            commit: "b".repeat(40),
            workspace_version: "0.27.0".to_string(),
            cli_version: "0.27.1".to_string(),
            pinned_at: "2026-10-22T00:00:00Z".to_string(),
        };
        write(dir.path(), pin.clone()).unwrap();
        assert_eq!(read(dir.path()).unwrap().unwrap().lingxia, pin);
        let text = fs::read_to_string(lock_path(dir.path())).unwrap();
        assert!(text.contains("\"workspaceVersion\""));
        fs::write(lock_path(dir.path()), text.replace(&"b".repeat(40), "bbbb")).unwrap();
        assert!(read(dir.path()).is_err());
        assert!(read(&dir.path().join("missing")).unwrap().is_none());
    }

    fn pinned(git: &str, reference: &str, version: &str) -> Pin {
        Pin {
            git: git.to_string(),
            reference: reference.to_string(),
            commit: "c".repeat(40),
            workspace_version: version.to_string(),
            cli_version: "0.27.0".to_string(),
            pinned_at: String::new(),
        }
    }

    #[test]
    fn a_fork_or_a_newer_release_keeps_its_pin() {
        let fork = pinned("https://github.com/someone/LingXia.git", "main", "0.27.0");
        assert!(matches!(
            against_release(&fork).unwrap(),
            AgainstRelease::Kept { .. }
        ));
        let older = pinned(OFFICIAL_GIT, "lingxia-crates-v0.0.1", "0.0.1");
        assert!(matches!(
            against_release(&older).unwrap(),
            AgainstRelease::Superseded { .. }
        ));
        let newer = pinned(OFFICIAL_GIT, "lingxia-crates-v99.0.0", "99.0.0");
        assert!(matches!(
            against_release(&newer).unwrap(),
            AgainstRelease::Kept { .. }
        ));
    }

    #[test]
    fn clear_leaves_a_project_that_never_pinned() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, pinned(OFFICIAL_GIT, "main", "0.27.0")).unwrap();
        let original =
            "[workspace]\nmembers = [\"native\"]\n\n[patch.crates-io]\nmine = { path = \"x\" }\n";
        let (pinned_manifest, _) = cargo::rewrite(
            original,
            &[cargo::path_patch(
                "lingxia",
                cargo::SOURCE_PATH,
                "crates/lingxia",
            )],
        );
        fs::write(root.join("Cargo.toml"), pinned_manifest).unwrap();
        for dir in [
            ".lingxia/lingxia-src/target",
            ".lingxia/source-sdk/apple/x",
            ".agents/skills/lingxia",
        ] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        fs::write(root.join(".lingxia/source.local"), "/src").unwrap();
        fs::write(root.join(".lingxia/template.json"), "{}").unwrap();

        clear(root).unwrap();

        assert!(read(root).unwrap().is_none());
        assert_eq!(
            fs::read_to_string(root.join("Cargo.toml")).unwrap(),
            original
        );
        for gone in [
            ".lingxia/lingxia-src",
            ".lingxia/source-sdk",
            ".lingxia/source.local",
            ".agents/skills/lingxia",
        ] {
            assert!(!root.join(gone).exists(), "{gone}");
        }
        assert!(root.join(".lingxia/template.json").exists());
    }
}
