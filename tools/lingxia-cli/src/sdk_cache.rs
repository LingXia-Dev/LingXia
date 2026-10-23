//! SDK acquisition + caching for external (non-workspace) projects.
//!
//! When `lingxia build` runs inside a user's project (outside the LingXia
//! monorepo), the native SDK is not present on disk. This module downloads the
//! per-platform SDK artifact published by `scripts/release/sdk.sh` to the
//! GitHub release `lingxia-sdk-v<version>`, verifies its SHA-256 against the
//! release's SHASUMS file, and unpacks it into a content-addressed cache under
//! `~/.lingxia/sdk/`. A `.ready` sentinel marks a complete install so repeat
//! builds short-circuit with no network access (mirrors `update.rs`).

use crate::github;
use anyhow::{Context, Result, anyhow, bail};
use sha2::Digest;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Sentinel file written inside a fully-populated cache dir.
const READY_SENTINEL: &str = ".ready";

/// Native SDK platforms distributed as GitHub release assets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkPlatform {
    Android,
    Apple,
    Harmony,
}

impl SdkPlatform {
    /// Exact release asset name produced by `scripts/release/sdk.sh`.
    pub fn asset_name(&self, version: &str) -> String {
        match self {
            SdkPlatform::Android => format!("lingxia-sdk-android-maven-{version}.zip"),
            SdkPlatform::Apple => format!("lingxia-sdk-apple-source-{version}.zip"),
            SdkPlatform::Harmony => format!("lingxia-sdk-harmony-{version}.har"),
        }
    }

    /// Per-platform cache subdirectory name under `~/.lingxia/sdk/`.
    fn cache_subdir(&self) -> &'static str {
        match self {
            SdkPlatform::Android => "android-maven",
            SdkPlatform::Apple => "apple",
            SdkPlatform::Harmony => "harmony",
        }
    }

    /// `sdk.sh` platforms_slug for the single-platform SHASUMS filename
    /// (`SHASUMS256-<version>-<slug>.txt`). The full release uses
    /// `apple-android-harmony`; we try that first and fall back to these.
    fn shasums_slug(&self) -> &'static str {
        match self {
            SdkPlatform::Android => "android",
            SdkPlatform::Apple => "apple",
            SdkPlatform::Harmony => "harmony",
        }
    }
}

/// Root of the SDK cache: `<state root>/sdk` (mirrors `update.rs`'s `cli/`).
pub fn sdk_cache_root() -> Option<PathBuf> {
    Some(crate::state_root::lingxia_dir().ok()?.join("sdk"))
}

/// GitHub release tag for an SDK version (matches `scripts/release/sdk.sh` `GH_TAG`).
fn release_tag(version: &str) -> String {
    format!("lingxia-sdk-v{version}")
}

/// The SDK version to fetch. This is intentionally independent of the CLI
/// binary version so CLI-only patch releases can reuse an existing SDK release.
pub fn sdk_version() -> String {
    crate::versions::current_versions().sdk
}

/// Candidate SHASUMS asset names to try, most-specific full-release name first.
fn shasums_candidates(platform: SdkPlatform, version: &str) -> Vec<String> {
    vec![
        format!("SHASUMS256-{version}-apple-android-harmony.txt"),
        format!("SHASUMS256-{version}-{}.txt", platform.shasums_slug()),
    ]
}

/// Compute the lowercase hex SHA-256 of `bytes`. Mirrors `assets/hash.rs`.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// Parse a SHASUMS file and return the recorded hash for `basename`.
///
/// Format (one per line, GNU coreutils style): `<hex>␠␠<basename>`.
pub(crate) fn shasum_for(shasums: &str, basename: &str) -> Option<String> {
    for line in shasums.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        // The remainder is the filename (may be prefixed with '*' for binary mode).
        let name = parts.next()?.trim_start_matches('*');
        if name == basename {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

/// Directory the given SDK version would occupy (`~/.lingxia/sdk/<plat>/<ver>`).
/// Does not require the cache to be populated.
pub fn cached_sdk_dir(platform: SdkPlatform, version: &str) -> Option<PathBuf> {
    Some(
        sdk_cache_root()?
            .join(platform.cache_subdir())
            .join(version),
    )
}

/// Whether `ensure_sdk` would be a cache hit for this platform/version.
pub fn sdk_is_cached(platform: SdkPlatform, version: &str) -> bool {
    cached_sdk_dir(platform, version)
        .map(|dir| dir.join(READY_SENTINEL).exists())
        .unwrap_or(false)
}

/// Ensure the SDK for `platform`/`version` is present in the cache and return
/// the resolved path:
/// - Android: the maven repo dir (contains `io/github/...`).
/// - Apple: the unpacked package dir (contains `Package.swift` + `Sources/`).
/// - Harmony: the `lingxia.har` file.
///
/// On a cache hit (sentinel present) returns immediately without any network.
pub fn ensure_sdk(platform: SdkPlatform, version: &str) -> Result<PathBuf> {
    let root = sdk_cache_root()
        .ok_or_else(|| anyhow!("Could not resolve home directory for SDK cache"))?;
    let dest = root.join(platform.cache_subdir()).join(version);
    let sentinel = dest.join(READY_SENTINEL);

    if sentinel.exists() {
        return resolve_cached_path(platform, &dest);
    }

    let asset_name = platform.asset_name(version);
    let tag = release_tag(version);
    let repo = github::release_repo();

    // Download the platform artifact.
    let asset_bytes = github::download_release_asset_from_repo(&repo, &tag, &asset_name)
        .with_context(|| {
            format!("Failed to download SDK asset '{asset_name}' from {repo} ({tag})")
        })?;

    // Download the SHASUMS file (try full-release name, then single-platform).
    let candidates = shasums_candidates(platform, version);
    let mut shasums_text: Option<String> = None;
    let mut last_err: Option<anyhow::Error> = None;
    for candidate in &candidates {
        match github::download_release_asset_from_repo(&repo, &tag, candidate) {
            Ok(bytes) => {
                shasums_text = Some(String::from_utf8_lossy(&bytes).into_owned());
                break;
            }
            Err(err) => last_err = Some(err),
        }
    }
    let shasums_text = shasums_text.ok_or_else(|| {
        anyhow!(
            "Could not download a SHASUMS file for SDK release {tag}\n  Tried: {}\n  Last error: {}",
            candidates.join(", "),
            last_err
                .map(|e| e.to_string())
                .unwrap_or_else(|| "(none)".to_string())
        )
    })?;

    // Verify SHA-256. HARD-FAIL on mismatch or missing entry.
    let expected = shasum_for(&shasums_text, &asset_name)
        .ok_or_else(|| anyhow!("SHASUMS file for {tag} has no entry for '{asset_name}'"))?;
    let actual = sha256_hex(&asset_bytes);
    if actual != expected {
        bail!(
            "SHA-256 verification failed for SDK asset '{asset_name}'\n  Expected: {expected}\n  Actual:   {actual}"
        );
    }

    install_asset(platform, &dest, version, &asset_bytes)
}

/// Unpack an SDK artifact into `dest` (temp-then-rename, like update.rs) and
/// mark it ready with `label`.
fn install_asset(
    platform: SdkPlatform,
    dest: &Path,
    label: &str,
    asset_bytes: &[u8],
) -> Result<PathBuf> {
    let tmp = dest.with_extension("tmp");
    if tmp.exists() {
        fs::remove_dir_all(&tmp)
            .with_context(|| format!("Failed to clean stale temp dir {}", tmp.display()))?;
    }
    fs::create_dir_all(&tmp)
        .with_context(|| format!("Failed to create temp dir {}", tmp.display()))?;

    let unpack_result = (|| -> Result<()> {
        match platform {
            SdkPlatform::Android => {
                extract_zip_strip_top(asset_bytes, &tmp, "maven")?;
            }
            SdkPlatform::Apple => {
                extract_zip_strip_top(asset_bytes, &tmp, "lingxia-apple-sdk")?;
            }
            SdkPlatform::Harmony => {
                // HAR is consumed as-is by ohpm; do not unpack.
                fs::write(tmp.join("lingxia.har"), asset_bytes)
                    .with_context(|| format!("Failed to write HAR into {}", tmp.display()))?;
            }
        }
        Ok(())
    })();

    if let Err(err) = unpack_result {
        let _ = fs::remove_dir_all(&tmp);
        return Err(err);
    }

    // Atomic publish: rename temp into the final dest.
    if dest.exists() {
        fs::remove_dir_all(dest)
            .with_context(|| format!("Failed to remove stale cache dir {}", dest.display()))?;
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create cache parent {}", parent.display()))?;
    }
    fs::rename(&tmp, dest).with_context(|| {
        format!(
            "Failed to publish SDK cache\n  From: {}\n  To: {}",
            tmp.display(),
            dest.display()
        )
    })?;

    // Touch the .ready sentinel.
    fs::write(dest.join(READY_SENTINEL), label.as_bytes())
        .with_context(|| format!("Failed to write {READY_SENTINEL} in {}", dest.display()))?;

    resolve_cached_path(platform, dest)
}

/// The SDK for `project_root`: its `lingxia.lock` when it has one (an
/// official release from the GitHub release, any other commit built from
/// that commit's source), otherwise this CLI's release line.
pub fn ensure_project_sdk(project_root: &Path, platform: SdkPlatform) -> Result<PathBuf> {
    if let Some(source) = crate::pin::overrides::source(project_root)? {
        return ensure_override_sdk(project_root, &source, platform);
    }
    let Some(lock) = crate::pin::read(project_root)? else {
        return ensure_sdk(platform, &sdk_version());
    };
    let pinned = lock.lingxia;
    if pinned.is_official_release() {
        ensure_sdk(platform, &pinned.workspace_version)
    } else {
        ensure_pinned_sdk(project_root, &pinned, platform)
    }
}

/// `<project>/.lingxia/source-sdk/<platform>/<key>`: SDKs built from source
/// (a pinned commit, or a local tree), beside the checkout CI caches with it.
fn source_sdk_dir(project_root: &Path, platform: SdkPlatform, key: &str) -> PathBuf {
    project_root
        .join(".lingxia")
        .join("source-sdk")
        .join(platform.cache_subdir())
        .join(key)
}

/// Where the project's SDK for `platform` stands, without fetching it.
pub fn project_sdk_status(project_root: &Path, platform: SdkPlatform) -> String {
    let ready = |dir: Option<PathBuf>| dir.is_some_and(|d| d.join(READY_SENTINEL).exists());
    if let Ok(Some(source)) = crate::pin::overrides::source(project_root) {
        let key = crate::pin::overrides::tree_key(&source);
        let dir = source_sdk_dir(project_root, platform, &key);
        return if ready(Some(dir)) {
            format!("built from the working tree ({key})")
        } else {
            format!("next build builds it from the working tree ({key})")
        };
    }
    match crate::pin::read(project_root) {
        Ok(Some(lock)) if !lock.lingxia.is_official_release() => {
            let pinned = lock.lingxia;
            let dir = source_sdk_dir(project_root, platform, &pinned.commit);
            if ready(Some(dir)) {
                format!("cached for {}", pinned.short_commit())
            } else {
                format!(
                    "next build builds it from {} (needs {})",
                    pinned.short_commit(),
                    source_build_tools(platform).join(", ")
                )
            }
        }
        Ok(lock) => {
            let version = lock
                .map(|l| l.lingxia.workspace_version)
                .unwrap_or_else(sdk_version);
            if sdk_is_cached(platform, &version) {
                format!("{version} cached")
            } else {
                format!("next build downloads {version}")
            }
        }
        Err(err) => format!("unknown: {err}"),
    }
}

/// The version the project's SDK artifacts carry (Android's maven
/// coordinate): the pinned line, else this CLI's.
pub fn project_sdk_version(project_root: &Path) -> String {
    if let Ok(Some(source)) = crate::pin::overrides::source(project_root)
        && let Ok(version) = crate::pin::manifest_version(&source.join("Cargo.toml"))
    {
        return version;
    }
    match crate::pin::read(project_root) {
        Ok(Some(lock)) => lock.lingxia.workspace_version,
        _ => sdk_version(),
    }
}

/// Keyed by commit, not version: a fork's 0.27.0 is not the official 0.27.0.
fn ensure_pinned_sdk(
    project_root: &Path,
    pinned: &crate::pin::Pin,
    platform: SdkPlatform,
) -> Result<PathBuf> {
    let dest = source_sdk_dir(project_root, platform, &pinned.commit);
    if dest.join(READY_SENTINEL).exists() {
        return resolve_cached_path(platform, &dest);
    }
    let asset_name = platform.asset_name(&pinned.workspace_version);
    let bytes = build_from_source(project_root, pinned, platform, &asset_name)?;
    install_asset(platform, &dest, &pinned.commit, &bytes)
}

/// An SDK built from the local LingXia working tree, keyed by the tree's
/// HEAD and uncommitted changes.
fn ensure_override_sdk(
    project_root: &Path,
    source: &Path,
    platform: SdkPlatform,
) -> Result<PathBuf> {
    let key = crate::pin::overrides::tree_key(source);
    let dest = source_sdk_dir(project_root, platform, &key);
    if dest.join(READY_SENTINEL).exists() {
        return resolve_cached_path(platform, &dest);
    }
    require_build_tools(platform, &key)?;
    let version = crate::pin::manifest_version(&source.join("Cargo.toml"))?;
    let bytes = build_in_checkout(source, &key, None, platform, &platform.asset_name(&version))?;
    install_asset(platform, &dest, &key, &bytes)
}

/// Build the platform's SDK artifact from the pinned commit's checkout with
/// the release script. The tools that platform needs must be installed.
fn build_from_source(
    project_root: &Path,
    pinned: &crate::pin::Pin,
    platform: SdkPlatform,
    asset_name: &str,
) -> Result<Vec<u8>> {
    require_build_tools(platform, pinned.short_commit())?;
    let checkout = crate::pin::ensure_source(
        project_root,
        &pinned.git,
        &pinned.commit,
        Some(&pinned.reference),
    )?;
    build_in_checkout(
        &checkout,
        pinned.short_commit(),
        Some(&pinned.commit),
        platform,
        asset_name,
    )
}

fn require_build_tools(platform: SdkPlatform, label: &str) -> Result<()> {
    let missing: Vec<&str> = source_build_tools(platform)
        .iter()
        .copied()
        .filter(|tool| which::which(tool).is_err())
        .collect();
    if !missing.is_empty() {
        bail!(
            "building the {} SDK from {label} needs {}; install them (building the app for this platform needs them too)",
            platform.shasums_slug(),
            missing.join(", ")
        );
    }
    Ok(())
}

/// The `cargo xtask sdk` interface this CLI calls; see xtask/Cargo.toml.
const XTASK_SDK_INTERFACE: i64 = 1;

/// The pinned commit's `cargo xtask sdk` interface, if it has one.
fn xtask_sdk_interface(checkout: &Path) -> Option<i64> {
    let manifest = fs::read_to_string(checkout.join("xtask/Cargo.toml")).ok()?;
    let table: toml::Table = manifest.parse().ok()?;
    table
        .get("package")?
        .get("metadata")?
        .get("lingxia")?
        .get("sdk-interface")?
        .as_integer()
}

/// Where a source build writes its artifact, inside the checkout.
pub(crate) const SOURCE_BUILD_OUT: &str = "dist-sdk-pin";

/// Run the release script for one platform in a LingXia checkout.
fn build_in_checkout(
    checkout: &Path,
    label: &str,
    commit: Option<&str>,
    platform: SdkPlatform,
    asset_name: &str,
) -> Result<Vec<u8>> {
    // One build at a time per checkout: two platforms or two projects would
    // otherwise share its node_modules and output directories.
    let _lock = crate::pin::lock_source(checkout)?;
    // A re-pin may have moved the tree since it was prepared; what is built
    // here is cached under `commit`.
    if let Some(commit) = commit
        && !crate::pin::source_at(checkout, commit)
    {
        bail!(
            "{} moved off {label} while preparing this build; run it again",
            checkout.display()
        );
    }
    let packages = checkout.join("packages");
    if !packages.join("node_modules").is_dir() {
        run_in(&packages, "npm", &["ci", "--no-audit", "--no-fund"])?;
    }
    let out = checkout.join(SOURCE_BUILD_OUT);
    let _ = fs::remove_dir_all(&out);
    let out_arg = out.to_string_lossy().into_owned();
    println!(
        "Building the {} SDK from {label} (first build of this source)…",
        platform.shasums_slug(),
    );
    let cli = std::env::current_exe().ok();
    let (mut command, what) = match xtask_sdk_interface(checkout) {
        Some(XTASK_SDK_INTERFACE) => {
            let mut command = std::process::Command::new("cargo");
            command.args(["run", "-q", "-p", "xtask", "--", "sdk"]);
            command.args(["--platform", platform.shasums_slug(), "--out", &out_arg]);
            // Resource generation uses this CLI instead of compiling the
            // checkout's (minutes, every new commit).
            if let Some(cli) = &cli {
                command.arg("--cli").arg(cli);
            }
            (command, "cargo xtask sdk".to_string())
        }
        Some(newer) => bail!(
            "{label} builds its SDKs through interface {newer}, newer than this CLI's {XTASK_SDK_INTERFACE}; run `lingxia upgrade`"
        ),
        // Commits from before `cargo xtask sdk` build through the release script.
        None => {
            let script = checkout.join("scripts/release/sdk.sh");
            let mut command = std::process::Command::new("bash");
            command.arg(&script).args([
                "--platform",
                platform.shasums_slug(),
                "--out",
                &out_arg,
                "--no-shasums",
            ]);
            command.envs(cli.as_ref().map(|cli| ("LINGXIA_CLI", cli)));
            (command, script.display().to_string())
        }
    };
    let status = command
        .current_dir(checkout)
        // The build reads its own `target/`; a project's `.cargo/config.toml`
        // above a checkout inside it could move that.
        .env("CARGO_TARGET_DIR", checkout.join("target"))
        .status()
        .with_context(|| format!("run {what}"))?;
    if !status.success() {
        bail!("{what} failed for {}", platform.shasums_slug());
    }
    let built = find_file(&out, asset_name)
        .ok_or_else(|| anyhow!("{what} did not produce {asset_name}"))?;
    // The Swift package names these generated targets; an SDK cached without
    // them fails every app build after this one.
    if platform == SdkPlatform::Apple {
        let generated = checkout.join("lingxia-sdk/apple/Sources/generated");
        for target in ["LingXiaRustAPI", "LingXiaSwiftAPI", "SwiftBridgeCore.h"] {
            if !generated.join(target).exists() {
                bail!(
                    "the Apple SDK built from {label} has no Sources/generated/{target}; not caching it"
                );
            }
        }
    }
    let bytes = fs::read(&built).with_context(|| format!("read {}", built.display()))?;
    let _ = fs::remove_dir_all(&out);
    Ok(bytes)
}

fn source_build_tools(platform: SdkPlatform) -> &'static [&'static str] {
    match platform {
        SdkPlatform::Apple => &["cargo", "node", "npm", "zip"],
        SdkPlatform::Android => &["cargo", "node", "npm", "java"],
        SdkPlatform::Harmony => &["cargo", "node", "npm", "ohpm", "hvigorw"],
    }
}

fn run_in(dir: &Path, program: &str, args: &[&str]) -> Result<()> {
    let status = std::process::Command::new(program)
        .args(args)
        .current_dir(dir)
        .status()
        .with_context(|| format!("run {program} in {}", dir.display()))?;
    if !status.success() {
        bail!("{program} {} failed in {}", args.join(" "), dir.display());
    }
    Ok(())
}

fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, name) {
                return Some(found);
            }
        } else if path.file_name().is_some_and(|n| n == name) {
            return Some(path);
        }
    }
    None
}

/// Map a populated cache dir to the platform-specific resolved path.
fn resolve_cached_path(platform: SdkPlatform, dest: &Path) -> Result<PathBuf> {
    let resolved = match platform {
        SdkPlatform::Harmony => dest.join("lingxia.har"),
        SdkPlatform::Android | SdkPlatform::Apple => dest.to_path_buf(),
    };
    if !resolved.exists() {
        bail!(
            "SDK cache appears incomplete (missing {})",
            resolved.display()
        );
    }
    Ok(resolved)
}

/// Extract a zip archive, stripping a single expected top-level directory so
/// `out_dir` directly contains the tree (e.g. `maven/` -> contents of maven).
/// Follows the `zip::ZipArchive` precedent in `commands/publish.rs`.
fn extract_zip_strip_top(bytes: &[u8], out_dir: &Path, top: &str) -> Result<()> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(reader).context("Failed to read downloaded SDK zip archive")?;

    let top_prefix = format!("{top}/");
    let mut written = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .with_context(|| format!("Failed to read zip entry #{index}"))?;
        let Some(enclosed) = entry.enclosed_name() else {
            // Skip entries with unsafe/absolute paths.
            continue;
        };
        let raw = enclosed.to_string_lossy().replace('\\', "/");

        // Strip the single top-level dir; skip the bare dir entry and siblings.
        let Some(rel) = raw.strip_prefix(&top_prefix) else {
            continue;
        };
        if rel.is_empty() {
            continue;
        }

        let out_path = out_dir.join(rel);
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
        fs::write(&out_path, &data)
            .with_context(|| format!("Failed to write {}", out_path.display()))?;
        written += 1;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = entry.unix_mode() {
                let _ = fs::set_permissions(&out_path, fs::Permissions::from_mode(mode));
            }
        }
    }
    if written == 0 {
        bail!("SDK zip contained no entries under '{top}/' (unexpected archive layout)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_workspace_speaks_the_xtask_interface_this_cli_calls() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        assert_eq!(xtask_sdk_interface(&root), Some(XTASK_SDK_INTERFACE));
        let older = tempfile::tempdir().unwrap();
        assert_eq!(xtask_sdk_interface(older.path()), None);
    }

    #[test]
    fn asset_names_match_sdk_sh_exactly() {
        assert_eq!(
            SdkPlatform::Android.asset_name("0.8.0"),
            "lingxia-sdk-android-maven-0.8.0.zip"
        );
        assert_eq!(
            SdkPlatform::Apple.asset_name("0.8.0"),
            "lingxia-sdk-apple-source-0.8.0.zip"
        );
        assert_eq!(
            SdkPlatform::Harmony.asset_name("1.2.3"),
            "lingxia-sdk-harmony-1.2.3.har"
        );
    }

    #[test]
    fn release_tag_matches_gh_tag() {
        assert_eq!(release_tag("0.8.0"), "lingxia-sdk-v0.8.0");
    }

    #[test]
    fn cache_subdirs_are_stable() {
        assert_eq!(SdkPlatform::Android.cache_subdir(), "android-maven");
        assert_eq!(SdkPlatform::Apple.cache_subdir(), "apple");
        assert_eq!(SdkPlatform::Harmony.cache_subdir(), "harmony");
    }

    #[test]
    fn shasums_candidates_try_full_release_first() {
        let candidates = shasums_candidates(SdkPlatform::Android, "0.8.0");
        assert_eq!(candidates[0], "SHASUMS256-0.8.0-apple-android-harmony.txt");
        assert_eq!(candidates[1], "SHASUMS256-0.8.0-android.txt");
    }

    #[test]
    fn shasum_line_parses_and_matches() {
        let asset = "lingxia-sdk-android-maven-0.8.0.zip";
        let bytes = b"hello lingxia";
        let hash = sha256_hex(bytes);
        let shasums = format!("{hash}  {asset}\ndeadbeef  some-other-file.txt\n");
        let recorded = shasum_for(&shasums, asset).expect("entry present");
        assert_eq!(recorded, hash);
    }

    #[test]
    fn tampered_byte_fails_verification() {
        let asset = "lingxia-sdk-apple-source-0.8.0.zip";
        let original = b"the real artifact bytes";
        let hash = sha256_hex(original);
        let shasums = format!("{hash}  {asset}\n");

        // Flip one byte: the recomputed hash must no longer match.
        let mut tampered = original.to_vec();
        tampered[0] ^= 0xFF;
        let recorded = shasum_for(&shasums, asset).expect("entry present");
        assert_ne!(sha256_hex(&tampered), recorded);
    }

    #[test]
    fn shasum_missing_entry_returns_none() {
        let shasums = "abc123  unrelated.zip\n";
        assert!(shasum_for(shasums, "lingxia-sdk-harmony-0.8.0.har").is_none());
    }

    #[test]
    fn missing_cache_is_not_ready() {
        assert!(!sdk_is_cached(SdkPlatform::Android, "0.0.0-never-cached"));
    }
}
