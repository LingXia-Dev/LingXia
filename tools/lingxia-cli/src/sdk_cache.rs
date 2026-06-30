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

/// Root of the SDK cache: `~/.lingxia/sdk` (mirrors `update.rs`'s `~/.lingxia/cli`).
pub fn sdk_cache_root() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".lingxia").join("sdk"))
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

    // Unpack into a temp dir then atomically rename into place (temp-then-rename
    // like update.rs install).
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
                extract_zip_strip_top(&asset_bytes, &tmp, "maven")?;
            }
            SdkPlatform::Apple => {
                extract_zip_strip_top(&asset_bytes, &tmp, "lingxia-apple-sdk")?;
            }
            SdkPlatform::Harmony => {
                // HAR is consumed as-is by ohpm; do not unpack.
                fs::write(tmp.join("lingxia.har"), &asset_bytes)
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
        fs::remove_dir_all(&dest)
            .with_context(|| format!("Failed to remove stale cache dir {}", dest.display()))?;
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create cache parent {}", parent.display()))?;
    }
    fs::rename(&tmp, &dest).with_context(|| {
        format!(
            "Failed to publish SDK cache\n  From: {}\n  To: {}",
            tmp.display(),
            dest.display()
        )
    })?;

    // Touch the .ready sentinel.
    fs::write(dest.join(READY_SENTINEL), version.as_bytes())
        .with_context(|| format!("Failed to write {READY_SENTINEL} in {}", dest.display()))?;

    resolve_cached_path(platform, &dest)
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
}
