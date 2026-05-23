use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-env-changed=LINGXIA_NATIVE_CLIENT_OUT");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let rust_dir = manifest_dir.join("src");

    // 1) Optional TypeScript client (driven by the lxapp bundler via env var).
    if let Some(out) = std::env::var_os("LINGXIA_NATIVE_CLIENT_OUT") {
        let out = PathBuf::from(out);
        let out = if out.is_absolute() {
            out
        } else {
            manifest_dir.join(out)
        };
        if let Err(err) =
            lingxia_native_codegen::generate_native_client_from_paths(&rust_dir, &out)
        {
            panic!("failed to generate LingXia native client: {err:#}");
        }
    }

    // 2) Rust auto-register module — always emitted to OUT_DIR. lib.rs
    //    `include!`s it and calls `__lingxia_native::install()` from
    //    HostAddon::install_host_apis, so adding a new `#[lingxia::native]`
    //    requires no manual register_host_entry line.
    let handlers_rs =
        PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("lingxia_native_handlers.rs");
    if let Err(err) =
        lingxia_native_codegen::generate_native_client_from_paths(&rust_dir, &handlers_rs)
    {
        panic!("failed to generate LingXia native handler registry: {err:#}");
    }
}
