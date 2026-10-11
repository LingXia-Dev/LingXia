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
    GooglePlay,
    Xiaomi,
    Oppo,
    Honor,
}

impl StorePlatform {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "windows" => Self::Windows,
            "ios" => Self::Ios,
            "macos" => Self::Macos,
            "harmony" => Self::Harmony,
            "googleplay" | "google-play" | "play" => Self::GooglePlay,
            "xiaomi" => Self::Xiaomi,
            "oppo" => Self::Oppo,
            "honor" => Self::Honor,
            other => bail!(
                "unsupported `--platform {other}` for store (expected: windows, ios, macos, \
                 harmony, googleplay, xiaomi, oppo, honor)"
            ),
        })
    }

    pub fn store_name(self) -> &'static str {
        match self {
            Self::Windows => "Microsoft Store",
            Self::Ios | Self::Macos => "App Store",
            Self::Harmony => "AppGallery",
            Self::GooglePlay => "Google Play",
            Self::Xiaomi => "Xiaomi GetApps/小米应用商店",
            Self::Oppo => "OPPO 软件商店",
            Self::Honor => "Honor AppGallery/荣耀应用市场",
        }
    }

    /// `dist/<subdir>/` where `build` writes this platform's artifact.
    pub fn dist_subdir(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Ios => "ios",
            Self::Macos => "macos",
            Self::Harmony => "harmony",
            // All Android stores consume the same `dist/android/` output.
            Self::GooglePlay | Self::Xiaomi | Self::Oppo | Self::Honor => "android",
        }
    }

    /// `dist` manifest artifact formats this store takes, in priority order.
    pub fn artifact_formats(self) -> &'static [&'static str] {
        match self {
            Self::Windows => &["msix"],
            Self::Ios => &["ipa"],
            Self::Macos => &["pkg"],
            Self::Harmony => &["app", "hap"],
            // Google Play prefers App Bundles; the Chinese stores take APKs.
            Self::GooglePlay => &["aab", "apk"],
            Self::Xiaomi | Self::Oppo | Self::Honor => &["apk"],
        }
    }
}

/// Per-run intent from CLI flags (never persisted).
///
/// `lingxia store submit` only uploads. Review / rollout is started in the
/// store console, not by this CLI.
#[derive(Clone, Debug, Default)]
pub struct SubmitOptions {
    pub release_notes: Option<String>,
    /// Per-store release track/channel (e.g. Google Play `internal`/`production`).
    pub track: Option<String>,
    pub test_version_id: Option<String>,
}

/// The artifact `lingxia package` recorded for this store in
/// `dist/<platform>/manifest.json`. `submit` never builds, and never guesses
/// by file name or age.
pub fn find_artifact(project_root: &Path, platform: StorePlatform) -> Result<PathBuf> {
    let dir = project_root.join("dist").join(platform.dist_subdir());
    crate::dist_manifest::resolve(&dir, platform.artifact_formats())
        .map(|(_, path)| path)
        .with_context(|| format!("No {} artifact to submit", platform.store_name()))
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
    fn find_artifact_without_a_package_points_at_lingxia_package() {
        let tmp = tempfile::tempdir().unwrap();
        let err = format!(
            "{:#}",
            find_artifact(tmp.path(), StorePlatform::Windows).unwrap_err()
        );
        assert!(err.contains("lingxia package"), "{err}");
    }

    #[test]
    fn find_artifact_takes_the_store_format_from_the_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("dist").join("android");
        std::fs::create_dir_all(&dir).unwrap();
        let file = |name: &str, format| {
            let path = dir.join(name);
            std::fs::write(&path, name).unwrap();
            crate::dist_manifest::Produced { format, path }
        };
        let files = [file("Demo.apk", "apk"), file("Demo.aab", "aab")];
        crate::dist_manifest::write(&dir, "android", "1.0.0", "prod", None, &files).unwrap();
        // A newer stray file is not what was packaged.
        std::fs::write(dir.join("Stray.aab"), b"x").unwrap();
        assert_eq!(
            find_artifact(tmp.path(), StorePlatform::GooglePlay).unwrap(),
            dir.join("Demo.aab")
        );
        assert_eq!(
            find_artifact(tmp.path(), StorePlatform::Xiaomi).unwrap(),
            dir.join("Demo.apk")
        );
    }
}
