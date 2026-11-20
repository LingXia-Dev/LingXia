fn main() -> lingxia_windows_sdk::Result<()> {
    // Registration only publishes the addon and its host-owned CLI commands;
    // it does not initialize a window, service, or database.
    host::lingxia_register_host_addon();
    // Answer product CLI invocations before opening windows or databases.
    #[cfg(feature = "control")]
    if let Some(code) = host::run_cli_if_invoked() {
        std::process::exit(code);
    }
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
