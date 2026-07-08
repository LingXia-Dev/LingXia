use crate::device::{
    ABOUT_COMMAND, CAPSULE_CLOSE_COMMAND, CLEAN_CACHE_COMMAND, DEVICE_COMMAND_BASE,
    OPEN_DEVTOOLS_COMMAND, RESTART_LXAPP_COMMAND, ROTATE_COMMAND, frame_spec, initial_device_index,
    is_phone, is_tablet, presets,
};
use lingxia_windows_sdk::WindowsShellTabBarPosition;
use lxapp::{LxAppDelegate, LxAppUiEventType};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// The device + orientation the simulator currently shows. The toolbar's
/// rotate button toggles `LANDSCAPE` for the active `CURRENT_DEVICE`; picking a
/// device from the selector resets to portrait.
static CURRENT_DEVICE: AtomicUsize = AtomicUsize::new(0);
static LANDSCAPE: AtomicBool = AtomicBool::new(false);

const ARG_ASSET_DIR: &str = "--asset-dir";
const ARG_LXAPP_PATH: &str = "--lxapp-path";
const ARG_DEV_WS_URL: &str = "--dev-ws-url";
const ARG_LINGXIAO_MOCK_DIR: &str = "--lingxiao-mock-dir";
const ARG_RUNNER_DEVICE: &str = "--runner-device";
const ARG_RUNNER_ENV: &str = "--runner-env";
const ARG_RESOURCE_LXAPP_PATHS: &str = "--resource-lxapp-paths";
const ENV_LXAPP_PATH: &str = "LINGXIA_LXAPP_PATH";
const ENV_DEV_WS_URL: &str = "LINGXIA_DEV_WS_URL";
const ENV_STATE_ROOT: &str = "LINGXIA_STATE_ROOT";
const ENV_LINGXIAO_MOCK_DIR: &str = "LINGXIAO_MOCK_DIR";
const ENV_RUNNER_DEVICE: &str = "LINGXIA_RUNNER_DEVICE";
const ENV_RUNNER_ENV: &str = "LINGXIA_RUNNER_ENV";
const ENV_RESOURCE_LXAPP_PATHS: &str = "LINGXIA_RESOURCE_LXAPP_PATHS";

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceLxAppPath {
    app_id: String,
    path: std::path::PathBuf,
}

struct RunnerDevtoolAddon;

impl lingxia::HostAddon for RunnerDevtoolAddon {
    // Cloud provider. Must register in this hook — the logic context is built
    // before `start_services`. Injected via `--with-provider cloud`. The runner
    // env contract (config.toml overrides, mock dir, functions.json routing) is
    // resolved by `lingxia_runner_config`, shared with the macOS runner.
    #[cfg(feature = "cloud")]
    fn install_logic_extensions(&self) {
        if let Err(err) = lingxia_cloud_client::init(cloud_options()) {
            eprintln!("[cloud] provider init failed: {err}");
        }
    }

    fn start_services(&self) {
        lingxia_devtool::start_devtool_bridge_from_env();
    }
}

/// Map the shared, cloud-free runner config onto the cloud client's option and
/// routing types (available only here, via the injected provider crate).
#[cfg(feature = "cloud")]
fn cloud_options() -> lingxia_cloud_client::CloudOptions {
    use lingxia_cloud_client::{CloudOptions, MockRouting, Provider};
    let cfg = lingxia_runner_config::from_env();
    let mut options = CloudOptions::default();
    if let Some(server) = cfg.lingxia_server {
        options = options.lingxia_server(server);
    }
    if let Some(id) = cfg.lingxia_id {
        options = options.lingxia_id(id);
    }
    if let Some(mock) = cfg.mock {
        let provider = |live| if live { Provider::Live } else { Provider::Mock };
        let routing = MockRouting {
            default: provider(mock.routing.default_live),
            overrides: mock
                .routing
                .overrides
                .into_iter()
                .map(|(name, live)| (name, provider(live)))
                .collect(),
        };
        options = options.lingxiao_mock(mock.dir).lingxiao_routing(routing);
    }
    options
}

pub(crate) fn run() -> lingxia_windows_sdk::Result<()> {
    let asset_dir = install_launch_args_env();
    register_resource_lxapp_paths_from_env();
    lingxia::register_host_addon(Box::new(RunnerDevtoolAddon));

    let default_device = initial_device_index();
    // Same orientation rule as the toolbar selector: tablets land in
    // landscape, phones/desktops in portrait.
    let initial_landscape = is_tablet(default_device);
    CURRENT_DEVICE.store(default_device, Ordering::Release);
    LANDSCAPE.store(initial_landscape, Ordering::Release);
    let initial_frame = frame_spec(default_device, initial_landscape);
    lingxia_windows_sdk::set_windows_default_shell_tabbar_position(tabbar_position_for_device(
        default_device,
    ));
    lingxia_windows_sdk::set_initial_app_window_device_frame(initial_frame.clone());
    let mut app = lingxia_windows_sdk::WindowsApp::from_env()
        .with_window_size(initial_frame.screen_width, initial_frame.screen_height);
    if let Some(asset_dir) = asset_dir {
        app = app.with_asset_dir(asset_dir);
    }
    let home_app_id = lingxia_windows_sdk::start_default_host(app)?;
    install_runner_commands(home_app_id.clone());
    lingxia::dev::register_device_controller(Box::new(RunnerDeviceController));
    apply_default_device(home_app_id, default_device, initial_landscape);
    std::process::exit(lingxia_windows_sdk::run_message_loop());
}

fn install_launch_args_env() -> Option<std::path::PathBuf> {
    let mut asset_dir = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let Some(value) = args.next() else {
            break;
        };
        let env_key = match arg.as_str() {
            ARG_ASSET_DIR => {
                asset_dir = Some(std::path::PathBuf::from(&value));
                None
            }
            ARG_LXAPP_PATH => Some(ENV_LXAPP_PATH),
            ARG_DEV_WS_URL => Some(ENV_DEV_WS_URL),
            ARG_LINGXIAO_MOCK_DIR => Some(ENV_LINGXIAO_MOCK_DIR),
            ARG_RUNNER_DEVICE => Some(ENV_RUNNER_DEVICE),
            ARG_RUNNER_ENV => Some(ENV_RUNNER_ENV),
            ARG_RESOURCE_LXAPP_PATHS => Some(ENV_RESOURCE_LXAPP_PATHS),
            _ => None,
        };
        if let Some(env_key) = env_key {
            // Runs at process startup before LingXia starts any worker threads.
            unsafe {
                std::env::set_var(env_key, value);
            }
        }
    }
    install_dev_state_root_env();
    asset_dir
}

/// Isolates this dev runner's data + cache under its own lxapp directory so two
/// runners for different projects can run at once. Without it every runner uses
/// the single per-product state root (`%LOCALAPPDATA%\LingXia Runner`), whose
/// metadata database (redb, exclusive file lock) and WebView2 profile can only
/// be held by one process — the second runner dies with "Database already open.
/// Cannot acquire lock." Honors an explicit `LINGXIA_STATE_ROOT` if one is set.
fn install_dev_state_root_env() {
    if std::env::var_os(ENV_STATE_ROOT).is_some() {
        return;
    }
    let Ok(lxapp_path) = std::env::var(ENV_LXAPP_PATH) else {
        return;
    };
    if lxapp_path.trim().is_empty() {
        return;
    }
    let state_root = std::path::Path::new(&lxapp_path)
        .join(".lingxia")
        .join("runner")
        .join("state");
    // Runs at process startup before LingXia starts any worker threads.
    unsafe {
        std::env::set_var(ENV_STATE_ROOT, state_root);
    }
}

fn register_resource_lxapp_paths_from_env() {
    let Ok(raw) = std::env::var(ENV_RESOURCE_LXAPP_PATHS) else {
        return;
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return;
    }
    let paths = match serde_json::from_str::<Vec<ResourceLxAppPath>>(raw) {
        Ok(paths) => paths,
        Err(err) => {
            eprintln!("lingxia-runner: invalid {ENV_RESOURCE_LXAPP_PATHS}: {err}");
            return;
        }
    };
    for entry in paths {
        let app_id = entry.app_id.trim();
        if app_id.is_empty() {
            continue;
        }
        if !entry.path.join("lxapp.json").is_file() {
            eprintln!(
                "lingxia-runner: resource lxapp path for {app_id} is missing lxapp.json: {}",
                entry.path.display()
            );
            continue;
        }
        lxapp::register_dev_bundle_source(app_id.to_string(), entry.path);
    }
}

/// Exposes device switching to the devtool bridge (`lxapp.device.*`). The
/// runner owns the presets + window frame, so it implements the shared
/// `DeviceController` trait and registers it in `run()`; the bridge calls
/// through `lingxia::dev` without depending on the runner or the windows SDK.
struct RunnerDeviceController;

impl lingxia::dev::DeviceController for RunnerDeviceController {
    fn list(&self) -> Vec<lingxia::dev::DeviceEntry> {
        let current = CURRENT_DEVICE.load(Ordering::Acquire);
        presets()
            .iter()
            .enumerate()
            .map(|(index, preset)| lingxia::dev::DeviceEntry {
                id: preset.id().to_string(),
                name: preset.name.clone(),
                group: preset.group().to_string(),
                width: preset.width.max(0) as u32,
                height: preset.height.max(0) as u32,
                current: index == current,
            })
            .collect()
    }

    fn get(&self) -> lingxia::dev::DeviceState {
        let index = CURRENT_DEVICE.load(Ordering::Acquire);
        let landscape = LANDSCAPE.load(Ordering::Acquire);
        device_state(index, landscape)
    }

    fn set(&self, id: &str, landscape: Option<bool>) -> Result<lingxia::dev::DeviceState, String> {
        let index = presets()
            .iter()
            .position(|preset| preset.id() == id)
            .ok_or_else(|| format!("unknown device id: {id}"))?;
        // Default orientation matches the toolbar selector: tablets landscape,
        // phones/desktops portrait.
        let landscape = landscape.unwrap_or_else(|| is_tablet(index));
        apply_device(index, landscape)?;
        Ok(device_state(index, landscape))
    }
}

/// Builds the reported device state for `index`/`landscape`, swapping the
/// screen edges in landscape so the size matches what the frame shows.
fn device_state(index: usize, landscape: bool) -> lingxia::dev::DeviceState {
    let preset = &presets()[index];
    let (width, height) = if landscape {
        (preset.height, preset.width)
    } else {
        (preset.width, preset.height)
    };
    lingxia::dev::DeviceState {
        id: preset.id().to_string(),
        name: preset.name.clone(),
        group: preset.group().to_string(),
        width: width.max(0) as u32,
        height: height.max(0) as u32,
        landscape,
    }
}

fn apply_device(index: usize, landscape: bool) -> Result<(), String> {
    // Device switching, rotation, and DevTools live on the simulator frame's
    // toolbar (built in `frame_spec`), so the runner attaches no native menu
    // bar — the phone screen stays chrome-free like the macOS runner.
    //
    // Apply the frame and tab-bar shape together on the window thread. Splitting
    // these into separate immediate + posted updates lets layout briefly sync
    // against the previous device (e.g. iPhone status bar forcing bottom tabs
    // while switching to iPad).
    let tabbar_position = tabbar_position_for_device(index);
    lingxia_windows_sdk::set_windows_default_shell_tabbar_position(tabbar_position);

    let mut applied = false;
    for app in lxapp::list_lxapps() {
        if app.status == "opened" {
            lingxia_windows_sdk::set_windows_shell_tabbar_position(&app.appid, tabbar_position);
            if let Err(err) = apply_device_to_app(&app.appid, index, landscape) {
                eprintln!(
                    "lingxia-runner: failed to apply device to {}: {err}",
                    app.appid
                );
            } else {
                applied = true;
            }
        }
    }
    if !applied {
        return Err("no opened lxapp is ready for device frame".to_string());
    }
    CURRENT_DEVICE.store(index, Ordering::Release);
    LANDSCAPE.store(landscape, Ordering::Release);
    Ok(())
}

fn apply_device_to_app(appid: &str, index: usize, landscape: bool) -> Result<(), String> {
    lingxia_windows_sdk::set_app_window_device_frame_and_tabbar_position(
        appid,
        frame_spec(index, landscape),
        tabbar_position_for_device(index),
    )
}

fn tabbar_position_for_device(index: usize) -> WindowsShellTabBarPosition {
    if is_phone(index) {
        WindowsShellTabBarPosition::Bottom
    } else {
        WindowsShellTabBarPosition::Left
    }
}

fn install_runner_commands(home_app_id: String) {
    // Handles both the simulator toolbar's device selector and its DevTools
    // action (the SDK routes frame-toolbar commands through this handler).
    lingxia_windows_sdk::set_windows_app_menu_command_handler(std::sync::Arc::new(
        move |command| {
            if command == OPEN_DEVTOOLS_COMMAND {
                if let Err(err) = lingxia_windows_sdk::open_current_page_devtools(&home_app_id) {
                    eprintln!("lingxia-runner: failed to open DevTools: {err}");
                }
                return;
            }

            if command == ROTATE_COMMAND {
                let index = CURRENT_DEVICE.load(Ordering::Acquire);
                let landscape = !LANDSCAPE.load(Ordering::Acquire);
                if let Err(err) = apply_device(index, landscape) {
                    eprintln!("lingxia-runner: failed to rotate device: {err}");
                }
                return;
            }

            if command == ABOUT_COMMAND {
                let target = current_or_home_app_id(&home_app_id);
                if is_phone(CURRENT_DEVICE.load(Ordering::Acquire))
                    && let Err(err) = show_lxapp_info_sheet(&target)
                {
                    eprintln!("lingxia-runner: failed to show lxapp info: {err}");
                }
                return;
            }

            if command == CAPSULE_CLOSE_COMMAND {
                if let Err(err) = close_current_lxapp(&home_app_id) {
                    eprintln!("lingxia-runner: failed to close current lxapp: {err}");
                }
                return;
            }

            if command == RESTART_LXAPP_COMMAND {
                let target = current_or_home_app_id(&home_app_id);
                if let Err(err) = restart_lxapp(&target, false) {
                    eprintln!("lingxia-runner: failed to restart lxapp: {err}");
                }
                return;
            }

            if command == CLEAN_CACHE_COMMAND {
                let target = current_or_home_app_id(&home_app_id);
                if let Err(err) = restart_lxapp(&target, true) {
                    eprintln!("lingxia-runner: failed to clean cache + restart lxapp: {err}");
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
            // Tablets default to landscape, phones/desktops to portrait.
            if let Err(err) = apply_device(index, is_tablet(index)) {
                eprintln!(
                    "lingxia-runner: failed to switch to {}: {err}",
                    presets()[index].name
                );
            }
        },
    ));
}

fn current_or_home_app_id(home_app_id: &str) -> String {
    let (appid, _, _) = lxapp::get_current_lxapp();
    if appid.is_empty() {
        home_app_id.to_string()
    } else {
        appid
    }
}

fn close_current_lxapp(home_app_id: &str) -> Result<(), String> {
    let target = current_or_home_app_id(home_app_id);
    let app = lxapp::try_get(&target).ok_or_else(|| format!("lxapp is not active: {target}"))?;
    let _ = app.on_lxapp_event(LxAppUiEventType::CapsuleClick, "close".to_string());
    Ok(())
}

fn show_lxapp_info_sheet(appid: &str) -> Result<(), String> {
    let app = lxapp::try_get(appid).ok_or_else(|| format!("lxapp is not active: {appid}"))?;
    let info = app.runtime_info();
    lingxia_windows_sdk::show_device_frame_info_sheet(
        appid,
        lingxia_windows_sdk::WindowsDeviceFrameInfoSheet {
            title: info.app_name,
            version: info.version,
            badge: release_badge(&info.release_type),
            actions: vec![
                lingxia_windows_sdk::WindowsDeviceFrameSheetAction {
                    command: CLEAN_CACHE_COMMAND,
                    label: "Clean Cache && Restart".to_string(),
                    icon: lingxia_windows_sdk::WindowsDesignIcon::CleanCache,
                },
                lingxia_windows_sdk::WindowsDeviceFrameSheetAction {
                    command: RESTART_LXAPP_COMMAND,
                    label: "Restart lxapp".to_string(),
                    icon: lingxia_windows_sdk::WindowsDesignIcon::Restart,
                },
            ],
        },
    )
}

/// Maps the lxapp release channel to the info-sheet header badge. Owned by the
/// runner so the SDK device frame stays free of lxapp/runner semantics.
fn release_badge(release_type: &str) -> Option<lingxia_windows_sdk::WindowsDeviceFrameBadge> {
    match release_type.to_ascii_lowercase().as_str() {
        "developer" => Some(lingxia_windows_sdk::WindowsDeviceFrameBadge {
            text: "DEV".to_string(),
            foreground: 0x1D4ED8,
            background: 0xDBEAFE,
        }),
        "preview" => Some(lingxia_windows_sdk::WindowsDeviceFrameBadge {
            text: "PRE".to_string(),
            foreground: 0xB45309,
            background: 0xFFEDD5,
        }),
        _ => None,
    }
}

/// In-place lxapp restart: recreate the JS app service (re-running `onLaunch`,
/// so globalData/network state is rebuilt) and reload the page WebView in the
/// existing window. The device-frame window and its overlays stay put — no
/// teardown or flash, unlike the full-lifecycle restart which recreates the
/// host window and makes the whole app vanish and reappear.
///
/// `clean_cache` first clears the lxapp's user cache (the capsule sheet's
/// "Clean Cache && Restart").
fn restart_lxapp(appid: &str, clean_cache: bool) -> Result<(), String> {
    let app = lxapp::try_get(appid).ok_or_else(|| format!("lxapp is not active: {appid}"))?;
    if clean_cache {
        app.clear_user_cache().map_err(|err| err.to_string())?;
    }
    app.restart_in_place().map_err(|err| err.to_string())
}

fn apply_default_device(home_app_id: String, default_device: usize, landscape: bool) {
    std::thread::spawn(move || {
        for attempt in 0..80 {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            if apply_device_to_app(&home_app_id, default_device, landscape).is_ok() {
                CURRENT_DEVICE.store(default_device, Ordering::Release);
                LANDSCAPE.store(landscape, Ordering::Release);
                return;
            }
        }
        eprintln!("lingxia-runner: home page webview never became ready for the device frame");
    });
}
