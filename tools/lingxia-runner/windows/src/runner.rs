use crate::device::{
    DEVICE_COMMAND_BASE, OPEN_DEVTOOLS_COMMAND, default_device_index, device_label, frame_spec,
    is_device_group_start, presets,
};
use lingxia_windows_sdk::{WindowsAppMenu, WindowsAppMenuEntry, WindowsAppMenuItem};

const VK_F12: u32 = 0x7B;

struct RunnerDevtoolAddon;

impl lingxia::HostAddon for RunnerDevtoolAddon {
    fn start_services(&self) {
        lingxia_devtool::start_devtool_bridge_from_env();
    }
}

pub(crate) fn run() -> lingxia_windows_sdk::Result<()> {
    lingxia::register_host_addon(Box::new(RunnerDevtoolAddon));

    let default_device = default_device_index();
    let initial_frame = frame_spec(default_device);
    lingxia_windows_sdk::set_initial_app_window_device_frame(initial_frame.clone());
    let app = lingxia_windows_sdk::WindowsApp::from_env()
        .with_window_size(initial_frame.screen_width, initial_frame.screen_height);
    let home_app_id = lingxia_windows_sdk::init(app)?;
    install_runner_menu(home_app_id.clone());
    apply_default_device(home_app_id, default_device);
    std::process::exit(lingxia_windows_sdk::run_message_loop());
}

fn runner_menus(active_device: Option<usize>) -> Vec<WindowsAppMenu> {
    let mut device_entries = Vec::new();
    for (index, preset) in presets().iter().enumerate() {
        if is_device_group_start(index) {
            device_entries.push(WindowsAppMenuEntry::separator());
        }
        device_entries.push(
            WindowsAppMenuItem::new(DEVICE_COMMAND_BASE + index as u32, device_label(preset))
                .checked(active_device == Some(index))
                .into(),
        );
    }

    vec![
        WindowsAppMenu::new("Device", device_entries),
        WindowsAppMenu::new(
            "View",
            [
                WindowsAppMenuItem::new(OPEN_DEVTOOLS_COMMAND, "Open DevTools\tF12")
                    .accelerator_vk(VK_F12)
                    .into(),
            ],
        ),
    ]
}

fn apply_device(home_app_id: &str, index: usize) -> Result<(), String> {
    lingxia_windows_sdk::set_app_window_device_frame(home_app_id, Some(frame_spec(index)))?;
    lingxia_windows_sdk::set_windows_app_menu(runner_menus(Some(index)));
    Ok(())
}

fn install_runner_menu(home_app_id: String) {
    lingxia_windows_sdk::set_windows_app_menu_command_handler(std::sync::Arc::new(
        move |command| {
            if command == OPEN_DEVTOOLS_COMMAND {
                if let Err(err) = lingxia_windows_sdk::open_current_page_devtools(&home_app_id) {
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
        },
    ));

    lingxia_windows_sdk::set_windows_app_menu(runner_menus(None));
}

fn apply_default_device(home_app_id: String, default_device: usize) {
    std::thread::spawn(move || {
        for attempt in 0..80 {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            if apply_device(&home_app_id, default_device).is_ok() {
                return;
            }
        }
        eprintln!("lingxia-runner: home page webview never became ready for the device frame");
    });
}
