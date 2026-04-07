use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=src/apple/ffi.rs");

    let target = env::var("TARGET").unwrap_or_default();

    let env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target.contains("linux") && env.eq("ohos") {
        napi_build_ohos::setup();
        println!("cargo:rustc-link-lib=dylib=ohvibrator.z");
        println!("cargo:rustc-link-lib=dylib=location_ndk");
    }

    if target.contains("apple") {
        let package_name = "LingXiaSwiftAPI";
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

        // Generate to lingxia-sdk/apple/Sources/generated/platform
        let lingxia_root = workspace_root(&manifest_dir);
        let sources_dir = lingxia_root
            .join("lingxia-sdk")
            .join("apple")
            .join("Sources");
        let generated_dir = sources_dir.join("generated");
        let platform_dir = generated_dir.join("LingXiaSwiftAPI");
        let generated_tmp_dir = out_dir.join("swift-bridge-generated");

        // Create destination directories
        std::fs::create_dir_all(&platform_dir).expect("Failed to create platform directory");
        std::fs::create_dir_all(&generated_tmp_dir)
            .expect("Failed to create temporary platform directory");

        // Generate Swift bridge files outside the Swift package source tree first.
        let bridges = swift_bridge_build::parse_bridges(vec!["src/apple/ffi.rs"]);
        bridges.write_all_concatenated(&generated_tmp_dir, package_name);

        // Remove SwiftBridgeCore files since lib will generate them
        let core_swift = generated_tmp_dir.join("SwiftBridgeCore.swift");
        if core_swift.exists() {
            fs::remove_file(&core_swift).expect("Failed to remove SwiftBridgeCore.swift");
        }

        let core_h = generated_tmp_dir.join("SwiftBridgeCore.h");
        if core_h.exists() {
            fs::remove_file(&core_h).expect("Failed to remove SwiftBridgeCore.h");
        }

        // Fix the generated header file to include stdint.h for uintptr_t
        let header_file = generated_tmp_dir
            .join(package_name)
            .join(format!("{}.h", package_name));
        if header_file.exists()
            && let Ok(contents) = fs::read_to_string(&header_file)
            && !contents.contains("#include <stdint.h>")
        {
            let new_contents = contents.replace(
                "#include <stdbool.h>",
                "#include <stdbool.h>\n#include <stdint.h>",
            );
            fs::write(&header_file, new_contents).expect("Failed to update header file");
        }

        // Add import CLingXiaPlatform to the generated Swift files
        let add_import_if_missing = |file_path: &std::path::Path, file_name: &str| {
            if let Ok(contents) = fs::read_to_string(file_path)
                && !contents.contains("import CLingXiaSwiftAPI")
            {
                let new_contents =
                    format!("import Foundation\nimport CLingXiaSwiftAPI\n\n{}", contents);
                fs::write(file_path, new_contents).unwrap_or_else(|_| {
                    panic!("Failed to add import statement to {} file", file_name)
                });
            }
        };

        // 1. Add to main LingXiaPlatform.swift file
        let swift_file_path = generated_tmp_dir
            .join(package_name)
            .join(format!("{}.swift", package_name));
        add_import_if_missing(&swift_file_path, "LingXiaPlatform.swift");

        // Create platform-specific module.modulemap
        let platform_modulemap_content = format!(
            r#"module CLingXiaSwiftAPI {{
    header "../SwiftBridgeCore.h"
    header "{package_name}/{package_name}.h"
    export *
}}"#,
            package_name = package_name
        );

        let generated_modulemap = generated_tmp_dir.join("module.modulemap");
        write_if_changed(&generated_modulemap, &platform_modulemap_content);

        copy_if_changed(
            &generated_tmp_dir
                .join(package_name)
                .join(format!("{}.h", package_name)),
            &platform_dir
                .join(package_name)
                .join(format!("{}.h", package_name)),
        );
        copy_if_changed(
            &generated_tmp_dir
                .join(package_name)
                .join(format!("{}.swift", package_name)),
            &platform_dir
                .join(package_name)
                .join(format!("{}.swift", package_name)),
        );
        copy_if_changed(&generated_modulemap, &platform_dir.join("module.modulemap"));

        println!(
            "cargo:warning=LingXiaSwiftAPI Swift bridge files generated to {}",
            platform_dir.display()
        );
    }
}

fn workspace_root(manifest_dir: &Path) -> &Path {
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> layout expected for workspace members")
}

fn write_if_changed(path: &Path, contents: &str) {
    match fs::read(path) {
        Ok(existing) if existing == contents.as_bytes() => return,
        _ => {}
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("Failed to create parent directory");
    }
    fs::write(path, contents).expect("Failed to write generated file");
}

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
