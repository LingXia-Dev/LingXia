//! SDK download and management module.
//!
//! Downloads LingXia SDK from GitHub releases and extracts to local Maven repository.

use crate::github;
use anyhow::{Context, Result};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

/// SDK type
#[derive(Debug, Clone, Copy)]
pub enum SdkPlatform {
    Android,
}

impl SdkPlatform {
    fn asset_name(&self, version: &str) -> String {
        match self {
            SdkPlatform::Android => format!("lingxia-sdk-android-maven-{}.zip", version),
        }
    }
}

/// Ensure SDK is available in the local Maven repository.
///
/// If the SDK is not found locally, download it from GitHub releases.
/// Returns the path to the local Maven repository.
pub fn ensure_sdk(project_root: &Path, platform: SdkPlatform, version: &str) -> Result<PathBuf> {
    // SDK goes into {project}-lib/target/maven
    let project_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("app");
    let maven_dir = project_root
        .join(format!("{project_name}-lib"))
        .join("target")
        .join("maven");
    let sdk_marker = maven_dir
        .join("com")
        .join("lingxia")
        .join("lingxia")
        .join(version)
        .join(format!("lingxia-{version}.aar"));

    // Check if SDK already exists
    if sdk_marker.exists() {
        return Ok(maven_dir);
    }

    println!("  Downloading LingXia SDK v{}...", version);

    let tag = format!("sdk-v{}", version);
    let asset_name = platform.asset_name(version);

    let zip_data = github::download_release_asset(&tag, &asset_name)?;

    // Create target directory and extract
    fs::create_dir_all(&maven_dir)?;
    extract_zip(&zip_data, &maven_dir)?;

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

/// Extract a zip archive to the target directory.
fn extract_zip(zip_data: &[u8], target_dir: &Path) -> Result<()> {
    use std::io::Cursor;

    let reader = Cursor::new(zip_data);
    let mut archive = zip::ZipArchive::new(reader).context("Failed to open zip archive")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("Failed to read zip entry")?;
        let outpath = match file
            .enclosed_name()
            .as_deref()
            .and_then(strip_to_maven_repo_root)
        {
            Some(path) => target_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)?;
                }
            }
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }

    Ok(())
}

fn strip_to_maven_repo_root(path: &Path) -> Option<PathBuf> {
    // Support zips with layout:
    // - maven/com/... (direct)
    // - <wrapper>/maven/com/... (wrapper dir)
    // If no "maven" component exists, extract as-is.
    use std::ffi::{OsStr, OsString};

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
