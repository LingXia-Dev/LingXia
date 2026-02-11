use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/android/java");

    let target = env::var("TARGET").unwrap_or_default();

    if target.contains("android") {
        compile_android_java();
    }
}

fn compile_android_java() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let java_src_dir = manifest_dir.join("src").join("android").join("java");

    if !java_src_dir.exists() {
        return;
    }

    // Check if ANDROID_SDK_ROOT is set
    if env::var("ANDROID_SDK_ROOT").is_err() {
        println!("cargo:error=ANDROID_SDK_ROOT is not set");
        return;
    }

    // Get JAR output directory from environment variable (must be set by build script)
    let jar_output_dir = match env::var("LINGXIA_JAR_OUTPUT_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            println!("cargo:error=LINGXIA_JAR_OUTPUT_DIR is not set");
            return;
        }
    };

    // Call Makefile to build JAR
    // ANDROID_SDK_ROOT must be set in the environment before running cargo build
    let mut make_cmd = Command::new("make");
    make_cmd
        .current_dir(&java_src_dir)
        .env("TARGET_DIR", &jar_output_dir);

    let output = make_cmd.output().expect("Failed to execute make");
    if !output.status.success() {
        panic!("Make failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    // JAR output path
    let jar_output = jar_output_dir.join("lingxia-webview.jar");

    println!("cargo:warning=Created JAR: {}", jar_output.display());
}
