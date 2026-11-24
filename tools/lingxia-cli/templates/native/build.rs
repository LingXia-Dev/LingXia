use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    // Rust auto-register module; `lib.rs` `include!`s it. The lingxia CLI writes the TS clients.
    let handlers_rs =
        PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("lingxia_native_handlers.rs");
    if let Err(err) =
        lingxia_native_codegen::generate_rust_registry(&manifest_dir.join("src"), &handlers_rs)
    {
        panic!("failed to generate LingXia native handler registry: {err:#}");
    }
}
