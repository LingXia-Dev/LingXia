use std::env;
use std::fs;
use std::path::PathBuf;

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

        // Generate to lingxia-sdk/apple/Sources/generated/platform
        let lingxia_root = manifest_dir.parent().unwrap();
        let sources_dir = lingxia_root
            .join("lingxia-sdk")
            .join("apple")
            .join("Sources");
        let generated_dir = sources_dir.join("generated");
        let platform_dir = generated_dir.join("LingXiaSwiftAPI");

        // Create destination directories
        std::fs::create_dir_all(&platform_dir).expect("Failed to create platform directory");

        // Generate Swift bridge files to platform directory
        let bridges = swift_bridge_build::parse_bridges(vec!["src/apple/ffi.rs"]);
        bridges.write_all_concatenated(&platform_dir, package_name);

        // Remove SwiftBridgeCore files since lib will generate them
        let core_swift = platform_dir.join("SwiftBridgeCore.swift");
        if core_swift.exists() {
            fs::remove_file(&core_swift).expect("Failed to remove SwiftBridgeCore.swift");
        }

        let core_h = platform_dir.join("SwiftBridgeCore.h");
        if core_h.exists() {
            fs::remove_file(&core_h).expect("Failed to remove SwiftBridgeCore.h");
        }

        // Fix the generated header file to include stdint.h for uintptr_t
        let header_file = platform_dir
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
        let swift_file_path = platform_dir
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

        fs::write(
            platform_dir.join("module.modulemap"),
            platform_modulemap_content,
        )
        .expect("Failed to write platform module.modulemap");

        println!(
            "cargo:warning=LingXiaSwiftAPI Swift bridge files generated to {}",
            platform_dir.display()
        );
    }
}
