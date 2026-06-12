fn main() {
    // Embed the application manifest (Win8+ supportedOS declarations) into
    // the Windows runner binary; the runtime's layered corner-cap child
    // windows require it (without it the OS runs the process in Vista
    // compatibility context).
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("app.manifest");
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
            manifest.display()
        );
        println!("cargo:rerun-if-changed=app.manifest");
    }
}
