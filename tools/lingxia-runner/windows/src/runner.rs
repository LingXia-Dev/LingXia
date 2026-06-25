use crate::device::{
    ABOUT_COMMAND, CLEAN_CACHE_COMMAND, DEVICE_COMMAND_BASE, OPEN_DEVTOOLS_COMMAND, QUIT_COMMAND,
    RESTART_LXAPP_COMMAND, ROTATE_COMMAND, default_device_index, frame_spec, is_phone, is_tablet,
    presets,
};
use lingxia_windows_sdk::WindowsShellTabBarPosition;
use lxapp::LxAppDelegate;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// The device + orientation the simulator currently shows. The toolbar's
/// rotate button toggles `LANDSCAPE` for the active `CURRENT_DEVICE`; picking a
/// device from the selector resets to portrait.
static CURRENT_DEVICE: AtomicUsize = AtomicUsize::new(0);
static LANDSCAPE: AtomicBool = AtomicBool::new(false);

const ARG_ASSET_DIR: &str = "--asset-dir";
const ARG_LXAPP_PATH: &str = "--lxapp-path";
const ARG_DEV_WS_URL: &str = "--dev-ws-url";
const ENV_ASSET_DIR: &str = "LINGXIA_ASSET_DIR";
const ENV_LXAPP_PATH: &str = "LINGXIA_LXAPP_PATH";
const ENV_DEV_WS_URL: &str = "LINGXIA_DEV_WS_URL";

struct RunnerDevtoolAddon;

impl lingxia::HostAddon for RunnerDevtoolAddon {
    fn start_services(&self) {
        lingxia_devtool::start_devtool_bridge_from_env();
    }
}

pub(crate) fn run() -> lingxia_windows_sdk::Result<()> {
    install_launch_args_env();
    lingxia::register_host_addon(Box::new(RunnerDevtoolAddon));

    let default_device = default_device_index();
    let initial_frame = frame_spec(default_device, false);
    lingxia_windows_sdk::set_windows_default_shell_tabbar_position(tabbar_position_for_device(
        default_device,
    ));
    lingxia_windows_sdk::set_initial_app_window_device_frame(initial_frame.clone());
    let app = lingxia_windows_sdk::WindowsApp::from_env()
        .with_window_size(initial_frame.screen_width, initial_frame.screen_height);
    let home_app_id = lingxia_windows_sdk::init(app)?;
    install_runner_commands(home_app_id.clone());
    apply_default_device(home_app_id, default_device);
    std::process::exit(lingxia_windows_sdk::run_message_loop());
}

fn install_launch_args_env() {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let Some(value) = args.next() else {
            break;
        };
        let env_key = match arg.as_str() {
            ARG_ASSET_DIR => Some(ENV_ASSET_DIR),
            ARG_LXAPP_PATH => Some(ENV_LXAPP_PATH),
            ARG_DEV_WS_URL => Some(ENV_DEV_WS_URL),
            _ => None,
        };
        if let Some(env_key) = env_key {
            // Runs at process startup before LingXia starts any worker threads.
            unsafe {
                std::env::set_var(env_key, value);
            }
        }
    }
}

fn apply_device(home_app_id: &str, index: usize, landscape: bool) -> Result<(), String> {
    // Device switching, rotation, and DevTools live on the simulator frame's
    // toolbar (built in `frame_spec`), so the runner attaches no native menu
    // bar — the phone screen stays chrome-free like the macOS runner.
    //
    // Set the tab-bar position *before* applying the frame: a phone uses a
    // bottom bar while a tablet/desktop uses a left sidebar, which changes the
    // content rect. Applying the frame last means its layout pass (which
    // resizes the webview) uses the final tab-bar shape, so switching e.g.
    // iPad -> iPhone re-renders the page at the right size immediately.
    lingxia_windows_sdk::set_windows_shell_tabbar_position(
        home_app_id,
        tabbar_position_for_device(index),
    );
    lingxia_windows_sdk::set_app_window_device_frame(
        home_app_id,
        Some(frame_spec(index, landscape)),
    )?;
    CURRENT_DEVICE.store(index, Ordering::Release);
    LANDSCAPE.store(landscape, Ordering::Release);
    Ok(())
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
                if let Err(err) = apply_device(&home_app_id, index, landscape) {
                    eprintln!("lingxia-runner: failed to rotate device: {err}");
                }
                return;
            }

            if command == ABOUT_COMMAND {
                if is_phone(CURRENT_DEVICE.load(Ordering::Acquire))
                    && let Err(err) = show_lxapp_info_sheet(&home_app_id)
                {
                    eprintln!("lingxia-runner: failed to show lxapp info: {err}");
                }
                return;
            }

            if command == QUIT_COMMAND {
                // Capsule close circle quits the single-app emulator, mirroring
                // the macOS runner (PR #28).
                if let Err(err) = lingxia::app::exit() {
                    eprintln!("lingxia-runner: failed to quit: {err}");
                }
                return;
            }

            if command == RESTART_LXAPP_COMMAND {
                if let Err(err) = restart_lxapp(&home_app_id, false) {
                    eprintln!("lingxia-runner: failed to restart lxapp: {err}");
                }
                return;
            }

            if command == CLEAN_CACHE_COMMAND {
                if let Err(err) = restart_lxapp(&home_app_id, true) {
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
            if let Err(err) = apply_device(&home_app_id, index, is_tablet(index)) {
                eprintln!(
                    "lingxia-runner: failed to switch to {}: {err}",
                    presets()[index].name
                );
            }
        },
    ));
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

/// Canonical lxapp restart, matching the macOS runner. The full lifecycle
/// recreates the lxapp and reopens it to its initial route as a *fresh session*,
/// so `onLaunch` re-runs and app-service state (globalData, network fetches)
/// is rebuilt. The previous in-place path only recreated the JS service and
/// reloaded the current page on the same session, so the page rendered before
/// the service re-initialized — leaving the screen with empty global state.
///
/// `clean_cache` first clears the lxapp's user cache (the capsule sheet's
/// "Clean Cache && Restart"). The device frame is re-armed so the recreated
/// host window is framed from first paint, and the active device + tab-bar are
/// re-applied once the fresh session's page WebView is ready.
fn restart_lxapp(home_app_id: &str, clean_cache: bool) -> Result<(), String> {
    let app =
        lxapp::try_get(home_app_id).ok_or_else(|| format!("lxapp is not active: {home_app_id}"))?;
    if clean_cache {
        app.clear_user_cache().map_err(|err| err.to_string())?;
    }
    let index = CURRENT_DEVICE.load(Ordering::Acquire);
    let landscape = LANDSCAPE.load(Ordering::Acquire);
    let old_session = app.session_id();
    // Arm the frame + tab-bar shape for the next host window the runtime creates
    // (the recreated session) so it comes up framed with no unframed flash.
    lingxia_windows_sdk::set_windows_default_shell_tabbar_position(tabbar_position_for_device(index));
    lingxia_windows_sdk::set_initial_app_window_device_frame(frame_spec(index, landscape));
    app.restart().map_err(|err| err.to_string())?;
    // `restart()` waits for the old session to report `Closed` before it
    // recreates. On Windows the host hides the lxapp window but never reports
    // that close (macOS reports it from the window-close path via FFI), so the
    // wait hits its ~1.5s fallback. Drive the close ourselves so the recreate
    // happens immediately. The call is session-guarded (no-op once the fresh
    // session is live), so it only ends the dying session's wait.
    app.on_lxapp_closed(old_session);
    reapply_device_after_restart(home_app_id.to_string(), old_session, index, landscape);
    Ok(())
}

/// Waits (off the UI thread) for the restarted lxapp to reach a fresh opened
/// session whose page WebView is ready, then re-applies the active device frame
/// and tab-bar position to the recreated host window. Mirrors the macOS runner's
/// `reopenCurrentAppAfterLifecycleAction` wait loop.
fn reapply_device_after_restart(
    home_app_id: String,
    old_session: u64,
    index: usize,
    landscape: bool,
) {
    std::thread::spawn(move || {
        for attempt in 0..200 {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            let Some(app) = lxapp::try_get(&home_app_id) else {
                continue;
            };
            // Only re-frame the fresh session, never the dying one.
            if app.session_id() == old_session || !app.is_opened() {
                continue;
            }
            if apply_device(&home_app_id, index, landscape).is_ok() {
                return;
            }
        }
        eprintln!("lingxia-runner: restarted home page never became ready for the device frame");
    });
}

fn apply_default_device(home_app_id: String, default_device: usize) {
    std::thread::spawn(move || {
        for attempt in 0..80 {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            if apply_device(&home_app_id, default_device, false).is_ok() {
                return;
            }
        }
        eprintln!("lingxia-runner: home page webview never became ready for the device frame");
    });
}
