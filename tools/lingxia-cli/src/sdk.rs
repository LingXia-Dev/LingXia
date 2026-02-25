//! SDK download and management module.
//!
//! Downloads LingXia SDK artifacts from GitHub releases and stages them locally.

use crate::github;
use anyhow::{Context, Result, anyhow};
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

pub fn resolve_sdk_version_from_rust_manifest(
    project_root: &Path,
    rust_lib_name: &str,
) -> Result<String> {
    let manifest_path = project_root.join(rust_lib_name).join("Cargo.toml");
    let content = fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;

    let mut section = "";
    for raw_line in content.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            continue;
        }

        if section != "dependencies" && !section.ends_with(".dependencies") {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "lingxia" {
            continue;
        }

        if let Some(version) = parse_dependency_version(value.trim()) {
            return Ok(version);
        }

        return Err(anyhow!(
            "Unable to parse 'lingxia' version from {}. Use a literal version, e.g. lingxia = {{ version = \"0.3.0\" }}.",
            manifest_path.display()
        ));
    }

    Err(anyhow!(
        "Dependency 'lingxia' not found in {}",
        manifest_path.display()
    ))
}

pub fn ensure_android_sdk_from_gradle(project_root: &Path, android_root: &Path) -> Result<()> {
    let gradle_kts = android_root.join("app/build.gradle.kts");
    let gradle = android_root.join("app/build.gradle");
    let gradle_path = if gradle_kts.exists() {
        gradle_kts
    } else if gradle.exists() {
        gradle
    } else {
        return Err(anyhow!(
            "Android Gradle file not found: expected {} or {}",
            android_root.join("app/build.gradle.kts").display(),
            android_root.join("app/build.gradle").display()
        ));
    };

    let content = fs::read_to_string(&gradle_path)
        .with_context(|| format!("Failed to read {}", gradle_path.display()))?;
    let sdk_version = parse_android_sdk_version_from_gradle(&content).ok_or_else(|| {
        anyhow!(
            "Unable to resolve LingXia Android SDK version from {}.\n\
Expected dependency format: implementation(\"com.lingxia:lingxia:<version>\")",
            gradle_path.display()
        )
    })?;

    ensure_sdk(project_root, SdkPlatform::Android, &sdk_version)?;
    Ok(())
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

fn parse_dependency_version(value: &str) -> Option<String> {
    let trimmed = value.trim();

    // `lingxia = "0.3.0"`
    if let Some(first) = trimmed.chars().next()
        && (first == '"' || first == '\'')
    {
        let inner = trimmed.trim_matches(first).trim();
        if !inner.is_empty() {
            return Some(inner.to_string());
        }
    }

    // `lingxia = { version = "0.3.0", ... }`
    let marker = "version";
    let idx = trimmed.find(marker)?;
    let tail = trimmed[idx + marker.len()..].trim_start();
    let tail = tail.strip_prefix('=')?.trim_start();
    let quote = tail.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &tail[quote.len_utf8()..];
    let end = rest.find(quote)?;
    let version = rest[..end].trim();
    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

fn parse_android_sdk_version_from_gradle(content: &str) -> Option<String> {
    const MARKER: &str = "com.lingxia:lingxia:";
    let start = content.find(MARKER)? + MARKER.len();
    let tail = &content[start..];
    let version: String = tail
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | '+'))
        .collect();
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_android_sdk_version_from_gradle, parse_dependency_version};

    #[test]
    fn parse_android_sdk_version_from_kts_dependency() {
        let content = r#"implementation("com.lingxia:lingxia:0.3.0")"#;
        assert_eq!(
            parse_android_sdk_version_from_gradle(content),
            Some("0.3.0".to_string())
        );
    }

    #[test]
    fn parse_android_sdk_version_returns_none_for_dynamic_expression() {
        let content = r#"implementation("com.lingxia:lingxia:${versions.lingxia}")"#;
        assert_eq!(parse_android_sdk_version_from_gradle(content), None);
    }

    #[test]
    fn parse_dependency_version_from_table_style() {
        let value = r#"{ version = "0.3.1", default-features = false }"#;
        assert_eq!(parse_dependency_version(value), Some("0.3.1".to_string()));
    }

    #[test]
    fn parse_dependency_version_from_string_style() {
        let value = r#""0.3.2""#;
        assert_eq!(parse_dependency_version(value), Some("0.3.2".to_string()));
    }
}
