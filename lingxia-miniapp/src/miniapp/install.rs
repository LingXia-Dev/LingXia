use super::{LINGXIA_DIR, MINIAPPS_DIR, VERSIONS_DIR};
use crate::{AppController, MiniAppError};

// Check if a mini app is installed
pub(crate) fn is_installed<T: AppController + ?Sized>(_controller: &T, _app_id: &str) -> bool {
    // In debug builds, always return false to force reinstall
    #[cfg(debug_assertions)]
    {
        return false;
    }

    #[cfg(not(debug_assertions))]
    {
        let version_path = _controller
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(VERSIONS_DIR)
            .join(format!("{}.txt", _app_id));

        version_path.exists()
    }
}

// Copy files from assets to destination directory and update version
pub(crate) fn install_home_miniapp(
    controller: &dyn AppController,
    appid: &str,
    version: &str,
) -> Result<(), MiniAppError> {
    // Calculate base app directory and destination directory using appid
    // Note: Base directories are already created by prepare_directory_structure
    let base_dir = controller
        .app_data_dir()
        .join(LINGXIA_DIR)
        .join(MINIAPPS_DIR);
    let destination = base_dir.join(appid);

    // Create the app-specific destination directory
    std::fs::create_dir_all(&destination).map_err(|e| {
        MiniAppError::IoError(format!(
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
                MiniAppError::IoError(format!(
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
            MiniAppError::IoError(format!("Failed to read asset file {}: {}", entry.path, e))
        })?;

        std::fs::write(&dest_file_path, buffer).map_err(|e| {
            MiniAppError::IoError(format!(
                "Failed to write file {}: {}",
                dest_file_path.display(),
                e
            ))
        })?;
    }

    // Update version file AFTER successful file copy
    update_version(controller, appid, version)?;

    Ok(())
}

fn update_version(
    controller: &dyn AppController,
    appid: &str,
    new_version: &str,
) -> Result<(), MiniAppError> {
    let version_dir = controller
        .app_data_dir()
        .join(LINGXIA_DIR)
        .join(VERSIONS_DIR);

    let version_path = version_dir.join(format!("{}.txt", appid));

    // fs::write runcate then write
    std::fs::write(version_path, new_version)?;

    Ok(())
}
