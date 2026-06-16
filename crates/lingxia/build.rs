//! Build script for lingxia.
//!
//! This generates Swift Bridge files for apple SDK. Run with:
//!   LINGXIA_GENERATE_BRIDGE=1 cargo build --target aarch64-apple-ios
//!   LINGXIA_GENERATE_BRIDGE=1 cargo build --target aarch64-apple-darwin
//!
//! For normal builds (Android, user apps), this does nothing.

use std::env;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    // HarmonyOS NAPI setup
    if target.contains("linux") && target_env == "ohos" {
        napi_build_ohos::setup();
    }

    // Keep Apple bridge sources in sync for workspace builds. External consumers can still
    // force generation explicitly with LINGXIA_GENERATE_BRIDGE=1.
    if should_generate_swift_bridge(&target) {
        generate_swift_bridge();
    }
}

fn workspace_root(manifest_dir: &Path) -> &Path {
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> layout expected for workspace members")
}

fn should_generate_swift_bridge(target: &str) -> bool {
    if !target.contains("apple") {
        return false;
    }

    if env::var("LINGXIA_GENERATE_BRIDGE").is_ok() {
        return true;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    workspace_root(&manifest_dir)
        .join("lingxia-sdk")
        .join("apple")
        .join("Sources")
        .exists()
}

// Swift bridge generation relies on `swift_bridge_build`, an apple-host-only
// build dependency. The build script compiles for the *host*, so on non-apple
// hosts (e.g. a Linux CI runner cross-building the lib for Android) this stub
// keeps build.rs compiling. It is never reached for non-apple targets, since
// should_generate_swift_bridge() requires `target.contains("apple")`.
#[cfg(not(any(target_os = "ios", target_os = "macos")))]
fn generate_swift_bridge() {
    unreachable!("Swift bridge generation is only supported on macOS/iOS hosts");
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn generate_swift_bridge() {
    println!("cargo:rerun-if-changed=src/ffi/apple.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");

    let package_name = "LingXiaRustAPI";
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let generated_dir = workspace_root(&manifest_dir).join("lingxia-sdk/apple/Sources/generated");
    let lib_dir = generated_dir.join("LingXiaRustAPI");
    let temp_dir = PathBuf::from(env::var("OUT_DIR").unwrap()).join("swift-bridge-generated");

    fs::create_dir_all(&lib_dir).expect("Failed to create directory");
    fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");

    swift_bridge_build::parse_bridges(vec!["src/ffi/apple.rs"])
        .write_all_concatenated(&temp_dir, package_name);

    // Add imports to generated Swift files
    for path in [
        temp_dir
            .join(package_name)
            .join(format!("{}.swift", package_name)),
        temp_dir.join("SwiftBridgeCore.swift"),
    ] {
        if let Ok(content) = fs::read_to_string(&path)
            && !content.contains("import CLingXiaRustAPI")
        {
            let new_content = format!("import Foundation\nimport CLingXiaRustAPI\n\n{}", content);
            write_if_changed(&path, new_content.as_bytes());
        }
    }

    // Write module map
    write_if_changed(&temp_dir.join("module.modulemap"), format!(
        "module CLingXiaRustAPI {{\n    header \"../SwiftBridgeCore.h\"\n    header \"{0}/{0}.h\"\n    export *\n}}",
        package_name
    ).as_bytes());

    // Move shared files after all in-place fixes are applied.
    for file in ["SwiftBridgeCore.swift", "SwiftBridgeCore.h"] {
        let src = temp_dir.join(file);
        if src.exists() {
            copy_if_changed(&src, &generated_dir.join(file));
        }
    }

    copy_if_changed(
        &temp_dir
            .join(package_name)
            .join(format!("{}.h", package_name)),
        &lib_dir
            .join(package_name)
            .join(format!("{}.h", package_name)),
    );
    copy_if_changed(
        &temp_dir
            .join(package_name)
            .join(format!("{}.swift", package_name)),
        &lib_dir
            .join(package_name)
            .join(format!("{}.swift", package_name)),
    );
    copy_if_changed(
        &temp_dir.join("module.modulemap"),
        &lib_dir.join("module.modulemap"),
    );

    println!(
        "cargo:warning=Generated Swift bridge to {}",
        lib_dir.display()
    );
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn write_if_changed(path: &Path, contents: &[u8]) {
    match fs::read(path) {
        Ok(existing) if existing == contents => return,
        _ => {}
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("Failed to create parent directory");
    }
    fs::write(path, contents).expect("Failed to write generated file");
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn copy_if_changed(source: &Path, destination: &Path) {
    let source_bytes = fs::read(source)
        .unwrap_or_else(|_| panic!("Failed to read generated source file {}", source.display()));

    match fs::read(destination) {
        Ok(existing) if existing == source_bytes => return,
        _ => {}
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("Failed to create destination directory");
    }
    fs::write(destination, source_bytes).unwrap_or_else(|_| {
        panic!(
            "Failed to copy generated file {} to {}",
            source.display(),
            destination.display()
        )
    });
}
