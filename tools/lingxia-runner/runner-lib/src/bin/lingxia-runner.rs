//! LingXia Runner for Windows — the dev runner that `lingxia dev` launches
//! for standalone lxapp projects (the counterpart of the macOS
//! "LingXia Runner.app").
//!
//! The runner hosts the lxapp as the screen of a simulated device frame —
//! a borderless window with rounded corners inside a bezel-and-shadow
//! companion window, mirroring the macOS Runner's `DeviceFrame`. A "Device"
//! menu switches between the macOS Runner's device presets and a "View"
//! menu offers "Open DevTools" (F12); while the device frame is shown the
//! window has no menu bar, and the same menus appear when right-clicking
//! the bezel (which also drags the window). It is configured entirely
//! through the environment:
//!
//! - `LINGXIA_ASSET_DIR`: host asset directory prepared by the CLI
//!   (`app.json` with the lxapp as home app + `bridge-runtime.js`).
//! - `LINGXIA_LXAPP_PATH`: standalone lxapp project (or `dist/`) whose built
//!   bundle is served live as the home app (dev bundle source).
//! - `LINGXIA_DEV_WS_URL`: dev-server websocket; the devtool bridge connects
//!   to it for logs/commands.
//! - `LINGXIA_APP_ID` / `LINGXIA_PRODUCT_NAME`: host identity and the
//!   `%LOCALAPPDATA%` state-root name.

#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

#[cfg(target_os = "windows")]
struct RunnerDevtoolAddon;

#[cfg(target_os = "windows")]
impl lingxia::HostAddon for RunnerDevtoolAddon {
    fn start_services(&self) {
        lingxia_devtool::start_devtool_bridge_from_env();
    }
}

/// Initial outer window size (pixels) before the default device frame is
/// applied (the frame application waits for the home page webview).
#[cfg(target_os = "windows")]
const RUNNER_WINDOW_SIZE: (i32, i32) = (420, 880);

/// Preset selected at startup: iPhone 15 Pro.
#[cfg(target_os = "windows")]
const DEFAULT_DEVICE: usize = 3;

/// Phone bezel styling, mirroring the macOS `DeviceFrame.Layout` constants:
/// a 4px near-black bezel for phones, a thin dark-gray border for
/// iPad/desktop sizes.
#[cfg(target_os = "windows")]
const PHONE_BEZEL: (i32, u32) = (4, 0x141414);
#[cfg(target_os = "windows")]
const DESKTOP_BEZEL: (i32, u32) = (1, 0x383838);

/// A simulated device: display name, logical screen size in pixels, and
/// frame shape. Mirrors the macOS Runner's `MobileDeviceSize` presets and
/// `DeviceFrame.Layout` radii (`Sources/CapsuleWindow/DeviceSpecs.swift`,
/// `DeviceFrame.swift`).
#[cfg(target_os = "windows")]
struct DevicePreset {
    name: &'static str,
    width: i32,
    height: i32,
    /// (bezel width, bezel color 0xRRGGBB)
    bezel: (i32, u32),
    outer_radius: i32,
    screen_radius: i32,
}

/// Frame spec for preset `index`, including the simulator toolbar: the
/// device selector lists every preset (check mark on the active one) and
/// the gear glyph opens DevTools. Toolbar selections dispatch the same
/// command ids as the menu model.
#[cfg(target_os = "windows")]
fn device_frame_spec(index: usize) -> lingxia::windows::WindowsDeviceFrame {
    use lingxia::windows::{WindowsAppMenuItem, WindowsDeviceFrame, WindowsDeviceFrameToolbar};

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
                    label: format!("{}\t{} × {}", item.name, item.width, item.height),
                    checked: item_index == index,
                    accelerator_vk: None,
                })
                .collect(),
            action_command: Some(OPEN_DEVTOOLS_COMMAND),
        }),
    }
}

#[cfg(target_os = "windows")]
const DEVICE_PRESETS: &[DevicePreset] = &[
    // ── iPhone ──
    DevicePreset { name: "iPhone SE", width: 375, height: 667, bezel: PHONE_BEZEL, outer_radius: 8, screen_radius: 0 },
    DevicePreset { name: "iPhone 13 mini", width: 375, height: 812, bezel: PHONE_BEZEL, outer_radius: 48, screen_radius: 44 },
    DevicePreset { name: "iPhone 13 Pro", width: 390, height: 844, bezel: PHONE_BEZEL, outer_radius: 48, screen_radius: 44 },
    DevicePreset { name: "iPhone 15 Pro", width: 393, height: 852, bezel: PHONE_BEZEL, outer_radius: 58, screen_radius: 54 },
    DevicePreset { name: "iPhone 11", width: 414, height: 896, bezel: PHONE_BEZEL, outer_radius: 48, screen_radius: 44 },
    DevicePreset { name: "iPhone 15 Pro Max", width: 430, height: 932, bezel: PHONE_BEZEL, outer_radius: 58, screen_radius: 54 },
    // ── iPad ──
    DevicePreset { name: "iPad", width: 768, height: 1024, bezel: DESKTOP_BEZEL, outer_radius: 8, screen_radius: 6 },
    DevicePreset { name: "iPad Pro 12.9\"", width: 1024, height: 1366, bezel: DESKTOP_BEZEL, outer_radius: 8, screen_radius: 6 },
    // ── Desktop ──
    DevicePreset { name: "Desktop 1280", width: 1280, height: 800, bezel: DESKTOP_BEZEL, outer_radius: 8, screen_radius: 6 },
    DevicePreset { name: "Desktop 1440", width: 1440, height: 900, bezel: DESKTOP_BEZEL, outer_radius: 8, screen_radius: 6 },
    DevicePreset { name: "Desktop 1920", width: 1920, height: 1080, bezel: DESKTOP_BEZEL, outer_radius: 8, screen_radius: 6 },
];

/// Preset indices that start a new size class (separator drawn before).
#[cfg(target_os = "windows")]
const DEVICE_GROUP_STARTS: [usize; 2] = [6, 8];

/// "Device" menu command ids are `DEVICE_COMMAND_BASE + preset index`.
#[cfg(target_os = "windows")]
const DEVICE_COMMAND_BASE: u32 = 0x0100;

#[cfg(target_os = "windows")]
const OPEN_DEVTOOLS_COMMAND: u32 = 0x0200;

/// Virtual-key code of F12, the DevTools accelerator. The menu accelerator
/// fires while the native window has focus; while the page itself has
/// focus, WebView2's built-in F12 opens DevTools (enabled by default).
#[cfg(target_os = "windows")]
const VK_F12: u32 = 0x7B;

/// Builds the runner menu bar: a "Device" menu listing the presets (check
/// mark on the active one; none while the default 420x880 window is kept)
/// and a "View" menu with the DevTools item.
#[cfg(target_os = "windows")]
fn runner_menus(active_device: Option<usize>) -> Vec<lingxia::windows::WindowsAppMenu> {
    use lingxia::windows::{WindowsAppMenu, WindowsAppMenuEntry, WindowsAppMenuItem};

    let mut device_entries = Vec::new();
    for (index, preset) in DEVICE_PRESETS.iter().enumerate() {
        if DEVICE_GROUP_STARTS.contains(&index) {
            device_entries.push(WindowsAppMenuEntry::Separator);
        }
        device_entries.push(WindowsAppMenuEntry::Item(WindowsAppMenuItem {
            id: DEVICE_COMMAND_BASE + index as u32,
            label: format!("{}\t{} × {}", preset.name, preset.width, preset.height),
            checked: active_device == Some(index),
            accelerator_vk: None,
        }));
    }

    vec![
        WindowsAppMenu {
            title: "Device".to_string(),
            entries: device_entries,
        },
        WindowsAppMenu {
            title: "View".to_string(),
            entries: vec![WindowsAppMenuEntry::Item(WindowsAppMenuItem {
                id: OPEN_DEVTOOLS_COMMAND,
                label: "Open DevTools\tF12".to_string(),
                checked: false,
                accelerator_vk: Some(VK_F12),
            })],
        },
    ]
}

/// Applies device preset `index`: presents the device frame (which also
/// sizes the window to the device screen) and moves the menu check mark.
#[cfg(target_os = "windows")]
fn apply_device(home_app_id: &str, index: usize) -> Result<(), String> {
    lingxia::windows::set_app_window_device_frame(home_app_id, Some(device_frame_spec(index)))?;
    lingxia::windows::set_windows_app_menu(runner_menus(Some(index)));
    Ok(())
}

/// Installs the runner menus and their command handler. Selecting a device
/// switches the device frame and moves the check mark; "Open DevTools" (or
/// F12) opens the WebView2 DevTools for the home lxapp's current page.
/// Nothing is persisted.
#[cfg(target_os = "windows")]
fn install_runner_menu(home_app_id: String) {
    lingxia::windows::set_windows_app_menu_command_handler(std::sync::Arc::new(move |command| {
        if command == OPEN_DEVTOOLS_COMMAND {
            if let Err(err) = lingxia::windows::open_current_page_devtools(&home_app_id) {
                eprintln!("lingxia-runner: failed to open DevTools: {err}");
            }
            return;
        }

        let Some(index) = command
            .checked_sub(DEVICE_COMMAND_BASE)
            .map(|index| index as usize)
            .filter(|index| *index < DEVICE_PRESETS.len())
        else {
            return;
        };
        if let Err(err) = apply_device(&home_app_id, index) {
            eprintln!(
                "lingxia-runner: failed to switch to {}: {err}",
                DEVICE_PRESETS[index].name
            );
        }
    }));

    lingxia::windows::set_windows_app_menu(runner_menus(None));
}

/// Presents the default device frame once the home page webview is up (the
/// frame resolves the window through the current page, which finishes
/// loading shortly after init returns).
#[cfg(target_os = "windows")]
fn apply_default_device(home_app_id: String) {
    std::thread::spawn(move || {
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(200));
            if apply_device(&home_app_id, DEFAULT_DEVICE).is_ok() {
                return;
            }
        }
        eprintln!("lingxia-runner: home page webview never became ready for the device frame");
    });
}

#[cfg(target_os = "windows")]
fn main() -> lingxia_windows::Result<()> {
    lingxia::register_host_addon(Box::new(RunnerDevtoolAddon));

    let app = lingxia_windows::WindowsApp::from_env()
        .with_window_size(RUNNER_WINDOW_SIZE.0, RUNNER_WINDOW_SIZE.1);
    let home_app_id = lingxia_windows::init(app)?;
    install_runner_menu(home_app_id.clone());
    apply_default_device(home_app_id);
    std::process::exit(lingxia_windows::run_message_loop());
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!(
        "lingxia-runner is the Windows dev runner; on macOS use the LingXia Runner app instead."
    );
    std::process::exit(1);
}
