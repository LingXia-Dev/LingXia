use std::io::Read;
use std::path::{Path, PathBuf};

use crate::AssetFileEntry;
use crate::error::PlatformError;

use super::device::{Device, DeviceHardware};
use super::file::FileService;
use super::location::Location;
use super::media_interaction::{MediaInteraction, MediaKind};
use super::media_runtime::MediaRuntime;
use super::network::Network;
use super::secure_store::SecureStore;
use super::share::ShareService;
use super::ui::{SurfacePresenter, UIUpdate, UserFeedback};
use super::update::UpdateService;
use super::wifi::Wifi;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationType {
    None = 0,
    Forward = 1,
    Backward = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LxAppOpenMode {
    #[default]
    Normal = 0,
    Panel = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenUrlTarget {
    External = 0,
    SelfTarget = 1,
    /// Open a new browser tab unconditionally (skips "navigate current tab" heuristic).
    NewBrowserTab = 2,
    /// Open in the compact in-app browser as an API-managed aside tab. It uses
    /// the one-row toolbar without address editing or user tab creation.
    AsideBrowser = 3,
}

impl OpenUrlTarget {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.map(|v| v.trim().to_ascii_lowercase()) {
            Some(v) if v == "self" => Self::SelfTarget,
            Some(v) if v == "new_browser_tab" => Self::NewBrowserTab,
            Some(v) if v == "aside" => Self::AsideBrowser,
            Some(v) if v == "external" => Self::External,
            Some(v) => {
                log::warn!("Invalid openURL target='{}', fallback to external", v);
                Self::External
            }
            None => Self::External,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpenUrlRequest {
    pub owner_appid: String,
    pub owner_session_id: u64,
    pub url: String,
    pub target: OpenUrlTarget,
}

impl From<i32> for AnimationType {
    fn from(value: i32) -> Self {
        match value {
            1 => AnimationType::Forward,
            2 => AnimationType::Backward,
            _ => AnimationType::None,
        }
    }
}

pub trait AppRuntime:
    Send
    + Sync
    + MediaInteraction
    + MediaRuntime
    + Network
    + SurfacePresenter
    + Device
    + DeviceHardware
    + SecureStore
    + ShareService
    + FileService
    + Location
    + UIUpdate
    + UpdateService
    + UserFeedback
    + Wifi
    + 'static
{
    /// Reads an asset file as a streaming reader.
    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, PlatformError>;

    /// Iterates over files in an asset directory.
    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, PlatformError>> + 'a>;

    /// Returns the app's data directory path.
    fn app_data_dir(&self) -> PathBuf;

    /// Returns the app's cache directory path.
    fn app_cache_dir(&self) -> PathBuf;

    /// Obtains the application identifier.
    fn get_app_identifier(&self) -> Result<String, PlatformError>;

    /// Copies media from the system album to a local file.
    fn copy_album_media_to_file(
        &self,
        uri: &str,
        dest_path: &Path,
        kind: MediaKind,
    ) -> Result<(), PlatformError> {
        MediaRuntime::copy_album_media_to_file(self, uri, dest_path, kind)
    }

    /// Returns the current system locale.
    fn get_system_locale(&self) -> &str;

    /// Show the UI container for the given LxApp and route.
    fn show_lxapp(
        &self,
        appid: String,
        path: String,
        session_id: u64,
        open_mode: LxAppOpenMode,
        panel_id: String,
    ) -> Result<(), PlatformError>;

    /// Hide the UI container for the given LxApp (does not destroy its runtime state).
    fn hide_lxapp(&self, appid: String, session_id: u64) -> Result<(), PlatformError>;

    /// Exits the host app.
    fn exit(&self) -> Result<(), PlatformError>;

    // Tray / badge chrome. These are cosmetic enhancements, so platforms that
    // lack the chrome (e.g. no menu-bar tray on mobile) no-op rather than error —
    // portable code can call them unconditionally. A supporting platform returns
    // Err only on genuine failure.

    /// Set the tray (menu-bar / system-tray) badge. Desktop only; no-op elsewhere.
    fn set_tray_badge(&self, _text: &str) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Set the tray icon (a resource path). Desktop only; no-op elsewhere.
    fn set_tray_icon(&self, _icon: &str) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Replace the resolved shell activator render list. Desktop skins only
    /// render presentation metadata and report stable ids.
    fn set_shell_activators(
        &self,
        _items: &[lingxia_shell::ResolvedShellActivator],
    ) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Replace the ordered mixed user Pin list. Platform skins resolve visual
    /// metadata only; target identity and the eight-item limit are shell-owned.
    fn set_shell_pins(&self, _items: &[lingxia_shell::ShellPin]) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Set the tray title (text beside the icon, macOS). Desktop only; no-op elsewhere.
    fn set_tray_title(&self, _text: &str) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Set the app-icon badge: dock (macOS) / taskbar (Windows) / launcher icon
    /// (iOS, Android). No-op on platforms where it is not yet wired.
    fn set_app_badge(&self, _text: &str) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Whether the app is registered to launch at system startup. Only reached
    /// on macOS/Windows — `lx.app.autostart` is not registered elsewhere — so
    /// the default is an error, not a no-op: a false answer here would be a lie.
    fn autostart_is_enabled(&self) -> Result<bool, PlatformError> {
        Err(PlatformError::NotSupported("autostart".to_string()))
    }

    /// Register or unregister the app as a per-user startup item.
    fn autostart_set_enabled(&self, _enabled: bool) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported("autostart".to_string()))
    }

    /// Replace the tray dropdown menu. `items_json` is a JSON array of
    /// `{ label?, separator?, enabled?, checked? }`. Item clicks are delivered
    /// back to JS by index. Desktop only; no-op elsewhere.
    fn set_tray_menu(&self, _items_json: &str) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Show or hide the tray status item itself. Desktop only; no-op elsewhere.
    fn set_tray_visible(&self, _visible: bool) -> Result<(), PlatformError> {
        Ok(())
    }

    /// When intercepting, a left-click on the tray is delivered only to JS
    /// (`lx.tray.onClick`) and does not run the tray's configured surface action.
    /// Desktop only; no-op elsewhere.
    fn set_tray_click_intercept(&self, _intercept: bool) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Navigates within the given LxApp using an animation.
    fn navigate(
        &self,
        appid: String,
        path: String,
        animation_type: AnimationType,
    ) -> Result<(), PlatformError>;

    /// Opens the given URL according to the host policy for the requested target.
    fn open_url(&self, req: OpenUrlRequest) -> Result<(), PlatformError>;

    /// Gets the capsule button bounding rect in screen coordinates.
    /// Returns JSON: {"width": f64, "height": f64, "top": f64, "right": f64, "bottom": f64, "left": f64}
    fn get_capsule_rect(
        &self,
    ) -> impl std::future::Future<Output = Result<String, PlatformError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::OpenUrlTarget;

    #[test]
    fn parse_supports_new_browser_tab() {
        assert_eq!(
            OpenUrlTarget::parse(Some("new_browser_tab")),
            OpenUrlTarget::NewBrowserTab
        );
    }

    #[test]
    fn parse_unknown_falls_back_to_external() {
        assert_eq!(
            OpenUrlTarget::parse(Some("foobar")),
            OpenUrlTarget::External
        );
    }
}
