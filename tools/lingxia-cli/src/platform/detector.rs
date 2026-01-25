use super::android::AndroidPlatform;
use super::Platform;
use anyhow::{anyhow, Result};
use std::path::Path;

/// Detect the platform type based on project structure.
///
/// This function examines the project directory for platform-specific files:
/// - Android: build.gradle.kts or build.gradle
/// - iOS: *.xcodeproj, *.xcworkspace, or Podfile
/// - HarmonyOS: build-profile.json5, hvigorfile.ts, or oh-package.json5
///
/// Returns a boxed Platform implementation for the detected platform.
pub fn detect_platform(project_root: &Path) -> Result<Box<dyn Platform>> {
    // Android: check for build.gradle or build.gradle.kts
    if is_android_project(project_root) {
        return Ok(Box::new(AndroidPlatform::new()));
    }

    // iOS: check for *.xcodeproj or *.xcworkspace
    if is_ios_project(project_root) {
        return Err(anyhow!(
            "iOS project detected, but iOS support is not yet implemented"
        ));
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
    project_root.join("build.gradle.kts").exists()
        || project_root.join("build.gradle").exists()
        || project_root.join("app/build.gradle.kts").exists()
        || project_root.join("app/build.gradle").exists()
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
    project_root.join("Podfile").exists()
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
    fn test_detect_harmony_project() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Create build-profile.json5
        fs::write(project_root.join("build-profile.json5"), "").unwrap();

        assert!(is_harmony_project(project_root));
    }
}
