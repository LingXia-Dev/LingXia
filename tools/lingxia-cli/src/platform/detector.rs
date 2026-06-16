use super::Platform;
use super::android::AndroidPlatform;
use super::ios::IosPlatform;
use super::spm;
use crate::config::dir_matches_host_config;
use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub const ANDROID_ASSETS_REL_PATH: &str = "app/src/main/assets";

#[derive(Debug, Clone, PartialEq)]
pub enum PlatformType {
    Android,
    Ios,
    MacOs,
    Harmony,
    Windows,
}

impl PlatformType {
    pub fn as_str(&self) -> &str {
        match self {
            PlatformType::Android => "android",
            PlatformType::Ios => "ios",
            PlatformType::MacOs => "macos",
            PlatformType::Harmony => "harmony",
            PlatformType::Windows => "windows",
        }
    }
}

/// Find nearest ancestor host project root containing `host_config_file`.
pub fn find_host_project_root(start: &Path, host_config_file: &str) -> Option<PathBuf> {
    let mut current = start.parent();
    while let Some(dir) = current {
        if dir_matches_host_config(dir, host_config_file) {
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
            "windows" | "win" => Ok(PlatformType::Windows),
            _ => Err(anyhow!("Unknown platform: {}", s)),
        }
    }
}

/// Detect all available platforms in the project root and common platform subdirectories.
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

    if is_windows_project(project_root) || is_windows_project(&project_root.join("windows")) {
        platforms.push(PlatformType::Windows);
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
        PlatformType::Harmony => Ok(Box::new(super::harmony::HarmonyPlatform::new())),
        PlatformType::Windows => Ok(Box::new(super::windows::WindowsPlatform::new())),
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
    let platforms = detect_available_platforms(project_root);
    match platforms.as_slice() {
        [platform] => Ok(platform.clone()),
        [] => Err(anyhow!(
            "Cannot detect platform type. Make sure you're in a valid LingXia project directory.\n\
             Supported platforms: Android, iOS, macOS, HarmonyOS, Windows"
        )),
        _ => Err(anyhow!(
            "Multiple platform candidates found: {}.\n\
             Pass --platform <android|ios|macos|harmony|windows> to disambiguate.",
            format_platform_list(&platforms)
        )),
    }
}

fn format_platform_list(platforms: &[PlatformType]) -> String {
    platforms
        .iter()
        .map(PlatformType::as_str)
        .collect::<Vec<_>>()
        .join(", ")
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
    if matches!(
        spm::infer_apple_swift_package_platform(project_root),
        Ok(Some(PlatformType::Ios))
    ) {
        return true;
    }

    false
}

/// Check if the project is a macOS project
fn is_macos_project(project_root: &Path) -> bool {
    matches!(
        spm::infer_apple_swift_package_platform(project_root),
        Ok(Some(PlatformType::MacOs))
    )
}

/// Check if the project is a HarmonyOS project
fn is_harmony_project(project_root: &Path) -> bool {
    project_root.join("build-profile.json5").exists()
        || project_root.join("hvigorfile.ts").exists()
        || project_root.join("oh-package.json5").exists()
}

/// Check if the project is a Windows Rust host project.
fn is_windows_project(project_root: &Path) -> bool {
    let manifest = project_root.join("Cargo.toml");
    if !manifest.exists() {
        return false;
    }

    std::fs::read_to_string(manifest)
        .map(|content| content.contains("[package]") && content.contains("lingxia-windows-sdk"))
        .unwrap_or(false)
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

        let windows_root = project_root.join("windows");
        fs::create_dir_all(&windows_root).unwrap();
        fs::write(
            windows_root.join("Cargo.toml"),
            "[package]\nname = \"demo-windows\"\nversion = \"0.1.0\"\n[dependencies]\nlingxia-windows-sdk = \"0.9\"\n",
        )
        .unwrap();

        let platforms = detect_available_platforms(project_root);
        assert!(platforms.contains(&PlatformType::Ios));
        assert!(platforms.contains(&PlatformType::Harmony));
        assert!(platforms.contains(&PlatformType::Windows));
    }

    #[test]
    fn test_detect_single_platform_from_subdir_layout() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        let ios_root = project_root.join("ios");
        fs::create_dir_all(&ios_root).unwrap();
        fs::create_dir_all(ios_root.join("MyApp.xcodeproj")).unwrap();

        assert_eq!(
            detect_platform_type(project_root).unwrap(),
            PlatformType::Ios
        );
    }

    #[test]
    fn test_detect_multiple_platforms_reports_disambiguation() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        let ios_root = project_root.join("ios");
        fs::create_dir_all(&ios_root).unwrap();
        fs::create_dir_all(ios_root.join("MyApp.xcodeproj")).unwrap();

        let harmony_root = project_root.join("harmony");
        fs::create_dir_all(&harmony_root).unwrap();
        fs::write(harmony_root.join("build-profile.json5"), "").unwrap();

        let err = detect_platform_type(project_root).unwrap_err().to_string();
        assert!(err.contains("Multiple platform candidates found: ios, harmony"));
        assert!(err.contains("Pass --platform <android|ios|macos|harmony|windows>"));
    }
}
