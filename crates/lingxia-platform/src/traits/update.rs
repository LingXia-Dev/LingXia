use std::path::Path;

use crate::error::PlatformError;

pub trait UpdateService: Send + Sync + 'static {
    /// Show download progress UI
    fn show_download_progress(&self) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Update download progress (0-100)
    fn update_download_progress(&self, _progress: i32) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Dismiss download progress UI
    fn dismiss_download_progress(&self) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Show update confirmation prompt and invoke callback with the result.
    ///
    /// # Arguments
    /// * `callback_id` - Callback ID for result
    /// * `update_info_json` - Optional JSON string with update details:
    ///   {"version":"1.2.0","size":15728640,"releaseNotes":["..."],"isForceUpdate":true}
    ///
    /// # Callback behavior
    /// - Confirm: callback success with payload (e.g. {"confirm":true})
    /// - Cancel: callback error code 2000
    fn show_update_prompt(
        &self,
        _callback_id: u64,
        _update_info_json: Option<&str>,
    ) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "show_update_prompt not implemented for this platform".to_string(),
        ))
    }

    /// Requests installation of an application update from a local package file.
    ///
    /// This launches the system installer and returns once the request is issued.
    /// It does not guarantee the update is installed successfully.
    ///
    /// # Arguments
    /// * `package_path` - Local, readable update package path (e.g. .apk on Android)
    ///
    /// # Platform Support / Notes
    /// - Android: Launches the system installer; requires user confirmation.
    ///   Requires `REQUEST_INSTALL_PACKAGES` and a `FileProvider` for APK sharing.
    /// - macOS: Planned support for .pkg/.dmg installers.
    /// - iOS: Not supported (App Store only).
    /// - HarmonyOS: Not implemented (returns error).
    fn install_update(&self, package_path: &Path) -> Result<(), PlatformError> {
        let _ = package_path;
        Err(PlatformError::NotSupported(
            "install_update not implemented for this platform".to_string(),
        ))
    }
}
