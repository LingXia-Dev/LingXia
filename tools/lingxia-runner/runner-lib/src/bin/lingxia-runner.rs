//! LingXia Runner for Windows — the dev runner that `lingxia dev` launches
//! for standalone lxapp projects (the counterpart of the macOS
//! "LingXia Runner.app").
//!
//! The runner hosts the lxapp in a plain phone-aspect window (standard
//! runtime only — no shell chrome, device frame, or devtools panel yet) and
//! is configured entirely through the environment:
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
#[cfg(target_os = "windows")]
const RUNNER_WINDOW_SIZE: (i32, i32) = (420, 880);

#[cfg(target_os = "windows")]
fn main() -> lingxia_windows::Result<()> {
    lingxia::register_host_addon(Box::new(RunnerDevtoolAddon));

    let app = lingxia_windows::WindowsApp::from_env()
        .with_window_size(RUNNER_WINDOW_SIZE.0, RUNNER_WINDOW_SIZE.1);
    lingxia_windows::init(app)?;
    std::process::exit(lingxia_windows::run_message_loop());
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!(
        "lingxia-runner is the Windows dev runner; on macOS use the LingXia Runner app instead."
    );
    std::process::exit(1);
}
