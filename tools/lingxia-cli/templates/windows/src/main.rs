fn main() -> lingxia_windows_sdk::Result<()> {
    // The executable is also this product's command line. Answered first,
    // before any window or database is opened, so a command never collides
    // with an instance already running.
    #[cfg(feature = "control")]
    if let Some(code) = host::run_cli_if_invoked() {
        std::process::exit(code);
    }
    host::lingxia_register_host_addon();
    let app = debug_asset_dir()
        .map(|asset_dir| lingxia_windows_sdk::WindowsApp::from_env().with_asset_dir(asset_dir))
        .unwrap_or_else(lingxia_windows_sdk::WindowsApp::from_env);
    let _ = lingxia_windows_sdk::start_default_host(app)?;
    std::process::exit(lingxia_windows_sdk::run_message_loop());
}

fn debug_asset_dir() -> Option<&'static str> {
    if cfg!(debug_assertions) {
        option_env!("LINGXIA_WINDOWS_DEBUG_ASSET_DIR")
    } else {
        None
    }
}
