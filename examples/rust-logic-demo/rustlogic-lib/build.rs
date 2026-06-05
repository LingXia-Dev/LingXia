use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-env-changed=LINGXIA_NATIVE_CLIENT_OUT");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let rust_dir = manifest_dir.join("src");

    // Optional TS client — emitted only when the lxapp bundler sets the env var.
    if let Some(out) = std::env::var_os("LINGXIA_NATIVE_CLIENT_OUT") {
        let out = PathBuf::from(out);
        let out = if out.is_absolute() {
            out
        } else {
            manifest_dir.join(out)
        };
        if let Err(err) = lingxia_native_codegen::generate_ts_client(&rust_dir, &out) {
            panic!("failed to generate LingXia native client: {err:#}");
        }
    }

    // Rust auto-register module — always emitted; `lib.rs` `include!`s it.
    let handlers_rs =
        PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("lingxia_native_handlers.rs");
    if let Err(err) = lingxia_native_codegen::generate_rust_registry(&rust_dir, &handlers_rs) {
        panic!("failed to generate LingXia native handler registry: {err:#}");
    }
}
