//! LingXia Runner for Windows — the dev runner that `lingxia dev` launches
//! for standalone lxapp projects (the counterpart of the macOS
//! "LingXia Runner.app").
//!
//! The runner hosts the lxapp in a plain phone-aspect window (standard
//! runtime, no shell chrome or device frame) with a small dev menu bar:
//! a "Device" menu that resizes the content area to the macOS Runner's
//! device presets, and a "View" menu with "Open DevTools" (F12). It is
//! configured entirely through the environment:
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

/// Default outer window size (pixels), roughly a phone-aspect viewport.
/// Device presets resize the content area instead (see [`DEVICE_PRESETS`]).
#[cfg(target_os = "windows")]
const RUNNER_WINDOW_SIZE: (i32, i32) = (420, 880);

/// A simulated device viewport: display name plus logical content size in
/// pixels. Mirrors the macOS Runner's `MobileDeviceSize` presets
/// (`Sources/CapsuleWindow/DeviceSpecs.swift`).
#[cfg(target_os = "windows")]
struct DevicePreset {
    name: &'static str,
    width: i32,
    height: i32,
}

#[cfg(target_os = "windows")]
const DEVICE_PRESETS: &[DevicePreset] = &[
    // ── iPhone ──
    DevicePreset { name: "iPhone SE", width: 375, height: 667 },
    DevicePreset { name: "iPhone 13 mini", width: 375, height: 812 },
    DevicePreset { name: "iPhone 13 Pro", width: 390, height: 844 },
    DevicePreset { name: "iPhone 15 Pro", width: 393, height: 852 },
    DevicePreset { name: "iPhone 11", width: 414, height: 896 },
    DevicePreset { name: "iPhone 15 Pro Max", width: 430, height: 932 },
    // ── iPad ──
    DevicePreset { name: "iPad", width: 768, height: 1024 },
    DevicePreset { name: "iPad Pro 12.9\"", width: 1024, height: 1366 },
    // ── Desktop ──
    DevicePreset { name: "Desktop 1280", width: 1280, height: 800 },
    DevicePreset { name: "Desktop 1440", width: 1440, height: 900 },
    DevicePreset { name: "Desktop 1920", width: 1920, height: 1080 },
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

/// Installs the runner menu bar and its command handler. Selecting a device
/// resizes the window's content area to that device's logical size and
/// moves the check mark; "Open DevTools" (or F12) opens the WebView2
/// DevTools for the home lxapp's current page. Nothing is persisted.
#[cfg(target_os = "windows")]
fn install_runner_menu(home_app_id: String) {
    use std::sync::Mutex;

    static ACTIVE_DEVICE: Mutex<Option<usize>> = Mutex::new(None);

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
        let preset = &DEVICE_PRESETS[index];
        match lingxia::windows::resize_app_window_content(
            &home_app_id,
            preset.width,
            preset.height,
        ) {
            Ok(()) => {
                if let Ok(mut active) = ACTIVE_DEVICE.lock() {
                    *active = Some(index);
                }
                lingxia::windows::set_windows_app_menu(runner_menus(Some(index)));
            }
            Err(err) => {
                eprintln!(
                    "lingxia-runner: failed to switch to {}: {err}",
                    preset.name
                );
            }
        }
    }));

    let active = ACTIVE_DEVICE.lock().ok().and_then(|active| *active);
    lingxia::windows::set_windows_app_menu(runner_menus(active));
}

#[cfg(target_os = "windows")]
fn main() -> lingxia_windows::Result<()> {
    lingxia::register_host_addon(Box::new(RunnerDevtoolAddon));

    let app = lingxia_windows::WindowsApp::from_env()
        .with_window_size(RUNNER_WINDOW_SIZE.0, RUNNER_WINDOW_SIZE.1);
    let home_app_id = lingxia_windows::init(app)?;
    install_runner_menu(home_app_id);
    std::process::exit(lingxia_windows::run_message_loop());
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!(
        "lingxia-runner is the Windows dev runner; on macOS use the LingXia Runner app instead."
    );
    std::process::exit(1);
}
