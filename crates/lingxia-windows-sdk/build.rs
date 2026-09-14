//! Keep the SVGs this crate embeds in sync with repo-root `design/icons/svg`
//! when both trees are present (git checkout). crates.io has only this crate.
fn main() {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let crate_icons = manifest.join("icons/svg");
    println!("cargo:rerun-if-changed={}", crate_icons.display());
    let back = crate_icons.join("icon_back.svg");
    if !back.is_file() {
        panic!(
            "missing {} — cargo package cannot see repo-root design/icons; copy SVGs into crates/lingxia-windows-sdk/icons/svg",
            back.display()
        );
    }
    let design = manifest.join("../../design/icons/svg");
    println!("cargo:rerun-if-changed={}", design.display());
    if !design.is_dir() {
        return;
    }
    for entry in std::fs::read_dir(&crate_icons).expect("read crate icons") {
        let entry = entry.expect("icon dirent");
        let name = entry.file_name();
        let packed = std::fs::read(entry.path()).expect("read packed icon");
        let canonical = design.join(&name);
        let src = std::fs::read(&canonical).unwrap_or_else(|_| {
            panic!(
                "{} is in the crate but missing from design/icons/svg",
                name.to_string_lossy()
            )
        });
        if packed != src {
            panic!(
                "{} drifted from design/icons/svg — recopy into crates/lingxia-windows-sdk/icons/svg",
                name.to_string_lossy()
            );
        }
    }
}
