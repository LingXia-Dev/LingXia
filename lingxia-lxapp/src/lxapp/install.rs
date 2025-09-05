use super::version::Version;
use super::{LINGXIA_DIR, LXAPPS_DIR, VERSIONS_DIR};
use crate::{LxApp, LxAppError};
use lingxia_platform::{Platform, AppRuntime};
use std::sync::Arc;

/// Check if a mini app is installed
///
/// # Arguments
/// * `controller` - App controller reference
/// * `appid` - ID of the mini app to check
///
/// # Returns
/// * `true` - If the app is installed (version file exists)
/// * `false` - If the app is not installed
pub(crate) fn is_installed(controller: Arc<Platform>, appid: &str) -> bool {
    let version_path = controller
        .app_data_dir()
        .join(LINGXIA_DIR)
        .join(VERSIONS_DIR)
        .join(format!("{}.txt", appid));

    // Check if version file exists
    version_path.exists()
}

impl LxApp {
    /// Check if this mini app needs to be updated to the specified version
    ///
    /// # Arguments
    /// * `required_version` - Version required (formatted as major.minor.patch)
    ///
    /// # Returns
    /// * `true` - If the app needs to be updated
    /// * `false` - If the app already has an equal or newer version
    pub(crate) fn should_update(&self, required_version: &str) -> bool {
        let installed_version = self.read_version();

        // Parse both versions
        if let (Ok(installed_version), Ok(required_version)) = (
            Version::parse(&installed_version),
            Version::parse(required_version),
        ) {
            // Compare versions - return true if installed is older than required
            return installed_version < required_version;
        }

        // Failed to read or parse version, treat as needing update
        true
    }
}

// Copy files from assets to destination directory and update version
pub(crate) fn install_home_lxapp(
    controller: Arc<Platform>,
    appid: &str,
    version: &str,
) -> Result<(), LxAppError> {
    // Calculate base app directory and destination directory using appid
    // Note: Base directories are already created by prepare_directory_structure
    let base_dir = controller.app_data_dir().join(LINGXIA_DIR).join(LXAPPS_DIR);
    let destination = base_dir.join(appid);

    // Delete the existing app directory if it exists to ensure no old files remain
    if destination.exists() {
        let _ = std::fs::remove_dir_all(&destination);
    }

    // Create the app-specific destination directory
    std::fs::create_dir_all(&destination).map_err(|e| {
        LxAppError::IoError(format!(
            "Failed to create app directory {}: {}",
            destination.display(),
            e
        ))
    })?;

    // Create an iterator over all files in the asset directory
    let entries = controller.asset_dir_iter(appid);

    // Process each entry (file) in the asset directory
    for entry_result in entries {
        let entry = entry_result?;

        // Extract the relative path within the app's asset directory
        let rel_path = entry
            .path
            .strip_prefix(&format!("{}/", appid))
            .unwrap_or(&entry.path);

        // Construct the destination file path
        let dest_file_path = destination.join(rel_path);

        // Create parent directories if needed
        if let Some(parent) = dest_file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                LxAppError::IoError(format!(
                    "Failed to create directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        // Copy the file content
        let mut reader = entry.reader;
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).map_err(|e| {
            LxAppError::IoError(format!("Failed to read asset file {}: {}", entry.path, e))
        })?;

        std::fs::write(&dest_file_path, buffer).map_err(|e| {
            LxAppError::IoError(format!(
                "Failed to write file {}: {}",
                dest_file_path.display(),
                e
            ))
        })?;
    }

    // Update version file AFTER successful file copy
    update_version(controller.clone(), appid, version)?;

    Ok(())
}

fn update_version(controller: Arc<Platform>, appid: &str, new_version: &str) -> Result<(), LxAppError> {
    let version_dir = controller
        .app_data_dir()
        .join(LINGXIA_DIR)
        .join(VERSIONS_DIR);

    let version_path = version_dir.join(format!("{}.txt", appid));

    // fs::write truncate then write
    std::fs::write(version_path, new_version)?;

    Ok(())
}
