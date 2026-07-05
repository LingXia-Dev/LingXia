fn main() -> lingxia_windows_sdk::Result<()> {
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
    Ok(())
}
