use super::detector::PlatformType;
use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppleSwiftPackageContext {
    pub host_project_root: PathBuf,
    pub inferred_platform: PlatformType,
}

#[derive(Debug, Clone, Default)]
pub struct AppleSwiftPackageInfoDefaults {
    pub bundle_id: Option<String>,
    pub product_name: Option<String>,
}

pub fn infer_apple_swift_package_platform(package_dir: &Path) -> Result<Option<PlatformType>> {
    let package_swift = package_dir.join("Package.swift");
    if !package_swift.exists() {
        return Ok(None);
    }

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

pub fn detect_local_apple_swift_package_platform(start: &Path) -> Result<Option<PlatformType>> {
    infer_apple_swift_package_platform(start)
}

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

pub fn resolve_apple_swift_package_dir(
    project_root: &Path,
    preferred_subdir: &str,
    shared_fallback_subdir: Option<&str>,
    platform_label: &str,
) -> Result<PathBuf> {
    if project_root.join("Package.swift").exists() {
        return Ok(project_root.to_path_buf());
    }

    let preferred_dir = project_root.join(preferred_subdir);
    if preferred_dir.join("Package.swift").exists() {
        return Ok(preferred_dir);
    }

    if let Some(shared_subdir) = shared_fallback_subdir {
        let shared_dir = project_root.join(shared_subdir);
        if shared_dir.join("Package.swift").exists() {
            return Ok(shared_dir);
        }
    }

    let mut expected = vec![format!("- {}/{preferred_subdir}/", project_root.display())];
    if let Some(shared_subdir) = shared_fallback_subdir {
        expected.push(format!(
            "- {}/{shared_subdir}/ (shared)",
            project_root.display()
        ));
    }

    Err(anyhow!(
        "{platform_label} Swift Package not found.\n\
         Expected Package.swift in:\n\
         {}",
        expected.join("\n")
    ))
}

pub fn read_package_info_defaults(info_plist_path: &Path) -> Result<AppleSwiftPackageInfoDefaults> {
    if !info_plist_path.exists() {
        return Ok(AppleSwiftPackageInfoDefaults::default());
    }

    let info: plist::Dictionary =
        plist::from_file(info_plist_path).context("Failed to read Info.plist")?;

    Ok(AppleSwiftPackageInfoDefaults {
        bundle_id: info
            .get("CFBundleIdentifier")
            .and_then(|value| value.as_string())
            .map(ToOwned::to_owned),
        product_name: info
            .get("CFBundleDisplayName")
            .and_then(|value| value.as_string())
            .or_else(|| info.get("CFBundleName").and_then(|value| value.as_string()))
            .map(ToOwned::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn infers_platform_from_package_directory_name() {
        let temp = TempDir::new().unwrap();
        let macos_dir = temp.path().join("macos");
        fs::create_dir_all(&macos_dir).unwrap();
        fs::write(
            macos_dir.join("Package.swift"),
            "// swift-tools-version: 6.0",
        )
        .unwrap();

        assert_eq!(
            detect_local_apple_swift_package_platform(&macos_dir).unwrap(),
            Some(PlatformType::MacOs)
        );
    }

    #[test]
    fn resolves_preferred_package_directory() {
        let temp = TempDir::new().unwrap();
        let ios_dir = temp.path().join("ios");
        fs::create_dir_all(&ios_dir).unwrap();
        fs::write(ios_dir.join("Package.swift"), "// swift-tools-version: 6.0").unwrap();

        let resolved = resolve_apple_swift_package_dir(temp.path(), "ios", None, "iOS").unwrap();
        assert_eq!(resolved, ios_dir);
    }
}
