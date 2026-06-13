use std::path::{Path, PathBuf};

pub fn configure_windows_app() {
    configure_windows_app_with_manifest("app.manifest");
}

pub fn configure_windows_app_with_manifest(manifest: impl AsRef<Path>) {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }

    let manifest = manifest.as_ref();
    let manifest_path = resolve_manifest_path(manifest);
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
        manifest_path.display()
    );
    println!("cargo:rustc-link-arg-bins=/SUBSYSTEM:WINDOWS");
    println!("cargo:rustc-link-arg-bins=/ENTRY:mainCRTStartup");
    println!("cargo:rerun-if-changed={}", manifest.display());
}

fn resolve_manifest_path(manifest: &Path) -> PathBuf {
    if manifest.is_absolute() {
        return manifest.to_path_buf();
    }
    std::env::current_dir()
        .expect("build script current directory")
        .join(manifest)
}
