//! Runner-facing Windows device-frame facade.
//!
//! The simulator frame is host-runner appearance, not general app logic. Keep
//! the public model here so Windows runners depend on `lingxia-windows-sdk`
//! instead of reaching into the lower `lingxia::windows` facade directly.

#[cfg(feature = "shell-chrome")]
use crate::shell::WindowsShellTabBarPosition;
use crate::{WindowsAppMenuCommandHandler, WindowsAppMenuItem, WindowsDesignIcon, app_menu};
pub use lingxia_webview::platform::windows::WindowsBrowserEmulationProfile;

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
    /// Command id dispatched by the rotate glyph (portrait/landscape toggle),
    /// when present. Drawn just left of the action glyph.
    pub rotate_command: Option<u32>,
    /// Items for the floating capsule's menu button. The dev runner uses this
    /// as a single About/info-sheet command. Empty hides the menu button.
    pub capsule_items: Vec<WindowsAppMenuItem>,
    /// Command id dispatched by the capsule's close (right) circle. The caller
    /// owns the meaning — the dev runner maps it to "quit the emulator". `None`
    /// leaves the circle inert.
    pub capsule_close_command: Option<u32>,
    /// The toolbar draws macOS-style close/minimize dots and therefore owns
    /// the window controls; the shell suppresses its own caption buttons.
    /// `false` (e.g. a simulated desktop) keeps the standard Windows
    /// min/max/close in the shell chrome and draws no dots.
    pub window_dots: bool,
}

/// Optional physical screen cutout rendered over the simulated screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsDeviceFrameCutout {
    pub width: i32,
    pub height: i32,
    pub corner_radius: i32,
}

/// Simulated iOS status bar across the top of the device screen: the time on
/// the leading edge and the signal/battery glyphs on the trailing edge,
/// flanking the cutout. The device frame draws it as an overlay above the
/// WebView2 content (the app's top safe-area sits beneath it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsDeviceFrameStatusBar {
    /// Status bar height in pixels (the device's top safe-area inset).
    pub height: i32,
    /// Text/glyph color as `0xRRGGBB`, chosen to contrast `background`. The
    /// device frame draws the real current time on the leading edge.
    pub foreground: u32,
    /// Opaque strip fill as `0xRRGGBB`. The shell sets this to the page's
    /// navigation-bar color so the bar color extends up over the status bar
    /// (matching the macOS runner), or the chrome color for a plain page.
    /// Ignored when `transparent` is set.
    pub background: u32,
    /// When set, the strip is painted with no fill — only the clock + indicators
    /// float over the WebView content (premultiplied alpha). Used for immersive
    /// (custom navigation-style) pages whose content bleeds up under the bar.
    pub transparent: bool,
}

/// A colored pill drawn at the trailing edge of the info-sheet header (for
/// example a release-channel marker). The caller owns the text and colors so
/// the device frame stays free of app/runner-specific semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsDeviceFrameBadge {
    pub text: String,
    /// Text color as `0xRRGGBB`.
    pub foreground: u32,
    /// Pill fill color as `0xRRGGBB`.
    pub background: u32,
}

/// One action row in the info sheet. The caller supplies both the icon and the
/// command id dispatched to the device-frame command handler when it is tapped,
/// so the device frame never infers either from the label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsDeviceFrameSheetAction {
    pub command: u32,
    pub label: String,
    pub icon: WindowsDesignIcon,
}

/// Generic info displayed by a device-frame bottom sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsDeviceFrameInfoSheet {
    pub title: String,
    pub version: String,
    /// Optional trailing header badge (e.g. the release channel).
    pub badge: Option<WindowsDeviceFrameBadge>,
    /// Action rows. The device frame owns presentation only; command ids are
    /// dispatched to the runner's device-frame command handler.
    pub actions: Vec<WindowsDeviceFrameSheetAction>,
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
    /// Optional top-centered screen cutout / Dynamic Island. `None` or empty
    /// dimensions leave the screen uninterrupted.
    pub cutout: Option<WindowsDeviceFrameCutout>,
    /// Optional simulated status bar (time + signal/battery) across the top.
    /// `None` leaves the screen's top edge bare.
    pub status_bar: Option<WindowsDeviceFrameStatusBar>,
    /// Bezel fill color as `0xRRGGBB`.
    pub bezel_color: u32,
    /// Fill color of the anti-aliased bezel mask outside the rounded screen,
    /// as `0xRRGGBB`. This normally matches `bezel_color` so the smooth overlay
    /// edge blends into the frame around the clipped content window.
    pub screen_corner_color: u32,
    /// Simulator toolbar floating above the device, when present.
    pub toolbar: Option<WindowsDeviceFrameToolbar>,
}

/// Presents or clears a simulated-device frame around the top-level window
/// showing `appid`.
pub fn set_app_window_device_frame(
    appid: &str,
    frame: Option<WindowsDeviceFrame>,
) -> Result<(), String> {
    let webview = current_page_webview(appid)?;
    let host_window = (frame.is_none())
        .then(|| crate::window_host::find_host_window_for_webview(&webview.webtag()).ok())
        .flatten();
    native::set_webview_device_frame(&webview.webtag(), frame)?;
    if let Some(host_window) = host_window {
        app_menu::refresh_host_window_menu(host_window.window);
    }
    Ok(())
}

/// Applies a simulated-device frame and shell tabbar position as one UI-thread
/// transaction. The Windows runner uses this for device switches so layout
/// does not briefly sync against the previous device frame.
#[cfg(feature = "shell-chrome")]
pub fn set_app_window_device_frame_and_tabbar_position(
    appid: &str,
    frame: WindowsDeviceFrame,
    tabbar_position: WindowsShellTabBarPosition,
) -> Result<(), String> {
    let webview = current_page_webview(appid)?;
    native::set_webview_device_frame_and_tabbar_position(
        &webview.webtag(),
        appid.to_string(),
        frame,
        tabbar_position,
    )
}

#[cfg(all(feature = "browser-runtime", feature = "shell-chrome"))]
pub(crate) fn set_browser_device_frame_and_tabbar_position(
    frame: WindowsDeviceFrame,
    tabbar_position: WindowsShellTabBarPosition,
) -> Result<(), String> {
    let webtag = current_browser_webtag()?;
    native::set_webview_device_frame_and_tabbar_position(
        &webtag,
        lingxia_browser::BUILTIN_BROWSER_APPID.to_string(),
        frame,
        tabbar_position,
    )
}

/// Applies a simulated-device frame to the next WebView host window created
/// by this process. Intended for runners that know their initial device
/// before the home lxapp is opened, so the first visible frame already has
/// the target shape.
pub fn set_initial_app_window_device_frame(frame: WindowsDeviceFrame) {
    native::set_initial_device_frame(frame);
}

pub(crate) fn set_device_frame_command_handler(handler: WindowsAppMenuCommandHandler) {
    native::set_device_frame_command_handler(handler);
}

/// True while the top-level window `window` is wrapped in a simulator device
/// frame (the runner). The shell drops its caption/app-menu on a framed screen.
/// Only the shell chrome reads this, so it's dead code without `shell-chrome`.
#[cfg_attr(not(feature = "shell-chrome"), allow(dead_code))]
pub(crate) fn window_has_device_frame(window: isize) -> bool {
    native::window_has_frame(window)
}

/// Screen corner radius + corner style for a framed content window (`None`
/// when unframed), used by the webview composition corner visuals.
pub(crate) fn device_frame_screen_clip_style(window: isize) -> Option<(i32, u32)> {
    native::content_screen_clip_style(window)
}

/// Work-area fit factor of a framed content window (`None` when unframed),
/// mirrored into the webview's rasterization scale and the native-overlay
/// coordinate mapping.
pub(crate) fn device_frame_fit_scale(window: isize) -> Option<f64> {
    native::content_fit_scale(window)
}

/// True while `window`'s device frame owns the window controls (macOS-style
/// close/minimize dots on its toolbar). The shell suppresses its caption
/// buttons only then — a framed simulated desktop keeps the standard Windows
/// min/max/close in the shell chrome.
#[cfg_attr(not(feature = "shell-chrome"), allow(dead_code))]
pub(crate) fn device_frame_owns_window_controls(window: isize) -> bool {
    native::frame_owns_window_controls(window)
}

/// Height of the simulated status bar for `window`, so the shell can reserve a
/// top inset and stack its nav bar + content below the status bar overlay.
#[cfg_attr(not(feature = "shell-chrome"), allow(dead_code))]
pub(crate) fn device_frame_status_bar_height(window: isize) -> i32 {
    native::status_bar_height(window)
}

/// Sets the simulated status bar's foreground + background colors for `window`
/// (on its UI thread). The shell drives this from the active page's navigation
/// bar so the bar color covers the status bar and the time/signal stay legible.
#[cfg_attr(not(feature = "shell-chrome"), allow(dead_code))]
pub(crate) fn set_device_frame_status_bar_style(
    window: isize,
    foreground: u32,
    background: u32,
    transparent: bool,
) {
    let _ = crate::window_host::post_to_window_thread(
        window,
        Box::new(move || native::set_status_bar_style(window, foreground, background, transparent)),
    );
}

pub(crate) fn set_device_frame_overlays_visible(window: isize, visible: bool) {
    native::set_frame_overlays_visible(window, visible);
}

pub(crate) use native::capsule_reserve_width;

/// Shows a generic bottom sheet over the active framed window for `appid`.
/// The caller owns the meaning of the text; the device frame only owns the
/// presentation.
pub fn show_device_frame_info_sheet(
    appid: &str,
    info: WindowsDeviceFrameInfoSheet,
) -> Result<(), String> {
    let webview = current_page_webview(appid)?;
    let host_window = crate::window_host::find_host_window_for_webview(&webview.webtag())
        .map_err(|err| err.to_string())?;
    let window = host_window.window;
    let info = native::DeviceFrameInfoSheet {
        title: info.title,
        version: info.version,
        badge: info.badge.map(|badge| native::InfoSheetBadge {
            text: badge.text,
            foreground: badge.foreground,
            background: badge.background,
        }),
        actions: info
            .actions
            .into_iter()
            .map(|action| native::SheetAction {
                label: action.label,
                command: action.command,
                icon: action.icon,
            })
            .collect(),
    };
    let posted = crate::window_host::post_to_window_thread(
        window,
        Box::new(move || native::show_info_sheet(window, info)),
    );
    if posted {
        Ok(())
    } else {
        Err("about sheet target window is not accepting messages".to_string())
    }
}

/// Opens the WebView2 DevTools window for the current page of `appid`.
pub fn open_current_page_devtools(appid: &str) -> Result<(), String> {
    let webview = current_page_webview(appid)?;
    lingxia_webview::platform::windows::find_webview_handler(&webview.webtag())
        .ok_or_else(|| "page WebView handler is not ready".to_string())?
        .open_devtools()
        .map_err(|err| err.to_string())
}

#[cfg(feature = "browser-runtime")]
pub(crate) fn open_browser_devtools() -> Result<(), String> {
    let webtag = current_browser_webtag()?;
    lingxia_webview::platform::windows::find_webview_handler(&webtag)
        .ok_or_else(|| "browser WebView handler is not ready".to_string())?
        .open_devtools()
        .map_err(|err| err.to_string())
}

#[cfg(feature = "browser-runtime")]
fn current_browser_webtag() -> Result<lingxia_webview::WebTag, String> {
    let tab =
        lingxia_browser::current_tab().ok_or_else(|| "browser has no active tab".to_string())?;
    Ok(lingxia_webview::WebTag::new(
        lingxia_browser::BUILTIN_BROWSER_APPID,
        &tab.path,
        Some(tab.session_id),
    ))
}

/// Applies the simulated browser form factor to new and existing WebViews.
///
/// Call this before creating the first WebView so WebView2's original UA Client
/// Hints can be captured for later desktop restoration. Existing pages created
/// under that configuration reload only when requested, so callers can switch
/// within one form-factor family without losing page state.
pub fn set_windows_browser_emulation_profile(
    profile: WindowsBrowserEmulationProfile,
    reload_existing: bool,
) -> Result<(), String> {
    lingxia_webview::platform::windows::set_windows_browser_emulation_profile_for_new_webviews(
        profile,
    );
    let mut failures = Vec::new();
    for webtag in lingxia_webview::runtime::list_webviews() {
        let Some(handler) = lingxia_webview::platform::windows::find_webview_handler(&webtag)
        else {
            continue;
        };
        if let Err(err) = handler.set_browser_emulation_profile(profile, reload_existing) {
            failures.push(format!("{}: {err}", webtag.key()));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

fn current_page_webview(appid: &str) -> Result<std::sync::Arc<lingxia_webview::WebView>, String> {
    let app = lxapp::try_get(appid).ok_or_else(|| format!("lxapp is not active: {appid}"))?;
    let page = app.current_page().map_err(|err| err.to_string())?;
    page.webview()
        .ok_or_else(|| "page WebView is not ready".to_string())
}
