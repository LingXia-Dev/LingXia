use super::Platform;
use super::android::AndroidPlatform;
use super::ios::IosPlatform;
use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub const ANDROID_ASSETS_REL_PATH: &str = "app/src/main/assets";

#[derive(Debug, Clone, PartialEq)]
pub enum PlatformType {
    Android,
    Ios,
    MacOs,
    Harmony,
}

#[derive(Debug, Clone)]
pub struct AppleSwiftPackageContext {
    pub host_project_root: PathBuf,
    pub inferred_platform: PlatformType,
}

impl PlatformType {
    pub fn as_str(&self) -> &str {
        match self {
            PlatformType::Android => "android",
            PlatformType::Ios => "ios",
            PlatformType::MacOs => "macos",
            PlatformType::Harmony => "harmony",
        }
    }
}

fn infer_apple_swift_package_platform(package_dir: &Path) -> Result<Option<PlatformType>> {
    let package_swift = package_dir.join("Package.swift");
    if !package_swift.exists() {
        return Ok(None);
    }

    // Prefer directory convention when present (`.../ios` or `.../macos`).
    if let Some(dir_name) = package_dir.file_name().and_then(|n| n.to_str()) {
        match dir_name.to_ascii_lowercase().as_str() {
            "ios" => return Ok(Some(PlatformType::Ios)),
            "macos" => return Ok(Some(PlatformType::MacOs)),
            _ => {}
        }
    }

    let content = std::fs::read_to_string(&package_swift)
        .with_context(|| format!("Failed to read {}", package_swift.display()))?;
    let has_ios = content.contains(".iOS") || content.contains(".ios");
    let has_macos = content.contains(".macOS") || content.contains(".macos");

    match (has_ios, has_macos) {
        (true, false) => Ok(Some(PlatformType::Ios)),
        (false, true) => Ok(Some(PlatformType::MacOs)),
        _ => Ok(None),
    }
}

/// When invoked inside an Apple Swift Package subdirectory, resolve the nearest
/// host project root (directory containing `host_config_file`) and infer platform.
pub fn find_apple_swift_package_context(
    start: &Path,
    host_config_file: &str,
) -> Result<Option<AppleSwiftPackageContext>> {
    let Some(inferred_platform) = infer_apple_swift_package_platform(start)? else {
        return Ok(None);
    };

    let mut current = start.parent();
    while let Some(dir) = current {
        if dir.join(host_config_file).exists() {
            return Ok(Some(AppleSwiftPackageContext {
                host_project_root: dir.to_path_buf(),
                inferred_platform,
            }));
        }
        current = dir.parent();
    }

    Ok(None)
}

/// Find nearest ancestor host project root containing `host_config_file`.
pub fn find_host_project_root(start: &Path, host_config_file: &str) -> Option<PathBuf> {
    let mut current = start.parent();
    while let Some(dir) = current {
        if dir.join(host_config_file).exists() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

impl FromStr for PlatformType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "android" => Ok(PlatformType::Android),
            "ios" => Ok(PlatformType::Ios),
            "macos" | "macosx" | "osx" | "mac" => Ok(PlatformType::MacOs),
            "harmony" | "harmonyos" => Ok(PlatformType::Harmony),
            _ => Err(anyhow!("Unknown platform: {}", s)),
        }
    }
}

#[cfg(test)]
/// Detect all available platforms in the project (test helper).
pub fn detect_available_platforms(project_root: &Path) -> Vec<PlatformType> {
    let mut platforms = Vec::new();

    if is_android_project(project_root) {
        platforms.push(PlatformType::Android);
    }

    if is_ios_project(project_root) || is_ios_project(&project_root.join("ios")) {
        platforms.push(PlatformType::Ios);
    }

    if is_macos_project(project_root) || is_macos_project(&project_root.join("macos")) {
        platforms.push(PlatformType::MacOs);
    }

    if is_harmony_project(project_root)
        || is_harmony_project(&project_root.join("harmony"))
        || is_harmony_project(&project_root.join("harmonyos"))
    {
        platforms.push(PlatformType::Harmony);
    }

    platforms
}

/// Create a platform instance for the given platform type
pub fn create_platform(platform_type: &PlatformType) -> Result<Box<dyn Platform>> {
    match platform_type {
        PlatformType::Android => Ok(Box::new(AndroidPlatform::new())),
        PlatformType::Ios => {
            super::apple::ensure_macos()?;
            Ok(Box::new(IosPlatform::new()))
        }
        PlatformType::MacOs => {
            super::apple::ensure_macos()?;
            Ok(Box::new(super::macos::MacosPlatform::new()))
        }
        PlatformType::Harmony => Err(anyhow!("HarmonyOS support is not yet implemented")),
    }
}

fn has_android_gradle_files(project_root: &Path) -> bool {
    project_root.join("build.gradle.kts").exists()
        || project_root.join("build.gradle").exists()
        || project_root.join("app/build.gradle.kts").exists()
        || project_root.join("app/build.gradle").exists()
}

/// Resolve the Android project directory
///
/// Supports both layouts:
/// - Multi-platform layout: `{projectRoot}/android/` (contains Gradle files)
/// - Standalone Android project: `{projectRoot}/` (contains Gradle files)
pub fn resolve_android_dir(project_root: &Path) -> PathBuf {
    let android_dir = project_root.join("android");
    if android_dir.exists() && android_dir.is_dir() && has_android_gradle_files(&android_dir) {
        android_dir
    } else {
        project_root.to_path_buf()
    }
}

/// Resolve the Android assets directory for the given project.
pub fn resolve_android_assets_dir(project_root: &Path) -> PathBuf {
    resolve_android_dir(project_root).join(ANDROID_ASSETS_REL_PATH)
}

/// Detect the platform type based on project structure.
///
/// This function examines the project directory for platform-specific files:
/// - Android: build.gradle.kts or build.gradle
/// - iOS: *.xcodeproj, *.xcworkspace, or Podfile
/// - HarmonyOS: build-profile.json5, hvigorfile.ts, or oh-package.json5
///
pub fn detect_platform_type(project_root: &Path) -> Result<PlatformType> {
    // Android: check for build.gradle or build.gradle.kts
    if is_android_project(project_root) {
        return Ok(PlatformType::Android);
    }

    // iOS: check for *.xcodeproj, *.xcworkspace, or Package.swift with iOS
    if is_ios_project(project_root) {
        return Ok(PlatformType::Ios);
    }

    // macOS: check for Package.swift with macOS or macOS project dir
    if is_macos_project(project_root) {
        return Ok(PlatformType::MacOs);
    }

    // HarmonyOS: check for build-profile.json5
    if is_harmony_project(project_root) {
        return Err(anyhow!(
            "HarmonyOS project detected, but HarmonyOS support is not yet implemented"
        ));
    }

    Err(anyhow!(
        "Cannot detect platform type. Make sure you're in a valid LingXia project directory.\n\
         Supported platforms: Android, iOS (coming soon), HarmonyOS (coming soon)"
    ))
}

/// Check if the project is an Android project
fn is_android_project(project_root: &Path) -> bool {
    // Check for Android subproject directory structure (LingXia multi-platform layout)
    let android_dir = project_root.join("android");
    if android_dir.exists() && android_dir.is_dir() && has_android_gradle_files(&android_dir) {
        return true;
    }

    // Check for standalone Android project (root directory)
    has_android_gradle_files(project_root)
}

/// Check if the project is an iOS project
fn is_ios_project(project_root: &Path) -> bool {
    // Check for Xcode project or workspace
    if let Ok(entries) = std::fs::read_dir(project_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy();
                if ext_str == "xcodeproj" || ext_str == "xcworkspace" {
                    return true;
                }
            }
        }
    }

    // Check for Podfile (CocoaPods)
    if project_root.join("Podfile").exists() {
        return true;
    }

    // Check for Swift Package with iOS platform
    let package_swift = project_root.join("Package.swift");
    if package_swift.exists()
        && let Ok(content) = std::fs::read_to_string(&package_swift)
        && (content.contains(".iOS") || content.contains(".ios"))
    {
        return true;
    }

    false
}

/// Check if the project is a macOS project
fn is_macos_project(project_root: &Path) -> bool {
    // Swift Package with macOS platform
    let package_swift = project_root.join("Package.swift");
    if package_swift.exists()
        && let Ok(content) = std::fs::read_to_string(&package_swift)
        && (content.contains(".macOS") || content.contains(".macos"))
    {
        return true;
    }

    false
}

/// Check if the project is a HarmonyOS project
fn is_harmony_project(project_root: &Path) -> bool {
    project_root.join("build-profile.json5").exists()
        || project_root.join("hvigorfile.ts").exists()
        || project_root.join("oh-package.json5").exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_android_project() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Create build.gradle.kts
        fs::write(project_root.join("build.gradle.kts"), "").unwrap();

        assert!(is_android_project(project_root));
    }

    #[test]
    fn test_detect_android_project_in_subdir() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();
        let android_root = project_root.join("android");
        fs::create_dir_all(android_root.join("app")).unwrap();
        fs::write(android_root.join("build.gradle.kts"), "").unwrap();
        fs::write(android_root.join("app/build.gradle.kts"), "").unwrap();

        assert!(is_android_project(project_root));
        assert_eq!(resolve_android_dir(project_root), android_root);
    }

    #[test]
    fn test_detect_harmony_project() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Create build-profile.json5
        fs::write(project_root.join("build-profile.json5"), "").unwrap();

        assert!(is_harmony_project(project_root));
    }

    #[test]
    fn test_detect_platforms_in_subdirs() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // iOS project under ios/
        let ios_root = project_root.join("ios");
        fs::create_dir_all(&ios_root).unwrap();
        fs::create_dir_all(ios_root.join("MyApp.xcodeproj")).unwrap();

        // Harmony project under harmony/
        let harmony_root = project_root.join("harmony");
        fs::create_dir_all(&harmony_root).unwrap();
        fs::write(harmony_root.join("build-profile.json5"), "").unwrap();

        let platforms = detect_available_platforms(project_root);
        assert!(platforms.contains(&PlatformType::Ios));
        assert!(platforms.contains(&PlatformType::Harmony));
    }
}
