use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/local_control/launcher_stub.rs");
    println!("cargo:rerun-if-env-changed=RUSTC");
    println!("cargo:rerun-if-env-changed=RUSTC_LINKER");
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || std::env::var_os("CARGO_FEATURE_LOCAL_CONTROL").is_none()
    {
        return;
    }

    // Keep the forwarder inside the runtime so existing host projects gain the
    // Windows command without adding a companion binary target. Signed-only
    // deployments still need to package and sign this extracted helper.
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let output =
        PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("lingxia-control-launcher.exe");
    let target = std::env::var("TARGET").unwrap();
    let mut rustc = Command::new(std::env::var_os("RUSTC").unwrap());
    rustc
        .arg(manifest.join("src/local_control/launcher_stub.rs"))
        .args(["--crate-name", "lingxia_control_launcher"])
        .args(["--crate-type", "bin"])
        .args(["--edition", "2024"])
        .args(["--target", &target])
        .args(["-C", "opt-level=z"])
        .args(["-C", "debuginfo=0"])
        .args(["-C", "strip=symbols"])
        .args(["-C", "panic=abort"])
        .args(["-D", "warnings"])
        .arg("-o")
        .arg(&output);

    if let Some(linker) = std::env::var_os("RUSTC_LINKER").filter(|value| !value.is_empty()) {
        let mut linker_arg = std::ffi::OsString::from("linker=");
        linker_arg.push(linker);
        rustc.arg("-C").arg(linker_arg);
    }

    if let Some(flags) = std::env::var_os("CARGO_ENCODED_RUSTFLAGS") {
        for flag in flags
            .to_string_lossy()
            .split('\u{1f}')
            .filter(|flag| !flag.is_empty())
        {
            rustc.arg(flag);
        }
    }

    let result = rustc
        .output()
        .expect("failed to start rustc for the Windows launcher");
    if !result.status.success() {
        panic!(
            "failed to build the Windows control launcher:\n{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
