use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // Track HEAD so the embedded sha follows the checkout; absent for
    // published crates, where the sha is intentionally left empty.
    if let Some(head) = git_output(&["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={head}");
    }
    println!(
        "cargo:rustc-env=LINGXIA_GIT_SHA_SHORT={}",
        git_output(&["rev-parse", "--short", "HEAD"]).unwrap_or_default()
    );
}

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!value.is_empty()).then_some(value)
}
