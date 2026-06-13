//! Runner-facing Windows device-frame facade.
//!
//! The simulator frame is host-runner appearance, not general app logic. Keep
//! the public model here so Windows runners depend on `lingxia-windows`
//! instead of reaching into the lower `lingxia::windows` facade directly.

use crate::{WindowsAppMenuCommandHandler, WindowsAppMenuItem, app_menu};

mod native;

/// Toolbar model floating above a simulated device frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsDeviceFrameToolbar {
    /// Label shown on the selector, for example the current device name.
    pub selector_label: String,
    /// Drop-down items offered by the selector.
    pub selector_items: Vec<WindowsAppMenuItem>,
    /// Command id dispatched by the trailing action glyph, when present.
    pub action_command: Option<u32>,
}

/// Visual description of one simulated device, in physical pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsDeviceFrame {
    /// Screen content width.
    pub screen_width: i32,
    /// Screen content height.
    pub screen_height: i32,
    /// Bezel ring width around the screen.
    pub bezel_width: i32,
    /// Corner radius of the bezel's outer silhouette.
    pub outer_corner_radius: i32,
    /// Corner radius of the screen. `0` keeps square screen corners.
    pub screen_corner_radius: i32,
    /// Bezel fill color as `0xRRGGBB`.
    pub bezel_color: u32,
    /// Simulator toolbar floating above the device, when present.
    pub toolbar: Option<WindowsDeviceFrameToolbar>,
}

/// Presents or clears a simulated-device frame around the top-level window
/// showing `appid`.
#[cfg(target_os = "windows")]
pub fn set_app_window_device_frame(
    appid: &str,
    frame: Option<WindowsDeviceFrame>,
) -> Result<(), String> {
    let webview = current_page_webview(appid)?;
    let host_window = (frame.is_none())
        .then(|| crate::webview_host::find_webview_host_window(&webview.webtag()).ok())
        .flatten();
    native::set_webview_device_frame(&webview.webtag(), frame)?;
    if let Some(host_window) = host_window {
        app_menu::refresh_host_window_menu(host_window.window);
    }
    Ok(())
}

/// Applies a simulated-device frame to the next WebView host window created
/// by this process. Intended for runners that know their initial device
/// before the home lxapp is opened, so the first visible frame already has
/// the target shape.
#[cfg(target_os = "windows")]
pub fn set_initial_app_window_device_frame(frame: WindowsDeviceFrame) {
    native::set_initial_device_frame(frame);
}

pub(crate) fn set_device_frame_command_handler(handler: WindowsAppMenuCommandHandler) {
    native::set_device_frame_command_handler(handler);
}

/// Opens the WebView2 DevTools window for the current page of `appid`.
#[cfg(target_os = "windows")]
pub fn open_current_page_devtools(appid: &str) -> Result<(), String> {
    let webview = current_page_webview(appid)?;
    lingxia_webview::platform::windows::find_webview_handler(&webview.webtag())
        .ok_or_else(|| "page WebView handler is not ready".to_string())?
        .open_devtools()
        .map_err(|err| err.to_string())
}

#[cfg(target_os = "windows")]
fn current_page_webview(appid: &str) -> Result<std::sync::Arc<lingxia_webview::WebView>, String> {
    let app = lxapp::try_get(appid).ok_or_else(|| format!("lxapp is not active: {appid}"))?;
    let page = app.current_page().map_err(|err| err.to_string())?;
    page.webview()
        .ok_or_else(|| "page WebView is not ready".to_string())
}
