use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/apple.rs");

    let target = env::var("TARGET").unwrap_or_default();

    let env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target.contains("linux") && env.eq("ohos") {
        napi_build_ohos::setup();
    }

    if target.contains("apple") {
        let package_name = "LingXiaRustAPI";
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

        // Generate to lingxia-sdk/apple/Sources/generated/lib
        let lingxia_root = manifest_dir.parent().unwrap();
        let sources_dir = lingxia_root
            .join("lingxia-sdk")
            .join("apple")
            .join("Sources");
        let generated_dir = sources_dir.join("generated");
        let lib_dir = generated_dir.join("LingXiaRustAPI");

        // Create destination directories
        std::fs::create_dir_all(&lib_dir).expect("Failed to create lib directory");

        // Generate Swift bridge files to lib directory
        swift_bridge_build::parse_bridges(vec!["src/apple.rs"])
            .write_all_concatenated(&lib_dir, package_name);

        // Move SwiftBridgeCore files to generated root for sharing
        let lib_core_swift = lib_dir.join("SwiftBridgeCore.swift");
        let shared_core_swift = generated_dir.join("SwiftBridgeCore.swift");
        if lib_core_swift.exists() {
            fs::rename(&lib_core_swift, &shared_core_swift)
                .expect("Failed to move SwiftBridgeCore.swift");
        }

        let lib_core_h = lib_dir.join("SwiftBridgeCore.h");
        let shared_core_h = generated_dir.join("SwiftBridgeCore.h");
        if lib_core_h.exists() {
            fs::rename(&lib_core_h, &shared_core_h).expect("Failed to move SwiftBridgeCore.h");
        }

        // Add import CLingXiaLib to the generated Swift files
        let add_import_if_missing = |file_path: &std::path::Path, file_name: &str| {
            if let Ok(contents) = fs::read_to_string(file_path) {
                if !contents.contains("import CLingXiaRustAPI") {
                    let new_contents =
                        format!("import Foundation\nimport CLingXiaRustAPI\n\n{}", contents);
                    fs::write(file_path, new_contents).expect(&format!(
                        "Failed to add import statement to {} file",
                        file_name
                    ));
                }
            }
        };

        // 1. Add to main LingXiaLib.swift file
        let swift_file_path = lib_dir
            .join(package_name)
            .join(format!("{}.swift", package_name));
        add_import_if_missing(&swift_file_path, "LingXiaLib.swift");

        // 2. Add to SwiftBridgeCore.swift file in generated root
        let core_file_path = generated_dir.join("SwiftBridgeCore.swift");
        add_import_if_missing(&core_file_path, "SwiftBridgeCore.swift");

        // Create lib-specific module.modulemap
        let lib_modulemap_content = format!(
            r#"module CLingXiaRustAPI {{
    header "../SwiftBridgeCore.h"
    header "{package_name}/{package_name}.h"
    export *
}}"#,
            package_name = package_name
        );

        fs::write(lib_dir.join("module.modulemap"), lib_modulemap_content)
            .expect("Failed to write lib module.modulemap");

        println!(
            "cargo:warning=LingXiaRustAPI Swift bridge files generated to {}",
            lib_dir.display()
        );
    }
}
