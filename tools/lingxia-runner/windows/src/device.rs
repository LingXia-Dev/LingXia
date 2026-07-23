use lingxia_windows_sdk::{
    WindowsAppMenuItem, WindowsBrowserEmulationProfile, WindowsDeviceFrame,
    WindowsDeviceFrameCutout, WindowsDeviceFrameStatusBar, WindowsDeviceFrameToolbar,
};
use serde::Deserialize;
use std::sync::OnceLock;

pub(crate) const DEVICE_COMMAND_BASE: u32 = 0x0100;

pub(crate) const OPEN_DEVTOOLS_COMMAND: u32 = 0x0200;

/// Toggles the current device between portrait and landscape.
pub(crate) const ROTATE_COMMAND: u32 = 0x0300;

/// Restarts the hosted lxapp (re-opens its initial route).
pub(crate) const RESTART_LXAPP_COMMAND: u32 = 0x0500;

/// Clears the hosted lxapp's cache, then restarts it.
pub(crate) const CLEAN_CACHE_COMMAND: u32 = 0x0600;

/// Shows the lxapp info (name + version).
pub(crate) const ABOUT_COMMAND: u32 = 0x0700;

/// Capsule close circle: dispatches the lxapp capsule close event.
pub(crate) const CAPSULE_CLOSE_COMMAND: u32 = 0x0800;

/// The selector dropdown only chooses the simulated frame/device.
fn device_selector_items(index: usize) -> Vec<WindowsAppMenuItem> {
    presets()
        .iter()
        .enumerate()
        .map(|(item_index, item)| {
            WindowsAppMenuItem::new(DEVICE_COMMAND_BASE + item_index as u32, device_label(item))
                .checked(item_index == index)
        })
        .collect()
}

/// The floating capsule's menu button opens the app-info bottom sheet.
fn capsule_menu_items() -> Vec<WindowsAppMenuItem> {
    vec![WindowsAppMenuItem::new(ABOUT_COMMAND, "About")]
}

#[derive(Debug, Deserialize)]
struct RunnerDevices {
    default: String,
    devices: Vec<DevicePreset>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DevicePreset {
    id: String,
    group: String,
    #[serde(rename = "browserProfile")]
    browser_profile: BrowserProfile,
    pub(crate) name: String,
    pub(crate) width: i32,
    pub(crate) height: i32,
    #[serde(rename = "bezelWidth")]
    bezel_width: i32,
    #[serde(rename = "bezelColor")]
    bezel_color: u32,
    #[serde(rename = "outerRadius")]
    outer_radius: i32,
    #[serde(rename = "screenRadius")]
    screen_radius: i32,
    notch: DeviceNotch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum BrowserProfile {
    Desktop,
    Phone,
    Tablet,
}

#[derive(Debug, Deserialize)]
struct DeviceNotch {
    width: i32,
    height: i32,
    #[serde(rename = "cornerRadius")]
    corner_radius: f32,
    #[serde(rename = "statusBarHeight")]
    status_bar_height: i32,
}

impl DevicePreset {
    /// Stable preset id from `devices.json` (e.g. "iphone-15-pro").
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    /// Form-factor group ("phone" | "tablet" | "desktop").
    pub(crate) fn group(&self) -> &str {
        &self.group
    }

    pub(crate) fn browser_profile(&self) -> WindowsBrowserEmulationProfile {
        match self.browser_profile {
            BrowserProfile::Desktop => WindowsBrowserEmulationProfile::Desktop,
            BrowserProfile::Phone => WindowsBrowserEmulationProfile::Phone,
            BrowserProfile::Tablet => WindowsBrowserEmulationProfile::Tablet,
        }
    }
}

fn runner_devices() -> &'static RunnerDevices {
    static DEVICES: OnceLock<RunnerDevices> = OnceLock::new();
    DEVICES.get_or_init(|| {
        serde_json::from_str(include_str!("../../devices.json"))
            .expect("runner devices.json must be valid")
    })
}

pub(crate) fn presets() -> &'static [DevicePreset] {
    &runner_devices().devices
}

pub(crate) fn default_device_index() -> usize {
    let devices = runner_devices();
    devices
        .devices
        .iter()
        .position(|preset| preset.id == devices.default)
        .unwrap_or(0)
}

/// Startup device: `LINGXIA_RUNNER_DEVICE` (the CLI `--runner` contract,
/// shared with the macOS runner) when it names a known device id, else the
/// manifest default.
pub(crate) fn initial_device_index() -> usize {
    std::env::var("LINGXIA_RUNNER_DEVICE")
        .ok()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .and_then(|id| presets().iter().position(|preset| preset.id == id))
        .unwrap_or_else(default_device_index)
}

pub(crate) fn is_tablet(index: usize) -> bool {
    presets()
        .get(index)
        .is_some_and(|preset| preset.group == "tablet")
}

pub(crate) fn is_phone(index: usize) -> bool {
    presets()
        .get(index)
        .is_some_and(|preset| preset.group == "phone")
}

pub(crate) fn device_label(preset: &DevicePreset) -> String {
    format!("{}\t{} x {}", preset.name, preset.width, preset.height)
}

/// Status-bar text/glyph color, contrasting the shell chrome it sits over so
/// the time + signal stay legible in both light and dark themes.
fn status_bar_foreground() -> u32 {
    let bg = lingxia_windows_sdk::windows_shell_background_color();
    let luminance =
        (((bg >> 16) & 0xff) * 299 + ((bg >> 8) & 0xff) * 587 + (bg & 0xff) * 114) / 1000;
    if luminance > 140 {
        0x1C_1C1E
    } else {
        0xF2_F2F7
    }
}

fn visual_bezel_width(preset: &DevicePreset) -> i32 {
    preset.bezel_width.max(0)
}

pub(crate) fn frame_spec(index: usize, landscape: bool) -> WindowsDeviceFrame {
    let preset = &presets()[index];
    // Landscape swaps the screen's long and short edges; the bezel, radii, and
    // toolbar follow the new width automatically.
    let (screen_width, screen_height) = if landscape {
        (preset.height, preset.width)
    } else {
        (preset.width, preset.height)
    };
    WindowsDeviceFrame {
        screen_width,
        screen_height,
        bezel_width: visual_bezel_width(preset),
        outer_corner_radius: preset.outer_radius,
        screen_corner_radius: preset.screen_radius,
        cutout: (!landscape && preset.notch.width > 0 && preset.notch.height > 0).then(|| {
            WindowsDeviceFrameCutout {
                width: preset.notch.width,
                height: preset.notch.height,
                corner_radius: preset.notch.corner_radius.round() as i32,
            }
        }),
        status_bar: (is_phone(index) && !landscape && preset.notch.status_bar_height > 0).then(
            || WindowsDeviceFrameStatusBar {
                height: preset.notch.status_bar_height,
                // Initial colors + opacity; the shell overrides these per page
                // from the active page's navigation-bar style (and switches the
                // strip transparent for immersive custom-navigation pages). The
                // real current time is drawn by the device frame.
                foreground: status_bar_foreground(),
                background: lingxia_windows_sdk::windows_shell_background_color(),
                transparent: false,
            },
        ),
        bezel_color: preset.bezel_color,
        // Keep clipped screen corners backed by the device shell instead of
        // exposing the desktop through the rounded edge.
        screen_corner_color: preset.bezel_color,
        toolbar: Some(WindowsDeviceFrameToolbar {
            selector_label: preset.name.clone(),
            selector_items: device_selector_items(index),
            action_command: Some(OPEN_DEVTOOLS_COMMAND),
            rotate_command: Some(ROTATE_COMMAND),
            capsule_items: if is_phone(index) {
                capsule_menu_items()
            } else {
                Vec::new()
            },
            capsule_close_command: is_phone(index).then_some(CAPSULE_CLOSE_COMMAND),
            // Phones/tablets are handheld mockups: the toolbar's macOS-style
            // dots own close/minimize. A simulated desktop keeps the standard
            // Windows caption buttons in the shell chrome instead.
            window_dots: is_phone(index) || is_tablet(index),
        }),
    }
}

/// Browser chrome already owns dismissal and tab actions. Keep the simulated
/// device selector/rotation controls, but do not overlay lxapp capsule actions.
pub(crate) fn browser_frame_spec(index: usize, landscape: bool) -> WindowsDeviceFrame {
    let mut frame = frame_spec(index, landscape);
    if let Some(toolbar) = frame.toolbar.as_mut() {
        toolbar.capsule_items.clear();
        toolbar.capsule_close_command = None;
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_spec_uses_manifest_device_shell() {
        for (index, preset) in presets().iter().enumerate() {
            let frame = frame_spec(index, false);
            assert_eq!(
                frame.bezel_width,
                preset.bezel_width.max(0),
                "{}",
                preset.id
            );
            assert_eq!(
                frame.outer_corner_radius, preset.outer_radius,
                "{}",
                preset.id
            );
            assert_eq!(
                frame.screen_corner_radius, preset.screen_radius,
                "{}",
                preset.id
            );
        }
    }

    #[test]
    fn rotating_keeps_the_device_shell_and_swaps_the_screen() {
        let index = default_device_index();
        let preset = &presets()[index];
        let frame = frame_spec(index, true);

        assert_eq!(frame.screen_width, preset.height);
        assert_eq!(frame.screen_height, preset.width);
        assert_eq!(frame.bezel_width, preset.bezel_width.max(0));
        assert_eq!(frame.outer_corner_radius, preset.outer_radius);
    }

    #[test]
    fn browser_frame_has_no_lxapp_capsule() {
        let frame = browser_frame_spec(default_device_index(), false);
        let toolbar = frame.toolbar.expect("runner frame has a toolbar");
        assert!(toolbar.capsule_items.is_empty());
        assert_eq!(toolbar.capsule_close_command, None);
    }
}
