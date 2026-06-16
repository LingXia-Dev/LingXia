use lingxia_windows_sdk::{WindowsAppMenuItem, WindowsDeviceFrame, WindowsDeviceFrameToolbar};
use serde::Deserialize;
use std::sync::OnceLock;

pub(crate) const DEVICE_COMMAND_BASE: u32 = 0x0100;

pub(crate) const OPEN_DEVTOOLS_COMMAND: u32 = 0x0200;

#[derive(Debug, Deserialize)]
struct RunnerDevices {
    default: String,
    devices: Vec<DevicePreset>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DevicePreset {
    id: String,
    group: String,
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

pub(crate) fn is_device_group_start(index: usize) -> bool {
    index > 0
        && presets()
            .get(index - 1)
            .zip(presets().get(index))
            .is_some_and(|(previous, current)| previous.group != current.group)
}

pub(crate) fn device_label(preset: &DevicePreset) -> String {
    format!("{}\t{} x {}", preset.name, preset.width, preset.height)
}

pub(crate) fn frame_spec(index: usize) -> WindowsDeviceFrame {
    let preset = &presets()[index];
    WindowsDeviceFrame {
        screen_width: preset.width,
        screen_height: preset.height,
        bezel_width: preset.bezel_width,
        outer_corner_radius: preset.outer_radius,
        screen_corner_radius: preset.screen_radius,
        bezel_color: preset.bezel_color,
        toolbar: Some(WindowsDeviceFrameToolbar {
            selector_label: preset.name.clone(),
            selector_items: presets()
                .iter()
                .enumerate()
                .map(|(item_index, item)| WindowsAppMenuItem {
                    id: DEVICE_COMMAND_BASE + item_index as u32,
                    label: device_label(item),
                    checked: item_index == index,
                    accelerator_vk: None,
                })
                .collect(),
            action_command: Some(OPEN_DEVTOOLS_COMMAND),
        }),
    }
}
