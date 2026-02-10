//! SDK download and management module.
//!
//! Downloads LingXia SDK artifacts from GitHub releases and stages them locally.

use crate::github;
use anyhow::{Context, Result};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

/// SDK type
#[derive(Debug, Clone, Copy)]
pub enum SdkPlatform {
    Android,
    Apple,
    Harmony,
}

impl SdkPlatform {
    fn asset_name(&self, version: &str) -> String {
        match self {
            SdkPlatform::Android => format!("lingxia-sdk-android-maven-{}.zip", version),
            SdkPlatform::Apple => format!("lingxia-sdk-apple-source-{}.zip", version),
            SdkPlatform::Harmony => format!("lingxia-sdk-harmony-{}.har", version),
        }
    }
}

/// Ensure SDK is available locally for the requested platform.
///
/// If the SDK is not found locally, download it from GitHub releases.
/// Returns the platform-specific staged path:
/// - Android: local Maven repository directory
/// - Apple: staged SwiftPM package directory
/// - Harmony: local HAR file path
///
pub fn ensure_sdk(project_root: &Path, platform: SdkPlatform, version: &str) -> Result<PathBuf> {
    match platform {
        SdkPlatform::Android => ensure_android_sdk(project_root, version),
        SdkPlatform::Apple => ensure_apple_sdk(project_root, version),
        SdkPlatform::Harmony => ensure_harmony_sdk(project_root, version),
    }
}

fn ensure_android_sdk(project_root: &Path, version: &str) -> Result<PathBuf> {
    let sdk_root = resolve_sdk_root(project_root);
    let maven_dir = sdk_root.join("target").join("maven");
    let sdk_marker = maven_dir
        .join("com")
        .join("lingxia")
        .join("lingxia")
        .join(version)
        .join(format!("lingxia-{version}.aar"));

    if sdk_marker.exists() {
        return Ok(maven_dir);
    }

    println!("  Downloading LingXia SDK v{}...", version);

    let asset_name = SdkPlatform::Android.asset_name(version);
    let tag = format!("sdk-v{version}");
    let zip_data = github::download_release_asset(&tag, &asset_name)?;

    fs::create_dir_all(&maven_dir)?;
    extract_zip(&zip_data, &maven_dir, strip_to_maven_repo_root)?;

    if !sdk_marker.exists() {
        anyhow::bail!(
            "SDK download completed but expected artifact was not found after extraction.\n  Expected: {}\n  Maven dir: {}",
            sdk_marker.display(),
            maven_dir.display()
        );
    }

    println!("  SDK downloaded to: {}", maven_dir.display());
    Ok(maven_dir)
}

fn ensure_apple_sdk(project_root: &Path, version: &str) -> Result<PathBuf> {
    let sdk_root = resolve_sdk_root(project_root);
    let staged_dir = sdk_root.join("target").join("spm").join("lingxia");
    let marker_path = staged_dir.join(".lingxia-sdk-version");
    let package_manifest = staged_dir.join("Package.swift");

    if package_manifest.exists() && version_marker_matches(&marker_path, version)? {
        return Ok(staged_dir);
    }

    println!("  Downloading LingXia Apple SDK v{}...", version);

    let asset_name = SdkPlatform::Apple.asset_name(version);
    let tag = format!("sdk-v{version}");
    let zip_data = github::download_release_asset(&tag, &asset_name)?;

    if staged_dir.exists() {
        fs::remove_dir_all(&staged_dir)?;
    }
    fs::create_dir_all(&staged_dir)?;
    extract_zip(&zip_data, &staged_dir, strip_to_apple_sdk_root)?;
    fs::write(&marker_path, format!("{version}\n"))?;

    if !package_manifest.exists() {
        anyhow::bail!(
            "Apple SDK download completed but expected Package.swift was not found.\n  Expected: {}\n  Staged dir: {}",
            package_manifest.display(),
            staged_dir.display()
        );
    }

    println!("  SDK downloaded to: {}", staged_dir.display());
    Ok(staged_dir)
}

fn ensure_harmony_sdk(project_root: &Path, version: &str) -> Result<PathBuf> {
    let sdk_root = resolve_sdk_root(project_root);
    let ohpm_dir = sdk_root.join("target").join("ohpm");
    let har_path = ohpm_dir.join("lingxia.har");
    let marker_path = ohpm_dir.join(".lingxia-sdk-version");

    if har_path.exists() && version_marker_matches(&marker_path, version)? {
        return Ok(har_path);
    }

    println!("  Downloading LingXia Harmony SDK v{}...", version);

    let asset_name = SdkPlatform::Harmony.asset_name(version);
    let tag = format!("sdk-v{version}");
    let har_data = github::download_release_asset(&tag, &asset_name)?;

    fs::create_dir_all(&ohpm_dir)?;
    fs::write(&har_path, har_data)?;
    fs::write(&marker_path, format!("{version}\n"))?;

    println!("  SDK downloaded to: {}", har_path.display());
    Ok(har_path)
}

type ZipPathMapper = fn(&Path) -> Option<PathBuf>;

/// Extract a zip archive to the target directory.
fn extract_zip(zip_data: &[u8], target_dir: &Path, map_path: ZipPathMapper) -> Result<()> {
    use std::io::Cursor;

    let reader = Cursor::new(zip_data);
    let mut archive = zip::ZipArchive::new(reader).context("Failed to open zip archive")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("Failed to read zip entry")?;
        let outpath = match file.enclosed_name().as_deref().and_then(map_path) {
            Some(path) => target_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent()
                && !parent.exists()
            {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }

    Ok(())
}

fn strip_to_maven_repo_root(path: &Path) -> Option<PathBuf> {
    let parts: Vec<OsString> = path
        .components()
        .map(|c| c.as_os_str().to_os_string())
        .collect();
    let start = parts
        .iter()
        .rposition(|c| c == OsStr::new("maven"))
        .map(|idx| idx + 1)
        .unwrap_or(0);

    if start >= parts.len() {
        return None;
    }

    let mut out = PathBuf::new();
    for part in parts.into_iter().skip(start) {
        out.push(part);
    }
    Some(out)
}

fn strip_to_apple_sdk_root(path: &Path) -> Option<PathBuf> {
    let parts: Vec<OsString> = path
        .components()
        .map(|c| c.as_os_str().to_os_string())
        .collect();

    let start = if parts
        .first()
        .is_some_and(|part| part == OsStr::new("lingxia-apple-sdk"))
    {
        1
    } else {
        0
    };

    if start >= parts.len() {
        return None;
    }

    let mut out = PathBuf::new();
    for part in parts.into_iter().skip(start) {
        out.push(part);
    }
    Some(out)
}

fn version_marker_matches(path: &Path, version: &str) -> Result<bool> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(content.trim() == version),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err.into()),
    }
}

fn resolve_sdk_root(start: &Path) -> PathBuf {
    start.to_path_buf()
}
