use lingxia::windows::{WindowsAppMenuItem, WindowsDeviceFrame, WindowsDeviceFrameToolbar};

pub(crate) const DEFAULT_DEVICE: usize = 3;

pub(crate) const DEVICE_COMMAND_BASE: u32 = 0x0100;

pub(crate) const OPEN_DEVTOOLS_COMMAND: u32 = 0x0200;

pub(crate) struct DevicePreset {
    pub(crate) name: &'static str,
    pub(crate) width: i32,
    pub(crate) height: i32,
    bezel: (i32, u32),
    outer_radius: i32,
    screen_radius: i32,
}

const PHONE_BEZEL: (i32, u32) = (4, 0x141414);
const DESKTOP_BEZEL: (i32, u32) = (1, 0x383838);

pub(crate) const DEVICE_GROUP_STARTS: [usize; 2] = [6, 8];

pub(crate) const DEVICE_PRESETS: &[DevicePreset] = &[
    DevicePreset {
        name: "iPhone SE",
        width: 375,
        height: 667,
        bezel: PHONE_BEZEL,
        outer_radius: 8,
        screen_radius: 0,
    },
    DevicePreset {
        name: "iPhone 13 mini",
        width: 375,
        height: 812,
        bezel: PHONE_BEZEL,
        outer_radius: 48,
        screen_radius: 44,
    },
    DevicePreset {
        name: "iPhone 13 Pro",
        width: 390,
        height: 844,
        bezel: PHONE_BEZEL,
        outer_radius: 48,
        screen_radius: 44,
    },
    DevicePreset {
        name: "iPhone 15 Pro",
        width: 393,
        height: 852,
        bezel: PHONE_BEZEL,
        outer_radius: 58,
        screen_radius: 54,
    },
    DevicePreset {
        name: "iPhone 11",
        width: 414,
        height: 896,
        bezel: PHONE_BEZEL,
        outer_radius: 48,
        screen_radius: 44,
    },
    DevicePreset {
        name: "iPhone 15 Pro Max",
        width: 430,
        height: 932,
        bezel: PHONE_BEZEL,
        outer_radius: 58,
        screen_radius: 54,
    },
    DevicePreset {
        name: "iPad",
        width: 768,
        height: 1024,
        bezel: DESKTOP_BEZEL,
        outer_radius: 8,
        screen_radius: 6,
    },
    DevicePreset {
        name: "iPad Pro 12.9\"",
        width: 1024,
        height: 1366,
        bezel: DESKTOP_BEZEL,
        outer_radius: 8,
        screen_radius: 6,
    },
    DevicePreset {
        name: "Desktop 1280",
        width: 1280,
        height: 800,
        bezel: DESKTOP_BEZEL,
        outer_radius: 8,
        screen_radius: 6,
    },
    DevicePreset {
        name: "Desktop 1440",
        width: 1440,
        height: 900,
        bezel: DESKTOP_BEZEL,
        outer_radius: 8,
        screen_radius: 6,
    },
    DevicePreset {
        name: "Desktop 1920",
        width: 1920,
        height: 1080,
        bezel: DESKTOP_BEZEL,
        outer_radius: 8,
        screen_radius: 6,
    },
];

pub(crate) fn device_label(preset: &DevicePreset) -> String {
    format!("{}\t{} x {}", preset.name, preset.width, preset.height)
}

pub(crate) fn frame_spec(index: usize) -> WindowsDeviceFrame {
    let preset = &DEVICE_PRESETS[index];
    WindowsDeviceFrame {
        screen_width: preset.width,
        screen_height: preset.height,
        bezel_width: preset.bezel.0,
        outer_corner_radius: preset.outer_radius,
        screen_corner_radius: preset.screen_radius,
        bezel_color: preset.bezel.1,
        toolbar: Some(WindowsDeviceFrameToolbar {
            selector_label: preset.name.to_string(),
            selector_items: DEVICE_PRESETS
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
