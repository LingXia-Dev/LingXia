#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

use std::path::PathBuf;

fn main() -> lingxia_windows::Result<()> {
    showcase_host::lingxia_register_host_addon();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let product_name =
        std::env::var("LINGXIA_PRODUCT_NAME").unwrap_or_else(|_| "LingXia".to_string());
    let app_id = std::env::var("LINGXIA_APP_ID")
        .unwrap_or_else(|_| "app.lingxia.example.lxapp".to_string());
    let state_root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(&product_name);
    let asset_dir = std::env::var_os("LINGXIA_ASSET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("assets"));

    let app = lingxia_windows::WindowsApp::new(
        state_root.join("data"),
        state_root.join("cache"),
        asset_dir,
    )
    .with_app_identifier(app_id)
    .with_product_name(product_name);

    let _home_app_id = lingxia_windows::init(app)?;
    #[cfg(target_os = "windows")]
    {
        let _ = lingxia_windows::run_message_loop();
    }
    Ok(())
}
