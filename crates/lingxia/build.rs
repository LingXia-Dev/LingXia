//! Build script for lingxia.
//!
//! This generates Swift Bridge files for apple SDK. Run with:
//!   LINGXIA_GENERATE_BRIDGE=1 cargo build --target aarch64-apple-ios
//!   LINGXIA_GENERATE_BRIDGE=1 cargo build --target aarch64-apple-darwin
//!
//! For normal builds (Android, user apps), this does nothing.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    // HarmonyOS NAPI setup
    if target.contains("linux") && target_env == "ohos" {
        napi_build_ohos::setup();
    }

    // iOS Swift Bridge generation - only when explicitly requested
    if target.contains("apple") && env::var("LINGXIA_GENERATE_BRIDGE").is_ok() {
        generate_swift_bridge();
    }
}

fn workspace_root(manifest_dir: &Path) -> &Path {
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> layout expected for workspace members")
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn generate_swift_bridge() {
    println!("cargo:rerun-if-changed=src/apple.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");

    let package_name = "LingXiaRustAPI";
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let generated_dir = workspace_root(&manifest_dir).join("lingxia-sdk/apple/Sources/generated");
    let lib_dir = generated_dir.join("LingXiaRustAPI");

    fs::create_dir_all(&lib_dir).expect("Failed to create directory");

    swift_bridge_build::parse_bridges(vec!["src/apple.rs"])
        .write_all_concatenated(&lib_dir, package_name);

    // Move shared files
    for file in ["SwiftBridgeCore.swift", "SwiftBridgeCore.h"] {
        let src = lib_dir.join(file);
        if src.exists() {
            fs::rename(&src, generated_dir.join(file)).ok();
        }
    }

    // Add imports to generated Swift files
    for path in [
        lib_dir
            .join(package_name)
            .join(format!("{}.swift", package_name)),
        generated_dir.join("SwiftBridgeCore.swift"),
    ] {
        if let Ok(content) = fs::read_to_string(&path) {
            if !content.contains("import CLingXiaRustAPI") {
                let new_content =
                    format!("import Foundation\nimport CLingXiaRustAPI\n\n{}", content);
                fs::write(&path, new_content).ok();
            }
        }
    }

    // Write module map
    fs::write(lib_dir.join("module.modulemap"), format!(
        "module CLingXiaRustAPI {{\n    header \"../SwiftBridgeCore.h\"\n    header \"{0}/{0}.h\"\n    export *\n}}",
        package_name
    )).ok();

    println!(
        "cargo:warning=Generated Swift bridge to {}",
        lib_dir.display()
    );
}
