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
}
