use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub fn configure_windows_app() {
    configure_windows_app_with_manifest("app.manifest");
}

pub fn configure_windows_app_with_manifest(manifest: impl AsRef<Path>) {
    if !is_windows_target() {
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
    copy_assets_to_target_profile_dir();
}

fn is_windows_target() -> bool {
    std::env::var("CARGO_CFG_TARGET_OS").ok().as_deref() == Some("windows")
        || std::env::var_os("CARGO_CFG_WINDOWS").is_some()
}

fn resolve_manifest_path(manifest: &Path) -> PathBuf {
    if manifest.is_absolute() {
        return manifest.to_path_buf();
    }
    std::env::current_dir()
        .expect("build script current directory")
        .join(manifest)
}

fn copy_assets_to_target_profile_dir() {
    let Some(manifest_dir) = std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from) else {
        return;
    };
    let source = manifest_dir.join("assets");
    if !source.is_dir() {
        return;
    }
    let Some(target_dir) = target_profile_dir() else {
        return;
    };
    let destination = target_dir.join("assets");
    if destination.exists() {
        fs::remove_dir_all(&destination).unwrap_or_else(|error| {
            panic!(
                "failed to clear stale Windows app assets at {}: {error}",
                destination.display()
            )
        });
    }
    copy_dir(&source, &destination).unwrap_or_else(|error| {
        panic!(
            "failed to copy Windows app assets from {} to {}: {error}",
            source.display(),
            destination.display()
        )
    });
}

fn target_profile_dir() -> Option<PathBuf> {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR")?);
    out_dir.ancestors().nth(3).map(Path::to_path_buf)
}

fn copy_dir(source: &Path, destination: &Path) -> io::Result<()> {
    println!("cargo:rerun-if-changed={}", source.display());
    fs::create_dir_all(destination)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_name = entry.file_name();
        if file_name == ".lingxia" {
            continue;
        }
        let source_path = entry.path();
        let destination_path = destination.join(file_name);
        println!("cargo:rerun-if-changed={}", source_path.display());
        if entry.file_type()?.is_dir() {
            copy_dir(&source_path, &destination_path)?;
        } else {
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &destination_path)?;
        }
    }

    Ok(())
}
