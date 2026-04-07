//! SDK download and management module.
//!
//! Downloads LingXia SDK artifacts from GitHub releases and stages them locally.

use crate::github;
use anyhow::{Context, Result, anyhow};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use toml::Value;

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
    let manifest = parse_manifest_value(&manifest_path, &content)?;
    let dependencies = manifest
        .get("dependencies")
        .and_then(Value::as_table)
        .ok_or_else(|| {
            anyhow!(
                "Dependency 'lingxia' not found in {}",
                manifest_path.display()
            )
        })?;
    let dependency = dependencies.get("lingxia").ok_or_else(|| {
        anyhow!(
            "Dependency 'lingxia' not found in {}",
            manifest_path.display()
        )
    })?;

    resolve_dependency_version(manifest_path.parent().unwrap_or(project_root), dependency)
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
    let tag = format!("lingxia-sdk-v{version}");
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

    if let Some(local_sdk_dir) = find_local_apple_sdk_root(project_root) {
        stage_local_apple_sdk(&local_sdk_dir, &staged_dir, &marker_path, version)?;
        return Ok(staged_dir);
    }

    println!("  Downloading LingXia Apple SDK v{}...", version);

    let asset_name = SdkPlatform::Apple.asset_name(version);
    let tag = format!("lingxia-sdk-v{version}");
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
    let tag = format!("lingxia-sdk-v{version}");
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

fn find_local_apple_sdk_root(project_root: &Path) -> Option<PathBuf> {
    for dir in project_root.ancestors() {
        if dir.join(".git").exists() || dir.join("Cargo.lock").exists() {
            if is_lingxia_repo_root(dir) {
                let candidate = dir.join("lingxia-sdk").join("apple");
                if candidate.join("Package.swift").exists() {
                    return Some(candidate);
                }
            }
            break;
        }
    }
    None
}

fn is_lingxia_repo_root(dir: &Path) -> bool {
    dir.join("Cargo.toml").exists()
        && dir
            .join("crates")
            .join("lingxia")
            .join("Cargo.toml")
            .exists()
        && dir
            .join("tools")
            .join("lingxia-cli")
            .join("Cargo.toml")
            .exists()
        && dir
            .join("lingxia-sdk")
            .join("apple")
            .join("Package.swift")
            .exists()
}

fn stage_local_apple_sdk(
    source_dir: &Path,
    staged_dir: &Path,
    marker_path: &Path,
    version: &str,
) -> Result<()> {
    if staged_dir.exists() {
        fs::remove_dir_all(staged_dir)?;
    }
    fs::create_dir_all(staged_dir)?;
    crate::platform::apple::copy_dir_recursive(source_dir, staged_dir)?;
    fs::write(marker_path, format!("{version}\n"))?;
    Ok(())
}

fn parse_manifest_value(manifest_path: &Path, content: &str) -> Result<Value> {
    toml::from_str(content)
        .with_context(|| format!("Failed to parse TOML manifest {}", manifest_path.display()))
}

fn resolve_dependency_version(manifest_dir: &Path, dependency: &Value) -> Result<String> {
    match dependency {
        Value::String(version) if !version.trim().is_empty() => Ok(version.trim().to_string()),
        Value::Table(table) => {
            if let Some(version) = extract_version_value(table.get("version"))? {
                return Ok(version);
            }

            if table.get("workspace").and_then(Value::as_bool) == Some(true) {
                return resolve_workspace_package_version(manifest_dir);
            }

            if let Some(path) = table.get("path").and_then(Value::as_str) {
                return resolve_version_from_dependency_path(manifest_dir, Path::new(path));
            }

            Err(anyhow!(
                "Unable to parse 'lingxia' version from dependency entry in {}",
                manifest_dir.join("Cargo.toml").display()
            ))
        }
        _ => Err(anyhow!(
            "Unsupported 'lingxia' dependency format in {}",
            manifest_dir.join("Cargo.toml").display()
        )),
    }
}

fn resolve_version_from_dependency_path(
    manifest_dir: &Path,
    dependency_path: &Path,
) -> Result<String> {
    let dependency_manifest = manifest_dir.join(dependency_path).join("Cargo.toml");
    let content = fs::read_to_string(&dependency_manifest)
        .with_context(|| format!("Failed to read {}", dependency_manifest.display()))?;
    let manifest = parse_manifest_value(&dependency_manifest, &content)?;
    let package = manifest
        .get("package")
        .and_then(Value::as_table)
        .ok_or_else(|| {
            anyhow!(
                "Missing [package] section in {}",
                dependency_manifest.display()
            )
        })?;

    if let Some(version) = extract_version_value(package.get("version"))? {
        return Ok(version);
    }

    resolve_workspace_package_version(dependency_manifest.parent().unwrap_or_else(|| manifest_dir))
}

fn extract_version_value(value: Option<&Value>) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };

    match value {
        Value::String(version) if !version.trim().is_empty() => {
            Ok(Some(version.trim().to_string()))
        }
        Value::Table(table) if table.get("workspace").and_then(Value::as_bool) == Some(true) => {
            Ok(None)
        }
        Value::Table(_) => Err(anyhow!(
            "Unsupported version field format in Cargo manifest"
        )),
        _ => Err(anyhow!(
            "Unsupported version field format in Cargo manifest"
        )),
    }
}

fn resolve_workspace_package_version(start: &Path) -> Result<String> {
    for dir in start.ancestors() {
        let manifest_path = dir.join("Cargo.toml");
        if !manifest_path.exists() {
            // Stop at repo root even if no Cargo.toml exists here.
            if dir.join(".git").exists() {
                break;
            }
            continue;
        }

        let content = fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
        let manifest = parse_manifest_value(&manifest_path, &content)?;
        let Some(workspace) = manifest.get("workspace").and_then(Value::as_table) else {
            // A Cargo.toml without [workspace] at the repo root means we've gone far enough.
            if dir.join(".git").exists() || dir.join("Cargo.lock").exists() {
                break;
            }
            continue;
        };
        let Some(package) = workspace.get("package").and_then(Value::as_table) else {
            continue;
        };
        if let Some(version) = package.get("version").and_then(Value::as_str)
            && !version.trim().is_empty()
        {
            return Ok(version.trim().to_string());
        }
    }

    Err(anyhow!(
        "Unable to resolve workspace.package.version starting from {}",
        start.display()
    ))
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
    use super::{
        extract_version_value, parse_android_sdk_version_from_gradle, parse_manifest_value,
        resolve_dependency_version,
    };
    use std::path::Path;
    use toml::Value;

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
        let dependency: Value = toml::from_str(
            r#"
                [dependencies]
                lingxia = { version = "0.3.1", features = ["cloud"] }
            "#,
        )
        .unwrap();
        let dependency = dependency
            .get("dependencies")
            .and_then(Value::as_table)
            .and_then(|deps| deps.get("lingxia"))
            .unwrap();
        assert_eq!(
            resolve_dependency_version(Path::new("/tmp"), dependency).unwrap(),
            "0.3.1"
        );
    }

    #[test]
    fn parse_dependency_version_from_string_style() {
        let dependency: Value = toml::from_str(
            r#"
                [dependencies]
                lingxia = "0.3.2"
            "#,
        )
        .unwrap();
        let dependency = dependency
            .get("dependencies")
            .and_then(Value::as_table)
            .and_then(|deps| deps.get("lingxia"))
            .unwrap();
        assert_eq!(
            resolve_dependency_version(Path::new("/tmp"), dependency).unwrap(),
            "0.3.2"
        );
    }

    #[test]
    fn extract_version_value_supports_workspace_marker() {
        let manifest =
            parse_manifest_value(Path::new("/tmp/Cargo.toml"), "version.workspace = true").unwrap();
        assert_eq!(
            extract_version_value(manifest.get("version")).unwrap(),
            None
        );
    }
}
