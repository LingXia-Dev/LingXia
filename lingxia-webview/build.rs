use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let target = env::var("TARGET").unwrap_or_default();

    if target.contains("apple") {
        let package_name = "LingXiaFFI";
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let sources_dir = manifest_dir.join("ios").join("lingxia").join("Sources");
        let generated_dir = sources_dir.join("generated");

        // Create destination directories
        std::fs::create_dir_all(&generated_dir).expect("Failed to create generated directory");

        // Generate Swift bridge files to generated directory
        swift_bridge_build::parse_bridges(vec!["src/apple/ffi.rs"])
            .write_all_concatenated(&generated_dir, package_name);

        // Add import CLingXiaFFI to the generated Swift files
        let add_import_if_missing = |file_path: &std::path::Path, file_name: &str| {
            if let Ok(contents) = fs::read_to_string(file_path) {
                if !contents.contains("import CLingXiaFFI") {
                    let new_contents =
                        format!("import Foundation\nimport CLingXiaFFI\n\n{}", contents);
                    fs::write(file_path, new_contents).expect(&format!(
                        "Failed to add import statement to {} file",
                        file_name
                    ));
                }
            }
        };

        // 1. Add to main LingxiaFFI.swift file
        let swift_file_path = generated_dir
            .join(package_name)
            .join(format!("{}.swift", package_name));
        add_import_if_missing(&swift_file_path, "LingxiaFFI.swift");

        // 2. Add to SwiftBridgeCore.swift file
        let core_file_path = generated_dir.join("SwiftBridgeCore.swift");
        add_import_if_missing(&core_file_path, "SwiftBridgeCore.swift");

        // Create module.modulemap in generated directory
        let modulemap_content = format!(
            r#"module C{package_name} {{
    header "SwiftBridgeCore.h"
    header "{package_name}/{package_name}.h"
    export *
}}"#,
            package_name = package_name
        );

        fs::write(generated_dir.join("module.modulemap"), modulemap_content)
            .expect("Failed to write module.modulemap");

        // println!( "cargo:warning=Swift bridge files generated to {}", generated_dir.display());
    }

    let env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target.contains("linux") && env.eq("ohos") {
        napi_build_ohos::setup();
    }
}
