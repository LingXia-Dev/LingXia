//! Shared types and helpers for `lingxia store` backends.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

/// The store target selected by `--platform`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorePlatform {
    Windows,
    Ios,
    Macos,
    Harmony,
}

impl StorePlatform {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "windows" => Self::Windows,
            "ios" => Self::Ios,
            "macos" => Self::Macos,
            "harmony" => Self::Harmony,
            other => bail!(
                "unsupported `--platform {other}` for store (expected: windows, ios, macos, harmony)"
            ),
        })
    }

    pub fn store_name(self) -> &'static str {
        match self {
            Self::Windows => "Microsoft Store",
            Self::Ios | Self::Macos => "App Store",
            Self::Harmony => "AppGallery",
        }
    }

    /// `dist/<subdir>/` where `build` writes this platform's artifact.
    pub fn dist_subdir(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Ios => "ios",
            Self::Macos => "macos",
            Self::Harmony => "harmony",
        }
    }

    /// Artifact extensions to look for, in priority order.
    pub fn artifact_exts(self) -> &'static [&'static str] {
        match self {
            Self::Windows => &["msixupload", "msix"],
            Self::Ios | Self::Macos => &["ipa", "pkg"],
            Self::Harmony => &["app", "hap"],
        }
    }
}

/// Per-run intent from CLI flags (never persisted).
#[derive(Clone, Debug, Default)]
pub struct SubmitOptions {
    /// Create the submission but do not commit it for review.
    pub draft: bool,
    pub release_notes: Option<String>,
    /// Reserved `--track` flag (per-store release track). Accepted by the CLI;
    /// no backend consumes it yet.
    #[allow(dead_code)]
    pub track: Option<String>,
}

/// Find the built artifact in `dist/<platform>/`. `submit` never builds — fail
/// clearly (pointing at `lingxia build`) when the artifact is missing.
pub fn find_artifact(project_root: &Path, platform: StorePlatform) -> Result<PathBuf> {
    let dir = project_root.join("dist").join(platform.dist_subdir());
    if !dir.is_dir() {
        bail!(
            "No build output at {} — run `lingxia build --platform {}` first.",
            dir.display(),
            platform.dist_subdir()
        );
    }
    for ext in platform.artifact_exts() {
        if let Some(found) = newest_with_ext(&dir, ext)? {
            return Ok(found);
        }
    }
    bail!(
        "No {} artifact ({}) in {} — run `lingxia build --platform {}` first.",
        platform.store_name(),
        platform.artifact_exts().join(" / ."),
        dir.display(),
        platform.dist_subdir()
    )
}

fn newest_with_ext(dir: &Path, ext: &str) -> Result<Option<PathBuf>> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).with_context(|| format!("read dir {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let matches = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case(ext))
            .unwrap_or(false);
        if matches {
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            if best.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                best = Some((mtime, path));
            }
        }
    }
    Ok(best.map(|(_, p)| p))
}

/// A shared ureq agent for store API calls.
pub fn http() -> ureq::Agent {
    crate::http_client::create_agent(180)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_platforms() {
        assert_eq!(
            StorePlatform::parse("Windows").unwrap(),
            StorePlatform::Windows
        );
        assert_eq!(StorePlatform::parse("ios").unwrap(), StorePlatform::Ios);
        assert!(StorePlatform::parse("android").is_err());
    }

    #[test]
    fn find_artifact_missing_dir_errors() {
        let tmp = std::env::temp_dir().join(format!("lx-store-art-{}", std::process::id()));
        let err = find_artifact(&tmp, StorePlatform::Windows).unwrap_err();
        assert!(err.to_string().contains("lingxia build"));
    }

    #[test]
    fn find_artifact_picks_by_extension() {
        let root = std::env::temp_dir().join(format!("lx-store-art2-{}", std::process::id()));
        let dir = root.join("dist").join("windows");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("App.msix"), b"x").unwrap();
        let found = find_artifact(&root, StorePlatform::Windows).unwrap();
        assert_eq!(found.extension().unwrap(), "msix");
        let _ = std::fs::remove_dir_all(&root);
    }
}
