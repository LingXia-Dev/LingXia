use crate::config::HOST_CONFIG_FILE;
use crate::platform::detector::PlatformType;
use crate::platform::{self, InstallConfig};
use anyhow::{Result, anyhow};
use std::env;
use std::path::{Path, PathBuf};

/// Execute the install command
///
/// Installs the built application to a connected device.
/// Auto-detects the artifact if path is not provided.
pub fn execute(
    artifact: Option<String>,
    device: Option<String>,
    platform_arg: Option<String>,
    reinstall: bool,
    quiet: bool,
) -> Result<()> {
    let current_dir = env::current_dir()?;
    let project_root = platform::detector::find_host_project_root(&current_dir, HOST_CONFIG_FILE)
        .unwrap_or_else(|| current_dir.clone());

    let mut artifact_path = artifact.map(PathBuf::from);
    let mut packaged_platform = None;
    if let Some(dir) = artifact_path
        .as_deref()
        .filter(|path| path.is_dir() && !is_app_bundle(path))
    {
        let (platform, file, summary) = packaged_installable(dir)?;
        if !quiet {
            println!("Package {summary}");
        }
        packaged_platform = Some(platform);
        artifact_path = Some(file);
    }

    // Detect platform from argument, the package manifest, artifact
    // extension, or project structure.
    let platform_type = match (platform_arg, packaged_platform) {
        (Some(p), Some(packaged)) if p.parse::<PlatformType>()? != packaged => {
            return Err(anyhow!(
                "the package is for {}, but --platform is {p}",
                packaged.as_str()
            ));
        }
        (Some(p), _) => p.parse::<PlatformType>()?,
        (None, Some(packaged)) => packaged,
        (None, None) => detect_platform_from_artifact(artifact_path.as_deref(), &project_root)?,
    };
    let platform = platform::detector::create_platform(&platform_type)?;

    let config = InstallConfig {
        project_root: project_root.clone(),
        artifact_path,
        device_id: device,
        reinstall,
        quiet,
    };

    platform.install(&config)?;

    Ok(())
}

/// An iOS `.app` is a directory too, but it is the artifact itself.
fn is_app_bundle(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
}

/// The installable artifact a `lingxia package` directory recorded, and a
/// one-line description of the package it came from.
fn packaged_installable(dir: &Path) -> Result<(PlatformType, PathBuf, String)> {
    let manifest = crate::dist_manifest::read(dir)?;
    let platform: PlatformType = manifest.platform.parse()?;
    let formats: &[&str] = match platform {
        PlatformType::Android => &["apk"],
        PlatformType::Ios => &["ipa"],
        PlatformType::Harmony => &["hap"],
        other => {
            return Err(anyhow!(
                "`lingxia install` installs onto Android, iOS, and HarmonyOS devices, not {}",
                other.as_str()
            ));
        }
    };
    let (manifest, file) = crate::dist_manifest::resolve(dir, formats)?;
    let summary = format!(
        "{} {} (env {}): {}",
        manifest.platform,
        manifest.version,
        manifest.env,
        file.display()
    );
    Ok((platform, file, summary))
}

/// Detect platform from artifact file extension or project structure
fn detect_platform_from_artifact(
    artifact: Option<&Path>,
    project_root: &Path,
) -> Result<PlatformType> {
    // First check artifact extension
    if let Some(ext) = artifact.and_then(|p| p.extension()) {
        let ext_str = ext.to_string_lossy().to_lowercase();
        match ext_str.as_str() {
            "apk" | "aab" => return Ok(PlatformType::Android),
            "app" | "ipa" => return Ok(PlatformType::Ios),
            "hap" => return Ok(PlatformType::Harmony),
            "exe" => return Ok(PlatformType::Windows),
            _ => {}
        }
    }

    // Fallback to project structure detection
    platform::detector::detect_platform_type(project_root).map_err(|e| {
        anyhow!(
            "{}\n\nTip: pass --artifact <path> to disambiguate when the project contains multiple platforms.",
            e
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_package_directory_installs_its_recorded_artifact() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        let apk = dir.join("Demo-1.0.0.apk");
        std::fs::write(&apk, b"apk").unwrap();
        crate::dist_manifest::write(
            dir,
            "android",
            "1.0.0",
            "prod",
            None,
            &[crate::dist_manifest::Produced {
                format: "apk",
                path: apk.clone(),
            }],
        )
        .unwrap();
        let (platform, file, summary) = packaged_installable(dir).unwrap();
        assert_eq!((platform, &file), (PlatformType::Android, &apk));
        assert_eq!(
            summary,
            format!("android 1.0.0 (env prod): {}", apk.display())
        );
    }

    #[test]
    fn an_ios_package_directory_installs_its_ipa() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        let ipa = dir.join("Demo-1.0.0-dev.ipa");
        std::fs::write(&ipa, b"ipa").unwrap();
        crate::dist_manifest::write(
            dir,
            "ios",
            "1.0.0",
            "dev",
            None,
            &[crate::dist_manifest::Produced {
                format: "ipa",
                path: ipa.clone(),
            }],
        )
        .unwrap();
        let (platform, file, _) = packaged_installable(dir).unwrap();
        assert_eq!((platform, file), (PlatformType::Ios, ipa));
    }

    #[test]
    fn an_app_bundle_is_an_artifact_not_a_package_directory() {
        assert!(is_app_bundle(Path::new("target/lingxia/ios/Demo.app")));
        assert!(!is_app_bundle(Path::new("dist/ios")));
    }
}
