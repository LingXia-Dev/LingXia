use crate::platform::BuildProfile;
use crate::platform::doctor::command_version_line;
use anyhow::{Context, Result, anyhow};
use std::path::Path;
use std::process::Command;

/// Parse build profile from `--release` style flags.
pub fn resolve_build_profile(release: bool) -> BuildProfile {
    if release {
        BuildProfile::Release
    } else {
        BuildProfile::Debug
    }
}

/// Apply common cargo build switches for profile.
pub fn apply_cargo_profile(cmd: &mut Command, profile: BuildProfile) {
    if matches!(profile, BuildProfile::Release) {
        cmd.arg("--release");
    }
}

pub fn cargo_version_line() -> Option<String> {
    command_version_line("cargo", &["--version"], false)
}

/// Execute `cargo rustc --lib --crate-type=<crate_type>` for a target.
///
/// Host `native` crates list `cdylib` + `staticlib` + `rlib` so one manifest
/// covers every platform. `cargo build` rustc's that crate once per type.
/// Pass `cdylib` for Android/Harmony `.so`, `staticlib` for Apple `.a`.
pub fn run_cargo_rustc_for_target<F>(
    manifest_path: &Path,
    working_dir: &Path,
    target_dir: &Path,
    target: &str,
    crate_type: &str,
    package: Option<&str>,
    profile: BuildProfile,
    configure: F,
) -> Result<()>
where
    F: FnOnce(&mut Command),
{
    let mut cmd = cargo_rustc_command(
        manifest_path,
        working_dir,
        target_dir,
        target,
        crate_type,
        package,
        profile,
    );
    configure(&mut cmd);

    let status = cmd.status().context("Failed to execute cargo rustc")?;
    if !status.success() {
        return Err(anyhow!("Rust build failed for target: {}", target));
    }
    Ok(())
}

/// Execute `cargo rustc --crate-type=staticlib` for a target with shared LingXia defaults.
pub fn run_cargo_rustc_staticlib_for_target<F>(
    manifest_path: &Path,
    working_dir: &Path,
    target_dir: &Path,
    target: &str,
    profile: BuildProfile,
    configure: F,
) -> Result<()>
where
    F: FnOnce(&mut Command),
{
    run_cargo_rustc_for_target(
        manifest_path,
        working_dir,
        target_dir,
        target,
        "staticlib",
        None,
        profile,
        configure,
    )
}

fn cargo_rustc_command(
    manifest_path: &Path,
    working_dir: &Path,
    target_dir: &Path,
    target: &str,
    crate_type: &str,
    package: Option<&str>,
    profile: BuildProfile,
) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.arg("rustc")
        .arg("--lib")
        .arg(format!("--crate-type={crate_type}"))
        .arg("--target")
        .arg(target)
        .arg("--manifest-path")
        .arg(manifest_path)
        .env("CARGO_TARGET_DIR", target_dir)
        .current_dir(working_dir);

    if let Some(package_name) = package {
        cmd.arg("-p").arg(package_name);
    }

    apply_cargo_profile(&mut cmd, profile);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_args(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn apply_cargo_profile_only_adds_release_when_requested() {
        let mut cmd = Command::new("cargo");
        apply_cargo_profile(&mut cmd, BuildProfile::Debug);

        let args = command_args(&cmd);
        assert!(!args.iter().any(|a| a == "--no-default-features"));
        assert!(!args.iter().any(|a| a == "--release"));
    }

    #[test]
    fn rustc_command_emits_only_the_requested_crate_type() {
        let cmd = cargo_rustc_command(
            Path::new("native/Cargo.toml"),
            Path::new("native"),
            Path::new("target"),
            "aarch64-linux-android",
            "cdylib",
            None,
            BuildProfile::Debug,
        );
        let args = command_args(&cmd);
        assert_eq!(
            args,
            [
                "rustc",
                "--lib",
                "--crate-type=cdylib",
                "--target",
                "aarch64-linux-android",
                "--manifest-path",
                "native/Cargo.toml",
            ]
        );
        assert!(
            !args
                .iter()
                .any(|a| a.contains("staticlib") || *a == "build")
        );
    }

    #[test]
    fn rustc_command_forwards_package_and_release() {
        let cmd = cargo_rustc_command(
            Path::new("native/Cargo.toml"),
            Path::new("native"),
            Path::new("target"),
            "aarch64-unknown-linux-ohos",
            "cdylib",
            Some("native"),
            BuildProfile::Release,
        );
        let args = command_args(&cmd);
        assert!(args.windows(2).any(|w| w == ["-p", "native"]));
        assert!(args.iter().any(|a| a == "--release"));
        assert!(args.iter().any(|a| a == "--crate-type=cdylib"));
    }
}
