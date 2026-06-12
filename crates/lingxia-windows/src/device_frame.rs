//! Runner-facing Windows device-frame facade.
//!
//! The simulator frame is host-runner appearance, not general app logic. Keep
//! the public model here so Windows runners depend on `lingxia-windows`
//! instead of reaching into the lower `lingxia::windows` facade directly.

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

pub use lingxia::windows::{
    WindowsAppMenu, WindowsAppMenuCommandHandler, WindowsAppMenuEntry, WindowsAppMenuItem,
    set_windows_app_menu, set_windows_app_menu_command_handler,
};

impl From<WindowsDeviceFrameToolbar> for lingxia::windows::WindowsDeviceFrameToolbar {
    fn from(value: WindowsDeviceFrameToolbar) -> Self {
        Self {
            selector_label: value.selector_label,
            selector_items: value.selector_items,
            action_command: value.action_command,
        }
    }
}

impl From<WindowsDeviceFrame> for lingxia::windows::WindowsDeviceFrame {
    fn from(value: WindowsDeviceFrame) -> Self {
        Self {
            screen_width: value.screen_width,
            screen_height: value.screen_height,
            bezel_width: value.bezel_width,
            outer_corner_radius: value.outer_corner_radius,
            screen_corner_radius: value.screen_corner_radius,
            bezel_color: value.bezel_color,
            toolbar: value.toolbar.map(Into::into),
        }
    }
}

/// Presents or clears a simulated-device frame around the top-level window
/// showing `appid`.
#[cfg(target_os = "windows")]
pub fn set_app_window_device_frame(
    appid: &str,
    frame: Option<WindowsDeviceFrame>,
) -> Result<(), String> {
    lingxia::windows::set_app_window_device_frame(appid, frame.map(Into::into))
}

/// Opens the WebView2 DevTools window for the current page of `appid`.
#[cfg(target_os = "windows")]
pub fn open_current_page_devtools(appid: &str) -> Result<(), String> {
    lingxia::windows::open_current_page_devtools(appid)
}
