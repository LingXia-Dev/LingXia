use crate::{AppController, MiniAppError};
use std::path::Path;

// Copy files from assets to destination directory
pub(crate) fn install_home_miniapp(
    controller: &dyn AppController,
    appid: &str,
    destination: &Path,
) -> Result<(), MiniAppError> {
    // Create an iterator over all files in the asset directory
    let entries = controller.asset_dir_iter(appid);

    // Process each entry (file) in the asset directory
    for entry_result in entries {
        let entry = entry_result?;
        let rel_path = entry
            .path
            .strip_prefix(&format!("{}/", appid))
            .unwrap_or(&entry.path);

        let dest_file_path = destination.join(rel_path);

        // Create parent directories if they don't exist
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

    Ok(())
}
