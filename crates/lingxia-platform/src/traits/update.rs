use std::path::Path;

use crate::error::PlatformError;

pub trait UpdateService: Send + Sync + 'static {
    /// Whether this platform installs host-app updates itself (download +
    /// in-place install/relaunch). Store-delivered platforms (iOS App Store,
    /// HarmonyOS AppGallery) return `false`: they must update through the
    /// store, so the update flow never downloads or self-installs there.
    ///
    /// Defaults to `false` — opt in per platform (macOS, Android).
    fn self_update_supported(&self) -> bool {
        false
    }

    /// Open the platform app-store page for this app so the user can update
    /// through the store. Used on store-delivered platforms when an update is
    /// available. Returns `true` if a store page was opened. Defaults to
    /// `false` (no in-app redirect; rely on the store's own update prompts).
    fn open_update_store(&self, _update_info_json: &str) -> Result<bool, PlatformError> {
        Ok(false)
    }

    /// Requests installation of an application update from a local package file.
    ///
    /// This starts the platform-specific apply flow and returns once the request
    /// is handed off to the updater helper.
    ///
    /// # Arguments
    /// * `package_path` - Local, readable update package path (e.g. .apk on Android)
    /// * `info_json` - Prompt metadata `{version, releaseNotes, isForceUpdate}`.
    ///   Release notes are shown in the "ready to update" prompt; when
    ///   `isForceUpdate` is true the prompt is blocking (no dismiss).
    ///
    /// # Platform Support / Notes
    /// - Android: Shows the post-download "ready to install" prompt (with
    ///   release notes), then launches the system installer on confirm.
    ///   Requires `REQUEST_INSTALL_PACKAGES` and a `FileProvider` for APK sharing.
    /// - macOS: Stages a prepared `.zip` or `.app` update, shows the
    ///   "ready to update" callout, and relaunches on the user's click.
    /// - iOS: Not supported (App Store only).
    /// - HarmonyOS: Not implemented (returns error).
    fn install_update(&self, package_path: &Path, info_json: &str) -> Result<(), PlatformError> {
        let _ = (package_path, info_json);
        Err(PlatformError::NotSupported(
            "install_update not implemented for this platform".to_string(),
        ))
    }
}
