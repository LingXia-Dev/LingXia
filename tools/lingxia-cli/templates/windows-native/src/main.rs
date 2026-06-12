fn main() -> lingxia_windows::Result<()> {
    host::lingxia_register_host_addon();
    let _ = lingxia_windows::quick_start()?;
    Ok(())
}
