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

    /// Whether this process was installed by a platform store (Play, App
    /// Store, MAS, Microsoft Store, AppGallery). Store-installed binaries
    /// never self-update, even when this build's yaml still says `direct`.
    fn installed_from_store(&self) -> bool {
        false
    }

    /// Open the platform app-store page for this app so the user can update
    /// through the store. Used on store-delivered platforms when an update is
    /// available. Returns `true` if a store page was opened. Defaults to
    /// `false` (no in-app redirect; rely on the store's own update prompts).
    fn open_update_store(&self, _update_info_json: &str) -> Result<bool, PlatformError> {
        Ok(false)
    }

    /// Show a "new version — open the store" prompt (card / callout / tray /
    /// alert). Confirm opens the store; the package is never downloaded.
    /// Returns `true` when a UI was presented.
    fn present_store_update(&self, _update_info_json: &str) -> Result<bool, PlatformError> {
        Ok(false)
    }

    /// Requests installation of an application update from a local package file.
    ///
    /// This starts the platform-specific apply flow and returns once the request
    /// is handed off to the updater helper.
    ///
    /// # Arguments
    /// * `package_path` - Local, readable update package path (e.g. .apk on Android)
    /// * `info_json` - Prompt metadata `{version, releaseNotes}` shown in the
    ///   dismissible "ready to update" prompt.
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

/// `storeUrl` from the store-channel prompt JSON.
pub fn store_url_in_update_info(info_json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(info_json)
        .ok()
        .and_then(|value| {
            value
                .get("storeUrl")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
}
