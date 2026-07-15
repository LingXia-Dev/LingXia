use crate::error::PlatformError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Description of a top-level window belonging to the host app.
///
/// Returned by [`AppScreenshot::list_app_windows`]. The `id` is opaque
/// platform-specific (macOS NSWindow.windowNumber, Windows HWND, etc.) and
/// is the value to pass back to [`AppScreenshot::take_app_screenshot`] to
/// target that specific window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    /// Opaque platform-specific identifier (stringified for portability).
    pub id: String,
    /// Window title (may be empty if the platform / app does not set one).
    pub title: String,
    /// `true` if this window currently has keyboard focus / is "key".
    pub focused: bool,
    /// `true` if this window is the app's main window (macOS concept).
    pub main: bool,
    /// `true` if the window is currently on-screen / not minimized.
    pub visible: bool,
    /// Width in the platform's window-content coordinate unit (points on
    /// Apple platforms, client pixels on Windows).
    pub width: u32,
    /// Height in the platform's window-content coordinate unit.
    pub height: u32,
}

/// Capture a PNG snapshot of the host app's window(s).
///
/// Conceptually one level above `WebViewController::take_screenshot`: the
/// WebView API only sees web content, while this captures the entire window
/// the host renders — host-drawn navigation bars, native overlays, multiple
/// WebViews composited together, etc.
#[async_trait]
pub trait AppScreenshot: Send + Sync {
    /// Enumerate the app's top-level windows.
    ///
    /// Mobile platforms typically return a single "main" entry. Desktop
    /// platforms return one entry per open NSWindow / HWND.
    async fn list_app_windows(&self) -> Result<Vec<WindowInfo>, PlatformError> {
        Err(PlatformError::NotSupported(
            "list_app_windows is not implemented for this platform".to_string(),
        ))
    }

    /// Resolve an optional id using the same default-window policy as capture
    /// and input, returning the concrete target for automation metadata.
    async fn resolve_app_window(
        &self,
        window_id: Option<&str>,
    ) -> Result<WindowInfo, PlatformError> {
        let windows = self.list_app_windows().await?;
        if let Some(window_id) = window_id {
            return windows
                .into_iter()
                .find(|window| window.id == window_id)
                .ok_or_else(|| {
                    PlatformError::InvalidParameter(format!(
                        "window id does not belong to this app: {window_id}"
                    ))
                });
        }
        windows
            .iter()
            .find(|window| window.focused && window.visible)
            .or_else(|| windows.iter().find(|window| window.main && window.visible))
            .or_else(|| windows.iter().find(|window| window.visible))
            .or_else(|| windows.first())
            .cloned()
            .ok_or_else(|| PlatformError::Platform("no app window is available".to_string()))
    }

    /// Capture and return PNG-encoded bytes of the app's window.
    ///
    /// When `window_id` is `None`, the platform picks a sensible default
    /// (key/focused window on desktop; the sole window on mobile).
    async fn take_app_screenshot(&self, window_id: Option<&str>) -> Result<Vec<u8>, PlatformError> {
        let _ = window_id;
        Err(PlatformError::NotSupported(
            "app screenshot is not implemented for this platform".to_string(),
        ))
    }
}
