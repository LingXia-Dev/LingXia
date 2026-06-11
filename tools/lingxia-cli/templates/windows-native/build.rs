fn main() {
    // Embed the application manifest (Win8+ supportedOS declarations); the
    // shell's layered corner-cap child windows require it at runtime.
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
