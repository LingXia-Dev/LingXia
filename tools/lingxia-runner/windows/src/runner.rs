use crate::device::{
    DEVICE_COMMAND_BASE, OPEN_DEVTOOLS_COMMAND, default_device_index, device_label, frame_spec,
    is_device_group_start, presets,
};
use lingxia_windows::{WindowsAppMenu, WindowsAppMenuEntry, WindowsAppMenuItem};

const RUNNER_WINDOW_SIZE: (i32, i32) = (420, 880);
const VK_F12: u32 = 0x7B;

struct RunnerDevtoolAddon;

impl lingxia::HostAddon for RunnerDevtoolAddon {
    fn start_services(&self) {
        lingxia_devtool::start_devtool_bridge_from_env();
    }
}

pub(crate) fn run() -> lingxia_windows::Result<()> {
    lingxia::register_host_addon(Box::new(RunnerDevtoolAddon));

    let app = lingxia_windows::WindowsApp::from_env()
        .with_window_size(RUNNER_WINDOW_SIZE.0, RUNNER_WINDOW_SIZE.1);
    let home_app_id = lingxia_windows::init(app)?;
    install_runner_menu(home_app_id.clone());
    apply_default_device(home_app_id);
    std::process::exit(lingxia_windows::run_message_loop());
}

fn runner_menus(active_device: Option<usize>) -> Vec<WindowsAppMenu> {
    let mut device_entries = Vec::new();
    for (index, preset) in presets().iter().enumerate() {
        if is_device_group_start(index) {
            device_entries.push(WindowsAppMenuEntry::Separator);
        }
        device_entries.push(WindowsAppMenuEntry::Item(WindowsAppMenuItem {
            id: DEVICE_COMMAND_BASE + index as u32,
            label: device_label(preset),
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

fn apply_device(home_app_id: &str, index: usize) -> Result<(), String> {
    lingxia_windows::set_app_window_device_frame(home_app_id, Some(frame_spec(index)))?;
    lingxia_windows::set_windows_app_menu(runner_menus(Some(index)));
    Ok(())
}

fn install_runner_menu(home_app_id: String) {
    lingxia_windows::set_windows_app_menu_command_handler(std::sync::Arc::new(move |command| {
        if command == OPEN_DEVTOOLS_COMMAND {
            if let Err(err) = lingxia_windows::open_current_page_devtools(&home_app_id) {
                eprintln!("lingxia-runner: failed to open DevTools: {err}");
            }
            return;
        }

        let Some(index) = command
            .checked_sub(DEVICE_COMMAND_BASE)
            .map(|index| index as usize)
            .filter(|index| *index < presets().len())
        else {
            return;
        };
        if let Err(err) = apply_device(&home_app_id, index) {
            eprintln!(
                "lingxia-runner: failed to switch to {}: {err}",
                presets()[index].name
            );
        }
    }));

    lingxia_windows::set_windows_app_menu(runner_menus(None));
}

fn apply_default_device(home_app_id: String) {
    std::thread::spawn(move || {
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(200));
            if apply_device(&home_app_id, default_device_index()).is_ok() {
                return;
            }
        }
        eprintln!("lingxia-runner: home page webview never became ready for the device frame");
    });
}
