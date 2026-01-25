use crate::platform::{self, InstallConfig};
use anyhow::Result;
use std::env;
use std::path::PathBuf;

/// Execute the install command
///
/// Installs the built application to a connected device.
/// Auto-detects the artifact if path is not provided.
pub fn execute(artifact: Option<String>, device: Option<String>) -> Result<()> {
    let project_root = env::current_dir()?;

    // Convert artifact path string to PathBuf if provided
    let artifact_path = artifact.map(PathBuf::from);

    let config = InstallConfig {
        project_root: project_root.clone(),
        artifact_path,
        device_id: device,
    };

    // Detect platform and install
    let platform = platform::detector::detect_platform(&project_root)?;
    platform.install(&config)?;

    Ok(())
}
