use super::metadata::{self, LxAppRecord, ReleaseType, SemanticVersion};
use super::version::Version;
use super::{LINGXIA_DIR, LXAPPS_DIR};
use crate::{LxApp, LxAppError};
use lingxia_platform::{AppRuntime, Platform};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use zip::read::ZipArchive;

/// Check if a mini app is installed
///
/// # Arguments
/// * `appid` - ID of the mini app to check
/// * `release_type` - Target release channel
///
/// # Returns
/// * `true` - If the app is installed (metadata exists)
/// * `false` - If the app is not installed
pub(crate) fn is_installed(appid: &str, release_type: ReleaseType) -> bool {
    metadata::exists(appid, release_type).unwrap_or(false)
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
        let installed_version = metadata::get(&self.appid, self.release_type)
            .ok()
            .flatten()
            .map(|record| record.version.to_version())
            .or_else(|| Version::parse(&self.version).ok());

        let required_version = match Version::parse(required_version) {
            Ok(v) => v,
            Err(_) => return true,
        };

        match installed_version {
            Some(installed) => installed < required_version,
            None => true,
        }
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

    // Persist install metadata only after successful file copy
    update_metadata(appid, version, &destination, ReleaseType::Release)?;

    Ok(())
}

/// Install a standard lxapp bundle from a zip archive on disk.
///
/// # Arguments
/// * `controller` - App controller reference
/// * `appid` - ID of the mini app
/// * `release_type` - Target release channel
/// * `version` - Semantic version string
/// * `zip_path` - Absolute path to the source zip file
///
/// # Returns
/// * `Ok(PathBuf)` - Destination installation directory
#[allow(dead_code)]
pub(crate) fn install_from_zip(
    controller: Arc<Platform>,
    appid: &str,
    release_type: ReleaseType,
    version: &str,
    zip_path: &Path,
) -> Result<PathBuf, LxAppError> {
    let base_dir = controller.app_data_dir().join(LINGXIA_DIR).join(LXAPPS_DIR);
    let destination = match release_type {
        ReleaseType::Release => base_dir.join(appid),
        other => base_dir.join(appid).join(other.as_str()),
    };

    if destination.exists() {
        std::fs::remove_dir_all(&destination).map_err(|e| {
            LxAppError::IoError(format!(
                "Failed to clear existing install directory {}: {}",
                destination.display(),
                e
            ))
        })?;
    }

    std::fs::create_dir_all(&destination).map_err(|e| {
        LxAppError::IoError(format!(
            "Failed to create destination directory {}: {}",
            destination.display(),
            e
        ))
    })?;

    let file = File::open(zip_path).map_err(|e| {
        LxAppError::IoError(format!(
            "Failed to open zip archive {}: {}",
            zip_path.display(),
            e
        ))
    })?;

    let mut archive = ZipArchive::new(file).map_err(|e| {
        LxAppError::IoError(format!(
            "Failed to read zip archive {}: {}",
            zip_path.display(),
            e
        ))
    })?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| LxAppError::IoError(format!("Cannot read zip entry #{index}: {e}")))?;

        let entry_path = match entry.enclosed_name() {
            Some(path) => destination.join(path),
            None => continue,
        };

        if entry.name().ends_with('/') {
            std::fs::create_dir_all(&entry_path).map_err(|e| {
                LxAppError::IoError(format!(
                    "Failed to create directory {}: {}",
                    entry_path.display(),
                    e
                ))
            })?;
            continue;
        }

        if let Some(parent) = entry_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                LxAppError::IoError(format!(
                    "Failed to create directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        let mut outfile = File::create(&entry_path).map_err(|e| {
            LxAppError::IoError(format!(
                "Failed to create file {}: {}",
                entry_path.display(),
                e
            ))
        })?;

        std::io::copy(&mut entry, &mut outfile).map_err(|e| {
            LxAppError::IoError(format!("Failed to extract {}: {}", entry_path.display(), e))
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = entry.unix_mode() {
                if let Err(e) =
                    std::fs::set_permissions(&entry_path, std::fs::Permissions::from_mode(mode))
                {
                    return Err(LxAppError::IoError(format!(
                        "Failed to set permissions on {}: {}",
                        entry_path.display(),
                        e
                    )));
                }
            }
        }
    }

    update_metadata(appid, version, &destination, release_type)?;
    Ok(destination)
}

fn update_metadata(
    appid: &str,
    version: &str,
    install_path: &std::path::Path,
    release_type: ReleaseType,
) -> Result<(), LxAppError> {
    let installed_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default();

    let parsed_version = Version::parse(version).map_err(|_| {
        LxAppError::InvalidParameter(format!("Invalid semantic version: {}", version))
    })?;

    let record = LxAppRecord::new(
        appid,
        release_type,
        SemanticVersion::from_version(&parsed_version),
        install_path.to_string_lossy().to_string(),
        installed_at,
    );

    metadata::upsert(&record)
}
