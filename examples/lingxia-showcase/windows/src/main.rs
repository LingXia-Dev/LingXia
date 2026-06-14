fn main() -> lingxia_windows_sdk::Result<()> {
    host::lingxia_register_host_addon();
    let _ = lingxia_windows_sdk::quick_start()?;
    Ok(())
}
