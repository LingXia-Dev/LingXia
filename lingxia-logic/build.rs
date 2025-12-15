use lingxia_gen::i18n::{I18nConfig, run};
use std::env;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // Rerun if i18n files change
    println!("cargo:rerun-if-changed=../i18n");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let config = I18nConfig {
        input: root.join("../i18n"),
        rust_out: Some(out_dir.join("i18n_generated.rs")),
        android_out: None,
        ios_out: None,
        harmony_out: None,
    };

    if let Err(e) = run(config) {
        eprintln!("Failed to generate i18n resources: {}", e);
        std::process::exit(1);
    }
}
