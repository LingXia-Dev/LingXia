use lingxia_windows_sdk::{
    WindowsAppMenuItem, WindowsBrowserEmulationProfile, WindowsDeviceFrame,
    WindowsDeviceFrameCutout, WindowsDeviceFrameSelectorEntry, WindowsDeviceFrameStatusBar,
    WindowsDeviceFrameToolbar,
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

/// Toolbar appearance glyph: pins the opposite of what the screen shows.
pub(crate) const APPEARANCE_COMMAND: u32 = 0x0900;

/// App-declared More actions occupy seven generation-scoped command slots.
pub(crate) const MORE_ACTION_COMMAND_BASE: u32 = 0x0A00;

/// The selector dropdown only chooses the simulated frame/device.
fn device_selector_items(index: usize) -> Vec<WindowsDeviceFrameSelectorEntry> {
    let mut entries = Vec::new();
    let mut previous_group = None;
    for (item_index, preset) in presets().iter().enumerate() {
        if previous_group != Some(preset.group()) {
            if previous_group.is_some() {
                entries.push(WindowsDeviceFrameSelectorEntry::separator());
            }
            entries.push(WindowsDeviceFrameSelectorEntry::header(group_title(
                preset.group(),
            )));
            previous_group = Some(preset.group());
        }
        entries.push(WindowsDeviceFrameSelectorEntry::item(
            WindowsAppMenuItem::new(
                DEVICE_COMMAND_BASE + item_index as u32,
                device_label(preset),
            )
            .checked(item_index == index),
        ));
    }
    entries
}

fn group_title(group: &str) -> &str {
    match group {
        "phone" => "iPhone",
        "tablet" => "iPad",
        "desktop" => "Desktop",
        other => other,
    }
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
    /// Points; an Android or HarmonyOS phone's are fractional (pixels over density). This
    /// Runner lays the screen out in whole points.
    pub(crate) width: f32,
    pub(crate) height: f32,
    #[serde(rename = "pixelRatio")]
    pub(crate) pixel_ratio: f32,
    #[serde(rename = "bezelWidth")]
    bezel_width: i32,
    #[serde(rename = "bezelColor")]
    bezel_color: u32,
    #[serde(rename = "outerRadius")]
    outer_radius: i32,
    #[serde(rename = "screenRadius")]
    screen_radius: i32,
    notch: DeviceNotch,
    /// What a phone's host gives a page, in device points.
    chrome: Option<DeviceChrome>,
}

#[derive(Debug, Deserialize)]
struct DeviceChrome {
    /// The host the phone runs: `ios`, `android` or `harmony`.
    host: String,
    #[serde(rename = "safeArea")]
    safe_area: SafeArea,
    #[serde(rename = "navigationBar")]
    navigation_bar: f32,
    #[serde(rename = "tabBar")]
    tab_bar: f32,
}

#[derive(Debug, Deserialize)]
struct SafeArea {
    portrait: [f32; 4],
    landscape: [f32; 4],
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

/// How far the host's tab bar reaches above the screen's bottom edge. The Android host stacks
/// it on the system bar; iOS grows it through the home indicator; HarmonyOS grows it by its
/// extra bottom (TabBar.ets).
fn tab_bar_reach(chrome: &DeviceChrome, landscape: bool) -> f32 {
    let bottom = if landscape {
        chrome.safe_area.landscape[2]
    } else {
        chrome.safe_area.portrait[2]
    };
    chrome.tab_bar
        + match chrome.host.as_str() {
            "android" | "ios" => bottom,
            "harmony" => (bottom - 6.0).max(12.0) - 12.0,
            _ => 0.0,
        }
}

/// The host tab bar a phone preset gives its pages: its items' band, and the reach past it
/// that [`gpu_simulation`] reports to GPU pages.
fn tab_bar_spec(
    preset: &DevicePreset,
    landscape: bool,
) -> lingxia_windows_sdk::WindowsDeviceFrameTabBar {
    let (band, reach) = match &preset.chrome {
        Some(chrome) => (chrome.tab_bar, tab_bar_reach(chrome, landscape)),
        None => (64.0, 64.0),
    };
    lingxia_windows_sdk::WindowsDeviceFrameTabBar {
        height: band.round() as i32,
        growth: (reach - band).max(0.0).round() as i32,
    }
}

/// What GPU pages measure on phone `index`: its viewport, ratio, safe area and host bars in
/// its points, whatever size the frame shows it at. Tablets and desktops lay out in the window.
pub(crate) fn gpu_simulation(
    index: usize,
    landscape: bool,
    text_scale: f32,
    high_contrast: bool,
) -> Option<lingxia::windows::GpuSimulation> {
    let preset = presets().get(index).filter(|_| is_phone(index))?;
    let viewport = if landscape {
        [preset.height, preset.width]
    } else {
        [preset.width, preset.height]
    };
    let (safe_area, navigation_bar, tab_bar_reach) = match &preset.chrome {
        Some(chrome) => {
            let safe = if landscape {
                chrome.safe_area.landscape
            } else {
                chrome.safe_area.portrait
            };
            (
                safe,
                chrome.navigation_bar,
                tab_bar_reach(chrome, landscape),
            )
        }
        None => (
            [preset.notch.status_bar_height as f32, 0.0, 0.0, 0.0],
            44.0,
            64.0,
        ),
    };
    Some(lingxia::windows::GpuSimulation {
        viewport,
        pixel_ratio: preset.pixel_ratio,
        safe_area,
        navigation_bar,
        tab_bar_reach,
        smallest_width: preset.width.min(preset.height),
        text_scale,
        high_contrast,
    })
}

pub(crate) fn device_label(preset: &DevicePreset) -> String {
    format!(
        "{}\t{} x {}",
        preset.name,
        preset.width.round(),
        preset.height.round()
    )
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

/// `capsule`: whether the lxapp in this frame gets the simulated host capsule.
pub(crate) fn frame_spec(index: usize, landscape: bool, capsule: bool) -> WindowsDeviceFrame {
    let preset = &presets()[index];
    // Landscape swaps the screen's long and short edges; the bezel, radii, and
    // toolbar follow the new width automatically.
    let (screen_width, screen_height) = if landscape {
        (preset.height, preset.width)
    } else {
        (preset.width, preset.height)
    };
    let (screen_width, screen_height) = (screen_width.round() as i32, screen_height.round() as i32);
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
        tab_bar: is_phone(index).then(|| tab_bar_spec(preset, landscape)),
        bezel_color: preset.bezel_color,
        // Keep clipped screen corners backed by the device shell instead of
        // exposing the desktop through the rounded edge.
        screen_corner_color: preset.bezel_color,
        toolbar: Some(WindowsDeviceFrameToolbar {
            selector_label: preset.name.clone(),
            selector_items: device_selector_items(index),
            action_command: Some(OPEN_DEVTOOLS_COMMAND),
            rotate_command: Some(ROTATE_COMMAND),
            appearance_command: Some(APPEARANCE_COMMAND),
            appearance_dark: crate::runner::effective_appearance_dark(),
            capsule_items: if is_phone(index) && capsule {
                capsule_menu_items()
            } else {
                Vec::new()
            },
            capsule_close_command: (is_phone(index) && capsule).then_some(CAPSULE_CLOSE_COMMAND),
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
    frame_spec(index, landscape, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_spec_uses_manifest_device_shell() {
        for (index, preset) in presets().iter().enumerate() {
            let frame = frame_spec(index, false, true);
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
        let frame = frame_spec(index, true, true);

        assert_eq!(frame.screen_width, preset.height.round() as i32);
        assert_eq!(frame.screen_height, preset.width.round() as i32);
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

    #[test]
    fn selector_groups_devices_by_form_factor() {
        let entries = device_selector_items(default_device_index());
        let headings = entries
            .iter()
            .filter_map(|entry| match entry {
                WindowsDeviceFrameSelectorEntry::Header(label) => Some(label.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let separators = entries
            .iter()
            .filter(|entry| matches!(entry, WindowsDeviceFrameSelectorEntry::Separator))
            .count();

        assert_eq!(headings, ["iPhone", "iPad", "Desktop"]);
        assert_eq!(separators, headings.len() - 1);
    }
}
