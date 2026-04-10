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

    // Convert artifact path string to PathBuf if provided
    let artifact_path = artifact.map(PathBuf::from);

    // Detect platform from argument, artifact extension, or project structure
    let platform_type = if let Some(p) = platform_arg {
        p.parse::<PlatformType>()?
    } else {
        detect_platform_from_artifact(artifact_path.as_deref(), &project_root)?
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

/// Detect platform from artifact file extension or project structure
fn detect_platform_from_artifact(
    artifact: Option<&Path>,
    project_root: &Path,
) -> Result<PlatformType> {
    // First check artifact extension
    if let Some(ext) = artifact.and_then(|p| p.extension()) {
        let ext_str = ext.to_string_lossy().to_lowercase();
        match ext_str.as_str() {
            "apk" => return Ok(PlatformType::Android),
            "app" | "ipa" => return Ok(PlatformType::Ios),
            "hap" => return Ok(PlatformType::Harmony),
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
