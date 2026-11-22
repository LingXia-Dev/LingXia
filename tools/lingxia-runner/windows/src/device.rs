use lingxia_windows_sdk::{
    WindowsAppMenuItem, WindowsBrowserEmulationProfile, WindowsDeviceFrame,
    WindowsDeviceFrameCutout, WindowsDeviceFrameHinge, WindowsDeviceFrameSelectorEntry,
    WindowsDeviceFrameStatusBar, WindowsDeviceFrameToolbar,
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

/// Flat `(device_index, posture_index)` table backing `DEVICE_COMMAND_BASE + slot`.
pub(crate) fn selector_targets() -> &'static [(usize, usize)] {
    static TARGETS: OnceLock<Vec<(usize, usize)>> = OnceLock::new();
    TARGETS.get_or_init(|| {
        let mut targets = Vec::new();
        for (device_index, preset) in presets().iter().enumerate() {
            if preset.postures.is_empty() {
                targets.push((device_index, 0));
            } else {
                for posture_index in 0..preset.postures.len() {
                    targets.push((device_index, posture_index));
                }
            }
        }
        targets
    })
}

pub(crate) fn selector_target_for_command(command: u32) -> Option<(usize, usize)> {
    let slot = command.checked_sub(DEVICE_COMMAND_BASE)? as usize;
    selector_targets().get(slot).copied()
}

/// The selector dropdown chooses the simulated device (and posture for foldables).
fn device_selector_items(
    index: usize,
    posture_index: usize,
) -> Vec<WindowsDeviceFrameSelectorEntry> {
    let mut entries = Vec::new();
    let mut previous_group = None;
    let mut slot = 0u32;
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
        if preset.postures.is_empty() {
            entries.push(WindowsDeviceFrameSelectorEntry::item(
                WindowsAppMenuItem::new(DEVICE_COMMAND_BASE + slot, device_label(preset))
                    .checked(item_index == index),
            ));
            slot += 1;
        } else {
            for (p_index, posture) in preset.postures.iter().enumerate() {
                let label = format!(
                    "{} — {}\t{} x {}",
                    preset.name,
                    posture.label,
                    posture.width.round(),
                    posture.height.round()
                );
                entries.push(WindowsDeviceFrameSelectorEntry::item(
                    WindowsAppMenuItem::new(DEVICE_COMMAND_BASE + slot, label)
                        .checked(item_index == index && p_index == posture_index),
                ));
                slot += 1;
            }
        }
    }
    entries
}

fn group_title(group: &str) -> &str {
    match group {
        "phone" => "iPhone",
        "fold" => "Foldable",
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

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DeviceHinge {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) orientation: String,
    pub(crate) posture: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DevicePosture {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) width: f32,
    pub(crate) height: f32,
    #[serde(default)]
    pub(crate) hinge: Option<DeviceHinge>,
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
    /// Foldable postures (`outer`, `open`, `tabletop`, `dual`, ...), empty for rigid devices.
    #[serde(default)]
    pub(crate) postures: Vec<DevicePosture>,
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

    /// Form-factor group ("phone" | "fold" | "tablet" | "desktop").
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

    /// Default posture index when switching to this device (prefers `"open"`, else `0`).
    pub(crate) fn default_posture_index(&self) -> usize {
        self.postures
            .iter()
            .position(|p| p.id == "open")
            .unwrap_or(0)
    }

    pub(crate) fn posture_index(&self, id: &str) -> Option<usize> {
        self.postures.iter().position(|p| p.id == id)
    }

    pub(crate) fn active_posture(&self, posture_index: usize) -> Option<&DevicePosture> {
        self.postures
            .get(posture_index)
            .or_else(|| self.postures.first())
    }

    pub(crate) fn effective_dimensions(&self, posture_index: usize, landscape: bool) -> (f32, f32) {
        let (w, h) = self
            .active_posture(posture_index)
            .map_or((self.width, self.height), |p| (p.width, p.height));
        if landscape { (h, w) } else { (w, h) }
    }

    pub(crate) fn effective_hinge(
        &self,
        posture_index: usize,
        landscape: bool,
    ) -> Option<DeviceHinge> {
        let h = self.active_posture(posture_index)?.hinge.as_ref()?;
        if !landscape {
            return Some(h.clone());
        }
        Some(DeviceHinge {
            x: h.y,
            y: h.x,
            width: h.height,
            height: h.width,
            orientation: if h.orientation == "vertical" {
                "horizontal".to_string()
            } else {
                "vertical".to_string()
            },
            posture: h.posture.clone(),
        })
    }

    pub(crate) fn toolbar_label(&self, posture_index: usize) -> String {
        match self.active_posture(posture_index) {
            Some(posture) => format!("{} ({})", self.name, posture.label),
            None => self.name.clone(),
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

/// Startup device: `LINGXIA_RUNNER_DEVICE` (shared with the macOS runner)
/// when it names a known device id, else the manifest default.
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

pub(crate) fn is_fold(index: usize) -> bool {
    presets()
        .get(index)
        .is_some_and(|preset| preset.group == "fold")
}

/// Phones and foldables render in handheld frames with simulated GPU viewports.
pub(crate) fn is_handheld(index: usize) -> bool {
    is_phone(index) || is_fold(index)
}

pub(crate) fn is_compact_screen(index: usize, posture_index: usize, landscape: bool) -> bool {
    let Some(preset) = presets().get(index) else {
        return true;
    };
    let (w, _) = preset.effective_dimensions(posture_index, landscape);
    w < 600.0
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

/// The host tab bar a phone/fold preset gives its pages: its items' band, and the reach past it
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

/// What GPU pages measure on phone/fold `index`: its viewport, ratio, safe area, host bars,
/// and hinge in its points, whatever size the frame shows it at. Tablets and desktops lay out
/// in the window.
pub(crate) fn gpu_simulation(
    index: usize,
    posture_index: usize,
    landscape: bool,
    text_scale: f32,
    high_contrast: bool,
) -> Option<lingxia::windows::GpuSimulation> {
    let preset = presets().get(index).filter(|_| is_handheld(index))?;
    let (w, h) = preset.effective_dimensions(posture_index, landscape);
    let viewport = [w, h];
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
    let hinge = preset.effective_hinge(posture_index, landscape).map(|h| {
        serde_json::json!({
            "x": h.x,
            "y": h.y,
            "width": h.width,
            "height": h.height,
            "orientation": h.orientation,
            "posture": h.posture,
        })
    });
    Some(lingxia::windows::GpuSimulation {
        viewport,
        pixel_ratio: preset.pixel_ratio,
        safe_area,
        navigation_bar,
        tab_bar_reach,
        smallest_width: w.min(h),
        text_scale,
        high_contrast,
        hinge,
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
pub(crate) fn frame_spec(
    index: usize,
    posture_index: usize,
    landscape: bool,
    capsule: bool,
) -> WindowsDeviceFrame {
    let preset = &presets()[index];
    let (screen_w, screen_h) = preset.effective_dimensions(posture_index, landscape);
    let (screen_width, screen_height) = (screen_w.round() as i32, screen_h.round() as i32);
    let handheld = is_handheld(index);
    let compact = is_compact_screen(index, posture_index, landscape);
    let hinge = preset
        .effective_hinge(posture_index, landscape)
        .map(|h| WindowsDeviceFrameHinge {
            x: h.x.round() as i32,
            y: h.y.round() as i32,
            width: h.width.round() as i32,
            height: h.height.round() as i32,
            vertical: h.orientation == "vertical",
            half_opened: h.posture == "half-opened",
        });
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
        hinge,
        status_bar: (handheld && !landscape && preset.notch.status_bar_height > 0).then(|| {
            WindowsDeviceFrameStatusBar {
                height: preset.notch.status_bar_height,
                // Initial colors + opacity; the shell overrides these per page
                // from the active page's navigation-bar style (and switches the
                // strip transparent for immersive custom-navigation pages). The
                // real current time is drawn by the device frame.
                foreground: status_bar_foreground(),
                background: lingxia_windows_sdk::windows_shell_background_color(),
                transparent: false,
            }
        }),
        tab_bar: (handheld && compact).then(|| tab_bar_spec(preset, landscape)),
        bezel_color: preset.bezel_color,
        // Keep clipped screen corners backed by the device shell instead of
        // exposing the desktop through the rounded edge.
        screen_corner_color: preset.bezel_color,
        toolbar: Some(WindowsDeviceFrameToolbar {
            selector_label: preset.toolbar_label(posture_index),
            selector_items: device_selector_items(index, posture_index),
            action_command: Some(OPEN_DEVTOOLS_COMMAND),
            rotate_command: Some(ROTATE_COMMAND),
            appearance_command: Some(APPEARANCE_COMMAND),
            appearance_dark: crate::runner::effective_appearance_dark(),
            capsule_items: if handheld && capsule {
                capsule_menu_items()
            } else {
                Vec::new()
            },
            capsule_close_command: (handheld && capsule).then_some(CAPSULE_CLOSE_COMMAND),
            // Phones/foldables/tablets are handheld mockups: the toolbar's
            // macOS-style dots own close/minimize. A simulated desktop keeps
            // the standard Windows caption buttons in the shell chrome instead.
            window_dots: handheld || is_tablet(index),
        }),
    }
}

/// Browser chrome already owns dismissal and tab actions. Keep the simulated
/// device selector/rotation controls, but do not overlay lxapp capsule actions.
pub(crate) fn browser_frame_spec(
    index: usize,
    posture_index: usize,
    landscape: bool,
) -> WindowsDeviceFrame {
    frame_spec(index, posture_index, landscape, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_spec_uses_manifest_device_shell() {
        for (index, preset) in presets().iter().enumerate() {
            let posture = preset.default_posture_index();
            let frame = frame_spec(index, posture, false, true);
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
        let frame = frame_spec(index, 0, true, true);

        assert_eq!(frame.screen_width, preset.height.round() as i32);
        assert_eq!(frame.screen_height, preset.width.round() as i32);
        assert_eq!(frame.bezel_width, preset.bezel_width.max(0));
        assert_eq!(frame.outer_corner_radius, preset.outer_radius);
    }

    #[test]
    fn browser_frame_has_no_lxapp_capsule() {
        let frame = browser_frame_spec(default_device_index(), 0, false);
        let toolbar = frame.toolbar.expect("runner frame has a toolbar");
        assert!(toolbar.capsule_items.is_empty());
        assert_eq!(toolbar.capsule_close_command, None);
    }

    #[test]
    fn selector_groups_devices_by_form_factor() {
        let entries = device_selector_items(default_device_index(), 0);
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

        assert_eq!(headings, ["iPhone", "Foldable", "iPad", "Desktop"]);
        assert_eq!(separators, headings.len() - 1);
    }

    #[test]
    fn foldable_postures_supply_dimensions_and_hinge() {
        let mate_x5 = presets()
            .iter()
            .position(|p| p.id() == "huawei-mate-x5")
            .expect("huawei-mate-x5 preset must exist");
        let preset = &presets()[mate_x5];
        let outer = preset.posture_index("outer").expect("outer posture");
        let open = preset.posture_index("open").expect("open posture");
        let tabletop = preset.posture_index("tabletop").expect("tabletop posture");

        let outer_frame = frame_spec(mate_x5, outer, false, true);
        assert_eq!(outer_frame.screen_width, 360);
        assert_eq!(outer_frame.hinge, None);

        let open_frame = frame_spec(mate_x5, open, false, true);
        assert_eq!(open_frame.screen_width, 784);
        assert!(
            open_frame
                .hinge
                .is_some_and(|h| h.vertical && !h.half_opened)
        );

        let tabletop_frame = frame_spec(mate_x5, tabletop, false, true);
        assert_eq!(tabletop_frame.screen_width, 868);
        assert!(
            tabletop_frame
                .hinge
                .is_some_and(|h| !h.vertical && h.half_opened)
        );
    }
}
