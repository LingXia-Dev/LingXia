use std::io::Read;
use std::path::{Path, PathBuf};

use crate::AssetFileEntry;
use crate::error::PlatformError;

use super::PlatformFuture;
use super::clipboard::ClipboardService;
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

/// One local notification to post or replace.
#[derive(Debug, Clone)]
pub struct LocalNotificationShow {
    pub id: String,
    pub title: String,
    pub body: String,
    pub applink: Option<String>,
    pub deliver_at_ms: Option<u64>,
    pub silent: bool,
}

/// One button on a desktop banner.
#[derive(Debug, Clone)]
pub struct DesktopBannerAction {
    pub id: String,
    pub label: String,
    pub style: DesktopBannerActionStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopBannerActionStyle {
    Default,
    Primary,
    Destructive,
}

impl DesktopBannerActionStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Primary => "primary",
            Self::Destructive => "destructive",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "default" => Some(Self::Default),
            "primary" => Some(Self::Primary),
            "destructive" => Some(Self::Destructive),
            _ => None,
        }
    }
}

/// Card chrome. `System` follows the OS; a hex color is a solid fill.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DesktopBannerBackground {
    #[default]
    System,
    Light,
    Dark,
    Color {
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    },
}

/// One desktop banner to present. The caller waits until it resolves.
#[derive(Debug, Clone)]
pub struct DesktopBannerShow {
    pub id: String,
    pub title: String,
    pub body: String,
    pub actions: Vec<DesktopBannerAction>,
    /// `None` means wait until a button, dismiss, or replace.
    pub timeout_ms: Option<u64>,
    pub background: DesktopBannerBackground,
}

/// How a desktop banner finished. Failures to present are errors, not this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopBannerOutcome {
    Action { id: String, action: String },
    Dismissed { id: String },
    TimedOut { id: String },
    Replaced { id: String },
}

impl DesktopBannerOutcome {
    pub fn id(&self) -> &str {
        match self {
            Self::Action { id, .. }
            | Self::Dismissed { id }
            | Self::TimedOut { id }
            | Self::Replaced { id } => id,
        }
    }

    pub fn reason(&self) -> Option<&'static str> {
        match self {
            Self::Action { .. } => None,
            Self::Dismissed { .. } => Some("dismissed"),
            Self::TimedOut { .. } => Some("timeout"),
            Self::Replaced { .. } => Some("replaced"),
        }
    }
}

/// What `notification_show` did with the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalNotificationStatus {
    /// Handed to the OS for display now.
    Shown,
    /// Queued with the OS for `deliver_at_ms`.
    Scheduled,
    /// Immediate show while the product is frontmost: nothing was posted.
    Suppressed,
}

impl LocalNotificationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shown => "shown",
            Self::Scheduled => "scheduled",
            Self::Suppressed => "suppressed",
        }
    }

    /// Parse the status word a native bridge returned.
    pub fn from_native(value: &str) -> Option<Self> {
        match value {
            "shown" => Some(Self::Shown),
            "scheduled" => Some(Self::Scheduled),
            "suppressed" => Some(Self::Suppressed),
            _ => None,
        }
    }
}

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
    /// When true, the host should create the in-app tab before returning and
    /// report its id. Fire-and-forget callers (new-window, navigation) leave
    /// this false so the work can hop off a WebView UI thread.
    pub want_tab_id: bool,
}

/// Outcome of [`AppRuntime::open_url`]. `tab_id` is set when the host named
/// the tab it opened; `None` means the browser chrome owns the strip.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenUrlResult {
    pub tab_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinBrowserPage {
    Downloads = 1,
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
    + ClipboardService
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
    /// `webtag` is the page instance's full webview tag; page tags are
    /// per-instance, so shells must not reconstruct them from the route.
    /// Platforms whose containers resolve through the runtime by path may
    /// ignore it.
    fn show_lxapp(
        &self,
        appid: String,
        title: String,
        path: String,
        webtag: String,
        session_id: u64,
        open_mode: LxAppOpenMode,
        panel_id: String,
    ) -> Result<(), PlatformError>;

    /// Notify the desktop skin that the next layout publication is an explicit
    /// request to put this lxapp in front. Most skins reconcile directly from
    /// the layout plan; Windows uses the intent to replace a browser cover
    /// without treating unrelated resize/aside publications as activations.
    fn request_lxapp_main_activation(&self, _appid: &str) {}

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

    /// Replace the resolved shell sidebar action render list. Desktop skins only
    /// render presentation metadata and report stable ids.
    fn set_shell_sidebar_actions(
        &self,
        _items: &[lingxia_shell::ResolvedShellSidebarAction],
    ) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Replace the ordered mixed user Pin list. Platform skins resolve visual
    /// metadata only; target identity and the eight-item limit are shell-owned.
    fn set_shell_pins(&self, _items: &[lingxia_shell::ShellPin]) -> Result<(), PlatformError> {
        Ok(())
    }

    /// Show or hide the shell's "an AI assistant is in control" indicator.
    /// Its Stop button calls [`crate::request_control_session_stop`]. Desktop
    /// shells only; no-op elsewhere.
    fn set_control_session_indicator(&self, _active: bool) -> Result<(), PlatformError> {
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

    /// Current OS permission without prompting: `"granted"`, `"denied"`, or
    /// `"default"` (not yet asked).
    fn notification_permission(&self) -> Result<String, PlatformError> {
        Err(PlatformError::NotSupported("notification".to_string()))
    }

    /// Prompt where the OS has a prompt. `"granted"` or `"denied"`; an
    /// unanswered prompt is an error, never a status.
    fn notification_request_permission(&self) -> Result<String, PlatformError> {
        Err(PlatformError::NotSupported("notification".to_string()))
    }

    /// Upsert a local notification: anything pending or delivered under `id`
    /// is replaced first, on every path. `deliver_at_ms` is epoch
    /// milliseconds; `None` or a time that is not in the future means now.
    fn notification_show(
        &self,
        _request: &LocalNotificationShow,
    ) -> Result<LocalNotificationStatus, PlatformError> {
        Err(PlatformError::NotSupported("notification".to_string()))
    }

    fn notification_cancel(&self, _id: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported("notification".to_string()))
    }

    fn notification_cancel_all(&self) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported("notification".to_string()))
    }

    /// Present a desktop banner and block until it resolves. Desktop only.
    fn banner_show(
        &self,
        _request: &DesktopBannerShow,
    ) -> Result<DesktopBannerOutcome, PlatformError> {
        Err(PlatformError::NotSupported("banner".to_string()))
    }

    /// Dismiss a visible or queued banner. Unknown ids are fine.
    fn banner_dismiss(&self, _id: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported("banner".to_string()))
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
    /// `webtag` is the destination page instance's full webview tag; page
    /// tags are per-instance, so shells must not reconstruct them from the
    /// route. Platforms whose containers resolve through the runtime by path
    /// may ignore it.
    fn navigate(
        &self,
        appid: String,
        path: String,
        webtag: String,
        animation_type: AnimationType,
    ) -> Result<(), PlatformError>;

    /// Opens the given URL according to the host policy for the requested target.
    fn open_url(&self, req: OpenUrlRequest) -> Result<OpenUrlResult, PlatformError>;

    /// Close a tab previously named by [`Self::open_url`]. Platforms that
    /// cannot name tabs leave this unimplemented — the JS handle then reports
    /// `scope: 'group'` and never calls it.
    fn close_browser_tab(&self, _tab_id: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported("browser tab".to_string()))
    }

    /// Bring a tab previously named by [`Self::open_url`] to the front.
    fn activate_browser_tab(&self, _tab_id: String) -> PlatformFuture {
        Box::pin(async { Err(PlatformError::NotSupported("browser tab".to_string())) })
    }

    fn open_builtin_browser_page(&self, _page: BuiltinBrowserPage) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "built-in browser pages".to_string(),
        ))
    }
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
