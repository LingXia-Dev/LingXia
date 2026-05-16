use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-env-changed=LINGXIA_NATIVE_CLIENT_OUT");

    let Some(out) = std::env::var_os("LINGXIA_NATIVE_CLIENT_OUT") else {
        return;
    };

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let rust_dir = manifest_dir.join("src");
    let out = PathBuf::from(out);
    let out = if out.is_absolute() {
        out
    } else {
        manifest_dir.join(out)
    };

    if let Err(err) = lingxia_native_codegen::generate_native_client_from_paths(&rust_dir, &out) {
        panic!("failed to generate LingXia native client: {err:#}");
    }
}
